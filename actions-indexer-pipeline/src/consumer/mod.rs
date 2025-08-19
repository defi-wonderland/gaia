//! Consumer module for the actions indexer pipeline.
//!
//! Provides the `ConsumeActions` trait for consuming blockchain action events
//! from data sources like substreams. Acts as the entry point for the pipeline,
//! feeding data to processing and loading components.

use crate::errors::ConsumerError;

pub mod stream;

use actions_indexer_shared::types::ActionRaw;
use async_trait::async_trait;
use stream::pb::sf::substreams::rpc::v2::BlockScopedData;
use stream::substreams_stream::SubstreamsStream;

/// Trait for consuming blockchain action events from data sources.
///
/// Provides a unified interface for different data sources (substreams, RPC, etc.).
#[async_trait]
pub trait ConsumeActions {
    /// Creates a stream for consuming blockchain action events.
    ///
    /// Returns a configured `SubstreamsStream` or an error if connection fails.
    async fn stream_events(&self) -> Result<SubstreamsStream, ConsumerError>;

    /// Decodes the block scoped data into a vector of `ActionRaw`s.
    ///
    /// Returns a vector of `ActionRaw`s or an error if the data cannot be decoded.
    async fn decode_block_scoped_data(&self, block_data: &BlockScopedData) -> Result<Vec<ActionRaw>, ConsumerError>;
}
