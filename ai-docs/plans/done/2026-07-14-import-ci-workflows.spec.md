# Import useful CI workflows from quartzite

**Source:** user description
**Date:** 2026-07-14
**Tracked in:** none — infrastructure bootstrap; owner opted out of a tracking issue.

## Scope

Bootstrap GitHub Actions CI for graphite-gp by adapting the useful, applicable
config from the sibling `../quartzite` repository. graphite-gp currently has
**no `.github/` directory** — this task creates CI from zero.

User constraint: **linux amd64 is sufficient for now** → single-OS
`ubuntu-latest`; drop quartzite's 3-OS (ubuntu/macos/windows) matrix and the
non-Linux GPU (DX12/Metal) machinery. A Linux software-Vulkan environment IS
provisioned on the ubuntu lane per an explicit owner request (Scope §3).

Deliverables:

**1. A CI workflow (`.github/workflows/ci.yml`)** — triggered on `push` and
`pull_request` to `main`, with a workflow-level `CARGO_BUILD_WARNINGS: deny` env
(rustc lint warnings become errors on the compile lanes — respected as of Rust
1.97; our MSRV is 1.97.0), containing (faithful to quartzite's scaffolding,
adapted to a single ubuntu lane):

- **`changes`** — `dorny/paths-filter@v4` job exposing `outputs.rust`, with the
  filter set: `**/*.rs`, `**/Cargo.toml`, `Cargo.lock`, `clippy.toml`,
  `.github/workflows/**`, `rust-toolchain*`. Gates build/test/clippy/docs/miri.
- **`format`** — runs **always** (not gated on `changes`);
  `cargo fmt --all -- --check`. Component `rustfmt`.
- **`build`** — gated on `changes.outputs.rust == 'true'`;
  `cargo build --workspace --keep-going` (`--keep-going` so every crate's lint
  errors surface in one run under `CARGO_BUILD_WARNINGS: deny`, rather than the
  first erroring crate masking the rest). sccache enabled.
- **`test`** — gated on `changes`; `cargo test --workspace`. sccache enabled.
  **Also hosts the mandatory Linux software-Vulkan env-init** (see below): apt
  install `mesa-vulkan-drivers` + `vulkan-tools`, run `vulkaninfo --summary`
  validation, and set `WGPU_BACKEND=vulkan` / `WGPU_ADAPTER_NAME=llvmpipe` /
  `LIBGL_ALWAYS_SOFTWARE=1`.
- **`clippy`** — gated on `changes`; exactly
  `cargo clippy --workspace --all-targets -- -D warnings`. Component `clippy`,
  sccache enabled.
- **`docs`** — gated on `changes`; `cargo doc --no-deps --workspace` with
  `RUSTDOCFLAGS="-D warnings"` (see Key decisions — matches the local gate, no
  `-D missing-docs`, no `--all-features`). No quartzite helper scripts.
- **`miri`** — gated on `changes`; **advisory** (`continue-on-error: true`, does
  NOT gate CI red). Nightly toolchain (`miri`, `rust-src`), `cargo miri setup`
  then `cargo miri test --workspace`, `MIRIFLAGS: -Zmiri-tree-borrows`.
- **`build-pass` / `test-pass` / `clippy-pass` / `docs-pass`** — per-job
  aggregation gates (`if: always()`, `needs: [changes, <job>]`) that pass iff
  `changes` succeeded and the target job succeeded-or-skipped. Stable
  branch-protection check names. **No `miri-pass`** (advisory job must not
  become a required check); **no `format-pass`** (format never skips → it is a
  direct required check).

**2. Dependabot config (`.github/dependabot.yml`)** — two ecosystems, adapted
from quartzite (`target-branch: main`):

- `cargo` — weekly, group all deps (minor+patch), ignore semver-major, commit
  prefix `chore(deps)`.
- `github-actions` — weekly, group all (minor+patch), ignore semver-major,
  commit prefix `chore(ci)`. Keeps the imported action pins fresh
  (`checkout@v6`, `setup-rust-toolchain@v1`, `sccache-action`,
  `paths-filter@v4`).

**3. Mandatory Linux software-Vulkan env-init (in the `test` job).** graphite-gp's
`render` crate (block 2) is a stub today and has no `wgpu`/`winit`/`vello`/`parley`
dependency yet, but WILL render via wgpu/Vulkan. So the ubuntu lane provisions and
validates a software-Vulkan environment now — ready for when GPU code lands and so
GPU code paths are exercised (not silently skipped for lack of an adapter):

- **apt install (mandatory, hard-fail):** `sudo apt-get update` then
  `sudo apt-get install -y mesa-vulkan-drivers vulkan-tools` — lavapipe software
  Vulkan ICD + `vulkaninfo`.
- **validation step:** `vulkaninfo --summary` — run as a genuine validation (no
  `|| true`), since with no GPU tests yet it is currently the *sole* check that a
  software adapter is enumerable. On a plain ubuntu-latest with the drivers
  installed this succeeds.
- **env vars on the `test` job:** `WGPU_BACKEND: vulkan`,
  `WGPU_ADAPTER_NAME: llvmpipe`, `LIBGL_ALWAYS_SOFTWARE: "1"`. Harmless today
  (no GPU code); load-bearing once wgpu code lands.
- **Mandatory** = these steps ride the required `test` → `test-pass` lane; the
  `test` job is not `continue-on-error` and the Vulkan steps are not individually
  skippable. They are governed only by the same `changes` rust-file gate as the
  rest of the test lane (skipped only when nothing rust/workflow/lockfile changed —
  i.e., when there is nothing to validate).

**4. Bundled root `Cargo.toml` change — MSRV bump.** In the same PR, bump
`[workspace.package]` `rust-version` from `"1.85"` to `"1.97.0"` (current stable,
verified live this session: `rustc 1.97.0 (2026-07-07)`, latest stable channel
`1.97.0`). `resolver = "3"` is **retained unchanged** (see Key decisions). This is
the only line changed in `Cargo.toml`. No workflow toolchain-pin change is needed:
`actions-rust-lang/setup-rust-toolchain@v1` defaults to stable (≥ 1.97.0), which
satisfies the new MSRV.

**5. Lint-configuration import from quartzite (pedantic/nursery escalated to
`deny`).** Adds workspace lint tables + a root `clippy.toml`, opts every member
crate in, and resolves the resulting findings. **This deliverable touches gp-core,
gp-gen, and gp-ai SOURCE** (the gp-render and gp-game stubs stay clean).

- **Root `Cargo.toml` — three lint tables** (adapted from quartzite; the owner's
  one correction is `pedantic`/`nursery` `warn` → `deny`, every other lint AS-IS):

  ```toml
  [workspace.lints.rust]
  missing_docs = "deny"

  [workspace.lints.rustdoc]
  broken_intra_doc_links = "deny"

  [workspace.lints.clippy]
  pedantic = { level = "deny", priority = -1 }   # quartzite: warn → owner correction: deny
  nursery  = { level = "deny", priority = -1 }   # quartzite: warn → owner correction: deny
  large_stack_frames = "deny"
  large_stack_arrays = "deny"
  undocumented_unsafe_blocks = "deny"
  must_use_candidate = "allow"        # <graphite-gp-appropriate justification>
  redundant_pub_crate = "allow"       # <graphite-gp-appropriate justification>
  return_self_not_must_use = "allow"  # <graphite-gp-appropriate justification>
  ```

  Each `= "allow"` MUST carry a one-line `#` justification comment written **for
  graphite-gp** — do NOT copy quartzite's hit-count comments ("170 hits" etc.);
  the allow-list discipline requires a justification, not a count (design/impl
  writes the final text). `priority = -1` on the group enables so the specific
  `= "allow"` entries override the group.

- **Root `clippy.toml`** (as-is from quartzite; clippy auto-discovers it from the
  workspace root — no per-crate `clippy.toml`):

  ```toml
  stack-size-threshold = 524288
  array-size-threshold = 524288
  ```

- **Per-crate opt-in:** each of the 5 member crates
  (`crates/{core,gen,render,ai,game}`) adds to its own `Cargo.toml`:

  ```toml
  [lints]
  workspace = true
  ```

  Required because the root is a *virtual* workspace (no root package to carry
  `[lints] workspace = true`); without the per-crate opt-in the `workspace.lints`
  tables are inert. (Verified: no crate currently has any `[lints]` section.)

- **Source fixes / carve-outs (gp-core, gp-gen, gp-ai):** resolve the
  pedantic/nursery/missing_docs findings the new denies surface (accurate inventory
  in Technical constraints). Each finding is resolved by EITHER a small
  behaviour-preserving fix OR a justified carve-out (workspace-level `allow` or
  local `#[allow(clippy::…)]` with a comment) — design/impl decides per-lint.
  **SPECIAL CARE:** `cast_sign_loss` is in the integer physics core; a fix must
  preserve exact integer semantics. **Design disposition (owner-approved):** the
  two `cast_sign_loss` sites are carved out with **LOCAL `#[allow(clippy::cast_sign_loss)]`
  + justification at the two call sites in `Corridor::new`/`index`** (NOT in
  `supercover`); a workspace-level allow for `cast_sign_loss` is rejected as too
  broad. Integer semantics stay byte-for-byte unchanged, the supercover C4
  case-table tests (`docs/design.md` §3 C4) are untouched, and all gp-core tests
  stay green (AC26 preserved).

- **Documentation:** record the allow-list / carve-out discipline as a
  linter-posture note in `ai-docs/code-style.md` (referenced by AGENTS.md § Code
  Style). Do NOT edit AGENTS.md itself unless strictly necessary.

## Out of scope

- macOS / Windows runners and the OS matrix (user constraint: linux amd64 only).
- Non-Linux GPU stacks: DX12 (Windows) / Metal (macOS) setup and the
  quartzite `gpu-tests` job's Windows/macOS lanes. (Linux software-Vulkan
  env-init is now IN scope — see Scope §3.)
- A test-running GPU job — graphite-gp has no GPU tests yet; the Vulkan env is
  provisioned in the `test` job rather than a dedicated test-less job.
- The `libfontconfig1-dev` (parley fonts) and `xvfb` / `libxkbcommon-x11-0`
  (winit X11) installs — deferred until the corresponding deps land (see
  Deferred); not needed for the Vulkan-core env-init.
- Feature-matrix job — no crate defines `[features]`.
- Coverage / `cargo-llvm-cov` / Codecov / `CODECOV_TOKEN` — explicitly excluded
  this round.
- quartzite docs-job helper scripts (`check-rustdoc-internal-refs.sh`,
  `check-ac-doc-leaks.sh`, `check-rustflags-uniformity.sh`) — absent here.
- The `#340` ImageVersion cache-bust + "Verify cargo identity" steps — see Key
  decisions (dropped as macOS-specific).
- quartzite miri specifics: `--exclude <renderer>` and the
  `-Zmiri-ignore-leaks` / `-Zmiri-disable-isolation` flags (serial_test /
  tracing workarounds that do not apply here).
- Manifest-level `[workspace.lints.rust] warnings = "deny"` — rejected as brittle
  across toolchains; the CI clippy job's `-D warnings` is the deny-by-default
  mechanism (see Key decisions). The lint-table entries are the carve-outs.
- Editing `AGENTS.md` for the linter-posture note — the discipline is documented
  in `ai-docs/code-style.md` (which AGENTS.md § Code Style already references).

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| Bencher benchmarking workflows (`base_benchmarks.yml`, `fork_pr_benchmarks_run.yml`, `fork_pr_benchmarks_track.yml`) | No benchmarks exist (no `benches/` dirs, no bench harness); needs `BENCHER_API_TOKEN` + a Bencher cloud project | Yes — revisit when benchmarks are added |
| GitHub Pages rustdoc deploy (`docs.yml` deploy job) | Needs Pages enabled + `id-token`/`pages` perms + push-to-`main` deploy; the docs *build* gate is already in `ci.yml` | Yes — revisit if hosted docs are wanted |
| `roadmap-sync` job | Runs `scripts/gen-roadmap.sh`, which does not exist here | Yes — revisit if a roadmap generator is added |
| Coverage / Codecov | Excluded this round by the owner | Yes — revisit if coverage tracking is wanted |
| macos/windows runners | User scoped to linux amd64 for now | Revisit when multi-OS support matters |
| `xvfb` + `libxkbcommon-x11-0` apt packages | winit X11 backend runtime; no `winit` dep or windowed tests yet | No — add in the PR that introduces winit |
| `libfontconfig1-dev` apt package | `parley` font rendering; no `parley` dep yet | No — add in the PR that introduces parley |

## Key decisions

| Question | Decision |
|---|---|
| Optional extras | **Core gates + advisory miri.** No coverage/Codecov. |
| Structure fidelity | **Faithful** — keep `dorny/paths-filter` change-detection, per-job `-pass` aggregation gates, and sccache caching (`mozilla-actions/sccache-action` + `SCCACHE_GHA_ENABLED`/`RUSTC_WRAPPER=sccache`/`SCCACHE_CACHE_SIZE=2G`). |
| Trigger branch | `main` (graphite-gp default; quartzite used `master`). Push + pull_request to `main`. |
| Runner OS | `ubuntu-latest` only. |
| Actions pinned | `actions/checkout@v6`, `actions-rust-lang/setup-rust-toolchain@v1`, `dorny/paths-filter@v4`, `mozilla-actions/sccache-action` (as in quartzite; Dependabot keeps them fresh). |
| **`#340` ImageVersion / cargo-identity steps** | **DROPPED.** The `#340` workaround targeted a macOS cross-point-release `~/.cargo/bin` cache collision (a polluted cargo shim restored across macos-15.7.4→15.7.5); it cannot occur on a single ubuntu-latest lane. Use a plain `cache-shared-key: ${{ runner.os }}-stable-v2` (no ImageVersion segment) and drop the "Verify cargo identity" steps. |
| sccache `cache-save-if` | `${{ github.ref == 'refs/heads/main' }}` (adapted from `master`). |
| Docs gate `-D missing-docs`? | **Match the local AGENTS.md gate exactly** — `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`; **no** `-D missing-docs`, **no** `--all-features` (no features exist). The "every public item documented" intent is instead enforced at compile time by `[workspace.lints.rust] missing_docs = "deny"` (Scope §5), so the docs job's rustdoc flags need no change. |
| miri toolchain pin | Unpinned `nightly` (advisory `continue-on-error` job; low maintenance; Dependabot does not manage toolchain pins). Revisitable if nightly churn becomes noisy. |
| miri scope / flags | `cargo miri test --workspace` (no crate excludes — no `unsafe`/FFI anywhere), `MIRIFLAGS: -Zmiri-tree-borrows` only. |
| miri lives in `ci.yml` | Because the owner asked miri to gate on the shared `changes` job output, and cross-workflow job outputs aren't referenceable, miri is a job in `ci.yml` (not a standalone `miri.yml`). Finer file layout left to design. |
| Dependabot version policy | Ignore semver-major; allow minor+patch. Consistent with AGENTS.md § Dependency Versions (caret semantics already permit within-major bumps). |
| **Vulkan env-init placement** | **Folded into the `test` job**, not a dedicated job or composite action. Rationale: the `test` job is where GPU tests will run, so the env is load-bearing exactly there and GPU code paths get actually exercised; a separate test-less job would only duplicate the `vulkaninfo` validation without exercising anything, and a composite action is premature with one consumer. |
| **Vulkan package set (now)** | Minimal Vulkan core only: `mesa-vulkan-drivers` + `vulkan-tools`. Deferred: `xvfb` + `libxkbcommon-x11-0` (winit) and `libfontconfig1-dev` (parley) until those deps land. Keeps the install fast and succeeding on a plain ubuntu-latest that has no GPU code. |
| **`vulkaninfo` hard vs soft** | Run `vulkaninfo --summary` as a genuine validation (NO `|| true`). quartzite uses `\|\| true` because its real GPU tests do the hard validation; graphite-gp has none yet, so `vulkaninfo` is the only Vulkan signal and must be able to fail if the adapter is missing. Revisit (soften) once real GPU tests provide stronger validation. |
| **"Mandatory" semantics** | The Vulkan init is not `continue-on-error` and not individually gated away; it rides the required `test` → `test-pass` lane. It is still governed by the shared `changes` rust-file gate (like build/clippy/docs) — skipped only on non-rust changes where there is nothing to validate. |
| **MSRV bump** | Root `[workspace.package]` `rust-version` `"1.85"` → `"1.97.0"` (current stable, verified live: `rustc 1.97.0 (2026-07-07)`, latest stable channel `1.97.0`). CI's `setup-rust-toolchain@v1` stable default (≥ 1.97.0) satisfies it — no workflow toolchain pin needed. Bundled into this PR at the owner's request. |
| **Keep `resolver = "3"`** | **Retained — NOT removed.** The root manifest is a *virtual* workspace (no `[package]` section); the "edition 2024 ⇒ resolver 3 default" rule applies only to a root *package*, not a virtual workspace. Empirically verified: removing the line makes cargo warn `virtual workspace defaulting to \`resolver = "1"\` despite one or more workspace members being on edition 2024 which implies \`resolver = "3"\`` and silently downgrade feature unification to resolver v1. So `resolver = "3"` is load-bearing here. The `resolver` line is unchanged by this task. |
| **pedantic / nursery = `deny`** | quartzite sets these `warn`; the owner's one correction is to escalate both to `deny` (with `priority = -1` so specific `= "allow"` entries still override the group). Every other imported lint stays AS-IS. `missing_docs` and `broken_intra_doc_links` are `deny` too. |
| **Keep CI `-D warnings` (don't rely on the deny-groups)** | The clippy job stays exactly `cargo clippy --workspace --all-targets -- -D warnings` (AC7 unchanged); NO `[workspace.lints.rust] warnings = "deny"` is added. **Evidence-verified:** `pedantic`/`nursery = deny` do NOT cover rustc-default warnings (`unused_variables`, `dead_code`) or the default `clippy::all` group — those stay warn-by-default and clippy exits 0 without `-D warnings`. `-D warnings` is the "anything not carved out is denied" mechanism; the lint-table allows are the carve-outs. Manifest `deny(warnings)` rejected as toolchain-brittle. |
| **CI hardening: `CARGO_BUILD_WARNINGS: deny` + `--keep-going`** | Set `CARGO_BUILD_WARNINGS: deny` (env `build.warnings`) at workflow level in `ci.yml` so rustc lint warnings become errors on the **compile** lanes (`build`/`test`/`docs`), not only the `clippy -D warnings` job. Affects adjustable-level lints on **local** crates only — NOT non-lint warnings or dependency warnings; **respected as of Rust 1.97** (older toolchains silently ignore it — fine, our MSRV is 1.97.0 and CI's stable is ≥ 1.97). `cargo build --workspace --keep-going` so all crates' lint errors surface in one run (the cargo doc pairs `--keep-going` with `deny`; without it the first erroring crate masks downstream findings — same masking as the lint-inventory root cause). **CI-side only (env in `ci.yml`), NOT a manifest change** — consistent with the "keep `-D warnings` in CI, no manifest `deny(warnings)`" decision above. Exact per-job placement (workflow-level vs per-job), whether `test`/`docs` also take `--keep-going`, and whether the advisory `miri` job is in/out of scope are design-phase calls. |
| **Allow-list / carve-out discipline** | Every `clippy::* = "allow"` in `[workspace.lints.clippy]` carries a one-line justification comment (cross-referencing a project doc where one overlaps); group enables use `priority = -1`; `large_stack_frames`/`large_stack_arrays` are listed separately as `deny` so each survives a future per-group rollback. In-source `#[allow(clippy::…)]` only when UNAVOIDABLE + justified (AGENTS.md § Rust Test Conventions already states this — referenced, not duplicated). Where a clean fix isn't possible, a JUSTIFIED carve-out is acceptable instead of a behaviour-changing fix. Documented in `ai-docs/code-style.md`. |
| **Per-crate `[lints] workspace = true`** | The virtual-workspace root has no package to carry `[lints]`, so ALL 5 member crates opt in individually; otherwise `workspace.lints` is inert. |
| **`clippy.toml` at workspace root** | `stack-size-threshold` / `array-size-threshold` = `524288`, imported as-is; clippy auto-discovers the root `clippy.toml` (no per-crate copies). Paired with the `large_stack_frames`/`large_stack_arrays` denies. |

## Technical constraints

- Files touched:
  - **New:** `.github/workflows/ci.yml`, `.github/dependabot.yml`, root
    `clippy.toml` (Scope §5).
  - **Root `Cargo.toml`:** `rust-version` bump (§4) + three lint tables
    (`workspace.lints.rust` / `.rustdoc` / `.clippy`, §5).
  - **All 5 crate `Cargo.toml`s** (`crates/{core,gen,render,ai,game}`): add
    `[lints] workspace = true` (§5).
  - **Source edits for lint fixes / justified carve-outs** (§5, inventory below):
    gp-core source, `crates/gen/src/lib.rs`, and `crates/ai/src/lib.rs`.
  - **`ai-docs/code-style.md`:** linter-posture / allow-list-discipline note (§5).
  - Whether miri/format-etc. are split across additional workflow files is a
    design-phase layout call, subject to the "miri in `ci.yml`" constraint above.
- **Accurate current-code lint inventory** (authoritative count from a
  `cargo clean` + **warn-level** scan run in the design phase) — an impl/risk note,
  not an over-promise:
  - **Undercount root cause (do not repeat):** an earlier deny-level scan
    undercounted. Under `-D`/deny, gp-core's `missing_docs`/pedantic findings become
    compile ERRORS that abort gp-core and stop clippy from ever analysing the 4
    crates that depend on it — masking the downstream findings. Always take the
    count from a `cargo clean` + warn-level scan.
  - `broken_intra_doc_links`: 0 hits — that deny breaks nothing today.
  - **gp-core:** `missing_docs` ×12, `use_self` ×3, `missing_const_for_fn` ×5,
    `missing_panics_doc` ×1, `too_long_first_doc_paragraph` ×2, `cast_sign_loss` ×2.
    (`must_use_candidate` hits auto-suppressed by the imported `allow`.)
  - **gp-gen:** `missing_const_for_fn` ×2 (`crates/gen/src/lib.rs:27,32`).
  - **gp-ai:** `too_long_first_doc_paragraph` ×2 (`crates/ai/src/lib.rs:11,31`).
  - **gp-render, gp-game:** stubs, clean.
  - Each finding is resolvable by a small behaviour-preserving fix OR a justified
    carve-out (design/impl decides per-lint). The two `cast_sign_loss` sites use
    LOCAL `#[allow]` + justification in `Corridor::new`/`index` (Scope §5); exact
    integer semantics preserved — do NOT alter numeric behaviour to satisfy a lint.
- The MSRV bump (`rust-version = "1.97.0"`) must not break the local AGENTS.md
  gates — it won't: the local toolchain is already `1.97.0`.
- Workspace: 5 crates (`crates/{core,gen,render,ai,game}`), edition 2024,
  `rust-version = "1.97.0"` (bumped by this task; was `"1.85"`), resolver 3
  (retained — see Key decisions). `gp-core` is integer-only/deterministic —
  standard cargo gates suffice; no special runtime setup.
- `env: CARGO_TERM_COLOR: always` at workflow level (matches quartzite).
- `/task` Step 9 verify runs `actionlint` when workflow files change — every
  added workflow must be actionlint-clean.
- The Vulkan env-init must succeed on today's GPU-code-free workspace: the apt
  install + `vulkaninfo` run on a plain ubuntu-latest, and `cargo test --workspace`
  is unaffected by the `WGPU_*` / `LIBGL_ALWAYS_SOFTWARE` env vars (no consumer
  yet). Step order in the `test` job: toolchain/sccache setup → Vulkan apt install
  → `vulkaninfo` validation → `cargo test`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `.github/workflows/ci.yml` exists; every job runs on `ubuntu-latest`. |
| AC2 | `ci.yml` triggers on `push` and `pull_request` to `main` (not `master`). |
| AC3 | A `changes` job uses `dorny/paths-filter@v4` with the rust filter set (`**/*.rs`, `**/Cargo.toml`, `Cargo.lock`, `clippy.toml`, `.github/workflows/**`, `rust-toolchain*`); build/test/clippy/docs/miri are gated on its `rust` output. |
| AC4 | `format` job runs `cargo fmt --all -- --check` and is NOT gated on `changes` (runs always). |
| AC5 | `build` job runs `cargo build --workspace --keep-going`. |
| AC6 | `test` job runs `cargo test --workspace`. |
| AC7 | `clippy` job runs exactly `cargo clippy --workspace --all-targets -- -D warnings`. |
| AC8 | `docs` job runs `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS: "-D warnings"` (no `-D missing-docs`, no `--all-features`); references none of quartzite's helper scripts. |
| AC9 | An advisory `miri` job exists with `continue-on-error: true`, nightly toolchain (`miri`, `rust-src`), `MIRIFLAGS: -Zmiri-tree-borrows`, running `cargo miri test --workspace`; it does NOT gate CI red and has NO `-pass` aggregation gate. |
| AC10 | `-pass` aggregation gates exist for build, test, clippy, docs (`if: always()`, `needs: [changes, <job>]`), passing iff `changes` succeeded and the job succeeded-or-skipped. |
| AC11 | build/test/clippy/docs jobs enable sccache (`SCCACHE_GHA_ENABLED`, `RUSTC_WRAPPER=sccache`, `mozilla-actions/sccache-action`); cache key contains no ImageVersion segment and there are no "Verify cargo identity" steps. |
| AC12 | No workflow references macos/windows runners, DX12/Metal GPU stacks, `libfontconfig1-dev`, `xvfb`, `libxkbcommon-x11-0`, feature-matrix flags, coverage/Codecov, or `scripts/gen-roadmap.sh`. (Linux software-Vulkan init IS in scope — see AC17–AC20.) |
| AC13 | `.github/dependabot.yml` exists with `cargo` and `github-actions` ecosystems: weekly, group-all minor+patch, ignore semver-major, commit prefixes `chore(deps)` / `chore(ci)` respectively, `target-branch: main`. |
| AC14 | Deferred items (Bencher ×3, Pages deploy, roadmap-sync, coverage) are NOT added. |
| AC15 | `actionlint` reports no errors on every added workflow file. |
| AC16 | All local AGENTS.md gates still pass (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`). |
| AC17 | The `test` job installs `mesa-vulkan-drivers` + `vulkan-tools` on ubuntu via a mandatory apt step (not `continue-on-error`), preceded by `apt-get update`. |
| AC18 | The `test` job runs a `vulkaninfo --summary` validation step (present, mandatory, no `\|\| true`) after the driver install. |
| AC19 | The `test` job sets `WGPU_BACKEND: vulkan`, `WGPU_ADAPTER_NAME: llvmpipe`, and `LIBGL_ALWAYS_SOFTWARE: "1"`. |
| AC20 | The Vulkan init is mandatory: the `test` job is not `continue-on-error`, its Vulkan steps are not individually gated/soft-failed, and a Vulkan-init failure fails `test` → `test-pass` (CI red). It is governed only by the shared `changes` rust-file gate. |
| AC21 | Root `Cargo.toml` `[workspace.package]` has `rust-version = "1.97.0"` (was `"1.85"`), and `resolver = "3"` is retained unchanged. |
| AC22 | Root `Cargo.toml` has `[workspace.lints.rust]` (`missing_docs = "deny"`), `[workspace.lints.rustdoc]` (`broken_intra_doc_links = "deny"`), and `[workspace.lints.clippy]` with `pedantic` and `nursery` at `{ level = "deny", priority = -1 }`; `large_stack_frames` / `large_stack_arrays` / `undocumented_unsafe_blocks` = `deny`; `must_use_candidate` / `redundant_pub_crate` / `return_self_not_must_use` = `allow`, each with a graphite-gp-appropriate one-line justification comment (no quartzite hit-count text). |
| AC23 | Root `clippy.toml` exists with `stack-size-threshold = 524288` and `array-size-threshold = 524288`; no per-crate `clippy.toml`. |
| AC24 | All 5 member crates (`crates/{core,gen,render,ai,game}`) have a `[lints]` section with `workspace = true` in their own `Cargo.toml`. |
| AC25 | `cargo clippy --workspace --all-targets -- -D warnings` passes clean on the whole workspace after the fixes/carve-outs (every pedantic/nursery/missing_docs finding either fixed or covered by a justified carve-out). |
| AC26 | All gp-core tests pass unchanged, including the supercover C4 case table (`docs/design.md` §3 C4); `cast_sign_loss` handling preserves exact integer semantics. |
| AC27 | The allow-list / carve-out linter-posture discipline is documented in `ai-docs/code-style.md`; `AGENTS.md` is not edited (unless strictly necessary). |
| AC28 | `ci.yml` sets `CARGO_BUILD_WARNINGS: deny` (workflow-level env) so the build/test/docs compile lanes fail on rustc lint warnings, and the workspace `cargo build` invocation uses `--keep-going` to surface all crates' lint errors in one run. |

## Open questions

- **Docs gate strictness — RESOLVED (round 5).** The docs job's rustdoc flags stay
  as decided (`RUSTDOCFLAGS="-D warnings"`, no `-D missing-docs`, no
  `--all-features`). Adding `[workspace.lints.rust] missing_docs = "deny"` (Scope
  §5) now enforces "every public item documented" at **compile time**,
  workspace-wide, which satisfies the original intent WITHOUT changing the docs
  job. No further action.
