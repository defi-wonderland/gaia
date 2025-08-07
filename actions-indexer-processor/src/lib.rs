mod errors;

use actions_indexer_types::models::{ActionEvent, Action};
use crate::errors::ProcessorError;

pub trait ProcessActions {
    fn process(&self, actions: &[ActionEvent]) -> Result<Vec<Action>, ProcessorError>;
}