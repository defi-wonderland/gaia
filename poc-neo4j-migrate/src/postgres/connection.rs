// PostgreSQL connection setup
use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::{get_database_url, PG_MAX_CONNECTIONS};

/// Connect to PostgreSQL and return a connection pool
pub async fn connect() -> Result<PgPool> {
    let database_url = get_database_url()?;
    
    let pool = PgPoolOptions::new()
        .max_connections(PG_MAX_CONNECTIONS)
        .connect(&database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;
    
    Ok(pool)
}
