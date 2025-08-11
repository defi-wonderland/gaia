use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("Placeholder error - implementation pending")]
    Placeholder,
}