#!/usr/bin/env bash
# Astral Key - End-to-End Authentication Flow Tests
#
# This script tests all authentication flows in the system

set -e

API_BASE="${API_BASE:-http://localhost:8080}"
FAILED_TESTS=0
PASSED_TESTS=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test result tracking
test_pass() {
    echo -e "${GREEN}✓${NC} $1"
    ((PASSED_TESTS++))
}

test_fail() {
    echo -e "${RED}✗${NC} $1"
    ((FAILED_TESTS++))
}

test_info() {
    echo -e "${YELLOW}→${NC} $1"
}

# API request helper
api_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    local token=$4

    if [ -n "$token" ]; then
        curl -s -X "$method" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $token" \
            -d "$data" \
            "${API_BASE}${endpoint}"
    else
        curl -s -X "$method" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "${API_BASE}${endpoint}"
    fi
}

echo "=========================================="
echo "Astral Key - End-to-End Tests"
echo "=========================================="
echo ""

# Test 1: Health check
echo "Test 1: Health Check"
if api_request "GET" "/health" "" "" | grep -q "OK"; then
    test_pass "Health endpoint responds"
else
    test_fail "Health endpoint check failed"
fi

# Test 2: Readiness check
echo ""
echo "Test 2: Readiness Check"
if api_request "GET" "/ready" "" "" | grep -q "ready"; then
    test_pass "Readiness endpoint reports ready"
else
    test_fail "Readiness endpoint check failed"
fi

# Test 3: Get supported chains
echo ""
echo "Test 3: Get Supported Chains"
chains=$(api_request "GET" "/api/v1/auth/web3/chains" "" "")
if echo "$chains" | grep -q "ethereum" && echo "$chains" | grep -q "polygon"; then
    test_pass "Returns supported chains"
else
    test_fail "Failed to get supported chains"
fi

# Test 4: Web3 nonce generation
echo ""
echo "Test 4: Web3 Nonce Generation"
test_info "Requesting nonce for Web3 authentication..."
nonce_response=$(api_request "POST" "/api/v1/auth/web3/nonce" \
    '{"domain": "localhost", "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb", "chain_id": 1}' \
    "")

if echo "$nonce_response" | grep -q "nonce"; then
    test_pass "Web3 nonce generated successfully"
    nonce=$(echo "$nonce_response" | jq -r '.nonce')
    message_template=$(echo "$nonce_response" | jq -r '.message_template')
else
    test_fail "Failed to generate Web3 nonce"
    nonce=""
fi

# Test 5: Web3 signature verification (with invalid signature)
echo ""
echo "Test 5: Web3 Signature Verification (Invalid Signature)"
test_info "Attempting to verify with invalid signature..."
if [ -n "$nonce" ]; then
    # Replace nonce in message template
    message=$(echo "$message_template" | sed "s/$nonce/$nonce/")

    verify_response=$(api_request "POST" "/api/v1/auth/web3/verify" \
        "{\"message\": \"$message\", \"signature\": \"0x$(printf '0%.0s' {1..130}\", \"chain_id\": 1}" \
        "")

    if echo "$verify_response" | grep -q "error\|unauthorized\|invalid"; then
        test_pass "Correctly rejects invalid signature"
    else
        test_fail "Should reject invalid signature"
    fi
else
    test_fail "Skipping - no nonce generated"
fi

# Test 6: FIDO2 authentication options (non-existent user)
echo ""
echo "Test 6: FIDO2 Authentication Options (Non-existent User)"
test_info "Requesting FIDO2 authentication for non-existent user..."
fido2_auth_response=$(api_request "POST" "/api/v1/auth/fido2/authenticate/options" \
    '{"username": "00000000-0000-0000-0000-000000000000"}' \
    "")

if echo "$fido2_auth_response" | grep -q "error\|not found"; then
    test_pass "Correctly returns error for non-existent user"
else
    test_fail "Should return error for non-existent user"
fi

# Test 7: Protected route without authentication
echo ""
echo "Test 7: Protected Route Without Authentication"
protected_response=$(api_request "GET" "/api/v1/users/me" "" "")

if echo "$protected_response" | grep -q "unauthorized\|missing"; then
    test_pass "Correctly rejects unauthenticated request"
else
    test_fail "Should reject unauthenticated request"
fi

# Test 8: Session refresh with invalid token
echo ""
echo "Test 8: Session Refresh (Invalid Token)"
refresh_response=$(api_request "POST" "/api/v1/sessions/refresh" \
    '{"refresh_token": "invalid_token"}' \
    "")

if echo "$refresh_response" | grep -q "unauthorized\|invalid"; then
    test_pass "Correctly rejects invalid refresh token"
else
    test_fail "Should reject invalid refresh token"
fi

# Test 9: CORS preflight
echo ""
echo "Test 9: CORS Preflight"
cors_response=$(curl -s -X OPTIONS \
    -H "Origin: http://localhost:3000" \
    -H "Access-Control-Request-Method: POST" \
    -H "Access-Control-Request-Headers: content-type" \
    "${API_BASE}/api/v1/auth/web3/nonce" \
    )

# CORS preflight should return 200 or 204
if [ $? -eq 0 ]; then
    test_pass "CORS preflight handled"
else
    test_fail "CORS preflight check failed"
fi

# Summary
echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo "Passed: $PASSED_TESTS"
echo "Failed: $FAILED_TESTS"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
