use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("Placeholder error - implementation pending")]
    Placeholder,
}
