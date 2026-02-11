//! Dead Letter Queue (DLQ) module for the search indexer.
//!
//! Provides resilient error handling by routing failed operations to a Kafka
//! DLQ topic. Tracks poisoned entities in-memory (rebuilt from the DLQ topic
//! on startup) to provide observability into which entities have had failures.
//!
//! ## Components
//!
//! - [`types`]: DLQ record schema and configuration
//! - [`producer`]: Kafka producer for DLQ events
//! - [`state`]: Poisoned entity tracking (in-memory, rebuilt from DLQ topic)

pub mod producer;
pub mod state;
pub mod types;

pub use producer::{DlqProducer, DlqProducerLike};
pub use state::DlqState;
pub use types::{DlqConfig, DlqRecord};
