use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConsumerError {
    #[error("Placeholder error - implementation pending")]
    Placeholder,
}
