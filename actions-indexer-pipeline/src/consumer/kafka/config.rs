//! Kafka consumer configuration.
//!
//! Provides configuration structures and utilities for creating Kafka consumers.

use std::env;

use anyhow::Result;
use hermes_kafka::{ClientConfig, StreamConsumer};

/// Configuration for creating a Kafka consumer.
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Kafka broker address (e.g., "localhost:9092")
    pub broker: String,
    /// Consumer group ID for offset tracking
    pub group_id: String,
    /// Topic to consume from
    pub topic: String,
    /// SASL username (enables SASL/SSL if set)
    pub username: Option<String>,
    /// SASL password (required if username is set)
    pub password: Option<String>,
    /// Custom CA certificate in PEM format
    pub ssl_ca_pem: Option<String>,
}

impl ConsumerConfig {
    /// Create a new ConsumerConfig with the given broker, group_id, and topic.
    pub fn new(
        broker: impl Into<String>,
        group_id: impl Into<String>,
        topic: impl Into<String>,
    ) -> Self {
        Self {
            broker: broker.into(),
            group_id: group_id.into(),
            topic: topic.into(),
            username: None,
            password: None,
            ssl_ca_pem: None,
        }
    }

    /// Create a ConsumerConfig from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `KAFKA_BROKER` - Broker address (uses provided default if not set)
    /// - `KAFKA_CONSUMER_GROUP` - Consumer group ID (uses provided default if not set)
    /// - `KAFKA_TOPIC` - Topic to consume from (uses provided default if not set)
    /// - `KAFKA_USERNAME` - SASL username (optional)
    /// - `KAFKA_PASSWORD` - SASL password (optional)
    /// - `KAFKA_SSL_CA_PEM` - Custom CA cert in PEM format (optional)
    pub fn from_env(
        default_broker: &str,
        default_group_id: &str,
        default_topic: &str,
    ) -> Self {
        Self {
            broker: env::var("KAFKA_BROKER").unwrap_or_else(|_| default_broker.to_string()),
            group_id: env::var("KAFKA_CONSUMER_GROUP").unwrap_or_else(|_| default_group_id.to_string()),
            topic: env::var("KAFKA_TOPIC").unwrap_or_else(|_| default_topic.to_string()),
            username: env::var("KAFKA_USERNAME").ok(),
            password: env::var("KAFKA_PASSWORD").ok(),
            ssl_ca_pem: env::var("KAFKA_SSL_CA_PEM").ok(),
        }
    }

    /// Set SASL credentials.
    pub fn with_credentials(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    /// Set custom CA certificate.
    pub fn with_ssl_ca(mut self, ca_pem: String) -> Self {
        self.ssl_ca_pem = Some(ca_pem);
        self
    }

    /// Create a Kafka StreamConsumer from this configuration.
    ///
    /// Configures the consumer with:
    /// - Manual offset commit (enable.auto.commit = false)
    /// - Read from earliest offset if no committed offset exists
    /// - SASL/SSL authentication if credentials are provided
    ///
    /// # Returns
    ///
    /// A configured `StreamConsumer` ready to subscribe to topics.
    pub fn create_consumer(&self) -> Result<StreamConsumer> {
        let mut client_config = ClientConfig::new();

        client_config
            .set("bootstrap.servers", &self.broker)
            .set("group.id", &self.group_id)
            .set("enable.auto.commit", "false") // Manual commit for at-least-once delivery
            .set("auto.offset.reset", "earliest") // Start from beginning if no committed offset
            .set("session.timeout.ms", "30000")
            .set("heartbeat.interval.ms", "10000");

        // If SASL credentials are provided, enable SASL/SSL (for managed Kafka)
        // Otherwise, use plaintext (for local development)
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            client_config
                .set("security.protocol", "SASL_SSL")
                .set("sasl.mechanisms", "PLAIN")
                .set("sasl.username", username)
                .set("sasl.password", password);

            // Use custom CA certificate if provided
            if let Some(ca_pem) = &self.ssl_ca_pem {
                client_config.set("ssl.ca.pem", ca_pem);
            }
        }

        Ok(client_config.create()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consumer_config_new() {
        let config = ConsumerConfig::new("localhost:9092", "test-group", "test-topic");
        
        assert_eq!(config.broker, "localhost:9092");
        assert_eq!(config.group_id, "test-group");
        assert_eq!(config.topic, "test-topic");
        assert!(config.username.is_none());
        assert!(config.password.is_none());
        assert!(config.ssl_ca_pem.is_none());
    }

    #[test]
    fn test_consumer_config_with_credentials() {
        let config = ConsumerConfig::new("localhost:9092", "test-group", "test-topic")
            .with_credentials("user".to_string(), "pass".to_string());
        
        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(config.password, Some("pass".to_string()));
    }

    #[test]
    fn test_consumer_config_with_ssl_ca() {
        let config = ConsumerConfig::new("localhost:9092", "test-group", "test-topic")
            .with_ssl_ca("-----BEGIN CERTIFICATE-----".to_string());
        
        assert_eq!(config.ssl_ca_pem, Some("-----BEGIN CERTIFICATE-----".to_string()));
    }
}

