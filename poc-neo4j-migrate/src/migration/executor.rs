// Migration executor - orchestrates the migration flow
use anyhow::Result;
use neo4rs::Graph;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Instant;
use tracing::info;

use crate::models::{Relation, Value};
use crate::{neo4j, postgres};

/// Migration executor that coordinates the migration process
pub struct MigrationExecutor {
    pg_pool: PgPool,
    neo4j_graph: Graph,
}

impl MigrationExecutor {
    /// Create a new migration executor
    pub fn new(pg_pool: PgPool, neo4j_graph: Graph) -> Self {
        Self {
            pg_pool,
            neo4j_graph,
        }
    }

    /// Execute the full migration process
    pub async fn execute(&self) -> Result<()> {
        let start_time = Instant::now();

        // Step 1: Read data from PostgreSQL
        info!("\n=== Reading data from PostgreSQL ===");
        let entities = postgres::read_entities(&self.pg_pool).await?;
        info!("✓ Read {} entities", entities.len());

        let relations = postgres::read_relations(&self.pg_pool).await?;
        info!("✓ Read {} relations", relations.len());

        let values = postgres::read_values(&self.pg_pool).await?;
        info!("✓ Read {} values", values.len());

        let properties = postgres::read_properties(&self.pg_pool).await?;
        info!("✓ Read {} properties", properties.len());

        // Step 2: Organize data - separate entity values from relation values
        let (values_by_entity, values_by_relation) =
            self.organize_values(&values, &relations);

        // Step 3: Write data to Neo4j
        info!("\n=== Writing data to Neo4j ===");

        // Clear existing data
        info!("Clearing existing Neo4j data...");
        neo4j::clear_data(&self.neo4j_graph).await?;
        info!("✓ Cleared existing data");

        // Create entity nodes
        info!("Creating entity nodes...");
        neo4j::write_entity_nodes(&self.neo4j_graph, &entities).await?;
        info!("✓ Created {} entity nodes", entities.len());

        // Create relationships
        info!("Creating relationships...");
        neo4j::write_relationships(&self.neo4j_graph, &relations).await?;
        info!("✓ Created {} relationships", relations.len());

        // Add value properties to entity nodes
        info!("Adding value properties to entities...");
        neo4j::write_entity_properties(&self.neo4j_graph, &values_by_entity).await?;
        info!(
            "✓ Added properties for {} entities",
            values_by_entity.len()
        );

        // Add value properties to relationships
        info!("Adding value properties to relationships...");
        neo4j::write_relation_properties(
            &self.neo4j_graph,
            &values_by_relation,
            &relations,
        )
        .await?;
        info!(
            "✓ Added properties for {} relationships",
            values_by_relation.len()
        );

        // Step 4: Create indexes and constraints
        info!("\n=== Creating Indexes and Constraints ===");
        neo4j::create_indexes_and_constraints(&self.neo4j_graph).await?;
        neo4j::verify_indexes(&self.neo4j_graph).await?;

        // Step 5: Report statistics
        let elapsed = start_time.elapsed();
        let total_entity_values: usize = values_by_entity.values().map(|v| v.len()).sum();
        let total_relation_values: usize = values_by_relation.values().map(|v| v.len()).sum();

        info!("\n=== Migration Complete ===");
        info!("Total time: {:.2}s", elapsed.as_secs_f64());
        info!("Entities: {}", entities.len());
        info!("Relations: {}", relations.len());
        info!("Entity Values: {}", total_entity_values);
        info!("Relation Values: {}", total_relation_values);
        info!("Properties: {}", properties.len());
        info!("\nIndexes and constraints have been created for optimal performance.");

        Ok(())
    }

    /// Organize values into entity values and relation values
    fn organize_values(
        &self,
        values: &[Value],
        relations: &[Relation],
    ) -> (HashMap<String, Vec<Value>>, HashMap<String, Vec<Value>>) {
        // Collect all relation entity_ids to identify which values belong to relations
        let relation_entity_ids: std::collections::HashSet<String> = relations
            .iter()
            .map(|r| r.entity_id.clone())
            .collect();

        // Separate values into entity values and relation values
        let mut values_by_entity: HashMap<String, Vec<Value>> = HashMap::new();
        let mut values_by_relation: HashMap<String, Vec<Value>> = HashMap::new();

        for value in values {
            if relation_entity_ids.contains(&value.entity_id) {
                values_by_relation
                    .entry(value.entity_id.clone())
                    .or_insert_with(Vec::new)
                    .push(value.clone());
            } else {
                values_by_entity
                    .entry(value.entity_id.clone())
                    .or_insert_with(Vec::new)
                    .push(value.clone());
            }
        }

        (values_by_entity, values_by_relation)
    }
}
