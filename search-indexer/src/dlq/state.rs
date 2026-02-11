//! DLQ state management for tracking poisoned entities.
//!
//! Maintains an in-memory HashSet of poisoned entity keys, rebuilt from
//! the Kafka DLQ topic on startup. No external persistence required -
//! the DLQ topic itself is the source of truth.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use hermes_instrumentation::{debug, info, warn};
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::Message;

use super::types::DlqRecord;
use crate::consumer::kafka_config::create_client_config;
use crate::errors::IngestError;

/// Manages the set of poisoned entities.
///
/// An entity becomes "poisoned" when an operation for it fails in OpenSearch
/// and is sent to the DLQ. The poisoned set is used for observability: when
/// a subsequent operation for a poisoned entity is processed, it is logged
/// at warn level with full operation details (allowing reconstruction of
/// entity state when the DLQ is later replayed).
///
/// The poisoned set is rebuilt from the DLQ Kafka topic on startup, so it
/// survives pod restarts without any additional persistence layer.
pub struct DlqState {
    /// In-memory set of poisoned entity document keys (`{entity_id}_{space_id}`).
    poisoned_entities: HashSet<String>,
    /// Maximum poisoned entities before circuit breaker trips.
    max_poisoned_entities: usize,
}

impl DlqState {
    /// Create a new DLQ state manager by scanning the DLQ Kafka topic.
    ///
    /// Reads all existing messages from the DLQ topic to rebuild the
    /// poisoned entity set. This is a one-time startup operation.
    pub async fn new(
        broker: &str,
        topic: &str,
        max_poisoned_entities: usize,
    ) -> Result<Self, IngestError> {
        let mut state = Self {
            poisoned_entities: HashSet::new(),
            max_poisoned_entities,
        };

        state.scan_dlq_topic(broker, topic).await?;

        Ok(state)
    }

    /// Scan the DLQ Kafka topic to rebuild the poisoned entity set.
    ///
    /// Creates a temporary consumer, reads all messages from the beginning
    /// of the topic, and extracts entity keys from DlqRecord payloads.
    async fn scan_dlq_topic(&mut self, broker: &str, topic: &str) -> Result<(), IngestError> {
        let broker = broker.to_string();
        let topic = topic.to_string();

        let result = tokio::task::spawn_blocking(move || {
            Self::scan_dlq_topic_blocking(&broker, &topic)
        })
        .await
        .map_err(|e| IngestError::dlq(format!("DLQ scan task panicked: {}", e)))?;

        match result {
            Ok(keys) => {
                let count = keys.len();
                self.poisoned_entities = keys;
                if count > 0 {
                    info!(
                        count = count,
                        "Loaded poisoned entities from DLQ topic"
                    );
                } else {
                    debug!("No poisoned entities found in DLQ topic");
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Blocking implementation of DLQ topic scan.
    fn scan_dlq_topic_blocking(
        broker: &str,
        topic: &str,
    ) -> Result<HashSet<String>, IngestError> {
        let group_id = format!(
            "search-indexer-dlq-scan-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        let config = create_client_config(broker, &group_id);
        let consumer: BaseConsumer = config
            .create()
            .map_err(|e| IngestError::dlq(format!("Failed to create DLQ scan consumer: {}", e)))?;

        // Get topic metadata
        let metadata = consumer
            .fetch_metadata(Some(topic), Duration::from_secs(10))
            .map_err(|e| {
                IngestError::dlq(format!("Failed to fetch DLQ topic metadata: {}", e))
            })?;

        let topic_metadata = metadata.topics().iter().find(|t| t.name() == topic);
        let partitions = match topic_metadata {
            None => {
                info!("DLQ topic not found in metadata, starting with empty poisoned set");
                return Ok(HashSet::new());
            }
            Some(t) if t.partitions().is_empty() => {
                info!("DLQ topic has no partitions, starting with empty poisoned set");
                return Ok(HashSet::new());
            }
            Some(t) => t.partitions(),
        };

        // Get high watermarks for each partition to know when to stop
        let mut partition_targets: HashMap<i32, i64> = HashMap::new();
        for p in partitions {
            match consumer.fetch_watermarks(topic, p.id(), Duration::from_secs(10)) {
                Ok((_, high)) if high > 0 => {
                    partition_targets.insert(p.id(), high);
                }
                Ok(_) => {} // Empty partition
                Err(e) => {
                    warn!(
                        partition = p.id(),
                        error = %e,
                        "Failed to fetch watermarks for DLQ partition, skipping"
                    );
                }
            }
        }

        if partition_targets.is_empty() {
            info!("DLQ topic is empty, starting with empty poisoned set");
            return Ok(HashSet::new());
        }

        // Assign partitions at the beginning
        let mut tpl = rdkafka::TopicPartitionList::new();
        for &partition_id in partition_targets.keys() {
            tpl.add_partition_offset(topic, partition_id, rdkafka::Offset::Beginning)
                .map_err(|e| {
                    IngestError::dlq(format!("Failed to add partition offset: {}", e))
                })?;
        }
        consumer.assign(&tpl).map_err(|e| {
            IngestError::dlq(format!("Failed to assign DLQ partitions: {}", e))
        })?;

        // Read all messages until we reach the watermarks
        let mut poisoned = HashSet::new();
        let mut partition_progress: HashMap<i32, i64> = HashMap::new();
        let mut total_messages = 0u64;

        loop {
            match consumer.poll(Duration::from_secs(5)) {
                Some(Ok(msg)) => {
                    let partition = msg.partition();
                    let offset = msg.offset();
                    partition_progress.insert(partition, offset + 1);
                    total_messages += 1;

                    if let Some(payload) = msg.payload() {
                        if let Ok(record) = serde_json::from_slice::<DlqRecord>(payload) {
                            if !record.entity_id.is_empty() && !record.space_id.is_empty() {
                                let key =
                                    format!("{}_{}", record.entity_id, record.space_id);
                                poisoned.insert(key);
                            }
                        }
                    }

                    // Check if we've consumed up to all watermarks
                    let all_caught_up = partition_targets.iter().all(|(pid, target)| {
                        partition_progress.get(pid).copied().unwrap_or(0) >= *target
                    });
                    if all_caught_up {
                        break;
                    }
                }
                Some(Err(e)) => {
                    warn!(error = %e, "Error reading DLQ message during scan, skipping");
                }
                None => {
                    // Timeout with no messages - we're done
                    break;
                }
            }
        }

        debug!(
            total_messages = total_messages,
            unique_entities = poisoned.len(),
            "DLQ topic scan complete"
        );

        Ok(poisoned)
    }

    /// Check if an entity is poisoned.
    pub fn is_poisoned(&self, entity_key: &str) -> bool {
        self.poisoned_entities.contains(entity_key)
    }

    /// Get the current number of poisoned entities.
    pub fn poisoned_count(&self) -> usize {
        self.poisoned_entities.len()
    }

    /// Check if adding a new poisoned entity would exceed the circuit breaker limit.
    pub fn would_exceed_limit(&self, entity_key: &str) -> bool {
        if self.poisoned_entities.contains(entity_key) {
            // Already poisoned, won't increase the count
            return false;
        }
        self.poisoned_entities.len() >= self.max_poisoned_entities
    }

    /// Poison an entity. Adds it to the in-memory set.
    ///
    /// Returns `true` if the entity was newly poisoned, `false` if already poisoned.
    pub fn poison_entity(&mut self, entity_key: &str) -> bool {
        self.poisoned_entities.insert(entity_key.to_string())
    }

    /// Create a DLQ state for testing without scanning a Kafka topic.
    pub fn new_for_test(max_poisoned_entities: usize) -> Self {
        Self {
            poisoned_entities: HashSet::new(),
            max_poisoned_entities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poison_entity_new() {
        let mut state = DlqState::new_for_test(100);
        assert!(state.poison_entity("entity1_space1"));
    }

    #[test]
    fn test_poison_entity_duplicate() {
        let mut state = DlqState::new_for_test(100);
        assert!(state.poison_entity("entity1_space1"));
        assert!(!state.poison_entity("entity1_space1"));
    }

    #[test]
    fn test_is_poisoned() {
        let mut state = DlqState::new_for_test(100);
        state.poison_entity("entity1_space1");
        assert!(state.is_poisoned("entity1_space1"));
    }

    #[test]
    fn test_is_not_poisoned() {
        let state = DlqState::new_for_test(100);
        assert!(!state.is_poisoned("entity1_space1"));
    }

    #[test]
    fn test_poisoned_count() {
        let mut state = DlqState::new_for_test(100);
        assert_eq!(state.poisoned_count(), 0);
        state.poison_entity("entity1_space1");
        assert_eq!(state.poisoned_count(), 1);
        state.poison_entity("entity2_space1");
        assert_eq!(state.poisoned_count(), 2);
        // Duplicate should not increase count
        state.poison_entity("entity1_space1");
        assert_eq!(state.poisoned_count(), 2);
    }

    #[test]
    fn test_would_exceed_limit_at_max() {
        let mut state = DlqState::new_for_test(2);
        state.poison_entity("entity1_space1");
        state.poison_entity("entity2_space1");
        // At limit, new entity would exceed
        assert!(state.would_exceed_limit("entity3_space1"));
    }

    #[test]
    fn test_would_exceed_limit_below_max() {
        let mut state = DlqState::new_for_test(3);
        state.poison_entity("entity1_space1");
        // Below limit, new entity would not exceed
        assert!(!state.would_exceed_limit("entity2_space1"));
    }

    #[test]
    fn test_would_exceed_limit_already_poisoned() {
        let mut state = DlqState::new_for_test(2);
        state.poison_entity("entity1_space1");
        state.poison_entity("entity2_space1");
        // Already poisoned entity won't increase count
        assert!(!state.would_exceed_limit("entity1_space1"));
    }

    #[test]
    fn test_multiple_entities() {
        let mut state = DlqState::new_for_test(100);
        let keys = vec![
            "e1_s1", "e2_s1", "e3_s2", "e4_s2", "e5_s3",
        ];
        for key in &keys {
            state.poison_entity(key);
        }
        assert_eq!(state.poisoned_count(), 5);
        for key in &keys {
            assert!(state.is_poisoned(key));
        }
        assert!(!state.is_poisoned("nonexistent_key"));
    }
}
