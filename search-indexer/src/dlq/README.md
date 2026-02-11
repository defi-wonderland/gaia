# Dead Letter Queue (DLQ)

The DLQ captures individual operations that fail during bulk indexing to OpenSearch. Failed operations are published as JSON records to a dedicated Kafka topic so they can be inspected, debugged, and potentially replayed without blocking the main consumer.

## Architecture

```
Kafka source topic
        │
        ▼
   ┌─────────┐
   │  Loader  │── bulk_operations() ──▶ OpenSearch
   └─────────┘
        │
        │ per-item failures
        ▼
   ┌─────────┐
   │   DLQ   │──▶ Kafka DLQ topic (e.g. dev.search-indexer.dlq)
   │ Producer│
   └─────────┘
        │
        ▼
   ┌─────────┐
   │   DLQ   │  Tracks poisoned entity_id+space_id pairs.
   │  State  │  Rebuilt from the DLQ topic on startup.
   └─────────┘
```

## When the DLQ is used

The DLQ only applies to **entity batches**. Score batches (`UpdateEntityGlobalScore`, `UpdateSpaceScore`, `UpdateEntitySpaceScore`) bypass the DLQ entirely — scores are idempotent and periodically recomputed, so a NACK and redelivery is sufficient.

For entity batches, the DLQ is consulted when `bulk_operations()` returns `Ok(summary)` with `failed > 0` (partial failure). Each failed item is published as a `DlqRecord` containing the full operation payload, then the batch is ACKed. Successfully indexed items are not reprocessed.

## Failure modes

### Per-item failures (→ DLQ eligible)

These are cases where `bulk_operations()` returns `Ok(summary)` with some items marked `success: false` and `retryable: false`:

| Failure | Cause |
|---|---|
| Painless script runtime error | Data corruption (e.g. `type_relations` field exists but isn't an array), null pointer in script |
| Mapping type conflict | Field value doesn't match index mapping (e.g. string sent where float expected) |
| Version conflict (bulk update) | Two concurrent bulk requests update the same document; the second gets `409` |

### Per-item infrastructure failures (→ NACK, not DLQ)

These are cases where `bulk_operations()` returns `Ok(summary)` with some items marked `success: false` and `retryable: true`. The entire batch is NACKed for Kafka redelivery. On the next attempt, transient errors will succeed and any permanent failures will re-fail and route to DLQ.

| Failure | HTTP Status | Cause |
|---|---|---|
| Shard unavailable | `503` | Rolling restart, shard relocation — primary shard for a specific document is temporarily down |
| Disk pressure / flood stage | `403` | OpenSearch hits disk watermarks, sets index to read-only (`cluster_block_exception`) |
| Circuit breaker / too many requests | `429` | OpenSearch rejects requests due to memory pressure |
| Server error | `500+` | Internal server error, gateway timeout, etc. |

### Total failures (→ always NACK, DLQ never consulted)

These are cases where `bulk_operations()` returns `Err`. The entire batch is NACKed for redelivery:

| Failure | Cause |
|---|---|
| OpenSearch unreachable | `send().await` returns connection error |
| HTTP-level failure | Bulk API returns non-2xx (e.g. `503` during full cluster restart) |
| Invalid UUID in batch | A single bad UUID causes `bulk_operations()` to return `Err` via `?`, failing the entire batch |
| Intermediate flush failure | `flush_pending_bulk!` fails mid-batch (e.g. between an Update and a subsequent Unset) |

### Silently skipped (no DLQ, no NACK)

| Failure | Cause |
|---|---|
| Version conflict on `update_by_query` | `RemoveTypeRelationById` and score operations use `Conflicts::Proceed`, so version conflicts are silently skipped |

## Race conditions

### update_by_query is not atomic

`RemoveTypeRelationById`, `UpdateEntityGlobalScore`, and `UpdateSpaceScore` use `update_by_query`, which takes a snapshot of the index then updates documents one-by-one. If another write modifies a document between the snapshot and the update, a version conflict occurs. Because these operations use `Conflicts::Proceed`, the conflicted update is **silently skipped** — meaning a `RemoveTypeRelationById` could be lost if the document was concurrently modified.

### Cross-instance concurrency

Two indexer instances processing different Kafka partitions could update the same entity concurrently. Both use `upsert`, so one creates the document and the other updates it. The "loser" of the version conflict gets a per-item failure, which routes to the DLQ.

### Refresh window

`flush_pending_bulk!` uses `refresh: true` to make changes visible before a subsequent `update_by_query`. There is a window between the refresh and the query where another write could land, causing version conflicts handled by `Conflicts::Proceed`.

## Circuit breaker

The DLQ tracks poisoned `entity_id + space_id` pairs in `DlqState`. If the number of unique poisoned pairs exceeds `DLQ_MAX_POISONED_ENTITIES` (default: 10,000), the indexer shuts down. This prevents unbounded DLQ growth from a systemic issue (e.g. mapping corruption).

## DLQ record format

Each record is published as JSON to the DLQ Kafka topic:

```json
{
  "dlq_id": "a1b2c3d4-...",
  "entity_id": "e5f6a7b8-...",
  "space_id": "c9d0e1f2-...",
  "operation_type": "Update",
  "error_message": "bulk operation failed: ...",
  "source_batch_type": "entities",
  "source_topic": "knowledge.edits",
  "source_partition": 0,
  "source_offset": 42,
  "failed_at": "2025-01-15T10:30:00Z",
  "retry_count": 0,
  "max_retries": 3,
  "operation_payload": {
    "Update": {
      "entity_id": "e5f6a7b8-...",
      "space_id": "c9d0e1f2-...",
      "name": "My Entity",
      "description": null,
      "avatar": null,
      "cover": null,
      "add_type_relation": null,
      "entity_global_score": null,
      "space_score": null,
      "entity_space_score": null,
      "deleted": null
    }
  }
}
```

The `operation_payload` field contains the full `EntityOperation` serialized as a JSON-tagged enum. To deserialize back:

```rust
let op: EntityOperation = serde_json::from_value(record.operation_payload)?;
```

Will be `null` if the operation could not be serialized (logged as an error).

## Configuration

| Environment Variable | Default | Description |
|---|---|---|
| `DLQ_ENABLED` | `true` | Enable/disable DLQ. When disabled, partial failures revert to NACK. |
| `DLQ_MAX_RETRIES` | `3` | Max retry attempts before permanent discard (Phase 2). |
| `DLQ_TOPIC` | `search-indexer.dlq` | Base topic name (environment prefix is prepended automatically). |
| `DLQ_MAX_POISONED_ENTITIES` | `10000` | Circuit breaker limit for unique poisoned entity+space pairs. |

## Module structure

| File | Purpose |
|---|---|
| `types.rs` | `DlqRecord` and `DlqConfig` definitions |
| `producer.rs` | Kafka producer with `send_best_effort()` for publishing DLQ records |
| `state.rs` | `DlqState` — tracks poisoned entities, rebuilt from DLQ topic on startup |
