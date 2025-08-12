//! Error types for the consumer module of the Actions Indexer Pipeline.
//! Defines specific errors that can occur during the consumption of action events.
use thiserror::Error;

/// Represents errors that can occur within the action consumer.
///
/// This enum consolidates various error conditions specific to the consumption
/// process, such as placeholder errors for unimplemented functionality.
#[derive(Debug, Error)]
pub enum ConsumerError {
    #[error("Placeholder error - implementation pending")]
    Placeholder,
}
