# Actions Indexer Pipeline

This crate defines the core traits and modules for processing actions within the indexer. It establishes the pipeline components for consuming, loading, processing, and orchestrating action events.

## Overview

The `actions-indexer-pipeline` crate consists of the following key modules:

- **Consumer:** Responsible for ingesting raw action events from a data source.
- **Processor:** Handles the business logic and transformations of raw action events into structured action data.
- **Loader:** Manages the persistence of processed action data into the repository.
- **Orchestrator:** Coordinates the flow between the consumer, processor, and loader, ensuring a seamless data pipeline.

## Usage

This crate provides the foundational interfaces and structures for building an action indexing pipeline. It is typically used as a dependency by the `actions-indexer` application, which ties these components together into a functional system.

To include this crate in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
actions-indexer-pipeline = { path = "../actions-indexer-pipeline" }
```
