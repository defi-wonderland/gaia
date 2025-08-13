//! This module defines the `ProcessActions` trait for processing raw action events.
//! It provides an interface for transforming `ActionEvent` data into structured `Action` data.
use crate::errors::ProcessorError;
use actions_indexer_shared::types::{ActionEvent, Action};

mod actions_processor;

pub use actions_processor::ActionsProcessor;

/// Defines the interface for processing raw `ActionEvent` data into structured `Action` data.
///
/// Implementors of this trait are responsible for applying business logic and transformations
/// to the incoming action events.
pub trait ProcessActions {
    /// Processes a slice of `ActionEvent`s and returns a vector of `Action`s.
    ///
    /// This method takes an array of raw `ActionEvent`s, applies necessary processing rules,
    /// and converts them into a structured `Action` format. It returns a `Vec<Action>` on successful processing.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `ActionEvent`s to be processed.
    ///
    /// # Returns
    ///
    /// A `Vec<Action>` on successful processing.
    fn process(&self, actions: &[ActionEvent]) -> Vec<Action>;
}

/// Defines the interface for handling a single `ActionEvent` and converting it into a structured `Action` data.
///
/// Implementors of this trait are responsible for applying business logic and transformations
/// to the incoming action event.
pub trait HandleAction: Send + Sync {
    /// Handles a single `ActionEvent` and converts it into a structured `Action` data.
    ///
    /// This method takes an `ActionEvent` and applies necessary business logic and transformations
    /// to convert it into a structured `Action` format. It returns a `Result` containing the processed `Action` on successful processing or a `ProcessorError` if an error occurs.
    ///
    /// # Arguments
    ///
    /// * `action` - The `ActionEvent` to be processed.
    ///
    /// # Returns
    ///
    /// A `Result` containing the processed `Action` on successful processing or a `ProcessorError` if an error occurs.
    fn handle(&self, action: &ActionEvent) -> Result<Action, ProcessorError>;
}
