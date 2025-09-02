use crate::errors::CursorRepositoryError;

/// Trait for interacting with the cursor repository.
///
/// This trait provides a clean abstraction over the underlying data store for the actions indexer system. It handles the retrieval and persistence of the cursor.
#[async_trait::async_trait]
pub trait CursorRepository: Send + Sync {
    async fn get_cursor(&self, id: &str) -> Result<Option<String>, CursorRepositoryError>;
    async fn save_cursor(&self, id: &str, cursor: &str, block_number: &i64) -> Result<(), CursorRepositoryError>;
}