//! Orchestrator module for the search indexer ingest.
//!
//! Coordinates the consumer, processor, and loader components.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, instrument, warn};

use crate::consumer::{KafkaConsumer, StreamMessage};
use crate::errors::IngestError;
use crate::loader::SearchLoader;
use crate::processor::EntityProcessor;

/// Configuration for the orchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Size of the message channel buffer.
    pub channel_buffer_size: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: 1000,
        }
    }
}

/// Orchestrator that coordinates the ingest components.
///
/// The orchestrator:
/// - Manages the lifecycle of ingest components
/// - Routes messages between components
/// - Handles shutdown signals
/// - Monitors ingest health
pub struct Orchestrator {
    consumer: Arc<KafkaConsumer>,
    processor: EntityProcessor,
    loader: SearchLoader,
    config: OrchestratorConfig,
    shutdown_tx: broadcast::Sender<()>,
    /// Total number of events processed since startup.
    total_events_processed: Arc<AtomicU64>,
    /// Total number of documents indexed since startup.
    total_documents_indexed: Arc<AtomicU64>,
}

impl Orchestrator {
    /// Create a new orchestrator with the given components.
    pub fn new(consumer: KafkaConsumer, processor: EntityProcessor, loader: SearchLoader) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            consumer: Arc::new(consumer),
            processor,
            loader,
            config: OrchestratorConfig::default(),
            shutdown_tx,
            total_events_processed: Arc::new(AtomicU64::new(0)),
            total_documents_indexed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a new orchestrator with custom configuration.
    pub fn with_config(
        consumer: KafkaConsumer,
        processor: EntityProcessor,
        loader: SearchLoader,
        config: OrchestratorConfig,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            consumer: Arc::new(consumer),
            processor,
            loader,
            config,
            shutdown_tx,
            total_events_processed: Arc::new(AtomicU64::new(0)),
            total_documents_indexed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Run the orchestrator.
    ///
    /// This method starts all ingest components and coordinates message flow.
    /// It blocks until a shutdown signal is received or an error occurs.
    #[instrument(skip(self))]
    pub async fn run(&mut self) -> Result<(), IngestError> {
        info!("Starting search indexer orchestrator");

        // Check if loader is ready
        self.loader.check_ready().await?;

        // Subscribe to Kafka topics
        self.consumer.subscribe()?;

        // Create event channel
        let (event_transmitter, mut event_receiver) =
            mpsc::channel::<StreamMessage>(self.config.channel_buffer_size);

        // Create acknowledgment channel
        let (ack_transmitter, ack_receiver) =
            mpsc::channel::<StreamMessage>(self.config.channel_buffer_size);

        // Start consumer in background
        let consumer = self.consumer.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        let consumer_handle = tokio::spawn(async move {
            if let Err(e) = consumer
                .run(event_transmitter, ack_receiver, shutdown_rx)
                .await
            {
                error!(error = %e, "Consumer error");
            }
        });

        // Process messages
        info!("Ready to process events from Kafka");

        // Set up progress logging timer (every 10 seconds)
        let total_events = Arc::clone(&self.total_events_processed);
        let total_docs = Arc::clone(&self.total_documents_indexed);
        let mut progress_timer = interval(Duration::from_secs(10));
        progress_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Track previous values for rate calculation
        let mut prev_events: u64 = 0;
        let mut prev_docs: u64 = 0;
        let mut prev_time = std::time::Instant::now();

        loop {
            tokio::select! {
                msg = event_receiver.recv() => {
                    match msg {
                        Some(StreamMessage::Events { events, offsets }) => {
                            info!(
                                event_count = events.len(),
                                offset_count = offsets.len(),
                                "Received events from consumer"
                            );
                            match self.process_events(events).await {
                                Ok(()) => {
                                    // Send success acknowledgment
                                    let _ = ack_transmitter.send(StreamMessage::Acknowledgment {
                                        offsets,
                                        success: true,
                                        error: None,
                                    }).await;
                                }
                                Err(e) => {
                                    error!(error = %e, "Failed to process events. Sending NACK to broker");
                                    // Send failure acknowledgment
                                    let _ = ack_transmitter.send(StreamMessage::Acknowledgment {
                                        offsets,
                                        success: false,
                                        error: Some(e.to_string()),
                                    }).await;
                                }
                            }
                        }
                        Some(StreamMessage::Error(e)) => {
                            error!(error = %e, "Received error from consumer");
                        }
                        Some(StreamMessage::End) | None => {
                            info!("Consumer stream ended");
                            break;
                        }
                        Some(StreamMessage::Acknowledgment { .. }) => {
                            // Ignore acknowledgments received on the wrong channel
                            warn!("Received acknowledgment on event channel (should be on ack channel)");
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal");
                    let _ = self.shutdown_tx.send(());
                    break;
                }
                _ = progress_timer.tick() => {
                    let events = total_events.load(Ordering::Relaxed);
                    let docs = total_docs.load(Ordering::Relaxed);

                    // Calculate rates per second
                    let now = std::time::Instant::now();
                    let elapsed_secs = now.duration_since(prev_time).as_secs_f64();

                    let events_per_sec = if elapsed_secs > 0.0 {
                        (events.saturating_sub(prev_events) as f64) / elapsed_secs
                    } else {
                        0.0
                    };

                    let docs_per_sec = if elapsed_secs > 0.0 {
                        (docs.saturating_sub(prev_docs) as f64) / elapsed_secs
                    } else {
                        0.0
                    };

                    info!(
                        events_processed = events,
                        documents_indexed = docs,
                        events_per_sec = format!("{:.2}", events_per_sec),
                        documents_per_sec = format!("{:.2}", docs_per_sec),
                        "Processing progress"
                    );

                    // Update previous values for next calculation
                    prev_events = events;
                    prev_docs = docs;
                    prev_time = now;
                }
            }
        }

        // Don't flush remaining documents on shutdown - we can't ACK them to Kafka anyway
        // since the consumer is shutting down. Any pending documents will be re-consumed
        // on next startup (at-least-once delivery semantics).

        // Wait for consumer to finish
        let _ = consumer_handle.await;

        let final_events = self.total_events_processed.load(Ordering::Relaxed);
        let final_docs = self.total_documents_indexed.load(Ordering::Relaxed);
        info!(
            total_events_processed = final_events,
            total_documents_indexed = final_docs,
            "Orchestrator shutdown complete"
        );
        Ok(())
    }

    /// Process a batch of events through the ingest.
    ///
    /// This method ensures documents are actually indexed in OpenSearch before returning Ok.
    /// The caller should only ACK to Kafka after this method returns successfully.
    async fn process_events(
        &mut self,
        events: Vec<crate::consumer::EntityEvent>,
    ) -> Result<(), IngestError> {
        let event_count = events.len();
        self.total_events_processed
            .fetch_add(event_count as u64, Ordering::Relaxed);

        debug!(event_count = event_count, "Processing batch of events");

        // Transform events to documents
        let processed = self.processor.process_batch(events)?;

        if processed.is_empty() {
            debug!("No documents to index after processing");
            return Ok(());
        }

        // Count documents to be indexed
        let index_count = processed
            .iter()
            .filter(|e| matches!(e, crate::processor::ProcessedEvent::Index(_)))
            .count();
        self.total_documents_indexed
            .fetch_add(index_count as u64, Ordering::Relaxed);

        // Load into search index (adds to pending buffer)
        self.loader.load(processed).await?;

        // Always flush to OpenSearch before returning Ok.
        // This ensures we only ACK to Kafka after documents are actually indexed.
        self.loader.flush().await?;

        Ok(())
    }

    /// Trigger a graceful shutdown.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}
