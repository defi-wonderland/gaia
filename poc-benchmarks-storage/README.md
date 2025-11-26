# Storage Benchmarks (PostgreSQL vs Neo4j)

A Rust benchmark tool to compare query/read performance between PostgreSQL and Neo4j for different query patterns.

**Companion tool**: See [`poc-edit-ingest`](../poc-edit-ingest) for write/ingest performance benchmarking.

## Features

- **Two benchmark modes**: Direct relation queries and multi-hop traversals
- Direct SQL queries to PostgreSQL (bypassing GraphQL overhead)
- Cypher queries to Neo4j
- Multi-run benchmarking with configurable iterations
- Comprehensive statistics: mean, median, min, max, standard deviation, percentiles (P50, P90, P95, P99)
- Warmup runs to avoid cold start bias
- Clear comparison output with performance ratios

## Benchmark Modes

### 1. Direct Query (PostgreSQL's Strength)
Simple lookup of relations by type ID with JOINs. PostgreSQL excels at these indexed, straightforward queries.

### 2. Multi-Hop Traversal (Neo4j's Strength)
Finding all entities within 2-4 relationship hops from a starting entity. Neo4j excels at graph traversal operations.

## Prerequisites

- PostgreSQL database with indexed data
- Neo4j database with migrated data
- Rust toolchain

## Configuration

Create a `.env` file in the project root with:

```env
DATABASE_URL=postgres://user:password@localhost/database
NEO4J_URI=bolt://localhost:7687
BENCHMARK_ITERATIONS=100  # Optional, defaults to 100
BENCHMARK_WARMUP=5        # Optional, defaults to 5
BENCHMARK_MODE=both       # Optional: "direct", "multihop", or "both" (default)
PROFILE_QUERIES=false     # Optional: set to "true" to enable query profiling (slower)
```

## Usage

Run all benchmarks:

```bash
cargo run --release -p poc-benchmarks-storage
```

Run only direct query benchmark:

```bash
BENCHMARK_MODE=direct cargo run --release -p poc-benchmarks-storage
```

Run only multi-hop traversal benchmark:

```bash
BENCHMARK_MODE=multihop cargo run --release -p poc-benchmarks-storage
```

The `--release` flag is important for accurate performance measurements.