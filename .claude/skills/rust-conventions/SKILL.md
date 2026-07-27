---
name: rust-conventions
description: Astral Key specific Rust coding conventions and patterns
user-invocable: false
---

# Astral Key Rust Conventions

This skill encodes the specific Rust patterns and conventions used in the Astral Key project. Claude should follow these conventions when writing or modifying Rust code.

## Error Handling

### NEVER use `unwrap()` in production code
```rust
// ❌ BAD
let user = get_user(id).unwrap();

// ✅ GOOD
let user = get_user(id).await?
```

### Always use `?` operator for error propagation
```rust
pub async fn get_user(id: Uuid) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await?
}
```

### Add context with `anyhow::Context`
```rust
use anyhow::Context;

pub async fn get_user(id: Uuid) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .context("Failed to fetch user from database")?
}
```

## Database Operations (SQLx)

### Use `sqlx::query_as` for type-safe queries
```rust
sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
    .bind(id)
    .fetch_one(&pool)
    .await?
```

### Always create migrations before writing queries
Never modify database schema without first creating a migration file.

### Use transactions for multi-step operations
```rust
let mut tx = pool.begin().await?;
// ... multiple operations ...
tx.commit().await?;
```

### Model naming follows database table names
```rust
// Table: users → Model: User
// Table: web3_wallets → Model: Web3Wallet
// Table: fido2_credentials → Model: Fido2Credential
```

**Important:** Astral Key uses **SQLite only** — no PostgreSQL, no Redis. All SQL must be SQLite-compatible.

## Async/Await Patterns

### Use `#[tokio::test]` for async tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_user_creation() {
        let user = User::new("test@example.com");
        assert!(user.id.is_some());
    }
}
```

### Never block in async context
- No `std::thread::sleep` - use `tokio::time::sleep`
- No blocking I/O - use async equivalents
- No heavy CPU work - use `tokio::task::spawn_blocking`

## Testing Conventions

### Unit tests in same file with `#[cfg(test)]`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_<function>_<condition>_<expected>() {
        // test code
    }
}
```

### Test naming: `test_<what>_<condition>_<expected>`
```rust
test_user_creation_with_valid_email_succeeds()
test_signature_verification_with_invalid_signature_fails()
test_session_refresh_with_expired_token_returns_error()
```

## Module Organization

```
src/
├── api/              # HTTP layer (handlers, routes, middleware)
├── auth/             # Authentication (jwt, fido2, web3, jit, keys, capabilities, mcp)
├── db/               # SQLite (pool, models)
└── utils/            # Utilities
```

## Authentication-Specific Patterns

### JWT Middleware
- Protected routes use `AuthenticatedUser` extractor
- Token expiry: 15 min access, 7 day refresh (configurable via JWT_ env vars)

### In-memory state (no Redis)
- FIDO2 challenge state uses an in-memory `HashMap<String, (String, Instant)>` with TTL
- SIWE nonces are stored in SQLite with a 15-minute TTL
- Rate limiting uses an in-memory token bucket
- No Redis, no external cache is needed

### Cryptographic Operations
- Nonce generation must use cryptographically secure random
- Signature verification must handle all error cases
- Never log sensitive data (tokens, secrets, private keys)

## Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Types | `PascalCase` | `UserService`, `Web3Provider` |
| Functions/Methods | `snake_case` | `get_user`, `verify_signature` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SESSION_AGE` |
| Modules | `snake_case` | `web3`, `fido2` |

## Documentation

### Module-level docs use `//!`
```rust
//! Web3 authentication module
//!
//! Handles Sign-In with Ethereum (SIWE) flow including
//! nonce generation, message validation, and signature verification.
```

### Public items must have rustdoc
```rust
/// Generates a cryptographically secure nonce for SIWE
///
/// # Errors
///
/// Returns error if RNG or database write fails.
///
/// # Examples
///
/// ```
/// let nonce = generate_nonce(&state).await?;
/// assert!(nonce.len() == 64);
/// ```
pub async fn generate_nonce(state: &AppState) -> Result<String> {
    // ...
}
```

## Performance Considerations

### Use connection pooling (SQLite)
- Database: `sqlx::sqlite::SqlitePool` with `max_connections: 5` (configurable)

### Prefer batch operations over loops
```rust
// ❌ BAD - N queries
for id in ids {
    get_user(id).await?;
}

// ✅ GOOD - 1 query
get_users(&ids).await?;
```

## Security Patterns

### Input validation
- Validate all inputs with strict parsing
- Never trust client input
- Use parameterized queries (SQLx does this automatically)

### Secrets management
- Use environment variables for all secrets
- `JWT_SECRET` is required (≥32 bytes)
- Never commit `.env` files
- Never log secrets

### Rate limiting
- In-memory token bucket (not Redis)
- Default: 100 requests/minute, 200 burst
- Returns `429 Too Many Requests` with `Retry-After` header
