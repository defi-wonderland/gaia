//! DLQ producer for publishing failed operations to the Kafka DLQ topic.

use std::time::Duration;

use hermes_instrumentation::{debug, error};
use hermes_kafka::{create_producer, BaseProducer, BaseRecord, Producer};

use super::types::DlqRecord;
use crate::errors::IngestError;

/// Trait for DLQ producers, enabling test implementations without Kafka.
pub trait DlqProducerLike: Send + 'static {
    /// Send a record to the DLQ, logging but not propagating errors.
    fn send_best_effort(&self, record: &DlqRecord);
    /// Flush all buffered messages to ensure delivery.
    fn flush(&self) -> Result<(), IngestError>;
}

/// Produces failed operation records to the Kafka DLQ topic.
///
/// Wraps `hermes_kafka::BaseProducer` with DLQ-specific serialization and routing.
/// Uses the entity key as the Kafka message key for partition affinity, ensuring
/// all events for the same entity land on the same partition and preserve order.
pub struct DlqProducer {
    producer: BaseProducer,
    topic: String,
}

impl DlqProducer {
    /// Create a new DLQ producer.
    ///
    /// # Arguments
    ///
    /// * `broker` - Kafka broker address (e.g., "localhost:9092")
    /// * `topic` - Full DLQ topic name (with environment prefix already applied)
    pub fn new(broker: &str, topic: String) -> Result<Self, IngestError> {
        let producer = create_producer(broker, "search-indexer-dlq")
            .map_err(|e| IngestError::dlq(format!("Failed to create DLQ producer: {}", e)))?;

        debug!(topic = %topic, "Created DLQ producer");

        Ok(Self { producer, topic })
    }

    /// Send a failed operation to the DLQ.
    ///
    /// Uses `{entity_id}_{space_id}` as the message key for partition affinity.
    /// If entity_id is empty, falls back to space_id.
    pub fn send(&self, record: &DlqRecord) -> Result<(), IngestError> {
        let payload = serde_json::to_vec(record)
            .map_err(|e| IngestError::dlq(format!("Failed to serialize DLQ record: {}", e)))?;

        let key = if record.entity_id.is_empty() {
            record.space_id.clone()
        } else {
            format!("{}_{}", record.entity_id, record.space_id)
        };

        let kafka_record = BaseRecord::to(&self.topic).key(&key).payload(&payload);

        self.producer
            .send(kafka_record)
            .map_err(|(e, _)| IngestError::dlq(format!("Failed to send to DLQ: {}", e)))?;

        debug!(
            entity_id = %record.entity_id,
            space_id = %record.space_id,
            operation_type = %record.operation_type,
            "Sent record to DLQ"
        );

        Ok(())
    }

    /// Flush all buffered messages to ensure they are delivered.
    pub fn flush(&self) -> Result<(), IngestError> {
        self.producer
            .flush(Duration::from_secs(5))
            .map_err(|e| IngestError::dlq(format!("Failed to flush DLQ producer: {}", e)))?;
        Ok(())
    }

    /// Send a record and log (but don't fail) if the send fails.
    ///
    /// This is used in the loader where DLQ send failures should not
    /// prevent the batch from being ACKed.
    pub fn send_best_effort(&self, record: &DlqRecord) {
        if let Err(e) = self.send(record) {
            error!(
                entity_id = %record.entity_id,
                space_id = %record.space_id,
                operation_type = %record.operation_type,
                error_message = %record.error_message,
                error = %e,
                "Failed to send to DLQ - operation details logged for recovery"
            );
        }
    }
}

impl DlqProducerLike for DlqProducer {
    fn send_best_effort(&self, record: &DlqRecord) {
        DlqProducer::send_best_effort(self, record);
    }

    fn flush(&self) -> Result<(), IngestError> {
        DlqProducer::flush(self)
    }
}
