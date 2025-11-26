// CSV Loader - Bulk load ranks from CSV file by sending to server
use base64::{engine::general_purpose, Engine as _};
use csv::ReaderBuilder;
use prost::Message;
use serde::Deserialize;
use std::env;
use std::time::Instant;
use tracing::{error, info, warn};
use wire::pb::grc20::Edit;

#[derive(Debug, Deserialize)]
struct RankRecord {
    rank_id: u32,
    rank_name: String,
    category: String,
    item_count: u32,
    encoded_edit: String,
}

#[derive(Debug, serde::Serialize)]
struct CacheRequest {
    data: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .compact()
        .init();

    // Get CSV file path from command line
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        error!("Usage: {} <csv_file_path>", args[0]);
        error!("Example: {} data/random-rankings.csv", args[0]);
        std::process::exit(1);
    }

    let csv_path = &args[1];
    let server_url = env::var("SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/cache".to_string());
    
    // Rate limiting: max requests per second (0 = no limit)
    let rate_limit_per_sec: u64 = env::var("RATE_LIMIT")
        .unwrap_or_else(|_| "50".to_string())
        .parse()
        .unwrap_or(50);

    info!("Loading ranks from CSV: {}", csv_path);
    info!("Target server: {}", server_url);
    info!("Rate limit: {} requests/sec (0 = unlimited)", rate_limit_per_sec);

    // Create HTTP client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Read CSV file
    let mut rdr = ReaderBuilder::new().has_headers(true).from_path(csv_path)?;

    let mut total_processed = 0;
    let mut total_failed = 0;
    let start_time = Instant::now();
    let mut last_request_time = Instant::now();
    let min_interval = if rate_limit_per_sec > 0 {
        std::time::Duration::from_millis(1000 / rate_limit_per_sec)
    } else {
        std::time::Duration::from_millis(0)
    };

    for (idx, result) in rdr.deserialize().enumerate() {
        let record: RankRecord = match result {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to parse CSV record {}: {:?}", idx + 1, e);
                total_failed += 1;
                continue;
            }
        };

        // Validate the encoded edit by decoding it
        let bytes = match general_purpose::STANDARD.decode(&record.encoded_edit) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(
                    "Failed to decode base64 for rank {}: {:?}",
                    record.rank_name, e
                );
                total_failed += 1;
                continue;
            }
        };

        // Verify it's a valid Edit protobuf
        if let Err(e) = Edit::decode(&bytes[..]) {
            error!(
                "Failed to decode Edit protobuf for rank {}: {:?}",
                record.rank_name, e
            );
            total_failed += 1;
            continue;
        }

        info!(
            "Sending rank #{}: {} ({} items)",
            record.rank_id, record.rank_name, record.item_count
        );

        // Apply rate limiting
        if rate_limit_per_sec > 0 {
            let elapsed = last_request_time.elapsed();
            if elapsed < min_interval {
                tokio::time::sleep(min_interval - elapsed).await;
            }
            last_request_time = Instant::now();
        }

        // Send to server
        let request_payload = CacheRequest {
            data: record.encoded_edit,
        };

        let response = match client
            .post(&server_url)
            .json(&request_payload)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to send rank {}: {:?}", record.rank_name, e);
                total_failed += 1;
                continue;
            }
        };

        if response.status().is_success() {
            total_processed += 1;
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!(
                "Failed to send rank {} (status {}): {}",
                record.rank_name, status, body
            );
            total_failed += 1;
        }

        // Progress update every 100 ranks
        if (idx + 1) % 100 == 0 {
            info!("Progress: {}/{} ranks processed", total_processed, idx + 1);
        }
    }

    let total_duration = start_time.elapsed();
    info!("=====================================");
    info!("CSV Import Complete!");
    info!("Total processed: {}", total_processed);
    info!("Total failed: {}", total_failed);
    info!("Total time: {:.2}s", total_duration.as_secs_f64());
    if total_processed > 0 {
        info!(
            "Average time per rank: {:.3}s",
            total_duration.as_secs_f64() / total_processed as f64
        );
    }
    info!("=====================================");

    Ok(())
}

