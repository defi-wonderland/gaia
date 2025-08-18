//! This module defines the `ProcessActions` trait for processing raw action events.
//! It provides an interface for transforming `ActionRaw` data into structured `Action` data.
use crate::errors::ProcessorError;
use actions_indexer_shared::types::{Action, ActionRaw};

mod actions_processor;

pub use actions_processor::ActionsProcessor;

/// Defines the interface for processing raw `ActionEvent` data into structured `Action` data.
///
/// Implementors of this trait are responsible for applying business logic and transformations
/// to the incoming action events.
pub trait ProcessActions {
    /// Processes a slice of `ActionRaw`s and returns a vector of `Action`s.
    ///
    /// This method takes an array of raw `ActionRaw`s, applies necessary processing rules,
    /// and converts them into a structured `Action` format. It returns a `Vec<Action>` on successful processing.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `ActionRaw`s to be processed.
    ///
    /// # Returns
    ///
    /// A `Vec<Action>` on successful processing.
    fn process(&self, actions: &[ActionRaw]) -> Vec<Action>;
}

/// Defines the interface for handling a single `ActionRaw` and converting it into a structured `Action` data.
///
/// Implementors of this trait are responsible for applying business logic and transformations
/// to the incoming action event.
pub trait HandleAction: Send + Sync {
    /// Handles a single `ActionRaw` and converts it into a structured `Action` data.
    ///
    /// This method takes an `ActionRaw` and applies necessary business logic and transformations
    /// to convert it into a structured `Action` format. It returns a `Result` containing the processed `Action` on successful processing or a `ProcessorError` if an error occurs.
    ///
    /// # Arguments
    ///
    /// * `action` - The `ActionRaw` to be processed.
    ///
    /// # Returns
    ///
    /// A `Result` containing the processed `Action` on successful processing or a `ProcessorError` if an error occurs.
    fn handle(&self, action: &ActionRaw) -> Result<Action, ProcessorError>;
}
