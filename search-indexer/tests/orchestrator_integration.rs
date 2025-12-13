//! Integration tests for the search indexer orchestrator.
//!
//! These tests use the real Orchestrator but mock dependencies
//! (KafkaConsumer and SearchIndexProvider) to ensure reliable testing.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;

use search_indexer::consumer::{EntityEvent, StreamMessage};
use search_indexer::errors::IngestError;
use search_indexer::loader::SearchLoader;
use search_indexer::orchestrator::{Consumer, Orchestrator, OrchestratorConfig};
use search_indexer::processor::EntityProcessor;
use search_indexer_repository::{
    BatchOperationResult, BatchOperationSummary, DeleteEntityRequest, SearchIndexError,
    SearchIndexProvider, UnsetEntityPropertiesRequest, UpdateEntityRequest,
};
use uuid::Uuid;

// Mock Consumer for testing
struct MockConsumer {
    events_to_send: Vec<EntityEvent>,
    should_error: bool,
    error_on_subscribe: bool,
}

impl MockConsumer {
    fn new(events: Vec<EntityEvent>) -> Self {
        Self {
            events_to_send: events,
            should_error: false,
            error_on_subscribe: false,
        }
    }

    fn with_subscribe_error(events: Vec<EntityEvent>) -> Self {
        Self {
            events_to_send: events,
            should_error: false,
            error_on_subscribe: true,
        }
    }
}

#[async_trait::async_trait]
impl Consumer for MockConsumer {
    fn subscribe(&self) -> Result<(), IngestError> {
        // Mock subscription - succeeds unless error_on_subscribe is true
        if self.error_on_subscribe {
            Err(IngestError::KafkaError("Mock subscribe error".to_string()))
        } else {
            Ok(())
        }
    }

    async fn run(
        &self,
        sender: mpsc::Sender<StreamMessage>,
        mut ack_receiver: mpsc::Receiver<StreamMessage>,
        mut shutdown: broadcast::Receiver<()>,
    ) -> Result<(), IngestError> {
        // If should_error is true, return an error immediately
        if self.should_error {
            return Err(IngestError::KafkaError("Mock consumer error".to_string()));
        }

        // Convert events to StreamMessage
        let events = self.events_to_send.clone();
        let offsets = vec![("test-topic".to_string(), 0, 1i64)]; // Mock offset

        // Send events
        let _ = sender.send(StreamMessage::Events { events, offsets }).await;

        // Send End message to signal completion
        let _ = sender.send(StreamMessage::End).await;

        // Wait for shutdown or acknowledgment
        tokio::select! {
            _ = shutdown.recv() => {
                // Shutdown received
            }
            Some(StreamMessage::Acknowledgment { success, .. }) = ack_receiver.recv() => {
                if !success {
                    return Err(IngestError::LoaderError("Processing failed".to_string()));
                }
            }
        }

        Ok(())
    }
}

// Mock Search Provider for testing
struct MockSearchProvider {
    updated_documents: std::sync::Mutex<Vec<UpdateEntityRequest>>,
    deleted_documents: std::sync::Mutex<Vec<DeleteEntityRequest>>,
    unset_properties_calls: std::sync::Mutex<Vec<UnsetEntityPropertiesRequest>>,
}

impl MockSearchProvider {
    fn new() -> Self {
        Self {
            updated_documents: std::sync::Mutex::new(Vec::new()),
            deleted_documents: std::sync::Mutex::new(Vec::new()),
            unset_properties_calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn get_updated_count(&self) -> usize {
        self.updated_documents.lock().unwrap().len()
    }

    fn get_deleted_count(&self) -> usize {
        self.deleted_documents.lock().unwrap().len()
    }

    fn get_unset_count(&self) -> usize {
        self.unset_properties_calls.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl SearchIndexProvider for MockSearchProvider {
    async fn ensure_index_exists(&self) -> Result<(), SearchIndexError> {
        Ok(())
    }

    async fn update_document(&self, request: &UpdateEntityRequest) -> Result<(), SearchIndexError> {
        self.updated_documents.lock().unwrap().push(request.clone());
        Ok(())
    }

    async fn delete_document(&self, request: &DeleteEntityRequest) -> Result<(), SearchIndexError> {
        self.deleted_documents.lock().unwrap().push(request.clone());
        Ok(())
    }

    async fn bulk_update_documents(
        &self,
        requests: &[UpdateEntityRequest],
    ) -> Result<BatchOperationSummary, SearchIndexError> {
        let mut updated = self.updated_documents.lock().unwrap();
        for request in requests {
            updated.push(request.clone());
        }

        Ok(BatchOperationSummary {
            total: requests.len(),
            succeeded: requests.len(),
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
        let mut deleted = self.deleted_documents.lock().unwrap();
        for request in requests {
            deleted.push(request.clone());
        }

        Ok(BatchOperationSummary {
            total: requests.len(),
            succeeded: requests.len(),
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
        request: &UnsetEntityPropertiesRequest,
    ) -> Result<(), SearchIndexError> {
        self.unset_properties_calls
            .lock()
            .unwrap()
            .push(request.clone());
        Ok(())
    }
}

/// Helper to create a test orchestrator with mocked dependencies
fn create_test_orchestrator(events: Vec<EntityEvent>) -> (Orchestrator, Arc<MockSearchProvider>) {
    let processor = EntityProcessor::new();
    let mock_provider = Arc::new(MockSearchProvider::new());
    let loader = SearchLoader::new(mock_provider.clone());

    let mock_consumer = Arc::new(MockConsumer::new(events));

    let orchestrator = Orchestrator::new(mock_consumer, processor, loader);

    (orchestrator, mock_provider)
}

/// Helper to create a test orchestrator with an error-prone consumer
fn create_error_test_orchestrator(
    events: Vec<EntityEvent>,
) -> (Orchestrator, Arc<MockSearchProvider>) {
    let processor = EntityProcessor::new();
    let mock_provider = Arc::new(MockSearchProvider::new());
    let loader = SearchLoader::new(mock_provider.clone());

    let mock_consumer = Arc::new(MockConsumer::with_subscribe_error(events));

    let orchestrator = Orchestrator::new(mock_consumer, processor, loader);

    (orchestrator, mock_provider)
}

#[tokio::test]
async fn test_orchestrator_full_integration() {
    // Test the complete orchestrator flow with mock consumer and provider
    let events = vec![
        EntityEvent::upsert(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Test Entity".to_string()),
            Some("Description".to_string()),
            None,
        ),
        EntityEvent::upsert(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Another Entity".to_string()),
            None,
            None,
        ),
    ];

    let (mut orchestrator, mock_provider) = create_test_orchestrator(events);

    // Run the orchestrator with a timeout to avoid hanging
    let result = timeout(Duration::from_secs(5), orchestrator.run()).await;

    // The orchestrator should complete successfully
    assert!(result.is_ok());
    let run_result = result.unwrap();
    assert!(run_result.is_ok());

    // Check that documents were indexed
    assert_eq!(mock_provider.get_updated_count(), 2);
}

#[tokio::test]
async fn test_orchestrator_with_delete_events() {
    let events = vec![
        EntityEvent::delete(Uuid::new_v4(), Uuid::new_v4()),
        EntityEvent::delete(Uuid::new_v4(), Uuid::new_v4()),
    ];

    let (mut orchestrator, mock_provider) = create_test_orchestrator(events);

    // Run the orchestrator
    let result = timeout(Duration::from_secs(5), orchestrator.run()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_ok());

    assert_eq!(mock_provider.get_deleted_count(), 2);
}

#[tokio::test]
async fn test_orchestrator_with_unset_properties() {
    let entity_id = Uuid::new_v4();
    let space_id = Uuid::new_v4();

    let events = vec![EntityEvent::unset_properties(
        entity_id,
        space_id,
        vec!["name".to_string(), "description".to_string()],
    )];

    let (mut orchestrator, mock_provider) = create_test_orchestrator(events);

    // Run the orchestrator
    let result = timeout(Duration::from_secs(5), orchestrator.run()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_ok());

    assert_eq!(mock_provider.get_unset_count(), 1);
}

#[tokio::test]
async fn test_orchestrator_configuration() {
    // Test that orchestrator can be created with custom configuration
    let processor = EntityProcessor::new();
    let mock_provider = Arc::new(MockSearchProvider::new());
    let loader = SearchLoader::new(mock_provider.clone());
    let mock_consumer = Arc::new(MockConsumer::new(vec![]));

    let config = OrchestratorConfig {
        channel_buffer_size: 2000,
    };

    let _orchestrator = Orchestrator::with_config(mock_consumer, processor, loader, config);

    // Verify configuration was applied (we can't easily test this without exposing internals,
    // but at least verify it compiles and creates successfully)
    assert!(true); // If we get here, creation succeeded
}

#[tokio::test]
async fn test_empty_event_batch_processing() {
    let events = vec![]; // Empty batch

    let (mut orchestrator, mock_provider) = create_test_orchestrator(events);

    // Run the orchestrator with empty events
    let result = timeout(Duration::from_secs(5), orchestrator.run()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_ok());

    // No documents should be processed
    assert_eq!(mock_provider.get_updated_count(), 0);
}

#[tokio::test]
async fn test_orchestrator_shutdown() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Create orchestrator with some events that will keep it running
    let events = vec![EntityEvent::upsert(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Some("Test Entity".to_string()),
        Some("Description".to_string()),
        None,
    )];

    let (orchestrator, _mock_provider) = create_test_orchestrator(events);
    let orchestrator = Arc::new(Mutex::new(orchestrator));

    // Clone for the shutdown task
    let orchestrator_clone = Arc::clone(&orchestrator);

    // Create a task that will shutdown the orchestrator after a short delay
    let shutdown_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let orchestrator = orchestrator_clone.lock().await;
        orchestrator.shutdown();
    });

    // Spawn orchestrator in background
    let orchestrator_run_clone = Arc::clone(&orchestrator);
    let orchestrator_handle = tokio::spawn(async move {
        let mut orchestrator = orchestrator_run_clone.lock().await;
        orchestrator.run().await
    });

    // Wait for both tasks to complete
    let (shutdown_result, orchestrator_result) = tokio::join!(shutdown_handle, orchestrator_handle);

    assert!(shutdown_result.is_ok(), "Shutdown task should succeed");
    assert!(
        orchestrator_result.is_ok(),
        "Orchestrator task should succeed"
    );

    let run_result = orchestrator_result.unwrap();
    assert!(
        run_result.is_ok(),
        "Orchestrator should complete successfully"
    );
}

#[tokio::test]
async fn test_orchestrator_error_handling() {
    // Create orchestrator with a consumer that will error
    let events = vec![EntityEvent::upsert(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Some("Test Entity".to_string()),
        Some("Description".to_string()),
        None,
    )];

    let (mut orchestrator, _mock_provider) = create_error_test_orchestrator(events);

    // Run the orchestrator - it should fail due to consumer error
    let result = timeout(Duration::from_secs(5), orchestrator.run()).await;
    assert!(result.is_ok(), "Orchestrator should complete");

    let run_result = result.unwrap();
    assert!(
        run_result.is_err(),
        "Orchestrator should return error from consumer"
    );

    // Verify it's the expected error type
    match run_result.unwrap_err() {
        IngestError::KafkaError(msg) => {
            assert_eq!(msg, "Mock subscribe error");
        }
        _ => panic!("Expected KafkaError"),
    }
}
