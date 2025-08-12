pub use actions_indexer_repository::{ActionsRepository, ActionsRepositoryError};
pub use actions_indexer_repository::PostgresActionsRepository;
pub use actions_indexer_shared::types::Changeset;
pub use crate::errors::LoaderError;
use std::sync::Arc;

pub struct ActionsLoader {
	actions_repository: Arc<dyn ActionsRepository>
}

impl ActionsLoader {
	pub fn new(actions_repository: Arc<dyn ActionsRepository>) -> Self {
		Self { actions_repository }
	}

    pub async fn persist_changeset<'a>(&self, changeset: &'a Changeset<'a>) -> Result<(), LoaderError> {
        self.actions_repository.persist_changeset(changeset).await?;
        Ok(())
    }
}
