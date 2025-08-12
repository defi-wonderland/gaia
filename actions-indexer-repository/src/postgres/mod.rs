//! This module provides the PostgreSQL implementation of the `ActionsRepository` trait.
//! It handles database interactions for persisting actions, user votes, and vote counts.
use async_trait::async_trait;
use actions_indexer_shared::types::{Action, Changeset, UserVote, VotesCount};
use crate::{ActionsRepository, ActionsRepositoryError};

/// A PostgreSQL implementation of the `ActionsRepository` trait.
///
/// This struct manages database connections and provides concrete implementations
/// for persisting and retrieving action-related data in a PostgreSQL database.
pub struct PostgresActionsRepository {
    pool: sqlx::PgPool,
}

impl PostgresActionsRepository {
    /// Creates a new `PostgresActionsRepository` instance.
    ///
    /// Establishes a connection pool to the PostgreSQL database using the provided URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The connection string for the PostgreSQL database.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(Self)` on successful connection or an
    /// `ActionsRepositoryError` if the database connection fails.
    pub async fn new(url: &str) -> Result<Self, ActionsRepositoryError> {
        let pool = sqlx::PgPool::connect(url).await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(Self { pool })
    }

    /// Inserts a slice of `Action` objects within an existing database transaction.
    ///
    /// This private helper method is used by public `ActionsRepository` methods
    /// to ensure transactional integrity when persisting actions.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `Action` objects to be inserted.
    /// * `tx` - A mutable reference to an active `sqlx::Transaction`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the insertion fails.
    async fn insert_actions_tx(&self, _actions: &[Action], _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), ActionsRepositoryError> {
        todo!("Implement insert_actions_tx for Postgres")
    }

    /// Updates a slice of `UserVote` objects within an existing database transaction.
    ///
    /// This private helper method is used by public `ActionsRepository` methods
    /// to ensure transactional integrity when updating user votes.
    ///
    /// # Arguments
    ///
    /// * `user_votes` - A slice of `UserVote` objects to be updated.
    /// * `tx` - A mutable reference to an active `sqlx::Transaction`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the update fails.
    async fn update_user_votes_tx(&self, _user_votes: &[UserVote], _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), ActionsRepositoryError> {
        todo!("Implement update_user_votes_tx for Postgres")
    }

    /// Updates a slice of `VotesCount` objects within an existing database transaction.
    ///
    /// This private helper method is used by public `ActionsRepository` methods
    /// to ensure transactional integrity when updating vote counts.
    ///
    /// # Arguments
    ///
    /// * `votes_counts` - A slice of `VotesCount` objects to be updated.
    /// * `tx` - A mutable reference to an active `sqlx::Transaction`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the update fails.
    async fn update_votes_counts_tx(&self, _votes_counts: &[VotesCount], _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), ActionsRepositoryError> {
        todo!("Implement update_votes_counts_tx for Postgres")
    }
}

#[async_trait]
impl ActionsRepository for PostgresActionsRepository {
    /// Inserts a slice of `Action` objects into the repository within a new transaction.
    ///
    /// This method creates a new database transaction, calls `insert_actions_tx` to perform
    /// the insertion, and then commits the transaction.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `Action` objects to be inserted.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the transaction
    /// or insertion fails.
    async fn insert_actions(
        &self,
        actions: &[Action],
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.insert_actions_tx(actions, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }

    /// Updates a slice of `UserVote` objects in the repository within a new transaction.
    ///
    /// This method creates a new database transaction, calls `update_user_votes_tx` to perform
    /// the update, and then commits the transaction.
    ///
    /// # Arguments
    ///
    /// * `user_votes` - A slice of `UserVote` objects to be updated.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the transaction
    /// or update fails.
    async fn update_user_votes(
        &self,
        user_votes: &[UserVote],
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.update_user_votes_tx(user_votes, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }

    /// Updates a slice of `VotesCount` objects in the repository within a new transaction.
    ///
    /// This method creates a new database transaction, calls `update_votes_counts_tx` to perform
    /// the update, and then commits the transaction.
    ///
    /// # Arguments
    ///
    /// * `votes_counts` - A slice of `VotesCount` objects to be updated.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the transaction
    /// or update fails.
    async fn update_votes_counts(
        &self,
        votes_counts: &[VotesCount],
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.update_votes_counts_tx(votes_counts, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }

    /// Persists a `Changeset` object to the repository within a new transaction.
    ///
    /// This method creates a new database transaction and then calls the private
    /// transactional helper methods (`insert_actions_tx`, `update_user_votes_tx`,
    /// and `update_votes_counts_tx`) to persist the components of the changeset.
    /// The transaction is committed upon success.
    ///
    /// # Arguments
    ///
    /// * `changeset` - A reference to the `Changeset` object to be persisted.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `ActionsRepositoryError` if the transaction
    /// or any persistence operation fails.
    async fn persist_changeset(
        &self,
        changeset: &Changeset<'_>,
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.insert_actions_tx(changeset.actions, &mut tx).await?;
        self.update_user_votes_tx(changeset.user_votes, &mut tx).await?;
        self.update_votes_counts_tx(changeset.votes_count, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }
}