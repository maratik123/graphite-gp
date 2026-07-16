# Project Tool / Subagent / Skill / Hook inventory

Project-defined harness surfaces. When adding a new project-defined name, grep this file first and pick a non-clashing name (AGENTS.md § Propagation Rule name-clash AXIOM).

## Subagents (`.claude/agents/`)
| Name | Spawned by | Role |
|------|-----------|------|
| `spec-writer` | `/interview` | Drafts the task spec, one round per call |
| `design` | `/task` Step 6 | Investigates code, produces the design doc |
| `design-review` | `/task` Step 7 | GO / ITERATE / STOP verdict on the design |
| `code-writer` | `/task` code group (via `/context-reset` step 3); `/bugfix` Step 5; `/main-ci-failed` Step 4; `/pr-ci-failed` Step 4; `/pr-commented` Step 4 | Code-writing implementor; frontmatter-pinned `model: sonnet` + `effort: medium`. **Mode A** — `/task` group-implementor (read `.progress.md`, do subtasks sequentially, gate + commit per subtask). **Mode B** — single-fix delegate (author the orchestrator's planned fix, gate, return WITHOUT committing). Spawns `image-check` on any golden mint/regen — Mode A must not commit the PNG, Mode B must not return, until it confirms image↔code consistency. Never runs self-review, never pushes. |
| `image-check` | `code-writer`, on any golden mint/regen (never CI) | Image↔code consistency check on a minted/regenerated golden PNG; frontmatter-pinned `model: sonnet` + `effort: medium`, `tools` omitted (inherit-all, so its `Read` renders the PNG). Derives the expected frame from the drawing code **before** opening the image, then returns PASS / FAIL; on FAIL the caller fixes the code and re-mints. **Mint/regen time only — never a CI gate** (CI has no model). Not a reviewer: judges the artifact against the code that generated it, never the writer's work. |
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
| PreToolUse (Bash `curl`/`wget` → `crates.io/api`) | **crates-io-ua** — block a crates.io API *fetch* carrying no User-Agent (`exit 2`). crates.io rejects UA-less requests with a JSON *error body*, so `jq -r '.crate.max_stable_version'` prints a literal `null` and **exits 0** — the AGENTS.md § *Dependency Versions* AXIOM reports success while the fact was never obtained. Fires only when the command **invokes `curl`/`wget`** AND names `crates.io/api` AND carries no UA; all four spellings (`-H`/`--header`/`-A`/`--user-agent`) satisfy it. The `curl`/`wget` conjunct is load-bearing: without it the hook blocks any command merely *containing* the substring — a `grep -rn 'crates.io/api'` over the docs makes no HTTP call and must not be blocked (this exact false positive fired on the first live run and is covered by a 22-case matrix). Escalated 2026-07-16 from `ai-docs/learnings.md`; the rule existed and was escalated, but the *recipe it prescribed* was the failure, and the failure is silent. |
| PostToolUse (Write\|Edit) | `cargo fmt` on `.rs`; **panic-gate** — flag `.unwrap()/.expect(/panic!` outside `#[cfg(test)]` |
| PostToolUse (Bash `git push`) | **pr-sync** — remind to re-read/sync the open PR body |

## Notes
- Default branch is `main` (not `master`).
- **CI, Dependabot and the design-system are all LIVE** (re-verified 2026-07-16 via `git ls-files`, the category-correct command per AGENTS.md § *Dependency Versions*): `.github/workflows/ci.yml` is tracked (jobs `changes`/`format`/`build`/`test`/`clippy`/`docs`/`miri` + the `*-pass` gates), `.github/dependabot.yml` is tracked, and `docs/design-system/` holds 72 tracked files. The `/pr-ci-failed`, `/main-ci-failed` and `/dependabot-pr` skills are therefore **active, not dormant** — this line previously asserted the opposite, having been written before any of it landed and never revisited. `ROADMAP.md` remains absent (`git ls-files ROADMAP.md` → empty), the one still-true third of the original claim.
- **Per-group implementor model+effort contract (PART 5).** The per-group implementor spawn — `/context-reset` § Handoff-protocol step 3, driven by `/task` Step 8 — is selected by the design's `## Handoff plan` group marker: a **code** group → `subagent_type="code-writer"`, whose `model: sonnet` + `effort: medium` are **frontmatter-pinned** (pass NO inline `model=`/effort override — there is no per-invocation `effort` parameter, so the file is the only lever that enforces the medium tier); an **instructions/harness** group → `subagent_type="general-purpose"` with inline `model="opus"` + inherited effort (typically xHigh, not pinned); both use the 1M-token window. The orchestrator model itself is unchanged (per-invocation). The quality-gate subagents (`design` / `design-review` / `self-review` / `spec-writer`) keep their Opus contract — only the per-group implementor spawn varies per group.
