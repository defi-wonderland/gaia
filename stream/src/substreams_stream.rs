use anyhow::{Error, anyhow};
use async_stream::try_stream;
use futures03::{Stream, StreamExt};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout};
use tokio_retry::strategy::ExponentialBackoff;
use tracing::{error, info, warn};

/// Max time to wait for the first usable block (BlockScopedData or BlockUndoSignal)
/// after a gRPC stream connects. If it elapses and we were resuming from a persisted
/// cursor, treat the cursor as stale: drop it and reconnect fresh.
///
/// The testnet produces ~1 block per 8 s, so 3 min = ~22 block-times — well past any
/// normal idle gap, but short enough to recover quickly when a cursor has aged out
/// of the server's fork database.
const STALE_CURSOR_WATCHDOG: Duration = Duration::from_secs(180);

use crate::pb::sf::substreams::rpc::v2::{
    BlockScopedData, BlockUndoSignal, Request, Response, response::Message,
};
use crate::pb::sf::substreams::v1::Modules;

use crate::substreams::SubstreamsEndpoint;

pub enum BlockResponse {
    New(BlockScopedData),
    Undo(BlockUndoSignal),
}

pub struct SubstreamsStream {
    stream: Pin<Box<dyn Stream<Item = Result<BlockResponse, Error>> + Send>>,
}

impl SubstreamsStream {
    pub fn new(
        endpoint: Arc<SubstreamsEndpoint>,
        cursor: Option<String>,
        modules: Option<Modules>,
        output_module_name: String,
        start_block: i64,
        end_block: u64,
    ) -> Self {
        SubstreamsStream {
            stream: Box::pin(stream_blocks(
                endpoint,
                cursor,
                modules,
                output_module_name,
                start_block,
                end_block,
            )),
        }
    }
}

// Create the Stream implementation that streams blocks with auto-reconnection.
fn stream_blocks(
    endpoint: Arc<SubstreamsEndpoint>,
    cursor: Option<String>,
    modules: Option<Modules>,
    output_module_name: String,
    start_block_num: i64,
    stop_block_num: u64,
) -> impl Stream<Item = Result<BlockResponse, Error>> {
    let mut latest_cursor = cursor.unwrap_or_default();
    let mut backoff = ExponentialBackoff::from_millis(500).max_delay(Duration::from_secs(45));
    let mut last_progress_report = Instant::now();
    // Tracks whether we've already fallen back to a cursorless reconnect in this
    // streaming session. Prevents oscillation — one fallback per poisoned cursor,
    // then we let normal backoff + the container's liveness probe handle it.
    let mut stale_cursor_fallback_used = false;

    try_stream! {
        loop {
            let module_names: Vec<_> = modules.as_ref()
                .map(|m| m.modules.iter().map(|x| x.name.as_str()).collect())
                .unwrap_or_default();

            println!("Blockstreams disconnected, connecting (endpoint {}, start block {}, stop block {}, cursor {}, output: {}, modules: {:?})",
                &endpoint,
                start_block_num,
                stop_block_num,
                &latest_cursor,
                &output_module_name,
                module_names
            );

            let result = endpoint.clone().substreams(Request {
                start_block_num,
                start_cursor: latest_cursor.clone(),
                stop_block_num,
                final_blocks_only: false,
                modules: modules.clone(),
                output_module: output_module_name.clone(),
                // There is usually no good reason for you to consume the stream development mode (so switching `true`
                // to `false`). If you do switch it, be aware that more than one output module will be send back to you,
                // and the current code in `process_block_scoped_data` (within your 'main.rs' file) expects a single
                // module.
                production_mode: true,
                debug_initial_store_snapshot_for_modules: vec![],
                noop_mode: false,

            }).await;

            match result {
                Ok(stream) => {
                    println!("Blockstreams connected");

                    let mut encountered_error = false;
                    // Inline stream consumption (replaces `for await response in stream`) so we can
                    // apply a watchdog timeout on the first block. If the stream sits silent after
                    // connecting — the known failure mode for a cursor that has aged out of the
                    // server's fork database — the watchdog kicks in, clears the cursor, and lets
                    // the outer loop reconnect without it.
                    let mut stream = Box::pin(stream);
                    let mut got_first_block = false;
                    loop {
                        let next = if got_first_block {
                            // After the first block, the stream is known-good. The server may
                            // legitimately idle at chain head, so no deadline here — just await.
                            stream.next().await
                        } else {
                            // Watchdog: enforce STALE_CURSOR_WATCHDOG until the first usable block.
                            match timeout(STALE_CURSOR_WATCHDOG, stream.next()).await {
                                Ok(n) => n,
                                Err(_elapsed) => {
                                    let had_cursor = !latest_cursor.is_empty();
                                    if had_cursor && !stale_cursor_fallback_used {
                                        warn!(
                                            event = "substreams.stale_cursor_fallback",
                                            endpoint = %endpoint,
                                            output_module = %output_module_name,
                                            start_block = start_block_num,
                                            stop_block = stop_block_num,
                                            watchdog_secs = STALE_CURSOR_WATCHDOG.as_secs(),
                                            stale_cursor_preview = %cursor_preview(&latest_cursor),
                                            "No blocks received after connect within watchdog window \
                                             while using a persisted cursor — assuming cursor aged out \
                                             of the server's fork database, dropping and reconnecting fresh"
                                        );
                                        latest_cursor = String::new();
                                        stale_cursor_fallback_used = true;
                                    } else {
                                        // Either already tried the fresh-cursor fallback this session,
                                        // or we started without a cursor. Treat as a normal connection
                                        // stall — let backoff + liveness probe handle it.
                                        warn!(
                                            event = "substreams.first_block_timeout",
                                            endpoint = %endpoint,
                                            output_module = %output_module_name,
                                            watchdog_secs = STALE_CURSOR_WATCHDOG.as_secs(),
                                            had_cursor = had_cursor,
                                            fallback_already_used = stale_cursor_fallback_used,
                                            "No blocks received after connect within watchdog window; reconnecting"
                                        );
                                    }
                                    encountered_error = true;
                                    break;
                                }
                            }
                        };
                        let response = match next {
                            Some(r) => r,
                            None => break,
                        };
                        match process_substreams_response(response, &mut last_progress_report).await {
                            BlockProcessedResult::BlockScopedData(block_scoped_data) => {
                                got_first_block = true;
                                stale_cursor_fallback_used = false;
                                // Reset backoff because we got a good value from the stream
                                backoff = ExponentialBackoff::from_millis(500).max_delay(Duration::from_secs(45));

                                let cursor = block_scoped_data.cursor.clone();
                                yield BlockResponse::New(block_scoped_data);

                                latest_cursor = cursor;
                            },
                            BlockProcessedResult::BlockUndoSignal(block_undo_signal) => {
                                got_first_block = true;
                                stale_cursor_fallback_used = false;
                                // Reset backoff because we got a good value from the stream
                                backoff = ExponentialBackoff::from_millis(500).max_delay(Duration::from_secs(45));

                                let cursor = block_undo_signal.last_valid_cursor.clone();
                                yield BlockResponse::Undo(block_undo_signal);

                                latest_cursor = cursor;
                            },
                            BlockProcessedResult::Skip() => {},
                            BlockProcessedResult::TonicError(status) => {
                                // Unauthenticated errors are not retried, we forward the error back to the
                                // stream consumer which handles it
                                if status.code() == tonic::Code::Unauthenticated {
                                    return Err(anyhow::Error::new(status.clone()))?;
                                }

                                if is_concurrent_stream_limit(&status) {
                                    error!(
                                        event = "substreams.concurrent_stream_limit_exceeded",
                                        endpoint = %endpoint,
                                        output_module = %output_module_name,
                                        start_block = start_block_num,
                                        stop_block = stop_block_num,
                                        cursor = %latest_cursor,
                                        status_code = %status.code(),
                                        error = %status,
                                        "Substreams stream rejected due to concurrent stream limit"
                                    );
                                }

                                println!("Received tonic error {:#}", status);
                                encountered_error = true;
                                break;
                            },
                        }
                    }

                    if !encountered_error {
                        println!("Stream completed, reached end block");
                        return
                    }
                },
                Err(e) => {
                    // We failed to connect and will try again; this is another
                    // case where we actually _want_ to back off in case we keep
                    // having connection errors.

                    println!("Unable to connect to endpoint: {:#}", e);
                    warn!(
                        event = "substreams.connection_failed",
                        endpoint = %endpoint,
                        output_module = %output_module_name,
                        start_block = start_block_num,
                        stop_block = stop_block_num,
                        cursor = %latest_cursor,
                        error = %e,
                        "Unable to connect to substreams endpoint"
                    );
                }
            }

            // If we reach this point, we must wait a bit before retrying
            if let Some(duration) = backoff.next() {
                sleep(duration).await
            } else {
                return Err(anyhow!("backoff requested to stop retrying, quitting"))?;
            }
        }
    }
}

fn is_concurrent_stream_limit(status: &tonic::Status) -> bool {
    status.code() == tonic::Code::ResourceExhausted
        && status
            .message()
            .contains("Concurrent stream limit exceeded")
}

/// Redact all but the first 16 characters of a cursor for logging.
///
/// Substreams cursors are opaque blobs that uniquely identify a block + fork state.
/// We log a prefix so stale-cursor fallback events are auditable in logs (e.g. when
/// looking up whether a given prod stall recycled its cursor), but not the full
/// ~220-character string — keeps log lines readable.
fn cursor_preview(cursor: &str) -> String {
    if cursor.is_empty() {
        "<empty>".to_string()
    } else if cursor.len() <= 16 {
        format!("{}…(len={})", cursor, cursor.len())
    } else {
        format!("{}…(len={})", &cursor[..16], cursor.len())
    }
}

enum BlockProcessedResult {
    Skip(),
    BlockScopedData(BlockScopedData),
    BlockUndoSignal(BlockUndoSignal),
    TonicError(tonic::Status),
}

async fn process_substreams_response(
    result: Result<Response, tonic::Status>,
    last_progress_report: &mut Instant,
) -> BlockProcessedResult {
    let response = match result {
        Ok(v) => v,
        Err(e) => return BlockProcessedResult::TonicError(e),
    };

    match response.message {
        Some(Message::Session(session)) => {
            println!(
                "Received session message (Workers {}, Trace ID {})",
                session.max_parallel_workers, &session.trace_id
            );
            info!(
                event = "substreams.session_started",
                max_parallel_workers = session.max_parallel_workers,
                trace_id = %session.trace_id,
                "Substreams session started"
            );
            BlockProcessedResult::Skip()
        }
        Some(Message::BlockScopedData(block_scoped_data)) => {
            BlockProcessedResult::BlockScopedData(block_scoped_data)
        }
        Some(Message::BlockUndoSignal(block_undo_signal)) => {
            BlockProcessedResult::BlockUndoSignal(block_undo_signal)
        }
        Some(Message::Progress(progress)) => {
            let processed_bytes = progress.processed_bytes.unwrap_or_default();

            // Show module stats
            let stats: Vec<_> = progress
                .modules_stats
                .iter()
                .map(|m| format!("{}: {} blocks", m.name, m.total_processed_block_count))
                .collect();

            if last_progress_report.elapsed() > Duration::from_secs(5) || stats.is_empty() {
                println!(
                    "Progress (Jobs: {}, Bytes: [R: {}, W: {}]) {}",
                    progress.running_jobs.len(),
                    processed_bytes.total_bytes_read,
                    processed_bytes.total_bytes_written,
                    if stats.is_empty() {
                        "initializing...".to_string()
                    } else {
                        stats.join(", ")
                    }
                );
                *last_progress_report = Instant::now();
            }

            // The `ModulesProgress` messages goal is to report active parallel processing happening
            // either to fill up backward (relative to your request's start block) some missing state
            // or pre-process forward blocks (again relative).
            //
            // You could log that in trace or accumulate to push as metrics. Here a snippet of code
            // that prints progress to standard out. If your `BlockScopedData` messages seems to never
            // arrive in production mode, it's because progresses is happening but not yet for the output
            // module you requested.
            //
            // let progresses: Vec<_> = progress
            //     .modules
            //     .iter()
            //     .filter_map(|module| {
            //         use crate::pb::sf::substreams::rpc::v2::module_progress::Type;

            //         if let Type::ProcessedRanges(range) = module.r#type.as_ref().unwrap() {
            //             Some(format!(
            //                 "{} @ [{}]",
            //                 module.name,
            //                 range
            //                     .processed_ranges
            //                     .iter()
            //                     .map(|x| x.to_string())
            //                     .collect::<Vec<_>>()
            //                     .join(", ")
            //             ))
            //         } else {
            //             None
            //         }
            //     })
            //     .collect();

            // println!("Progess {}", progresses.join(", "));

            BlockProcessedResult::Skip()
        }
        None => {
            println!("Got None on substream message");
            BlockProcessedResult::Skip()
        }
        _ => BlockProcessedResult::Skip(),
    }
}

impl Stream for SubstreamsStream {
    type Item = Result<BlockResponse, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod cursor_preview_tests {
    use super::cursor_preview;

    #[test]
    fn empty_cursor_renders_sentinel() {
        assert_eq!(cursor_preview(""), "<empty>");
    }

    #[test]
    fn short_cursor_preserved_with_length() {
        assert_eq!(cursor_preview("abc"), "abc…(len=3)");
    }

    #[test]
    fn long_cursor_truncated_with_length() {
        // Matches the shape of real Pinax cursors we'd encounter in prod.
        let full =
            "sLwuz9N_53_pxBpo1pyHhKWwLpcyB1hsUgLnIBJF09qj8CaQ28ujVGJ2YR-Dwvjzj0HoGln41omcFX559slU6dS_";
        let out = cursor_preview(full);
        assert!(out.starts_with("sLwuz9N_53_pxBpo"), "prefix: {out}");
        assert!(out.contains(&format!("len={}", full.len())), "length annotation: {out}");
        // Don't leak more than 16 chars of the cursor.
        assert!(!out.contains("HhKWw"), "leaked beyond 16-char prefix: {out}");
    }

    #[test]
    fn cursor_of_exactly_16_chars_still_annotated() {
        // Defensive: the `len > 16` branch takes care of the common case; this
        // exercises the `<= 16` branch so regressions in future refactors don't
        // accidentally panic via out-of-bounds slicing.
        assert_eq!(cursor_preview("0123456789abcdef"), "0123456789abcdef…(len=16)");
    }
}
