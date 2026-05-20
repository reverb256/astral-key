---
name: Agent task
description: Well-scoped task for autonomous agent execution via Kelos/kagent
title: ""
labels: agent-ready
assignees: ""
---

## Task
One-line summary of what needs to be done.

## Context for Agent
**Fill this section so an autonomous agent can execute without asking questions.**

- **Pre-flight checks:** What services/state must be verified before starting?
- **What to inspect first:** Files, logs, or endpoints to read before writing code
- **What NOT to touch:** Files or directories explicitly out of scope
- **Previous attempts:** Is there feedback from a prior attempt to learn from?

## Background
Why is this needed? What's the business or technical reason? Link to any related roadmap or EPIC.

## Scope
### In scope
- Specific files/directories
- Specific changes

### Out of scope
- Everything not listed above
- Related-but-separate concerns (they have their own issues)

## Dependencies
- **Blocks on:** #
- **Blocked by:** #
- **Related issues:** #

## Required changes
List each specific change, concretely enough for autonomous execution:

1. `path/to/file.py` — description of change
2. `path/to/other.nix` — description of change

## Clean long-term solution mandate
Agents MUST implement the cleanest long-term fix, not a workaround:

- **Fix root causes, not symptoms** — Address the underlying problem
- **Nix-native** — All deployment config in Nix modules. No raw k8s YAML or wrapper scripts
- **Gateway routing** — AI traffic goes through the AI Inference Gateway, not directly to providers
- **Test coverage** — Include tests that prove the fix works and won't regress
- **Zero tech debt** — No `# TODO: fix later`, no workarounds, no half-measures
- **Documentation** — Update AGENTS.md if new patterns or conventions are introduced

## Steps
- [ ] Step 1
- [ ] Step 2
- [ ] Verification

## Acceptance Criteria
- [ ] Changes applied and working
- [ ] Build passes
- [ ] Tests pass
- [ ] PR links back to this issue with `Closes #NNN`
- [ ] AGENTS.md updated if new patterns introduced

## Workflow
- [ ] Single PR per task
- [ ] Work in worktree: `git worktree add -b issue-NNN-desc /data/projects/own/REPO-NNN main`
- [ ] Branch: `issue-NNN-short-description`
- [ ] Commit messages reference `(#NNN)`
- [ ] PR body contains `Closes #NNN`
- [ ] PR reviewed before merge (even solo)

## Reference
- Related issue: #
- ROADMAP.md section:
