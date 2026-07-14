# Design: Import useful CI workflows from quartzite

**Issue:** none — infrastructure bootstrap; owner opted out of a tracking issue (spec `Tracked in: none`).
**Date:** 2026-07-14
**Spec:** `ai-docs/plans/2026-07-14-import-ci-workflows.spec.md` (28 ACs, 5 deliverables)

## Approach

Bootstrap GitHub Actions CI from zero by adapting quartzite's scaffolding to a
single `ubuntu-latest` lane, bump MSRV, and import the workspace lint tables
(pedantic/nursery escalated to `deny`) with the resulting source findings
resolved. Every Scope / Key-decision / Technical-constraint choice in the spec is
settled and carried through unchanged.

### File layout (Design-must-cover #1) — confirmed

Three new files plus edits to seven manifests and three source files:

- **`.github/workflows/ci.yml`** — *one* workflow holds **all** jobs: `changes`,
  `format`, `build`, `test` (incl. the Vulkan env-init), `clippy`, `docs`,
  `miri`, and the four `-pass` gates. Single-file is forced, not stylistic:
  - `miri` must read `needs.changes.outputs.rust`, and **cross-workflow job
    outputs are not referenceable** (spec Key decision "miri lives in `ci.yml`").
    So miri cannot live in a standalone `miri.yml`.
  - `build/test/clippy/docs` also gate on `changes.outputs.rust`; keeping them in
    the same file as `changes` is required for `needs:`.
  - `format` runs always (no `changes` dep) but has no reason to live in a
    separate file — one workflow is simplest and matches quartzite's `ci.yml`.
- **`.github/dependabot.yml`** — `cargo` + `github-actions` ecosystems.
- **root `clippy.toml`** — `stack-size-threshold` / `array-size-threshold`;
  clippy auto-discovers it at the workspace root (no per-crate copies).

### CI job graph (Design-must-cover #2)

Adapted from `../quartzite/.github/workflows/ci.yml` + `miri.yml`, dropping the
OS matrix, the `#340` ImageVersion/cargo-identity steps, the `gpu-tests` /
`features` / `roadmap-sync` jobs, the docs helper scripts, and the `libfontconfig1-dev`
apt installs (deferred until parley lands).

- Workflow-level `env: CARGO_TERM_COLOR: always` **and `CARGO_BUILD_WARNINGS: deny`**
  (AC28 — added by the amendment; see the dedicated note below). Triggers: `push` +
  `pull_request` to `main`.
- **`changes`** — `runs-on: ubuntu-latest`, `permissions: { pull-requests: read }`
  (needed by `dorny/paths-filter@v4` to read PR file lists), `outputs.rust` from
  the `filter` step. Steps: `actions/checkout@v6` → `dorny/paths-filter@v4`. The
  checkout is required — `paths-filter` diffs file lists against the checked-out
  repo on `push` events (on `pull_request` it uses the API, but the checkout must
  be present for the push path). Filter set is **exactly** the amended AC3 (owner-approved
  addition of `clippy.toml`, spec amended in parallel): `**/*.rs`, `**/Cargo.toml`,
  `Cargo.lock`, `clippy.toml`, `.github/workflows/**`, `rust-toolchain*`. Including
  `clippy.toml` makes a clippy.toml-only edit trigger the rust jobs (matches
  quartzite's own filter comment).
- **`format`** — no `needs`; `actions/checkout@v6` → `setup-rust-toolchain@v1`
  (`components: rustfmt`) → `cargo fmt --all -- --check`. Drop quartzite's
  "Verify cargo identity" step (`#340`).
- **`build` / `test` / `clippy` / `docs`** — `needs: changes`,
  `if: needs.changes.outputs.rust == 'true'`, `runs-on: ubuntu-latest`. Each job
  `env`: `SCCACHE_GHA_ENABLED: "true"`, `RUSTC_WRAPPER: "sccache"`,
  `SCCACHE_CACHE_SIZE: "2G"`. Common steps: checkout → `setup-rust-toolchain@v1`
  (`cache-shared-key: ${{ runner.os }}-stable-v2` — **no ImageVersion segment**;
  `cache-save-if: ${{ github.ref == 'refs/heads/main' }}`; `components: clippy`
  only on the clippy job) → `mozilla-actions/sccache-action@v0.0.10` → the job
  command. No `libfontconfig1-dev` / apt step on build/clippy/docs. All four inherit
  the workflow-level `CARGO_BUILD_WARNINGS: deny` (see note below).
  - `build`: `cargo build --workspace --keep-going`.
  - `clippy`: `cargo clippy --workspace --all-targets -- -D warnings` (AC7 exact —
    **not** touched; no `--keep-going`, clippy already reports across all crates).
  - `test`: two-step idiom (`cargo test` has **no** `--keep-going`) —
    `cargo build --workspace --tests --keep-going` then
    `cargo test --workspace --tests --no-fail-fast` (Vulkan-gated; see below).
  - `docs`: `cargo doc --no-deps --workspace --keep-going` with
    `env: RUSTDOCFLAGS: "-D warnings"` — **no** `-D missing-docs`, **no**
    `--all-features`, **no** helper-script steps (AC8).
- **`test`** — same skeleton, plus the mandatory Vulkan env-init (below).
- **`miri`** — `needs: changes`, `if: needs.changes.outputs.rust == 'true'`,
  `runs-on: ubuntu-latest`, `continue-on-error: true`,
  `env: MIRIFLAGS: -Zmiri-tree-borrows` (only — drop quartzite's
  `-Zmiri-ignore-leaks` / `-Zmiri-disable-isolation`). Steps: checkout →
  `setup-rust-toolchain@v1` (`toolchain: nightly` unpinned, `components: miri, rust-src`,
  `cache-shared-key: ${{ runner.os }}-nightly-miri-v1`) → `cargo miri setup` →
  `cargo miri test --workspace` (no `--exclude`, no sccache — sccache would
  interfere with miri and helps nothing). **No `miri-pass` gate** (AC9).
- **`-pass` gates** — `build-pass`, `test-pass`, `clippy-pass`, `docs-pass` only.
  Each: `runs-on: ubuntu-latest`, `needs: [changes, <job>]`, `if: always()`, one
  bash step:
  ```bash
  c="${{ needs.changes.result }}"; r="${{ needs.<job>.result }}"
  if [[ "$c" != "success" ]] || [[ "$r" != "success" && "$r" != "skipped" ]]; then exit 1; fi
  ```
  This reproduces quartzite's "changes succeeded AND job succeeded-or-skipped"
  semantics on one OS (AC10). **No `format-pass`** (format never skips → it is
  itself a direct required check); **no `miri-pass`** (advisory must not become
  required).
  - **Display-name scheme (single-OS, no matrix — deviates from quartzite by
    necessity).** quartzite can give a base job and its `-pass` gate the *same*
    `name:` because its base jobs carry an OS-matrix suffix
    (`Build (ubuntu-latest)`) that disambiguates them from the gate's `Build`. We
    dropped the matrix, so two jobs both titled `Build` would produce two check
    runs of the same name and break branch protection. Therefore: the **base**
    jobs take distinct display names — `Build (compile)`, `Test (run)`,
    `Clippy (lint)`, `Docs (build)` (the last matches quartzite's own
    `docs` → "Docs (build)" precedent) — and the **`-pass` gates own the stable
    required-check names** `Build` / `Test` / `Clippy` / `Docs`. `Format` is a
    direct required check with a unique name (no pass job). This is what Group A
    shipped in the committed `ci.yml`.

**Vulkan env-init in `test` (Design-must-cover #2, AC17–AC20).** The `test` job
`env` adds `WGPU_BACKEND: vulkan`, `WGPU_ADAPTER_NAME: llvmpipe`,
`LIBGL_ALWAYS_SOFTWARE: "1"` alongside the sccache vars. Step order (spec
Technical constraints):
1. checkout → `setup-rust-toolchain@v1` → `sccache-action` (toolchain/cache first)
2. **apt install** (mandatory, no `continue-on-error`, no `|| true`):
   `sudo apt-get update` then `sudo apt-get install -y mesa-vulkan-drivers vulkan-tools`
3. **`vulkaninfo --summary`** validation (mandatory, **no `|| true`**) — the sole
   Vulkan signal until real GPU tests exist
4. **Build tests:** `cargo build --workspace --tests --keep-going` — compiles all
   test binaries, collecting every test target's compile errors (incl.
   `CARGO_BUILD_WARNINGS=deny` lint-warnings-as-errors) in one run; this is the
   test lane's `--keep-going` equivalent
5. **Test:** `cargo test --workspace --tests --no-fail-fast` — runs all tests
   without stopping at the first failure

The Vulkan steps ride the required `test` → `test-pass` lane and are gated only by
the shared `changes` rust-file filter (skipped only when nothing rust/workflow/lock
changed). A driver-install or `vulkaninfo` failure fails `test` → CI red (AC20).

**`CARGO_BUILD_WARNINGS: deny` + `--keep-going` (AC28 — amendment).** Verified facts
(owner-provided, checked live): `CARGO_BUILD_WARNINGS=deny` makes cargo emit an
**error** for any *local* crate whose compile produces adjustable rustc lint
warnings; it is respected as of Rust 1.97 (CI stable ≥ 1.97 honors it; MSRV is now
1.97.0). It affects only adjustable lints, not non-lint or dependency warnings.
`--keep-going` collects every crate's errors instead of aborting at the first.
Design decisions recorded:
- **Placement — workflow-level `env`** (alongside `CARGO_TERM_COLOR: always`) so the
  `build` / `test` / `clippy` / `docs` compile lanes all honor it with one line.
  `changes` / `-pass` / `format` run no compile, so are unaffected either way. The
  advisory **`miri`** job inherits it too; because miri is `continue-on-error`, a
  warning-turned-error in miri's build cannot gate CI red — acceptable, so no
  per-job scoping is needed.
- **`--keep-going`** on `build` (primary — where `CARGO_BUILD_WARNINGS=deny` bites
  for pure compilation and where first-error masking matters most) and on `docs`
  (`cargo doc --no-deps --workspace --keep-going`; both verified valid). **Not** on
  `clippy` (AC7 pins its command exactly, and clippy already reports across all
  crates). **`cargo test` has no `--keep-going`** (verified `cargo 1.97.0`:
  `error: unexpected argument '--keep-going'`, on stable and nightly). Per cargo's
  own tip and `cargo-test.md`, the `test` lane instead uses the two-step idiom —
  `cargo build --workspace --tests --keep-going` (compile all test binaries,
  collecting every error incl. `CARGO_BUILD_WARNINGS=deny` warnings-as-errors) then
  `cargo test --workspace --tests --no-fail-fast` (run all tests without stopping at
  the first failure). No doctests exist today (only a ```` ```text ```` block in
  `sim.rs`, not a compiled doctest), so `--tests` (which excludes doctests) loses
  zero coverage now — revisit if doctests are added. `--keep-going` /
  `--no-fail-fast` are **diagnostic-only** — they do not change pass/fail (cargo
  still exits non-zero if any crate errors/any test fails).
  - **Decision — loose spec phrasing accepted, not tightened.** The spec names
    `--keep-going` positively only on `build` (AC28/AC5), but its Key Decisions row
    already delegates `test`/`docs` `--keep-going` to the design, and AC6/AC8 are
    not "exactly"-qualified, so the extra diagnostic flags satisfy them. Applying
    the all-errors posture to all three compile lanes (build/docs via `--keep-going`;
    test via the `--tests --keep-going` + `--no-fail-fast` idiom) serves the owner's
    stated "collect all errors" intent. That scope **stands**; no spec AC is
    tightened. (Supersedes the earlier fold-in's `cargo test --keep-going` for the
    test lane — that flag does not exist.)
- **Overlap, not conflict.** Once Group B's deny lint tables are active,
  `missing_docs` / `pedantic` / `nursery` are already compile-errors for *all*
  cargo invocations; `CARGO_BUILD_WARNINGS=deny` **additionally** catches
  rustc-**default** adjustable warnings (`unused_variables`, `dead_code`, …) in the
  `build` / `test` / `docs` lanes — complementary coverage, not redundant. The
  workspace is warning-clean at rustc-default level today, so this edit is green on
  the current branch state regardless of the Group-B ordering.

### Lint-work ordering to keep every gate green (Design-must-cover #3)

The hazard: enabling the deny tables + `clippy.toml` + per-crate `[lints] workspace = true`
**before** the source findings are resolved would make `cargo build` /
`cargo clippy -- -D warnings` fail immediately on the gp-core findings — and,
because `missing_docs = "deny"` turns gp-core's findings into **compile errors**,
the four crates that depend on gp-core would fail to compile at all, masking their
own findings (see Risks — this crate-dependency masking is why the initial estimate
undercounted the stub crates; the amended spec now records the full inventory).
Sequencing avoids any red boundary:

1. **Resolve all source findings first** (subtask 5), while the deny tables are
   **not yet active**. These lints (pedantic/nursery/missing_docs) are not in
   `clippy::all` nor rustc-default, so `cargo clippy -- -D warnings` stays
   trivially green at this boundary; adding `///` docs, `Self`, `const`, and
   `#[allow]` carve-outs introduces no new default-level warnings. Boundary check:
   the warn-level probe (below) returns **zero** findings across all five crates,
   and `cargo test --workspace` stays green.
2. **Enable the denies atomically** (subtask 6): add the three `[workspace.lints.*]`
   tables + root `clippy.toml` + `[lints] workspace = true` to all five crates in
   one step. The gate flips to active with the source already clean, so
   `cargo clippy --workspace --all-targets -- -D warnings` is green immediately.

Boundary-verification probe (warn-level so no crate aborts, mirroring the FULL deny
set with the same three carve-out allows — this is how subtask 5 proves completeness
before the denies are switched on):
```
cargo clippy --workspace --all-targets -- \
  -W clippy::pedantic -W clippy::nursery -W missing_docs -W rustdoc::broken_intra_doc_links \
  -W clippy::large_stack_frames -W clippy::large_stack_arrays -W clippy::undocumented_unsafe_blocks \
  -A clippy::must_use_candidate -A clippy::redundant_pub_crate -A clippy::return_self_not_must_use
```
The three separately-denied lints (`large_stack_frames` / `large_stack_arrays` /
`undocumented_unsafe_blocks`) have zero plausible hits today (no `unsafe`; threshold
524288), but are included so the completeness check mirrors the full deny set;
subtask 6's full `-D warnings` run is the real backstop.

### Per-lint disposition for the source findings (Design-must-cover #4)

Full finding set verified this session via a **clean** `cargo clippy --workspace
--all-targets` run under the exact post-change deny/allow set (`cargo clean` first,
then warn-level so gp-core errors don't mask downstream crates). The initial
estimate undercounted (see Risks for the root cause); the authoritative clean-scan
set below is what the amended spec now records (file:line → lint → disposition):

**`crates/core/src/geom.rs`**

| Site | Lint | Disposition |
|---|---|---|
| L15 `pub x` | `missing_docs` | add `///` (fix) |
| L16 `pub y` | `missing_docs` | add `///` (fix) |
| L20 `Point::new` | `missing_docs` | add `///` (fix) |
| L27 `neighbors4` (incl. return type `[Point;4]` + L29–32 body) | `use_self` | `Point`→`Self` at all sites incl. the array return type (fix) |
| L72 `Corridor::new` | `missing_panics_doc` | add `# Panics` section documenting the non-negative-dims `assert!` (fix) |
| L81 `(width * height) as usize` | `cast_sign_loss` | **carve-out**: local `#[allow(clippy::cast_sign_loss)]` + comment "`width`,`height` asserted `>= 0` immediately above" — integer semantics unchanged |
| L85 `origin` | `missing_docs` + `missing_const_for_fn` | add `///` **and** `const` (fix ×2) |
| L88 `width` | `missing_docs` + `missing_const_for_fn` | add `///` **and** `const` (fix ×2) |
| L91 `height` | `missing_docs` + `missing_const_for_fn` | add `///` **and** `const` (fix ×2) |
| L112 `is_empty` | `missing_docs` | add `///` (fix) |
| L116 `index` | `missing_const_for_fn` | add `const` (fix) |
| L121 `(dy * self.width + dx) as usize` | `cast_sign_loss` | **carve-out**: local `#[allow(clippy::cast_sign_loss)]` + comment "`dx,dy ∈ [0,width)×[0,height)` by the guard above ⇒ non-negative" — integer semantics unchanged |

**`crates/core/src/sim.rs`**

| Site | Lint | Disposition |
|---|---|---|
| L13–16 `pub x,y,vx,vy` (CarState) | `missing_docs` ×4 | add `///` each (fix) |
| L26 `Action` enum doc | `too_long_first_doc_paragraph` | insert a paragraph break (blank `///` line) so the first paragraph is under threshold — text preserved (fix) |
| L45–50 `Action::ALL` | `use_self` | `Action`→`Self` at all arms (fix) |
| L56–60 `Action::accel` match arms | `use_self` | `Action`→`Self` at all arms (fix) |
| L118 `LapCounter::new` | `missing_docs` | add `///` (fix) |
| L128 `raw` | `missing_const_for_fn` | add `const` (fix) |

**`crates/core/src/track.rs`**

| Site | Lint | Disposition |
|---|---|---|
| L18 `StartFinish` doc | `too_long_first_doc_paragraph` | insert a paragraph break — text preserved (fix) |

**`crates/gen/src/lib.rs`** *(named as a source crate by the amended spec)*

| Site | Lint | Disposition |
|---|---|---|
| L27 `min_width` (`self.cars.div_ceil(2)`; `div_ceil` const-stable since 1.73) | `missing_const_for_fn` | add `const` (fix) |
| L32 `start_finish_width` | `missing_const_for_fn` | add `const` (fix) |

**`crates/ai/src/lib.rs`** *(named as a source crate by the amended spec)*

| Site | Lint | Disposition |
|---|---|---|
| L11 `Features` doc | `too_long_first_doc_paragraph` | insert a paragraph break (fix) |
| L31 `policy_action` doc | `too_long_first_doc_paragraph` | insert a paragraph break (fix) |

`crates/render` and `crates/game` have **zero** findings (verified) — their
`[lints] workspace = true` opt-in (AC24) is a required no-op.

**`cast_sign_loss` and integer semantics (AC26).** Both cast sites are in
`Corridor::new` / `Corridor::index`; **neither is in `supercover`** (which is
`i64`-only and never casts to `usize`). Carving them out with local `#[allow]` +
justification leaves the arithmetic byte-for-byte unchanged, so the supercover C4
case table (`docs/design.md` §3 C4) and all gp-core tests pass **unchanged**. A
workspace-level `cast_sign_loss = "allow"` is **rejected** — too broad; the lint
stays active for the rest of the integer core.

All `const` additions are MSRV-1.97.0-safe (field access, integer arithmetic,
comparison, `Option`, and `u32::div_ceil` are all const-stable ≤ 1.73).

### MSRV bump (§4) and lint tables (§5) on root `Cargo.toml`

Two independent edits to `Cargo.toml`: (a) `rust-version = "1.85"` → `"1.97.0"`,
`resolver = "3"` retained unchanged (AC21); (b) the three `[workspace.lints.*]`
tables (AC22). Kept as separate subtasks because (a) is safe at any point (local
toolchain is already 1.97.0) whereas (b) must land only after the source fixes.

The three `= "allow"` entries each carry a **graphite-gp-specific** one-line `#`
justification (no quartzite hit-count text, AC22). Suggested rationale text
(impl finalizes): `must_use_candidate` — project-wide opt-in `#[must_use]` posture,
applied deliberately not blanket; `redundant_pub_crate` — keep explicit `pub(crate)`
even in private modules (read-locally cheaper than re-deriving visibility);
`return_self_not_must_use` — same family/rationale as `must_use_candidate`.
`pedantic`/`nursery` use `{ level = "deny", priority = -1 }` so the specific allows
override the group; `large_stack_frames`/`large_stack_arrays`/`undocumented_unsafe_blocks`
listed separately as `deny`.

### TDD posture (Design-must-cover #5)

These are config/lint changes — there is **no new `#[test]`** to write, and none
should be invented. The acceptance "tests" are the gates themselves:

- `cargo clippy --workspace --all-targets -- -D warnings` clean on a **full-clean**
  run (AC25) — the RED→GREEN signal: the deny tables are "RED" against the
  unfixed source; the source fixes make them "GREEN".
- The **existing** gp-core suite (12 supercover C4 cases + others) passes
  **unchanged**: `cargo test -p gp-core` (baseline confirmed green this session).
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` clean (AC16).
- `actionlint` clean on `ci.yml` (Design-must-cover #6, AC15) — run at Step 9
  verify; every added workflow file must be actionlint-clean.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Create `ci.yml`: `changes` (runs `actions/checkout@v6` → `dorny/paths-filter@v4`; checkout required for the `push`-event diff; filter set = amended AC3, **includes `clippy.toml`**) + `format` + `build` + `test` (with Vulkan env-init, step order per §2) + `clippy` + `docs` + `miri` + `build/test/clippy/docs-pass`. sccache on build/test/clippy/docs; no `#340` steps; no `libfontconfig1-dev`. Must be actionlint-clean (AC1–AC12, AC15, AC17–AC20). | `.github/workflows/ci.yml` | — |
| 2 | Create `.github/dependabot.yml`: `cargo` (`chore(deps)`) + `github-actions` (`chore(ci)`), weekly, group-all minor+patch, ignore semver-major, `target-branch: main` (AC13, AC14). | `.github/dependabot.yml` | — |
| 3 | Bump root `[workspace.package] rust-version` `"1.85"` → `"1.97.0"`; keep `resolver = "3"` unchanged (AC21). Verify `cargo build` green. | `Cargo.toml` | — |
| 4 | **Amendment (own commit).** Amend the already-committed `ci.yml` (subtask 1, commit e01f200): add workflow-level `env: CARGO_BUILD_WARNINGS: deny`; add `--keep-going` to `build` (`cargo build --workspace --keep-going`) and `docs` (`cargo doc --no-deps --workspace --keep-going`); replace the `test` command with the two-step idiom (`cargo test` has **no** `--keep-going`, verified 1.97.0) — `cargo build --workspace --tests --keep-going` then `cargo test --workspace --tests --no-fail-fast`, both after the `vulkaninfo` step; **not** `clippy` (AC7-exact). Re-run `actionlint`. Green on current branch (workspace is rustc-default warning-clean today). (AC28; **AC5** is shared — subtask 1 committed the bare `cargo build --workspace`, subtask 4 completes it with `--keep-going`). | `.github/workflows/ci.yml` | 1 |
| 5 | Resolve **all** source findings across **all three source crates** per the disposition tables (denies **not yet active**): **gp-core** — `missing_docs` ×12, `use_self` ×3 items, `missing_const_for_fn` ×5, `missing_panics_doc` ×1, `too_long_first_doc_paragraph` ×2, `cast_sign_loss` ×2 (local-`#[allow]` carve-out, `Corridor::new`/`index` — not `supercover`); **gp-gen** — `missing_const_for_fn` ×2 (L27, L32); **gp-ai** — `too_long_first_doc_paragraph` ×2 (L11, L31). Verify: the warn-probe (mirroring the **full** deny set, incl. `large_stack_frames`/`large_stack_arrays`/`undocumented_unsafe_blocks`) returns 0 findings across all 5 crates; `cargo test --workspace` green (supercover C4 unchanged, AC26); `cargo clippy -- -D warnings` still green (boundary for AC25). | `crates/core/src/{geom.rs,sim.rs,track.rs}`, `crates/gen/src/lib.rs`, `crates/ai/src/lib.rs` | — |
| 6 | Enable denies atomically: add `[workspace.lints.rust/.rustdoc/.clippy]` (with justified allow comments) to root `Cargo.toml`; create root `clippy.toml`; add `[lints] workspace = true` to all 5 crate manifests. Verify **full-clean** `cargo clippy --workspace --all-targets -- -D warnings` + `cargo build`/`test`/`fmt --check`/doc gate all green (AC16, AC22–AC25). | `Cargo.toml`, `clippy.toml`, `crates/{core,gen,render,ai,game}/Cargo.toml` | 5 |
| 7 | Document the allow-list / carve-out linter-posture discipline in `ai-docs/code-style.md`; do **not** edit `AGENTS.md` (AC27). Trace any new relative link with `realpath`. | `ai-docs/code-style.md` | 6 |

## Handoff plan

M = 7 (was 6; the AC28 ci.yml amendment adds one subtask). Grouping contract per
`.claude/agents/design.md` § Rules → handoff-grouping: non-terminal groups are
exactly **3** consecutive subtasks; the terminal group is within **1..=3**; a
`/context-reset` handoff opens **every** group, including the first. Groups:
A = 1–3, B = 4–6, C = 7.

- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) before
  starting subtask 1.
- **Group A:** subtasks 1–3 (infra: `ci.yml`, `dependabot.yml`, MSRV bump) — exactly
  3 subtasks (non-terminal). **Already shipped** (Group A committed; `ci.yml` at
  commit e01f200). At the 3→4 boundary the repo is green.
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent
  `/task` resumes in Group B with fresh context.
- **Group B:** subtasks 4–6 — exactly 3 subtasks (non-terminal). Subtask 4 (the AC28
  `ci.yml` amendment) is applied **first**, as its own standalone commit
  immediately after the Group-A handoff and before the lint source work; then
  subtask 5 (source fixes) → subtask 6 (deny tables). Each keeps every gate green
  at its boundary (the amendment is rustc-default warning-clean today; source fixes
  precede the deny flip). At the 6→7 boundary the workspace is fully clippy-clean.
- **Handoff after Group B:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent
  `/task` resumes in Group C with fresh context.
- **Group C:** subtask 7 (code-style linter-posture doc) — terminal group (1 subtask;
  within the 1..=3 range).

## Risks

- **Initial estimate undercounted the source findings — corrected (verified).** An
  earlier estimate put the work at "~7 items, all in gp-core, missing_docs 0 hits,
  stubs clean". The clean clippy scan this session showed the true set: gp-core has
  **12 `missing_docs` + 3 `use_self` items + 5 `missing_const_for_fn` + 1
  `missing_panics_doc` + 2 `too_long_first_doc_paragraph` + 2 `cast_sign_loss`**,
  and **gp-gen** (2 `missing_const_for_fn`) and **gp-ai** (2
  `too_long_first_doc_paragraph`) are **not** clean. Root cause: under `-D`,
  gp-core's `missing_docs`/pedantic findings become **compile errors** that abort
  gp-core, which blocks clippy from ever analyzing the four crates that depend on it
  — a crate-dependency-level version of the "hard-error gate masks later findings"
  hazard. This was corrected via the owner-approved amendment; the amended spec and
  the disposition tables above now carry the full set. *Mitigation:* the tables
  enumerate the **complete** set from a `cargo clean` + warn-level scan (nothing
  aborts), and subtask 5's boundary probe re-runs it (mirroring the full deny set)
  to confirm zero remain before the denies flip on. Every finding is resolved by
  one of the two authorized mechanisms (fix or justified carve-out).
- **gp-gen + gp-ai source fixes: owner-approved, first-class in subtask 5.** The
  owner approved the full whole-workspace cleanup (not just gp-core). Subtask 5
  now explicitly covers all three source crates: `crates/gen/src/lib.rs`
  (`missing_const_for_fn` ×2, L27 `min_width` + L32 `start_finish_width`),
  `crates/ai/src/lib.rs` (`too_long_first_doc_paragraph` ×2, L11 `Features` +
  L31 `policy_action`), and the full gp-core inventory (`missing_docs` ×12,
  `use_self` ×3 items, `missing_const_for_fn` ×5, `missing_panics_doc` ×1,
  `too_long_first_doc_paragraph` ×2, `cast_sign_loss` ×2). All four gen/ai edits
  are trivial `const` additions / paragraph breaks — behaviour-preserving. This is
  no longer a scope-boundary flag.
- **Mid-sequence red gate** if the deny tables land before the source fixes.
  *Mitigation:* the subtask 5→6 ordering + green-boundary checks above; subtask 6
  runs a full-clean clippy before its boundary is accepted.
- **`cast_sign_loss` altering integer semantics.** *Mitigation:* carve-out only
  (local `#[allow]` + justification), never an arithmetic change; both sites are
  outside `supercover`, so the C4 case table is untouched (AC26).
- **actionlint failures** (unquoted `${{ }}` in `run:`, bad `needs`, matrix leftovers).
  *Mitigation:* single-OS lane removes matrix expressions; `-pass` bash uses the
  quartzite-proven quoting; Step 9 runs actionlint (AC15).
- **`missing_const_for_fn` is a nursery lint that can regress across toolchains.**
  All const additions here are const-stable well below MSRV 1.97.0; low risk. If a
  future toolchain flips one, the deny surfaces it in CI — acceptable.

## Test Design

No new `#[test]` functions (config/lint task). Verification is by gates:

- **Location:** existing `crates/core/src/geom.rs` `#[cfg(test)] mod tests` (the 12
  supercover C4 cases) — must pass **unchanged**.
  - *Entry point:* `supercover` (and its callers). *Scenarios:* the C4 case table
    (axial, dual-vertex all-four, gcd1/gcd2/gcd3 diagonals, degenerate, symmetry,
    no-duplicate, both-endpoints). *Fixtures:* `cover_set` / `cells` helpers already
    present.
  - *Command:* `cargo test -p gp-core` (baseline green this session; the fixes touch
    only docs/`Self`/`const`/`#[allow]`, none of the covered logic).
- **Gate "tests":**
  - `cargo clippy --workspace --all-targets -- -D warnings` on a **full-clean** run
    → clean (AC25). Sub-probe (subtask 5): the warn-level command in Approach → 0
    findings across all 5 crates.
  - `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --all -- --check`,
    `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` → all green (AC16).
  - `actionlint .github/workflows/ci.yml` → no errors (AC15).

## Open questions

- **Resolved (owner-approved amendment):** `clippy.toml` is now **in** the `changes`
  filter set. The amended AC3 (spec updated in parallel) reads `**/*.rs`,
  `**/Cargo.toml`, `Cargo.lock`, `clippy.toml`, `.github/workflows/**`,
  `rust-toolchain*`, so a clippy.toml-only edit triggers the rust jobs (matches
  quartzite's own filter comment). No open item remains.
- **Confirmed, not reopened:** single `ubuntu-latest` lane, `main` triggers, `#340`
  dropped, `pedantic`/`nursery = deny`, `resolver = "3"` kept, MSRV 1.97.0, Vulkan
  init in `test`, dependabot cargo+github-actions, `Tracked in: none` — all carried
  through as specified.
