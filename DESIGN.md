# Payment Transaction Service - Design Specification

## Overview

This document describes the design of a production-ready payment transaction service built in Rust. The service handles deposits, withdrawals, and transfers between accounts with atomic guarantees and webhook notifications.

## Architecture

### Hexagonal Architecture (Ports & Adapters)

```
┌─────────────────────────────────────────────────────────────────┐
│                         payments-app                            │
│                      (Server Entry Point)                       │
├─────────────────────────────────────────────────────────────────┤
│                         payments-hex                            │
│              ┌──────────────────────────────────┐              │
│              │       PaymentService             │              │
│              │    (Application Core)            │              │
│              └──────────────────────────────────┘              │
│                            │                                    │
│     ┌──────────────────────┼──────────────────────┐            │
│     │                      │                      │            │
│  HttpServer            auth_middleware      handlers            │
│  (Inbound)              (Security)        (REST API)           │
├─────────────────────────────────────────────────────────────────┤
│                        payments-repo                            │
│     ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│     │  PostgresRepo │  │  SqliteRepo  │  │ WebhookWorker│      │
│     │  (Outbound)   │  │  (Outbound)  │  │  (Outbound)  │      │
│     └──────────────┘  └──────────────┘  └──────────────┘      │
├─────────────────────────────────────────────────────────────────┤
│                        payments-types                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Domain Types │ Port Traits │ DTOs │ Errors            │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

| Crate | Responsibility |
|-------|----------------|
| `payments-types` | Domain models, port traits, DTOs, error types |
| `payments-repo` | Database adapters (Postgres/SQLite), webhook worker, security utilities |
| `payments-hex` | Application service, HTTP handlers, authentication middleware |
| `payments-app` | Server bootstrap, configuration, main entry point |
| `payments-client` | Typed Rust SDK for API consumers |
| `payments-cli` | Command-line interface |

## Database Schema

### Accounts Table

```sql
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    balance BIGINT NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    created_at TIMESTAMPTZ NOT NULL
);
```

### Transactions Table

```sql
CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    direction TEXT NOT NULL,        -- 'DEPOSIT', 'WITHDRAWAL', 'TRANSFER'
    amount BIGINT NOT NULL,
    currency TEXT NOT NULL,
    source_account_id UUID,         -- NULL for deposits
    destination_account_id UUID,    -- NULL for withdrawals
    idempotency_key TEXT UNIQUE,    -- For duplicate detection
    reference TEXT,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_transactions_source ON transactions(source_account_id);
CREATE INDEX idx_transactions_dest ON transactions(destination_account_id);
CREATE INDEX idx_transactions_idempotency ON transactions(idempotency_key);
```

### API Keys Table

```sql
CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,  -- SHA-256 hash
    account_id UUID REFERENCES accounts(id),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
```

### Webhook Events Table

```sql
CREATE TABLE webhook_events (
    id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);

CREATE INDEX idx_webhook_status ON webhook_events(status, created_at);
```

## API Design

### Authentication

All API endpoints except `/health` and `POST /api/bootstrap` require authentication via Bearer token:

```
Authorization: Bearer sk_xxxxxxxxxxxxxxxxxxxxx
```

**Bootstrap Flow:**
1. On first run, call `POST /api/bootstrap` to create the first API key
2. This endpoint only works when zero API keys exist in the system
3. The raw API key is returned once and cannot be retrieved again

### Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/health` | No | Health check |
| `POST` | `/api/bootstrap` | No* | Create first API key |
| `POST` | `/api/accounts` | Yes | Create account |
| `GET` | `/api/accounts` | Yes | List accounts |
| `GET` | `/api/accounts/{id}` | Yes | Get account |
| `GET` | `/api/accounts/{id}/transactions` | Yes | List transactions |
| `POST` | `/api/transactions/deposit` | Yes | Deposit funds |
| `POST` | `/api/transactions/withdraw` | Yes | Withdraw funds |
| `POST` | `/api/transactions/transfer` | Yes | Transfer funds |

*Only works when no API keys exist

### Money Representation

All monetary amounts are stored and transmitted as **integers in the smallest currency unit** (cents for USD):

```json
{
  "amount": 10000,    // $100.00
  "currency": "USD"
}
```

This avoids floating-point precision issues common in financial systems.

### Idempotency

Transaction endpoints support idempotency keys to prevent duplicate operations:

```json
{
  "account_id": "...",
  "amount": 5000,
  "currency": "USD",
  "idempotency_key": "txn-abc-123"
}
```

If the same `idempotency_key` is sent again, the original transaction is returned without creating a duplicate.

## Security

### API Key Storage

- Raw API keys are prefixed with `sk_` for identification
- Keys are hashed using SHA-256 before storage
- Only the hash is stored; raw keys cannot be recovered
- Verification uses constant-time comparison to prevent timing attacks

### Webhook Signing

Outgoing webhooks are signed using HMAC-SHA256:

```
X-Webhook-Signature: <hex-encoded-signature>
X-Webhook-Event-Id: <uuid>
X-Webhook-Event-Type: <type>
```

Receivers should verify:
```
expected = HMAC-SHA256(secret, request_body)
actual = header["X-Webhook-Signature"]
if constant_time_compare(expected, actual) { accept }
```

## Operational Considerations

### Database Selection

| Feature | PostgreSQL | SQLite |
|---------|------------|--------|
| Use case | Production | Development/Testing |
| Concurrency | High | Limited |
| SKIP LOCKED | ✓ | ✗ |
| JSONB | ✓ | JSON as TEXT |

### Atomicity

All balance-modifying operations use database transactions:
- Deposits: atomic balance increment
- Withdrawals: atomic balance check + decrement
- Transfers: atomic balance decrement (source) + increment (destination)

### Error Handling

| HTTP Status | Meaning |
|-------------|---------|
| 400 | Bad request (validation failure, insufficient funds) |
| 401 | Unauthorized (missing/invalid API key) |
| 404 | Resource not found |
| 500 | Internal server error |

## Observability

### Distributed Tracing

The service integrates OpenTelemetry for distributed tracing, exporting spans via OTLP/gRPC.

**Instrumented Operations:**
- `create_account` (fields: `owner`)
- `get_account` / `list_accounts` (fields: `account_id`)
- `deposit` / `withdraw` (fields: `account_id`, `amount`)
- `transfer` (fields: `from`, `to`, `amount`)
- `register_webhook` / `list_webhooks` (fields: `url`)
- `bootstrap` (fields: `key_name`)

### HTTP Metrics

HTTP server metrics are exported via OTLP/HTTP using `axum-otel-metrics`:

| Metric | Type | Description |
|--------|------|-------------|
| `http.server.active_requests` | Gauge | Current active HTTP requests |
| `http.server.request.duration` | Histogram | Request latency distribution |
| `http.server.request.body.size` | Histogram | Request body size |
| `http.server.response.body.size` | Histogram | Response body size |

### Configuration:

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector URL (traces via gRPC) | `http://localhost:4317` |
| `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | Collector URL (metrics via HTTP) | `http://localhost:4318` |
| `OTEL_SERVICE_NAME` | Service identifier | `payments-service` |

**Viewing Traces:**

`docker compose up -d` starts Jaeger at [http://localhost:16686](http://localhost:16686).

## Trade-offs & Decisions

### 1. Integer Money vs Decimal

**Decision:** Use `i64` cents instead of decimal types.

**Rationale:**
- Avoids floating-point precision issues
- Faster arithmetic operations
- No external decimal library needed
- Sufficient for amounts up to $92 quadrillion

### 2. Synchronous vs Async Webhooks

**Decision:** Async with retry queue.

**Rationale:**
- Transaction commits immediately (better UX)
- Webhook failures don't block transactions
- Automatic retry with exponential backoff

### 3. API Key vs JWT

**Decision:** Simple API keys with database verification.

**Rationale:**
- Simpler implementation
- Immediate revocation capability
- No token expiration complexity
- Suitable for server-to-server communication

### 4. Feature Flags for Databases

**Decision:** Compile-time feature flags (`postgres`/`sqlite`).

**Rationale:**
- No runtime overhead
- Binary only includes needed code
- Clear separation of database-specific logic

### 5. Bootstrap Endpoint

**Decision:** Special endpoint for first API key creation.

**Rationale:**
- Avoids hardcoded credentials
- Self-service initial setup
- Protected by "zero keys" check

## Future Enhancements

- [x] Rate limiting middleware
- [x] OpenTelemetry tracing integration
- [x] OpenAPI/Swagger documentation
- [ ] Multi-currency exchange rates
- [ ] Account statements/exports
- [ ] Audit logging
- [ ] Key rotation support

## API Documentation

Interactive API documentation is provided via **OpenAPI 3.0** specification with **Swagger UI**.

### Endpoints

- **Swagger UI**: `/swagger-ui` - Interactive API explorer with "Try it out" functionality
- **OpenAPI JSON**: `/api-docs/openapi.json` - Raw OpenAPI 3.0 specification

### Features

| Feature | Description |
|---------|-------------|
| **Interactive Testing** | Click "Try it out" on any endpoint to test directly |
| **Authentication** | Click "Authorize" button to set Bearer token for protected endpoints |
| **Request Schemas** | Full request body schemas with examples |
| **Response Schemas** | Documented response formats for all status codes |
| **Parameter Docs** | Path, query, and body parameter descriptions |

### Implementation

OpenAPI documentation is generated using `utoipa` crate with SwaggerUI integration:

```rust
// payments-hex/src/openapi.rs
#[derive(OpenApi)]
#[openapi(
    paths(health, bootstrap, create_account, ...),
    components(schemas(AccountResponse, DepositRequest, ...)),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
```

The spec is served at runtime without requiring a separate OpenAPI YAML file.
