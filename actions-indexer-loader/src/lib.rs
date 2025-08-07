mod errors;

use actions_indexer_types::models::{Action, UserVote, VotesCount};
use crate::errors::LoaderError;

#[async_trait::async_trait]
pub trait ActionsRepository<DB: sqlx::Database>: Send + Sync {
    async fn insert_actions(
        &self,
        actions: &[Action],
        tx: &mut sqlx::Transaction<'_, DB>,
    ) -> Result<(), LoaderError>;

    async fn update_user_votes(
        &self,
        user_votes: &[UserVote],
        tx: &mut sqlx::Transaction<'_, DB>,
    ) -> Result<(), LoaderError>;

    async fn update_votes_counts(
        &self,
        votes_counts: &[VotesCount],
        tx: &mut sqlx::Transaction<'_, DB>,
    ) -> Result<(), LoaderError>;
}