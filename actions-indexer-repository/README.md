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
