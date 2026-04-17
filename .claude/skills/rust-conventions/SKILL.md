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
// Returns strongly-typed User struct
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
- No heavy CPU work - spawn to blocking task thread

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

### Integration tests in `tests/` directory
- Named `*_integration_tests.rs` or descriptively
- Test API endpoints and module interactions
- Use Testcontainers for external services

### Test naming: `test_<what>_<condition>_<expected>`
```rust
test_user_creation_with_valid_email_succeeds()
test_signature_verification_with_invalid_signature_fails()
test_session_refresh_with_expired_token_returns_error()
```

### Target: >80% code coverage

## Module Organization

```
src/
├── api/              # HTTP layer (handlers, routes)
├── auth/             # Authentication (jwt, web3, fido2)
├── db/               # Database (pool, models)
├── cache/            # Redis (pool, operations)
└── utils/            # Utilities
```

## Authentication-Specific Patterns

### JWT Middleware
- Protected routes use `AuthenticatedUser` extractor
- Token expiry: 15 min access, 7 day refresh
- Always check Redis blacklist for revoked tokens

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
/// Returns error if RNG fails or Redis storage fails.
///
/// # Examples
///
/// ```
/// let nonce = generate_nonce(&pool).await?;
/// assert!(nonce.len() == 32);
/// ```
pub async fn generate_nonce(pool: &RedisPool) -> Result<String> {
    // ...
}
```

## Performance Considerations

### Use connection pooling (already configured)
- Database: `sqlx::PgPool`
- Redis: `redis::Pool`

### Prefer batch operations over loops
```rust
// ❌ BAD - N queries
for id in ids {
    get_user(id).await?;
}

// ✅ GOOD - 1 query
get_users(&ids).await?;
```

### Cache frequently accessed data in Redis
- User sessions
- SIWE nonces (with TTL)
- Rate limit counters

## Security Patterns

### Input validation
- Use `validator` crate for struct validation
- Never trust client input
- Sanitize data before database queries

### Secrets management
- Use environment variables for secrets
- Never commit `.env` files
- Use `secrecy` crate for sensitive strings

### Rate limiting
- Use `governor` crate for rate limiting
- Store counters in Redis with TTL
