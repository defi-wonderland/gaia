//! This module defines the `Orchestrator` responsible for coordinating the
//! action processing pipeline.
//! It integrates the consumer, processor, and loader components to manage the
//! flow of action events from ingestion to persistence.
use crate::errors::OrchestratorError;
use crate::consumer::{ActionsConsumer, StreamMessage};
use crate::processor::ActionsProcessor;
use crate::loader::ActionsLoader;
use tokio::sync::mpsc;
use std::process::exit;

/// `Orchestrator` is responsible for coordinating the consumption, processing,
/// and loading of actions.
///
/// It holds references to the `ConsumeActions`, `ProcessActions`, and
/// `ActionsLoader` traits, enabling a flexible and extensible pipeline.
pub struct Orchestrator {
    pub actions_consumer: Box<ActionsConsumer>,
    pub actions_processor: Box<ActionsProcessor>,
    pub actions_loader: Box<ActionsLoader>,
}

impl Orchestrator {
    /// Creates a new `Orchestrator` instance.
    ///
    /// # Arguments
    ///
    /// * `actions_consumer` - A boxed `ActionsConsumer` instance
    /// * `actions_processor` - A boxed `ActionsProcessor` instance
    /// * `actions_loader` - A boxed `ActionsLoader` instance
    ///
    /// # Returns
    ///
    /// A new `Orchestrator` instance.
    pub fn new(
        actions_consumer: Box<ActionsConsumer>,
        actions_processor: Box<ActionsProcessor>,
        actions_loader: Box<ActionsLoader>,
    ) -> Self {
        Self {
            actions_consumer,
            actions_processor,
            actions_loader,
        }
    }

    /// Runs the orchestrator, initiating the action processing pipeline.
    ///
    /// This method is the main entry point for starting the continuous flow of
    /// action consumption, processing, and loading.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `OrchestratorError` if an error occurs
    /// during the orchestration process.
    pub async fn run(self) -> Result<(), OrchestratorError> {
        let (tx, mut rx) = mpsc::channel(1000); 
        
        let consumer_tx = tx.clone();
        let consumer = self.actions_consumer;
        tokio::spawn(async move {
            if let Err(e) = consumer.run(consumer_tx).await {
                eprintln!("Consumer error: {:?}", e);
            }
        });
        
        while let Some(message) = rx.recv().await {
            match message {
                StreamMessage::BlockData(block_data) => {
                    
                }
                StreamMessage::UndoSignal(undo_signal) => {
                    println!("UndoSignal: {:?}", undo_signal);
                }
                StreamMessage::Error(error) => {
                    println!("Error: {:?}", error);
                }
                StreamMessage::StreamEnd => {
                    println!("StreamEnd");
                }
            }   
        }
        Ok(())
    }
}