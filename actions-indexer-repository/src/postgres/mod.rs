use async_trait::async_trait;
use actions_indexer_shared::types::{Action, Changeset, UserVote, VotesCount};
use crate::{ActionsRepository, ActionsRepositoryError};

pub struct PostgresActionsRepository {
    pool: sqlx::PgPool,
}

impl PostgresActionsRepository {
    pub async fn new(url: &str) -> Result<Self, ActionsRepositoryError> {
        let pool = sqlx::PgPool::connect(url).await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(Self { pool })
    }

    async fn insert_actions_tx(&self, actions: &[Action], tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), ActionsRepositoryError> {
        todo!("Implement insert_actions_tx for Postgres")
    }

    async fn update_user_votes_tx(&self, user_votes: &[UserVote], tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), ActionsRepositoryError> {
        todo!("Implement update_user_votes_tx for Postgres")
    }

    async fn update_votes_counts_tx(&self, votes_counts: &[VotesCount], tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), ActionsRepositoryError> {
        todo!("Implement update_votes_counts_tx for Postgres")
    }
}

#[async_trait]
impl ActionsRepository for PostgresActionsRepository {
    async fn insert_actions(
        &self,
        actions: &[Action],
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.insert_actions_tx(actions, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }

    async fn update_user_votes(
        &self,
        user_votes: &[UserVote],
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.update_user_votes_tx(user_votes, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }

    async fn update_votes_counts(
        &self,
        votes_counts: &[VotesCount],
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.update_votes_counts_tx(votes_counts, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }

    async fn persist_changeset(
        &self,
        changeset: &Changeset,
    ) -> Result<(), ActionsRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        self.insert_actions_tx(changeset.actions, &mut tx).await?;
        self.update_user_votes_tx(changeset.user_votes, &mut tx).await?;
        self.update_votes_counts_tx(changeset.votes_count, &mut tx).await?;
        tx.commit().await.map_err(|e| ActionsRepositoryError::DatabaseError(e))?;
        Ok(())
    }
}