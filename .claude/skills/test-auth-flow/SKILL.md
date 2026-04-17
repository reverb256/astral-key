---
name: test-auth-flow
description: Generate API test cases for Web3, FIDO2, and JWT authentication flows
---

# Test Authentication Flow

Generate test cases for Astral Key authentication endpoints. This skill helps create curl-based tests and integration tests for auth flows.

## Usage

Invoke this skill when:
- Adding new authentication endpoints
- Testing existing auth flows
- Creating regression tests for auth bugs
- Writing e2e tests for new features

## Authentication Flows

### 1. Web3 SIWE Flow

**Endpoints:**
- `POST /api/v1/auth/web3/nonce` - Request SIWE nonce
- `POST /api/v1/auth/web3/verify` - Verify signature and get JWT

**Test Cases:**

#### Success Path: Valid Signature
```bash
# 1. Request nonce
curl -X POST http://localhost:8080/api/v1/auth/web3/nonce \
  -H "Content-Type: application/json" \
  -d '{
    "domain": "localhost",
    "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
    "chain_id": 1
  }'

# 2. Sign the message_template with wallet
# 3. Verify signature
curl -X POST http://localhost:8080/api/v1/auth/web3/verify \
  -H "Content-Type: application/json" \
  -d '{
    "message": "domain=localhost...",
    "signature": "0x...",
    "chain_id": 1
  }'

# Expected: access_token and refresh_token
```

#### Failure Cases
- Invalid signature format
- Expired nonce
- Mismatched address
- Invalid chain_id

### 2. FIDO2/WebAuthn Flow

**Endpoints:**
- `POST /api/v1/auth/fido2/authenticate/options` - Get assertion challenge
- `POST /api/v1/auth/fido2/authenticate/verify` - Verify assertion
- `POST /api/v1/auth/fido2/register/options` - Get attestation challenge (requires auth)
- `POST /api/v1/auth/fido2/register/verify` - Complete registration (requires auth)

**Test Cases:**

#### Authentication Flow
```bash
# 1. Get authentication options
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/options \
  -H "Content-Type: application/json" \
  -d '{"username": "<user_uuid>"}'

# 2. Use challenge with navigator.credentials.get()
# 3. Verify assertion
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/verify \
  -H "Content-Type: application/json" \
  -d '{
    "id": "<credential_id>",
    "raw_id": "<raw_id>",
    "response": {
      "client_data_json": "<base64>",
      "authenticator_data": "<base64>",
      "signature": "<base64>",
      "user_handle": "<user_uuid>"
    },
    "type": "public-key"
  }'
```

#### Registration Flow (requires JWT)
```bash
# 1. Get registration options
curl -X POST http://localhost:8080/api/v1/auth/fido2/register/options \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{
    "username": "mykey",
    "display_name": "My Security Key"
  }'

# 2. Use challenge with navigator.credentials.create()
# 3. Complete registration
curl -X POST http://localhost:8080/api/v1/auth/fido2/register/verify \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '<credential_creation_response>'
```

### 3. JWT Session Flow

**Endpoints:**
- `POST /api/v1/sessions/refresh` - Refresh tokens
- `GET /api/v1/sessions` - List sessions (requires auth)
- `DELETE /api/v1/sessions/current` - Logout (requires auth)

**Test Cases:**

#### Token Refresh
```bash
curl -X POST http://localhost:8080/api/v1/sessions/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token": "<refresh_token>"}'

# Expected: new access_token and refresh_token
```

#### Protected Route Access
```bash
curl -X GET http://localhost:8080/api/v1/users/me \
  -H "Authorization: Bearer <access_token>"

# Expected: user profile JSON
```

#### Logout
```bash
curl -X DELETE http://localhost:8080/api/v1/sessions/current \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{"refresh_token": "<token>"}'
```

## Integration Test Template

```rust
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_web3_authentication_flow() {
    let client = Client::new();
    let api_base = "http://localhost:8080";

    // Step 1: Request nonce
    let nonce_resp = client
        .post(format!("{}/api/v1/auth/web3/nonce", api_base))
        .json(&json!({
            "domain": "localhost",
            "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
            "chain_id": 1
        }))
        .send()
        .await
        .expect("Failed to request nonce");

    assert_eq!(nonce_resp.status(), 200);

    let nonce_data: serde_json::Value = nonce_resp
        .json()
        .await
        .expect("Failed to parse nonce response");

    let nonce = nonce_data["nonce"].as_str().expect("No nonce in response");

    // Step 2: Verify signature (would need real signature)
    // ...

    // Step 3: Use access_token for protected route
    // ...
}
```

## Test Script Generator

When invoked, this skill can:
1. Generate curl commands for a specific auth flow
2. Create Rust integration test scaffolding
3. Add test cases to `scripts/test-e2e.sh`
4. Generate test data fixtures

## Common Test Patterns

### Extract JWT from response
```bash
ACCESS_TOKEN=$(curl -s -X POST http://localhost:8080/api/v1/auth/web3/verify \
  -H "Content-Type: application/json" \
  -d '{"message": "...", "signature": "...", "chain_id": 1}' \
  | jq -r '.access_token')
```

### Use JWT in subsequent requests
```bash
curl -X GET http://localhost:8080/api/v1/users/me \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

### Check for expected error
```bash
if curl -s ... | jq -e '.error' > /dev/null; then
    echo "Got expected error"
else
    echo "Should have returned error"
    exit 1
fi
```

## Test Data

### Valid Ethereum Addresses (for testing)
```
0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
0x1234567890123456789012345678901234567890
0xabcdef0123456789abcdef0123456789abcdef01
```

### Test UUIDs
```bash
# Generate random test UUID
TEST_UUID=$(uuidgen)
```

## Adding Tests to E2E Script

To add a new test to `scripts/test-e2e.sh`:

```bash
# Test N: Description
echo ""
echo "Test N: Description"
test_info "Doing something..."
response=$(api_request "POST" "/endpoint" '{"data": "value"}' "")

if echo "$response" | grep -q "expected_value"; then
    test_pass "Test passed"
else
    test_fail "Test failed"
fi
```
