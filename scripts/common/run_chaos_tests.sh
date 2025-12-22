#!/usr/bin/env bash
# Shared Chaos test logic
# Requires BASE_URL variable and optionally CHAOS_ITERATIONS

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

if [ -z "${BASE_URL:-}" ]; then
    print_error "BASE_URL variable not set"
    exit 1
fi

ITERATIONS=${CHAOS_ITERATIONS:-20}
TESTS_PASSED=0
TESTS_FAILED=0

echo -e "${RED}"
echo "╔══════════════════════════════════════════════╗"
echo "║       💥 PAYMENTS CHAOS TESTING 💥           ║"
echo "║     Stress • Edge Cases • Validation         ║"
echo "╚══════════════════════════════════════════════╝"
echo -e "${NC}"
echo "Iterations: $ITERATIONS"
echo "Base URL: $BASE_URL"
echo

# ─────────────────────────────────────────────────────────────────────────────
# BOOTSTRAP - Get API Key
# ─────────────────────────────────────────────────────────────────────────────
print_step "Bootstrapping API Key..."
BOOTSTRAP_RESP=$(curl -s -X POST "$BASE_URL/api/bootstrap" -H "Content-Type: application/json" -d '{"name":"chaos-test-key"}')
API_KEY=$(echo "$BOOTSTRAP_RESP" | grep -o '"api_key":"[^"]*"' | cut -d'"' -f4)
if [[ -n "$API_KEY" ]]; then
    print_success "API Key obtained: ${API_KEY:0:10}..."
else
    print_error "Failed to get API key: $BOOTSTRAP_RESP"
    exit 1
fi

AUTH_HEADER="Authorization: Bearer $API_KEY"

http_post() {
    curl -s -X POST "$BASE_URL$1" -H "Content-Type: application/json" -H "$AUTH_HEADER" -d "$2"
}

http_get() {
    curl -s -H "$AUTH_HEADER" "$BASE_URL$1"
}

create_account() {
    local name="$1"
    local result=$(http_post "/api/accounts" "{\"name\":\"$name\",\"currency\":\"USD\"}")
    echo "$result" | grep -o '"id":"[^"]*"' | cut -d'"' -f4
}

deposit() {
    http_post "/api/transactions/deposit" "{\"account_id\":\"$1\",\"amount\":$2,\"currency\":\"USD\"}"
}

withdraw() {
    http_post "/api/transactions/withdraw" "{\"account_id\":\"$1\",\"amount\":$2,\"currency\":\"USD\"}"
}

transfer() {
    http_post "/api/transactions/transfer" "{\"from_account_id\":\"$1\",\"to_account_id\":\"$2\",\"amount\":$3,\"currency\":\"USD\"}"
}

get_balance() {
    http_get "/api/accounts/$1" | grep -o '"amount":[0-9]*' | cut -d':' -f2
}

# ─────────────────────────────────────────────────────────────────────────────
# 1. Rapid Account Creation
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 1: Rapid Account Creation"
print_info "Creating $ITERATIONS accounts..."
start_time=$(date +%s)
for i in $(seq 1 $ITERATIONS); do
    create_account "ChaosAccount-$i" >/dev/null
done
end_time=$(date +%s)
print_success "Created accounts in $((end_time - start_time))s"
TESTS_PASSED=$((TESTS_PASSED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# 2. Sequential Deposits
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 2: Sequential Deposits"
DEPOSIT_ACCOUNT=$(create_account "ChaosDeposit")
DEPOSIT_AMOUNT=100
for i in $(seq 1 $ITERATIONS); do
    deposit "$DEPOSIT_ACCOUNT" $DEPOSIT_AMOUNT >/dev/null
done
EXPECTED_BALANCE=$((ITERATIONS * DEPOSIT_AMOUNT))
ACTUAL_BALANCE=$(get_balance "$DEPOSIT_ACCOUNT")
assert_eq "Balance Integrity" "$EXPECTED_BALANCE" "$ACTUAL_BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# 3. Bidirectional Transfers
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 3: Bidirectional Transfers"
ALICE=$(create_account "ChaosAlice")
BOB=$(create_account "ChaosBob")
deposit "$ALICE" 50000 >/dev/null
deposit "$BOB" 50000 >/dev/null
print_info "Performing $ITERATIONS transfers..."
for i in $(seq 1 $ITERATIONS); do
    transfer "$ALICE" "$BOB" 100 >/dev/null
    transfer "$BOB" "$ALICE" 100 >/dev/null
done
ALICE_BALANCE=$(get_balance "$ALICE")
BOB_BALANCE=$(get_balance "$BOB")
TOTAL=$((ALICE_BALANCE + BOB_BALANCE))
assert_eq "Money Conservation (Total=100000)" "100000" "$TOTAL" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# 4. Overdraft Prevention
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 4: Overdraft Prevention"
OVERDRAFT_ACCOUNT=$(create_account "ChaosOverdraft")
deposit "$OVERDRAFT_ACCOUNT" 1000 >/dev/null
withdraw "$OVERDRAFT_ACCOUNT" 600 >/dev/null
RESULT=$(withdraw "$OVERDRAFT_ACCOUNT" 600 2>&1)
FINAL_BALANCE=$(get_balance "$OVERDRAFT_ACCOUNT")
assert_eq "Overdraft blocked (Balance=400)" "400" "$FINAL_BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# 5. Rapid Reads
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 5: Rapid Fire Reads"
LOAD_ACCOUNT=$(create_account "ChaosLoad")
deposit "$LOAD_ACCOUNT" 1000000 >/dev/null
start_time=$(date +%s)
for i in $(seq 1 $ITERATIONS); do
    http_get "/api/accounts/$LOAD_ACCOUNT" >/dev/null
    http_get "/health" >/dev/null
done
end_time=$(date +%s)
print_success "Completed reads in $((end_time - start_time + 1))s"
TESTS_PASSED=$((TESTS_PASSED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# 6. Invalid Input
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 6: Invalid Handling"
ERRORS=0
# Empty name
if http_post "/api/accounts" '{"name":"   ","currency":"USD"}' | grep -qi "error"; then
    print_success "Empty name rejected"
else
    print_error "Empty name accepted"
    ERRORS=$((ERRORS + 1))
fi
# Zero amount
if http_post "/api/transactions/deposit" '{"account_id":"'"$LOAD_ACCOUNT"'","amount":0,"currency":"USD"}' | grep -qi "error\|positive"; then
    print_success "Zero amount rejected"
else
    print_error "Zero amount accepted"
    ERRORS=$((ERRORS + 1))
fi

if [[ $ERRORS -eq 0 ]]; then
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# ─────────────────────────────────────────────────────────────────────────────
# 7. Idempotency
# ─────────────────────────────────────────────────────────────────────────────
print_step "CHAOS TEST 7: Idempotency"
IDEM_ACCOUNT=$(create_account "ChaosIdempotency")
IDEM_KEY="chaos-idem-$(date +%s)"
http_post "/api/transactions/deposit" "{\"account_id\":\"$IDEM_ACCOUNT\",\"amount\":500,\"currency\":\"USD\",\"idempotency_key\":\"$IDEM_KEY\"}" >/dev/null
http_post "/api/transactions/deposit" "{\"account_id\":\"$IDEM_ACCOUNT\",\"amount\":500,\"currency\":\"USD\",\"idempotency_key\":\"$IDEM_KEY\"}" >/dev/null
IDEM_BALANCE=$(get_balance "$IDEM_ACCOUNT")
assert_eq "Idempotency (Balance=500)" "500" "$IDEM_BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

echo
if [[ "$TESTS_FAILED" -eq 0 ]]; then
    print_success "All $TESTS_PASSED Chaos tests passed!"
    exit 0
else
    print_error "$TESTS_FAILED Chaos tests failed ($TESTS_PASSED passed)"
    exit 1
fi
