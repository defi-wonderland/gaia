# POC Edit Decoder

A service that receives Edit protobuf messages from a frontend and processes them through the KgIndexer.

## Quick Start

```bash
# Set DATABASE_URL in .env file
echo "DATABASE_URL=postgresql://user:password@localhost:5432/gaia" > .env

# Run the server
cargo run
```

The server will start on `http://127.0.0.1:8080`

**CORS:** Enabled for localhost origins (ports 3000 and 5173)

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

## Example

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
// Output: { "status": "success", "message": "Edit sent to decoder (1 receivers)" }
```

### cURL

```bash
# Example with base64-encoded Edit
curl -X POST http://127.0.0.1:8080/cache \
  -H "Content-Type: application/json" \
  -d '{
    "data": "ChAji791fP5NuZCniGAFSVjtEg5Gb29kIFJhbmsgTGlzdBpbCllKEGsJYmxpVUhn..."
  }'
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

## How It Works

1. **Frontend** encodes Edit protobuf bytes as base64 and sends to `POST /cache`
2. **HTTP Server** decodes base64 → protobuf bytes → Edit struct
3. **Channel** broadcasts Edit to decoder task
4. **Decoder Task** processes Edit through KgIndexer
5. **KgIndexer** writes to PostgreSQL database (properties, entities, relations, etc.)

## Health Check

```bash
curl http://127.0.0.1:8080/health
# Response: "Combined server is running"
```

