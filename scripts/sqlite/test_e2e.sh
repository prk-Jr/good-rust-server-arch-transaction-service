#!/usr/bin/env bash
# SQLite E2E Tests Wrapper

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

TEMP_DIR=$(mktemp -d)
DB_PATH="$TEMP_DIR/e2e.db"
export DATABASE_URL="sqlite://${DB_PATH}?mode=rwc"
PORT=3035
BASE_URL="http://localhost:$PORT"

cleanup() {
    if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

print_step "Building server (SQLite)..."
cd "$(dirname "$0")/../.."
cargo build -p payments-app --no-default-features --features sqlite --quiet

print_step "Starting server..."
PORT=$PORT cargo run -p payments-app --no-default-features --features sqlite --quiet &
SERVER_PID=$!
sleep 2

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    print_error "Server failed to start"
    exit 1
fi

export BASE_URL
"$(dirname "$0")/../common/run_e2e_tests.sh"
