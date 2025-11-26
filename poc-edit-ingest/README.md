# POC Edit Ingest

A refactored service that receives Edit protobuf messages and processes them through the KgIndexer with dual PostgreSQL + Neo4j support for performance benchmarking.

**Companion tool**: See [`poc-benchmarks-storage`](../poc-benchmarks-storage) for read/query performance benchmarking.

## Architecture

The project is organized into focused modules for maintainability:

```
poc-edit-ingest/
├── src/
│   ├── main.rs              # Server entry point
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration & constants
│   ├── models.rs            # Shared types
│   ├── server/              # HTTP server
│   │   ├── mod.rs
│   │   ├── handlers.rs
│   │   └── state.rs
│   ├── decoder/             # Edit processing
│   │   ├── mod.rs
│   │   └── worker.rs
│   ├── storage/             # Database layer
│   │   ├── mod.rs
│   │   ├── initializer.rs
│   │   └── dual_writer.rs
│   ├── neo4j_writer.rs      # Neo4j integration
│   └── bin/                 # Client binaries
│       ├── csv_loader.rs
│       └── setup_rank_properties.rs
```

## Quick Start

### 1. Server Setup

```bash
# Set environment variables in .env file
echo "DATABASE_URL=postgresql://user:password@localhost:5432/gaia" > .env
echo "ENABLE_NEO4J=true" >> .env
echo "NEO4J_URI=bolt://localhost:7687" >> .env

# Run the server
cargo run
```

The server starts on `http://127.0.0.1:8080` with CORS enabled for localhost origins (ports 3000 and 5173).

### 2. CSV Bulk Import

Load multiple edits from a CSV file:

```bash
# Run the CSV loader client
cargo run --bin csv_loader data/random-rankings.csv

# Or with custom server URL
SERVER_URL=http://127.0.0.1:8080/cache cargo run --bin csv_loader data/random-rankings.csv
```

**CSV Format:**
```csv
rank_id,rank_name,category,item_count,encoded_edit
1,Top Artists,music,100,<base64-encoded-protobuf>
```

### 3. Single Edit Test

Send a single test edit:

```bash
cargo run --bin setup_rank_properties
```

## API

### POST /cache

Submit an Edit for processing.

**Request:**
```json
{
  "data": "<base64-encoded protobuf bytes>"
}
```

**Response (Success):**
```json
{
  "status": "success",
  "message": "Edit sent to decoder (1 receivers)"
}
```

**Response (Error):**
```json
{
  "status": "error",
  "message": "Failed to decode base64: Invalid byte 61, offset 0."
}
```

### GET /health

Health check endpoint.

**Response:** `"Combined server is running"`

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | **Required** |
| `ENABLE_NEO4J` | Enable Neo4j dual-write | `false` |
| `NEO4J_URI` | Neo4j connection URI | - |
| `SERVER_URL` | Server endpoint (for clients) | `http://127.0.0.1:8080/cache` |

## How It Works

### Server Flow

1. **HTTP Server** receives base64-encoded Edit via `POST /cache`
2. **Handler** decodes base64 → protobuf → Edit struct
3. **Channel** broadcasts Edit to background decoder task
4. **Decoder Worker** processes Edit through storage layer
5. **Dual Writer** writes to both PostgreSQL (primary) and Neo4j (secondary)
6. Performance metrics are logged for comparison

### Client Flow (CSV Loader)

1. **CSV Reader** parses CSV file with encoded edits
2. **HTTP Client** sends each edit to server via POST request
3. **Progress Tracking** logs success/failure statistics
4. **Summary** displays total time and averages

## Dual Storage Strategy

- **PostgreSQL**: Primary storage. If this fails, the operation fails.
- **Neo4j**: Secondary storage for benchmarking. If this fails, we log a warning but continue.

This allows performance comparison without compromising data integrity.

## Examples

### JavaScript/TypeScript (Frontend)

```javascript
// Your Edit protobuf bytes
const editBytes = new Uint8Array([10,16,35,139,191,116,124,254,77,185,...]);

// Convert to base64
const base64Data = btoa(String.fromCharCode(...editBytes));

// Send to server (works from localhost:3000 or localhost:5173)
const response = await fetch('http://127.0.0.1:8080/cache', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ data: base64Data })
});

const result = await response.json();
console.log(result);
```

### cURL

```bash
curl -X POST http://127.0.0.1:8080/cache \
  -H "Content-Type: application/json" \
  -d '{"data": "ChAji791fP5NuZCniGAFSVjtEg5Gb29kIFJhbmsgTGlzdBpbCllKEGsJYmxp..."}'
```

### Python

```python
import base64
import requests

# Your Edit protobuf bytes
edit_bytes = bytes([10,16,35,139,191,116,124,254,77,185,...])

# Convert to base64
base64_data = base64.b64encode(edit_bytes).decode('utf-8')

# Send to server
response = requests.post(
    'http://127.0.0.1:8080/cache',
    json={'data': base64_data}
)

print(response.json())
```

## Available Binaries

| Binary | Purpose | Usage |
|--------|---------|-------|
| `poc-edit-ingest` | Main server | `cargo run` |
| `csv_loader` | Bulk CSV import | `cargo run --bin csv_loader <csv_file>` |
| `setup_rank_properties` | Single test edit | `cargo run --bin setup_rank_properties` |

## Development

### Building

```bash
# Build all binaries
cargo build

# Build release version
cargo build --release
```

### Running Tests

```bash
cargo test
```

## Configuration

All configuration constants are centralized in `src/config.rs`:

- Property UUIDs (name, rank_type, ranks, score)
- Default space ID
- Server host/port
- CORS settings

To modify, edit `src/config.rs` instead of searching through the codebase.
