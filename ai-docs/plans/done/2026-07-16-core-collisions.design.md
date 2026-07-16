# Design: gp-core car-collision resolution (same-final-cell only) + canonical-doc correction + gp-gen seeded-RNG adoption

**Issue:** #10 (gp-core car-collision resolution, fully resolved) · #49 (gp-gen adopts `rand` + `rand_chacha`, partially resolved — dep + seed→ChaCha wiring)
**Date:** 2026-07-16
**Spec:** `ai-docs/plans/2026-07-16-core-collisions.spec.md` (amended 2026-07-16, product-owner directed)

## Approach

Amendment delta from the prior design: the collision **conflict predicate is now
same-final-cell only** (`A.pos == B.pos`). The swap/pass-through detector is **dropped**
entirely — with it go the Union-Find, the `supercover ∩ supercover` overlap check, the `i64`
velocity dot-product, and the `from = pos − v` (`checked_sub`) segment derivation. Grouping
collapses to a plain bucket. The same-cell **resolution** machinery (seeded ChaCha8 RNG,
group/within-group shuffle, first-car-occupied-before-displacement, reused-`CorridorScratch`
nearest-free geodesic BFS, `u32` equidistant tie-draw, occupied-after-each single pass,
velocity-retained teleport, exhaustion→stay) is **unchanged**. New scope: **Part C**, the
two canonical docs that still mandate the now-dropped swap check are corrected. The #49 RNG
adoption is unchanged.

### Verified facts (checked earlier, still valid — reused, not re-derived)

- **Zero callers** of `resolve_collisions` (`ast-index usages` = 0; only the stub at
  `crates/core/src/sim.rs:452`). Signature change is a clean break (AGENTS.md § API Stability).
- **No `rand` anywhere** — absent from every `Cargo.toml` and from `cargo tree`.
- **Live versions:** `rand` 0.10.2, `rand_chacha` 0.10.0, `rand_core` 0.10.1 — both `rand` and
  `rand_chacha` declare `rand_core ^0.10.0` (`kind=normal`) → Cargo unifies one `rand_core` so
  the blanket `Rng` trait is a single type across both (required for `ChaCha8Rng` to satisfy
  `shuffle`/`random_range`). Pin `0.10` / `0.10`.
- **`rand` 0.10 API (from extracted `rand-0.10.1` source):** `Rng` is re-exported from
  `rand_core` (the generator trait); `random_range` is on `RngExt: Rng` (`use rand::RngExt;`);
  `shuffle` is on `SliceRandom` (`use rand::seq::SliceRandom;`), ungated, in-place Fisher–Yates
  (no `alloc`, no OS entropy) that samples `< u32::MAX` slices as **`u32`** → cross-arch
  reproducible; `seed_from_u64` on `SeedableRng` (`use rand::SeedableRng;`). `seq`/`distr` are
  ungated → both compile under `default-features = false`.
- **Reuse target:** `CorridorScratch::geodesic_bfs(&mut self, d, seed, visit: FnMut(usize, &[Point]) -> ControlFlow<B>) -> Option<B>`
  (layer-at-a-time, `ControlFlow::Break` early-stop, layers are *unordered tie sets* with
  fixed reproducible intra-layer order, O(1) scratch reset), `Corridor::contains`. `CarState { x, y, vx, vy }`
  (`Copy`, `Hash`, `Eq`), `Point` (`Hash`, `Eq`).
- **`sim.rs` size:** 456 non-test / 1049 incl-test — already over the incl-test *soft* cap
  (800); in-file addition risks the *hard* 1500 cap → new code goes in a `sim` submodule.

### Chosen solution

**Part A/B shared — dependency stack.** Add to **both** `crates/core/Cargo.toml` and
`crates/gen/Cargo.toml`:

```toml
rand = { version = "0.10", default-features = false, features = ["alloc"] }
rand_chacha = { version = "0.10", default-features = false }
```

`default-features = false` drops `std_rng`/`sys_rng`/`thread_rng` → **`getrandom` never enters
the tree** (auditable via `cargo tree` — a stronger, greppable form of the "no OS entropy on
the path" AC than "don't call `rng()`"); matches gp-core's I/O-free charter. `alloc`
(**GO-note 3**): *not strictly required by gp-core's shuffle + random_range* — both are
alloc-free in rand 0.10.1 (verified from source) — but **kept on both crates for line
uniformity** and gp-gen's later `choose`/weighted sampling, at zero cost in a `std` workspace.
Then `cargo update` + `cargo build` to refresh the tracked `Cargo.lock`.

**ChaCha variant — `ChaCha8Rng` (both crates).** Non-crypto use (a game replay seed, not a
security context) + `gp-core` sits on the AI-training hot path (design §3a) where the fastest
sufficient-quality variant is preferred; portability/reproducibility are identical across
ChaCha8/12/20. `ChaCha20Rng` is a one-token swap if a reviewer prefers the crypto-standard.

**Part B — gp-gen.** `GenParams` already carries `seed: u64`. Add
`pub fn rng(&self) -> ChaCha8Rng { ChaCha8Rng::seed_from_u64(self.seed) }` (public — AC10
"exposes"; `ChaCha8Rng` in the signature is fine, gp-gen is the consumer, not a type-plumbing
downstream). No `thread_rng`, no OS entropy. `generate()` stays `todo!` (#49 remainder Deferred).

**Part A — `resolve_collisions`.** New signature (clean break):
`pub fn resolve_collisions(d: &Corridor, cars: &mut [CarState], seed: u64)`. `seed: u64` (not
`&mut impl RngCore`) keeps every rand type out of gp-core's public API; the RNG is built
internally (`ChaCha8Rng::seed_from_u64(seed)`). The seed is **still needed** — it drives the
same-cell winner/displacement shuffle and the equidistant tie-break.

New code lands in **`crates/core/src/sim/collision.rs`** (a submodule of the existing `sim.rs`
— legal in Rust 2024 alongside `sim.rs` without renaming to `sim/mod.rs`). `sim.rs` replaces
its stub with `mod collision;` + `pub use collision::resolve_collisions;` — the public path
`gp_core::sim::resolve_collisions` is **preserved**, and the fresh file stays well under all
size caps.

**Algorithm** (design §3 "Коллизия машинок"; cited pointer-only). One universal path for
singletons and same-cell groups:

1. **Group by same final cell (RNG-independent, deterministic).** Bucket:
   `HashMap<Point, Vec<usize>>` filled by iterating `cars` in index order → each group's `Vec`
   is ascending-index-ordered, and `group[0]` is the group's min (first-appearance) car index.
   **HashMap iteration order is non-deterministic (`RandomState`)** — so materialize
   `let mut groups: Vec<Vec<usize>> = buckets.into_values().collect();` then
   `groups.sort_unstable_by_key(|g| g[0]);` → a **canonical, RNG-independent group order**
   (min car index is unique across groups). This fixed order is what makes the subsequent
   RNG consumption reproducible (AC7). *No Union-Find, no segment derivation, no
   supercover/dot-product — the predicate is `A.pos == B.pos` and nothing else.*
2. **Seed + fixed RNG-consumption order** (the AC3/AC7 replay contract):
   `let mut rng = ChaCha8Rng::seed_from_u64(seed);` → **(a)** `groups.shuffle(&mut rng)`;
   **(b)** for each group in shuffled order, `group.shuffle(&mut rng)` (picks the winner =
   post-shuffle index 0 + displacement order of the rest).
3. **Phase 1 — winners (no RNG).** For every group, insert the first car's `pos` into
   `HashSet<Point> occupied`; that car does not move. Singletons have only a winner →
   unchanged (AC1).
4. **Phase 2 — losers (single linear pass, AC6).** One `CorridorScratch::new(d)` reused for
   every query. For each group (shuffled order), for each non-first car (shuffled order):
   `scratch.geodesic_bfs(d, pos_i, visit)` where `visit(_dist, layer)` computes
   `free = layer.iter().copied().filter(|c| !occupied.contains(c)).collect::<Vec<_>>()`. First
   layer with `!free.is_empty()` → `Break(free)`. **The `*c != pos_i` own-cell-exclusion clause
   is removed** (spec §2): with same-cell-only grouping every loser's own cell *is* the
   occupied winner cell (layer 0 = `{pos_i}` ⊆ `occupied`), so `!occupied.contains(c)` alone
   already skips it. Target: `free[0]` if `free.len() == 1`, else
   `free[rng.random_range(0..u32::try_from(free.len()).expect("layer ≤ area fits u32")) as usize]`
   — the tie index is drawn as **`u32`** (matching rand's own usize-as-u32 policy) so the pick
   is reproducible across 32/64-bit targets (AC3/AC7). Write `cars[i].x/y = target` (**`vx`/`vy`
   untouched** — AC4), `occupied.insert(target)` (AC6). No supercover check, no lap counter
   (teleport, AC4).
5. **Exhaustion fallback** (BFS `None`: `pos ∉ D`, or fully packed): leave the car at `pos_i`
   unchanged (non-panicking, degenerate; ties to the deferred radius-cap).

**Determinism contract (AC3/AC7) — the `resolve_collisions` `///` MUST spell out these two
lines verbatim** so the replay contract can't drift under bucket grouping: **(a)** the
canonical pre-shuffle group ordering `sort_unstable_by_key(|g| g[0])` (min car index — HashMap
iteration order is *not* used); **(b)** the fixed RNG-consumption order `groups.shuffle` →
per-group `shuffle` → per-loser `u32` tie draw (only when `free.len() > 1`). Given the same
seed + inputs these two lines make the output byte-identical (and cross-arch via the `u32`
draw).

**Allowed, not resolved.** Cars whose move-segments swap, thread mid-segment, or cross
orthogonally but **end on distinct cells** never share a bucket → untouched (AC5). No detector.

Borrow-check: inside the `geodesic_bfs` closure, `rng` is captured `&mut` and `occupied` `&`;
`scratch` is the distinct `&mut self` receiver — no aliasing.

**Arithmetic-safety note.** With the detector gone, `resolve_collisions` has **no manual
integer arithmetic** — the only near-arithmetic is `u32::try_from(free.len())` (a fallible
conversion, not an op). No `checked_*` chains and **no new `#[allow(clippy::arithmetic_side_effects)]`**
are introduced; the BFS distance arithmetic stays inside `geodesic_bfs` (already handled).

**Part C — canonical-doc correction.** Exact replacement wording drafted below (§ *Part C —
exact doc wording*) so the code-writer applies it precisely (Russian for `design.md`, matching
its tone; English amendment blocks for the English `design-review.md`).

## Decomposition

| # | Task | Files | Depends on | Change-type |
|---|------|-------|------------|-------------|
| 1 | **Dependency stack.** Add `rand` + `rand_chacha` (pinned `0.10`/`0.10`, `default-features = false`, `rand` `features = ["alloc"]`) to both manifests; `cargo update` + `cargo build`; confirm `getrandom` absent from `cargo tree -p gp-core`. | `crates/core/Cargo.toml`, `crates/gen/Cargo.toml`, `Cargo.lock` | — | code |
| 2 | **gp-gen `GenParams::rng` + determinism test.** `pub fn rng(&self) -> ChaCha8Rng` via `seed_from_u64`; `///` doc; `#[cfg(test)]`: same seed ⇒ identical draw stream, different seed ⇒ different (non-vacuous). (AC9-gen, AC10, AC11) | `crates/gen/src/lib.rs` | 1 | code |
| 3 | **collision module + `resolve_collisions`.** Create `sim/collision.rs`; in `sim.rs` delete the stub, add `mod collision;` + `pub use collision::resolve_collisions;`. Implement: bucket grouping (`HashMap<Point, Vec<usize>>` → `sort_unstable_by_key(g[0])`), `ChaCha8Rng::seed_from_u64`, fixed shuffle order (groups then within-group), Phase 1 winners, Phase 2 reused-`CorridorScratch` BFS nearest-free with `u32` tie draw + occupied-after-each, exhaustion→stay. Full `///` doc (same-cell-only predicate; teleport, velocity-retained, no lap/supercover; **the AC3/AC7 determinism contract spelled out verbatim — the canonical `sort_unstable_by_key(\|g\| g[0])` ordering + the fixed `groups.shuffle` → per-group `shuffle` → per-loser `u32` tie-draw sequence — see Approach § algorithm**). | `crates/core/src/sim.rs`, `crates/core/src/sim/collision.rs` | 1 | code |
| 4 | **`resolve_collisions` behavioral tests (AC1–AC8).** Resolve: three-into-one-cell; displaced-into-occupied-ring; singleton unchanged; velocity preserved; repeated-call byte-identical (AC7); equidistant seeded pick (AC3). **Allowed:** two cars swapping cells / threading segments **ending on distinct cells** left **unchanged** (AC5). Pin a seed; assert exact positions + velocities. Mirror tiny `car()`/`filled()` helpers (**GO-note 4**: 2 test sites < ≥3-site threshold — mirror, no shared crate). | `crates/core/src/sim/collision.rs` | 3 | code |
| 5 | **`docs/design.md` §3/§6 corrections.** Apply the six edits from § *Part C — exact doc wording* (rewrite [D1] swap paragraph ~L279, [N2] paragraph ~L281, [D1] occupancy sentence ~L283, "Решённые" line ~L411, §6 review-order item 5 ~L426, §6 "Статус" N2 mention ~L430). Keep §3 ~L268–277 (the same-cell algorithm) intact. (AC12) | `docs/design.md` | — | instructions/harness |
| 6 | **`docs/design-review.md` [D1]/[N2] amendment + reconcile.** Append the 2026-07-16 product-owner amendment blocks to the D1 (~L152) and N2 (~L330) entries; annotate the roll-up line ~L303 and Round-3 verdict ~L368 (from § *Part C — exact doc wording*). No fabricated review round. (AC13) | `docs/design-review.md` | — | instructions/harness |

Scope: 6 tasks (≤ 15). Part A (#10) = tasks 3–4; Part B (#49) = task 2; shared = task 1;
Part C = tasks 5–6.

## Part C — exact doc wording

**`docs/design.md` (Russian, product-owner amendment 2026-07-16):**

1. **[D1] swap paragraph (~L279).** Replace the whole `**[D1] Swap/pass-through — отдельный
   чек, а не только same-final-cell.** …` paragraph with:
   > **[D1] Swap/pass-through — НЕ отслеживается (правка продукт-оунера, 2026-07-16): разрешаются только same-final-cell конфликты.** Ранее здесь предполагалась same-turn проверка, что две машинки не меняются клетками и не threading сквозь друг друга. **Отменено директивой продукт-оунера:** коллизия-слой дедупит **только** машинки, кончающие ход в **одной** клетке (`A.pos == B.pos`). Обмен клетками (A:P→Q, B:Q→P), threading mid-segment и ортогональные пересечения отрезков, **кончающиеся в разных клетках**, — разрешены и не корректируются. Согласовано с уже принятым [D1(a)] «traffic-физика не вводится, узкие места позиционные/визуальные»: у машинок, кончающих ход в разных клетках, occupancy-конфликта нет, и форсировать смещение было бы избыточно.

2. **[N2] paragraph (~L281).** Replace the whole `**[N2] Разрешение обнаруженного swap — …`
   paragraph with:
   > **[N2] Разрешения swap нет (правка продукт-оунера, 2026-07-16).** Поскольку сам swap/pass-through чек отменён (см. [D1] выше), отдельного правила «что происходит при обнаруженном swap» не требуется. Через сидированный nearest-free BFS (§ коллизии выше) прогоняются **только** same-final-cell конфликты: одна машинка остаётся/выигрывает по шафлу, остальные смещаются на ближайшую свободную; скорость сохраняется, счётчик не трогается.

3. **[D1] occupancy sentence (~L283).** Replace the final sentence `Swap/pass-through чек выше
   — единственная обязательная правка коллизий; traffic-физика не добавляется.` with:
   > Коллизия-слой разрешает **только same-final-cell конфликты** (`A.pos == B.pos`); swap / mid-segment / ортогональные пересечения, кончающиеся в разных клетках, разрешены (правка продукт-оунера, 2026-07-16); traffic-физика не добавляется.

4. **"Решённые" line (~L411).** Replace `Swap/pass-through чек — обязательный correctness-фикс
   независимо от этого.` (within the `Occupancy-механика — РЕШЕНО (a) [D1]` bullet) with:
   > Коллизия-слой разрешает только same-final-cell конфликты (`A.pos == B.pos`); swap/pass-through чек отменён правкой продукт-оунера (2026-07-16) — пересечения, кончающиеся в разных клетках, разрешены.

5. **§6 review-order item 5 (~L426).** Replace `5. **D1** — swap/pass-through чек (correctness)
   + решение по occupancy (design).` with:
   > 5. **D1** — occupancy решено (a); коллизии — только same-final-cell (swap/pass-through чек отменён правкой продукт-оунера 2026-07-16).

6. **§6 "Статус" line (~L430).** Replace the substring `N2 (разрешение swap через
   nearest-free), N4` with:
   > N2 (разрешение swap — отменено правкой продукт-оунера 2026-07-16: коллизии только same-final-cell), N4

**`docs/design-review.md` (English; append/annotate — no new review round):**

7. **D1 entry — append after the Recommendation (~L152, before `### D2`):**
   > **Amendment — 2026-07-16 (product-owner directive, supersedes the swap/pass-through recommendation).** Option (a) is adopted in full and the same-turn swap/pass-through check is **dropped**: the collision model resolves **same-final-cell conflicts only** (`A.pos == B.pos`). Cars whose move-segments swap, thread mid-segment, or cross orthogonally but **end on distinct cells** are allowed unchanged — under (a) narrows are positional/visual, so two cars ending apart have no occupancy conflict and a forced displacement would be gratuitous. This supersedes the "At minimum, add a same-turn swap/pass-through check" sentence above. Recorded against this entry — not a new review round.

8. **N2 entry — append after the paragraph (~L330):**
   > **Amendment — 2026-07-16 (product-owner directive, supersedes N2).** With the swap/pass-through check dropped (see the D1 amendment), there is no detected swap to resolve: the seeded nearest-free placement runs for **same-final-cell conflicts only**. N2's "fold a detected swap into the collision layer" resolution is moot. Recorded against this entry — not a new review round.

9. **Round-2 roll-up (~L303).** Replace `D1 option (a) + mandatory swap check ✅` with:
   > D1 option (a); mandatory swap check **superseded 2026-07-16** (see D1 amendment) ✅

10. **Round-3 verdict (~L368).** Replace the substring `N2 swap→nearest-free,` with:
    > N2 swap→nearest-free (later **superseded 2026-07-16** — see the N2 amendment: same-cell-only),

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. **M = 6**, two change-types:
subtasks 1–4 are **code** (Rust `*.rs` + `Cargo.toml`/`Cargo.lock`), subtasks 5–6 are
**instructions/harness** (`docs/*.md`). Change-type homogeneity forces a boundary between
them; minimization packs each change-type into ONE group → **2 groups** (≤ 4, no user gate;
both within the size cap of 10).

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, 1M-token window,
  via the `code-writer` subagent — subtasks **1–4** (code change-type). Entered in its own
  `/context-reset` subagent (per `.claude/skills/context-reset/SKILL.md` § Compaction recovery
  (re-entry)), per the every-group handoff contract. Size 4 ∈ `1..=10`.
- **Handoff after Group A:** spawn `/context-reset` per the same reference. Parent `/task`
  resumes in Group B with fresh context.
- **Group B** — model `opus`, effort **inherited from the orchestrator (typically xHigh) — NOT
  pinned**, 1M-token window, via the `general-purpose` subagent — subtasks **5–6**
  (instructions/harness change-type: `docs/design.md`, `docs/design-review.md`). **Terminal
  group** (2 subtasks; within `1..=10`).

The `design`, `design-review`, `self-review`, `spec-writer` gates stay on Opus regardless of
these markers — only the per-group implementor model + effort varies.

## Risks

- **Non-deterministic group order from `HashMap` iteration** would break AC7. *Mitigation:*
  materialize buckets to a `Vec` and `sort_unstable_by_key(|g| g[0])` (unique min car index)
  → canonical RNG-independent order *before* any shuffle. Explicit in the algorithm + `///`.
- **Cross-machine replay non-determinism** (a `usize` draw differs 32- vs 64-bit).
  *Mitigation:* tie index drawn as **`u32`** (rand's own policy); `shuffle` already samples
  `< u32::MAX` as `u32`; ChaCha + `seed_from_u64` are fixed integer algorithms.
- **RNG stream drift** breaking AC3/AC7. *Mitigation:* consumption order pinned in the `///`
  and design (`groups.shuffle` → per-group `shuffle` → per-loser tie draw only when
  `free.len() > 1`); grouping is RNG-independent.
- **`getrandom` leaking into the I/O-free core.** *Mitigation:* `default-features = false`;
  task 1 asserts `getrandom` absent from `cargo tree -p gp-core`.
- **`sim.rs` size** near the hard incl-test cap. *Mitigation:* new code in a fresh
  `sim/collision.rs`; `sim.rs` net-shrinks (stub → 2 lines).
- **Doc edits drifting from the residual mandatory-swap language.** *Mitigation:* AC12/AC13
  grep checks below; exact old→new pairs drafted in § Part C so the edits are mechanical.
- **`rand_core` version split** would break the blanket `Rng` impl. *Mitigation:* verified
  both crates require `rand_core ^0.10.0` → single unified `0.10.1`.

## Test Design

- **gp-gen determinism** — `crates/gen/src/lib.rs` `#[cfg(test)] mod tests`.
  - Entry: `GenParams::rng`. Scenarios: two `rng()` from the same `seed` yield an identical
    `next_u64` sequence (AC11); a different `seed` yields a different sequence (non-vacuous).
    Fixture: a `GenParams` literal helper.
- **`resolve_collisions` behavioral (AC1–AC8)** — `crates/core/src/sim/collision.rs`
  `#[cfg(test)] mod tests`. Entry: `resolve_collisions(d, cars, seed)`. Pin a concrete `seed`;
  assert **exact** final `CarState`s.
  - *AC8 resolve — three-into-one-cell:* three cars at one cell in `filled(N,N)` → winner
    stays, the other two on distinct nearest-free cells; velocities unchanged (AC6 distinctness).
  - *AC8 resolve — displaced-into-occupied-ring:* pre-place a car so the immediate ring around
    a two-car cell is occupied → the loser lands on the first *free* BFS layer (AC2).
  - *AC8 allowed — swap ending apart:* `A:(1,2)→(2,2)`, `B:(2,2)→(1,2)` (distinct final cells)
    → **both unchanged** (positions + velocities intact) (AC5).
  - *AC8 allowed — thread ending apart:* opposing multi-cell segments with overlapping
    supercovers but distinct final cells (e.g. `A:(0,2)→(2,2)`, `B:(2,2)→(0,2)`) → **both
    unchanged** (AC5).
  - *AC1 singleton:* a lone non-colliding car returned unchanged.
  - *AC4:* `vx`/`vy` invariant across every displacement; no `LapCounter` in the signature.
  - *AC7 determinism:* two clones, same seed → byte-identical `cars`; repeated calls reproduce.
  - *AC3 equidistant:* a symmetric fixture with a genuine intra-layer tie → the seeded pick is
    exact and stable per seed.
  - Fixtures: `car(x,y,vx,vy)` + `filled(w,h)` mirrored from `sim.rs`'s test module (GO-note 4).
- **AC12 (design.md) — verified by grep (no code):**
  - **All six pre-edit phrases must be ABSENT** (one alternation covering *every* design.md edit target, so a forgotten edit to L279/L281/L283/L411/L426/L430 cannot slip through):
    `rg -n "Swap/pass-through — отдельный чек|Разрешение обнаруженного swap|единственная обязательная правка коллизий|обязательный correctness-фикс|swap/pass-through чек \(correctness\)|разрешение swap через nearest-free" docs/design.md` → **empty**.
  - `rg -n "продукт-оунера|только same-final-cell" docs/design.md` → **present** (amendment applied; note the shorthand is spelled out as `продукт-оунера`, not the ambiguous `ПО`).
  - `rg -n "заносится в occupied|ближайшая свободная точка по геодезии" docs/design.md` → **still present** (§3 ~L268–277 same-cell algorithm preserved, unchanged).
- **AC13 (design-review.md) — verified by grep (no code):**
  - `rg -n "Amendment — 2026-07-16" docs/design-review.md` → **two hits** (D1 + N2 entries).
  - `rg -n "mandatory swap check ✅" docs/design-review.md` → **empty** (roll-up now reads "superseded 2026-07-16").
  - `rg -n "superseded 2026-07-16" docs/design-review.md` → present on the roll-up (~L303) and Round-3 verdict (~L368).

## Open questions

All spec Open questions are resolved (recorded for the reviewer):

- **Conflict predicate** → same-final-cell only (`A.pos == B.pos`); no swap/thread detector,
  no supercover-overlap, no velocity dot, no `from = pos − v` (all dropped by the 2026-07-16
  amendment; prior GO-note 1 *orthogonal crossing* resolved by it, GO-note 2 *`from = pos − v`
  precondition* moot).
- **`resolve_collisions` param shape** → `seed: u64` (no rand type in the public API).
- **No-free-cell exhaustion** → keep the car at its colliding position (non-panicking;
  degenerate; ties to the deferred radius-cap).
- **ChaCha variant** → `ChaCha8Rng` (non-crypto + AI-training hot path; ChaCha20 a trivial swap).
- **Group formation + deterministic RNG order** → bucket by `pos`, then
  `sort_unstable_by_key(|g| g[0])` for a canonical order, then `groups.shuffle` → per-group
  `shuffle` → per-loser `u32` tie draw only on a genuine tie; cross-arch-reproducible.
- **`alloc` feature on gp-core** (GO-note 3) → kept for uniformity + gp-gen's future sampling;
  documented as YAGNI-but-zero-cost.

No question requires product-owner input — none block implementation.
