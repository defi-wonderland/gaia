//! Error types for the processor module of the Actions Indexer Pipeline.
//! Defines specific errors that can occur during the processing of action events.
use thiserror::Error;

/// Represents errors that can occur within the action processor.
///
/// This enum consolidates various error conditions specific to the processing
/// of actions, such as placeholder errors for unimplemented functionality.
#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("Invalid vote")]
    InvalidVote,
}
