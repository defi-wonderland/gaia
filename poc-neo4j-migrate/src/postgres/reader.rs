// PostgreSQL data reading functions
use anyhow::{Context, Result};
use bigdecimal::BigDecimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::{Entity, Property, Relation, Value};

/// Read all entities from PostgreSQL
pub async fn read_entities(pool: &PgPool) -> Result<Vec<Entity>> {
    let rows = sqlx::query(
        "SELECT id, created_at, created_at_block, updated_at, updated_at_block FROM entities ORDER BY id"
    )
    .fetch_all(pool)
    .await
    .context("Failed to read entities")?;

    Ok(rows
        .iter()
        .map(|row| Entity {
            id: row.get::<Uuid, _>("id").to_string(),
            created_at: row.get("created_at"),
            created_at_block: row.get("created_at_block"),
            updated_at: row.get("updated_at"),
            updated_at_block: row.get("updated_at_block"),
        })
        .collect())
}

/// Read all relations from PostgreSQL
pub async fn read_relations(pool: &PgPool) -> Result<Vec<Relation>> {
    let rows = sqlx::query(
        "SELECT id, entity_id, type_id, from_entity_id, from_space_id, from_version_id, 
                to_entity_id, to_space_id, to_version_id, position, space_id, verified 
         FROM relations ORDER BY id"
    )
    .fetch_all(pool)
    .await
    .context("Failed to read relations")?;

    Ok(rows
        .iter()
        .map(|row| Relation {
            id: row.get::<Uuid, _>("id").to_string(),
            entity_id: row.get::<Uuid, _>("entity_id").to_string(),
            type_id: row.get::<Uuid, _>("type_id").to_string(),
            from_entity_id: row.get::<Uuid, _>("from_entity_id").to_string(),
            from_space_id: row
                .get::<Option<Uuid>, _>("from_space_id")
                .map(|u| u.to_string()),
            from_version_id: row
                .get::<Option<Uuid>, _>("from_version_id")
                .map(|u| u.to_string()),
            to_entity_id: row.get::<Uuid, _>("to_entity_id").to_string(),
            to_space_id: row
                .get::<Option<Uuid>, _>("to_space_id")
                .map(|u| u.to_string()),
            to_version_id: row
                .get::<Option<Uuid>, _>("to_version_id")
                .map(|u| u.to_string()),
            position: row.get("position"),
            space_id: row.get::<Uuid, _>("space_id").to_string(),
            verified: row.get("verified"),
        })
        .collect())
}

/// Read all values from PostgreSQL
pub async fn read_values(pool: &PgPool) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, property_id, entity_id, space_id, string, boolean, number, 
                point, time, language, unit 
         FROM values ORDER BY entity_id, property_id"
    )
    .fetch_all(pool)
    .await
    .context("Failed to read values")?;

    Ok(rows
        .iter()
        .map(|row| Value {
            id: row.get("id"),
            property_id: row.get::<Uuid, _>("property_id").to_string(),
            entity_id: row.get::<Uuid, _>("entity_id").to_string(),
            space_id: row.get::<Uuid, _>("space_id").to_string(),
            string: row.get("string"),
            boolean: row.get("boolean"),
            number: row
                .get::<Option<BigDecimal>, _>("number")
                .map(|n| n.to_string()),
            point: row.get("point"),
            time: row.get("time"),
            language: row.get("language"),
            unit: row.get("unit"),
        })
        .collect())
}

/// Read all properties from PostgreSQL
pub async fn read_properties(pool: &PgPool) -> Result<Vec<Property>> {
    let rows = sqlx::query("SELECT id, type::text as type FROM properties ORDER BY id")
        .fetch_all(pool)
        .await
        .context("Failed to read properties")?;

    Ok(rows
        .iter()
        .map(|row| Property {
            id: row.get::<Uuid, _>("id").to_string(),
            data_type: row.get("type"),
        })
        .collect())
}
