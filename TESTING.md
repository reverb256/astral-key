# Astral Key - Testing Guide

This document describes how to test the Astral Key authentication microservice.

## Prerequisites

Before running tests, ensure you have:

1. **Docker and Docker Compose** installed
2. **Nix** (optional, for reproducible builds)
3. **make** or **just** command runner

## Setting Up Test Environment

### Start Required Services

```bash
# Start PostgreSQL, Redis, and Vaultwarden
docker-compose up -d

# Verify services are running
docker-compose ps
```

### Run Database Migrations

```bash
# Using just (if available)
just migrate

# Or using sqlx directly
sqlx migrate run --database-url postgresql://postgres:postgres@localhost/astral_key

# Or with Docker
docker-compose exec -T astral-key sqlx migrate run --database-url postgresql://postgres:postgres@localhost/astral_key
```

### Start the Application

```bash
# Development mode
cargo run

# Or using just
just dev

# Or with Nix
nix-shell --pure --run 'cargo run'
```

## Running Tests

### Unit Tests

Run unit tests for individual modules:

```bash
# Run all unit tests
cargo test --lib

# Run tests with output
cargo test --lib -- --nocapture

# Run tests for specific module
cargo test --lib jwt::
cargo test --lib web3::
cargo test --lib fido2::
```

### Integration Tests

Run API integration tests:

```bash
# Run all integration tests
cargo test --test api_integration_tests

# Run specific test
cargo test --test api_integration_tests test_health_endpoint
```

### End-to-End Tests

Run the comprehensive e2e test script:

```bash
# Run all e2e tests
./scripts/test-e2e.sh

# With custom API base URL
API_BASE=http://localhost:8080 ./scripts/test-e2e.sh
```

The e2e test script covers:
- Health and readiness checks
- Web3 nonce generation
- Web3 signature verification
- FIDO2 authentication options
- Protected route authentication
- Session refresh
- CORS handling

## Manual Testing

### Test Web3 Authentication Flow

```bash
# 1. Request a nonce
curl -X POST http://localhost:8080/api/v1/auth/web3/nonce \
  -H "Content-Type: application/json" \
  -d '{
    "domain": "localhost",
    "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
    "chain_id": 1
  }'

# 2. Sign the SIWE message with your Ethereum wallet
# (Use the nonce and message_template from step 1)

# 3. Verify signature and get JWT tokens
curl -X POST http://localhost:8080/api/v1/auth/web3/verify \
  -H "Content-Type: application/json" \
  -d '{
    "message": "<signed message>",
    "signature": "<signature>",
    "chain_id": 1
  }'

# 4. Use the access_token from step 3
curl -X GET http://localhost:8080/api/v1/users/me \
  -H "Authorization: Bearer <access_token>"
```

### Test FIDO2/Passkey Authentication Flow

```bash
# 1. Get authentication options
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/options \
  -H "Content-Type: application/json" \
  -d '{
    "username": "<user_uuid>"
  }'

# 2. Use the challenge with WebAuthn API
# (navigator.credentials.get())

# 3. Verify assertion and get JWT tokens
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/verify \
  -H "Content-Type: application/json" \
  -d '{
    "id": "<credential_id>",
    "raw_id": "<raw_credential_id>",
    "response": {
      "client_data_json": "<...>",
      "authenticator_data": "<...>",
      "signature": "<...>"
    },
    "type": "public-key"
  }'
```

### Test Session Management

```bash
# Refresh tokens
curl -X POST http://localhost:8080/api/v1/sessions/refresh \
  -H "Content-Type: application/json" \
  -d '{
    "refresh_token": "<refresh_token>"
  }'

# List active sessions
curl -X GET http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer <access_token>"

# Logout (revoke session)
curl -X DELETE http://localhost:8080/api/v1/sessions/current \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{
    "refresh_token": "<refresh_token>"
  }'
```

### Test FIDO2 Registration (Requires Authentication)

```bash
# Get registration options
curl -X POST http://localhost:8080/api/v1/auth/fido2/register/options \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{
    "username": "mykey",
    "display_name": "My Security Key"
  }'

# Use the challenge with WebAuthn API
# (navigator.credentials.create())

# Complete registration
curl -X POST http://localhost:8080/api/v1/auth/fido2/register/verify \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{
    "id": "<credential_id>",
    "raw_id": "<raw_credential_id>",
    "response": {
      "client_data_json": "<...>",
      "attestation_object": "<...>"
    },
    "type": "public-key"
  }'

# List registered credentials
curl -X GET http://localhost:8080/api/v1/auth/fido2/credentials \
  -H "Authorization: Bearer <access_token>"
```

## Test Coverage

### Current Coverage

- ✅ JWT token generation and validation
- ✅ Web3 nonce generation
- ✅ SIWE message parsing
- ✅ Database model operations
- ✅ Cache operations
- ⚠️ Web3 signature verification (needs real Ethereum signature)
- ⚠️ FIDO2 attestation (needs WebAuthn authenticator)

### Running Coverage Analysis

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage analysis
cargo tarpaulin --out Html

# View report
open html/index.html
```

## CI/CD Testing

Tests run automatically on:
- Every pull request
- Push to main or develop branches
- Manual workflow dispatch

See `.github/workflows/ci.yml` for CI/CD configuration.

## Troubleshooting

### Tests Fail with "Connection Refused"

Ensure the server is running:
```bash
cargo run
```

### Tests Fail with "Database Unavailable"

Ensure PostgreSQL is running and migrations are applied:
```bash
docker-compose up -d
just migrate
```

### Tests Fail with "Redis Unavailable"

Ensure Redis is running:
```bash
docker-compose up -d
```

### Integration Tests Time Out

Increase timeout in test configuration or check service health:
```bash
curl http://localhost:8080/health
curl http://localhost:8080/ready
```

## Continuous Testing

For rapid development feedback:

```bash
# Watch mode (requires cargo-watch)
cargo install cargo-watch
cargo watch -x test

# Test with auto-run on save
cargo watch -x 'test --lib'
```
