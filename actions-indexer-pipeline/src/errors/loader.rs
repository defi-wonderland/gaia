use thiserror::Error;
use actions_indexer_repository::ActionsRepositoryError;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Repository error: {0}")]
    Repository(#[from] ActionsRepositoryError),
}
