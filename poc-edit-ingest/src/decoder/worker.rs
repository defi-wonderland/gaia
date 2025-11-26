// Background worker that processes edits
use chrono::{DateTime, Utc};
use indexer::cache::PreprocessedEdit;
use stream::{
    pb::sf::substreams::{rpc::v2::BlockScopedData, v1::Clock},
    utils::BlockMetadata,
};
use indexer::KgData;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::models::EditMessage;
use crate::storage::{initialize_indexer, initialize_neo4j, DualWriter};

/// Background task that processes incoming edits
pub async fn decoder_task(mut edit_receiver: broadcast::Receiver<EditMessage>) {
    info!("Decoder task started, waiting for edits...");

    // Initialize database connection and caches
    let kg_indexer = match initialize_indexer().await {
        Ok(indexer) => indexer,
        Err(e) => {
            error!("Failed to initialize indexer: {:?}", e);
            return;
        }
    };

    // Initialize Neo4j writer if enabled
    let neo4j_writer = initialize_neo4j().await;

    // Create dual writer
    let dual_writer = DualWriter::new(kg_indexer, neo4j_writer);

    // Process incoming edits
    while let Ok(message) = edit_receiver.recv().await {
        let preprocessed_edit = PreprocessedEdit {
            space_id: message.space_id,
            edit: Some(message.edit.clone()),
            is_errored: false,
            cid: message.content_uri.clone(),
        };

        let now: DateTime<Utc> = Utc::now();
        let rfc3339_string = now.to_rfc3339();

        let kg_data = KgData {
            block: BlockMetadata {
                cursor: "".to_string(),
                block_number: 0,
                timestamp: rfc3339_string.clone(),
            },
            edits: vec![preprocessed_edit],
            spaces: vec![],
            added_editors: vec![],
            added_members: vec![],
            removed_editors: vec![],
            removed_members: vec![],
            added_subspaces: vec![],
            removed_subspaces: vec![],
            executed_proposals: vec![],
            created_proposals: vec![],
        };

        let block_scoped_data = BlockScopedData {
            output: None,
            clock: Some(Clock {
                id: "".to_string(),
                number: 0,
                timestamp: None,
            }),
            cursor: "".to_string(),
            final_block_height: 0,
            debug_map_outputs: vec![],
            debug_store_outputs: vec![],
        };

        // Write to both databases using dual writer
        if let Err(e) = dual_writer
            .write_edit(
                &message.edit,
                &message.space_id,
                &message.content_uri,
                &block_scoped_data,
                kg_data,
            )
            .await
        {
            error!("Failed to process edit {}: {:?}", message.content_uri, e);
        }
    }
}
