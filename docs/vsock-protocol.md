# vsock Communication Protocol

## Overview

The IronClaw vsock protocol provides a secure, low-latency communication channel between the host orchestrator and guest VMs. It uses Unix domain sockets for communication and is the only permitted communication method for network-isolated VMs.

## Design Goals

1. **Security**: Messages are size-limited to prevent DoS attacks
2. **Simplicity**: JSON-based protocol for easy debugging and implementation
3. **Performance**: Low-latency communication with async/await support
4. **Reliability**: Request/response pattern with error handling

## Protocol Specification

### Message Format

All messages follow a simple binary format:

```
[Length (4 bytes, big-endian)][JSON Payload (Length bytes)]
```

**Length**: 32-bit unsigned integer, big-endian byte order
**Payload**: UTF-8 encoded JSON

**Maximum Size**: 16,777,216 bytes (16 MB)

### Message Types

#### 1. Request Message

Sent from guest to host to request an operation:

```json
{
  "Request": {
    "id": "unique-request-id",
    "method": "method_name",
    "params": {
      "key": "value"
    }
  }
}
```

**Fields**:
- `id`: Unique identifier for the request (used to match responses)
- `method`: Name of the method to invoke
- `params`: Method parameters (any JSON value)

**Example**:
```json
{
  "Request": {
    "id": "req-001",
    "method": "read_file",
    "params": {
      "path": "/tmp/test.txt"
    }
  }
}
```

#### 2. Response Message

Sent from host to guest in response to a request:

```json
{
  "Response": {
    "id": "req-001",
    "result": {
      "content": "file content here"
    },
    "error": null
  }
}
```

**Fields**:
- `id`: Request ID (matches the original request)
- `result`: Operation result (any JSON value, or null)
- `error`: Error message (string, or null if successful)

**Success Example**:
```json
{
  "Response": {
    "id": "req-001",
    "result": {
      "content": "Hello, World!"
    },
    "error": null
  }
}
```

**Error Example**:
```json
{
  "Response": {
    "id": "req-001",
    "result": null,
    "error": "File not found: /tmp/test.txt"
  }
}
```

#### 3. Notification Message

Sent from either host or guest without expecting a response:

```json
{
  "Notification": {
    "method": "event_name",
    "params": {
      "key": "value"
    }
  }
}
```

**Fields**:
- `method`: Event name
- `params": Event parameters (any JSON value)

**Example**:
```json
{
  "Notification": {
    "method": "log_message",
    "params": {
      "level": "info",
      "message": "VM started successfully"
    }
  }
}
```

## Rust Implementation

### Host Side

#### Creating a Listener

```rust
use ironclaw_orchestrator::vm::vsock::{VsockHostListener, VsockMessageHandler};

#[derive(Clone)]
struct MyHandler;

#[async_trait::async_trait]
impl VsockMessageHandler for MyHandler {
    async fn handle_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        match method {
            "read_file" => {
                let path = params["path"].as_str().unwrap();
                // Read file and return content
                Ok(json!({"content": "file content"}))
            }
            _ => anyhow::bail!("Unknown method: {}", method),
        }
    }

    async fn handle_notification(&self, method: &str, params: serde_json::Value) -> Result<()> {
        match method {
            "log_message" => {
                println!("Log: {}", params["message"]);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = VsockHostListener::new("vm-123".to_string()).await?;

    // Run handler loop
    listener.run_handler(MyHandler).await?;

    Ok(())
}
```

#### Accepting Single Connection

```rust
let listener = VsockHostListener::new("vm-123".to_string()).await?;
let conn = listener.accept().await?;

// Handle messages for this connection
conn.handle_messages(MyHandler).await?;
```

### Guest Side

#### Connecting to Host

```rust
use ironclaw_orchestrator::vm::vsock::{VsockClient, VsockMessage};

#[tokio::main]
async fn main() -> Result<()> {
    let client = VsockClient::new("/tmp/ironclaw/vsock/vm-123.sock".into());
    let mut conn = client.connect().await?;

    // Send request
    let result = conn.send_request("read_file", json!({"path": "/tmp/test.txt"})).await?;
    println!("Result: {}", result);

    // Send notification
    conn.send_notification("log_message", json!({"message": "Hello from VM"})).await?;

    Ok(())
}
```

## Standard Methods

### File Operations

#### read_file

Read a file from the host's filesystem.

**Request**:
```json
{
  "Request": {
    "id": "req-001",
    "method": "read_file",
    "params": {
      "path": "/tmp/test.txt"
    }
  }
}
```

**Response** (Success):
```json
{
  "Response": {
    "id": "req-001",
    "result": {
      "content": "file content here"
    },
    "error": null
  }
}
```

**Response** (Error):
```json
{
  "Response": {
    "id": "req-001",
    "result": null,
    "error": "File not found: /tmp/test.txt"
  }
}
```

#### write_file

Write a file to the host's filesystem (requires approval).

**Request**:
```json
{
  "Request": {
    "id": "req-002",
    "method": "write_file",
    "params": {
      "path": "/tmp/test.txt",
      "content": "new content"
    }
  }
}
```

**Response** (Success):
```json
{
  "Response": {
    "id": "req-002",
    "result": {
      "success": true
    },
    "error": null
  }
}
```

### System Operations

#### get_status

Get the status of the VM.

**Request**:
```json
{
  "Request": {
    "id": "req-003",
    "method": "get_status",
    "params": {}
  }
}
```

**Response**:
```json
{
  "Response": {
    "id": "req-003",
    "result": {
      "status": "running",
      "uptime_seconds": 123
    },
    "error": null
  }
}
```

#### shutdown

Shutdown the VM gracefully.

**Request**:
```json
{
  "Request": {
    "id": "req-004",
    "method": "shutdown",
    "params": {}
  }
}
```

**Response**:
```json
{
  "Response": {
    "id": "req-004",
    "result": {
      "success": true
    },
    "error": null
  }
}
```

## Standard Notifications

### log_message

Send a log message to the host.

**Notification**:
```json
{
  "Notification": {
    "method": "log_message",
    "params": {
      "level": "info",
      "message": "Operation completed successfully"
    }
  }
}
```

### heartbeat

Periodic heartbeat to keep the connection alive.

**Notification**:
```json
{
  "Notification": {
    "method": "heartbeat",
    "params": {
      "timestamp": 1234567890
    }
  }
}
```

## Error Handling

### Error Types

1. **Parse Error**: Invalid JSON or message format
2. **Method Not Found**: Unknown method requested
3. **Invalid Params**: Method parameters are invalid
4. **Internal Error**: Server-side error

### Error Response Format

All errors include a descriptive error message:

```json
{
  "Response": {
    "id": "req-001",
    "result": null,
    "error": "Descriptive error message here"
  }
}
```

### Error Codes (Future)

Future versions may include error codes:

```json
{
  "Response": {
    "id": "req-001",
    "result": null,
    "error": {
      "code": "FILE_NOT_FOUND",
      "message": "File not found: /tmp/test.txt"
    }
  }
}
```

## Security Considerations

### Message Size Limits

All messages are limited to 16 MB to prevent DoS attacks:

```rust
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
```

Messages exceeding this limit are rejected during deserialization.

### Input Validation

All method parameters should be validated:

```rust
async fn handle_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
    match method {
        "read_file" => {
            // Validate path parameter
            let path = params["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing path"))?;
            // Validate path is within allowed directory
            // ...
        }
        // ...
    }
}
```

### Authentication (Future)

Future versions may include authentication tokens:

```json
{
  "Request": {
    "id": "req-001",
    "auth": "token-here",
    "method": "read_file",
    "params": {
      "path": "/tmp/test.txt"
    }
  }
}
```

## Performance

### Benchmarks

- **Connection Setup**: ~1-2ms
- **Message Round-trip**: <1ms for small messages
- **Throughput**: >1GB/s for large messages
- **Concurrent Connections**: Supports multiple simultaneous connections

### Optimization Tips

1. **Reuse Connections**: Keep connections open for multiple requests
2. **Batch Operations**: Combine multiple operations into a single request
3. **Use Notifications**: Use notifications for one-way messages (no response)
4. **Async I/O**: Use async/await for concurrent operations

## Testing

### Unit Tests

```bash
cargo test --lib vm::vsock
```

### Integration Tests

```bash
cargo test --lib vm::tests::test_vsock_message_serialization
cargo test --lib vm::tests::test_vsock_message_size_limit
```

### Manual Testing

```bash
# Start host listener
nc -l /tmp/ironclaw/vsock/test.sock

# Connect from guest
nc -U /tmp/ironclaw/vsock/test.sock

# Send message (manual format)
echo -ne '\x00\x00\x00\x2C{"Request":{"id":"1","method":"test","params":{}}}' | nc -U /tmp/ironclaw/vsock/test.sock
```

## Troubleshooting

### Connection Refused

**Symptom**: "Failed to connect to vsock socket"

**Solution**:
```bash
# Check socket exists
ls -l /tmp/ironclaw/vsock/vm-123.sock

# Check permissions
stat /tmp/ironclaw/vsock/vm-123.sock

# Restart orchestrator
```

### Message Too Large

**Symptom**: "Message size exceeds maximum allowed size"

**Solution**:
```rust
// Split large messages into chunks
const CHUNK_SIZE: usize = 1024 * 1024; // 1MB

for chunk in data.chunks(CHUNK_SIZE) {
    conn.send_request("send_chunk", json!({"data": chunk})).await?;
}
```

### Deserialization Error

**Symptom**: "Failed to deserialize vsock message"

**Solution**:
```rust
// Validate JSON before sending
if let Err(e) = serde_json::to_value(&message) {
    anyhow::bail!("Invalid message format: {}", e);
}
```

## Future Enhancements

### Protocol v2 (Planned)

1. **Binary Protocol**: CBOR/MessagePack for better performance
2. **Streaming**: Support for streaming large payloads
3. **Compression**: Automatic compression for large messages
4. **Encryption**: TLS over Unix sockets for additional security
5. **Authentication**: Token-based authentication

### Backward Compatibility

Protocol v2 will maintain backward compatibility with v1 through version negotiation:

```json
{
  "Request": {
    "protocol_version": 2,
    "id": "req-001",
    "method": "read_file",
    "params": {
      "path": "/tmp/test.txt"
    }
  }
}
```

## References

- [Unix Domain Sockets](https://man7.org/linux/man-pages/man7/unix.7.html)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [Async/Await in Rust](https://rust-lang.github.io/async-book/)
- [Tokio Documentation](https://tokio.rs/)
