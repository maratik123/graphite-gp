# Dependency Versions — live-lookup reference

This page extracts the live-lookup table from [`AGENTS.md` § Dependency Versions](../AGENTS.md#dependency-versions). The AXIOM headline and pinning bullets stay in AGENTS.md.

## Why query live first

Whenever you write a specific version of a Cargo crate or a GitHub Action — anywhere (`Cargo.toml`, workflow file, issue body, spec, design doc, learning, any `ai-docs/**` page) — query the live source first. Treating remembered versions as authoritative reliably puts wrong majors into specs (a remembered `criterion 0.5` when the live max-stable is a later major; a remembered `actions/checkout@v3` when live is `@v4`). The same logic applies one level deeper: a third-party GitHub Action's **behaviour** (what env vars it exports, what files it produces, which defaults it sets) is also stale in training data and in marketplace blurbs — treating a README blurb as authoritative can land a false claim into spec + design (e.g. assuming an action exports an env var that its `src/setup.ts` never sets, when only the user-set path in the README's "Rust code" subsection actually applies). And one dimension further still: the **current project's own dep graph** is also stale in memory — claiming "would add X as a dep" against an X already reached transitively by every leaf crate (e.g. via `gp-core`) puts a false premise into an issue body that the user has to correct, and the rationale that follows the false premise is often arguing the wrong trade-off entirely. The remedy is symmetric: `grep -r '<X>' --include='Cargo.toml' .` + `cargo tree --invert <X>` before writing the claim.

## Lookup table

| If you need to write... | Run this first |
|---|---|
| A Cargo crate version | `curl -sS "https://crates.io/api/v1/crates/<name>" \| jq -r '.crate.max_stable_version'` |
| A GitHub Action version | `gh api /repos/<owner>/<repo>/releases --jq '.[0].tag_name'` (and verify the action's Node runtime is current) |
| A version into a long-lived doc (won't be revisited for months) | Annotate `(verified current YYYY-MM-DD)` next to the version |
| A **load-bearing claim about an Action's behaviour** (env vars it exports, defaults it sets, files it produces — anything the spec or design relies on) | `gh api /repos/<owner>/<repo>/contents/action.yml --jq '.content' \| base64 -d` AND `gh api /repos/<owner>/<repo>/contents/src/setup.ts --jq '.content' \| base64 -d \| grep -inE 'exportVariable\|process\.env\|GITHUB_ENV\|saveState'` (or `src/main.ts` for run-step actions). Cite the source-line evidence in the design — README narrative alone is **not** evidence. |
| A **claim about whether dep `<X>` is / isn't / would-be-added-as a dep in this project** (any "would add X", "introduce X", "pull in X", "avoid X as a dep", "X is not currently a dependency") | `grep -rn '<X>' --include='Cargo.toml' .` to surface direct manifest hits; `cargo tree --invert <X>` to surface transitive presence via any leaf crate. Any hit → drop the false-premise wording; rewrite naming the actual concern (perf-sensitivity, feature-gate, test-prod parity, binary-size). This row exists because a crate is easily claimed as a would-be-new-dep when `cargo tree --invert <X>` would in fact show it already reached by every leaf crate via `gp-core`. |

Then apply the pinning rule (in AGENTS.md) to the **observed** version, never the remembered one. If `setup.ts` / `main.ts` does not export the env vars your design assumed, set them explicitly in the workflow (per-job `env:` or `echo >> $GITHUB_ENV` after the action step) — don't rely on "the action probably sets it".
