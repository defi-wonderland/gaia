use actions_indexer_shared::types::{ActionEvent, Action};
use crate::errors::ProcessorError;

pub trait ProcessActions {
    fn process(&self, actions: &[ActionEvent]) -> Result<Vec<Action>, ProcessorError>;
}