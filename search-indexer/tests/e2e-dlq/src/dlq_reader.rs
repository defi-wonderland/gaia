use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::Message;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, info, warn};

/// DLQ record as stored in the Kafka DLQ topic.
/// Redefined locally since the e2e test is a standalone project.
#[derive(Debug, Clone, Deserialize)]
pub struct DlqRecord {
    pub dlq_id: String,
    pub entity_id: String,
    pub space_id: String,
    pub operation_type: String,
    pub error_message: String,
    pub source_batch_type: String,
    pub source_topic: Option<String>,
    pub source_partition: Option<i32>,
    pub source_offset: Option<i64>,
    pub failed_at: String,
    pub retry_count: u32,
    pub max_retries: u32,
    pub operation_payload: serde_json::Value,
}

/// Reads records from the DLQ Kafka topic.
pub struct DlqReader {
    consumer: BaseConsumer,
    topic: String,
}

impl DlqReader {
    /// Create a new DLQ reader.
    pub fn new(broker: &str, topic: &str) -> Result<Self> {
        let group_id = format!(
            "e2e-dlq-reader-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        let consumer: BaseConsumer = ClientConfig::new()
            .set("bootstrap.servers", broker)
            .set("group.id", &group_id)
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .create()
            .context("Failed to create DLQ reader consumer")?;

        info!("Created DLQ reader for topic '{}' with group '{}'", topic, group_id);

        Ok(Self {
            consumer,
            topic: topic.to_string(),
        })
    }

    /// Read all records from the DLQ topic.
    ///
    /// Returns all DLQ records that can be deserialized. Reads until
    /// all partitions are caught up to their high watermarks, or until
    /// the timeout is reached with no new messages.
    pub fn read_all_records(&self, poll_timeout: Duration) -> Result<Vec<DlqRecord>> {
        // Get topic metadata
        let metadata = self
            .consumer
            .fetch_metadata(Some(&self.topic), Duration::from_secs(10))
            .context("Failed to fetch DLQ topic metadata")?;

        let topic_metadata = metadata.topics().iter().find(|t| t.name() == self.topic);
        let partitions = match topic_metadata {
            None => {
                info!("DLQ topic not found in metadata");
                return Ok(Vec::new());
            }
            Some(t) if t.partitions().is_empty() => {
                info!("DLQ topic has no partitions");
                return Ok(Vec::new());
            }
            Some(t) => t.partitions(),
        };

        // Get high watermarks to know when to stop
        let mut partition_targets = std::collections::HashMap::new();
        for p in partitions {
            match self
                .consumer
                .fetch_watermarks(&self.topic, p.id(), Duration::from_secs(10))
            {
                Ok((_, high)) if high > 0 => {
                    partition_targets.insert(p.id(), high);
                }
                Ok(_) => {} // Empty partition
                Err(e) => {
                    warn!(
                        partition = p.id(),
                        error = %e,
                        "Failed to fetch watermarks for DLQ partition"
                    );
                }
            }
        }

        if partition_targets.is_empty() {
            info!("DLQ topic is empty");
            return Ok(Vec::new());
        }

        // Assign partitions at the beginning
        let mut tpl = rdkafka::TopicPartitionList::new();
        for &partition_id in partition_targets.keys() {
            tpl.add_partition_offset(&self.topic, partition_id, rdkafka::Offset::Beginning)
                .context("Failed to add partition offset")?;
        }
        self.consumer
            .assign(&tpl)
            .context("Failed to assign DLQ partitions")?;

        // Read all messages
        let mut records = Vec::new();
        let mut partition_progress = std::collections::HashMap::new();

        loop {
            match self.consumer.poll(poll_timeout) {
                Some(Ok(msg)) => {
                    let partition = msg.partition();
                    let offset = msg.offset();
                    partition_progress.insert(partition, offset + 1);

                    if let Some(payload) = msg.payload() {
                        match serde_json::from_slice::<DlqRecord>(payload) {
                            Ok(record) => {
                                debug!(
                                    entity_id = %record.entity_id,
                                    space_id = %record.space_id,
                                    "Read DLQ record"
                                );
                                records.push(record);
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to deserialize DLQ record, skipping");
                            }
                        }
                    }

                    // Check if caught up
                    let all_caught_up = partition_targets.iter().all(|(pid, target)| {
                        partition_progress.get(pid).copied().unwrap_or(0) >= *target
                    });
                    if all_caught_up {
                        break;
                    }
                }
                Some(Err(e)) => {
                    warn!(error = %e, "Error reading DLQ message");
                }
                None => {
                    // Timeout with no messages
                    break;
                }
            }
        }

        info!(
            total_records = records.len(),
            "DLQ topic read complete"
        );

        Ok(records)
    }

    /// Count the number of records in the DLQ topic.
    pub fn count_records(&self) -> Result<usize> {
        let records = self.read_all_records(Duration::from_secs(5))?;
        Ok(records.len())
    }
}
