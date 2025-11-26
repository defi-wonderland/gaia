// Neo4j data writing functions
use anyhow::{Context, Result};
use neo4rs::{Graph, Query};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info, warn};

use crate::config::{
    ENTITY_BATCH_REPORT_INTERVAL, ENTITY_BATCH_SIZE, ENTITY_PROPERTIES_CONCURRENCY_LIMIT,
    ENTITY_PROPERTIES_REPORT_INTERVAL, RELATION_CONCURRENCY_LIMIT, RELATION_PROPERTIES_CONCURRENCY_LIMIT,
    RELATION_PROPERTIES_REPORT_INTERVAL, RELATION_REPORT_INTERVAL,
};
use crate::models::{Entity, Relation, Value};

/// Clear all existing data from Neo4j
pub async fn clear_data(graph: &Graph) -> Result<()> {
    let query = Query::new("MATCH (n) DETACH DELETE n".to_string());
    graph
        .run(query)
        .await
        .context("Failed to clear Neo4j data")?;
    Ok(())
}

/// Create entity nodes in Neo4j in batches
pub async fn write_entity_nodes(graph: &Graph, entities: &[Entity]) -> Result<()> {
    let batch_size = ENTITY_BATCH_SIZE;
    let total_batches = (entities.len() + batch_size - 1) / batch_size;

    for (batch_idx, chunk) in entities.chunks(batch_size).enumerate() {
        let query_str =
            String::from("UNWIND $entities AS entity CREATE (n:Entity) SET n = entity");

        let entities_data: Vec<HashMap<&str, String>> = chunk
            .iter()
            .map(|e| {
                let mut map = HashMap::new();
                map.insert("id", e.id.clone());
                map.insert("created_at", e.created_at.clone());
                map.insert("created_at_block", e.created_at_block.clone());
                map.insert("updated_at", e.updated_at.clone());
                map.insert("updated_at_block", e.updated_at_block.clone());
                map
            })
            .collect();

        let query = Query::new(query_str).param("entities", entities_data);

        graph
            .run(query)
            .await
            .with_context(|| {
                format!(
                    "Failed to create entity nodes batch {}/{}",
                    batch_idx + 1,
                    total_batches
                )
            })?;

        if (batch_idx + 1) % ENTITY_BATCH_REPORT_INTERVAL == 0 || batch_idx + 1 == total_batches {
            info!("  Progress: {}/{} batches", batch_idx + 1, total_batches);
        }
    }

    // Create index on entity id for faster lookups
    let index_query = Query::new(
        "CREATE INDEX entity_id_index IF NOT EXISTS FOR (n:Entity) ON (n.id)".to_string(),
    );
    graph.run(index_query).await.ok(); // Ignore if index already exists

    Ok(())
}

/// Create relationships in Neo4j with concurrent processing
pub async fn write_relationships(graph: &Graph, relations: &[Relation]) -> Result<()> {
    let concurrency_limit = RELATION_CONCURRENCY_LIMIT;
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let graph = Arc::new(graph.clone());
    let skipped = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let total = relations.len();
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut tasks = Vec::new();

    for relation in relations {
        let relation = relation.clone();
        let graph = graph.clone();
        let semaphore = semaphore.clone();
        let skipped = skipped.clone();
        let processed = processed.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let query_str = format!(
                "MATCH (from:Entity {{id: $from_id}}) \
                 MATCH (to:Entity {{id: $to_id}}) \
                 CREATE (from)-[r:RELATES_TO]->(to) \
                 SET r.id = $id, \
                     r.entity_id = $entity_id, \
                     r.type_id = $type_id, \
                     r.space_id = $space_id, \
                     r.from_space_id = $from_space_id, \
                     r.from_version_id = $from_version_id, \
                     r.to_space_id = $to_space_id, \
                     r.to_version_id = $to_version_id, \
                     r.position = $position, \
                     r.verified = $verified"
            );

            let mut query = Query::new(query_str)
                .param("from_id", relation.from_entity_id.clone())
                .param("to_id", relation.to_entity_id.clone())
                .param("id", relation.id.clone())
                .param("entity_id", relation.entity_id.clone())
                .param("type_id", relation.type_id.clone())
                .param("space_id", relation.space_id.clone());

            query = query.param("from_space_id", relation.from_space_id.clone());
            query = query.param("from_version_id", relation.from_version_id.clone());
            query = query.param("to_space_id", relation.to_space_id.clone());
            query = query.param("to_version_id", relation.to_version_id.clone());
            query = query.param("position", relation.position.clone());
            query = query.param("verified", relation.verified);

            if let Err(e) = graph.run(query).await {
                warn!("Failed to create relationship {}: {}", relation.id, e);
                skipped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }

            let current = processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if current % RELATION_REPORT_INTERVAL == 0 || current == total {
                info!("  Progress: {}/{} relationships", current, total);
            }
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        task.await.context("Task panicked")?;
    }

    let skipped_count = skipped.load(std::sync::atomic::Ordering::Relaxed);
    if skipped_count > 0 {
        warn!("Skipped {} relationships due to errors", skipped_count);
    }

    Ok(())
}

/// Add value properties to entity nodes
pub async fn write_entity_properties(
    graph: &Graph,
    values_by_entity: &HashMap<String, Vec<Value>>,
) -> Result<()> {
    let concurrency_limit = ENTITY_PROPERTIES_CONCURRENCY_LIMIT;
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let graph = Arc::new(graph.clone());
    let skipped = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let total = values_by_entity.len();
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut tasks = Vec::new();

    for (entity_id, values) in values_by_entity {
        let entity_id = entity_id.clone();
        let values = values.clone();
        let graph = graph.clone();
        let semaphore = semaphore.clone();
        let skipped = skipped.clone();
        let processed = processed.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let mut set_clauses = Vec::new();

            for (idx, value) in values.iter().enumerate() {
                let prop_prefix = format!("prop_{}", value.property_id.replace("-", "_"));

                if value.string.is_some() {
                    set_clauses.push(format!("n.{}_string = $s{}", prop_prefix, idx));
                }
                if value.number.is_some() {
                    set_clauses.push(format!("n.{}_number = $n{}", prop_prefix, idx));
                }
                if value.boolean.is_some() {
                    set_clauses.push(format!("n.{}_boolean = $b{}", prop_prefix, idx));
                }
                if value.time.is_some() {
                    set_clauses.push(format!("n.{}_time = $t{}", prop_prefix, idx));
                }
                if value.point.is_some() {
                    set_clauses.push(format!("n.{}_point = $p{}", prop_prefix, idx));
                }
                if value.language.is_some() {
                    set_clauses.push(format!("n.{}_language = $l{}", prop_prefix, idx));
                }
                if value.unit.is_some() {
                    set_clauses.push(format!("n.{}_unit = $u{}", prop_prefix, idx));
                }
            }

            if !set_clauses.is_empty() {
                let query_str = format!(
                    "MATCH (n:Entity {{id: $entity_id}}) SET {}",
                    set_clauses.join(", ")
                );

                let mut query = Query::new(query_str).param("entity_id", entity_id.clone());

                for (idx, value) in values.iter().enumerate() {
                    if let Some(ref s) = value.string {
                        query = query.param(&format!("s{}", idx), s.clone());
                    }
                    if let Some(ref n) = value.number {
                        query = query.param(&format!("n{}", idx), n.clone());
                    }
                    if let Some(b) = value.boolean {
                        query = query.param(&format!("b{}", idx), b);
                    }
                    if let Some(ref t) = value.time {
                        query = query.param(&format!("t{}", idx), t.clone());
                    }
                    if let Some(ref p) = value.point {
                        query = query.param(&format!("p{}", idx), p.clone());
                    }
                    if let Some(ref l) = value.language {
                        query = query.param(&format!("l{}", idx), l.clone());
                    }
                    if let Some(ref u) = value.unit {
                        query = query.param(&format!("u{}", idx), u.clone());
                    }
                }

                if let Err(e) = graph.run(query).await {
                    warn!("Failed to add properties for entity {}: {}", entity_id, e);
                    skipped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }

            let current = processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if current % ENTITY_PROPERTIES_REPORT_INTERVAL == 0 || current == total {
                info!("  Progress: {}/{} entities", current, total);
            }
        });

        tasks.push(task);
    }

    for task in tasks {
        task.await.context("Task panicked")?;
    }

    let skipped_count = skipped.load(std::sync::atomic::Ordering::Relaxed);
    if skipped_count > 0 {
        warn!("Skipped {} entities when adding properties", skipped_count);
    }

    Ok(())
}

/// Add value properties to relationships
pub async fn write_relation_properties(
    graph: &Graph,
    values_by_relation: &HashMap<String, Vec<Value>>,
    relations: &[Relation],
) -> Result<()> {
    // Create a mapping from entity_id to relation id for quick lookup
    let entity_to_relation: Arc<HashMap<String, String>> = Arc::new(
        relations
            .iter()
            .map(|r| (r.entity_id.clone(), r.id.clone()))
            .collect(),
    );

    let concurrency_limit = RELATION_PROPERTIES_CONCURRENCY_LIMIT;
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let graph = Arc::new(graph.clone());
    let skipped = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let total = values_by_relation.len();
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut tasks = Vec::new();

    for (entity_id, values) in values_by_relation {
        let entity_id = entity_id.clone();
        let values = values.clone();
        let graph = graph.clone();
        let semaphore = semaphore.clone();
        let skipped = skipped.clone();
        let processed = processed.clone();
        let entity_to_relation = entity_to_relation.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let relation_id = match entity_to_relation.get(&entity_id) {
                Some(id) => id.clone(),
                None => {
                    warn!("No relation found for entity_id {}", entity_id);
                    skipped.fetch_add(values.len(), std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            let mut set_clauses = Vec::new();

            for (idx, value) in values.iter().enumerate() {
                let prop_prefix = format!("prop_{}", value.property_id.replace("-", "_"));

                if value.string.is_some() {
                    set_clauses.push(format!("r.{}_string = $s{}", prop_prefix, idx));
                }
                if value.number.is_some() {
                    set_clauses.push(format!("r.{}_number = $n{}", prop_prefix, idx));
                }
                if value.boolean.is_some() {
                    set_clauses.push(format!("r.{}_boolean = $b{}", prop_prefix, idx));
                }
                if value.time.is_some() {
                    set_clauses.push(format!("r.{}_time = $t{}", prop_prefix, idx));
                }
                if value.point.is_some() {
                    set_clauses.push(format!("r.{}_point = $p{}", prop_prefix, idx));
                }
                if value.language.is_some() {
                    set_clauses.push(format!("r.{}_language = $l{}", prop_prefix, idx));
                }
                if value.unit.is_some() {
                    set_clauses.push(format!("r.{}_unit = $u{}", prop_prefix, idx));
                }
            }

            if !set_clauses.is_empty() {
                let query_str = format!(
                    "MATCH ()-[r:RELATES_TO {{id: $relation_id}}]->() SET {}",
                    set_clauses.join(", ")
                );

                let mut query = Query::new(query_str).param("relation_id", relation_id.clone());

                for (idx, value) in values.iter().enumerate() {
                    if let Some(ref s) = value.string {
                        query = query.param(&format!("s{}", idx), s.clone());
                    }
                    if let Some(ref n) = value.number {
                        query = query.param(&format!("n{}", idx), n.clone());
                    }
                    if let Some(b) = value.boolean {
                        query = query.param(&format!("b{}", idx), b);
                    }
                    if let Some(ref t) = value.time {
                        query = query.param(&format!("t{}", idx), t.clone());
                    }
                    if let Some(ref p) = value.point {
                        query = query.param(&format!("p{}", idx), p.clone());
                    }
                    if let Some(ref l) = value.language {
                        query = query.param(&format!("l{}", idx), l.clone());
                    }
                    if let Some(ref u) = value.unit {
                        query = query.param(&format!("u{}", idx), u.clone());
                    }
                }

                if let Err(e) = graph.run(query).await {
                    warn!("Failed to add properties for relation {}: {}", relation_id, e);
                    skipped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }

            let current = processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if current % RELATION_PROPERTIES_REPORT_INTERVAL == 0 || current == total {
                info!("  Progress: {}/{} relations", current, total);
            }
        });

        tasks.push(task);
    }

    for task in tasks {
        task.await.context("Task panicked")?;
    }

    let skipped_count = skipped.load(std::sync::atomic::Ordering::Relaxed);
    if skipped_count > 0 {
        warn!("Skipped {} relations when adding properties", skipped_count);
    }

    Ok(())
}
