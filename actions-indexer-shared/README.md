# Actions Indexer Shared

This crate defines shared data structures and types used across the actions indexer ecosystem. It provides common definitions for various entities, ensuring consistency and interoperability between different components of the indexer.

## Overview

The `actions-indexer-shared` crate includes fundamental data types such as:

- **`ActionEvent`**: Represents a raw, unprocessed event related to an action.
- **`Action`**: Represents a processed and structured action with its relevant data.
- **`UserVote`**: Stores information about a user's vote on an entity or space.
- **`VotesCount`**: Aggregates vote counts (e.g., upvotes, downvotes) for entities or spaces.
- **`Changeset`**: Bundles a collection of changes (actions, user votes, vote counts) for atomic persistence.

## Usage

This crate is a foundational dependency for other `actions-indexer` crates, providing the common language and structure for data exchange.

To include this crate in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
actions-indexer-shared = { path = "../actions-indexer-shared" }
```
