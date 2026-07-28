# Dependency Versions — live-lookup reference

This page extracts the live-lookup table from [`AGENTS.md` § Dependency Versions](../AGENTS.md#dependency-versions). The AXIOM headline and pinning bullets stay in AGENTS.md.

## Why query live first

Whenever you write a specific version of a Cargo crate or a GitHub Action — anywhere (`Cargo.toml`, workflow file, issue body, spec, design doc, learning, any `ai-docs/**` page) — query the live source first. Treating remembered versions as authoritative reliably puts wrong majors into specs (a remembered `criterion 0.5` when the live max-stable is a later major; a remembered `actions/checkout@v3` when live is `@v4`). The same logic applies one level deeper: a third-party GitHub Action's **behaviour** (what env vars it exports, what files it produces, which defaults it sets) is also stale in training data and in marketplace blurbs — treating a README blurb as authoritative can land a false claim into spec + design (e.g. assuming an action exports an env var that its `src/setup.ts` never sets, when only the user-set path in the README's "Rust code" subsection actually applies). And one dimension further still: the **current project's own dep graph** is also stale in memory — claiming "would add X as a dep" against an X already reached transitively by every leaf crate (e.g. via `gp-core`) puts a false premise into an issue body that the user has to correct, and the rationale that follows the false premise is often arguing the wrong trade-off entirely. The remedy is symmetric: `grep -r '<X>' --include='Cargo.toml' .` + `cargo tree --invert <X>` before writing the claim.

## Lookup table

| If you need to write... | Run this first |
|---|---|
| A Cargo crate version | `curl -sS -H "User-Agent: graphite-gp-agent (<contact-email>)" "https://crates.io/api/v1/crates/<name>" \| jq -r '.crate.max_stable_version'` — the `User-Agent` is **required** (see § *Failure mode* below); a bare `curl` returns `null` and exits 0 |
| A GitHub Action version | `gh api /repos/<owner>/<repo>/releases --jq '.[0].tag_name'` (and verify the action's Node runtime is current) |
| A version into a long-lived doc (won't be revisited for months) | Annotate `(verified current YYYY-MM-DD)` next to the version |
| A **load-bearing claim about an Action's behaviour** (env vars it exports, defaults it sets, files it produces — anything the spec or design relies on) | `gh api /repos/<owner>/<repo>/contents/action.yml --jq '.content' \| base64 -d` AND `gh api /repos/<owner>/<repo>/contents/src/setup.ts --jq '.content' \| base64 -d \| grep -inE 'exportVariable\|process\.env\|GITHUB_ENV\|saveState'` (or `src/main.ts` for run-step actions). Cite the source-line evidence in the design — README narrative alone is **not** evidence. |
| A **claim about whether dep `<X>` is / isn't / would-be-added-as a dep in this project** (any "would add X", "introduce X", "pull in X", "avoid X as a dep", "X is not currently a dependency") | `grep -rn '<X>' --include='Cargo.toml' .` to surface direct manifest hits; `cargo tree --invert <X>` to surface transitive presence via any leaf crate. Any hit → drop the false-premise wording; rewrite naming the actual concern (perf-sensitivity, feature-gate, test-prod parity, binary-size). This row exists because a crate is easily claimed as a would-be-new-dep when `cargo tree --invert <X>` would in fact show it already reached by every leaf crate via `gp-core`. |

## Beyond deps — the other three categories the AXIOM covers

`AGENTS.md` § *Dependency Versions* states the AXIOM and the STOP-substring trigger list; these are the per-category verification recipes it points at. The AXIOM's scope is deliberately wider than "dependencies": it covers **any** claim whose truth lives outside your context.

| If you're about to write... | Verify first with |
|---|---|
| A specific flag / subcommand / capability of an external tool (`cargo`, `gh`, `actionlint`, …) — e.g. *"`cargo test` supports `--keep-going`"* | `cargo <cmd> --help` (or run the command), or read the offline docs at `~/.rustup/toolchains/stable-*/share/doc/`. **Never assert a tool flag from memory.** |
| A claim that a file is **committed / tracked / ignored** (*"the repo commits X"*, *"X is gitignored"*, *"there are no stale Y"*) | **Match the command to the FILE CATEGORY, and name the category before choosing:** tracked → `git ls-files <path>`; ignored + which rule → `git check-ignore -v <path>`; untracked-but-not-ignored → `git status --porcelain`; ignored included → `git status --porcelain --ignored`; exists on disk at all → `ls` / `find`. `find`/`ls` prove on-disk presence, **never** tracked status. `git status` is **blind to ignored files** — empty output is NEVER proof a path is absent, and is actively misleading for any question about gitignored build/regen output, which is exactly where stale-artifact questions live. |
| A claim about an **upstream issue/PR's current state** (*"bug X is unfixed"*, *"affects 1.98 beta"*, *"no fix released"*) | `gh issue view <N> --json state,comments` — the issue *body* is frozen at filing time; the **closing comment** carries the resolution. **When the user cites a URL with a `#fragment`, fetch THAT anchor** — the fragment is the citation, the page is merely where it lives; a user linking a specific comment has usually already found the answer. |

> **The generalisation worth carrying:** an exit-0 command is evidence about *the question it asks*, not the question you meant. Before citing any command as proof, name the **category** the claim belongs to and confirm the command reaches that category — a tool blind to the category returns a clean, confident, wrong answer.

## Failure mode — `jq` + an error body = a silent `null`

**crates.io requires a `User-Agent`.** Its [data-access policy](https://crates.io/data-access) rejects UA-less requests with an *error body*:

```json
{"errors":[{"detail":"We are unable to process your request at this time. This usually means that you are in violation of our API data access policy (https://crates.io/data-access) ..."}]}
```

That body is **valid JSON with no `.crate` key**, so `jq -r '.crate.max_stable_version'` prints the literal string `null` and **exits 0**. The pipeline reports success while the fact was never obtained — and `null` reads as "no stable version" / "crate not found", a *fact-shaped non-answer*. Taken at face value it yields either a false "no stable release found" or a fallback to the remembered version, which is exactly what the AXIOM exists to prevent.

**Rules:**

- Treat a `null` from **any** version lookup as **"the query failed"**, never as data. Re-run without the `jq` filter and read the raw body before concluding anything.
- A version lookup returning `null` for a crate you have good reason to believe is published is a **tooling** failure until proven otherwise, not a fact about the crate.
- **Generalises beyond crates.io:** whenever a verification recipe pipes an HTTP response through `jq`, a policy / auth / rate-limit error body is *still valid JSON*, so `jq` prints `null` and exits 0. A stale recipe defeats the AXIOM as thoroughly as not running it — and more quietly, because the operator believes a check ran.
- A `PreToolUse` hook (`.claude/settings.json`) blocks UA-less `crates.io/api` calls mechanically; the rule above still governs every other `jq`-piped lookup, which no hook covers.

Then apply the pinning rule (in AGENTS.md) to the **observed** version, never the remembered one. If `setup.ts` / `main.ts` does not export the env vars your design assumed, set them explicitly in the workflow (per-job `env:` or `echo >> $GITHUB_ENV` after the action step) — don't rely on "the action probably sets it".
