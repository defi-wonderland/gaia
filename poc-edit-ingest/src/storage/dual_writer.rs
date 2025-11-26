// Dual writer for PostgreSQL + Neo4j
use anyhow::Result;
use indexer::KgIndexer;
use std::time::Instant;
use stream::{pb::sf::substreams::rpc::v2::BlockScopedData, PreprocessedSink};
use indexer::KgData;
use tracing::{info, warn};
use uuid::Uuid;
use wire::pb::grc20::Edit;


use crate::neo4j_writer::Neo4jWriter;

/// Wraps KgIndexer and optional Neo4jWriter for dual-database writes
pub struct DualWriter {
    kg_indexer: KgIndexer,
    neo4j_writer: Option<Neo4jWriter>,
}

impl DualWriter {
    pub fn new(kg_indexer: KgIndexer, neo4j_writer: Option<Neo4jWriter>) -> Self {
        Self {
            kg_indexer,
            neo4j_writer,
        }
    }

    /// Write edit to both PostgreSQL and Neo4j (if enabled)
    /// PostgreSQL is primary - if it fails, the entire operation fails
    /// Neo4j is secondary - if it fails, we log a warning but continue
    pub async fn write_edit(
        &self,
        edit: &Edit,
        space_id: &Uuid,
        content_uri: &str,
        block_scoped_data: &BlockScopedData,
        kg_data: KgData,
    ) -> Result<()> {
        // Write to PostgreSQL (primary)
        let pg_start = Instant::now();
        self.kg_indexer
            .process_block_scoped_data(block_scoped_data, kg_data.clone())
            .await?;
        let pg_duration = pg_start.elapsed();

        info!(
            "✓ PostgreSQL write completed in {:.3}s",
            pg_duration.as_secs_f64()
        );

        // If PostgreSQL succeeded and Neo4j is enabled, write to Neo4j (secondary)
        if let Some(ref writer) = self.neo4j_writer {
            info!("Writing to Neo4j...");
            let neo4j_start = Instant::now();

            let block = &kg_data.block;
            let timestamp = block.timestamp.clone();
            if let Err(e) = write_edit_to_neo4j(writer, edit, space_id, &timestamp).await {
                warn!("Failed to write to Neo4j: {:?}", e);
                warn!("PostgreSQL data is safe, continuing...");
            } else {
                let neo4j_duration = neo4j_start.elapsed();
                info!(
                    "✓ Neo4j write completed in {:.3}s",
                    neo4j_duration.as_secs_f64()
                );
                info!(
                    "Performance comparison - PostgreSQL: {:.3}s, Neo4j: {:.3}s",
                    pg_duration.as_secs_f64(),
                    neo4j_duration.as_secs_f64()
                );
            }
        }

        Ok(())
    }
}

/// Helper function to write edit data to Neo4j
async fn write_edit_to_neo4j(
    writer: &Neo4jWriter,
    edit: &Edit,
    space_id: &Uuid,
    timestamp: &str,
) -> Result<()> {
    use indexer::models::{
        entities::EntitiesModel, relations::RelationsModel, values::ValuesModel,
    };
    use std::sync::Arc;
    use stream::utils::BlockMetadata;

    use crate::config::get_property_tuples;

    let properties = get_property_tuples();
    let properties_cache = Arc::new(
        indexer::cache::properties_cache::PropertiesCache::from_tuples(&properties)?,
    );

    let block = BlockMetadata {
        cursor: "".to_string(),
        block_number: 0,
        timestamp: timestamp.to_string(),
    };

    // Extract entities
    let entities = EntitiesModel::map_edit_to_entities(edit, &block);
    writer.write_entities(&entities).await?;

    // Extract relations
    let (set_relations, _update_relations, _unset_relations, _delete_relations) =
        RelationsModel::map_edit_to_relations(edit, space_id);
    writer.write_relations(&set_relations).await?;

    let relation_ids = set_relations
        .iter()
        .map(|r| r.entity_id)
        .collect::<Vec<Uuid>>();

    // Extract values
    let (values, _deleted_values) =
        ValuesModel::map_edit_to_values(edit, space_id, &properties_cache).await;
    writer.write_values(&values, &relation_ids).await?;

    Ok(())
}
