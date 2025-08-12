use actions_indexer_shared::types::{Action, UserVote, VotesCount, Changeset};
use crate::errors::ActionsRepositoryError;

#[async_trait::async_trait]
pub trait ActionsRepository: Send + Sync {
    async fn insert_actions(
        &self,
        actions: &[Action],
    ) -> Result<(), ActionsRepositoryError>;

    async fn update_user_votes(
        &self,
        user_votes: &[UserVote],
    ) -> Result<(), ActionsRepositoryError>;

    async fn update_votes_counts(
        &self,
        votes_counts: &[VotesCount],
    ) -> Result<(), ActionsRepositoryError>;

    async fn persist_changeset(
        &self,
        changeset: &Changeset<'_>,
    ) -> Result<(), ActionsRepositoryError>;
}
