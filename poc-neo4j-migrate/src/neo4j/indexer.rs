// Neo4j index and constraint creation
use anyhow::Result;
use neo4rs::{Graph, Query};
use tracing::{info, warn};

/// Create all indexes and constraints for optimal query performance
pub async fn create_indexes_and_constraints(graph: &Graph) -> Result<()> {
    // Create unique constraint on Entity.id (also creates an index)
    info!("Creating unique constraint on Entity.id...");
    let constraint_query = Query::new(
        "CREATE CONSTRAINT entity_id_unique IF NOT EXISTS FOR (e:Entity) REQUIRE e.id IS UNIQUE"
            .to_string(),
    );
    match graph.run(constraint_query).await {
        Ok(_) => info!("✓ Created unique constraint on Entity.id"),
        Err(e) => warn!(
            "Failed to create constraint on Entity.id (may already exist): {}",
            e
        ),
    }

    // Create index on relationship type_id property
    info!("Creating index on RELATES_TO.type_id...");
    let type_id_index = Query::new(
        "CREATE INDEX rel_type_id IF NOT EXISTS FOR ()-[r:RELATES_TO]-() ON (r.type_id)"
            .to_string(),
    );
    match graph.run(type_id_index).await {
        Ok(_) => info!("✓ Created index on RELATES_TO.type_id"),
        Err(e) => warn!("Failed to create index on type_id: {}", e),
    }

    // Create index on relationship entity_id property
    info!("Creating index on RELATES_TO.entity_id...");
    let entity_id_index = Query::new(
        "CREATE INDEX rel_entity_id IF NOT EXISTS FOR ()-[r:RELATES_TO]-() ON (r.entity_id)"
            .to_string(),
    );
    match graph.run(entity_id_index).await {
        Ok(_) => info!("✓ Created index on RELATES_TO.entity_id"),
        Err(e) => warn!("Failed to create index on entity_id: {}", e),
    }

    // Create index on relationship space_id property
    info!("Creating index on RELATES_TO.space_id...");
    let space_id_index = Query::new(
        "CREATE INDEX rel_space_id IF NOT EXISTS FOR ()-[r:RELATES_TO]-() ON (r.space_id)"
            .to_string(),
    );
    match graph.run(space_id_index).await {
        Ok(_) => info!("✓ Created index on RELATES_TO.space_id"),
        Err(e) => warn!("Failed to create index on space_id: {}", e),
    }

    Ok(())
}

/// Verify that indexes were created successfully
pub async fn verify_indexes(graph: &Graph) -> Result<()> {
    info!("\nVerifying indexes...");
    let verify_query = Query::new("SHOW INDEXES".to_string());
    match graph.execute(verify_query).await {
        Ok(mut result) => {
            let mut count = 0;
            while result.next().await?.is_some() {
                count += 1;
            }
            info!("✓ Total indexes in database: {}", count);
        }
        Err(e) => warn!("Could not verify indexes: {}", e),
    }

    Ok(())
}
