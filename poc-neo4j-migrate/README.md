# PostgreSQL to Neo4j Migration Tool

A refactored Rust utility to migrate graph data from PostgreSQL to Neo4j for benchmarking purposes.

## Architecture

The project is organized into focused modules for maintainability:

```
poc-neo4j-migrate/
├── src/
│   ├── main.rs              # Entry point (~30 lines)
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration & constants
│   ├── models.rs            # Data structures
│   ├── postgres/            # PostgreSQL operations
│   │   ├── mod.rs
│   │   ├── connection.rs    # Connection setup
│   │   └── reader.rs        # Read functions
│   ├── neo4j/               # Neo4j operations  
│   │   ├── mod.rs
│   │   ├── connection.rs    # Connection setup
│   │   ├── writer.rs        # Write functions
│   │   └── indexer.rs       # Index/constraint creation
│   └── migration/           # Migration orchestration
│       ├── mod.rs
│       └── executor.rs      # Migration flow
```

## Overview

This tool migrates the following tables from PostgreSQL to Neo4j:
- **entities** → Neo4j Entity nodes
- **relations** → Neo4j RELATES_TO relationships
- **values** → Properties on Entity nodes and relationships
- **properties** → Property type metadata

## Prerequisites

- PostgreSQL database with data
- Neo4j instance running (without authentication)
- Rust toolchain installed

## Environment Variables

Create a `.env` file in the project root with:

```bash
DATABASE_URL=postgresql://user:password@localhost:5432/dbname
NEO4J_URI=bolt://localhost:7687  # or neo4j://localhost:7687
```

## Usage

### Build the tool

```bash
cargo build --release -p poc-neo4j-migrate
```

### Run the migration

```bash
# From the workspace root
cargo run --release -p poc-neo4j-migrate

# Or run the binary directly
./target/release/poc-neo4j-migrate
```

## Configuration

All configuration constants are centralized in `src/config.rs`:

- **Batch sizes**: Entity batch size, property batch size
- **Concurrency limits**: Parallel processing for relationships and properties
- **Progress reporting intervals**: How often to log progress updates
- **Connection pool settings**: PostgreSQL max connections

## Migration Process

The migration follows these steps:

1. **Read data from PostgreSQL**
   - Entities
   - Relations
   - Values
   - Properties

2. **Organize data**
   - Separate entity values from relation values
   - Create lookup maps for efficient processing

3. **Write to Neo4j**
   - Clear existing data (optional)
   - Create entity nodes (batch processing)
   - Create relationships (concurrent processing)
   - Add properties to entities (concurrent processing)
   - Add properties to relationships (concurrent processing)

4. **Create indexes and constraints**
   - Unique constraint on Entity.id
   - Index on RELATES_TO.type_id
   - Index on RELATES_TO.entity_id
   - Index on RELATES_TO.space_id

5. **Report statistics**
   - Total migration time
   - Counts for all data types
   - Performance metrics

## Performance Features

- **Batch processing**: Entities are created in configurable batches
- **Concurrent writes**: Relationships and properties are written concurrently with configurable limits
- **Progress tracking**: Regular progress updates during long-running operations
- **Index creation**: Automatic index creation for optimal query performance

## Key Benefits

- **Maintainability**: Clear module boundaries, single responsibility
- **Reusability**: PostgreSQL readers and Neo4j writers can be used independently
- **Testability**: Each module can be tested in isolation
- **Configurability**: Centralized configuration for easy tuning
- **Performance**: Optimized batch and concurrent processing
