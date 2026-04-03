---
title: "feat: GET /ranks endpoint to fetch ranks from the knowledge graph"
type: feat
date: 2026-04-03
---

# feat: GET /ranks endpoint to fetch ranks from the knowledge graph

## Overview

Add a `GET /ranks?id=<rankEntityId>` REST endpoint to the Gaia API that reconstructs a rank from its underlying knowledge graph primitives (entities, relations, properties) and returns a clean JSON response for frontend consumers.

Ranks are created via the geo-sdk's `createRank()` function, which writes standard GRC-20 entities, relations, and properties. Since these are generic KG primitives, fetching a rank today requires manual graph traversal across multiple tables. This endpoint abstracts that complexity behind a single HTTP call.

Linear epic: [GEO-470](https://linear.app/defi-wonderland/issue/GEO-470)

## Problem Statement

The geo-sdk provides `createRank()` for writing ranks to the knowledge graph, but there is no corresponding read path. Frontend developers who want to display a leaderboard or ranking must:

1. Know the rank entity ID
2. Query the entity's relations to find vote links
3. For each vote relation, fetch the vote entity's properties to get ordinal position or weighted score
4. Resolve voted entity names for display
5. Determine if the rank is ordinal or weighted
6. Sort votes by position or score

This is error-prone, requires deep knowledge of the KG data model, and produces N+1 query patterns on the client.

## Proposed Solution

A dedicated `/ranks` endpoint that:

1. Takes a rank entity UUID as input
2. Fetches the rank's metadata and all votes in 2 SQL queries (no N+1)
3. Validates rank invariants before returning
4. Returns a pre-sorted, display-ready JSON response

### Architecture Decision

This is a read-only endpoint in the API layer. No new database tables, migrations, or indexer changes are needed — ranks already exist as standard entities/relations/properties in the KG. The endpoint reconstructs the rank by querying the existing `values` and `relations` tables with targeted JOINs.

This follows the same pattern as the profile endpoint (`/profile`), which also reconstructs a higher-level concept (user profile) from generic KG primitives.

## How Ranks are Stored in the KG

When the geo-sdk's `createRank()` runs, it produces:

1. A **rank entity** with `NAME_PROPERTY` (text) and optional `DESCRIPTION_PROPERTY` (text)
2. A **type relation** (`TYPES_PROPERTY` → `RANK_TYPE`) identifying the entity as a rank
3. **Vote relations** (`RANK_VOTES_RELATION_TYPE`) from the rank entity to each voted entity
4. Each vote relation has a **vote entity** (the `entityId` field on the relation row) with either:
   - `VOTE_ORDINAL_VALUE_PROPERTY` (text — fractional index string like `"a0"`, `"a1"`) for position-based ranks
   - `VOTE_WEIGHTED_VALUE_PROPERTY` (float) for score-based ranks

### System IDs

| Name | UUID (dashless) | Purpose |
|---|---|---|
| `RANK_VOTES_RELATION_TYPE` | `19a4cfff45f24150abf2af0f43eb2eec` | Relation type for vote links |
| `VOTE_ORDINAL_VALUE_PROPERTY` | `49ee1b8918204e75a1ae38a2dcaad4a5` | Fractional index on vote entity (text) |
| `VOTE_WEIGHTED_VALUE_PROPERTY` | `103701ddcabe4a8e835b10345327b647` | Numeric score on vote entity (float) |
| `NAME_PROPERTY` | `a126ca530c8e48d5b88882c734c38935` | Entity name |
| `DESCRIPTION_PROPERTY` | `9b1f76ff971140ce861e59dc3fa7d037` | Entity description |
| `RANK_TYPE` | `5c74731dfabb4dc8b5c53346521c639a` | Type entity for ranks |
| `TYPES_PROPERTY` | `8f151ba4de204e3c9cb499ddf96f48f1` | Relation type for type assignments |

### KG-to-DB mapping

| KG Concept | DB Table | Key Columns |
|---|---|---|
| Rank name/description | `values` | `entity_id`, `property_id`, `text` |
| Vote link (rank → voted entity) | `relations` | `from_entity_id`, `to_entity_id`, `type_id`, `entity_id` |
| Vote value (position or score) | `values` | `entity_id` (= relation's `entity_id`), `property_id`, `text` or `float` |

## Rank Invariants

Since ranks are built on top of generic GRC-20 primitives, the database may contain malformed rank data. The endpoint validates these invariants before returning a response:

1. **Has votes**: A rank must have one or more votes
2. **Every vote has a value**: Each vote must have either an ordinal or weighted value set
3. **No duplicate voted entities**: Two votes in the same rank cannot target the same entity
4. **No duplicate ordinal positions**: In an ordinal rank, two votes cannot share the same fractional index value

Invalid ranks return HTTP 422 with a descriptive error message.

## Response Shape

### `GET /ranks?id=<uuid>` — 200 OK

```json
{
  "id": "abc123def456...",
  "name": "Best Sci-Fi Movies",
  "description": "Community rank of sci-fi films",
  "rankType": "ordinal",
  "spaceId": "space123...",
  "votes": [
    {
      "entityId": "def456...",
      "entityName": "Blade Runner 2049",
      "voteEntityId": "ghi789...",
      "value": "a1"
    },
    {
      "entityId": "jkl012...",
      "entityName": "Interstellar",
      "voteEntityId": "mno345...",
      "value": "a2"
    }
  ]
}
```

- `rankType`: `"ordinal"` or `"weighted"`, derived from which value property is present on the votes
- `votes`: pre-sorted server-side (ordinal: fractional index ASC, weighted: score DESC)
- `value`: string for ordinal, number for weighted
- `entityName`: voted entity's name, included to avoid client-side N+1 lookups
- `voteEntityId`: the vote relation's entity UUID, included for debugging/reference

### Error responses

| Status | Condition | Body |
|---|---|---|
| 400 | Invalid UUID format | `{ error: "Invalid parameter", message }` |
| 404 | Rank entity not found | `{ error: "Not found", message }` |
| 422 | Rank invariant violated | `{ error: "Invalid rank", message }` |
| 500 | Database error | `{ error: "Internal server error", message: "An unexpected error occurred" }` |

## SQL Strategy

Two queries total, regardless of vote count:

### Query 1: Rank metadata

```sql
SELECT property_id, text
FROM "values"
WHERE entity_id = $rankId
  AND property_id IN ($NAME_PROPERTY, $DESCRIPTION_PROPERTY)
```

### Query 2: Votes with values and entity names

```sql
SELECT
  r.entity_id AS vote_entity_id,
  r.to_entity_id AS voted_entity_id,
  r.space_id,
  v_ordinal.text AS ordinal_value,
  v_weighted.float AS weighted_value,
  v_name.text AS voted_entity_name
FROM relations r
LEFT JOIN "values" v_ordinal
  ON v_ordinal.entity_id = r.entity_id
  AND v_ordinal.property_id = $VOTE_ORDINAL_VALUE_PROPERTY
LEFT JOIN "values" v_weighted
  ON v_weighted.entity_id = r.entity_id
  AND v_weighted.property_id = $VOTE_WEIGHTED_VALUE_PROPERTY
LEFT JOIN "values" v_name
  ON v_name.entity_id = r.to_entity_id
  AND v_name.property_id = $NAME_PROPERTY
WHERE r.from_entity_id = $rankId
  AND r.type_id = $RANK_VOTES_RELATION_TYPE
ORDER BY v_ordinal.text ASC NULLS LAST, v_weighted.float DESC NULLS LAST
```

Both queries run in parallel via `Effect.all`.

## Scope

### In scope

- `api/src/ranks/types.ts` — response and internal types
- `api/src/ranks/queries.ts` — database query functions
- `api/src/ranks/validation.ts` — rank invariant validation
- `api/src/ranks/index.ts` — Hono router with Effect pipeline
- `api/main.ts` — mount the router and logging middleware
- OpenAPI documentation via `describeRoute()`

### Out of scope

- Database migrations (no new tables needed)
- Indexer changes
- geo-sdk changes (read function on the SDK side)
- Pagination (can be added later if ranks grow large)
- Batch endpoint for fetching multiple ranks

## Technical Approach

### Phase 1: Types and response schema ([GEO-471](https://linear.app/defi-wonderland/issue/GEO-471))

Create `api/src/ranks/types.ts` with:

- `RankResponse` — top-level endpoint response type
- `RankVote` — single vote entry
- `RawVoteRow` — raw SQL result row mapping
- `InvalidRankError` — tagged error for invariant violations

### Phase 2: Database queries ([GEO-473](https://linear.app/defi-wonderland/issue/GEO-473))

Create `api/src/ranks/queries.ts` with:

- `getRankMetadata(db, rankId)` — fetches name and description
- `getRankVotes(db, rankId)` — fetches all votes with JOINed values and entity names
- `QueryError` — tagged error for DB failures

Both functions follow the `Effect.tryPromise` + `Effect.withSpan` pattern used throughout the codebase. System IDs defined as module-level constants via `normalizeUuid()`.

### Phase 3: Invariant validation ([GEO-476](https://linear.app/defi-wonderland/issue/GEO-476))

Create `api/src/ranks/validation.ts` with:

- `validateRank(votes: RawVoteRow[])` — pure function that checks all 4 invariants

Check order: empty → missing values → duplicate entities → duplicate ordinal positions. Returns `Effect.fail(InvalidRankError)` on first violation with a descriptive message.

### Phase 4: Router and endpoint ([GEO-479](https://linear.app/defi-wonderland/issue/GEO-479))

Create `api/src/ranks/index.ts` with `createRanksRouter(db, runtime)`:

Request flow:
```
Input validation (UUID format)
  → getRankMetadata(db, rankId)  ─┐
  → getRankVotes(db, rankId)     ─┤  (parallel via Effect.all)
                                   ↓
  → validateRank(rawVotes)          // check invariants
  → determine rankType              // ordinal if ordinal_value present, else weighted
  → map to RankResponse             // normalize UUIDs, pick value field
  → return JSON
```

Mount in `api/main.ts`:
```typescript
app.use("/ranks/*", canonicalRequestLogging())
app.route("/ranks", createRanksRouter(db, runtime))
```

### Dependency graph

```
GEO-471 (types) ──→ GEO-473 (queries) ──┐
                 ──→ GEO-476 (validation) ┤──→ GEO-479 (router + mount)
```

GEO-473 and GEO-476 can be worked in parallel once types are done.

## Local Research

Relevant existing patterns in this repo:

- `api/src/profile/index.ts` — simplest router pattern, reconstructs a concept from KG primitives
- `api/src/profile/queries.ts` — Effect.tryPromise + raw SQL + system ID constants
- `api/src/versioned/router.ts` — Effect.gen pipeline with tagged error handling
- `api/src/versioned/queries.ts` — batch query patterns with JOINs
- `api/src/services/storage/schema.ts` — `values` and `relations` table definitions
- `api/src/utils/uuid.ts` — UUID validation and normalization utilities

External reference:

- [geo-sdk ranks module](https://github.com/geobrowser/geo-sdk/tree/main/src/ranks) — `createRank()` implementation that defines the KG structure we're reading
