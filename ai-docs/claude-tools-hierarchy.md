# Project Tool / Subagent / Skill / Hook inventory

Project-defined harness surfaces. When adding a new project-defined name, grep this file first and pick a non-clashing name (AGENTS.md § Propagation Rule name-clash AXIOM).

## Subagents (`.claude/agents/`)
| Name | Spawned by | Role |
|------|-----------|------|
| `spec-writer` | `/interview` | Drafts the task spec, one round per call |
| `design` | `/task` Step 6 | Investigates code, produces the design doc |
| `design-review` | `/task` Step 7 | GO / ITERATE / STOP verdict on the design |
| `self-review` | `/task` Step 10, `/project-review`, CI/PR skills, `/bugfix` | Skeptical pre-push reviewer (REJECT gates) |
| `review-findings` | `/project-review` | Whole-codebase review variant |
| `self-improve` | `/improve` | Escalates learnings → rules/hooks |
| `learnings-escalation-audit` | `/ai-audit` Phase 1 | Verifies `Escalated?` accuracy; flags stale validations |
| `triage-runner` | `/triage` | Batch-promotes deferred rows to gh issues |

## Skills (`.claude/skills/`)
`/task`, `/interview`, `/next`, `/improve`, `/ai-audit`, `/project-review`, `/bugfix`, `/verify-change`, `/context-reset`, `/triage`, `/pr-commented`, `/pr-ci-failed`, `/main-ci-failed`, `/pr-merged`, `/dependabot-pr`.

## Hooks (`.claude/settings.json`)
| Event | Purpose |
|-------|---------|
| SessionStart | `ast-index` update/rebuild; read-rules reminder |
| PreToolUse (Bash `git commit`) | **branch-guard** — block commits on `main` (`exit 2` + recovery recipe); refresh `ast-index` |
| PostToolUse (Write\|Edit) | `cargo fmt` on `.rs`; **panic-gate** — flag `.unwrap()/.expect(/panic!` outside `#[cfg(test)]` |
| PostToolUse (Bash `git push`) | **pr-sync** — remind to re-read/sync the open PR body |

## Notes
- Default branch is `main` (not `master`).
- No CI workflows / `ROADMAP.md` / design-system yet — the `/pr-ci-failed`, `/main-ci-failed`, `/dependabot-pr` skills are dormant until `.github/workflows/*` and dependencies exist.
