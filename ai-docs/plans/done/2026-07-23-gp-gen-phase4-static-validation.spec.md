# gp-gen Ф4 — static validation (connectivity · single hole · width · finger liveness)

**Source:** issue #27
**Date:** 2026-07-23
**Tracked in:** #27

## Scope

Implement gp-gen phase **Ф4** (`docs/design.md` §2, `phase4_static_checks`) — the
cheap static-validation pass that runs every repair iteration and emits a typed
list of `Issue`s for the (future) Ф6 local-repair phase — plus the geometry
primitives it needs.

1. **`Issue` type** (new, in gp-gen) enumerating the five Ф4 findings:
   `DISCONNECTED`, `BAD_TOPOLOGY`, `NARROW(section)`, `NARROW_SF(section)`,
   `LOST_HAIRPIN(finger)`. Payloads carry enough locality for Ф6 to act on
   (exact shape is a design decision — see Open questions). The type must give
   the tests an exact, order-independent issue **set** to assert against.

2. **`phase4_static_checks`** — the orchestrator. Consumes the fine corridor `D`
   plus the width floors `n` (global) and `m` (start/finish) and the S/F line,
   and whatever skeleton/section context the sub-checks need (see Key decisions),
   and returns `Vec<Issue>` (or an equivalent set/collection). Runs the four
   checks in a fixed order; empty result ⟺ statically valid.

3. **The four static checks:**
   - **Connectivity** → `DISCONNECTED` unless `D` is a single 4-connected
     component. Reuses `gp_core::geom::component_count` (#5).
   - **Topology** → `BAD_TOPOLOGY` unless the complement `¬D` has exactly one
     bounded component of `≥ 1` point. Reuses
     `gp_core::geom::bounded_complement_components` (#5), whose count already
     excludes empty and border-touching (unbounded) components and counts only
     `≥ 1`-cell holes.
   - **Width** → `NARROW(section)` for every cross-section whose width `< n`,
     and `NARROW_SF(section)` for every cross-section on the S/F line whose
     width `< m`. Uses the distance-transform as a cheap pre-filter (only
     sections near a DT-low cell can be narrow) followed by an **exact
     cross-section point count** for confirmation (`docs/design.md` §2 Ф4:
     "distance-transform как пре-фильтр + точный подсчёт по поперечным
     сечениям").
   - **Finger liveness** → `LOST_HAIRPIN(finger)` for every expected infield
     finger (S-hairpin, `docs/design.md` §1) that has been absorbed — its
     separating infield strip pinched so the two flanking corridor arms merged.

4. **Reusable geometry primitives** (AC5 — "consumed by centerline"), integer-only
   and deterministic, over `gp_core::geom::Corridor`:
   - **Distance transform** — 4-connected (Manhattan) distance from each `D`
     cell to the nearest `¬D` cell (`docs/design.md` §1: 4-connectivity is the
     fixed metric for width / distance-transform / topology).
   - **Medial axis** — the ridge of the distance transform (`docs/design.md`
     §"Два разных «центра»" / D2): a branching set/graph of ridge cells, about
     *width*, requiring no arc-length parameterization. Provides the normals the
     cross-section sampler walks along ("нормаль из скелета"/medial axis, §2 Ф4
     comment).
   - **Cross-section width** — the exact perpendicular in-`D` point count at a
     section.

## Out of scope

- **`DYNAMICALLY_DISCONNECTED` / V=1 liveness** (`oracle_liveness_V1`) — a
  *separate* call in the pipeline loop (`docs/design.md` §2 lines 101–102), not
  part of `phase4_static_checks`. Its own future issue.
- **Ф5 full oracle**, **Ф6 local repair** (consumes these `Issue`s), **Ф7
  output**. Ф4 only *emits* issues; nothing repairs them here.
- **`s_field` and the racing `centerline` curve** — Ф7 products. This task ships
  only the DT + medial-axis primitives centerline will later consume, not the
  curve construction (trim-to-loop / arc-length resample / `racing_line`).
- Wiring Ф4 into the top-level `generate` pipeline (`lib.rs::generate` stays
  `todo!`).

## Deferred

- Exact medial-axis extraction algorithm and output shape | geometric-detail,
  design-owned | no separate issue (part of this task's design phase).
- Crate home of the DT / medial-axis / cross-section primitives | see Open
  questions | no separate issue.

## Key decisions

| Question | Decision |
|---|---|
| Connectivity + topology checks | Delegate to the existing #5 gp-core helpers (`component_count`, `bounded_complement_components`) — do not reimplement 4-conn flood-fill. `bounded_complement_components == 1` already encodes "exactly one bounded hole of ≥ 1 point". |
| Distance-transform metric | 4-connected (Manhattan) distance to the nearest `¬D` cell (`docs/design.md` §1 fixes 4-connectivity for the width/DT metric). Integer-only. |
| Width check strategy | DT as a cheap pre-filter (skip sections that are provably wide), then an exact cross-section point count to decide `< n` / `< m`. Both per `docs/design.md` §2 Ф4. |
| S/F width floor | `NARROW_SF` fires only for sections lying on the S/F line, with floor `m` (= `GenParams::start_finish_width`, cars abreast); the global `NARROW` floor is `n` (= `GenParams::min_width` = ⌈m/2⌉). |
| Cross-section normals | Derived from the medial axis / skeleton ("нормаль из скелета", §2 Ф4). `phase4_static_checks` therefore receives the section/normal context it needs (skeleton and/or medial axis) in addition to `(D, n, m, sf)`; the literal pseudocode signature is schematic. |
| Infield-finger reference | Expected fingers come from the skeleton's infield hole `P` (Ф1's `CoarseSkeleton.hole` protrusions); `absorbed` is checked against the current `D`. S-hairpin minimum cross gauge = two arms (`≥ n` each) + infield neck (`≥ 1`) = `≥ 2n+1` points (`docs/design.md` §1). |
| Determinism / totality | Integer-only and deterministic (`docs/design.md` §3a); total — no production panic, no `Result` (mirrors Ф1/Ф2/Ф3 `saturating_*` discipline). Byte-identical output across repeated calls on identical input. |
| Primitive placement (call sites) | Two consumers today, both in gp-gen: Ф4 (this task) and the future Ф7 centerline. Design chooses the home crate (gp-core geom alongside the #5 helpers, vs. gp-gen-local + `pub` re-export). Flagged, not baked in. |

## Technical constraints

- Crate: `gp-gen` (`crates/gen/`), a new `phase4` module wired into `lib.rs`
  beside `phase1`/`phase2`/`phase3`. New primitives land wherever design places
  them (Key decisions).
- Physics/geometry core is integer-only and deterministic (`docs/design.md`
  §3a; AGENTS.md § Code Style) — no floating-point arithmetic in the DT /
  medial-axis / cross-section / check code.
- Reuse `gp_core::geom` (`Corridor`, `Point`, `Side`, `component_count`,
  `bounded_complement_components`, `flood_fill`, `CorridorScratch`) and
  `gp_core::track::{StartFinish, RaceDir}` / `gp-gen::{CoarseSkeleton,
  Phase3Output}` rather than adding parallel primitives.
- File-size soft/hard caps (AGENTS.md § Code Style): split `phase4` if it grows
  past the 500/800 (excl./incl. tests) soft line.
- Every public item carries a `///` doc; strict clippy (`-D warnings`); Miri
  gate applies (pure integer code — no FFI, expected Miri-clean, no `-ignore`
  gating anticipated).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `phase4_static_checks` reports `DISCONNECTED` iff `D` is not a single 4-connected component; a clean single-component `D` yields no `DISCONNECTED`. |
| AC2 | Reports `BAD_TOPOLOGY` iff the complement `¬D` does not have exactly one bounded component of `≥ 1` point (annulus→disk merge, or ≥ 2 holes, both trip it; a valid annulus does not). |
| AC3 | Reports `NARROW(section)` for a cross-section with width `< n`, and `NARROW_SF(section)` for an S/F-line cross-section with width `< m`; sections `≥` their floor produce neither. |
| AC4 | Reports `LOST_HAIRPIN(finger)` when an expected infield finger is absorbed (its arms merged / neck pinched below the `2n+1` gauge); a finger that survives produces none. |
| AC5 | A distance transform and a medial axis are available as reusable, deterministic, integer-only primitives over `Corridor`, with their own unit tests, positioned so the future centerline (Ф7) can consume them. |
| AC6 | A clean, valid hand-built ring (single component, one hole, all sections `≥ n`, S/F `≥ m`, fingers intact) produces an **empty** issue list. |
| AC7 | Each hand-built adversarial corridor — a pinch `< n`, a merged-hole (annulus→disk), a too-thin S/F, an absorbed finger — produces **exactly** its intended issue set (asserted as an order-independent set), and no spurious extra issue. |
| AC8 | All Ф4 output is deterministic (byte/set-identical across repeated calls) and integer-only; the whole path is total (no production panic, no `Result`). |
| AC9 | Files with ~50+ lines of substantial logic carry a `#[cfg(test)] mod tests`; `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and the doc gate pass. |

## Open questions

- **Home crate for the DT / medial-axis / cross-section primitives** — gp-core
  `geom` (shared, alongside the #5 flood-fill family; matches AC5's "reusable")
  vs. gp-gen-local with a `pub` re-export (matches D2's "medial axis lives in
  Ф4"). Two consumers today, both gp-gen. Design's call.
- **Exact medial-axis extraction algorithm and output shape** (set of ridge
  cells vs. a graph; tie-breaking on plateaus of the DT) — geometric detail for
  the design phase; only the *contract* (deterministic, integer, consumed by
  centerline) is fixed here.
- **`Issue` payload shape** — how much locality `NARROW` / `NARROW_SF` /
  `LOST_HAIRPIN` carry so Ф6 can map an issue to the dual edge / wall it must
  move (`docs/design.md` §2 Ф6). Design-owned; the ACs constrain only the issue
  *identity*, not the payload.
- **Whether NARROW / LOST_HAIRPIN can fire on real Ф1→Ф3 output in round 1**, or
  are defensive checks exercised only by the hand-built adversarial fixtures
  (mirrors Ф2's "round-1 never carves narrow" carve-vacuity). Does not block
  design — the checks must be correct on adversarial input regardless.
