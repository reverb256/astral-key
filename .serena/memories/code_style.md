# Astral Key - Code Style & Conventions

## Rust Conventions

### Edition & Style
- **Rust Edition:** 2021
- **Formatting:** `cargo fmt` (default rustfmt)
- **Linting:** `cargo clippy` - must pass with no warnings (`-D warnings`)
- **Documentation:** Public items must have rustdoc comments

### Code Organization

**Module-level documentation:**
```rust
//! Module-level documentation
//!
//! Detailed description of what this module does and how it fits into the system.
```

**Function documentation:**
```rust
/// Function documentation
///
/// # Errors
///
/// Returns error if...
///
/// # Examples
///
/// ```
/// let result = function();
/// assert!(result.is_ok());
/// ```
pub fn function() -> Result<()> {
    // ...
}
```

### Naming Conventions
- **Types:** `PascalCase` (e.g., `UserService`, `Web3Provider`)
- **Functions/Methods:** `snake_case` (e.g., `get_user`, `verify_signature`)
- **Constants:** `SCREAMING_SNAKE_CASE` (e.g., `MAX_SESSION_AGE`)
- **Modules:** `snake_case` (e.g., `web3`, `fido2`)

## Error Handling

### Use `?` Operator for Propagation
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

### Anti-Patterns to Avoid
- **NEVER** use `unwrap()` in production code (only in tests)
- **NO** blocking operations in async context
- **NO** hardcoded secrets (use environment variables)
- **NEVER** commit database changes without migration
- **NO** direct database access without connection pool
- **NEVER** skip authentication in protected routes

## Database Operations

### Use SQLx with Compile-Time Checked Queries
```rust
// Always create migrations first
// migrations/xxx_create_users.sql

// In code - use query_as for type-safe results
sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
    .bind(id)
    .fetch_one(&pool)
    .await?
```

### Use Transactions for Multi-Step Operations
```rust
let mut tx = pool.begin().await?;
// ... multiple operations ...
tx.commit().await?;
```

## Testing

### Unit Tests
- Place in same file as code using `#[cfg(test)]`
- Name tests descriptively: `test_<what>_<condition>_<expected>`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_user_creation_with_valid_email_succeeds() {
        let user = User::new("test@example.com");
        assert!(user.id.is_some());
    }
}
```

### Integration Tests
- Place in `tests/` directory
- Test API endpoints and module interactions
- Target: >80% code coverage

## Commit Messages

Follow **conventional commit** format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

**Examples:**
```
feat(auth): implement FIDO2 registration ceremony

Add WebAuthn registration flow using webauthn-rs.
Includes challenge generation and attestation verification.

Closes #123
```

```
fix(db): resolve connection pool exhaustion

Increase max_connections and add connection timeout.
```

## Project-Specific Patterns

### Authentication
- Protected routes use `AuthenticatedUser` extractor
- JWT tokens: 15 min access, 7 day refresh
- Always include blacklist check for revoked tokens

### Database Models
- All models use UUID primary keys
- Use `sqlx::FromRow` derive for query mapping
- Implement CRUD operations in model modules

### Error Types
- Custom errors in `src/error.rs` using `thiserror`
- Use `anyhow::Context` for adding context to external errors

### Configuration
- All config in `src/config.rs`
- Use environment variables for secrets
- Provide sensible defaults for non-sensitive values

## File Organization

```
src/
├── api/              # HTTP layer (handlers, routes, middleware)
├── auth/             # Authentication modules (jwt, web3, fido2)
├── db/               # Database layer (pool, models)
├── cache/            # Redis cache (pool, operations)
├── utils/            # Utilities
├── lib.rs            # Library exports
├── main.rs           # Binary entry point
├── error.rs          # Error types
├── config.rs         # Configuration
└── state.rs          # Application state
```

## Async/Await Guidelines

- Use `tokio::test` for async tests
- Always annotate async functions with `async fn`
- Use `.await` properly - don't block in async context
- Prefer `tokio::spawn` for concurrent operations when appropriate
