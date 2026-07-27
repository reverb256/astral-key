# Astral Key — Code Style & Conventions

## Rust Conventions

### Edition & Style
- **Rust Edition:** 2021
- **Formatting:** `cargo fmt` (default rustfmt)
- **Linting:** `cargo clippy` — must pass with no warnings (`-D warnings`)
- **Documentation:** Public items should have rustdoc comments

### Code Organization

**Module-level documentation:**
```rust
//! Module-level documentation
//!
//! Detailed description of what this module does.
```

**Function documentation:**
```rust
/// Function documentation
///
/// # Errors
///
/// Returns error if...
pub fn function() -> Result<()> {
    // ...
}
```

### Naming Conventions
- **Types:** `PascalCase`
- **Functions/Methods:** `snake_case`
- **Constants:** `SCREAMING_SNAKE_CASE`
- **Modules:** `snake_case`

## Error Handling

- Use `?` operator for propagation
- Use `thiserror` for library errors, `anyhow` for application-level
- **NEVER** use `unwrap()` or `expect()` in production code (tests only)
- **NO** blocking operations in async context
- **NO** hardcoded secrets (use environment variables)

## Database Operations

- SQLite via sqlx (no PostgreSQL, no Redis)
- All migrations in `migrations/` — applied automatically on server start
- Use `sqlx::query_as` for type-safe queries
- Use transactions for multi-step operations
- SQLite-compatible SQL only

## Testing

- Unit tests live next to code: `#[cfg(test)] mod tests`
- Name tests descriptively: `test_<what>_<condition>_<expected>`
- Integration tests go in `tests/` (not yet implemented)

## Commit Messages

Follow conventional commit format:

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

## Project-Specific Patterns

### Authentication
- Protected routes use `AuthenticatedUser` extractor
- JWT tokens: 15 min access, 7 day refresh (configurable via env only)
- Refresh token rotation on each use

### Database Models
- All models use UUID primary keys
- Use `sqlx::FromRow` derive for query mapping
- Implement CRUD in model modules

### Error Types
- Custom `AuthError` enum in `src/error.rs` (string error codes)
- `into_response` maps to HTTP status codes automatically

### Configuration
- All config in `src/config.rs` via environment variables
- No config files (TOML/YAML/INI)
- `JWT_SECRET` required on startup (panics if missing or <32 bytes)

## File Organization

```
src/
├── api/              # HTTP layer (handlers, routes, middleware)
├── auth/             # Auth modules (jwt, fido2, web3, jit, keys, capabilities, mcp)
├── db/               # SQLite pool + models
├── utils/            # Utilities
├── lib.rs            # Library exports
├── main.rs           # Binary entry point
├── error.rs          # Error types
├── config.rs         # Configuration
└── state.rs          # AppState
```

## Async/Await Guidelines

- Use `tokio::test` for async tests
- Avoid `std::thread::spawn`
- Use `tokio::spawn` for concurrent operations when appropriate
- Always prefer `.await` over blocking calls
