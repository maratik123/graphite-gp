# gp-core car-collision resolution (same-final-cell only) + canonical-doc correction + gp-gen seeded-RNG adoption (rand + rand_chacha)

**Source:** issues #10 (car-collision resolution) and #49 (gp-gen adopts rand + rand_chacha)
**Date:** 2026-07-16
**Tracked in:** #10 (fully resolved) · #49 (partially resolved — dep adoption + seed→ChaCha wiring; generator stochastic usage deferred)

**Amendment (2026-07-16, product-owner directed).** Collision detection is now **same-final-cell only** — the conflict predicate is exactly `A.pos == B.pos` (two or more cars ending the turn on the same cell). The swap / pass-through detector (design.md [D1]/[N2]) is **dropped**: cell-swaps (A:P→Q while B:Q→P), head-on / mid-segment threading, and orthogonal path crossings that end on **distinct** cells are all **allowed**. Rationale to record: design.md [D1(a)] already states "no traffic physics — chokepoints are positional/visual", so two cars ending on distinct cells have no occupancy conflict and forcing a displacement would be gratuitous. Because the two canonical documents still mandate the now-dropped swap check, this task **also corrects them** (`docs/design.md`, `docs/design-review.md`). The seeded ChaCha RNG stack and the same-cell resolution machinery (#49) are unchanged and still required (they drive the same-cell winner shuffle, displacement order, and equidistant tie-break).

Both issues share one thread: the seeded, replay-deterministic RNG stack. The user
folded #49 in because #10 is the first *real* consumer of that stack (the collision
shuffle + equidistant tiebreak), so the two are landed together.

## Scope

### Part A — `resolve_collisions` (#10, `gp-core`, `crates/core/src/sim.rs`)

Replace the `todo!("car-collision resolution (design doc §3)")` stub. The collision
layer runs *outside* movement physics (design §3 "Коллизия машинок"). **The conflict
predicate is same final cell only: `A.pos == B.pos`.** Algorithm:

1. **Group by same final cell.** Cars that end the turn on the exact same cell form a
   conflict group. There is no other conflict class.
2. **Seeded shuffle.** Under the ChaCha RNG (below), shuffle the groups and the cars
   within each group. The shuffle decides which car stays (wins) and the displacement
   order.
3. **First car stays, occupied *before* displacements.** The first car of each group
   keeps its cell and is entered into the occupied set before any car is moved — this
   same step covers singletons (non-colliding cars), so there is one universal path.
4. **Displace each subsequent car to the nearest free cell** by in-`D` 4-connected
   geodesic BFS (reuse `CorridorScratch::geodesic_bfs` / `geodesic_layers` from #5,
   `crates/core/src/geom/graph.rs`). BFS intrinsically gives corridor distance, stays
   inside the track, never crosses a wall, and groups equidistant cells into one layer.
   Pick the target from the **first BFS layer holding a free cell**; ties within that
   layer are broken by the seeded RNG (uniform index).
5. **Update occupancy after every placement.** Single linear pass — no cascades, no
   cycles (nearest-free was chosen over "revert to previous cell" precisely because a
   previous cell may be occupied → cascades; design §3).
6. **Displaced car retains its velocity.** `vx`/`vy` untouched (zeroing would revive the
   "ram the pack to brake" abuse). The displacement is a **teleport**: no supercover
   check (BFS guarantees target `∈ D`) and no lap-counter interaction.

**Allowed, not resolved — no swap/pass-through detector.** Two cars whose move-segments
swap cells, thread through each other mid-segment, or cross orthogonally but end on
**distinct** cells are left **unchanged** — they have no occupancy conflict under the
same-final-cell predicate. The conflict predicate is `A.pos == B.pos` and nothing else:
there is **no** supercover-overlap check, **no** velocity dot-product, and **no**
`from = pos − v` segment derivation anywhere in the predicate. Rationale: design §3
[D1(a)] already establishes "no traffic physics — chokepoints are positional/visual", so
forcing a displacement on cars that end apart would be gratuitous.

**Signature (clean break, zero callers — verified).** From
`resolve_collisions(_d: &Corridor, _cars: &mut [CarState])` to
`resolve_collisions(d: &Corridor, cars: &mut [CarState], seed: u64)`. The `seed` is
**still needed** — it drives the same-cell winner/displacement shuffle and the
equidistant nearest-free tie-break. `resolve_collisions` constructs its ChaCha RNG
internally via `SeedableRng::seed_from_u64(seed)`, so no third-party RNG type crosses
gp-core's public API (no re-export needed). Exact param shape (seed vs `&mut impl
RngCore`) is design's call.

### Part B — seeded RNG adoption in `gp-gen` (#49, `crates/gen/`) — actionable subset

`GenParams` already carries `seed: u64` (verified, `crates/gen/src/lib.rs`), so the seed
is already threaded. The actionable work now:

1. Add `rand` + `rand_chacha` to `crates/gen/Cargo.toml`.
2. Provide a seeded ChaCha RNG constructor from `GenParams.seed` (e.g. a `GenParams::rng(&self)`
   method, or a helper) using `SeedableRng::seed_from_u64` — replay-deterministic, no
   `thread_rng`, no OS entropy on the generation path.
3. A `#[cfg(test)]` determinism test: same seed ⇒ identical RNG output stream.

The generator pipeline (`generate()`, phases Ф1–Ф7) is still `todo!`, so #49's "use the RNG
for all stochastic choices in generation" has no call sites yet → that part is **Deferred**
(see below), not pretend-implemented.

### Part A/B shared — dependency stack (#10 + #49)

Add `rand` + `rand_chacha` to **both** `crates/core/Cargo.toml` and `crates/gen/Cargo.toml`
(each crate is an independent consumer). Each crate constructs its own ChaCha RNG from a
`u64` seed — no shared RNG newtype (only 2 consumer sites, below the ≥3-site threshold for
a shared extraction; design may revisit). rand supplies the shuffle + uniform-index
sampling traits; rand_chacha supplies the portable, version-stable ChaCha stream engine.

### Part C — correct the canonical docs (#10 amendment, product-owner directive)

Both edits are **described here** for the `design` subagent to decompose and the
code-writer to execute in Step 8 — **the spec itself does not touch the docs**. The
corrections must reflect the new rule (same-final-cell-only; swaps / crossings ending on
distinct cells allowed) and must **not** fabricate a new review round.

1. **`docs/design.md` §3 "Коллизия машинок" / [D1] / [N2].**
   - Rewrite the [D1] "Swap/pass-through — отдельный чек" paragraph (~line 279): the
     swap/pass-through check is no longer part of the collision model.
   - Rewrite the [N2] "Разрешение обнаруженного swap" paragraph (~line 281): there is no
     detected-swap resolution; only same-final-cell conflicts route through the nearest-free
     BFS.
   - Fix the [D1] occupancy sentence (~line 283) that calls the swap check "единственная
     обязательная правка коллизий": collision resolution now handles **same-final-cell
     conflicts only**; swaps / mid-segment crossings / orthogonal crossings ending on
     distinct cells are allowed (consistent with the already-stated [D1(a)] "no traffic
     physics").
   - Fix the "Решённые … Occupancy-механика — РЕШЕНО (a) [D1]" entry (~line 411) that
     calls the swap check "обязательный correctness-фикс".
   - Fix the §6 review-order item 5 (~line 426): "D1 — swap/pass-through чек (correctness) + …".
   - Fix the §6 "Статус" line (~line 430) mention of "N2 (разрешение swap через nearest-free)".
   - **Keep intact:** the same-cell resolution algorithm itself (§3 ~lines 268–277: shuffle,
     first-car-occupied-before-displacement, nearest-free geodesic BFS, occupied-after-each,
     velocity-retained teleport).
2. **`docs/design-review.md` [D1] / [N2].**
   - Amend the D1 entry (~lines 133–152) and the N2 entry (~lines 323–330) to **record the
     superseding product-owner decision** (same-final-cell-only; swap check dropped), dated
     **2026-07-16** and clearly marked as a product-owner amendment. Append / annotate —
     **do not** invent a fake review round.
   - Reconcile the roll-up / verdict lines that still assert the swap check is mandatory
     (e.g. the "D1 option (a) + mandatory swap check ✅" line ~303 and the "N2 swap→nearest-free"
     summary ~368) so the review record stays internally consistent.

## Out of scope

- Wiring `resolve_collisions` into `step` / the game loop — zero callers today; the issue
  scope is the function itself.
- The master-seed → per-turn-seed derivation for collisions (a replay / game-loop concern,
  block 3b). `resolve_collisions` receives an already-deterministic `u64`.
- Occupancy / traffic physics — design §3 [D1(a)]: chokepoints are positional / visual only;
  no queuing / blocking mechanic and no swap/pass-through detector. Cars that swap, thread,
  or cross but **end on distinct cells** are allowed unchanged — the collision layer resolves
  same-final-cell conflicts only.
- Crash / respawn handling — `resolve_crash` is separate and already landed.
- The gp-gen generator pipeline itself (Ф1–Ф7 in `generate()`) — remains `todo!`; only the
  RNG scaffolding is in scope here.
- A shared / re-exported seeded-RNG type from gp-core (the #51 `enumflags2::BitFlags`
  re-export pattern does not apply — gp-gen samples with rand's own traits, it is not a
  type-plumbing downstream).

## Deferred

- Radius-capped BFS with fallback `v=0` in place | design §3 marks it "optional" (safety
  net), not required by the ACs | likely no separate issue — revisit on a near-full-track
  pathology.
- gp-gen: using the ChaCha RNG for the generator's stochastic choices (Ф1–Ф7 sampling) |
  `generate()` is `todo!`; no stochastic call sites exist yet | folds into the gp-gen
  generator implementation task (block 1); #49 stays open for that remainder.

## Key decisions

| Question | Decision |
|---|---|
| Conflict predicate | **Same final cell only: `A.pos == B.pos`** (product-owner amendment, 2026-07-16). Cars ending on distinct cells never conflict, even if their move-segments swap, thread, or cross. |
| Swap / pass-through detector | **Dropped** — no such detector, no supercover-overlap check, no velocity dot-product, no `from = pos − v` derivation. Swaps / mid-segment / orthogonal crossings ending on distinct cells are allowed (design §3 [D1(a)] "no traffic physics"). |
| Canonical-doc correction | This task corrects `docs/design.md` §3 [D1]/[N2] (+ the §6 review-order / status / "Решённые" lines) and the `docs/design-review.md` [D1]/[N2] entries so they stop mandating the swap check. Described in Scope Part C; executed by the code-writer, not the spec. |
| RNG stack | `rand` + `rand_chacha` — a rand_chacha ChaCha stream seeded from a `u64` via rand's `SeedableRng` (portable, version-stable across machines & runs). Not `SmallRng`, not `fastrand`, not a hand-rolled PRNG. Still required: it drives the same-cell shuffle + equidistant tie-break. |
| Dep placement | Both `gp-core` and `gp-gen` declare `rand` + `rand_chacha` directly. gp-core keeps them internal (seed is a plain `u64` param — no third-party type in its public API, no re-export). |
| Shared RNG newtype? | No — each crate builds its own ChaCha RNG from the seed. Only 2 consumer sites (gp-core, gp-gen), below the ≥3-site shared-extraction threshold; design may revisit. |
| `resolve_collisions` callers today | Zero (verified) — signature change to add `seed: u64` is a clean break. |
| Velocity representation | `CarState { x, y, vx, vy }`; "velocity retained" = `vx`/`vy` left unchanged by displacement. |
| Lap-counter interaction | `LapCounter` is a *separate* struct, not a field of `CarState`; "untouched" = `resolve_collisions` neither receives nor mutates a `LapCounter`. |
| Nearest-free engine | Reuse `CorridorScratch::geodesic_bfs` / `geodesic_layers` (#5); layers are unordered tie-sets, RNG selects within a layer. |
| GenParams.seed | Already present (`crates/gen/src/lib.rs`) — #49's seed-threading is structurally done; wire it to the ChaCha constructor. |
| ChaCha variant | Design's choice (ChaCha8 / 12 / 20 are all portable & version-stable); default to the conservative variant unless perf argues otherwise. |
| Tracking | #10 fully resolved by this PR; #49 partially (dep adoption + seed→ChaCha wiring done; generator stochastic usage deferred → #49 stays open). |

## Technical constraints

- **Determinism / replay is a hard AC** for both parts: same seed + same inputs ⇒
  byte-identical output. **No `thread_rng`, no OS entropy** on either the collision path or
  the generation path — construct the RNG only from the explicit seed.
- **Integer-only core** (design §3a): the ChaCha RNG yields `u32`/`u64`, consumed only as
  an integer shuffle order and an integer uniform index among equidistant free cells — no
  non-integer arithmetic enters the physics core.
- **Dependency pins** (verified live on crates.io, 2026-07-16; pin per AGENTS.md §
  Dependency Versions — `0.x` for `0.x.y`):
  - `rand` → `0.10` (max stable 0.10.2)
  - `rand_chacha` → `0.10` (max stable 0.10.0)
  - Compatibility: rand 0.10.2 and rand_chacha 0.10.0 both require `rand_core ^0.10.0`
    — same `rand_core` major, compatible.
  - After editing the manifests, run `cargo update` + `cargo build` to refresh `Cargo.lock`.
- Reuse one `CorridorScratch` for the repeated collision BFS queries (its documented reuse
  contract).
- `geodesic_bfs` layers are documented as *unordered* tie-sets — attach no semantics to
  intra-layer order; the seeded RNG selects within a layer. (Intra-layer order is fixed &
  reproducible, so a seeded uniform index into the layer is a valid, deterministic pick.)
- Standard AGENTS.md file-size / doc / clippy (`-D warnings`) conventions; add `#[cfg(test)]`
  coverage in both crates.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Singletons / non-colliding cars are handled by the same path — the group's first car is occupied before displacements, so a lone car is unchanged. |
| AC2 | A displaced car lands on the first BFS layer that contains a free cell (nearest by in-`D` 4-conn geodesic distance). |
| AC3 | Equidistant free cells are resolved by the seeded RNG; the same seed yields an identical pick. |
| AC4 | A displaced car retains its velocity (`vx`/`vy` unchanged); no `LapCounter` is touched; no supercover check runs on the teleport. |
| AC5 | Cars whose move-segments swap or cross (cell-swap, head-on / mid-segment threading, or orthogonal crossing) but **end on distinct cells** are left **unchanged** — only same-final-cell conflicts (`A.pos == B.pos`) are resolved (no traffic physics; design §3 [D1(a)]). |
| AC6 | Resolution is a single linear pass — occupancy updated after each placement; no cascades or cycles. |
| AC7 | `resolve_collisions` determinism: same seed + same inputs ⇒ byte-identical final positions and velocities across repeated calls. |
| AC8 | Collision tests cover, asserting exact final positions and velocities: **resolve cases** — three cars into one cell, and a car displaced into an occupied ring; **allowed cases** — two cars swapping cells and two cars threading segments while ending on distinct cells are left **unchanged** (positions and velocities intact). |
| AC9 | `rand` + `rand_chacha` are added to both `crates/core/Cargo.toml` and `crates/gen/Cargo.toml`, pinned `0.10` / `0.10`; `cargo build` + `cargo clippy --workspace --all-targets -- -D warnings` pass. |
| AC10 | `gp-gen` exposes a seeded ChaCha RNG constructed from `GenParams.seed` via `SeedableRng::seed_from_u64`; no `thread_rng` / OS entropy on the generation path. |
| AC11 | A gp-gen determinism test asserts the same `GenParams.seed` yields an identical RNG output stream. |
| AC12 | `docs/design.md` §3 [D1]/[N2] (and the §6 review-order / "Статус" / "Решённые" lines) no longer mandate the swap/pass-through check; they state same-final-cell-only resolution with swaps / mid-segment / orthogonal crossings ending on distinct cells allowed. The same-cell resolution algorithm (§3 shuffle → first-car-occupied → nearest-free BFS → velocity-retained teleport) is preserved. |
| AC13 | `docs/design-review.md` records a product-owner amendment (dated 2026-07-16) on the [D1] and [N2] entries superseding the mandatory swap-check recommendation; no fabricated review round, and no roll-up / verdict line still asserts the swap check is mandatory. |

## Open questions

- **No-free-cell exhaustion** (`D` fully packed, or more conflicting cars than free cells):
  BFS exhausts with no free cell. Defensible default for design — keep the car at its
  colliding position (degenerate same-cell overlap only in this pathological case, cars ≤
  cells normally); ties to the deferred radius-cap. Not exercised by the mandatory ACs.
- **ChaCha variant** (8 / 12 / 20) — design default (all portable & version-stable).
