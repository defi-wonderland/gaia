use indexer::{
    cache::{postgres::PostgresCache, properties_cache::PropertiesCache},
    error::IndexingError,
    storage::postgres::PostgresStorage,
    KgData, KgIndexer,
};
use std::{env, sync::Arc};

use axiom_rs::Client as AxiomClient;
use dotenv::dotenv;
use stream::{pb::sf::substreams::rpc::v2::BlockScopedData, PreprocessedSink};
use tracing::{error, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PKG_FILE: &str = "geo_substream.spkg";
const MODULE_NAME: &str = "geo_out";
const START_BLOCK: i64 = 67162;

use serde_json::{json, Value};
use std::sync::Mutex;

// Simple in-memory buffer for Axiom logs to batch them
static AXIOM_LOG_BUFFER: Mutex<Vec<Value>> = Mutex::new(Vec::new());

struct AxiomLayer {
    dataset: String,
}

impl AxiomLayer {
    fn new(dataset: String) -> Self {
        Self { dataset }
    }
}

use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

impl<S> Layer<S> for AxiomLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = JsonVisitor::new();
        event.record(&mut visitor);

        // Check if current event has block_number in its fields
        let mut block_number = None;
        if let Some(Value::Number(bn)) = visitor.fields.get("block_number") {
            block_number = bn.as_u64();
        } else if let Some(Value::Number(bn)) = visitor.fields.get("block") {
            block_number = bn.as_u64();
        }

        let mut log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": event.metadata().level().to_string(),
            "target": event.metadata().target(),
            "service": "gaia.indexer",
            "fields": visitor.fields
        });

        // Add block_number to top level if found
        if let Some(bn) = block_number {
            log_entry["block_number"] = json!(bn);
        }

        if let Ok(mut buffer) = AXIOM_LOG_BUFFER.lock() {
            buffer.push(log_entry);

            // Flush buffer when it gets large (simple batching)
            if buffer.len() >= 10 {
                let logs = buffer.drain(..).collect::<Vec<_>>();
                let dataset = self.dataset.clone();

                tokio::spawn(async move {
                    if let Ok(client) = AxiomClient::new() {
                        if let Err(e) = client.ingest(&dataset, logs).await {
                            eprintln!("Failed to send logs to Axiom: {}", e);
                        }
                    }
                });
            }
        }
    }
}

struct JsonVisitor {
    fields: serde_json::Map<String, Value>,
}

impl JsonVisitor {
    fn new() -> Self {
        Self {
            fields: serde_json::Map::new(),
        }
    }
}

impl tracing::field::Visit for JsonVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            Value::String(format!("{:?}", value)),
        );
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), Value::Bool(value));
    }
}

#[tokio::main]
async fn main() -> Result<(), IndexingError> {
    dotenv().ok();

    // Initialize tracing
    init_tracing()?;

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let storage = PostgresStorage::new(&database_url).await;

    match storage {
        Ok(result) => {
            let cache = PostgresCache::new().await?;
            let properties_cache = PropertiesCache::from_storage(&result).await?;

            let indexer = KgIndexer::new(result, cache, properties_cache);

            let endpoint_url =
                env::var("SUBSTREAMS_ENDPOINT").expect("SUBSTREAMS_ENDPOINT not set");

            info!(
                endpoint = %endpoint_url,
                package = PKG_FILE,
                module = MODULE_NAME,
                start_block = START_BLOCK,
                "Starting indexer"
            );

            let _result = indexer
                .run(&endpoint_url, PKG_FILE, MODULE_NAME, START_BLOCK, 0)
                .await;
        }
        Err(error) => {
            error!("Error initializing stream: {}", error);
        }
    }

    // Flush any remaining Axiom logs
    flush_axiom_logs().await;
    info!("Indexer shutting down");

    Ok(())
}

async fn flush_axiom_logs() {
    let axiom_dataset = env::var("AXIOM_DATASET").unwrap_or_else(|_| "gaia.indexer".to_string());

    if let Ok(mut buffer) = AXIOM_LOG_BUFFER.lock() {
        if !buffer.is_empty() {
            let logs = buffer.drain(..).collect::<Vec<_>>();
            if let Ok(client) = AxiomClient::new() {
                if let Err(e) = client.ingest(&axiom_dataset, logs).await {
                    eprintln!("Failed to flush logs to Axiom: {}", e);
                }
            }
        }
    }
}

fn init_tracing() -> Result<(), IndexingError> {
    // Check if Axiom token is available
    let axiom_token = env::var("AXIOM_TOKEN").ok();
    let axiom_dataset = env::var("AXIOM_DATASET").unwrap_or_else(|_| "gaia.indexer".to_string());

    let registry = tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "indexer=info,stream=info".into()),
    );

    if axiom_token.is_some() {
        // Set up tracing with Axiom layer
        let layers = registry.with(AxiomLayer::new(axiom_dataset.clone()));

        // Only add console logging in debug builds
        #[cfg(debug_assertions)]
        let layers = layers.with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .json(), // Use JSON format for structured logging
        );

        layers.init();

        #[cfg(debug_assertions)]
        info!(
            service_name = "gaia.indexer",
            service_version = env!("CARGO_PKG_VERSION"),
            axiom_dataset = axiom_dataset,
            "Tracing initialized with Axiom ingestion and console logging"
        );

        #[cfg(not(debug_assertions))]
        info!(
            service_name = "gaia.indexer",
            service_version = env!("CARGO_PKG_VERSION"),
            axiom_dataset = axiom_dataset,
            "Tracing initialized with Axiom ingestion only"
        );
    } else {
        // Only set up console tracing in debug builds
        #[cfg(debug_assertions)]
        {
            registry
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .json(), // Use JSON format for structured logging
                )
                .init();

            info!(
                service_name = "gaia.indexer",
                service_version = env!("CARGO_PKG_VERSION"),
                "Tracing initialized with console logging (AXIOM_TOKEN not set)"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            // In release mode without Axiom, just use a minimal registry
            registry.init();

            info!(
                service_name = "gaia.indexer",
                service_version = env!("CARGO_PKG_VERSION"),
                "Tracing initialized without console logging (release mode, AXIOM_TOKEN not set)"
            );
        }
    }

    Ok(())
}
