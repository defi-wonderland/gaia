# Actions Substreams

A Substreams module for indexing action events from the Gaia knowledge graph on Arbitrum blockchain. This substream extracts and processes action events emitted by the actions smart contract, providing structured data for downstream indexing and analysis.

## Overview

The actions substream monitors blockchain events from the actions smart contract and transforms them into structured `Action` messages. These actions represent user interactions within the Gaia knowledge graph ecosystem, including entity operations, group management, and metadata updates.

### Key Features

- **Real-time Action Processing**: Streams action events as they occur on the blockchain
- **Structured Data Output**: Converts raw blockchain logs into structured protobuf messages
- **Efficient Filtering**: Only processes events from the tracked actions contract
- **Metadata Decoding**: Extracts and decodes action metadata, entity IDs, and group information

## Architecture

This substream is part of the larger Gaia data service stack:

```
Blockchain (Arbitrum) → Actions Substream → Actions Indexer Pipeline → Database → API
```

## Contract Information

- **Network**: Arbitrum
- **Actions Contract**: `0x20b8d41da487c80e06667409e81ab8b173c9e076`
- **Starting Block**: 62436

## Data Schema

The substream outputs `Actions` messages containing individual `Action` records with the following fields:

```protobuf
message Action {
    uint64 action_type = 1;          // Type of action performed
    uint64 action_version = 2;       // Version of the action schema
    string sender = 3;               // Address that initiated the action
    string entity = 4;               // Target entity UUID
    optional string group_id = 5;    // Optional group UUID
    string space_pov = 6;            // Space address (point of view)
    optional bytes metadata = 7;     // Optional action metadata
    uint64 block_number = 8;         // Block number where action occurred
    uint64 block_timestamp = 9;      // Block timestamp
    string tx_hash = 10;            // Transaction hash
}
```

## Building and Running

### Prerequisites

- [Rust](https://www.rust-lang.org/) (with wasm32-unknown-unknown target)
- [Substreams CLI](https://substreams.dev/documentation/getting-started/quickstart)

### Build the Substream

```bash
# Build the WASM module
substreams build

# Optionally build optimized for production
cargo build --release --target wasm32-unknown-unknown
```

### Environment Variables

- `SUBSTREAMS_API_TOKEN`: Authentication token for the Substreams service
- `SUBSTREAMS_ENDPOINT`: Substreams endpoint URL

## Development

### Testing

Run the substream against historical blocks to verify output:

```bash
substreams run map_actions -s 62436 -t +1000
```

## Modules

### `map_actions`

**Type**: Map module  
**Input**: `sf.ethereum.type.v2.Block`  
**Output**: `proto:actions.v1.Actions`

Processes Ethereum blocks and extracts action events from the monitored contract. The module:

1. Filters transactions for logs from the actions contract
2. Decodes action event data using custom log parsing
3. Enriches action data with block and transaction metadata
4. Outputs structured Action messages for downstream processing
