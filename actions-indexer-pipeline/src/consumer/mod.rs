use actions_indexer_shared::types::ActionEvent;
use crate::errors::ConsumerError;

use tokio_stream::Stream;
use std::pin::Pin;

pub trait ConsumeActions {
    fn stream_events(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<Vec<ActionEvent>, ConsumerError>> + Send>>;
}
