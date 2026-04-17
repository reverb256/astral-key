# Astral Key - Claude Code Automations

This directory contains all Claude Code customizations for the Astral Key project.

## What's Configured

### Skills (3)
| Skill | Type | Purpose |
|-------|------|---------|
| `create-migration` | User-only | Generate SQLx migrations with validation |
| `rust-conventions` | Claude-only | Encode project Rust patterns |
| `test-auth-flow` | Both | Generate auth API tests |

### Agents (2)
| Agent | Purpose |
|-------|---------|
| `crypto-reviewer` | Security review of auth/crypto code |
| `api-tester` | Generate integration tests for new endpoints |

### Hooks (3)
| Hook | Trigger | Action |
|------|---------|--------|
| Auto-format | Post Edit/Write | Run `cargo fmt` |
| Run tests | Post Edit/Write | Run module tests (background) |
| Block sensitive | Pre Edit/Write | Require confirmation |

### MCP Servers (2)
| Server | Purpose |
|--------|---------|
| context7 | Live docs for Rust libraries |
| GitHub | Issue/PR/CI integration |

## Quick Start

1. Install prerequisites:
```bash
npm install -g npx  # For context7 MCP
```

2. The automations are active automatically when using Claude Code in this project.

3. Invoke user skills:
   - `/create-migration` - Create a database migration
   - `/test-auth-flow` - Generate auth tests

## Documentation

See `CLAUDE.md` for detailed documentation.
