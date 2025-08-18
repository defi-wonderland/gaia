//! This module defines the `ConsumeActions` trait for consuming action events.
//! It provides an interface for streaming action data, which can then be
//! processed and loaded by other components of the indexer pipeline.
use actions_indexer_shared::types::ActionRaw;
use crate::errors::ConsumerError;

use tokio_stream::Stream;
use std::pin::Pin;

pub trait ConsumeActions {
    /// Streams action events as a `Pin<Box<dyn Stream<Item = Result<Vec<ActionRaw>, ConsumerError>> + Send>>`.
    ///
    /// This method provides an asynchronous stream of `ActionRaw` vectors, allowing
    /// for efficient consumption of action data. Each item in the stream is a `Result`,
    /// which can contain a vector of `ActionRaw` on success or a `ConsumerError` on failure.
    ///
    /// # Returns
    ///
    /// A `Pin<Box<dyn Stream<Item = Result<Vec<ActionRaw>, ConsumerError>> + Send>>`
    /// representing the stream of action events.
    fn stream_events(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<Vec<ActionRaw>, ConsumerError>> + Send>>;
}
