//! Loader module for the search indexer ingest.
//!
//! Loads processed documents into the search index using UpdateEntityRequest.

use hermes_instrumentation::{debug, error, info_span, instrument, Instrument};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::consumer::StreamMessage;
use crate::errors::IngestError;
use crate::metrics::SearchIndexerMetrics;
use crate::orchestrator::ProcessedBatch;
use crate::processor::ProcessedEvent;
use search_indexer_repository::{
    EntityOperation, RemoveTypeRelationData, SearchIndexProvider, TypeRelationData,
    UnsetEntityPropertiesRequest, UpdateEntityGlobalScoreRequest, UpdateEntityRequest,
    UpdateEntitySpaceScoreRequest, UpdateSpaceScoreRequest,
};

use crate::dlq::{DlqProducerLike, DlqRecord, DlqState};
use chrono::Utc;

/// Loader that indexes documents into the search engine.
///
/// The loader is responsible for:
/// - Batching documents for efficient bulk indexing
/// - Converting EntityDocuments to EntityOperations
/// - Maintaining operation order for consistency
pub struct SearchLoader {
    provider: Arc<dyn SearchIndexProvider>,
    /// All pending operations, maintained in order for correct sequencing
    pending_operations: Vec<EntityOperation>,
}

/// DLQ components for the loader task.
///
/// Groups the producer, state manager, and config needed for DLQ routing.
/// Passed as `Option<LoaderDlq>` to `run()` - `None` when DLQ is disabled.
pub struct LoaderDlq {
    /// DLQ producer (Kafka in production, test double in tests).
    pub producer: Box<dyn DlqProducerLike>,
    /// Poisoned entity state manager.
    pub state: DlqState,
    /// Maximum retry attempts for DLQ records.
    pub max_retries: u32,
}

impl SearchLoader {
    /// Create a new search loader with the given provider.
    pub fn new(provider: Arc<dyn SearchIndexProvider>) -> Self {
        Self {
            provider,
            pending_operations: Vec::new(),
        }
    }

    /// Load a batch of processed events.
    ///
    /// Converts events to EntityOperations and processes them IN ORDER using bulk_operations.
    /// This maintains consistency when multiple operations affect the same entity.
    ///
    /// Returns `(summaries, operations)` where `operations` is the flat list of
    /// `EntityOperation`s that were sent to `bulk_operations`. The results within
    /// each summary are 1:1 with `operations` in the same order, so callers can
    /// match a failed result to its original operation by index.
    #[instrument(skip(self, events), fields(event_count = events.len()))]
    pub async fn load(
        &mut self,
        events: Vec<ProcessedEvent>,
    ) -> Result<(Vec<search_indexer_repository::BatchOperationSummary>, Vec<EntityOperation>), IngestError> {
        if events.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }

        // Convert events to EntityOperations, maintaining order
        for event in events {
            match event {
                ProcessedEvent::Index(doc) => {
                    self.pending_operations
                        .push(EntityOperation::Update(UpdateEntityRequest {
                            entity_id: doc.entity_id.to_string(),
                            space_id: doc.space_id.to_string(),
                            name: doc.name,
                            description: doc.description,
                            avatar: doc.avatar,
                            cover: doc.cover,
                            add_type_relation: None,
                            entity_global_score: doc.entity_global_score,
                            space_score: doc.space_score,
                            entity_space_score: doc.entity_space_score,
                            deleted: doc.deleted,
                        }));
                }
                ProcessedEvent::UnsetProperties {
                    entity_id,
                    space_id,
                    property_keys,
                } => {
                    self.pending_operations.push(EntityOperation::Unset(
                        UnsetEntityPropertiesRequest {
                            entity_id: entity_id.to_string(),
                            space_id: space_id.to_string(),
                            property_keys,
                        },
                    ));
                }
                ProcessedEvent::AddTypeRelation {
                    entity_id,
                    space_id,
                    relation_id,
                    entity_to_id,
                } => {
                    self.pending_operations
                        .push(EntityOperation::Update(UpdateEntityRequest {
                            entity_id: entity_id.to_string(),
                            space_id: space_id.to_string(),
                            name: None,
                            description: None,
                            avatar: None,
                            cover: None,
                            add_type_relation: Some(TypeRelationData {
                                relation_id: relation_id.to_string(),
                                entity_to_id: entity_to_id.to_string(),
                            }),
                            entity_global_score: None,
                            space_score: None,
                            entity_space_score: None,
                            deleted: None,
                        }));
                }
                ProcessedEvent::RemoveTypeRelationById { relation_id } => {
                    self.pending_operations
                        .push(EntityOperation::RemoveTypeRelationById(
                            RemoveTypeRelationData {
                                relation_id: relation_id.to_string(),
                            },
                        ));
                }
                ProcessedEvent::UpdateEntityGlobalScore { entity_id, score } => {
                    self.pending_operations
                        .push(EntityOperation::UpdateEntityGlobalScore(
                            UpdateEntityGlobalScoreRequest {
                                entity_id: entity_id.to_string(),
                                score,
                            },
                        ));
                }
                ProcessedEvent::UpdateSpaceScore { space_id, score } => {
                    self.pending_operations
                        .push(EntityOperation::UpdateSpaceScore(UpdateSpaceScoreRequest {
                            space_id: space_id.to_string(),
                            score,
                        }));
                }
                ProcessedEvent::UpdateEntitySpaceScore {
                    entity_id,
                    space_id,
                    score,
                } => {
                    self.pending_operations
                        .push(EntityOperation::UpdateEntitySpaceScore(
                            UpdateEntitySpaceScoreRequest {
                                entity_id: entity_id.to_string(),
                                space_id: space_id.to_string(),
                                score,
                            },
                        ));
                }
            }
        }

        // Process all operations in a single bulk call, maintaining order
        let operations: Vec<EntityOperation> = self.pending_operations.drain(..).collect();
        let count = operations.len();

        debug!(count = count, "Processing operations in order");

        let result = async { self.provider.bulk_operations(&operations).await }
            .instrument(info_span!(
                "search_indexer.bulk_operations",
                operation_count = count
            ))
            .await;

        match result {
            Ok(summary) => {
                if summary.failed > 0 {
                    error!(
                        succeeded = summary.succeeded,
                        failed = summary.failed,
                        "Bulk operations completed with some failures"
                    );
                    for result in summary.results.iter().filter(|r| !r.success) {
                        if let Some(ref err) = result.error {
                            error!(
                                entity_id = %result.entity_id,
                                space_id = %result.space_id,
                                operation_type = %result.operation_type,
                                error = %err,
                                "Failed operation"
                            );
                        }
                    }
                } else {
                    debug!(
                        count = summary.succeeded,
                        "Successfully completed all operations"
                    );
                }
                Ok((vec![summary], operations))
            }
            Err(e) => {
                error!(error = %e, count = count, "Failed bulk operations");
                Err(IngestError::loader(format!(
                    "Failed to process {} operations: {}",
                    count, e
                )))
            }
        }
    }

    /// Check if the provider is ready (for health checks).
    /// Note: The current SearchIndexProvider doesn't have a health_check method,
    /// so we just return Ok for now.
    pub async fn check_ready(&self) -> Result<(), IngestError> {
        // The provider is ready if it was created successfully
        Ok(())
    }

    /// Run the loader task.
    ///
    /// Receives processed batches from the processor, loads them into the search index,
    /// and sends acknowledgments back to the appropriate consumer (entity or scores).
    /// Returns a tokio task handle.
    ///
    /// When DLQ is enabled (`dlq` is `Some`):
    /// - All events are processed normally through OpenSearch (no blocking)
    /// - Partial failures: failed ops are sent to DLQ, entities are poisoned, batch is ACKed
    /// - Successful ops on poisoned entities are logged at warn level with full details
    /// - Total failures (e.g., OpenSearch down) still NACK the batch
    ///
    /// # Arguments
    ///
    /// * `loader_rx` - Channel to receive processed batches from the processor
    /// * `entity_ack_tx` - Channel to send acknowledgments back to the entity consumer
    /// * `scores_ack_tx` - Channel to send acknowledgments back to the scores consumer
    /// * `metrics` - Metrics tracker
    /// * `dlq` - Optional DLQ components (None when DLQ is disabled)
    pub fn run(
        mut self,
        mut loader_rx: mpsc::Receiver<ProcessedBatch>,
        entity_ack_tx: mpsc::Sender<StreamMessage>,
        scores_ack_tx: mpsc::Sender<StreamMessage>,
        metrics: Arc<SearchIndexerMetrics>,
        dlq: Option<LoaderDlq>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Destructure DLQ components for use in the loop
            let (dlq_producer, mut dlq_state, max_retries): (
                Option<Box<dyn DlqProducerLike>>,
                Option<DlqState>,
                u32,
            ) = match dlq {
                Some(d) => (Some(d.producer), Some(d.state), d.max_retries),
                None => (None, None, 0),
            };

            while let Some(batch) = loader_rx.recv().await {
                // Determine which ack channel to use based on batch type
                let ack_tx = if batch.is_scores_batch {
                    &scores_ack_tx
                } else {
                    &entity_ack_tx
                };
                let batch_type = if batch.is_scores_batch {
                    "scores"
                } else {
                    "entities"
                };

                // Process ALL events through OpenSearch (no skipping for poisoned entities)
                match self.load(batch.events).await {
                    Ok((operation_summaries, operations)) => {
                        let total_failed =
                            operation_summaries.iter().map(|s| s.failed).sum::<usize>();
                        let total_succeeded =
                            operation_summaries.iter().map(|s| s.succeeded).sum::<usize>();

                        if total_failed > 0 && dlq_producer.is_some() && !batch.is_scores_batch {
                            // DLQ enabled (entity batches only): route failures to DLQ, poison entities, ACK batch
                            // Score batches bypass DLQ — scores are idempotent and recomputed
                            // periodically, so NACK + Kafka retry is a better fit.

                            // Check if any failure is retryable (infrastructure error).
                            // If so, NACK the whole batch for redelivery instead of routing to DLQ.
                            // Retryable errors (shard unavailable, disk pressure, etc.) will succeed
                            // on redelivery; any permanent failures will re-fail and go to DLQ then.
                            let has_retryable = operation_summaries.iter()
                                .flat_map(|s| &s.results)
                                .any(|r| !r.success && r.retryable);

                            if has_retryable {
                                for summary in &operation_summaries {
                                    for result in summary.results.iter().filter(|r| !r.success) {
                                        let error_msg = result
                                            .error
                                            .as_ref()
                                            .map(|e| e.to_string())
                                            .unwrap_or_else(|| "Unknown error".to_string());
                                        error!(
                                            entity_id = %result.entity_id,
                                            space_id = %result.space_id,
                                            operation_type = %result.operation_type,
                                            error_message = %error_msg,
                                            retryable = result.retryable,
                                            source_batch_type = %batch_type,
                                            "Operation failed (retryable infrastructure error) - NACKing batch"
                                        );
                                    }
                                }
                                if let Err(send_err) = ack_tx
                                    .send(StreamMessage::Acknowledgment {
                                        offsets: batch.offsets,
                                        success: false,
                                        error: Some(format!(
                                            "Bulk operations completed with {} failures ({} retryable) - NACKing for redelivery",
                                            total_failed, total_failed
                                        )),
                                    })
                                    .await
                                {
                                    error!(error = %send_err, "Failed to send failure acknowledgment - channel closed");
                                }
                                continue;
                            }

                            let producer = dlq_producer.as_ref().unwrap();

                            // Build a flat index into operations to match results 1:1
                            let mut op_index = 0;
                            for summary in &operation_summaries {
                                for result in &summary.results {
                                    if !result.success {
                                        let error_msg = result
                                            .error
                                            .as_ref()
                                            .map(|e| e.to_string())
                                            .unwrap_or_else(|| "Unknown error".to_string());

                                        let dlq_id = uuid::Uuid::new_v4().to_string();

                                        // Log full details of every DLQ'd operation
                                        error!(
                                            dlq_id = %dlq_id,
                                            entity_id = %result.entity_id,
                                            space_id = %result.space_id,
                                            operation_type = %result.operation_type,
                                            error_message = %error_msg,
                                            source_batch_type = %batch_type,
                                            retry_count = 0,
                                            max_retries = max_retries,
                                            "Operation failed - routing to DLQ"
                                        );

                                        // Serialize the original operation for replay
                                        let operation_payload = match operations.get(op_index) {
                                            Some(op) => match serde_json::to_value(op) {
                                                Ok(val) => val,
                                                Err(e) => {
                                                    error!(
                                                        entity_id = %result.entity_id,
                                                        space_id = %result.space_id,
                                                        operation_type = %result.operation_type,
                                                        error = %e,
                                                        "Failed to serialize operation payload for DLQ record - record will be sent without payload"
                                                    );
                                                    serde_json::Value::Null
                                                }
                                            },
                                            None => {
                                                error!(
                                                    entity_id = %result.entity_id,
                                                    space_id = %result.space_id,
                                                    op_index = op_index,
                                                    operations_len = operations.len(),
                                                    "Operation index out of bounds when building DLQ record - record will be sent without payload"
                                                );
                                                serde_json::Value::Null
                                            }
                                        };

                                        // Send failed operation to DLQ
                                        let record = DlqRecord {
                                            dlq_id,
                                            entity_id: result.entity_id.clone(),
                                            space_id: result.space_id.clone(),
                                            operation_type: result.operation_type.clone(),
                                            error_message: error_msg.clone(),
                                            source_batch_type: batch_type.to_string(),
                                            source_topic: None,
                                            source_partition: None,
                                            source_offset: None,
                                            failed_at: Utc::now(),
                                            retry_count: 0,
                                            max_retries,
                                            operation_payload,
                                        };
                                        producer.send_best_effort(&record);
                                        metrics
                                            .total_dlq_events
                                            .fetch_add(1, Ordering::Relaxed);

                                        // Poison entity-scoped operations
                                        if !result.entity_id.is_empty()
                                            && !result.space_id.is_empty()
                                        {
                                            let entity_key = format!(
                                                "{}_{}",
                                                result.entity_id, result.space_id
                                            );

                                            if let Some(ref mut state) = dlq_state {
                                                // Circuit breaker check
                                                if state.would_exceed_limit(&entity_key) {
                                                    error!(
                                                        poisoned_count = state.poisoned_count(),
                                                        entity_key = %entity_key,
                                                        "Circuit breaker: max poisoned entities reached, shutting down loader"
                                                    );
                                                    let _ = ack_tx
                                                        .send(StreamMessage::Acknowledgment {
                                                            offsets: batch.offsets,
                                                            success: false,
                                                            error: Some(format!(
                                                                "Circuit breaker: max poisoned entities ({}) reached",
                                                                state.poisoned_count()
                                                            )),
                                                        })
                                                        .await;
                                                    return; // Exit loader task
                                                }

                                                let is_new =
                                                    state.poison_entity(&entity_key);
                                                if is_new {
                                                    metrics
                                                        .total_poisoned_entities
                                                        .fetch_add(1, Ordering::Relaxed);
                                                }
                                            }
                                        }
                                    } else if let Some(ref state) = dlq_state {
                                        // Successful operation - log if entity is poisoned
                                        if !result.entity_id.is_empty()
                                            && !result.space_id.is_empty()
                                        {
                                            let entity_key = format!(
                                                "{}_{}",
                                                result.entity_id, result.space_id
                                            );
                                            if state.is_poisoned(&entity_key) {
                                                error!(
                                                    entity_id = %result.entity_id,
                                                    space_id = %result.space_id,
                                                    operation_type = %result.operation_type,
                                                    entity_key = %entity_key,
                                                    "Poisoned entity operation succeeded in OpenSearch \
                                                     - logging for DLQ replay reconciliation"
                                                );
                                            }
                                        }
                                    }
                                    op_index += 1;
                                }
                            }

                            error!(
                                succeeded = total_succeeded,
                                failed = total_failed,
                                "Batch completed with partial failures - failed operations routed to DLQ"
                            );

                            // ACK the batch - failures are in the DLQ
                            if let Err(send_err) = ack_tx
                                .send(StreamMessage::Acknowledgment {
                                    offsets: batch.offsets,
                                    success: true,
                                    error: None,
                                })
                                .await
                            {
                                error!(error = %send_err, "Failed to send acknowledgment - channel closed");
                            }

                            metrics
                                .total_documents_indexed
                                .fetch_add(total_succeeded as u64, Ordering::Relaxed);
                        } else if total_failed > 0 {
                            // DLQ not available: log all failed operations and NACK
                            for summary in &operation_summaries {
                                for result in summary.results.iter().filter(|r| !r.success) {
                                    let error_msg = result
                                        .error
                                        .as_ref()
                                        .map(|e| e.to_string())
                                        .unwrap_or_else(|| "Unknown error".to_string());
                                    error!(
                                        entity_id = %result.entity_id,
                                        space_id = %result.space_id,
                                        operation_type = %result.operation_type,
                                        error_message = %error_msg,
                                        source_batch_type = %batch_type,
                                        "Operation failed (DLQ unavailable) - NACKing batch"
                                    );
                                }
                            }
                            if let Err(send_err) = ack_tx
                                .send(StreamMessage::Acknowledgment {
                                    offsets: batch.offsets,
                                    success: false,
                                    error: Some(format!(
                                        "Bulk operations completed with {} failures (DLQ unavailable)",
                                        total_failed
                                    )),
                                })
                                .await
                            {
                                error!(error = %send_err, "Failed to send failure acknowledgment - channel closed");
                            }
                        } else {
                            // All operations successful
                            // Log any operations on poisoned entities for observability
                            if let Some(ref state) = dlq_state {
                                for summary in &operation_summaries {
                                    for result in &summary.results {
                                        if !result.entity_id.is_empty()
                                            && !result.space_id.is_empty()
                                        {
                                            let entity_key = format!(
                                                "{}_{}",
                                                result.entity_id, result.space_id
                                            );
                                            if state.is_poisoned(&entity_key) {
                                                error!(
                                                    entity_id = %result.entity_id,
                                                    space_id = %result.space_id,
                                                    operation_type = %result.operation_type,
                                                    entity_key = %entity_key,
                                                    "Poisoned entity operation succeeded in OpenSearch \
                                                     - logging for DLQ replay reconciliation"
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            if let Err(send_err) = ack_tx
                                .send(StreamMessage::Acknowledgment {
                                    offsets: batch.offsets,
                                    success: true,
                                    error: None,
                                })
                                .await
                            {
                                error!(error = %send_err, "Failed to send success acknowledgment - channel closed");
                            }

                            metrics
                                .total_documents_indexed
                                .fetch_add(total_succeeded as u64, Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        // Total failure (e.g., OpenSearch down) - NACK regardless of DLQ
                        error!(error = %e, "Failed to load batch");
                        if let Err(send_err) = ack_tx
                            .send(StreamMessage::Acknowledgment {
                                offsets: batch.offsets,
                                success: false,
                                error: Some(e.to_string()),
                            })
                            .await
                        {
                            error!(error = %send_err, "Failed to send failure acknowledgment - channel closed");
                        }
                    }
                }
            }

            // Flush DLQ producer before shutdown
            if let Some(ref producer) = dlq_producer {
                if let Err(e) = producer.flush() {
                    error!(error = %e, "Failed to flush DLQ producer on shutdown");
                }
            }

            debug!("Loader task shutting down");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use search_indexer_repository::{
        BatchOperationResult, BatchOperationSummary, DeleteEntityRequest, SearchIndexError,
        UnsetEntityPropertiesRequest,
    };
    use search_indexer_shared::EntityDocument;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    /// Represents an operation that was performed, used to track ordering.
    #[derive(Debug, Clone, PartialEq)]
    enum TrackedOperation {
        Update {
            entity_id: String,
            add_type_relation: Option<TypeRelationData>,
        },
        Delete {
            entity_id: String,
        },
        Unset {
            entity_id: String,
            property_keys: Vec<String>,
        },
        RemoveTypeRelationById {
            relation_id: String,
        },
    }

    /// Mock search provider for testing.
    struct MockSearchProvider {
        operation_count: AtomicUsize,
        /// Tracks all operations in the order they were executed
        operation_order: std::sync::Mutex<Vec<TrackedOperation>>,
    }

    impl MockSearchProvider {
        fn new() -> Self {
            Self {
                operation_count: AtomicUsize::new(0),
                operation_order: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn get_operation_order(&self) -> Vec<TrackedOperation> {
            self.operation_order.lock().unwrap().clone()
        }

        fn get_operation_count(&self) -> usize {
            self.operation_count.load(Ordering::SeqCst)
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
            self.operation_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn delete_document(
            &self,
            _request: &DeleteEntityRequest,
        ) -> Result<(), SearchIndexError> {
            self.operation_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn unset_document_properties(
            &self,
            _request: &UnsetEntityPropertiesRequest,
        ) -> Result<(), SearchIndexError> {
            self.operation_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn bulk_operations(
            &self,
            operations: &[EntityOperation],
        ) -> Result<BatchOperationSummary, SearchIndexError> {
            let count = operations.len();
            self.operation_count.fetch_add(count, Ordering::SeqCst);

            // Track operation order
            let mut ops = self.operation_order.lock().unwrap();
            for op in operations {
                match op {
                    EntityOperation::Update(r) => {
                        ops.push(TrackedOperation::Update {
                            entity_id: r.entity_id.clone(),
                            add_type_relation: r.add_type_relation.clone(),
                        });
                    }
                    EntityOperation::Delete(r) => {
                        ops.push(TrackedOperation::Delete {
                            entity_id: r.entity_id.clone(),
                        });
                    }
                    EntityOperation::Unset(r) => {
                        ops.push(TrackedOperation::Unset {
                            entity_id: r.entity_id.clone(),
                            property_keys: r.property_keys.clone(),
                        });
                    }
                    EntityOperation::RemoveTypeRelationById(r) => {
                        ops.push(TrackedOperation::RemoveTypeRelationById {
                            relation_id: r.relation_id.clone(),
                        });
                    }
                    // Score updates are tracked but don't need detailed tracking in tests
                    EntityOperation::UpdateEntityGlobalScore(_)
                    | EntityOperation::UpdateSpaceScore(_)
                    | EntityOperation::UpdateEntitySpaceScore(_) => {
                        // Score updates pass through - no special tracking needed
                    }
                }
            }
            drop(ops);

            let results: Vec<BatchOperationResult> = operations
                .iter()
                .map(|op| BatchOperationResult {
                    entity_id: op.entity_id().to_string(),
                    space_id: op.space_id().to_string(),
                    operation_type: op.operation_type().to_string(),
                    success: true,
                    retryable: false,
                    error: None,
                })
                .collect();

            Ok(BatchOperationSummary {
                total: count,
                succeeded: count,
                failed: 0,
                results,
            })
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

        assert_eq!(provider.get_operation_count(), 2);
    }

    #[tokio::test]
    async fn test_delete_processing() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let entity_id = Uuid::new_v4();
        let space_id = Uuid::new_v4();

        // Create a soft delete document (Index with deleted=true)
        let mut doc = EntityDocument::new(entity_id, space_id, None, None);
        doc.deleted = Some(true);

        let events = vec![ProcessedEvent::Index(doc)];

        loader.load(events).await.unwrap();

        assert_eq!(provider.get_operation_count(), 1);
        let ops = provider.get_operation_order();
        assert!(matches!(ops[0], TrackedOperation::Update { .. }));
    }

    #[tokio::test]
    async fn test_unset_properties_processing() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let events = vec![ProcessedEvent::UnsetProperties {
            entity_id: Uuid::new_v4(),
            space_id: Uuid::new_v4(),
            property_keys: vec!["name".to_string(), "description".to_string()],
        }];

        loader.load(events).await.unwrap();

        assert_eq!(provider.get_operation_count(), 1);
        let ops = provider.get_operation_order();
        assert!(matches!(ops[0], TrackedOperation::Unset { .. }));
    }

    #[tokio::test]
    async fn test_mixed_event_types() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let mut delete_doc = EntityDocument::new(Uuid::new_v4(), Uuid::new_v4(), None, None);
        delete_doc.deleted = Some(true);

        let events = vec![
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Entity 1".to_string()),
                None,
            )),
            ProcessedEvent::Index(delete_doc), // Soft delete
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Entity 2".to_string()),
                Some("Description".to_string()),
            )),
            ProcessedEvent::UnsetProperties {
                entity_id: Uuid::new_v4(),
                space_id: Uuid::new_v4(),
                property_keys: vec!["name".to_string()],
            },
        ];

        loader.load(events).await.unwrap();

        // All 4 operations processed in a single bulk call
        assert_eq!(provider.get_operation_count(), 4);
        let ops = provider.get_operation_order();
        assert!(matches!(ops[0], TrackedOperation::Update { .. })); // Index
        assert!(matches!(ops[1], TrackedOperation::Update { .. })); // Soft delete (now Update)
        assert!(matches!(ops[2], TrackedOperation::Update { .. })); // Index
        assert!(matches!(ops[3], TrackedOperation::Unset { .. }));
    }

    #[tokio::test]
    async fn test_load_multiple_documents() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        // Add multiple documents
        let events = vec![
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Entity 1".to_string()),
                None,
            )),
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Entity 2".to_string()),
                None,
            )),
        ];

        loader.load(events).await.unwrap();
        // Should process all documents immediately

        assert_eq!(provider.get_operation_count(), 2);
    }

    #[tokio::test]
    async fn test_load_processes_immediately() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let events = vec![ProcessedEvent::Index(EntityDocument::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Test Entity".to_string()),
            None,
        ))];

        loader.load(events).await.unwrap();
        // Load processes immediately
        assert_eq!(provider.get_operation_count(), 1);
    }

    #[tokio::test]
    async fn test_load_empty_events() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        // Load empty events should succeed
        let (summaries, operations) = loader.load(vec![]).await.unwrap();
        assert_eq!(summaries.len(), 0);
        assert_eq!(operations.len(), 0);
        assert_eq!(provider.get_operation_count(), 0);
    }

    #[tokio::test]
    async fn test_default_configuration() {
        let provider = Arc::new(MockSearchProvider::new());
        let _loader = SearchLoader::new(provider);

        // Test that default config works - if we get here, creation succeeded
    }

    #[tokio::test]
    async fn test_check_ready() {
        let provider = Arc::new(MockSearchProvider::new());
        let loader = SearchLoader::new(provider);

        // check_ready should always return Ok for the current implementation
        let result = loader.check_ready().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_entity_document_conversion() {
        let entity_id = Uuid::new_v4();
        let space_id = Uuid::new_v4();
        let doc = EntityDocument::new(
            entity_id,
            space_id,
            Some("Test Name".to_string()),
            Some("Test Description".to_string()),
        );

        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let events = vec![ProcessedEvent::Index(doc)];
        loader.load(events).await.unwrap();

        // Verify the document was processed
        assert_eq!(provider.get_operation_count(), 1);
    }

    #[tokio::test]
    async fn test_add_type_relation_processing() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let entity_id = Uuid::new_v4();
        let space_id = Uuid::new_v4();
        let relation_id = Uuid::new_v4();
        let entity_to_id = Uuid::new_v4();

        let events = vec![ProcessedEvent::AddTypeRelation {
            entity_id,
            space_id,
            relation_id,
            entity_to_id,
        }];

        loader.load(events).await.unwrap();

        // AddTypeRelation should create an Update operation with add_type_relation set
        assert_eq!(provider.get_operation_count(), 1);

        let ops = provider.get_operation_order();
        assert!(matches!(
            &ops[0],
            TrackedOperation::Update { add_type_relation: Some(rel), .. }
            if rel.entity_to_id == entity_to_id.to_string()
        ));
    }

    #[tokio::test]
    async fn test_remove_type_relation_processing() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let relation_id = Uuid::new_v4();

        let events = vec![ProcessedEvent::RemoveTypeRelationById { relation_id }];

        loader.load(events).await.unwrap();

        // RemoveTypeRelationById goes through bulk_operations
        assert_eq!(provider.get_operation_count(), 1);
    }

    #[tokio::test]
    async fn test_multiple_add_type_relations() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let entity_to_id1 = Uuid::new_v4();
        let entity_to_id2 = Uuid::new_v4();

        let events = vec![
            ProcessedEvent::AddTypeRelation {
                entity_id: Uuid::new_v4(),
                space_id: Uuid::new_v4(),
                relation_id: Uuid::new_v4(),
                entity_to_id: entity_to_id1,
            },
            ProcessedEvent::AddTypeRelation {
                entity_id: Uuid::new_v4(),
                space_id: Uuid::new_v4(),
                relation_id: Uuid::new_v4(),
                entity_to_id: entity_to_id2,
            },
        ];

        loader.load(events).await.unwrap();

        assert_eq!(provider.get_operation_count(), 2);

        let ops = provider.get_operation_order();
        assert!(matches!(
            &ops[0],
            TrackedOperation::Update { add_type_relation: Some(rel), .. }
            if rel.entity_to_id == entity_to_id1.to_string()
        ));
        assert!(matches!(
            &ops[1],
            TrackedOperation::Update { add_type_relation: Some(rel), .. }
            if rel.entity_to_id == entity_to_id2.to_string()
        ));
    }

    #[tokio::test]
    async fn test_mixed_type_relation_operations() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let add_entity_to_id = Uuid::new_v4();
        let remove_relation_id = Uuid::new_v4();

        let events = vec![
            ProcessedEvent::AddTypeRelation {
                entity_id: Uuid::new_v4(),
                space_id: Uuid::new_v4(),
                relation_id: Uuid::new_v4(),
                entity_to_id: add_entity_to_id,
            },
            ProcessedEvent::RemoveTypeRelationById {
                relation_id: remove_relation_id,
            },
            ProcessedEvent::Index(EntityDocument::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("Test Entity".to_string()),
                None,
            )),
        ];

        loader.load(events).await.unwrap();

        // 3 operations: AddTypeRelation, RemoveTypeRelationById, and Index
        assert_eq!(provider.get_operation_count(), 3);

        // Verify the operations are in order
        let ops = provider.get_operation_order();
        assert_eq!(ops.len(), 3);

        // First should be the add_type_relation (Update)
        assert!(matches!(
            &ops[0],
            TrackedOperation::Update { add_type_relation: Some(rel), .. }
            if rel.entity_to_id == add_entity_to_id.to_string()
        ));

        // Second should be the RemoveTypeRelationById
        assert!(matches!(
            &ops[1],
            TrackedOperation::RemoveTypeRelationById { relation_id }
            if *relation_id == remove_relation_id.to_string()
        ));

        // Third should be the regular index (Update with no add_type_relation)
        assert!(matches!(
            &ops[2],
            TrackedOperation::Update {
                add_type_relation: None,
                ..
            }
        ));
    }

    /// This test verifies that type relation operations are processed in order.
    ///
    /// The scenario: RemoveTypeRelationById followed by AddTypeRelation
    #[tokio::test]
    async fn test_type_relation_operations_preserve_order() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let entity_id = Uuid::new_v4();
        let space_id = Uuid::new_v4();
        let relation_id = Uuid::new_v4();
        let entity_to_id = Uuid::new_v4();

        // Events in this order: Remove first, then Add
        let events = vec![
            ProcessedEvent::RemoveTypeRelationById { relation_id },
            ProcessedEvent::AddTypeRelation {
                entity_id,
                space_id,
                relation_id,
                entity_to_id,
            },
        ];

        loader.load(events).await.unwrap();

        // 2 operations via bulk_operations
        assert_eq!(
            provider.get_operation_count(),
            2,
            "Should have 2 operations"
        );

        // Verify operations are tracked
        let ops = provider.get_operation_order();
        assert_eq!(ops.len(), 2, "Should have 2 operations tracked");

        // First operation should be RemoveTypeRelationById
        assert!(
            matches!(&ops[0], TrackedOperation::RemoveTypeRelationById { relation_id: rid } if *rid == relation_id.to_string()),
            "First operation should be RemoveTypeRelationById, got: {:?}",
            ops[0]
        );

        // Second operation should be AddTypeRelation (via Update)
        assert!(
            matches!(&ops[1], TrackedOperation::Update { add_type_relation: Some(rel), .. } if rel.entity_to_id == entity_to_id.to_string()),
            "Second operation should be AddTypeRelation (via Update), got: {:?}",
            ops[1]
        );
    }

    /// Test that an UpdateEntityRequest with both add_type_relation AND other properties
    /// results in two separate bulk operations.
    #[tokio::test]
    async fn test_update_with_add_type_relation_and_properties() {
        let provider = Arc::new(MockSearchProvider::new());
        let mut loader = SearchLoader::new(provider.clone());

        let entity_id = Uuid::new_v4();
        let space_id = Uuid::new_v4();
        let relation_id = Uuid::new_v4();
        let entity_to_id = Uuid::new_v4();

        // Create a document with name set
        let doc = EntityDocument::new(
            entity_id,
            space_id,
            Some("Test Entity Name".to_string()),
            Some("Test Description".to_string()),
        );
        // We need to simulate having add_type_relation - but EntityDocument doesn't have this field.
        // Instead, we'll test this at a lower level by creating the operation directly.
        // For now, let's just test that Index + AddTypeRelation for the same entity creates proper operations.

        let events = vec![
            // First, add a type relation
            ProcessedEvent::AddTypeRelation {
                entity_id,
                space_id,
                relation_id,
                entity_to_id,
            },
            // Then, index the document with name/description
            ProcessedEvent::Index(doc),
        ];

        loader.load(events).await.unwrap();

        // Should have 2 operations: one for add_type_relation, one for the document update
        assert_eq!(provider.get_operation_count(), 2);

        let ops = provider.get_operation_order();
        assert_eq!(ops.len(), 2);

        // First should be add_type_relation
        assert!(matches!(
            &ops[0],
            TrackedOperation::Update { add_type_relation: Some(rel), .. }
            if rel.entity_to_id == entity_to_id.to_string()
        ));

        // Second should be regular document update (no add_type_relation)
        assert!(matches!(
            &ops[1],
            TrackedOperation::Update {
                add_type_relation: None,
                ..
            }
        ));
    }
}
