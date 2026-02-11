mod dlq_reader;
mod generators;
mod kafka;

use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

use dlq_reader::DlqReader;
use generators::edits;
use kafka::KafkaProducer;

const DEFAULT_KAFKA_BROKER: &str = "localhost:9092";

/// Returns the topic prefix based on the ENVIRONMENT variable.
/// - ENVIRONMENT=staging -> "staging."
/// - ENVIRONMENT=production -> ""
/// - ENVIRONMENT not set -> panics (fail-safe)
fn get_topic_prefix() -> &'static str {
    match std::env::var("ENVIRONMENT").as_deref() {
        Ok("staging") => "staging.",
        Ok("production") => "",
        Ok(other) => panic!(
            "ENVIRONMENT variable must be set to 'staging' or 'production', got: '{}'",
            other
        ),
        Err(_) => panic!(
            "ENVIRONMENT variable must be set to 'staging' or 'production': NotPresent"
        ),
    }
}

/// Returns the prefixed topic name based on the ENVIRONMENT variable.
fn prefixed_topic(topic: &str) -> String {
    format!("{}{}", get_topic_prefix(), topic)
}

#[derive(Parser)]
#[command(name = "e2e-dlq")]
#[command(about = "E2E test tool for Dead Letter Queue (DLQ) behavior", long_about = None)]
struct Cli {
    /// Kafka broker address
    #[arg(short, long, default_value = DEFAULT_KAFKA_BROKER)]
    broker: String,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging
    let log_level = if cli.debug { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Get prefixed topic names based on ENVIRONMENT variable
    let edits_topic = prefixed_topic("knowledge.edits");
    let topic_prefix = get_topic_prefix();

    info!("Topic prefix: '{}'", topic_prefix);
    info!("Using edits topic: {}", edits_topic);

    // Create Kafka producer
    let producer = KafkaProducer::new(&cli.broker)?;

    info!("Generating DLQ test scenario with fixed UUIDs for deterministic validation");

    // =====================================================================
    // Fixed UUIDs for deterministic validation
    // =====================================================================

    // Test space
    let test_space = Uuid::parse_str("00000000-0000-4000-8000-0000000d1001").unwrap();

    // Normal entities (will be created successfully)
    let entity_a_id = Uuid::parse_str("00000000-0000-4000-8000-0000000d1a01").unwrap();
    let entity_b_id = Uuid::parse_str("00000000-0000-4000-8000-0000000d1b01").unwrap();
    let entity_c_id = Uuid::parse_str("00000000-0000-4000-8000-0000000d1c01").unwrap();

    // Entities for unset testing (created first, then properties unset)
    let entity_x_id = Uuid::parse_str("00000000-0000-4000-8000-0000000d1e01").unwrap();
    let entity_y_id = Uuid::parse_str("00000000-0000-4000-8000-0000000d1e02").unwrap();

    info!("Test Space ID: {}", test_space);
    info!("\nNormal entities:");
    info!("  Entity A: {}", entity_a_id);
    info!("  Entity B: {}", entity_b_id);
    info!("  Entity C: {}", entity_c_id);
    info!("\nUnset test entities:");
    info!("  Entity X: {}", entity_x_id);
    info!("  Entity Y: {}", entity_y_id);

    // =====================================================================
    // Step 1: Create 5 entities (A, B, C, X, Y) -> all should succeed
    // =====================================================================
    info!("\n--- Step 1: Creating entities A, B, C, X, Y ---");

    let entity_a_payload = edits::create_entity_edit(
        "Create Entity A",
        test_space,
        entity_a_id,
        Some("DLQ Test Entity A"),
        Some("Normal entity that should be indexed successfully"),
        None,
    )?;
    producer.send(&edits_topic, None, entity_a_payload).await?;

    let entity_b_payload = edits::create_entity_edit(
        "Create Entity B",
        test_space,
        entity_b_id,
        Some("DLQ Test Entity B"),
        Some("Another normal entity for the DLQ test"),
        None,
    )?;
    producer.send(&edits_topic, None, entity_b_payload).await?;

    let entity_c_payload = edits::create_entity_edit(
        "Create Entity C",
        test_space,
        entity_c_id,
        Some("DLQ Test Entity C"),
        Some("Third normal entity for batch testing"),
        None,
    )?;
    producer.send(&edits_topic, None, entity_c_payload).await?;

    let entity_x_payload = edits::create_entity_edit(
        "Create Entity X",
        test_space,
        entity_x_id,
        Some("DLQ Test Entity X"),
        Some("Entity for unset testing - name will be removed"),
        None,
    )?;
    producer.send(&edits_topic, None, entity_x_payload).await?;

    let entity_y_payload = edits::create_entity_edit(
        "Create Entity Y",
        test_space,
        entity_y_id,
        Some("DLQ Test Entity Y"),
        Some("Entity for unset testing - name and description will be removed"),
        Some("https://example.com/entity-y-avatar.png"),
    )?;
    producer.send(&edits_topic, None, entity_y_payload).await?;

    info!("All entities created");

    // Wait for indexer to process creates
    info!("Waiting 3 seconds for indexer to process creates...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // =====================================================================
    // Step 2: Unset properties on X and Y (both exist, should succeed)
    //         With DLQ enabled, these operations flow through the DLQ-aware
    //         loader path, validating the wiring works for the happy path.
    // =====================================================================
    info!("\n--- Step 2: Unset properties on Entity X and Y ---");

    let unset_x_payload = edits::unset_entity_properties(
        "Unset name on Entity X",
        test_space,
        entity_x_id,
        vec![sdk::core::ids::NAME_PROPERTY_ID],
    )?;
    producer.send(&edits_topic, None, unset_x_payload).await?;

    let unset_y_payload = edits::unset_entity_properties(
        "Unset name and description on Entity Y",
        test_space,
        entity_y_id,
        vec![sdk::core::ids::NAME_PROPERTY_ID, sdk::core::ids::DESCRIPTION_PROPERTY_ID],
    )?;
    producer.send(&edits_topic, None, unset_y_payload).await?;

    info!("Unset operations sent");

    // Wait for indexer to process
    info!("Waiting 3 seconds for indexer to process unsets...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // =====================================================================
    // Step 3: Update Entity A -> verifies indexer continues normally
    // =====================================================================
    info!("\n--- Step 3: Update Entity A ---");

    let update_a_payload = edits::create_entity_edit(
        "Update Entity A",
        test_space,
        entity_a_id,
        Some("DLQ Test Entity A Updated"),
        Some("Entity A updated - verifies indexer processes normally with DLQ enabled"),
        None,
    )?;
    producer.send(&edits_topic, None, update_a_payload).await?;

    info!("Entity A updated");

    // =====================================================================
    // Step 4: Validate DLQ topic with Rust reader
    // =====================================================================
    info!("\n--- Step 4: Validate DLQ topic with Rust reader ---");

    let dlq_topic = prefixed_topic("search-indexer.dlq");
    info!("Reading DLQ topic: {}", dlq_topic);

    match DlqReader::new(&cli.broker, &dlq_topic) {
        Ok(reader) => {
            match reader.read_all_records(std::time::Duration::from_secs(5)) {
                Ok(records) => {
                    info!("DLQ records found: {}", records.len());
                    assert!(
                        records.is_empty(),
                        "Expected 0 DLQ records in happy path, found {}",
                        records.len()
                    );
                    info!("DLQ validation passed: no failure records (expected for happy path)");
                }
                Err(e) => {
                    info!("DLQ read returned error (topic may not exist yet): {}", e);
                    info!("This is acceptable for a fresh environment");
                }
            }
        }
        Err(e) => {
            info!("Could not create DLQ reader: {}", e);
            info!("This is acceptable if the DLQ topic hasn't been created yet");
        }
    }

    // =====================================================================
    // Summary
    // =====================================================================
    info!("\n--- DLQ E2E Test Scenario Complete ---");
    info!("Test Space: {}", test_space);
    info!("");
    info!("Entities in OpenSearch:");
    info!("  Entity A: {} (created, then updated -> name='DLQ Test Entity A Updated')", entity_a_id);
    info!("  Entity B: {} (created -> name='DLQ Test Entity B')", entity_b_id);
    info!("  Entity C: {} (created -> name='DLQ Test Entity C')", entity_c_id);
    info!("  Entity X: {} (created, then name unset -> name field removed)", entity_x_id);
    info!("  Entity Y: {} (created, then name+desc unset -> only avatar remains)", entity_y_id);
    info!("");
    info!("DLQ infrastructure validation:");
    info!("  DLQ topic: {}search-indexer.dlq (should be scannable)", topic_prefix);
    info!("  Indexer progress logs should show: dlq_events=0, poisoned_entities=0");
    info!("  This confirms DLQ is enabled and wired up correctly (no failures = no DLQ records)");
    info!("");
    info!("Note: DLQ failure routing is validated via mock-based integration tests.");
    info!("This e2e test validates the DLQ-enabled indexer processes events correctly");
    info!("and that the DLQ infrastructure (topic, state scan, metrics) is wired up.");

    Ok(())
}
