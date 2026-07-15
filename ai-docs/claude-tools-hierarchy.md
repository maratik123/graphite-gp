# Project Tool / Subagent / Skill / Hook inventory

Project-defined harness surfaces. When adding a new project-defined name, grep this file first and pick a non-clashing name (AGENTS.md § Propagation Rule name-clash AXIOM).

## Subagents (`.claude/agents/`)
| Name | Spawned by | Role |
|------|-----------|------|
| `spec-writer` | `/interview` | Drafts the task spec, one round per call |
| `design` | `/task` Step 6 | Investigates code, produces the design doc |
| `design-review` | `/task` Step 7 | GO / ITERATE / STOP verdict on the design |
| `code-writer` | `/task` code group (via `/context-reset` step 3); `/bugfix` Step 5; `/main-ci-failed` Step 4; `/pr-ci-failed` Step 4; `/pr-commented` Step 4 | Code-writing implementor; frontmatter-pinned `model: sonnet` + `effort: medium`. **Mode A** — `/task` group-implementor (read `.progress.md`, do subtasks sequentially, gate + commit per subtask). **Mode B** — single-fix delegate (author the orchestrator's planned fix, gate, return WITHOUT committing). Never runs self-review, never pushes. |
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
- **Per-group implementor model+effort contract (PART 5).** The per-group implementor spawn — `/context-reset` § Handoff-protocol step 3, driven by `/task` Step 8 — is selected by the design's `## Handoff plan` group marker: a **code** group → `subagent_type="code-writer"`, whose `model: sonnet` + `effort: medium` are **frontmatter-pinned** (pass NO inline `model=`/effort override — there is no per-invocation `effort` parameter, so the file is the only lever that enforces the medium tier); an **instructions/harness** group → `subagent_type="general-purpose"` with inline `model="opus"` + inherited effort (typically xHigh, not pinned); both use the 1M-token window. The orchestrator model itself is unchanged (per-invocation). The quality-gate subagents (`design` / `design-review` / `self-review` / `spec-writer`) keep their Opus contract — only the per-group implementor spawn varies per group.
