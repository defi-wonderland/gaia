use actions_indexer_consumer::ConsumeActions;
use actions_indexer_loader::ActionsRepository;
use actions_indexer_processor::ProcessActions;
use actions_indexer_types::errors::IndexingError;

pub struct Dependencies {
    pub consumer: Box<dyn ConsumeActions>,
    pub processor: Box<dyn ProcessActions>,
    pub loader: Box<dyn ActionsRepository<sqlx::Postgres>>,
}

impl Dependencies {
    pub async fn new() -> Result<Self, IndexingError> {
        todo!("Dependencies::new() implementation required")
    }
}