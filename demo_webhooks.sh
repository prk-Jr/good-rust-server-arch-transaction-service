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

# Clean up any existing processes
pkill -f "payments-app" || true
rm -f /tmp/demo_webhooks.db

print_step "Starting Payment Server (SQLite)..."
export DATABASE_URL="sqlite:///tmp/demo_webhooks.db?mode=rwc"
export PORT=3040
export RUST_LOG=error

# Start server in background
cargo run -q -p payments-app --no-default-features --features sqlite &
SERVER_PID=$!

# Ensure server stops on script exit
trap "kill $SERVER_PID" EXIT

print_step "Waiting for server to be ready..."
sleep 5

# Bootstrap API Key
print_step "Bootstrapping API Key (via CLI)..."
export PAYMENTS_API_URL="http://localhost:3040"
API_KEY=$(cargo run -q -p payments-cli -- bootstrap --name "demo-key")

if [ -z "$API_KEY" ]; then
    print_error "Failed to bootstrap key."
    exit 1
fi

export PAYMENTS_API_KEY="$API_KEY"
print_success "API Key obtained: ${API_KEY:0:10}..."

# Start Webhook Listener (Netcat)
WEBHOOK_PORT=3041
print_step "Starting Webhook Listener on port $WEBHOOK_PORT..."
# This will listen for one request, print it, and exit
# We run it in a loop to capture multiple events if needed, but for now just one
(
    while true; do 
        echo -e "HTTP/1.1 200 OK\r\n\r\n" | nc -l localhost $WEBHOOK_PORT | tee -a webhook_received.log
    done
) &
LISTENER_PID=$!
trap "kill $SERVER_PID $LISTENER_PID 2>/dev/null" EXIT

# Register Webhook
print_step "Registering Webhook..."
cargo run -q -p payments-cli -- webhook register \
    --url "http://localhost:$WEBHOOK_PORT/webhook" \
    --events "deposit.success"

# List Webhooks
print_step "Listing Webhooks..."
cargo run -q -p payments-cli -- webhook list

# Trigger an Event
print_step "Triggering Event: Creating Account & Deposit..."
# Create Account
ACCOUNT_JSON=$(cargo run -q -p payments-cli -- account create "Demo Corp" --currency USD)
# Extract ID more robustly (handle potential whitespace/newlines)
ACCOUNT_ID=$(echo "$ACCOUNT_JSON" | grep -o '"id": *"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACCOUNT_ID" ]; then
    print_error "Failed to create account. output: $ACCOUNT_JSON"
    exit 1
fi
print_success "Created Account: $ACCOUNT_ID"

# Deposit (Triggers transaction.created)
print_step "Depositing funds (Wait for webhook!)..."
sleep 2 # Give listener a moment
cargo run -q -p payments-cli -- transaction deposit \
    --account "$ACCOUNT_ID" \
    --amount 1000 \
    --currency USD

print_step "Check the listener output above for the webhook payload!"
sleep 2 # Allow webhook to arrive

print_step "DEBUG: Dumping webhook_events table..."
sqlite3 /tmp/demo_webhooks.db "SELECT * FROM webhook_events;" || true

print_step "Demo Complete! 🎉"

