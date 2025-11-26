// Configuration constants and environment helpers
use anyhow::{Context, Result};

// Batch processing configuration
pub const ENTITY_BATCH_SIZE: usize = 1000;
pub const PROPERTY_BATCH_SIZE: usize = 1000;

// Concurrency limits
pub const RELATION_CONCURRENCY_LIMIT: usize = 50;
pub const ENTITY_PROPERTIES_CONCURRENCY_LIMIT: usize = 50;
pub const RELATION_PROPERTIES_CONCURRENCY_LIMIT: usize = 50;

// Progress reporting intervals
pub const ENTITY_BATCH_REPORT_INTERVAL: usize = 10;
pub const RELATION_REPORT_INTERVAL: usize = 1000;
pub const ENTITY_PROPERTIES_REPORT_INTERVAL: usize = 500;
pub const RELATION_PROPERTIES_REPORT_INTERVAL: usize = 500;

// PostgreSQL connection pool configuration
pub const PG_MAX_CONNECTIONS: u32 = 5;

/// Get DATABASE_URL from environment
pub fn get_database_url() -> Result<String> {
    std::env::var("DATABASE_URL").context("DATABASE_URL must be set")
}

/// Get NEO4J_URI from environment
pub fn get_neo4j_uri() -> Result<String> {
    std::env::var("NEO4J_URI").context("NEO4J_URI must be set")
}

