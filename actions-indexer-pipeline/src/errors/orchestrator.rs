//! Error types for the orchestrator module of the Actions Indexer Pipeline.
//! Defines specific errors that can occur during the orchestration process.
use thiserror::Error;
use crate::errors::consumer::ConsumerError;
use actions_indexer_repository::errors::ActionsRepositoryError;

/// Represents errors that can occur within the action orchestrator.
///
/// This enum consolidates various error conditions specific to the orchestration
/// process, such as placeholder errors for unimplemented functionality.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("Consumer error: {0}")]
    Consumer(#[from] ConsumerError),
    #[error("Actions repository error: {0}")]
    ActionsRepository(#[from] ActionsRepositoryError),
}