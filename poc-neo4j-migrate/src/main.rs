use anyhow::Result;
use poc_neo4j_migrate::{migration::MigrationExecutor, neo4j, postgres};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    info!("Starting PostgreSQL to Neo4j migration");

    // Connect to PostgreSQL
    info!("Connecting to PostgreSQL...");
    let pg_pool = postgres::connect().await?;
    info!("✓ Connected to PostgreSQL");

    // Connect to Neo4j
    info!("Connecting to Neo4j...");
    let neo4j_graph = neo4j::connect().await?;
    info!("✓ Connected to Neo4j");

    // Create and execute migration
    let executor = MigrationExecutor::new(pg_pool, neo4j_graph);
    executor.execute().await?;

    Ok(())
}
