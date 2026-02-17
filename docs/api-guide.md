# LuminaGuard Daemon API Guide

## Overview

The LuminaGuard Daemon exposes a comprehensive REST API for managing agent execution, approvals, VMs, and MCP servers. All endpoints are documented in the OpenAPI 3.0 specification (see `docs/openapi.yaml`).

## Quick Start

### Starting the Daemon

```bash
luminaguard daemon --metrics-port 9090
```

The daemon will start on `localhost:8080` with:
- REST API at `http://localhost:8080/api/v1/`
- Metrics endpoint at `http://localhost:9090/metrics`
- Health check at `http://localhost:8080/health`

### Authentication

All endpoints require bearer token authentication:

```bash
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/tasks
```

## Core Concepts

### Tasks

A **task** represents a single agent execution. When you create a task:

1. A new JIT Micro-VM is spawned
2. The agent reasoning loop executes within the VM
3. High-risk actions trigger approval requests
4. Results and logs are captured
5. The VM is destroyed upon completion

**Task Lifecycle:**
```
pending → running → (approval) → completed/failed
```

### Approvals

The "Approval Cliff" requires explicit user approval for high-risk actions:

- **Green actions** (autonomous): Reading files, searching, checking logs
- **Red actions** (paused): Editing code, deleting files, sending emails, crypto transfers

When a red action is triggered, an approval request is created with:
- Description of the intended change
- Risk assessment (low, medium, high, critical)
- Detailed "diff card" showing what will change
- Expiration timeout (default 5 minutes)

### VMs

Each task spawns an ephemeral **Micro-VM** that:

- Is created on-demand in <100ms
- Provides full isolation via Firecracker
- Is destroyed immediately after task completion
- Cannot persist malware or changes

### MCP Servers

**Model Context Protocol** servers provide standardized tools and resources:

- Connect via stdio, HTTP, or SSE transport
- Provide tools (read_file, execute_command, etc.)
- List capabilities (tools, resources, sampling, prompts)
- Are isolated per VM or shared across VMs

## API Endpoints

### Health & Monitoring

#### Get Health Status

```http
GET /health
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-02-17T10:30:00Z",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "active_vms": 2
}
```

#### Get Prometheus Metrics

```http
GET /metrics
```

Returns metrics in Prometheus text format. Scrape interval: 15s recommended.

### Tasks

#### Create Task

```http
POST /api/v1/tasks
Content-Type: application/json

{
  "description": "Download and analyze the latest invoice",
  "timeout_seconds": 300,
  "require_approval": true,
  "metadata": {
    "invoice_date": "2025-02-17"
  }
}
```

**Response (201 Created):**
```json
{
  "id": "5f3a9c2b-1234-5678-abcd-ef1234567890",
  "status": "running",
  "created_at": "2025-02-17T10:30:00Z",
  "started_at": "2025-02-17T10:30:01Z",
  "vm_id": "vm-5f3a9c2b"
}
```

#### List Tasks

```http
GET /api/v1/tasks?status=completed&limit=20&offset=0
```

**Query Parameters:**
- `status` - Filter by: pending, running, completed, failed
- `limit` - Results per page (default 20, max 100)
- `offset` - Pagination offset (default 0)

**Response:**
```json
{
  "tasks": [
    {
      "id": "5f3a9c2b-1234-5678-abcd-ef1234567890",
      "status": "completed",
      "created_at": "2025-02-17T10:30:00Z",
      "completed_at": "2025-02-17T10:35:00Z",
      "result": {
        "output": "Invoice downloaded and analyzed",
        "exit_code": 0
      }
    }
  ],
  "total": 42,
  "limit": 20,
  "offset": 0
}
```

#### Get Task Details

```http
GET /api/v1/tasks/{taskId}
```

#### Cancel Task

```http
DELETE /api/v1/tasks/{taskId}
```

Returns the task with status updated to "cancelled".

### Approvals

#### List Pending Approvals

```http
GET /api/v1/approvals?status=pending
```

**Response:**
```json
[
  {
    "id": "a1b2c3d4-5678-9abc-def0-123456789abc",
    "task_id": "5f3a9c2b-1234-5678-abcd-ef1234567890",
    "action_type": "FileDelete",
    "risk_level": "high",
    "description": "Delete sensitive.key (2.5KB)",
    "changes": [
      {
        "type": "FileDelete",
        "path": "config/sensitive.key",
        "size_bytes": 2560
      }
    ],
    "status": "pending",
    "created_at": "2025-02-17T10:30:05Z",
    "expires_at": "2025-02-17T10:35:05Z"
  }
]
```

#### Get Approval Details

```http
GET /api/v1/approvals/{approvalId}
```

#### Respond to Approval

```http
POST /api/v1/approvals/{approvalId}
Content-Type: application/json

{
  "approved": true,
  "reason": "Looks safe, proceed"
}
```

**Response:**
```json
{
  "id": "a1b2c3d4-5678-9abc-def0-123456789abc",
  "status": "approved",
  "responded_at": "2025-02-17T10:30:30Z",
  ...
}
```

### Virtual Machines

#### List Active VMs

```http
GET /api/v1/vms
```

**Response:**
```json
[
  {
    "id": "vm-5f3a9c2b",
    "status": "running",
    "type": "standard",
    "created_at": "2025-02-17T10:30:01Z",
    "resource_usage": {
      "memory_mb": 256,
      "cpu_percent": 45.2,
      "disk_mb": 512
    },
    "spawn_time_ms": 87.5
  }
]
```

#### Get VM Details

```http
GET /api/v1/vms/{vmId}
```

#### Destroy VM

```http
DELETE /api/v1/vms/{vmId}
```

Force-terminate a VM and clean up resources.

### MCP Servers

#### List MCP Servers

```http
GET /api/v1/mcp
```

**Response:**
```json
[
  {
    "id": "mcp-filesystem",
    "name": "filesystem",
    "status": "connected",
    "version": "1.0.0",
    "capabilities": ["tools", "resources"]
  },
  {
    "id": "mcp-github",
    "name": "github",
    "status": "connected",
    "version": "0.5.0",
    "capabilities": ["tools", "resources", "prompts"]
  }
]
```

#### List Tools from MCP Server

```http
GET /api/v1/mcp/{serverId}/tools
```

**Response:**
```json
[
  {
    "name": "read_file",
    "description": "Read the contents of a file",
    "input_schema": {
      "type": "object",
      "properties": {
        "path": {
          "type": "string",
          "description": "Path to file to read"
        }
      },
      "required": ["path"]
    },
    "category": "filesystem"
  }
]
```

## Error Handling

All errors follow RFC 7807 Problem Details specification:

```json
{
  "type": "https://api.luminaguard.io/errors/task-not-found",
  "title": "Task Not Found",
  "status": 404,
  "detail": "Task 5f3a9c2b-1234-5678-abcd-ef1234567890 does not exist",
  "instance": "/api/v1/tasks/5f3a9c2b-1234-5678-abcd-ef1234567890",
  "timestamp": "2025-02-17T10:30:00Z"
}
```

### Common Status Codes

- **200 OK** - Success
- **201 Created** - Resource created successfully
- **400 Bad Request** - Invalid request parameters
- **401 Unauthorized** - Missing or invalid authentication
- **403 Forbidden** - Permission denied
- **404 Not Found** - Resource not found
- **409 Conflict** - Request cannot be processed due to conflict
- **500 Internal Server Error** - Server error
- **503 Service Unavailable** - Server temporarily unavailable

## Rate Limiting

API endpoints are rate-limited to prevent abuse:

- **Default**: 1000 requests per minute per API token
- **Burst**: Up to 100 requests per second
- **Retry-After**: Returned when limit exceeded

Headers:
- `X-RateLimit-Limit`: Request limit
- `X-RateLimit-Remaining`: Requests remaining
- `X-RateLimit-Reset`: Unix timestamp when limit resets

## Examples

### Example 1: Run a Task with Approval

```bash
#!/bin/bash
TOKEN="your-api-token"
BASE_URL="http://localhost:8080"

# 1. Create task
TASK=$(curl -s -X POST "$BASE_URL/api/v1/tasks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Delete old logs"
  }')

TASK_ID=$(echo $TASK | jq -r '.id')
echo "Task created: $TASK_ID"

# 2. Wait for approval request
sleep 1
APPROVALS=$(curl -s "$BASE_URL/api/v1/approvals?status=pending" \
  -H "Authorization: Bearer $TOKEN")

APPROVAL_ID=$(echo $APPROVALS | jq -r '.[0].id')
echo "Approval required: $APPROVAL_ID"

# 3. Review and approve
curl -s -X POST "$BASE_URL/api/v1/approvals/$APPROVAL_ID" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "approved": true,
    "reason": "Looks good"
  }' | jq '.'

# 4. Check task completion
curl -s "$BASE_URL/api/v1/tasks/$TASK_ID" \
  -H "Authorization: Bearer $TOKEN" | jq '.'
```

### Example 2: Monitor with Prometheus

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'luminaguard'
    static_configs:
      - targets: ['localhost:9090']
```

Then query metrics:

```bash
curl http://localhost:9090/metrics | grep vm_spawn_time_seconds
```

## SDK Usage

### Python

```python
from luminaguard_client import LuminaGuardClient

client = LuminaGuardClient(
    base_url="http://localhost:8080",
    api_token="your-token"
)

# Create task
task = client.tasks.create(
    description="Download invoice",
    timeout_seconds=300
)

# Wait for completion
task = client.tasks.wait(task.id)
print(f"Task completed with status: {task.status}")
```

### Rust

```rust
use luminaguard_client::Client;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new(
        "http://localhost:8080".parse()?,
        "your-token"
    );
    
    let task = client.tasks().create("Download invoice").await?;
    let result = client.tasks().get(&task.id).await?;
    Ok(())
}
```

## Best Practices

1. **Use Bearer Tokens** - Always use secure, rotating tokens
2. **Handle Rate Limits** - Implement exponential backoff
3. **Monitor Metrics** - Set up Prometheus scraping
4. **Set Timeouts** - Always specify task timeout_seconds
5. **Validate Approvals** - Review approval details before granting
6. **Clean Up Resources** - Cancel stale tasks and destroy VMs
7. **Log Requests** - Track API usage for debugging

## Troubleshooting

### 401 Unauthorized

Check your API token:
```bash
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/health
```

### 503 Service Unavailable

Daemon may be starting up. Check logs:
```bash
journalctl -u luminaguard -f
```

### High Latency

Check active VMs and metrics:
```bash
curl http://localhost:8080/api/v1/vms | jq '.[] | .resource_usage'
```

## Support

For issues or questions:
- GitHub Issues: https://github.com/anchapin/luminaguard/issues
- Documentation: https://github.com/anchapin/luminaguard/tree/main/docs
