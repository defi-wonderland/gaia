#[derive(Debug, thiserror::Error)]
pub enum IndexingError {
    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] actions_indexer_pipeline::errors::OrchestratorError),
}
