# Actions Indexer Repository

This crate provides traits and implementations for interacting with the actions data repository. It abstracts the underlying database operations, allowing other crates to persist and retrieve action-related data without direct knowledge of the storage mechanism.

## Overview

The `actions-indexer-repository` crate includes:

- **Interfaces:** Defines the `ActionsRepository` trait, which specifies the contract for data persistence operations (e.g., inserting actions, updating user votes, persisting changesets).
- **PostgreSQL Implementation:** Provides a concrete implementation of the `ActionsRepository` trait for PostgreSQL databases, handling connection pooling and transactional operations.
- **Error Handling:** Defines specific error types related to repository operations, such as database errors.

## Usage

This crate is primarily used by the `actions-indexer-pipeline` crate, specifically by the `ActionsLoader` component, to store processed action data. It can also be used independently by any application that needs to interact with the actions data store.

To include this crate in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
actions-indexer-repository = { path = "../actions-indexer-repository" }
```

## Testing

This crate uses integration tests with a real PostgreSQL database to verify repository functionality.

### Local Development Setup

1. **Start PostgreSQL database:**
   ```bash
   docker run --rm --name pg_test \
     -e POSTGRES_PASSWORD=postgres \
     -e POSTGRES_USER=postgres \
     -e POSTGRES_DB=postgres \
     -p 5432:5432 -d postgres:16
   ```

2. **Set environment variable:**
   ```bash
   export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/postgres"
   ```

3. **Run integration tests:**
   ```bash
   # Run all integration tests
   cargo test --test '*'
   
   # Run specific test file
   cargo test --test postgres_integration
   
   # Run specific test function
   cargo test --test postgres_integration test_insert_raw_action
   
   # Or simply run all tests (same as integration tests in this crate)
   cargo test
   ```