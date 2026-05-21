# Contributing to Astral Key

## Workflow

**Certification Gate**: Issues labeled `agent-ready` must be refined into `certified-ready` before work begins. This process converts conceptual tasks into high-signal implementation maps.
Every change follows: **Issue → Branch/Worktree → PR → Merge → Close Issue**

1. **Issues**: Every change needs a GitHub issue. No issue = no code.
2. **Worktrees**: Use `git worktree add`.
3. **Branch naming**: `issue-NNN-short-description`
4. **Commit messages**: `type: description (#NNN)` — conventional commits with issue ref.
5. **PR**: Push branch → create PR with `Closes #NNN`.

## Quality Gates & Review
- **Post-Implementation Review (PIR):** For high-impact changes, the author MUST run a PIR using the `/review-work` skill to verify architectural and security integrity.
- **Linear History:** All branches must be rebased onto the main branch before merge.
- **Naming:** Branches must follow `issue-NNN-description`.

## Coding Standards
- Rust 2021 edition, Async-first (Tokio).
- No `unwrap()` in production code.
- All public items must have rustdoc comments.
