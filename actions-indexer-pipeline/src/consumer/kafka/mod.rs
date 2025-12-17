//! Kafka consumer module for the actions indexer pipeline.
//!
//! Provides configuration and utilities for consuming from Kafka topics.

mod config;
mod provider;

pub use config::ConsumerConfig;
pub use provider::KafkaStreamProvider;

