mod errors;

use actions_indexer_types::models::ActionEvent;
use crate::errors::ConsumerError;

use futures03::Stream;
use std::pin::Pin;

pub trait ConsumeActions {
    fn stream_events(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<Vec<ActionEvent>, ConsumerError>> + Send>>;
}
