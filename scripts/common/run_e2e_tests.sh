#!/usr/bin/env bash
# Shared E2E test logic
# Requires BASE_URL variable (e.g. "http://localhost:3000")

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

if [ -z "${BASE_URL:-}" ]; then
    print_error "BASE_URL variable not set"
    exit 1
fi

TESTS_PASSED=0
TESTS_FAILED=0

# ─────────────────────────────────────────────────────────────────────────────
# BOOTSTRAP - Get API Key
# ─────────────────────────────────────────────────────────────────────────────
print_step "Bootstrapping API Key"
BOOTSTRAP_RESP=$(curl -s -X POST "$BASE_URL/api/bootstrap" -H "Content-Type: application/json" -d '{"name":"test-key"}')
API_KEY=$(echo "$BOOTSTRAP_RESP" | grep -o '"api_key":"[^"]*"' | cut -d'"' -f4)
if [[ -n "$API_KEY" ]]; then
    print_success "API Key obtained: ${API_KEY:0:10}..."
else
    print_error "Failed to get API key: $BOOTSTRAP_RESP"
    exit 1
fi

AUTH_HEADER="Authorization: Bearer $API_KEY"

assert_status() {
    local name="$1" expected="$2" url="$3" method="${4:-GET}" data="${5:-}"
    if [[ "$method" == "POST" ]]; then
        actual=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$url" -H "Content-Type: application/json" -H "$AUTH_HEADER" -d "$data")
    else
        actual=$(curl -s -o /dev/null -w "%{http_code}" -H "$AUTH_HEADER" "$url")
    fi
    assert_eq "$name (HTTP $expected)" "$expected" "$actual" && \
    TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))
}

# ─────────────────────────────────────────────────────────────────────────────
# HEALTH (no auth needed)
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing Health Endpoint"
HEALTH_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health")
assert_eq "Health Check (HTTP 200)" "200" "$HEALTH_STATUS" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))
HEALTH=$(curl -s "$BASE_URL/health")
assert_contains "Health Body" "healthy" "$HEALTH" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# ACCOUNT CRUD
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing Account Operations"

# Create account
ALICE_RESP=$(curl -s -X POST "$BASE_URL/api/accounts" -H "Content-Type: application/json" -H "$AUTH_HEADER" -d '{"name":"Alice","currency":"USD"}')
ALICE_ID=$(echo "$ALICE_RESP" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
assert_contains "Create account ID" "id" "$ALICE_RESP" && \
assert_contains "Create account Name" "Alice" "$ALICE_RESP" && \
TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# Get account
GET_RESP=$(curl -s -H "$AUTH_HEADER" "$BASE_URL/api/accounts/$ALICE_ID")
assert_contains "Get account by ID" "Alice" "$GET_RESP" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# List accounts
LIST_RESP=$(curl -s -H "$AUTH_HEADER" "$BASE_URL/api/accounts")
assert_contains "List accounts" "Alice" "$LIST_RESP" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_PASSED + 1))

# Account not found
assert_status "Get non-existent account" "404" "$BASE_URL/api/accounts/00000000-0000-0000-0000-000000000000"

# Invalid UUID
assert_status "Get invalid UUID" "400" "$BASE_URL/api/accounts/invalid-uuid"

# Empty name validation
assert_status "Create with empty name" "400" "$BASE_URL/api/accounts" "POST" '{"name":"   ","currency":"USD"}'

# ─────────────────────────────────────────────────────────────────────────────
# DEPOSIT
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing Deposit Operations"

DEP_RESP=$(curl -s -X POST "$BASE_URL/api/transactions/deposit" -H "Content-Type: application/json" -H "$AUTH_HEADER" \
    -d "{\"account_id\":\"$ALICE_ID\",\"amount\":10000,\"currency\":\"USD\"}")
assert_contains "Deposit returns ID" "id" "$DEP_RESP" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

BALANCE=$(curl -s -H "$AUTH_HEADER" "$BASE_URL/api/accounts/$ALICE_ID" | grep -o '"amount":[0-9]*' | cut -d':' -f2)
assert_eq "Balance after deposit" "10000" "$BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# Zero amount
assert_status "Deposit zero amount" "400" "$BASE_URL/api/transactions/deposit" "POST" \
    "{\"account_id\":\"$ALICE_ID\",\"amount\":0,\"currency\":\"USD\"}"

# Negative amount
assert_status "Deposit negative amount" "400" "$BASE_URL/api/transactions/deposit" "POST" \
    "{\"account_id\":\"$ALICE_ID\",\"amount\":-100,\"currency\":\"USD\"}"

# ─────────────────────────────────────────────────────────────────────────────
# WITHDRAW
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing Withdraw Operations"

WITH_RESP=$(curl -s -X POST "$BASE_URL/api/transactions/withdraw" -H "Content-Type: application/json" -H "$AUTH_HEADER" \
    -d "{\"account_id\":\"$ALICE_ID\",\"amount\":3000,\"currency\":\"USD\"}")
assert_contains "Withdraw returns ID" "id" "$WITH_RESP" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

BALANCE=$(curl -s -H "$AUTH_HEADER" "$BASE_URL/api/accounts/$ALICE_ID" | grep -o '"amount":[0-9]*' | cut -d':' -f2)
assert_eq "Balance after withdraw" "7000" "$BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# Insufficient funds
assert_status "Withdraw insufficient funds" "400" "$BASE_URL/api/transactions/withdraw" "POST" \
    "{\"account_id\":\"$ALICE_ID\",\"amount\":99999,\"currency\":\"USD\"}"

# ─────────────────────────────────────────────────────────────────────────────
# TRANSFER
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing Transfer Operations"

BOB_RESP=$(curl -s -X POST "$BASE_URL/api/accounts" -H "Content-Type: application/json" -H "$AUTH_HEADER" -d '{"name":"Bob","currency":"USD"}')
BOB_ID=$(echo "$BOB_RESP" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

XFER_RESP=$(curl -s -X POST "$BASE_URL/api/transactions/transfer" -H "Content-Type: application/json" -H "$AUTH_HEADER" \
    -d "{\"from_account_id\":\"$ALICE_ID\",\"to_account_id\":\"$BOB_ID\",\"amount\":2000,\"currency\":\"USD\"}")
assert_contains "Transfer returns ID" "id" "$XFER_RESP" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

ALICE_BAL=$(curl -s -H "$AUTH_HEADER" "$BASE_URL/api/accounts/$ALICE_ID" | grep -o '"amount":[0-9]*' | cut -d':' -f2)
BOB_BAL=$(curl -s -H "$AUTH_HEADER" "$BASE_URL/api/accounts/$BOB_ID" | grep -o '"amount":[0-9]*' | cut -d':' -f2)
assert_eq "Alice balance after transfer" "5000" "$ALICE_BAL" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))
assert_eq "Bob balance after transfer" "2000" "$BOB_BAL" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# Self transfer
assert_status "Self transfer rejected" "400" "$BASE_URL/api/transactions/transfer" "POST" \
    "{\"from_account_id\":\"$ALICE_ID\",\"to_account_id\":\"$ALICE_ID\",\"amount\":100,\"currency\":\"USD\"}"

# Cross-currency transfer
EUR_RESP=$(curl -s -X POST "$BASE_URL/api/accounts" -H "Content-Type: application/json" -H "$AUTH_HEADER" -d '{"name":"Euro Account","currency":"EUR"}')
EUR_ID=$(echo "$EUR_RESP" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
curl -s -X POST "$BASE_URL/api/transactions/deposit" -H "Content-Type: application/json" -H "$AUTH_HEADER" \
    -d "{\"account_id\":\"$EUR_ID\",\"amount\":5000,\"currency\":\"EUR\"}" >/dev/null

CROSS_RESP=$(curl -s -X POST "$BASE_URL/api/transactions/transfer" -H "Content-Type: application/json" -H "$AUTH_HEADER" \
    -d "{\"from_account_id\":\"$EUR_ID\",\"to_account_id\":\"$ALICE_ID\",\"amount\":100,\"currency\":\"EUR\"}")
assert_contains "Cross-currency transfer" "error" "$CROSS_RESP" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────
echo
if [[ "$TESTS_FAILED" -eq 0 ]]; then
    print_success "All $TESTS_PASSED E2E tests passed!"
    exit 0
else
    print_error "$TESTS_FAILED E2E tests failed ($TESTS_PASSED passed)"
    exit 1
fi

