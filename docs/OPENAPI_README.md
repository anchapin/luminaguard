# OpenAPI Documentation

This directory contains the OpenAPI 3.0 specification for the LuminaGuard Daemon API.

## Files

- **openapi.yaml** - Complete OpenAPI 3.0 specification
- **api-guide.md** - User-friendly API guide with examples

## Using the OpenAPI Specification

### Swagger UI

View interactive documentation:

```bash
# Option 1: Use online Swagger Editor
# Visit: https://editor.swagger.io/
# Click "File" > "Import URL"
# Paste: https://raw.githubusercontent.com/anchapin/luminaguard/main/docs/openapi.yaml

# Option 2: Run Swagger UI locally
docker run -p 8888:8080 \
  -e SWAGGER_JSON=/spec/openapi.yaml \
  -v $(pwd)/docs:/spec \
  swaggerapi/swagger-ui
```

Then visit `http://localhost:8888`

### ReDoc

Alternative interactive documentation:

```bash
docker run -p 8080:8080 \
  -v $(pwd)/docs:/spec \
  redoc \
  sh -c 'exec npx redoc-cli serve /spec/openapi.yaml --host 0.0.0.0'
```

### Generate Client SDKs

Generate language-specific client libraries:

```bash
# Install OpenAPI Generator
npm install -g @openapitools/openapi-generator-cli

# Generate Python client
openapi-generator-cli generate \
  -i docs/openapi.yaml \
  -g python \
  -o ./sdks/python

# Generate Rust client
openapi-generator-cli generate \
  -i docs/openapi.yaml \
  -g rust \
  -o ./sdks/rust

# Generate TypeScript client
openapi-generator-cli generate \
  -i docs/openapi.yaml \
  -g typescript-fetch \
  -o ./sdks/typescript
```

### Validate Specification

```bash
# Install validator
npm install -g swagger-cli

# Validate
swagger-cli validate docs/openapi.yaml
```

### Generate API Documentation

Convert OpenAPI to static HTML:

```bash
# Using Redoc
npx redoc-cli bundle docs/openapi.yaml -o docs/api-docs.html

# Using OpenAPI spec
npx @openapitools/openapi-generator-cli generate \
  -i docs/openapi.yaml \
  -g html \
  -o ./docs/html
```

## Integration with Daemon

The daemon serves the OpenAPI spec at:

```
GET /api/v1/openapi.yaml
```

Enable in daemon configuration:

```yaml
api:
  expose_openapi_spec: true
  openapi_endpoint: /api/v1/openapi.yaml
```

## API Design Principles

This API follows these principles:

1. **RESTful Design** - Standard HTTP methods and status codes
2. **Resource-Based** - URLs represent resources, not actions
3. **Versioning** - `/api/v1/` for API version 1
4. **Problem Details** - RFC 7807 for error responses
5. **Pagination** - Consistent limit/offset pagination
6. **Filtering** - Query parameters for filtering
7. **Rate Limiting** - Standard rate limit headers
8. **Authentication** - Bearer token in Authorization header
9. **Security** - All endpoints require HTTPS in production
10. **Documentation** - Complete OpenAPI specification

## Schema Objects

### Request/Response Flow

```
Client Request
    ↓
OpenAPI Validation (via middleware)
    ↓
Handler Processing
    ↓
Response Serialization (via OpenAPI schema)
    ↓
Client Response
```

### Common Objects

- **TaskRequest** - Input for creating a task
- **TaskResponse** - Output from task operations
- **ApprovalRequest** - Approval workflow object
- **ErrorResponse** - RFC 7807 Problem Details

## Extending the API

When adding new endpoints:

1. Add to `openapi.yaml` under `paths:`
2. Define request/response schemas in `components/schemas:`
3. Document in `api-guide.md`
4. Implement handler in Rust code
5. Regenerate client SDKs if published
6. Update tests to validate against schema

### Example: Add New Endpoint

```yaml
/api/v1/webhooks:
  post:
    summary: Register webhook
    tags:
      - Webhooks
    requestBody:
      required: true
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/WebhookRequest'
    responses:
      '201':
        description: Webhook registered
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/WebhookResponse'
```

## Testing

### Integration Tests

```bash
# Run API tests against OpenAPI spec
cargo test --test api_integration -- --nocapture

# Validate all responses against schema
cargo test --test api_schema_validation
```

### Manual Testing

```bash
# Using curl
curl -X GET http://localhost:8080/api/v1/tasks \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  | jq '.'

# Using httpie
http GET http://localhost:8080/api/v1/tasks \
  Authorization:"Bearer $TOKEN"

# Using Postman
# Import: docs/openapi.yaml
# Set environment variable: {{base_url}} = http://localhost:8080
# Set environment variable: {{token}} = your-api-token
```

## Versioning

This API uses semantic versioning in the path:

- `/api/v1/` - Current stable version
- `/api/v2/` - Future breaking changes
- `/api/experimental/` - Unstable endpoints

Stable endpoints (v1) guarantee:
- No breaking changes to request/response schemas
- No removal of endpoints
- New fields are always optional
- New endpoints added under v1 path

## Support & Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for:
- How to propose API changes
- How to contribute to documentation
- Code review process

## Resources

- [OpenAPI 3.0 Specification](https://spec.openapis.org/oas/v3.0.0)
- [REST API Best Practices](https://restfulapi.net/)
- [Problem Details RFC 7807](https://tools.ietf.org/html/rfc7807)
- [HTTP Status Codes](https://httpwg.org/specs/rfc9110.html)
