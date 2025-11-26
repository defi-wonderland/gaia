// Neo4j connection setup
use anyhow::{Context, Result};
use neo4rs::Graph;

use crate::config::get_neo4j_uri;

/// Connect to Neo4j and return a Graph instance
pub async fn connect() -> Result<Graph> {
    let neo4j_uri = get_neo4j_uri()?;
    
    let graph = Graph::new(&neo4j_uri, "", "")
        .context("Failed to connect to Neo4j")?;
    
    Ok(graph)
}
