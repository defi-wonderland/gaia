# Actions Indexer

This crate provides the main application for indexing actions from Substreams. It orchestrates the entire process of consuming action events from a Substreams endpoint, processing them through registered handlers, and persisting the results to a PostgreSQL database.

## Overview

The `actions-indexer` application follows a consumer-processor-loader architecture and is responsible for:

- **Consuming**: Reading action events from Substreams using a configured endpoint and package
- **Processing**: Handling actions through registered handlers (currently supports vote actions)
- **Loading**: Persisting processed actions to a PostgreSQL database
- **Orchestrating**: Coordinating the data flow through the entire pipeline

## Architecture

The application is built on three main components:

- **ActionsConsumer**: Consumes actions from Substreams using the `SubstreamsStreamProvider`
- **ActionsProcessor**: Processes actions through registered handlers (e.g., `VoteHandler` for vote actions)
- **ActionsLoader**: Persists processed actions using the `PostgresActionsRepository`

## Supported Actions

Currently, the indexer supports the following action types:

- **Vote Actions**: Handles voting with values Up (0), Down (1), and Remove (2)

## Configuration

### Environment Variables

The following environment variables must be set before running the application:

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL database connection string |
| `SUBSTREAMS_ENDPOINT` | The Substreams API endpoint URL |
| `SUBSTREAMS_API_TOKEN` | Authentication token for Substreams API access |

You can set these variables in a `.env` file in the project root:

```bash
DATABASE_URL= # Your database connection string
SUBSTREAMS_ENDPOINT= # Substreams endpoint URL
SUBSTREAMS_API_TOKEN= # Substream API token
```

### Substreams Package

The application uses a packaged Substreams module located at:
- **Package**: `./src/package/geo-actions-v0.1.0.spkg`
- **Module**: `map_actions`

## Database Setup

Before running the application, you need to create the required database tables. You have several options:

```bash
# Extract connection details from DATABASE_URL or use it directly
psql $DATABASE_URL -f ../actions-indexer-repository/src/postgres/migrations/0000_init_actions.sql
```

The migration will create the following tables:
- `raw_actions` - Stores processed blockchain actions
- `user_votes` - Individual voting records  
- `votes_count` - Aggregated vote tallies per entity/space

## Build and Run

To build the `actions-indexer` application:

```bash
cargo build
```

To run the application:

```bash
cargo run
```