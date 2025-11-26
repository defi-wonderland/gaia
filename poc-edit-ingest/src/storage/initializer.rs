// Database and cache initialization functions
use anyhow::{Context, Result};
use indexer::{KgIndexer, PostgresCache, PostgresStorage, PropertiesCache};
use tracing::{info, warn};

use crate::config::{get_database_url, get_neo4j_uri, get_property_tuples, is_neo4j_enabled};
use crate::neo4j_writer::Neo4jWriter;

/// Initialize the KgIndexer with PostgreSQL storage, cache, and properties
pub async fn initialize_indexer() -> Result<KgIndexer> {
    let database_url = get_database_url();
    let storage = PostgresStorage::new(&database_url)
        .await
        .context("Failed to initialize PostgreSQL storage")?;

    let cache = PostgresCache::new()
        .await
        .context("Failed to initialize PostgreSQL cache")?;

    let properties = get_property_tuples();
    let properties_cache = PropertiesCache::from_tuples(&properties)
        .context("Failed to initialize properties cache")?;

    let kg_indexer = KgIndexer::new(storage, cache, properties_cache);
    info!("KgIndexer initialized successfully");

    Ok(kg_indexer)
}

/// Initialize Neo4j writer if enabled and configured
pub async fn initialize_neo4j() -> Option<Neo4jWriter> {
    if !is_neo4j_enabled() {
        info!("Neo4j disabled (ENABLE_NEO4J=false)");
        return None;
    }

    match get_neo4j_uri() {
        Some(uri) => match Neo4jWriter::new(&uri).await {
            Ok(writer) => {
                info!("Neo4j writer initialized successfully");
                Some(writer)
            }
            Err(e) => {
                warn!("Failed to initialize Neo4j writer: {:?}", e);
                warn!("Continuing with PostgreSQL only");
                None
            }
        },
        None => {
            warn!("NEO4J_URI not set, continuing with PostgreSQL only");
            None
        }
    }
}
