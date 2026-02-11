//! DLQ record types for the search indexer dead letter queue.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A failed operation record sent to the Dead Letter Queue.
///
/// Contains enough information for debugging, retry, and operational decisions.
/// Serialized as JSON and published to the Kafka DLQ topic.
///
/// # Serialized format
///
/// Each record is published as a JSON message to the DLQ Kafka topic:
///
/// ```json
/// {
///   "dlq_id": "a1b2c3d4-...",
///   "entity_id": "e5f6a7b8-...",
///   "space_id": "c9d0e1f2-...",
///   "operation_type": "Update",
///   "error_message": "bulk operation failed: ...",
///   "source_batch_type": "entities",
///   "source_topic": null,
///   "source_partition": null,
///   "source_offset": null,
///   "failed_at": "2025-01-15T10:30:00Z",
///   "retry_count": 0,
///   "max_retries": 3,
///   "operation_payload": {
///     "Update": {
///       "entity_id": "e5f6a7b8-...",
///       "space_id": "c9d0e1f2-...",
///       "name": "My Entity",
///       "description": null,
///       "avatar": null,
///       "cover": null,
///       "add_type_relation": null,
///       "entity_global_score": null,
///       "space_score": null,
///       "entity_space_score": null,
///       "deleted": null
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqRecord {
    /// Unique ID for this DLQ record (UUID).
    pub dlq_id: String,

    /// The entity_id of the failed operation.
    pub entity_id: String,

    /// The space_id of the failed operation.
    pub space_id: String,

    /// The operation type that failed (e.g., "Update", "Unset", "RemoveTypeRelation").
    pub operation_type: String,

    /// The error message from the failed operation.
    pub error_message: String,

    /// Source batch type: "entities" or "scores".
    pub source_batch_type: String,

    /// Original Kafka source topic (for traceability).
    pub source_topic: Option<String>,

    /// Original Kafka source partition (for traceability).
    pub source_partition: Option<i32>,

    /// Original Kafka source offset (for traceability).
    pub source_offset: Option<i64>,

    /// When the failure occurred.
    pub failed_at: DateTime<Utc>,

    /// Retry count (0 for first failure, incremented on each retry attempt).
    pub retry_count: u32,

    /// Maximum retries before permanent discard.
    pub max_retries: u32,

    /// The full operation that failed, serialized as a JSON-tagged enum matching
    /// `EntityOperation` from `search-indexer-repository`. The outer key is the variant
    /// name, and the value is the variant's struct fields:
    ///
    /// - `{"Update": {"entity_id": "...", "space_id": "...", "name": "...", ...}}`
    /// - `{"Delete": {"entity_id": "...", "space_id": "..."}}`
    /// - `{"Unset": {"entity_id": "...", "space_id": "...", "property_keys": [...]}}`
    /// - `{"RemoveTypeRelationById": {"relation_id": "..."}}`
    /// - `{"UpdateEntityGlobalScore": {"entity_id": "...", "score": 0.5}}`
    /// - `{"UpdateSpaceScore": {"space_id": "...", "score": 0.5}}`
    /// - `{"UpdateEntitySpaceScore": {"entity_id": "...", "space_id": "...", "score": 0.5}}`
    ///
    /// To deserialize back: `serde_json::from_value::<EntityOperation>(record.operation_payload)`.
    /// Will be `null` if the operation could not be serialized (logged as an error).
    pub operation_payload: serde_json::Value,
}

/// Configuration for the DLQ system.
#[derive(Debug, Clone)]
pub struct DlqConfig {
    /// Whether DLQ is enabled. When disabled, partial failures revert to NACK behavior.
    pub enabled: bool,

    /// Maximum retry attempts before discarding during Phase 2 retry.
    pub max_retries: u32,

    /// Kafka topic name for DLQ events (with environment prefix applied).
    pub topic: String,

    /// Maximum number of poisoned entities before the indexer shuts down (circuit breaker).
    pub max_poisoned_entities: usize,
}

impl DlqConfig {
    /// Default base topic name for DLQ events.
    const DEFAULT_DLQ_TOPIC: &'static str = "search-indexer.dlq";

    /// Default maximum retries.
    const DEFAULT_MAX_RETRIES: u32 = 3;

    /// Default maximum poisoned entities (circuit breaker).
    const DEFAULT_MAX_POISONED_ENTITIES: usize = 10_000;

    /// Build DLQ configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `DLQ_ENABLED`: Enable/disable DLQ (default: true)
    /// - `DLQ_MAX_RETRIES`: Max retry attempts (default: 3)
    /// - `DLQ_TOPIC`: Base DLQ topic name (default: search-indexer.dlq)
    /// - `DLQ_MAX_POISONED_ENTITIES`: Circuit breaker limit (default: 10000)
    pub fn from_env(topic_prefix: &str) -> Self {
        let enabled = std::env::var("DLQ_ENABLED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let max_retries = std::env::var("DLQ_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::DEFAULT_MAX_RETRIES);

        let base_topic = std::env::var("DLQ_TOPIC")
            .unwrap_or_else(|_| Self::DEFAULT_DLQ_TOPIC.to_string());
        let topic = format!("{}{}", topic_prefix, base_topic);

        let max_poisoned_entities = std::env::var("DLQ_MAX_POISONED_ENTITIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::DEFAULT_MAX_POISONED_ENTITIES);

        Self {
            enabled,
            max_retries,
            topic,
            max_poisoned_entities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dlq_record() -> DlqRecord {
        DlqRecord {
            dlq_id: "test-dlq-id-123".to_string(),
            entity_id: "entity-abc".to_string(),
            space_id: "space-xyz".to_string(),
            operation_type: "Update".to_string(),
            error_message: "Simulated failure".to_string(),
            source_batch_type: "entities".to_string(),
            source_topic: Some("knowledge.edits".to_string()),
            source_partition: Some(0),
            source_offset: Some(42),
            failed_at: Utc::now(),
            retry_count: 0,
            max_retries: 3,
            operation_payload: serde_json::json!({
                "Update": {
                    "entity_id": "entity-abc",
                    "space_id": "space-xyz",
                    "name": "Test Entity",
                    "description": null,
                    "avatar": null,
                    "cover": null,
                    "add_type_relation": null,
                    "entity_global_score": null,
                    "space_score": null,
                    "entity_space_score": null,
                    "deleted": null
                }
            }),
        }
    }

    #[test]
    fn test_dlq_record_serialization_roundtrip() {
        let record = sample_dlq_record();
        let json = serde_json::to_string(&record).expect("serialize");
        let deserialized: DlqRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(record.dlq_id, deserialized.dlq_id);
        assert_eq!(record.entity_id, deserialized.entity_id);
        assert_eq!(record.space_id, deserialized.space_id);
        assert_eq!(record.operation_type, deserialized.operation_type);
        assert_eq!(record.error_message, deserialized.error_message);
        assert_eq!(record.source_batch_type, deserialized.source_batch_type);
        assert_eq!(record.source_topic, deserialized.source_topic);
        assert_eq!(record.source_partition, deserialized.source_partition);
        assert_eq!(record.source_offset, deserialized.source_offset);
        assert_eq!(record.retry_count, deserialized.retry_count);
        assert_eq!(record.max_retries, deserialized.max_retries);
        assert_eq!(record.operation_payload, deserialized.operation_payload);
    }

    #[test]
    fn test_dlq_record_json_fields() {
        let record = sample_dlq_record();
        let json_value: serde_json::Value =
            serde_json::to_value(&record).expect("to_value");
        let obj = json_value.as_object().expect("should be object");
        let expected_fields = [
            "dlq_id",
            "entity_id",
            "space_id",
            "operation_type",
            "error_message",
            "source_batch_type",
            "source_topic",
            "source_partition",
            "source_offset",
            "failed_at",
            "retry_count",
            "max_retries",
            "operation_payload",
        ];
        for field in &expected_fields {
            assert!(obj.contains_key(*field), "missing field: {}", field);
        }
    }

    #[test]
    fn test_dlq_config_defaults() {
        // Clear DLQ env vars to ensure defaults
        std::env::remove_var("DLQ_ENABLED");
        std::env::remove_var("DLQ_MAX_RETRIES");
        std::env::remove_var("DLQ_TOPIC");
        std::env::remove_var("DLQ_MAX_POISONED_ENTITIES");

        let config = DlqConfig::from_env("test.");
        assert!(config.enabled);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.topic, "test.search-indexer.dlq");
        assert_eq!(config.max_poisoned_entities, 10_000);
    }
}
