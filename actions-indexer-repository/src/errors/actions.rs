//! Error types for the actions repository.
//! Defines specific errors that can occur during database operations related to actions.
use thiserror::Error;

/// Represents errors that can occur within the actions repository.
///
/// This enum consolidates various error conditions specific to database interactions,
/// such as SQLx errors during database operations.
#[derive(Debug, Error)]
pub enum ActionsRepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}