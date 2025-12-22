#!/usr/bin/env bash
# PostgreSQL CLI Tests Wrapper

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

cleanup() {
    print_step "Cleaning up..."
    docker compose -f docker-compose.test.yml down -v 2>/dev/null || true
}
trap cleanup EXIT

cd "$(dirname "$0")/../.."

print_step "Building payments-cli..."
cargo build -p payments-cli --quiet
export CLI="cargo run -p payments-cli --quiet --"

print_step "Starting PostgreSQL and payments-server (Docker)..."
docker compose -f docker-compose.test.yml up -d --build

print_step "Waiting for services..."
sleep 5
export PAYMENTS_API_URL="http://localhost:3000"
export BASE_URL="http://localhost:3000"

# Wait for healthy
MAX_RETRIES=30
RETRY=0
while ! $CLI health > /dev/null 2>&1; do
    RETRY=$((RETRY + 1))
    if [[ $RETRY -ge $MAX_RETRIES ]]; then
        print_error "Server failed to become healthy"
        docker compose -f docker-compose.test.yml logs app
        exit 1
    fi
    sleep 2
done
print_success "Services ready"

"scripts/common/run_cli_tests.sh"
