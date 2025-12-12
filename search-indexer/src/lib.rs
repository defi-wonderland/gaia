//! # Search Indexer
//!
//! Search indexer for the Geo Knowledge Graph - consumes events from Kafka
//! and indexes them into OpenSearch.
//!
//! ## Architecture
//!
//! The indexer follows the Consumer-Processor-Loader pattern:
//!
//! 1. **Consumer**: Receives events from Kafka
//! 2. **Processor**: Transforms events into search documents
//! 3. **Loader**: Indexes documents into OpenSearch
//! 4. **Orchestrator**: Coordinates the ingest flow
//!
//! ## Modules
//!
//! - [`config`]: Configuration and dependency initialization
//! - [`consumer`]: Kafka consumer for entity events
//! - [`processor`]: Transforms events into documents
//! - [`loader`]: Indexes documents into OpenSearch
//! - [`orchestrator`]: Coordinates the ingest flow
//! - [`errors`]: Error types for the indexer

pub mod config;
pub mod consumer;
pub mod errors;
pub mod loader;
pub mod orchestrator;
pub mod processor;

pub use config::Dependencies;
pub use errors::IngestError;

use thiserror::Error;

/// Errors that can occur during indexer initialization or execution.
#[derive(Error, Debug)]
pub enum IndexingError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Ingest error.
    #[error("Ingest error: {0}")]
    IngestError(#[from] IngestError),
}

impl IndexingError {
    /// Create a configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }
}
