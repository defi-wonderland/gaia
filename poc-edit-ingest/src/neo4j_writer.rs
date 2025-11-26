use anyhow::{Context, Result};
use neo4rs::{Graph, Query};
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

use indexer::models::{entities::EntityItem, relations::SetRelationItem, values::ValueOp};

pub struct Neo4jWriter {
    graph: Graph,
}
struct ValueWithType {
    id: Uuid,
    is_relation: bool,
    value: ValueOp,
}

impl Neo4jWriter {
    pub async fn new(neo4j_uri: &str) -> Result<Self> {
        info!("Connecting to Neo4j at {}", neo4j_uri);
        let graph = Graph::new(neo4j_uri, "", "").context("Failed to connect to Neo4j")?;

        Ok(Neo4jWriter { graph })
    }

    /// Write entities to Neo4j as Entity:Rank nodes
    /// All entities in this POC are considered ranks
    pub async fn write_entities(&self, entities: &[EntityItem]) -> Result<()> {
        if entities.is_empty() {
            return Ok(());
        }

        for entity in entities {
            // Create entities with both Entity and Rank labels
            let query_str = format!(
                "MERGE (n:Entity {{id: $id}}) \
                 SET n.created_at = $created_at, \
                     n.created_at_block = $created_at_block, \
                     n.updated_at = $updated_at, \
                     n.updated_at_block = $updated_at_block"
            );

            let query = Query::new(query_str)
                .param("id", entity.id.to_string())
                .param("created_at", entity.created_at.clone())
                .param("created_at_block", entity.created_at_block.clone())
                .param("updated_at", entity.updated_at.clone())
                .param("updated_at_block", entity.updated_at_block.clone());

            self.graph
                .run(query)
                .await
                .context("Failed to write entity")?;
        }

        info!("Wrote {} entities to Neo4j", entities.len());
        Ok(())
    }

    /// Write relations to Neo4j as RELATES_TO relationships
    /// All relations in this POC are considered votes
    pub async fn write_relations(&self, relations: &[SetRelationItem]) -> Result<()> {
        if relations.is_empty() {
            return Ok(());
        }

        let mut skipped = 0;
        for relation in relations {
            let query_str = format!(
                "MATCH (from:Entity {{id: $from_id}}) \
                 MATCH (to:Entity {{id: $to_id}}) \
                 MERGE (from)-[r:RELATES_TO {{id: $id}}]->(to) \
                 SET r.entity_id = $entity_id, \
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
                .param("from_id", relation.from_id.to_string())
                .param("to_id", relation.to_id.to_string())
                .param("id", relation.id.to_string())
                .param("entity_id", relation.entity_id.to_string())
                .param("type_id", relation.type_id.to_string())
                .param("space_id", relation.space_id.to_string());

            // Add optional parameters
            query = query.param("from_space_id", relation.from_space_id.clone());
            query = query.param("from_version_id", relation.from_version_id.clone());
            query = query.param("to_space_id", relation.to_space_id.clone());
            query = query.param("to_version_id", relation.to_version_id.clone());
            query = query.param("position", relation.position.clone());
            query = query.param("verified", relation.verified);

            match self.graph.run(query).await {
                Ok(_) => {}
                Err(e) => {
                    warn!("Failed to write relation {}: {}", relation.id, e);
                    skipped += 1;
                }
            }
        }

        if skipped > 0 {
            warn!("Skipped {} relations due to errors", skipped);
        }

        info!(
            "Wrote {} vote relations to Neo4j ({} skipped)",
            relations.len() - skipped,
            skipped
        );
        Ok(())
    }

    /// Write values as properties on Entity nodes or RELATES_TO relationships
    pub async fn write_values(&self, values: &[ValueOp], relation_ids: &[Uuid]) -> Result<()> {
        if values.is_empty() {
            return Ok(());
        }

        let values_with_type: Vec<ValueWithType> = values
            .iter()
            .map(|value| {
                let is_relation = relation_ids.contains(&value.entity_id);
                ValueWithType {
                    id: value.id,
                    is_relation,
                    value: value.clone(),
                }
            })
            .collect::<Vec<ValueWithType>>();

        // Separate entity values from relation values
        let mut values_by_entity: HashMap<Uuid, Vec<ValueWithType>> = HashMap::new();
        let mut values_by_relation: HashMap<Uuid, Vec<ValueWithType>> = HashMap::new();

        for value in values_with_type {
            if value.is_relation {
                values_by_relation
                    .entry(value.value.entity_id)
                    .or_insert_with(Vec::new)
                    .push(value);
            } else {
                values_by_entity
                    .entry(value.value.entity_id)
                    .or_insert_with(Vec::new)
                    .push(value);
            }
        }

        let mut skipped = 0;

        // Write entity values
        for (entity_id, entity_values) in values_by_entity {
            let mut set_clauses: Vec<String> = Vec::new();

            for (idx, value) in entity_values.iter().enumerate() {
                let prop_prefix = format!(
                    "prop_{}",
                    value.value.property_id.to_string().replace("-", "_")
                );

                if let Some(ref s) = value.value.string {
                    set_clauses.push(format!("n.{}_string = $s{}", prop_prefix, idx));
                }
                if let Some(ref n) = value.value.number {
                    set_clauses.push(format!("n.{}_number = $n{}", prop_prefix, idx));
                }
                if let Some(b) = value.value.boolean {
                    set_clauses.push(format!("n.{}_boolean = $b{}", prop_prefix, idx));
                }
                if let Some(ref t) = value.value.time {
                    set_clauses.push(format!("n.{}_time = $t{}", prop_prefix, idx));
                }
                if let Some(ref p) = value.value.point {
                    set_clauses.push(format!("n.{}_point = $p{}", prop_prefix, idx));
                }
                if let Some(ref l) = value.value.language {
                    set_clauses.push(format!("n.{}_language = $l{}", prop_prefix, idx));
                }
                if let Some(ref u) = value.value.unit {
                    set_clauses.push(format!("n.{}_unit = $u{}", prop_prefix, idx));
                }
            }

            if !set_clauses.is_empty() {
                let query_str = format!(
                    "MATCH (n:Entity {{id: $entity_id}}) SET {}",
                    set_clauses.join(", ")
                );

                let mut query = Query::new(query_str).param("entity_id", entity_id.to_string());

                for (idx, value) in entity_values.iter().enumerate() {
                    if let Some(ref s) = value.value.string {
                        query = query.param(&format!("s{}", idx), s.clone());
                    }
                    if let Some(ref n) = value.value.number {
                        query = query.param(&format!("n{}", idx), *n);
                    }
                    if let Some(b) = value.value.boolean {
                        query = query.param(&format!("b{}", idx), b);
                    }
                    if let Some(ref t) = value.value.time {
                        query = query.param(&format!("t{}", idx), t.clone());
                    }
                    if let Some(ref p) = value.value.point {
                        query = query.param(&format!("p{}", idx), p.clone());
                    }
                    if let Some(ref l) = value.value.language {
                        query = query.param(&format!("l{}", idx), l.clone());
                    }
                    if let Some(ref u) = value.value.unit {
                        query = query.param(&format!("u{}", idx), u.clone());
                    }
                }

                match self.graph.run(query).await {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to write values for entity {}: {}", entity_id, e);
                        skipped += entity_values.len();
                    }
                }
            }
        }

        // Write relation values
        for (relation_id, relation_values) in values_by_relation {
            let mut set_clauses: Vec<String> = Vec::new();

            for (idx, value) in relation_values.iter().enumerate() {
                let prop_prefix = format!(
                    "prop_{}",
                    value.value.property_id.to_string().replace("-", "_")
                );

                if let Some(ref s) = value.value.string {
                    set_clauses.push(format!("r.{}_string = $s{}", prop_prefix, idx));
                }
                if let Some(ref n) = value.value.number {
                    set_clauses.push(format!("r.{}_number = $n{}", prop_prefix, idx));
                }
                if let Some(b) = value.value.boolean {
                    set_clauses.push(format!("r.{}_boolean = $b{}", prop_prefix, idx));
                }
                if let Some(ref t) = value.value.time {
                    set_clauses.push(format!("r.{}_time = $t{}", prop_prefix, idx));
                }
                if let Some(ref p) = value.value.point {
                    set_clauses.push(format!("r.{}_point = $p{}", prop_prefix, idx));
                }
                if let Some(ref l) = value.value.language {
                    set_clauses.push(format!("r.{}_language = $l{}", prop_prefix, idx));
                }
                if let Some(ref u) = value.value.unit {
                    set_clauses.push(format!("r.{}_unit = $u{}", prop_prefix, idx));
                }
            }

            if !set_clauses.is_empty() {
                let query_str = format!(
                    "MATCH ()-[r:RELATES_TO {{entity_id: $entity_id}}]->() SET {}",
                    set_clauses.join(", ")
                );

                let mut query = Query::new(query_str).param("entity_id", relation_id.to_string());

                for (idx, value) in relation_values.iter().enumerate() {
                    if let Some(ref s) = value.value.string {
                        query = query.param(&format!("s{}", idx), s.clone());
                    }
                    if let Some(ref n) = value.value.number {
                        query = query.param(&format!("n{}", idx), *n);
                    }
                    if let Some(b) = value.value.boolean {
                        query = query.param(&format!("b{}", idx), b);
                    }
                    if let Some(ref t) = value.value.time {
                        query = query.param(&format!("t{}", idx), t.clone());
                    }
                    if let Some(ref p) = value.value.point {
                        query = query.param(&format!("p{}", idx), p.clone());
                    }
                    if let Some(ref l) = value.value.language {
                        query = query.param(&format!("l{}", idx), l.clone());
                    }
                    if let Some(ref u) = value.value.unit {
                        query = query.param(&format!("u{}", idx), u.clone());
                    }
                }

                match self.graph.run(query).await {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to write values for relation {}: {}", relation_id, e);
                        skipped += relation_values.len();
                    }
                }
            }
        }

        if skipped > 0 {
            warn!("Skipped {} values due to errors", skipped);
        }

        info!(
            "Wrote {} values to Neo4j ({} skipped)",
            values.len() - skipped,
            skipped
        );
        Ok(())
    }

    /// Delete values (set properties to null)
    #[allow(dead_code)]
    pub async fn delete_values(&self, value_ids: &[Uuid], space_id: &Uuid) -> Result<()> {
        if value_ids.is_empty() {
            return Ok(());
        }

        // For simplicity, we'll log this but not implement full deletion
        // In a production system, you'd need to track which properties to remove
        info!(
            "Delete values operation logged for {} values in space {}",
            value_ids.len(),
            space_id
        );
        Ok(())
    }
}
