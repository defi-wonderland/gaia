use thiserror::Error;

#[derive(Debug, Error)]
pub enum ActionsRepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}