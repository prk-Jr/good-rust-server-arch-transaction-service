#!/usr/bin/env bash

# Payments Workspace Validation Suite
# Runs checks, feature-matrix tests, and build to ensure nothing is broken.
# Usage: ./validate_all.sh

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_step() {
    echo -e "${BLUE}==== $1 ====${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

echo -e "${BLUE}"
echo "╔══════════════════════════════════════════════╗"
echo "║         Payments Validation Suite            ║"
echo "║      Tests | Checks | Feature Matrix         ║"
echo "╚══════════════════════════════════════════════╝"
echo -e "${NC}"

# Run from repo root
cd "$(dirname "$0")"

start_time=$(date +%s)

failures=()
warns=()

run_required() {
    local desc="$1"
    shift
    print_step "$desc"
    if "$@"; then
        print_success "$desc"
    else
        print_error "$desc failed"
        failures+=("$desc")
    fi
}

run_warn() {
    local desc="$1"
    shift
    print_step "$desc"
    if "$@"; then
        print_success "$desc"
    else
        print_warning "$desc had issues (continuing)"
        warns+=("$desc")
    fi
}

# 1) Code checks
run_required "cargo check (all targets)" cargo check --all-targets
run_warn "cargo clippy (all targets, all features)" cargo clippy --all-targets --all-features -- -D warnings

# 2) Tests (feature matrix)
run_required "payments-types tests" cargo test -p payments-types
run_required "payments-repo tests (postgres)" cargo test -p payments-repo --features postgres
run_required "payments-repo tests (sqlite)" cargo test -p payments-repo --no-default-features --features sqlite
run_required "payments-hex tests" cargo test -p payments-hex
run_required "payments-client tests" cargo test -p payments-client
run_required "payments-app tests (postgres default)" cargo test -p payments-app
run_required "payments-app tests (sqlite feature)" cargo test -p payments-app --no-default-features --features sqlite

# 3) Build verification
run_required "release build (postgres default)" cargo build --release
run_required "release build (sqlite feature)" cargo build --release --no-default-features --features sqlite

# 4) Integration Tests (SQLite)
run_required "sqlite: test_cli" ./scripts/sqlite/test_cli.sh
run_required "sqlite: test_e2e" ./scripts/sqlite/test_e2e.sh
run_required "sqlite: chaos_test" env CHAOS_ITERATIONS=5 ./scripts/sqlite/chaos_test.sh

# 5) Integration Tests (PostgreSQL)
run_required "postgres: test_cli" ./scripts/postgres/test_cli.sh
run_required "postgres: test_e2e" ./scripts/postgres/test_e2e.sh
run_required "postgres: chaos_test" env CHAOS_ITERATIONS=5 ./scripts/postgres/chaos_test.sh

end_time=$(date +%s)
duration=$((end_time - start_time))
minutes=$((duration / 60))
seconds=$((duration % 60))

echo
echo -e "${BLUE}╔══════════════════════════════════════════════╗"
echo "║               VALIDATION SUMMARY             ║"
echo -e "╚══════════════════════════════════════════════╝${NC}"

if [ ${#failures[@]} -eq 0 ]; then
    print_success "All required steps passed in ${minutes}m ${seconds}s"
else
    print_error "Failures: ${failures[*]}"
    exit 1
fi

if [ ${#warns[@]} -gt 0 ]; then
    print_warning "Warnings: ${warns[*]}"
fi

echo "Steps run:"
echo "  • cargo check, clippy (warn only)"
echo "  • tests: payments-types, payments-repo (postgres/sqlite), payments-hex, payments-client, payments-app (postgres/sqlite)"
echo "  • release builds: default postgres, sqlite feature"

print_success "Payments workspace is healthy."
