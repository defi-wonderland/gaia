//! This module defines the `ProcessActions` trait for processing raw action events.
//! It provides an interface for transforming `ActionEvent` data into structured `Action` data.
use actions_indexer_shared::types::{ActionEvent, Action};
use crate::errors::ProcessorError;

/// Defines the interface for processing raw `ActionEvent` data into structured `Action` data.
///
/// Implementors of this trait are responsible for applying business logic and transformations
/// to the incoming action events.
pub trait ProcessActions {
    /// Processes a slice of `ActionEvent`s and returns a `Result` containing a vector of `Action`s.
    ///
    /// This method takes an array of raw `ActionEvent`s, applies necessary processing rules,
    /// and converts them into a structured `Action` format. It returns a `Result` that is
    /// `Ok(Vec<Action>)` on successful processing or a `ProcessorError` if an error occurs.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `ActionEvent`s to be processed.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(Vec<Action>)` on successful processing or a
    /// `ProcessorError` if an error occurs during processing.
    fn process(&self, actions: &[ActionEvent]) -> Result<Vec<Action>, ProcessorError>;
}