#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

print_step() { echo -e "\n${BLUE}👉 $1${NC}"; }
print_success() { echo -e "${GREEN}✅ $1${NC}"; }
print_error() { echo -e "${RED}❌ $1${NC}"; }

# Clean up
pkill -f "payments-app" || true
rm -f /tmp/test_webhooks.db
rm -f webhook_all.log

print_step "Starting Payment Server (SQLite)..."
export DATABASE_URL="sqlite:///tmp/test_webhooks.db?mode=rwc"
export PORT=3045  # Different port to avoid conflicts
export RUST_LOG=error

# Start server
cargo run -q -p payments-app --no-default-features --features sqlite &
SERVER_PID=$!
trap "kill $SERVER_PID 2>/dev/null" EXIT

print_step "Waiting for server..."
sleep 5

# Bootstrap
print_step "Bootstrapping..."
export PAYMENTS_API_URL="http://localhost:3045"
API_KEY=$(cargo run -q -p payments-cli -- bootstrap --name "test-key")

if [ -z "$API_KEY" ]; then
    print_error "Failed to bootstrap."
    exit 1
fi
export PAYMENTS_API_KEY="$API_KEY"
print_success "API Key: ${API_KEY:0:10}..."

# Start Listener
LISTENER_PORT=3046
print_step "Starting Listener on $LISTENER_PORT..."
# Ensure any previous listener is dead
pkill -f "payments-cli.*webhook listen" || true

cargo run -q -p payments-cli -- webhook listen --port $LISTENER_PORT > webhook_all.log 2>&1 &
LISTENER_PID=$!

# Wait for listener to bind
echo -n "Waiting for listener..."
for i in {1..30}; do
    if nc -z localhost $LISTENER_PORT; then
        echo " Ready!"
        break
    fi
    echo -n "."
    sleep 0.1
done
sleep 1 # Extra buffer
trap "kill $SERVER_PID $LISTENER_PID 2>/dev/null" EXIT

# Register Webhook
print_step "Registering Webhook for ALL events..."
cargo run -q -p payments-cli -- webhook register \
    --url "http://localhost:$LISTENER_PORT/webhook" \
    --events "deposit.success,withdraw.success,transfer.success"

# 1. Create Accounts
print_step "Creating Accounts..."
ACCT_A=$(cargo run -q -p payments-cli -- account create "Alice" --currency USD | grep -o '"id": *"[^"]*"' | cut -d'"' -f4 | head -n1)
ACCT_B=$(cargo run -q -p payments-cli -- account create "Bob" --currency USD | grep -o '"id": *"[^"]*"' | cut -d'"' -f4 | head -n1)

print_success "Alice: $ACCT_A"
print_success "Bob: $ACCT_B"

# 2. Deposit (Triggers deposit.success)
print_step "Testing DEPOSIT..."
cargo run -q -p payments-cli -- transaction deposit --account "$ACCT_A" --amount 1000 --currency USD
sleep 2

# 3. Withdraw (Triggers withdraw.success)
print_step "Testing WITHDRAW..."
cargo run -q -p payments-cli -- transaction withdraw --account "$ACCT_A" --amount 200 --currency USD
sleep 2

# 4. Transfer (Triggers transfer.success)
print_step "Testing TRANSFER..."
cargo run -q -p payments-cli -- transaction transfer --from "$ACCT_A" --to "$ACCT_B" --amount 300 --currency USD
sleep 2

# Verify Log
print_step "Verifying Log..."
echo "----------------------------------------"
cat webhook_all.log
echo "----------------------------------------"

if grep -q "deposit.success" webhook_all.log && \
   grep -q "withdraw.success" webhook_all.log && \
   grep -q "transfer.success" webhook_all.log; then
    print_success "All 3 events received! 🎉"
else
    print_error "Missing events in log!"
    exit 1
fi
