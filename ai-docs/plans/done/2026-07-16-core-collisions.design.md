# Design: gp-core car-collision resolution (same-final-cell only) + canonical-doc correction + gp-gen seeded-RNG adoption

**Issue:** #10 (gp-core car-collision resolution) · #49 (gp-gen adopts `rand` + `rand_chacha`) · **PR #68**
**Date:** 2026-07-16
**Spec:** `ai-docs/plans/done/2026-07-16-core-collisions.spec.md` (Amendment 1 same-final-cell + Amendment 2 RNG-handle, both product-owner directed)

## Approach

**This revision is the Amendment-2 delta only.** Amendment 1 (same-final-cell predicate,
`rand`+`rand_chacha` deps, `GenParams::rng`, the `sim/collision.rs` module + AC1–AC8 tests,
and the Part C `docs/design.md` / `docs/design-review.md` corrections) is **already
implemented and merged** — verified in the tree:

- git log: `41cb35d feat(core): implement resolve_collisions (same-final-cell only)`,
  `7f737de test(core): cover resolve_collisions AC1-AC8`, `c6cb3ed feat(gen): add GenParams::rng`,
  `47cd434 docs(design): correct collision rule to same-final-cell-only`,
  `d409c2b docs(design-review): record 2026-07-16 product-owner amendment`,
  `d39c68e docs(plans): finalize core-collisions task`.
- `crates/core/src/sim/collision.rs` exists (signature `resolve_collisions(d, cars, seed: u64)`),
  `sim.rs` has `mod collision;` + the `pub use`.
- `docs/design.md` Part C markers present (`правка продукт-оунера, 2026-07-16`; all six old
  swap-mandate phrases absent — AC12 satisfied); `docs/design-review.md` carries both
  `Amendment — 2026-07-16` blocks + the reconciled roll-up/verdict (AC13 satisfied).

**Amendment 2 changes exactly one thing:** the physics RNG is now a **caller-owned,
long-lived `&mut ChaCha8Rng` handle**, not a per-call `seed: u64` that the function re-seeds
internally. The caller (the block-3b game loop, out of scope) owns **one** physics
`ChaCha8Rng`, seeded once at race start and advanced across turns; `resolve_collisions` just
**draws from** the passed handle for the same-cell winner/displacement shuffle and the
equidistant tie-break — it never constructs or re-seeds an RNG. The conflict predicate,
grouping, BFS, occupancy pass, determinism-consumption order, and every AC1–AC8/AC12/AC13
behaviour are **unchanged**.

### RNG param type — decision (the one "design's call" for Amendment 2)

**Chosen: concrete `rng: &mut ChaCha8Rng`.** Signature:
`pub fn resolve_collisions(d: &Corridor, cars: &mut [CarState], rng: &mut ChaCha8Rng)`.

Rejected alternatives:

- **`&mut impl RngCore` / `&mut impl Rng` (generic).** The `shuffle`/`random_range` bounds are
  `R: Rng` (rand-0.10's `Rng` = the former `RngCore`), so a generic form *works* — but it adds
  a generic type parameter, monomorphizes per RNG type (there is exactly **one** physics RNG
  type — YAGNI), and still puts a rand trait bound in the public signature. The spec's earlier
  "Rejected `&mut impl RngCore`" note was precisely about a *generic bound* leaking `rand_core`
  into the API; it argues **against** this option and **for** the concrete handle.
- **A gp-core `PhysicsRng(ChaCha8Rng)` newtype.** Would hide the `rand_chacha` type behind a
  gp-core type, but adds machinery (the newtype + `Rng`/`RngCore` forwarding or a bespoke
  `draw` API) the reviewer did not ask for, for a single consumer. Below the ≥3-site
  shared-extraction threshold (spec: "no shared RNG newtype today"). A clean later refactor if
  the engine choice ever needs hiding.

Why the concrete handle:

- The spec's whole point is to **share one concrete instance across turns** — a concrete
  handle is the natural, honest type (there is exactly one physics RNG).
- **Workspace consistency:** gp-gen already exposes `ChaCha8Rng` publicly
  (`GenParams::rng() -> ChaCha8Rng`), so gp-core taking `&mut ChaCha8Rng` matches the
  per-domain-stream architecture.
- **Simplest:** no generic, no monomorphization, no newtype. `use rand_chacha::ChaCha8Rng;`
  is already in `collision.rs`.
- The prior "no rand type in gp-core's public API" goal is **explicitly superseded** by the
  spec — a shared handle *necessarily* exposes a rand/rand_chacha (or newtype) type; that is
  inherent to the reviewer's request, and a concrete `ChaCha8Rng` is the least-machinery way.

**Per-domain RNG architecture (record, per PR #68):** four independent long-lived streams —
(1) physics/simulation (this handle), (2) track generation (`GenParams::rng`, merged),
(3) AI learning, (4) AI inference. Streams 3–4 and the game-loop's ownership/per-turn
advancement of the physics stream are **Deferred** (gp-ai / block 3b not built). No
RNG-registry/manager type is invented now.

### Verified facts (reused, still valid)

- **rand 0.10 API:** `Rng` re-exported from `rand_core`; `random_range` on `RngExt`; `shuffle`
  on `SliceRandom` (in-place, samples `< u32::MAX` as `u32` → cross-arch reproducible);
  `seed_from_u64` on `SeedableRng`. `default-features = false` keeps `getrandom` out of the
  tree. `rand`/`rand_chacha` pinned `0.10`/`0.10`, `rand_core` unified `^0.10.0`. ChaCha8.
- **Current merged `collision.rs`** (the edit surface): imports `use rand::{RngExt, SeedableRng};`
  + `use rand::seq::SliceRandom;` + `use rand_chacha::ChaCha8Rng;`. Body builds
  `let mut rng = ChaCha8Rng::seed_from_u64(seed);` (line 67) then `groups.shuffle(&mut rng)` /
  per-group `group.shuffle(&mut rng)` / `rng.random_range(0..n)`. `SeedableRng` is used **only**
  by that internal `seed_from_u64` call.

### The edit (algorithm unchanged, RNG source flipped)

1. **Signature** (line 59): `…, seed: u64` → `…, rng: &mut ChaCha8Rng`.
2. **Drop internal construction** (line 67): remove `let mut rng = ChaCha8Rng::seed_from_u64(seed);`.
   The two `shuffle` calls become `groups.shuffle(&mut *rng)` / `group.shuffle(&mut *rng)`
   (reborrow the `&mut ChaCha8Rng` param); `rng.random_range(0..n)` is unchanged (method call
   auto-reborrows). No other body change — grouping, Phase 1/2, BFS, `u32` tie draw, occupancy,
   exhaustion→stay all identical.
3. **Imports — clippy `-D warnings` trap.** Removing the `seed_from_u64` call makes `SeedableRng`
   **unused in production** → `unused_imports` would fail the lint gate. Change line 7 to
   `use rand::RngExt;` and move `use rand::SeedableRng;` **into the `#[cfg(test)] mod tests`
   block** (the tests now call `ChaCha8Rng::seed_from_u64(K)`; the trait must be in test scope —
   a `use super::*;` glob no longer supplies it once the production import is gone).
4. **`///` doc:** replace the determinism-contract wording "Given the same `d`, `cars`, and
   `seed` … A single `ChaCha8Rng::seed_from_u64(seed)` is built, then consumed …" with "Given
   the same `d`, `cars`, and **RNG state** (a `ChaCha8Rng` at the same stream position) … The
   **caller-supplied `&mut ChaCha8Rng` is drawn from — never re-seeded** — consumed in exactly
   this sequence …". Rules (a) canonical `sort_unstable_by_key(|g| g[0])` pre-shuffle order and
   (b) `groups.shuffle` → per-group `shuffle` → per-loser `u32` tie draw (only when
   `free.len() > 1`) are **unchanged**; note that the function **advances the caller's handle**
   and never seeds.
5. **Test call sites** (lines 144/152/186/217/228/237–238/249/256): each
   `resolve_collisions(&d, &mut cars, K)` becomes
   `let mut rng = ChaCha8Rng::seed_from_u64(K); resolve_collisions(&d, &mut cars, &mut rng);`.
   **Baked exact-state assertions are unchanged** — `seed_from_u64(K)` produces the identical
   ChaCha8 stream whether built inside the function or by the test, and the consumption order is
   identical, so every asserted final position/velocity (AC3's `(2,2)` stays / `(2,3)`, etc.)
   holds verbatim; only the call shape changes.

## Decomposition

| # | Task | Files | Depends on | Change-type |
|---|------|-------|------------|-------------|
| 1 | **Flip `resolve_collisions` to a caller-owned `&mut ChaCha8Rng` handle.** Signature `seed: u64` → `rng: &mut ChaCha8Rng`; drop the internal `ChaCha8Rng::seed_from_u64(seed)` and draw from `&mut *rng`; move `SeedableRng` out of production imports into the `#[cfg(test)] mod tests` block (production `unused_imports` fix); update the `///` determinism-contract wording ("seeded internally" → "caller-supplied RNG state, never re-seeded"); update every `#[cfg(test)]` call site to `let mut rng = ChaCha8Rng::seed_from_u64(K); resolve_collisions(&d, &mut cars, &mut rng);` (baked expected values unchanged). Gate: `cargo build` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test -p gp-core`. | `crates/core/src/sim/collision.rs` | — | code |

**Single atomic subtask** (`M = 1`): the signature change breaks the test call sites, so
production + tests + imports must land together to keep the build/clippy/test gate green — it is
not decomposable into independently-compiling steps. No `docs/design.md` / `docs/design-review.md`
change (Part C merged; the spec confirms design.md doesn't pin the signature and its [N4]
pseudocode already threads `rng` by handle — AC12/AC13 stand). Parts B (`GenParams::rng`) and the
dependency stack are already merged — not re-decomposed.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. **M = 1**, a single **code**
change-type subtask (Rust `*.rs`) → **one group**, terminal.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, 1M-token window, via
  the `code-writer` subagent — subtask **1** (code change-type). **Terminal group** (1 subtask;
  within the `1..=10` range). It is the only group and, per the every-group handoff contract,
  is entered in its own `/context-reset` subagent (per `.claude/skills/context-reset/SKILL.md`
  § Compaction recovery (re-entry)). No inter-group handoff. **1 group ≤ 4** (no user gate).

There is **no instructions/harness (docs) group** — Amendment 1's Part C corrections are merged
and Amendment 2 needs no doc change. The `design`, `design-review`, `self-review`, `spec-writer`
gates stay on Opus regardless of this marker.

## Risks

- **`unused_imports` on `SeedableRng`** after dropping the internal seed → `-D warnings` failure.
  *Mitigation:* the edit explicitly moves `SeedableRng` into the test module (subtask 1); the
  clippy gate is in the subtask's acceptance.
- **AC7 determinism now means "same seed → fresh handle," not "reuse one handle."** Reusing a
  single advanced handle across two runs yields *different* (still deterministic) outputs because
  the stream position moved. *Mitigation:* each determinism comparison run constructs a **fresh**
  `ChaCha8Rng::seed_from_u64(K)` (Test Design below); documented in the `///` ("advances the
  caller's handle, never re-seeds").
- **Baked test values drifting.** They do **not** — `seed_from_u64(K)` gives a byte-identical
  ChaCha8 stream whether built inside the fn or by the test, and the consumption order is
  unchanged; the values need no re-derivation. *Mitigation:* only the call shape changes; the
  gate `cargo test -p gp-core` confirms the untouched assertions still pass.
- **Public API now names `ChaCha8Rng`.** Accepted deliberately (spec) — inherent to sharing one
  concrete handle; the "no rand type in the public API" goal is superseded.
- **Reborrow ergonomics.** `groups.shuffle(&mut *rng)` (explicit reborrow of the `&mut` param);
  `rng.random_range(…)` auto-reborrows. No new arithmetic, no new `#[allow]`, zero production
  panics — all unchanged from the merged base.

## Test Design

All existing gp-gen and `resolve_collisions` tests stay; only the collision call sites change.

- **`resolve_collisions` (AC1–AC8)** — `crates/core/src/sim/collision.rs` `#[cfg(test)] mod tests`.
  - Every call site: `let mut rng = ChaCha8Rng::seed_from_u64(K); resolve_collisions(&d, &mut cars, &mut rng);`
    (add `use rand::SeedableRng;` in the test module). The **baked exact-state assertions are
    unchanged** (three-into-one-cell, displaced-into-occupied-ring, singleton, swap/thread
    ending-apart-unchanged, velocity-preserved, equidistant pick) — identical stream, identical
    consumption order.
  - *AC7 determinism:* the two comparison runs each construct a **fresh**
    `ChaCha8Rng::seed_from_u64(K)` (same K) and pass `&mut` → **byte-identical** final
    positions/velocities. Not one reused/advanced handle.
- **gp-gen determinism (AC10/AC11)** — merged, unchanged (`GenParams::rng`).
- **AC12/AC13 (docs)** — already satisfied by the merged Part C corrections; re-confirmable by
  the greps in the merged design history. No new doc edit.

## Open questions

All resolved (recorded for the reviewer):

- **RNG param shape** → concrete `&mut ChaCha8Rng` caller-owned handle (not a seed; not a
  generic bound; not a newtype) — justified above.
- **ChaCha variant** → `ChaCha8Rng` (matches the merged code + `GenParams::rng`; the concrete
  handle type).
- **No-free-cell exhaustion** → keep the car at its colliding position (unchanged from the
  merged base).
- **Amendment-2 doc correction?** → none (spec-confirmed: design.md doesn't pin the signature;
  [N4] already threads `rng` by handle). Part C (Amendment 1) is merged and stands.

No question requires product-owner input — none block implementation.
