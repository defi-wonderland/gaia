//! Bulk operation utilities for OpenSearch.
//!
//! This module provides functions for executing and parsing bulk operations
//! with OpenSearch.

use opensearch::{BulkOperation, BulkParts, OpenSearch};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, error};

use crate::errors::SearchIndexError;
use crate::types::{BatchOperationResult, BatchOperationSummary};

/// Wrapper for bulk update operations with doc_as_upsert support.
#[derive(Serialize)]
pub struct BulkUpdateBody {
    pub doc: Value,
    pub doc_as_upsert: bool,
}

/// Wrapper for bulk scripted update operations.
#[derive(Serialize)]
pub struct BulkScriptBody {
    pub script: BulkScript,
}

/// Script definition for bulk scripted updates.
#[derive(Serialize)]
pub struct BulkScript {
    pub source: String,
    pub lang: &'static str,
}

impl BatchOperationSummary {
    /// Create an empty BatchOperationSummary.
    pub fn empty() -> Self {
        Self {
            total: 0,
            succeeded: 0,
            failed: 0,
            results: Vec::new(),
        }
    }
}

/// Metadata for tracking bulk operation results.
#[derive(Debug, Clone)]
pub struct BulkOperationMeta {
    pub entity_id: String,
    pub space_id: String,
}

/// Execute a bulk request and parse the response into a BatchOperationSummary.
pub async fn execute_bulk<B: Serialize>(
    client: &OpenSearch,
    alias: &str,
    operations: Vec<BulkOperation<B>>,
    metas: &[BulkOperationMeta],
    action: &str,
) -> Result<BatchOperationSummary, SearchIndexError> {
    let response = client
        .bulk(BulkParts::Index(alias))
        .body(operations)
        .send()
        .await
        .map_err(|e| SearchIndexError::bulk_index(e.to_string()))?;

    let status = response.status_code();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        error!(status = %status, body = %error_body, "Bulk {} request failed", action);
        return Err(SearchIndexError::bulk_index(format!(
            "Bulk {} failed with status {}: {}",
            action, status, error_body
        )));
    }

    let response_body: Value = response
        .json()
        .await
        .map_err(|e| SearchIndexError::parse(e.to_string()))?;

    let summary = parse_bulk_response(&response_body, metas, action);

    debug!(
        total = summary.total,
        succeeded = summary.succeeded,
        failed = summary.failed,
        "Bulk {} completed",
        action
    );

    Ok(summary)
}

/// Parse the bulk API response and build a BatchOperationSummary.
pub fn parse_bulk_response(
    response_body: &Value,
    metas: &[BulkOperationMeta],
    action: &str,
) -> BatchOperationSummary {
    let mut results = Vec::with_capacity(metas.len());
    let mut succeeded = 0;
    let mut failed = 0;

    let items = response_body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for (i, meta) in metas.iter().enumerate() {
        let item_result = items.get(i).and_then(|item| item.get(action));

        let (success, error) = if let Some(result) = item_result {
            let status = result.get("status").and_then(|s| s.as_u64()).unwrap_or(0);
            let is_success =
                (200..300).contains(&(status as u16)) || status == 404 && action == "delete"; // 404 on delete is OK

            if is_success {
                (true, None)
            } else {
                let error_msg = result
                    .get("error")
                    .map(|e| {
                        e.get("reason")
                            .and_then(|r| r.as_str())
                            .map(|reason| reason.to_string())
                            .unwrap_or_else(|| e.to_string())
                    })
                    .unwrap_or_else(|| {
                        format!("Bulk {} failed with status {}", action, status)
                    });
                (false, Some(SearchIndexError::bulk_index(error_msg)))
            }
        } else {
            // No result found for this index - this shouldn't happen
            (
                false,
                Some(SearchIndexError::bulk_index(format!(
                    "No result found for operation at index {}",
                    i
                ))),
            )
        };

        if success {
            succeeded += 1;
        } else {
            failed += 1;
        }

        results.push(BatchOperationResult {
            entity_id: meta.entity_id.clone(),
            space_id: meta.space_id.clone(),
            success,
            error,
        });
    }

    BatchOperationSummary {
        total: metas.len(),
        succeeded,
        failed,
        results,
    }
}

