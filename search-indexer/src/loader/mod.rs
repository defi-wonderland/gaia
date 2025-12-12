//! Loader module for the search indexer ingest.
//!
//! Loads processed documents into the search index using UpdateEntityRequest.

use std::sync::Arc;
use tracing::{debug, error, instrument, warn};

use crate::errors::IngestError;
use crate::processor::ProcessedEvent;
use search_indexer_repository::{
    DeleteEntityRequest, SearchIndexProvider, UnsetEntityPropertiesRequest, UpdateEntityRequest,
};

/// Configuration for the search loader.
#[derive(Debug, Clone)]
pub struct LoaderConfig {
    /// Number of documents to batch before flushing.
    pub batch_size: usize,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self { batch_size: 100 }
    }
}

/// Loader that indexes documents into the search engine.
///
/// The loader is responsible for:
/// - Batching documents for efficient bulk indexing
/// - Converting EntityDocuments to UpdateEntityRequests
pub struct SearchLoader {
    provider: Arc<dyn SearchIndexProvider>,
    config: LoaderConfig,
    pending_updates: Vec<UpdateEntityRequest>,
    pending_deletes: Vec<DeleteEntityRequest>,
}

impl SearchLoader {
    /// Create a new search loader with the given provider.
    pub fn new(provider: Arc<dyn SearchIndexProvider>) -> Self {
        Self {
            provider,
            config: LoaderConfig::default(),
            pending_updates: Vec::new(),
            pending_deletes: Vec::new(),
        }
    }

    /// Create a new search loader with custom configuration.
    pub fn with_config(provider: Arc<dyn SearchIndexProvider>, config: LoaderConfig) -> Self {
        let batch_size = config.batch_size;
        Self {
            provider,
            config,
            pending_updates: Vec::with_capacity(batch_size),
            pending_deletes: Vec::new(),
        }
    }

    /// Load a batch of processed events.
    ///
    /// Documents are batched and flushed when the batch size is reached.
    #[instrument(skip(self, events), fields(event_count = events.len()))]
    pub async fn load(&mut self, events: Vec<ProcessedEvent>) -> Result<(), IngestError> {
        for event in events {
            match event {
                ProcessedEvent::Index(doc) => {
                    // Convert EntityDocument to UpdateEntityRequest
                    let update_request = UpdateEntityRequest {
                        entity_id: doc.entity_id.to_string(),
                        space_id: doc.space_id.to_string(),
                        name: doc.name,
                        description: doc.description,
                        avatar: doc.avatar,
                        cover: doc.cover,
                        entity_global_score: doc.entity_global_score,
                        space_score: doc.space_score,
                        entity_space_score: doc.entity_space_score,
                    };
                    self.pending_updates.push(update_request);
                }
                ProcessedEvent::Delete {
                    entity_id,
                    space_id,
                } => {
                    let delete_request = DeleteEntityRequest {
                        entity_id: entity_id.to_string(),
                        space_id: space_id.to_string(),
                    };
                    self.pending_deletes.push(delete_request);
                }
                ProcessedEvent::UnsetProperties {
                    entity_id,
                    space_id,
                    property_keys,
                } => {
                    // Process unset operations immediately (they're usually infrequent)
                    let unset_request = UnsetEntityPropertiesRequest {
                        entity_id: entity_id.to_string(),
                        space_id: space_id.to_string(),
                        property_keys,
                    };
                    if let Err(e) = self
                        .provider
                        .unset_document_properties(&unset_request)
                        .await
                    {
                        warn!(
                            entity_id = %entity_id,
                            space_id = %space_id,
                            error = %e,
                            "Failed to unset document properties"
                        );
                    }
                }
            }
        }

        // Flush if we've reached batch size
        if self.pending_updates.len() >= self.config.batch_size {
            self.flush().await?;
        }

        // Process deletes immediately (they're usually less frequent)
        if !self.pending_deletes.is_empty() {
            self.process_deletes().await?;
        }

        Ok(())
    }

    /// Flush all pending documents to the search index.
    #[instrument(skip(self))]
    pub async fn flush(&mut self) -> Result<(), IngestError> {
        if self.pending_updates.is_empty() {
            return Ok(());
        }

        let updates: Vec<UpdateEntityRequest> = self.pending_updates.drain(..).collect();
        let count = updates.len();

        debug!(count = count, "Flushing documents to search index");

        // Use bulk_update_documents from SearchIndexProvider
        match self.provider.bulk_update_documents(&updates).await {
            Ok(summary) => {
                if summary.failed > 0 {
                    warn!(
                        succeeded = summary.succeeded,
                        failed = summary.failed,
                        "Bulk update completed with some failures"
                    );
                    // Log individual failures
                    for result in summary.results.iter().filter(|r| !r.success) {
                        if let Some(ref err) = result.error {
                            error!(
                                entity_id = %result.entity_id,
                                error = %err,
                                "Failed to update document"
                            );
                        }
                    }
                } else {
                    debug!(
                        count = summary.succeeded,
                        "Successfully updated all documents"
                    );
                }
                Ok(())
            }
            Err(e) => {
                error!(error = %e, count = count, "Failed to bulk update documents");
                Err(IngestError::loader(format!(
                    "Failed to bulk update {} documents: {}",
                    count, e
                )))
            }
        }
    }

    /// Process pending delete operations.
    async fn process_deletes(&mut self) -> Result<(), IngestError> {
        let deletes: Vec<DeleteEntityRequest> = self.pending_deletes.drain(..).collect();

        for delete_request in deletes {
            if let Err(e) = self.provider.delete_document(&delete_request).await {
                // Log but don't fail - document might not exist
                warn!(
                    entity_id = %delete_request.entity_id,
                    space_id = %delete_request.space_id,
                    error = %e,
                    "Failed to delete document"
                );
            }
        }

        Ok(())
    }

    /// Check if the provider is ready (for health checks).
    /// Note: The current SearchIndexProvider doesn't have a health_check method,
    /// so we just return Ok for now.
    pub async fn check_ready(&self) -> Result<(), IngestError> {
        // The provider is ready if it was created successfully
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use search_indexer_repository::{
        BatchOperationResult, BatchOperationSummary, SearchIndexError, UnsetEntityPropertiesRequest,
    };
    use search_indexer_shared::EntityDocument;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    /// Mock search provider for testing.
    struct MockSearchProvider {
        updated_count: AtomicUsize,
        deleted_count: AtomicUsize,
    }

    impl MockSearchProvider {
        fn new() -> Self {
            Self {
                updated_count: AtomicUsize::new(0),
                deleted_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl SearchIndexProvider for MockSearchProvider {
        async fn ensure_index_exists(&self) -> Result<(), SearchIndexError> {
            Ok(())
        }

        async fn update_document(
            &self,
            _request: &UpdateEntityRequest,
        ) -> Result<(), SearchIndexError> {
            self.updated_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn delete_document(
            &self,
            _request: &DeleteEntityRequest,
        ) -> Result<(), SearchIndexError> {
            self.deleted_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn bulk_update_documents(
            &self,
            requests: &[UpdateEntityRequest],
        ) -> Result<BatchOperationSummary, SearchIndexError> {
            let count = requests.len();
            self.updated_count.fetch_add(count, Ordering::SeqCst);
            Ok(BatchOperationSummary {
                total: count,
                succeeded: count,
                failed: 0,
                results: requests
                    .iter()
                    .map(|r| BatchOperationResult {
                        entity_id: r.entity_id.clone(),
                        space_id: r.space_id.clone(),
                        success: true,
                        error: None,
                    })
                    .collect(),
            })
        }

        async fn bulk_delete_documents(
            &self,
            requests: &[DeleteEntityRequest],
        ) -> Result<BatchOperationSummary, SearchIndexError> {
            let count = requests.len();
            self.deleted_count.fetch_add(count, Ordering::SeqCst);
            Ok(BatchOperationSummary {
                total: count,
                succeeded: count,
                failed: 0,
                results: requests
                    .iter()
                    .map(|r| BatchOperationResult {
                        entity_id: r.entity_id.clone(),
                        space_id: r.space_id.clone(),
                        success: true,
                        error: None,
                    })
                    .collect(),
            })
        }

        async fn unset_document_properties(
            &self,
            _request: &UnsetEntityPropertiesRequest,
        ) -> Result<(), SearchIndexError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_load_and_flush() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let events = vec![
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Test 1".to_string()),
                None,
            )),
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Test 2".to_string()),
                None,
            )),
        ];

        loader.load(events).await.unwrap();
        loader.flush().await.unwrap();

        assert_eq!(provider.updated_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_delete_processing() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let events = vec![ProcessedEvent::Delete {
            entity_id: Uuid::new_v4(),
            space_id: Uuid::new_v4(),
        }];

        loader.load(events).await.unwrap();

        assert_eq!(provider.deleted_count.load(Ordering::SeqCst), 1);
    }
}
