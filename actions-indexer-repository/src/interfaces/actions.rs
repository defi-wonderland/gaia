//! This module defines the `ActionsRepository` trait, which provides an interface
//! for interacting with the underlying data store for actions, user votes, and vote counts.
//! It abstracts the database operations for persistence and retrieval.
use actions_indexer_shared::types::{Action, UserVote, VotesCount, Changeset};
use crate::errors::ActionsRepositoryError;

/// A trait that defines the interface for interacting with the actions data repository.
///
/// Implementors of this trait provide methods for inserting actions, updating user votes,
/// updating vote counts, and persisting entire changesets.
#[async_trait::async_trait]
pub trait ActionsRepository: Send + Sync {
    /// Inserts a slice of `Action` objects into the repository.
    ///
    /// This asynchronous method is responsible for persisting new action data.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `Action` objects to be inserted.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the insertion fails.
    async fn insert_actions(
        &self,
        actions: &[Action],
    ) -> Result<(), ActionsRepositoryError>;

    /// Updates a slice of `UserVote` objects in the repository.
    ///
    /// This asynchronous method is responsible for updating existing user vote data.
    ///
    /// # Arguments
    ///
    /// * `user_votes` - A slice of `UserVote` objects to be updated.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the update fails.
    async fn update_user_votes(
        &self,
        user_votes: &[UserVote],
    ) -> Result<(), ActionsRepositoryError>;

    /// Updates a slice of `VotesCount` objects in the repository.
    ///
    /// This asynchronous method is responsible for updating existing vote count data.
    ///
    /// # Arguments
    ///
    /// * `votes_counts` - A slice of `VotesCount` objects to be updated.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the update fails.
    async fn update_votes_counts(
        &self,
        votes_counts: &[VotesCount],
    ) -> Result<(), ActionsRepositoryError>;

    /// Persists a `Changeset` object to the repository.
    ///
    /// This asynchronous method handles the complete persistence of a changeset,
    /// which can include new actions, updated user votes, and updated vote counts.
    ///
    /// # Arguments
    ///
    /// * `changeset` - A reference to the `Changeset` object to be persisted.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the persistence fails.
    async fn persist_changeset(
        &self,
        changeset: &Changeset<'_>,
    ) -> Result<(), ActionsRepositoryError>;
}
