# Echo Module

A simple test module for MAIA that demonstrates the module interface and provides basic echo capabilities.

## Purpose

The Echo module serves as:

1. **Testing Target**: Validates module loading, initialization, and request handling
2. **Reference Implementation**: Shows how to properly implement the `MaiaModule` trait
3. **Development Tool**: Useful for debugging the module system

## Capabilities

### `echo.simple`

Returns the input unchanged, wrapped in a response object.

**Request:**

```json
{
  "capability": "echo.simple",
  "payload": {
    "any": "data",
    "you": "want"
  }
}
```

**Response:**

```json
{
  "echo": {
    "any": "data",
    "you": "want"
  },
  "capability": "echo.simple"
}
```

### `echo.timestamp`

Adds UTC timestamp information to the input data. Useful for testing request timing and latency.

**Request:**

```json
{
  "capability": "echo.timestamp",
  "payload": {
    "message": "hello",
    "user_id": 123
  }
}
```

**Response:**

```json
{
  "data": {
    "message": "hello",
    "user_id": 123
  },
  "timestamp": "2025-01-15T10:30:45.123Z",
  "unix_timestamp": 1705318245,
  "capability": "echo.timestamp"
}
```

### `echo.stats`

Returns module statistics including request counts and bytes processed.

**Request:**

```json
{
  "capability": "echo.stats",
  "payload": {}
}
```

**Response:**

```json
{
  "total_requests": 42,
  "total_bytes": 1024,
  "requests_by_capability": {
    "echo.simple": 20,
    "echo.timestamp": 15,
    "echo.stats": 7
  }
}
```

## Resource Requirements

The Echo module is extremely lightweight:

- **Memory**: 10 MB
- **CPU**: 10 shares (minimal)
- **Disk**: None
- **Network**: None
- **GPU**: None

## Isolation

Designed for **WASM** isolation level (full sandboxing).

## Dependencies

The Echo module has no dependencies on other modules or external services.

## Usage

### As a Library (Rust)

```rust
use maia_module_echo::EchoModule;
use maia_sdk::prelude::*;

#[tokio::main]
async fn main() {
    let mut module = EchoModule::new();

    // Initialize
    let context = create_module_context();
    module.initialize(context).await.unwrap();

    // Start
    module.start().await.unwrap();

    // Make a request
    let request = Request::new(
        "echo.timestamp",
        serde_json::json!({"message": "MAIA", "version": "0.1.0"})
    );

    let response = module.handle_request(request).await.unwrap();
    println!("Response: {}", response.result.unwrap());
}
```

### Via MAIA Core

```bash
# Load the module
maia module load echo-module.wasm

# Send a request
maia request echo.timestamp '{"message": "Hello", "data": 42}'

# Get statistics
maia request echo.stats '{}'
```

## Testing

The module includes comprehensive tests:

```bash
cargo test
```

Tests cover:

- Module lifecycle (initialize, start, stop)
- All capabilities (simple, timestamp, stats)
- Error handling (invalid capabilities)
- Health checks and metrics
- Manifest and capability declarations
- Timestamp accuracy and format

## Implementation Notes

### Thread Safety

The module uses `Arc<EchoStats>` for shared statistics tracking across requests, with atomic operations for counters and
a RwLock for the capability map.

### Error Handling

Demonstrates proper error handling patterns:

- Returns `FatalError::CapabilityNotFound` for unknown capabilities
- Returns `TemporaryError::ModuleUnavailable` when not running
- All inputs accepted for timestamp (no validation errors)

### Logging

Uses the `ModuleCallback` system to log initialization, start, and stop events back to the MAIA core.

### Metrics

Tracks:

- Total requests processed
- Total bytes processed
- Requests per capability

## Building

### As Rust Library

```bash
cargo build --release
```

### As WASM Module

```bash
cargo build --release --target wasm32-wasi
```

The WASM binary will be at:

```
target/wasm32-wasi/release/maia_module_echo.wasm
```

## License

MIT License - same as MAIA core.

## Contributing

This module is intentionally kept simple. For enhancements:

1. Keep it lightweight
2. Maintain comprehensive tests
3. Document all changes
4. Ensure it remains a good reference implementation