# API Test Generator

A specialized subagent for generating API integration tests for new endpoints in the Astral Key project.

## Purpose

Generate comprehensive integration tests for new API endpoints, including success cases, error cases, and edge cases. Tests are added to both the e2e bash script and Rust integration tests.

## Test Generation Guidelines

### For Each New Endpoint

Generate tests covering:

1. **Success Case** - Valid request returns expected response
2. **Authentication** - Protected routes reject unauthenticated requests
3. **Validation** - Invalid input returns appropriate error
4. **Edge Cases** - Boundary conditions and special values
5. **Error Cases** - Server errors handled gracefully

## Test Templates

### GET Endpoint (Public)

```bash
# Test: GET /public/endpoint
echo ""
echo "Test: Public GET endpoint"
if api_request "GET" "/public/endpoint" "" "" | grep -q "expected_field"; then
    test_pass "Public GET returns expected data"
else
    test_fail "Public GET failed"
fi
```

### POST Endpoint (Public)

```bash
# Test: POST /public/endpoint
echo ""
echo "Test: Public POST endpoint"
test_info "Creating resource with valid data..."
response=$(api_request "POST" "/public/endpoint" '{"field": "value"}' "")

if echo "$response" | grep -q "id"; then
    test_pass "Resource created successfully"
    resource_id=$(echo "$response" | jq -r '.id')
else
    test_fail "Failed to create resource"
fi
```

### GET Endpoint (Protected)

```bash
# Test: Protected GET without auth
echo ""
echo "Test: Protected endpoint rejects unauthenticated"
protected_response=$(api_request "GET" "/api/v1/protected" "" "")

if echo "$protected_response" | grep -q "unauthorized\|missing"; then
    test_pass "Correctly rejects unauthenticated request"
else
    test_fail "Should require authentication"
fi

# Test: Protected GET with auth
echo ""
echo "Test: Protected endpoint accepts authenticated"
if [ -n "$ACCESS_TOKEN" ]; then
    auth_response=$(api_request "GET" "/api/v1/protected" "" "$ACCESS_TOKEN")

    if echo "$auth_response" | grep -q "expected_data"; then
        test_pass "Authenticated request succeeds"
    else
        test_fail "Authenticated request failed"
    fi
fi
```

### POST Endpoint (Protected)

```bash
# Test: Protected POST with valid auth
echo ""
echo "Test: Protected POST with authentication"
if [ -n "$ACCESS_TOKEN" ]; then
    create_response=$(api_request "POST" "/api/v1/protected/resource" \
        '{"name": "test"}' \
        "$ACCESS_TOKEN")

    if echo "$create_response" | grep -q "id\|created"; then
        test_pass "Protected resource created"
    else
        test_fail "Failed to create protected resource"
    fi
fi
```

### DELETE Endpoint (Protected)

```bash
# Test: DELETE endpoint
echo ""
echo "Test: Delete resource"
if [ -n "$ACCESS_TOKEN" ] && [ -n "$resource_id" ]; then
    delete_response=$(api_request "DELETE" "/api/v1/resource/$resource_id" "" "$ACCESS_TOKEN")

    if echo "$delete_response" | grep -q "deleted\|success\|204"; then
        test_pass "Resource deleted successfully"
    else
        test_fail "Failed to delete resource"
    fi
fi
```

### Validation Error Tests

```bash
# Test: Invalid input validation
echo ""
echo "Test: Invalid input rejected"
invalid_response=$(api_request "POST" "/api/v1/resource" \
    '{"invalid": "data"}' \
    "")

if echo "$invalid_response" | grep -q "error\|validation\|invalid"; then
    test_pass "Invalid input correctly rejected"
else
    test_fail "Should reject invalid input"
fi
```

## Rust Integration Test Template

```rust
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_endpoint_success() {
    let client = Client::new();
    let api_base = std::env::var("API_BASE")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let response = client
        .get(format!("{}/api/v1/endpoint", api_base))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse response");

    assert!(body["data"].is_object());
}

#[tokio::test]
async fn test_endpoint_unauthorized() {
    let client = Client::new();
    let api_base = std::env::var("API_BASE")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let response = client
        .get(format!("{}/api/v1/protected", api_base))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_endpoint_validation_error() {
    let client = Client::new();
    let api_base = std::env::var("API_BASE")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let response = client
        .post(format!("{}/api/v1/resource", api_base))
        .json(&json!({"invalid": "data"}))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse response");

    assert!(body["error"].is_string());
}
```

## Adding Tests to E2E Script

When a new endpoint is added, append tests to `scripts/test-e2e.sh`:

```bash
# Test N: [Description]
echo ""
echo "Test N: [Description]"
test_info "[What is being tested]..."
response=$(api_request "METHOD" "/endpoint" '{"data": "value"}' "TOKEN")

if echo "$response" | grep -q "expected_value"; then
    test_pass "Test passed"
else
    test_fail "Test failed"
fi
```

## Test Data Helpers

### Generate Test Data
```bash
# Generate random UUID
TEST_UUID=$(uuidgen)

# Generate random string
RANDOM_STRING=$(openssl rand -hex 16)

# Generate test address
TEST_ADDRESS="0x$(openssl rand -hex 20)"
```

### Extract Values from Responses
```bash
# Extract ID from response
ID=$(echo "$response" | jq -r '.id')

# Extract token
TOKEN=$(echo "$response" | jq -r '.access_token')

# Extract nested value
VALUE=$(echo "$response" | jq -r '.data.field')
```

## Test Coverage Tracking

Mark tests as added for each endpoint:

- [ ] GET /health
- [ ] GET /ready
- [ ] POST /api/v1/auth/web3/nonce
- [ ] POST /api/v1/auth/web3/verify
- [ ] POST /api/v1/auth/fido2/authenticate/options
- [ ] POST /api/v1/auth/fido2/authenticate/verify
- [ ] POST /api/v1/auth/fido2/register/options (protected)
- [ ] POST /api/v1/auth/fido2/register/verify (protected)
- [ ] GET /api/v1/auth/fido2/credentials (protected)
- [ ] DELETE /api/v1/auth/fido2/credentials/:id (protected)
- [ ] POST /api/v1/sessions/refresh
- [ ] GET /api/v1/sessions (protected)
- [ ] DELETE /api/v1/sessions/current (protected)
- [ ] GET /api/v1/users/me (protected)

## Trigger Conditions

This agent should be invoked when:
- New API endpoints are added in `src/api/routes.rs`
- New handlers are added in `src/api/handlers/`
- Authentication requirements change
- Response formats change
