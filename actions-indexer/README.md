# Actions Indexer

This crate provides the main application for indexing actions. It orchestrates the entire process of consuming, processing, and loading action events into the repository.

## Overview

The `actions-indexer` application is responsible for:

- Initializing and managing application-wide dependencies.
- Coordinating the data flow through the actions pipeline (consumer, processor, loader).
- Handling overall application startup and shutdown.

## Build and Run

To build the `actions-indexer` application, navigate to the crate's root directory and run:

```bash
cargo build
```

To run the application, use:

```bash
cargo run
```

// TODO: Add environment variables config section

Ensure you have the necessary environment variables configured for database connections and other dependencies.
