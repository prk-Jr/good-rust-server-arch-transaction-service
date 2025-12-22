# Payments Transaction Service

A production-ready payment transaction service built with Rust, implementing hexagonal architecture with PostgreSQL and SQLite backends.

## ✨ Features

- **Account Management** - Create, read, and list accounts with multi-currency support
- **Transactions** - Deposits, withdrawals, and transfers with atomic guarantees
- **API Key Authentication** - Secure API access with hashed keys
- **Webhook Events** - HMAC-SHA256 signed webhook notifications
- **Webhook Registration** - REST API for registering webhook endpoints
- **Rate Limiting** - Per-API-key throttling (100 req/min)
- **Idempotency** - Prevent duplicate transactions with idempotency keys
- **Multiple Backends** - PostgreSQL (production) and SQLite (testing/development)

## 🏗️ Architecture

```
payments/
├── payments-types/    # Domain types, DTOs, port traits
├── payments-repo/     # Repository adapters (Postgres, SQLite)
├── payments-hex/      # Application service & HTTP handlers
├── payments-app/      # Server entry point
├── payments-client/   # Typed Rust SDK
└── payments-cli/      # Command-line interface
```

See [DESIGN.md](./DESIGN.md) for detailed architecture documentation.

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+
- Docker & Docker Compose (for PostgreSQL)

### Running with PostgreSQL

```bash
# Start database
docker compose up -d

# Run server
cargo run -p payments-app
```

### Running with SQLite

```bash
export DATABASE_URL="sqlite://payments.db?mode=rwc"
cargo run -p payments-app --no-default-features --features sqlite
```

## 🛠️ CLI Usage

The project includes a robust CLI tool `payments-cli` for interacting with the API.

### 1. Build the CLI
```bash
cargo build -p payments-cli
alias payments="cargo run -q -p payments-cli --"
```

### 2. Bootstrap (First Run)
Create your initial API key:
```bash
payments bootstrap --name "my-key"
# Output: sk_xxxxxxxxxxxxxxxxxxxxx
export PAYMENTS_API_KEY="sk_xxxxxxxxxxxxxxxxxxxxx"
```

### 3. Manage Accounts
```bash
# Create Account
payments account create "Alice" --currency USD

# List Accounts
payments account list

# Get Balance
payments account get --id <ACCOUNT_ID>
```

### 4. Transactions
```bash
# Deposit
payments transaction deposit --account <ID> --amount 1000 --currency USD

# Transfer
payments transaction transfer --from <ID> --to <ID> --amount 500

# Withdraw
payments transaction withdraw --account <ID> --amount 200
```

### 5. Webhooks
```bash
# Register a webhook
payments webhook register --url "http://localhost:3000/hook" --events "deposit.success,transfer.success"

# Start a local listener (for testing)
payments webhook listen --port 3000
```

## 🔐 Authentication

All API endpoints (except `/health` and `/api/bootstrap`) require authentication.

### Getting Your First API Key

```bash
# Bootstrap creates the first API key (only works when no keys exist)
curl -X POST http://localhost:3000/api/bootstrap \
  -H "Content-Type: application/json" \
  -d '{"name": "my-first-key"}'
```

Response:
```json
{
  "api_key": "sk_ABC123...",
  "message": "First API key created. Save this key securely - it won't be shown again!"
}
```

### Using Your API Key

```bash
curl http://localhost:3000/api/accounts \
  -H "Authorization: Bearer sk_ABC123..."
```

## 📡 API Reference

### Health Check

```
GET /health
```

No authentication required.

### Accounts

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/accounts` | Create account |
| `GET` | `/api/accounts` | List accounts |
| `GET` | `/api/accounts/{id}` | Get account |
| `GET` | `/api/accounts/{id}/transactions` | List account transactions |

**Create Account**
```bash
curl -X POST http://localhost:3000/api/accounts \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "currency": "USD"}'
```

### Transactions

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/transactions/deposit` | Deposit funds |
| `POST` | `/api/transactions/withdraw` | Withdraw funds |
| `POST` | `/api/transactions/transfer` | Transfer between accounts |

**Deposit**
```bash
curl -X POST http://localhost:3000/api/transactions/deposit \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": "uuid-here",
    "amount": 10000,
    "currency": "USD",
    "idempotency_key": "unique-key-123"
  }'
```

**Transfer**
```bash
curl -X POST http://localhost:3000/api/transactions/transfer \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "from_account_id": "source-uuid",
    "to_account_id": "dest-uuid",
    "amount": 5000,
    "currency": "USD"
  }'
```

### Webhooks

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/webhooks` | Register webhook endpoint |
| `GET` | `/api/webhooks` | List webhook endpoints |

**Register Webhook**
```bash
curl -X POST http://localhost:3000/api/webhooks \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://your-service.com/webhook",
    "events": ["transaction.created"]
  }'
```

Response includes a `secret` for verifying webhook signatures.

### Rate Limiting

API requests are rate limited to **100 requests per minute** per API key.

Exceeding the limit returns:
```json
{
  "error": "Rate limit exceeded. Please try again later.",
  "retry_after_seconds": 60
}
```

## 🔧 CLI Usage

```bash
# Set API key
export PAYMENTS_API_KEY="sk_..."
export PAYMENTS_API_URL="http://localhost:3000"

# Health check
cargo run -p payments-cli -- health

# Create account
cargo run -p payments-cli -- account create "Alice Corp" --currency USD

# Deposit
cargo run -p payments-cli -- transaction deposit \
  --account <ACCOUNT_ID> --amount 10000 --currency USD

# Transfer
cargo run -p payments-cli -- transaction transfer \
  --from <FROM_ID> --to <TO_ID> --amount 5000 --currency USD
```

## 🧪 Testing

```bash
# Run all tests
./validate_all.sh

# Unit tests only
cargo test --workspace --features sqlite

# Integration tests (SQLite)
./scripts/sqlite/test_e2e.sh

# Integration tests (PostgreSQL via Docker)
./scripts/postgres/test_e2e.sh
```

## 📦 Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `3000` |
| `DATABASE_URL` | Database connection string | Required |
| `RUST_LOG` | Log level | `info` |
| `PAYMENTS_API_KEY` | API key (for CLI) | - |
| `WEBHOOK_SECRET` | HMAC secret for webhook signing | - |
| `WEBHOOK_URL` | Webhook endpoint URL | - |

## 📄 License

MIT
