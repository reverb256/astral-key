# Astral Key - Claude Code Configuration

This directory contains Claude Code automations, skills, hooks, and agents configured for the Astral Key project.

## Configuration Files

| File | Purpose |
|------|---------|
| `settings.json` | Hooks (auto-format, sensitive file protection) and permissions |
| `mcp.json` | MCP server connections (context7, GitHub) |
| `CLAUDE.md` | This file - documentation for Claude automations |

## Skills (`skills/`)

Skills are reusable workflows that can be invoked by Claude or by users via `/skill-name`.

### `create-migration` (User-only)
Generates SQLx database migration files with proper naming conventions and templates.

**Usage:** Invoke this skill when creating a new database migration.

**Features:**
- Validates snake_case naming
- Checks for duplicate migration names
- Auto-increments migration number
- Includes UP/DOWN migration template

**Script:** `create-migration/create.sh`

### `rust-conventions` (Claude-only)
Encodes Astral Key's specific Rust patterns and coding conventions.

**Usage:** Claude automatically references this when writing Rust code.

**Covers:**
- Error handling patterns (no unwrap, anyhow::Context)
- Database operations (SQLx patterns)
- Async/await best practices
- Testing conventions
- Module organization
- Security patterns

### `test-auth-flow` (Both)
Generates API test cases for Web3, FIDO2, and JWT authentication flows.

**Usage:** Invoke when adding new auth endpoints or testing existing flows.

**Provides:**
- curl command templates for each auth flow
- Rust integration test templates
- Test data helpers
- E2E script additions

## Agents (`agents/`)

Agents are specialized subagents that run in parallel to review or analyze code.

### `crypto-reviewer`
Reviews authentication and cryptographic code for security vulnerabilities.

**Trigger:** Automatically invoked for changes to:
- `src/auth/` directory
- `src/utils/crypto.rs`
- Token/session logic changes

**Reviews:**
- Web3/SIWE nonce generation and signature verification
- FIDO2/WebAuthn challenge and attestation verification
- JWT token security and secret management
- Timing attack vulnerabilities
- Secret logging prevention

### `api-tester`
Generates integration tests for new API endpoints.

**Trigger:** Automatically invoked for changes to:
- `src/api/routes.rs`
- `src/api/handlers/` directory
- New endpoint additions

**Generates:**
- Success and failure test cases
- Authentication/authorization tests
- Validation error tests
- E2e bash script additions
- Rust integration test scaffolding

## Hooks (`settings.json`)

Hooks are automatic actions triggered by tool events.

### PostToolUse Hooks

1. **Auto-format** (`cargo fmt`)
   - Runs after any `Edit` or `Write` operation
   - Keeps code formatted consistently with CI requirements

2. **Run Related Tests** (background)
   - Runs tests for the modified module
   - Non-blocking: doesn't wait for completion
   - Provides immediate feedback on test failures

### PreToolUse Hooks

1. **Sensitive File Protection**
   - Requires user confirmation before editing sensitive files
   - Protected files: `.env*`, `*.key`, `*.pem`, `Cargo.lock`
   - Callback: `UserPromptSubmit`

## MCP Servers (`mcp.json`)

### context7
Live documentation lookup for Rust libraries.

**Provides:**
- Up-to-date API docs for axum, sqlx, ethers-rs, webauthn-rs, tokio
- Code examples from official documentation
- Quick reference for library-specific patterns

**Install:**
```bash
npx -y @modelcontextprotocol/server-context7
```

### GitHub
GitHub integration for repository operations.

**Provides:**
- Issue and PR management
- CI/CD status checks
- Repository search
- Action triggers

**Requires:** `gh` CLI installed and authenticated

## Quick Reference

### For Users

| Action | Command |
|--------|---------|
| Create migration | Invoke `create-migration` skill |
| Test auth flow | Invoke `test-auth-flow` skill |
| Check settings | View `.claude/settings.json` |

### For Claude

Claude will automatically:
- Reference `rust-conventions` when writing code
- Invoke `crypto-reviewer` for auth changes
- Invoke `api-tester` for API changes
- Run `cargo fmt` after edits
- Run related tests after edits (background)

## File Permissions

The following files are **protected** (require confirmation):
- `.env*` - Environment configuration
- `*.key` - Private key files
- `*.pem`, `*.p12` - Certificates and keys
- `secrets/*` - Secret directories
- `Cargo.lock` - Dependency lock file

## Environment

The following environment variable is set for Claude:
- `RUST_BACKTRACE=1` - Full backtraces for debugging

## Customization

To add new automations:

1. **New Skill**: Create `.claude/skills/<name>/SKILL.md`
2. **New Agent**: Create `.claude/agents/<name>.md`
3. **New Hook**: Add to `hooks` in `.claude/settings.json`
4. **New MCP Server**: Add to `mcpServers` in `.claude/mcp.json`

## Troubleshooting

### Hooks not running
- Check `.claude/settings.json` syntax
- Verify commands are in PATH
- Check permissions for allowed commands

### MCP servers not connecting
- Verify `npx` is installed
- Check network connectivity
- Review MCP server logs with `--mcp-debug`

### Skills not found
- Ensure `SKILL.md` file exists in skill directory
- Check frontmatter YAML is valid
- Verify skill name matches directory name

## Contributing

When adding new automations:
1. Document in this CLAUDE.md
2. Follow existing patterns
3. Test before committing
4. Update this README
