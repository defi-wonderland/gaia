use actions_indexer_pipeline::consumer::ConsumeActions;
use actions_indexer_pipeline::loader::ActionsLoader;
use actions_indexer_pipeline::processor::ProcessActions;
use crate::errors::IndexingError;

pub struct Dependencies {
    pub consumer: Box<dyn ConsumeActions>,
    pub processor: Box<dyn ProcessActions>,
    pub loader: Box<ActionsLoader>,
}

impl Dependencies {
    pub async fn new() -> Result<Self, IndexingError> {
        todo!("Dependencies::new() implementation required")
    }
}