//! Error types for the Actions Indexer application.
//! Defines a comprehensive set of errors that can occur during the indexing process,
//! consolidating errors from various modules like the orchestrator.
#[derive(Debug, thiserror::Error)]
pub enum IndexingError {
    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] actions_indexer_pipeline::errors::OrchestratorError),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Repository error: {0}")]
    Repository(#[from] actions_indexer_repository::ActionsRepositoryError),
}
