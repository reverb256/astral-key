# Astral Key — Task Completion Checklist

When you complete a development task, follow this checklist:

## 1. Build & Test

```bash
# Ensure code compiles
cargo check

# Run tests
cargo test --lib

# Run linter
cargo clippy -- -D warnings
```

## 2. Format Code

```bash
cargo fmt
```

## 3. Database Changes

If you modified the database schema:

```bash
cargo install sqlx-cli
sqlx migrate add -r <description>
```

Files go in `migrations/` directory. Migrations run automatically on startup.

## 4. Documentation

- [ ] Update `knowledge.md` if project structure or commands changed
- [ ] Update `docs/api.md` if endpoints changed
- [ ] Update `docs/errors.md` if error types changed
- [ ] Update `ROADMAP.md` if completing roadmap items

## 5. Commit

Follow conventional commit format and run `cargo fmt` before committing.
