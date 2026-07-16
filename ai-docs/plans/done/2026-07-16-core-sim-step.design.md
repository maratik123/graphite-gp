# Design: gp-core sim step + legal_move/legal_mask verification

**Issue:** #7
**Date:** 2026-07-16

## Approach

The genuine code delta is a single function: replace the `sim::step` `todo!()`
stub (`crates/core/src/sim.rs:130`) with the pure kinematic update from
`docs/design.md` §3 — **accelerate first, then advance by the new velocity**.
Everything else on the movement path (`legal_move`, `legal_mask`,
`Action`/`Action::ALL`/`Action::accel`, `CarState`/`CarState::pos`) already
exists and is correct on `main`; this task only *confirms* those with tests.

### Key decisions — resolved

Both spec Key-decision defaults are finalized here to concrete choices:

**1. Drop the `d: &Corridor` parameter → `pub const fn step(s: CarState, a: Action) -> CarState`.**
The §3 kinematic formula `(x+vx', y+vy')` reads no `D`; `step` is the
assumed-legal update and does no legality check (that is `legal_move`'s sole
job — Key decision "Single legality path"). Verified **zero callers**:
`ast-index callers step` and `rg -U 'step\s*\(' crates/ --type rust` return only
the stub definition itself, so the arity change breaks nothing. AGENTS.md
§ *API Stability* mandates the clean break (no `_d` shim); `CARGO_BUILD_WARNINGS=deny`
(CI) forbids a lingering underscored `_d` anyway. No concrete forward-looking
need for `d` exists — `step` is purely kinematic by design.

**2. Overflow handling → the fn-level documented `#[allow]` house pattern, plain `+`.**
`step` performs four adds: `vx' = vx + ax`, `vy' = vy + ay`, `x' = x + vx'`,
`y' = y + vy'`. Under the assumed-legal precondition these are **exactly** the
four sums `legal_move` computes via its `checked_add` chain
(`sim.rs:99-110`: `vx2`, `vy2`, `px`, `py`) and proves in-`i32` before returning
`true`. So on the domain `step` is contracted for (an action legal under
`legal_move`/`legal_mask`), plain `+` never overflows.

Chosen form: a fn-level `#[allow(clippy::arithmetic_side_effects, reason = …)]`
with an assumed-legal precondition doc + covering tests — the established house
pattern (`supercover` `geom/mod.rs:354`, `Size::area` `:110`, `Rect::index` `:172`),
and one of the two forms explicitly blessed by the workspace lint comment
(`Cargo.toml:51-55`). **Rejected: `wrapping_add`/`saturating_add`.** Those change
semantics on out-of-domain input — they would return a *wrong-but-non-panicking*
`CarState` for an illegal action, silently masking a caller bug, whereas the
`#[allow]`+precondition documents the contract honestly ("caller passes a legal
action; out-of-domain is unsupported") and keeps the body a literal transcription
of the §3 formula. The covering tests for the domain bound are the AC1/AC2/AC5
`step` tests (all exercise in-domain legal states).

**3. Return type `-> CarState` (infallible).** Per spec Key decision — no
`Option`/`Result`; legality is upstream.

**4. `#[inline]`, `const fn` (required).** Small hot function on the movement
path; `#[inline]` matches `Action::accel`/`CarState::pos`. `const fn` is
**required, not optional**: every op in the body (`Action::accel`, integer `+`,
`CarState` construction) is const-eval-legal, so `clippy::nursery::missing_const_for_fn`
— denied via the workspace `-D warnings` gate that § Technical constraints itself
mandates — forces `const`. A non-`const` `step` fails that clippy gate; shipped
form is `pub const fn step(s: CarState, a: Action) -> CarState`. `step` consumes
`Action::accel()` for `(ax, ay)` and does **not** re-encode the action table
(spec Technical constraint).

### Doc comment (replaces the TODO stub doc)

`step` is public, so it needs a real `///` (the `missing_docs` deny). Content:
the accelerate-then-advance rule (one line minimum), the assumed-legal
precondition, and the overflow precondition backing the `#[allow]` (cite that the
four sums equal `legal_move`'s proven-in-range `checked_add` chain). Formatting
constraint: `clippy::doc_link_code` (nursery, `-D warnings`) fires when an
intra-doc link is adjoined directly against an inline-code span — separate them
(e.g. keep prose/whitespace between a `` [`legal_move`] `` link and an adjacent
`` `…` `` span) so the doc gate stays green.

### TDD note

`legal_move`/`legal_mask` already exist, so the AC3/AC4 confirmation tests are
*written against live code* — they pass on first run (that is the confirmation).
For `step`, the signature change and body are coupled (a 2-arg call won't compile
against the 3-arg stub), so subtask 1 lands signature + body together; the
implementer drafts the AC5/AC1 expected-state assertions as the target before
writing the body (TDD intent), and subtask 2 commits them.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Implement `step`: drop `d` → `pub const fn step(s: CarState, a: Action) -> CarState` (`const` required — see § Approach point 4); body = accelerate-then-advance via `Action::accel` and plain `+`; fn-level `#[allow(clippy::arithmetic_side_effects, reason = …)]` + `#[inline]`; real `///` doc (accel-then-advance + assumed-legal + overflow precondition). Removes the `todo!`. | `crates/core/src/sim.rs` | — |
| 2 | `step` behavioural tests (AC5, AC1, AC2): five-actions-from-rest exact states; non-zero-velocity accelerate-then-advance case; determinism assertion. | `crates/core/src/sim.rs` (`#[cfg(test)] mod tests`) | 1 |
| 3 | `legal_move` confirmation test (AC3): hand-built corridor — clear chord through `D` legal, fast chord whose supercover clips a wall (with `p1 ∈ D`) illegal. | `crates/core/src/sim.rs` (`#[cfg(test)] mod tests`) | — |

AC4 requires **no** subtask: the existing `legal_mask_contains_exactly_the_legal_actions`
test (`sim.rs:253`) already asserts `mask.contains(a) == legal_move(d, s, a)` over
`Action::ALL`, non-empty, and `!= BitFlags::all()` — the full AC4 guarantee. The
group's final `cargo test` gate confirms it stays green.

## Handoff plan

`M = 3`, all subtasks change Rust code (`crates/core/src/sim.rs`) — one
homogeneous **code** group, minimized (no change-type switch, no dependency
break, ≤ 10). `M ≥ 1`, so this section is mandatory even for a single group.

- **Group A** — model `sonnet` (sonnet-5), effort `medium` (pinned in the
  `code-writer` frontmatter), 1M-token window — subtasks 1–3 (code change-type:
  `*.rs`). Routes at implementation to `subagent_type="code-writer"` (no inline
  `model=`/effort override — there is no per-invocation effort parameter). This
  group is entered via `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry), and
  is the **terminal** group (3 subtasks; within `1..=10`). No inter-group
  handoff — the single group completes Step 8 in its own `/context-reset`
  subagent. 1 group ≤ the default max of 4.

The `design`, `design-review`, and `self-review` gates stay on Opus regardless
of this marker — only the per-group implementor model + effort varies.

## Risks

- **Argument-order bug (advance by old vs new velocity).** Mitigation: AC1's
  non-zero-starting-velocity case (`vx=3, vy=1`, action `North` → expected
  `(3,2,3,2)`, *not* `(3,1,…)`) fails loudly if the body advances position by the
  old velocity. This is the single most likely implementation error and is
  directly asserted.
- **Overflow `#[allow]` scope creep.** Mitigation: the `reason` string is bounded
  to the assumed-legal domain and cites `legal_move`'s `checked_add` proof;
  matches the three existing house sites verbatim in structure. Clippy
  `-D warnings` gate confirms no residual raw-add violation (AC6).
- **`arithmetic_side_effects` gate aborts on first violation, masking later ones.**
  Low risk (only four adds, all in one fn), but the group must re-run
  `cargo clippy --workspace --all-targets -- -D warnings` after the `#[allow]`
  lands to confirm no additional site surfaced; surface any out-of-contract class
  to the orchestrator rather than absorbing it.
- **Non-vacuous AC3.** Mitigation: assert `d.contains(p1)` is `true` for the clip
  case so the rejection is provably from the *supercover* rule, not the endpoint
  check — otherwise the test could pass for the wrong reason.
- **`clippy::use_self` / doc-link denies** already active on this file; the new
  doc's intra-doc links (`[`Action::accel`]`, `[`legal_move`]`, `[`CarState`]`)
  must resolve — `RUSTDOCFLAGS="-D warnings" cargo doc` gate covers it.

## Test Design

All tests live in `crates/core/src/sim.rs` `#[cfg(test)] mod tests`. gp-core has
**no dev-dependencies** (no `rstest`/`pretty_assertions`) — use plain
`assert_eq!` and `for a in Action::ALL` iteration, matching the existing tests.
Do **not** introduce a new dev-dep.

### Subtask 2 — `step` behaviour

- **Entry point:** `const fn step(s: CarState, a: Action) -> CarState`.
- **AC5 — five actions from rest** (one test, e.g. `step_from_rest_shifts_by_action_delta`):
  fixed start `s = CarState { x: 5, y: 5, vx: 0, vy: 0 }`. Assert exact states:
  `Coast → (5,5,0,0)`; `East → (6,5,1,0)`; `West → (4,5,-1,0)`;
  `North → (5,6,0,1)`; `South → (5,4,0,-1)`. Velocity equals the applied delta;
  position shifts by the same delta.
- **AC1 — accelerate-then-advance ordering** (`step_advances_by_new_velocity`):
  `s = { x:0, y:0, vx:3, vy:1 }`, action `North` (accel `(0,1)`) → `v' = (3,2)`,
  `p1 = (0+3, 0+2) = (3,2)`. Assert `step(s, North) == CarState { x:3, y:2, vx:3, vy:2 }`.
  Advancing position by the *old* velocity would give `y = 1`; the `y = 2`
  assertion is what distinguishes new-v from old-v advance. Add a second
  `East`-with-nonzero-`vx` case (`s = { x:2, y:7, vx:-1, vy:4 }`, `East` →
  `v'=(0,4)`, `p1=(2,11)` ⇒ `CarState { x:2, y:11, vx:0, vy:4 }`) to cover the
  x-axis symmetrically.
- **AC2 — determinism** (`step_is_deterministic`): pick any non-trivial `(s, a)`;
  assert `step(s, a) == step(s, a)`. No RNG/I/O is reachable from the body, so
  equality is total.

### Subtask 3 — `legal_move` confirmation (AC3)

- **Entry point:** `legal_move(d: &Corridor, s: CarState, a: Action) -> bool`.
- **Fixture** (`legal_move_rejects_wall_clipping_chord`): `Corridor::new(Point::new(0,0), 4, 4)`;
  mark drivable `(0,0), (1,0), (2,0), (1,1)`; leave `(0,1)` (and all else) off-`D`.
  - **Clear chord (legal):** `s = { x:0, y:0, vx:2, vy:0 }`, `Coast` → `v'=(2,0)`,
    `p1 = (2,0)`. `supercover((0,0),(2,0)) = {(0,0),(1,0),(2,0)} ⊆ D` → assert
    `legal_move == true`.
  - **Wall-clipping chord (illegal):** `s = { x:0, y:0, vx:0, vy:1 }`, `East` →
    `v'=(1,1)`, `p1 = (1,1)`. `supercover((0,0),(1,1)) = {(0,0),(1,0),(0,1),(1,1)}`
    (dual-vertex, all four — locked by `geom`'s `dual_vertex_diagonal_all_four`
    test); `(0,1) ∉ D` → assert `legal_move == false`. Also assert
    `d.contains(Point::new(1,1)) == true` so the rejection is provably the
    supercover/§3 C4 rule, not the endpoint check (non-vacuous).

### AC4 — `legal_mask` exactness

No new test. Confirmed green by the existing `legal_mask_contains_exactly_the_legal_actions`
(`sim.rs:253`), which already asserts typed `BitFlags<Action>` membership equals
`legal_move` over `Action::ALL`. The group's `cargo test` gate re-runs it.

### AC6 — no `arithmetic_side_effects` violation

No dedicated test — validated by the `cargo clippy --workspace --all-targets -- -D warnings`
gate passing with the fn-level `#[allow]` + `reason` in place. The AC1/AC2/AC5
`step` tests are the "covering tests" the house pattern requires for the
documented domain bound.

## Open questions

None. Both spec Key-decision defaults (drop `d`; overflow via the house-pattern
`#[allow]`) are finalized above with justification. Spec § Open questions already
records nothing design-blocking.
