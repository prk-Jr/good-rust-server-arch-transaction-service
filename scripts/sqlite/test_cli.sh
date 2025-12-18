#!/usr/bin/env bash
# SQLite CLI Tests Wrapper

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

TEMP_DIR=$(mktemp -d)
DB_PATH="$TEMP_DIR/test.db"
export DATABASE_URL="sqlite://${DB_PATH}?mode=rwc"
export PORT=3033
export PAYMENTS_API_URL="http://localhost:$PORT"

cleanup() {
    if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

print_step "Building server & CLI (SQLite)..."
cd "$(dirname "$0")/../.."
cargo build -p payments-app --no-default-features --features sqlite --quiet
cargo build -p payments-cli --quiet

print_step "Starting server..."
cargo run -p payments-app --no-default-features --features sqlite --quiet &
SERVER_PID=$!
sleep 2

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    print_error "Server failed to start"
    exit 1
fi

export CLI="cargo run -p payments-cli --quiet --"
"$(dirname "$0")/../common/run_cli_tests.sh"
