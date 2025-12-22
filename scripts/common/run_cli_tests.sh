#!/usr/bin/env bash
# Shared CLI test logic
# Requires CLI variable to be set to the command prefix (e.g. "cargo run -p payments-cli --")
# Requires BASE_URL variable to be set for bootstrap

set -euo pipefail
source "$(dirname "$0")/../common/lib.sh"

if [ -z "${CLI:-}" ]; then
    print_error "CLI variable not set"
    exit 1
fi

if [ -z "${BASE_URL:-}" ]; then
    print_error "BASE_URL variable not set"
    exit 1
fi

TESTS_PASSED=0
TESTS_FAILED=0

# ─────────────────────────────────────────────────────────────────────────────
# BOOTSTRAP - Get API Key
# ─────────────────────────────────────────────────────────────────────────────
print_step "Bootstrapping API Key..."
BOOTSTRAP_RESP=$(curl -s -X POST "$BASE_URL/api/bootstrap" -H "Content-Type: application/json" -d '{"name":"cli-test-key"}')
API_KEY=$(echo "$BOOTSTRAP_RESP" | grep -o '"api_key":"[^"]*"' | cut -d'"' -f4)
if [[ -n "$API_KEY" ]]; then
    print_success "API Key obtained: ${API_KEY:0:10}..."
    export PAYMENTS_API_KEY="$API_KEY"
else
    print_error "Failed to get API key: $BOOTSTRAP_RESP"
    exit 1
fi

# ─────────────────────────────────────────────────────────────────────────────
# Test: Health check (no auth needed)
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing health endpoint..."
if $CLI health; then
    print_success "Health check passed"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    print_error "Health check failed"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# ─────────────────────────────────────────────────────────────────────────────
# Test: Create account
# ─────────────────────────────────────────────────────────────────────────────
print_step "Creating account 'Alice Corp'..."
ALICE_JSON=$($CLI account create "Alice Corp" --currency USD)
ALICE_ID=$(echo "$ALICE_JSON" | grep -o '"id": *"[^"]*"' | cut -d'"' -f4)
assert_contains "Create account Alice" "Alice" "$ALICE_JSON" && \
echo "   Created: $ALICE_ID" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

print_step "Creating account 'Bob Inc'..."
BOB_JSON=$($CLI account create "Bob Inc" --currency USD)
BOB_ID=$(echo "$BOB_JSON" | grep -o '"id": *"[^"]*"' | cut -d'"' -f4)
assert_contains "Create account Bob" "Bob" "$BOB_JSON" && \
echo "   Created: $BOB_ID" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Test: Get account
# ─────────────────────────────────────────────────────────────────────────────
print_step "Getting Alice's account..."
ALICE_GET=$($CLI account get "$ALICE_ID")
assert_contains "Get Alice account" "Alice Corp" "$ALICE_GET" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Test: List accounts
# ─────────────────────────────────────────────────────────────────────────────
print_step "Listing all accounts..."
ACCOUNTS=$($CLI account list)
if echo "$ACCOUNTS" | grep -q "Alice Corp" && echo "$ACCOUNTS" | grep -q "Bob Inc"; then
    print_success "Account list verified"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    print_error "Account list missing accounts"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# ─────────────────────────────────────────────────────────────────────────────
# Test: Deposit
# ─────────────────────────────────────────────────────────────────────────────
print_step "Depositing 10000 cents (\$100) to Alice..."
DEPOSIT=$($CLI transaction deposit --account "$ALICE_ID" --amount 10000 --currency USD)
assert_contains "Deposit succeeded" "id" "$DEPOSIT" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# Verify balance
ALICE_BALANCE=$($CLI account get "$ALICE_ID" | grep -o '"amount": *[0-9]*' | head -1 | grep -o '[0-9]*')
assert_eq "Alice balance check" "10000" "$ALICE_BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Test: Transfer
# ─────────────────────────────────────────────────────────────────────────────
print_step "Transferring 3500 cents (\$35) from Alice to Bob..."
TRANSFER=$($CLI transaction transfer --from "$ALICE_ID" --to "$BOB_ID" --amount 3500 --currency USD)
assert_contains "Transfer succeeded" "id" "$TRANSFER" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# Verify balances
ALICE_BALANCE=$($CLI account get "$ALICE_ID" | grep -o '"amount": *[0-9]*' | head -1 | grep -o '[0-9]*')
BOB_BALANCE=$($CLI account get "$BOB_ID" | grep -o '"amount": *[0-9]*' | head -1 | grep -o '[0-9]*')

assert_eq "Alice balance after transfer" "6500" "$ALICE_BALANCE" && \
assert_eq "Bob balance after transfer" "3500" "$BOB_BALANCE" && \
TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Test: Withdraw
# ─────────────────────────────────────────────────────────────────────────────
print_step "Withdrawing 1500 cents (\$15) from Bob..."
WITHDRAW=$($CLI transaction withdraw --account "$BOB_ID" --amount 1500 --currency USD)
assert_contains "Withdraw succeeded" "id" "$WITHDRAW" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

BOB_BALANCE=$($CLI account get "$BOB_ID" | grep -o '"amount": *[0-9]*' | head -1 | grep -o '[0-9]*')
assert_eq "Bob balance after withdraw" "2000" "$BOB_BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Test: Insufficient funds
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing insufficient funds (withdraw 99999 from Bob)..."
INSUF_OUTPUT=$($CLI transaction withdraw --account "$BOB_ID" --amount 99999 --currency USD 2>&1 || true)
if echo "$INSUF_OUTPUT" | grep -qi "insufficient\|error\|failed"; then
    print_success "Insufficient funds correctly rejected"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    print_error "Insufficient funds was not rejected!"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# ─────────────────────────────────────────────────────────────────────────────
# Test: Idempotency
# ─────────────────────────────────────────────────────────────────────────────
print_step "Testing idempotency with same key..."
IDEM_KEY="test-idem-key-$(date +%s)"
$CLI transaction deposit --account "$ALICE_ID" --amount 500 --currency USD --idempotency-key "$IDEM_KEY" >/dev/null
$CLI transaction deposit --account "$ALICE_ID" --amount 500 --currency USD --idempotency-key "$IDEM_KEY" >/dev/null

# Balance should only increase by 500, not 1000. Start was 6500. Expected 7000.
ALICE_BALANCE=$($CLI account get "$ALICE_ID" | grep -o '"amount": *[0-9]*' | head -1 | grep -o '[0-9]*')
assert_eq "Idempotency check (balance should be 7000)" "7000" "$ALICE_BALANCE" && TESTS_PASSED=$((TESTS_PASSED + 1)) || TESTS_FAILED=$((TESTS_FAILED + 1))

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────
echo
if [[ "$TESTS_FAILED" -eq 0 ]]; then
    print_success "All $TESTS_PASSED CLI tests passed!"
    exit 0
else
    print_error "$TESTS_FAILED CLI tests failed ($TESTS_PASSED passed)"
    exit 1
fi
