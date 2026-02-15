# HTTP Transport Integration Tests

The HTTP transport integration tests are located in:
- `orchestrator/src/mcp/http_transport.rs`

## Running Tests

```bash
cd orchestrator
cargo test --lib mcp::http_transport
```

## Known Test Coverage

- test_http_transport_creation
- test_http_transport_with_timeout
- test_http_transport_url_getter
- test_http_transport_load_balancing_creation
- test_http_transport_with_retry_enabled
- test_http_transport_with_custom_retry_config
- test_http_transport_with_custom_headers
- test_http_transport_send_when_disconnected
- test_http_transport_recv_when_disconnected
- test_http_transport_disconnect

