#!/usr/bin/env bash
# PostgreSQL E2E Tests Wrapper

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

cleanup() {
    print_step "Cleaning up..."
    docker compose -f docker-compose.test.yml down -v 2>/dev/null || true
}
trap cleanup EXIT

cd "$(dirname "$0")/../.."

print_step "Starting PostgreSQL and payments-server (Docker)..."
docker compose -f docker-compose.test.yml up -d --build

print_step "Waiting for services..."
sleep 5
export BASE_URL="http://localhost:3000"

# Wait for healthy
MAX_RETRIES=30
RETRY=0
while ! curl -s "$BASE_URL/health" > /dev/null; do
    RETRY=$((RETRY + 1))
    if [[ $RETRY -ge $MAX_RETRIES ]]; then
        print_error "Server failed to become healthy"
        docker compose -f docker-compose.test.yml logs app
        exit 1
    fi
    sleep 2
done
print_success "Services ready"

"scripts/common/run_e2e_tests.sh"
