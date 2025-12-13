//! Kafka consumer implementation for the search indexer.
//!
//! Consumes entity events from Kafka topics and forwards them to the ingest.

use prost::Message;
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    message::Message as KafkaMessage,
    TopicPartitionList,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::consumer::messages::{EntityEvent, StreamMessage};
use crate::errors::IngestError;

use hermes_schema::pb::knowledge::HermesEdit;
use indexer_utils::id::transform_id_bytes;
use sdk::core::ids::{AVATAR_ATTRIBUTE, DESCRIPTION_ATTRIBUTE, NAME_ATTRIBUTE};
use wire::pb::grc20::op::Payload;

/// The Kafka topic for knowledge edits.
const KNOWLEDGE_EDITS_TOPIC: &str = "knowledge.edits";

/// Default batch size for Kafka message batching.
const DEFAULT_BATCH_SIZE: usize = 50;

/// Default batch timeout in milliseconds.
const DEFAULT_BATCH_TIMEOUT_MS: u64 = 1000;

/// Pending message information for batching.
struct PendingMessage {
    events: Vec<EntityEvent>,
}

/// Kafka consumer for entity events.
pub struct KafkaConsumer {
    consumer: StreamConsumer,
    topics: Vec<String>,
    batch_size: usize,
    batch_timeout: Duration,
}

impl KafkaConsumer {
    /// Create a new Kafka consumer.
    ///
    /// # Arguments
    ///
    /// * `brokers` - Kafka broker addresses (comma-separated)
    /// * `group_id` - Consumer group ID
    ///
    /// # Returns
    ///
    /// * `Ok(KafkaConsumer)` - A new consumer instance
    /// * `Err(IngestError)` - If consumer creation fails
    pub fn new(brokers: &str, group_id: &str) -> Result<Self, IngestError> {
        Self::with_batch_config(
            brokers,
            group_id,
            DEFAULT_BATCH_SIZE,
            DEFAULT_BATCH_TIMEOUT_MS,
        )
    }

    /// Create a new Kafka consumer with custom batch configuration.
    ///
    /// # Arguments
    ///
    /// * `brokers` - Kafka broker addresses (comma-separated)
    /// * `group_id` - Consumer group ID
    /// * `batch_size` - Number of messages to batch before sending
    /// * `batch_timeout_ms` - Maximum time to wait before flushing a partial batch (milliseconds)
    ///
    /// # Returns
    ///
    /// * `Ok(KafkaConsumer)` - A new consumer instance
    /// * `Err(IngestError)` - If consumer creation fails
    pub fn with_batch_config(
        brokers: &str,
        group_id: &str,
        batch_size: usize,
        batch_timeout_ms: u64,
    ) -> Result<Self, IngestError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "6000")
            .create()
            .map_err(|e| IngestError::kafka(e.to_string()))?;

        info!(
            brokers = %brokers,
            group_id = %group_id,
            batch_size = batch_size,
            batch_timeout_ms = batch_timeout_ms,
            "Created Kafka consumer with batching"
        );

        Ok(Self {
            consumer,
            topics: vec![KNOWLEDGE_EDITS_TOPIC.to_string()],
            batch_size,
            batch_timeout: Duration::from_millis(batch_timeout_ms),
        })
    }

    /// Subscribe to configured topics.
    pub fn subscribe(&self) -> Result<(), IngestError> {
        let topics: Vec<&str> = self.topics.iter().map(|s| s.as_str()).collect();
        self.consumer
            .subscribe(&topics)
            .map_err(|e| IngestError::kafka(e.to_string()))?;

        info!(topics = ?self.topics, "Subscribed to Kafka topics");
        Ok(())
    }

    /// Start consuming messages and send them through the channel.
    ///
    /// Messages are batched before being sent to improve efficiency.
    ///
    /// # Arguments
    ///
    /// * `sender` - Channel to send messages to
    /// * `ack_receiver` - Channel to receive acknowledgments from orchestrator
    /// * `shutdown` - Shutdown signal receiver
    #[instrument(skip(self, sender, ack_receiver, shutdown))]
    pub async fn run(
        &self,
        sender: mpsc::Sender<StreamMessage>,
        mut ack_receiver: mpsc::Receiver<StreamMessage>,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), IngestError> {
        use futures::StreamExt;

        let mut message_stream = self.consumer.stream();
        let mut batch: Vec<PendingMessage> = Vec::with_capacity(self.batch_size);
        let mut pending_offsets: Vec<(String, i32, i64)> = Vec::new();
        let mut flush_timer = tokio::time::interval(self.batch_timeout);
        flush_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Skip the first tick immediately
        flush_timer.tick().await;

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Consumer received shutdown signal");
                    // Don't flush pending messages - they haven't been committed
                    // and will be re-read from the last committed offset on restart
                    let _ = sender.send(StreamMessage::End).await;
                    break;
                }
                // Handle acknowledgments from orchestrator
                ack_msg = ack_receiver.recv() => {
                    match ack_msg {
                        Some(StreamMessage::Acknowledgment { offsets, success, error }) => {
                            if success {
                                if let Err(e) = self.commit_offsets(&offsets).await {
                                    error!(error = %e, "Failed to commit offsets after acknowledgment");
                                } else {
                                    debug!(offset_count = offsets.len(), "Committed offsets after successful processing");
                                }
                            } else {
                                error!(
                                    offset_count = offsets.len(),
                                    error = error.as_deref().unwrap_or("Unknown error"),
                                    "Not committing offsets due to processing failure"
                                );
                            }
                        }
                        Some(StreamMessage::End) | None => {
                            info!("Acknowledgment channel closed");
                            break;
                        }
                        _ => {
                            // Ignore other message types
                        }
                    }
                }
                message = message_stream.next() => {
                    match message {
                        Some(Ok(msg)) => {
                            info!(
                                topic = %msg.topic(),
                                partition = msg.partition(),
                                offset = msg.offset(),
                                "Received message from Kafka"
                            );
                            match self.parse_message(&msg) {
                                Ok(Some(pending)) => {
                                    batch.push(pending);
                                    pending_offsets.push((msg.topic().to_string(), msg.partition(), msg.offset()));

                                    // Flush if batch is full
                                    if batch.len() >= self.batch_size {
                                        let offsets_to_send = pending_offsets.clone();
                                        self.flush_batch(&batch, &offsets_to_send, &sender).await?;
                                        batch.clear();
                                        pending_offsets.clear();
                                    }
                                }
                                Ok(None) => {
                                    // Message parsed but no events extracted
                                    // Unexpected message, commit offset immediately so,
                                    // We don't re-read this irrelevant message on restart
                                    // We don't hold up batch processing waiting for messages that have no work
                                    debug!(
                                        topic = %msg.topic(),
                                        partition = msg.partition(),
                                        offset = msg.offset(),
                                        "Message parsed but no events extracted"
                                    );

                                    let mut tpl = TopicPartitionList::new();
                                    tpl.add_partition_offset(
                                        msg.topic(),
                                        msg.partition(),
                                        rdkafka::Offset::Offset(msg.offset() + 1)
                                    )
                                    .map_err(|e| IngestError::kafka(e.to_string()))?;
                                    self.consumer
                                        .commit(&tpl, rdkafka::consumer::CommitMode::Async)
                                        .map_err(|e| IngestError::kafka(e.to_string()))?;
                                }
                                Err(e) => {
                                    error!(
                                        topic = %msg.topic(),
                                        partition = msg.partition(),
                                        offset = msg.offset(),
                                        error = %e,
                                        "Failed to parse message"
                                    );
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!(error = %e, "Kafka error");
                            let _ = sender.send(StreamMessage::Error(e.to_string())).await;
                        }
                        None => {
                            info!("Kafka stream ended");
                            // Flush any pending messages
                            if !batch.is_empty() {
                                let offsets_to_send = pending_offsets.clone();
                                self.flush_batch(&batch, &offsets_to_send, &sender).await?;
                            }
                            let _ = sender.send(StreamMessage::End).await;
                            break;
                        }
                    }
                }
                _ = flush_timer.tick() => {
                    // Flush if timeout reached and we have pending messages
                    if !batch.is_empty() {
                        debug!(count = batch.len(), "Flushing batch due to timeout");
                        let offsets_to_send = pending_offsets.clone();
                        self.flush_batch(&batch, &offsets_to_send, &sender).await?;
                        batch.clear();
                        pending_offsets.clear();
                    }
                }
            }
        }

        Ok(())
    }

    /// Flush a batch of pending messages to the channel.
    async fn flush_batch(
        &self,
        batch: &[PendingMessage],
        offsets: &[(String, i32, i64)],
        sender: &mpsc::Sender<StreamMessage>,
    ) -> Result<(), IngestError> {
        if batch.is_empty() {
            return Ok(());
        }

        // Collect all events from the batch
        let mut all_events = Vec::new();
        for pending in batch {
            all_events.extend(pending.events.clone());
        }

        if !all_events.is_empty() {
            info!(
                event_count = all_events.len(),
                message_count = batch.len(),
                offset_count = offsets.len(),
                "Sending batch of events to processor"
            );
            sender
                .send(StreamMessage::Events {
                    events: all_events,
                    offsets: offsets.to_vec(),
                })
                .await
                .map_err(|e| IngestError::ChannelError(e.to_string()))?;
        }

        Ok(())
    }

    /// Commit offsets for a batch of messages.
    async fn commit_offsets(&self, offsets: &[(String, i32, i64)]) -> Result<(), IngestError> {
        if offsets.is_empty() {
            return Ok(());
        }

        let mut tpl = TopicPartitionList::new();
        for (topic, partition, offset) in offsets {
            tpl.add_partition_offset(topic, *partition, rdkafka::Offset::Offset(offset + 1))
                .map_err(|e| IngestError::kafka(e.to_string()))?;
        }

        self.consumer
            .commit(&tpl, rdkafka::consumer::CommitMode::Async)
            .map_err(|e| IngestError::kafka(e.to_string()))?;

        Ok(())
    }

    /// Parse a Kafka message into pending message data.
    fn parse_message(
        &self,
        msg: &rdkafka::message::BorrowedMessage<'_>,
    ) -> Result<Option<PendingMessage>, IngestError> {
        let payload = match msg.payload() {
            Some(p) => p,
            None => {
                debug!("Received message with empty payload");
                return Ok(None);
            }
        };

        let topic = msg.topic();
        let partition = msg.partition();
        let offset = msg.offset();

        debug!(
            topic = %topic,
            partition = partition,
            offset = offset,
            "Processing message"
        );

        // Parse the message based on topic
        let events = if topic == KNOWLEDGE_EDITS_TOPIC {
            match self.parse_edit_message(payload, msg) {
                Ok(events) => events,
                Err(e) => {
                    error!(
                        topic = %topic,
                        partition = partition,
                        offset = offset,
                        error = %e,
                        "Failed to parse edit message"
                    );
                    return Err(e);
                }
            }
        } else {
            warn!(topic = %topic, "Unknown topic");
            return Ok(None);
        };

        if events.is_empty() {
            debug!(
                topic = %topic,
                partition = partition,
                offset = offset,
                "Message parsed but no events extracted (likely filtered out)"
            );
            return Ok(None);
        }

        Ok(Some(PendingMessage { events }))
    }

    /// Parse a HermesEdit message into entity events.
    fn parse_edit_message(
        &self,
        payload: &[u8],
        msg: &rdkafka::message::BorrowedMessage<'_>,
    ) -> Result<Vec<EntityEvent>, IngestError> {
        let edit = HermesEdit::decode(payload)
            .map_err(|e| IngestError::parse(format!("Failed to decode HermesEdit: {}", e)))?;

        // Parse space_id - it may be hex-encoded UUID bytes or standard UUID string format
        let space_id_str = &edit.space_id;
        let space_id = if space_id_str.len() == 32
            && space_id_str.chars().all(|c| c.is_ascii_hexdigit())
        {
            // Hex-encoded UUID bytes (32 hex chars) - convert to UUID format
            let uuid_str = format!(
                "{}-{}-{}-{}-{}",
                &space_id_str[0..8],
                &space_id_str[8..12],
                &space_id_str[12..16],
                &space_id_str[16..20],
                &space_id_str[20..32]
            );
            Uuid::parse_str(&uuid_str)
                .map_err(|e| IngestError::parse(format!("Invalid hex-encoded space_id: {}", e)))?
        } else {
            // Standard UUID string format
            Uuid::parse_str(space_id_str)
                .map_err(|e| IngestError::parse(format!("Invalid space_id: {}", e)))?
        };

        let mut events = Vec::new();
        let mut skipped_entities = 0;

        debug!(
            space_id = %space_id,
            edit_name = %edit.name,
            op_count = edit.ops.len(),
            "Parsing edit message"
        );

        // Process each operation in the edit
        for op in &edit.ops {
            if let Some(payload) = &op.payload {
                match payload {
                    Payload::UpdateEntity(entity) => {
                        if let Some(event) =
                            self.process_update_entity(entity, space_id, &edit, msg)
                        {
                            events.push(event);
                        } else {
                            skipped_entities += 1;
                        }
                    }
                    Payload::UnsetEntityValues(unset) => {
                        // Handle unsetting entity properties (e.g., clearing name/description)
                        if let Ok(id_bytes) = transform_id_bytes(unset.id.clone()) {
                            let entity_id = Uuid::from_bytes(id_bytes);

                            // Convert property IDs to property key names
                            let mut property_keys = Vec::new();
                            for property_bytes in &unset.properties {
                                if let Ok(prop_id_bytes) =
                                    transform_id_bytes(property_bytes.clone())
                                {
                                    let property_id = bs58::encode(&prop_id_bytes).into_string();

                                    // Map known property IDs to their search index field names
                                    if property_id == NAME_ATTRIBUTE {
                                        property_keys.push("name".to_string());
                                    } else if property_id == DESCRIPTION_ATTRIBUTE {
                                        property_keys.push("description".to_string());
                                    } else if property_id == AVATAR_ATTRIBUTE {
                                        property_keys.push("avatar".to_string());
                                    }
                                    // Other properties are not indexed in search, so we ignore them
                                }
                            }

                            if !property_keys.is_empty() {
                                info!(
                                    entity_id = %entity_id,
                                    space_id = %space_id,
                                    property_keys = ?property_keys,
                                    "Unsetting entity properties"
                                );
                                events.push(EntityEvent::unset_properties(
                                    entity_id,
                                    space_id,
                                    property_keys,
                                ));
                            }
                        } else {
                            debug!(
                                entity_id_bytes = ?unset.id,
                                space_id = %space_id,
                                edit_name = %edit.name,
                                "Skipped unset entity values (invalid entity ID)"
                            );
                        }
                    }
                    _ => {
                        // Other operation types don't affect search index
                        // (CreateRelation, UpdateRelation, DeleteRelation, CreateProperty, UnsetRelationFields)
                        debug!("Skipped operation type (not relevant for search index)");
                    }
                }
            }
        }

        if skipped_entities > 0 {
            debug!(
                skipped_count = skipped_entities,
                "Some entities were skipped during processing"
            );
        }

        Ok(events)
    }

    /// Process an UpdateEntity operation.
    fn process_update_entity(
        &self,
        entity: &wire::pb::grc20::Entity,
        space_id: Uuid,
        edit: &HermesEdit,
        msg: &rdkafka::message::BorrowedMessage<'_>,
    ) -> Option<EntityEvent> {
        let entity_id_bytes = match transform_id_bytes(entity.id.clone()) {
            Ok(bytes) => bytes,
            Err(_) => {
                // Log the full Kafka message when entity ID transformation fails
                let payload = msg.payload().unwrap_or(&[]);
                debug!(
                    topic = %msg.topic(),
                    partition = msg.partition(),
                    offset = msg.offset(),
                    key = ?msg.key(),
                    payload_len = payload.len(),
                    payload_bytes = ?payload,
                    entity_id_bytes = ?entity.id,
                    entity_values_count = entity.values.len(),
                    edit_name = %edit.name,
                    edit_id = ?edit.id,
                    space_id = %space_id,
                    "Skipped entity update (invalid entity ID)"
                );
                return None;
            }
        };
        let entity_id = Uuid::from_bytes(entity_id_bytes);

        // Extract name, description, and avatar from values
        let mut name: Option<String> = None;
        let mut description: Option<String> = None;
        let mut avatar: Option<String> = None;

        for value in &entity.values {
            let property_id_bytes = match transform_id_bytes(value.property.clone()) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            // Convert property ID bytes to base58 or check against known IDs
            let property_id = bs58::encode(&property_id_bytes).into_string();

            if property_id == NAME_ATTRIBUTE {
                name = Some(value.value.clone());
                info!(
                    entity_id = %entity_id,
                    space_id = %space_id,
                    property_id = %property_id,
                    name_value = %value.value,
                    "name attribute detected in property edit"
                );
            } else if property_id == DESCRIPTION_ATTRIBUTE {
                description = Some(value.value.clone());
                info!(
                    entity_id = %entity_id,
                    space_id = %space_id,
                    property_id = %property_id,
                    description_value = %value.value,
                    "description attribute detected in property edit"
                );
            } else if property_id == AVATAR_ATTRIBUTE {
                avatar = Some(value.value.clone());
                info!(
                    entity_id = %entity_id,
                    space_id = %space_id,
                    property_id = %property_id,
                    avatar_value = %value.value,
                    "avatar attribute detected in property edit"
                );
            }
        }

        Some(EntityEvent::upsert(
            entity_id,
            space_id,
            name,
            description,
            avatar,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(KNOWLEDGE_EDITS_TOPIC, "knowledge.edits");
        assert_eq!(DEFAULT_BATCH_SIZE, 50);
        assert_eq!(DEFAULT_BATCH_TIMEOUT_MS, 1000);
    }
}
