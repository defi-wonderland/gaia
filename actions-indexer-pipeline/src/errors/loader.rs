//! Error types for the loader module of the Actions Indexer Pipeline.
//! Defines specific errors that can occur during the loading and persistence
//! of processed action data.
use thiserror::Error;
use actions_indexer_repository::ActionsRepositoryError;

/// Represents errors that can occur within the action loader.
///
/// This enum consolidates various error conditions specific to the loading
/// process, including errors propagated from the actions repository.
#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Repository error: {0}")]
    Repository(#[from] ActionsRepositoryError),
}
