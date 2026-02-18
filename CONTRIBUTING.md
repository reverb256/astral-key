# Contributing to Astral Key

Thank you for your interest in contributing to Astral Key! This document provides guidelines and instructions for contributing.

## Project Status

**IMPORTANT:** Astral Key is currently in **early prototype stage** (~5% complete). The architecture is designed, but most core features are not yet implemented. See [STATUS.md](STATUS.md) for current progress and [ROADMAP.md](ROADMAP.md) for the implementation plan.

## Ways to Contribute

We welcome contributions in many forms:

### Code Contributions

1. **Pick a task** from [ROADMAP.md](ROADMAP.md) or [STATUS.md](STATUS.md)
2. **Check for existing issues** or create one to discuss your approach
3. **Write code** following our coding standards (below)
4. **Add tests** for your changes
5. **Update documentation** as needed
6. **Submit a pull request**

### Documentation

- Improve existing documentation
- Add examples and tutorials
- Fix typos and clarify explanations
- Translate documentation

### Testing

- Write unit tests
- Write integration tests
- Report bugs with reproduction steps
- Test on different platforms

### Code Review

- Review pull requests
- Suggest improvements
- Catch security issues

## Development Setup

### Prerequisites

- **Nix** with flakes enabled (recommended)
- **Rust** 1.75+ (if not using Nix)
- **Docker** or **Podman** for local services

### Using Nix (Recommended)

```bash
# Clone the repository
git clone https://github.com/reverb256/astral-key.git
cd astral-key

# Enter development environment
nix develop

# Or with direnv
direnv allow
```

### Without Nix

```bash
# Install Rust via rustup
rustup install stable
rustup default stable

# Install system dependencies (Ubuntu/Debian)
sudo apt-get install -y postgresql redis-server protobuf-compiler pkg-config libssl-dev

# Install development tools
cargo install cargo-watch cargo-edit sqlx-cli
```

### Starting Local Services

```bash
# Start PostgreSQL and Redis
just db-up

# Run database migrations
just migrate

# Start development server
just dev
```

## Coding Standards

### Rust Conventions

- **Edition:** Rust 2021
- **Style:** `cargo fmt` (default rustfmt)
- **Lints:** `cargo clippy` - must pass with no warnings
- **Documentation:** Public items must have rustdoc comments

```rust
//! Module-level documentation

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

### Error Handling

- Use `?` operator for error propagation
- Never use `unwrap()` in production code (only in tests)
- Provide context with `.context()` from anyhow

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

### Database Queries

- Use SQLx with compile-time checked queries
- Always create migrations before writing queries
- Use transactions for multi-step operations

```rust
// In migrations/001_initial.sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ...
);

// In code
sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
    .bind(id)
    .fetch_one(&pool)
    .await?
```

### Testing

- **Unit tests:** In the same file as the code
- **Integration tests:** In `tests/` directory
- **Target:** >80% code coverage

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

### Anti-Patterns to Avoid

- **NO** blocking operations in async context
- **NEVER** use `unwrap()` outside tests
- **NO** hardcoded secrets (use environment variables)
- **NEVER** commit database changes without migration
- **NO** direct database access without connection pool
- **NEVER** skip authentication in protected routes

## Commit Messages

Follow conventional commit format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Test changes
- `chore`: Build/process changes

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

## Pull Request Process

1. **Fork** the repository
2. **Create a branch** for your changes
   ```bash
   git checkout -b feat/your-feature-name
   ```
3. **Make changes** following coding standards
4. **Run tests and lints**
   ```bash
   just test
   just lint
   just fmt
   ```
5. **Commit** with clear message
6. **Push** to your fork
7. **Open pull request** with:
   - Description of changes
   - Reference to related issues
   - Screenshots if applicable
8. **Address review feedback**

### PR Checklist

- [ ] Code follows project style guidelines
- [ ] Tests pass locally (`just test`)
- [ ] Lints pass (`just lint`)
- [ ] Documentation updated
- [ ] Commit messages follow conventions
- [ ] No merge conflicts

## Project Structure

```
astral-key/
├── src/
│   ├── api/              # HTTP layer
│   ├── auth/             # Authentication modules
│   ├── db/               # Database layer
│   ├── cache/            # Redis cache
│   └── utils/            # Utilities
├── migrations/           # SQL migrations
├── tests/                # Integration tests
├── docs/                 # Documentation
└── nix/                  # Nix configuration
```

## Priority Areas

We're currently focusing on:

1. **FIDO2/WebAuthn Implementation** (Week 4-6) - Primary focus
2. **Web3 SIWE Implementation** (Week 6-7)
3. **Database Models** (Week 2)
4. **Testing** (Week 9-10)

See [ROADMAP.md](ROADMAP.md) for full timeline.

## Questions?

- **GitHub Issues:** For bugs and feature requests
- **Discussions:** For questions and ideas
- **Pull Requests:** For code contributions

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).

## Code of Conduct

Be respectful, inclusive, and collaborative. We're all here to build something great together.

---

Thank you for contributing to Astral Key! 🚀
