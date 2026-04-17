# Astral Key - Task Completion Checklist

When you complete a development task, follow this checklist:

## 1. Build & Test

```bash
# Ensure code compiles
cargo build

# Run all tests
just test
# or: cargo test --all-features

# Run linter (must pass with no warnings)
just lint
# or: cargo clippy --all-features -- -D warnings
```

## 2. Format Code

```bash
# Format Rust code
just fmt
# or: cargo fmt && nixpkgs-fmt .
```

## 3. Database Changes

If you modified the database schema:

```bash
# Create migration
just migrate-new <descriptive_name>

# Run migration to verify
just migrate
```

**Never commit database changes without a migration!**

## 4. Documentation

- [ ] Update relevant module documentation (`///` comments)
- [ ] Update public API docs (rustdoc)
- [ ] Update STATUS.md if implementing roadmap items
- [ ] Update TESTING.md if adding new test coverage

## 5. Commit

Follow conventional commit format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Run pre-commit checks (if configured):
```bash
just pre-commit
```

## 6. Before Opening PR

```bash
# Full test suite
just test

# Linting
just lint

# Format check
cargo fmt --check

# Security audit
just audit

# Documentation builds
cargo doc --no-deps
```

## Quality Gates

- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code formatted
- [ ] Documentation updated
- [ ] Database migrations included (if applicable)
- [ ] No new security vulnerabilities (`cargo audit`)
- [ ] Commit message follows conventions

## Specific Checks by Task Type

### Feature Addition
- [ ] Feature flag in Cargo.toml (if needed)
- [ ] Integration tests added
- [ ] API documentation updated
- [ ] STATUS.md updated

### Bug Fix
- [ ] Regression test added
- [ ] Root cause documented in commit

### Refactoring
- [ ] Tests pass before and after
- [ ] No behavior changes

### Database Changes
- [ ] Migration reversible
- [ ] Tested with fresh database
- [ ] Model types updated

### Authentication Changes
- [ ] Security implications reviewed
- [ ] Token blacklist considered
- [ ] Session management updated

## After Merge

- [ ] Delete feature branch
- [ ] Update ROADMAP.md if needed
- [ ] Close related issues
