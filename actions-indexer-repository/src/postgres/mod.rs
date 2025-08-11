use async_trait::async_trait;
use sqlx::Transaction;
use actions_indexer_shared::types::{Action, UserVote, VotesCount};
use crate::{ActionsRepository, ActionsRepositoryError};

pub struct PostgresActionsRepository {}

impl PostgresActionsRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ActionsRepository<sqlx::Postgres> for PostgresActionsRepository {
    async fn insert_actions(
        &self,
        _actions: &[Action],
        _tx: &mut Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), ActionsRepositoryError> {
        // TODO: Implement Postgres-specific logic
        todo!("Implement insert_actions for Postgres")
    }

    async fn update_user_votes(
        &self,
        _user_votes: &[UserVote],
        _tx: &mut Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), ActionsRepositoryError> {
        // TODO: Implement Postgres-specific logic
        todo!("Implement update_user_votes for Postgres")
    }

    async fn update_votes_counts(
        &self,
        _votes_counts: &[VotesCount],
        _tx: &mut Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), ActionsRepositoryError> {
        // TODO: Implement Postgres-specific logic
        todo!("Implement update_votes_counts for Postgres")
    }
}