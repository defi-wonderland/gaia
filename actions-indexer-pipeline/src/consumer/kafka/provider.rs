//! Kafka stream provider for consuming vote events from Hermes.
//!
//! This module provides a Kafka-based implementation of the `ConsumeActionsStream` trait,
//! enabling the actions indexer to consume vote events from the Hermes Kafka stream
//! instead of directly from substreams.

use async_trait::async_trait;
use futures03::StreamExt;
use hermes_kafka::{Consumer, Message, StreamConsumer};
use hermes_schema::pb::voting::HermesVoteCast;
use prost::Message as ProstMessage;
use tokio::sync::mpsc;

use crate::consumer::{BlockDataMessage, ConsumeActionsStream, StreamMessage};
use crate::errors::ConsumerError;

use super::conversion::hermes_vote_to_action_raw;
use super::ConsumerConfig;

/// Kafka stream provider for consuming action events from Hermes.
///
/// This provider connects to a Kafka topic (e.g., `curation.votes`) and consumes
/// `HermesVoteCast` protobuf messages, converting them to `ActionRaw` for processing
/// by the existing pipeline.
pub struct KafkaStreamProvider {
    config: ConsumerConfig,
}

impl KafkaStreamProvider {
    /// Creates a new `KafkaStreamProvider` with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Kafka consumer configuration including broker, group_id, and topic
    ///
    /// # Returns
    ///
    /// A new `KafkaStreamProvider` instance.
    pub fn new(config: ConsumerConfig) -> Self {
        Self { config }
    }

    /// Creates a new `KafkaStreamProvider` from environment variables.
    ///
    /// Uses the following defaults:
    /// - Broker: `localhost:9092`
    /// - Group ID: `actions-indexer`
    /// - Topic: `curation.votes`
    ///
    /// # Returns
    ///
    /// A new `KafkaStreamProvider` configured from environment variables.
    pub fn from_env() -> Self {
        Self {
            config: ConsumerConfig::from_env("localhost:9092", "actions-indexer", "curation.votes"),
        }
    }

    /// Creates and subscribes a Kafka consumer to the configured topic.
    fn create_subscribed_consumer(&self) -> Result<StreamConsumer, ConsumerError> {
        let consumer = self.config.create_consumer()
            .map_err(|e| ConsumerError::KafkaConnection(e.to_string()))?;
        
        consumer.subscribe(&[&self.config.topic])
            .map_err(|e| ConsumerError::KafkaSubscription(e.to_string()))?;
        
        Ok(consumer)
    }
}

#[async_trait]
impl ConsumeActionsStream for KafkaStreamProvider {
    /// Streams action events from Kafka through a channel.
    ///
    /// This method:
    /// 1. Creates a Kafka consumer and subscribes to the configured topic
    /// 2. Polls for messages in a loop using async stream
    /// 3. Decodes `HermesVoteCast` protobuf messages from message payload
    /// 4. Converts to `ActionRaw` and sends through the channel as `BlockData`
    /// 5. Commits offsets after successful channel send (at-least-once delivery)
    ///
    /// # Arguments
    ///
    /// * `sender` - Channel sender for streaming messages to the orchestrator
    /// * `_cursor` - Ignored for Kafka (offset tracking is handled by consumer groups)
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or a `ConsumerError` if streaming fails.
    async fn stream_events(
        &self,
        sender: mpsc::Sender<StreamMessage>,
        _cursor: Option<String>,
    ) -> Result<(), ConsumerError> {
        let consumer = self.create_subscribed_consumer()?;
        
        let now = chrono::Utc::now();
        println!("{} - KafkaStreamProvider: Connected to broker at {}", now.to_rfc3339(), self.config.broker);
        println!("{} - KafkaStreamProvider: Subscribed to topic '{}'", now.to_rfc3339(), self.config.topic);
        println!("{} - KafkaStreamProvider: Consumer group '{}'", now.to_rfc3339(), self.config.group_id);

        // Create the message stream from the consumer
        let mut message_stream = consumer.stream();
        
        // Consumption loop
        while let Some(message_result) = message_stream.next().await {
            match message_result {
                Ok(borrowed_message) => {
                    // Get the message payload
                    let payload = match borrowed_message.payload() {
                        Some(payload) => payload,
                        None => {
                            // Empty payload - skip this message
                            eprintln!("KafkaStreamProvider: Received message with empty payload, skipping");
                            continue;
                        }
                    };

                    // Decode the HermesVoteCast protobuf message
                    let vote_cast = match HermesVoteCast::decode(payload) {
                        Ok(vote) => vote,
                        Err(e) => {
                            eprintln!("KafkaStreamProvider: Failed to decode HermesVoteCast: {}", e);
                            // Send error through channel but continue processing
                            sender.send(StreamMessage::Error(
                                ConsumerError::DecodingActions(format!("protobuf decode error: {}", e))
                            ))
                            .await
                            .map_err(|e| ConsumerError::ChannelSend(e.to_string()))?;
                            continue;
                        }
                    };

                    // Convert to ActionRaw
                    let action_raw = match hermes_vote_to_action_raw(&vote_cast) {
                        Ok(action) => action,
                        Err(e) => {
                            eprintln!("KafkaStreamProvider: Failed to convert vote to action: {}", e);
                            // Send error through channel but continue processing
                            sender.send(StreamMessage::Error(e))
                                .await
                                .map_err(|e| ConsumerError::ChannelSend(e.to_string()))?;
                            continue;
                        }
                    };

                    // Extract cursor and block number from the vote metadata
                    let (cursor, block_number) = match &vote_cast.meta {
                        Some(meta) => (meta.cursor.clone(), meta.block_number as i64),
                        None => {
                            // Use Kafka offset as fallback cursor
                            let offset_cursor = format!(
                                "{}:{}:{}",
                                borrowed_message.topic(),
                                borrowed_message.partition(),
                                borrowed_message.offset()
                            );
                            (offset_cursor, 0)
                        }
                    };

                    // Send the action through the channel
                    sender.send(StreamMessage::BlockData(BlockDataMessage {
                        actions: vec![action_raw],
                        cursor,
                        block_number,
                    }))
                    .await
                    .map_err(|e| ConsumerError::ChannelSend(e.to_string()))?;

                    // Commit the offset after successful processing (at-least-once delivery)
                    // Note: Task 6 will add more sophisticated error handling and retry logic
                    if let Err(e) = consumer.commit_message(&borrowed_message, rdkafka::consumer::CommitMode::Async) {
                        eprintln!("KafkaStreamProvider: Failed to commit offset: {}", e);
                        // Continue processing - the message will be redelivered if needed
                    }
                }
                Err(e) => {
                    eprintln!("KafkaStreamProvider: Kafka consume error: {}", e);
                    // Send error through channel
                    sender.send(StreamMessage::Error(
                        ConsumerError::KafkaConsume(e.to_string())
                    ))
                    .await
                    .map_err(|e| ConsumerError::ChannelSend(e.to_string()))?;
                    
                    // For transient errors, continue; for fatal errors, break
                    // Most rdkafka errors are recoverable, so we continue
                }
            }
        }

        // Stream ended (consumer was closed or disconnected)
        let now = chrono::Utc::now();
        println!("{} - KafkaStreamProvider: Stream ended", now.to_rfc3339());
        
        sender.send(StreamMessage::StreamEnd)
            .await
            .map_err(|e| ConsumerError::ChannelSend(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_stream_provider_new() {
        let config = ConsumerConfig::new("localhost:9092", "test-group", "test-topic");
        let provider = KafkaStreamProvider::new(config);
        
        assert_eq!(provider.config.broker, "localhost:9092");
        assert_eq!(provider.config.group_id, "test-group");
        assert_eq!(provider.config.topic, "test-topic");
    }

    #[test]
    fn test_kafka_stream_provider_from_env() {
        let provider = KafkaStreamProvider::from_env();
        
        // Uses defaults when env vars not set
        assert_eq!(provider.config.broker, "localhost:9092");
        assert_eq!(provider.config.group_id, "actions-indexer");
        assert_eq!(provider.config.topic, "curation.votes");
    }
}

