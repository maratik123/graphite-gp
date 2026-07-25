# Design: gp-game controller abstraction + player input controller

**Issue:** [#42](https://github.com/maratik123/graphite-gp/issues/42)
**Spec:** [`ai-docs/plans/2026-07-25-game-controller-player.spec.md`](2026-07-25-game-controller-player.spec.md)
**Date:** 2026-07-25

---

## Approach

### The four spec-delegated open questions, settled

#### Q4 (first, because it constrains everything else) — module placement: **`gp-game` GAINS a lib target**

The spec left this open on the grounds that in-crate `#[cfg(test)] mod tests` already
run inside the bin target, so tests are reachable either way. That is true but is
**not the binding constraint**. The binding constraint is `dead_code`.

Under R2-Q2 the controller seam has **no in-binary consumer** — `main.rs` behaviour is
unchanged, so nothing reachable from `fn main` ever names `Controller`, `PlayerController`,
`Roster`, or the key map. In a **bin-only** crate, rustc's `dead_code` lint fires on
every item unreachable from `main`, *including `pub` items*, and `cargo clippy
--workspace --all-targets -- -D warnings` (AC14) turns that into a hard error.
`#[cfg(test)]` usage does **not** satisfy the lint, because the non-test build of the
bin target is compiled too.

`[measured: minimal repro at <scratch>/dctest — bin-only crate, src/main.rs with
`mod ctrl;`, ctrl.rs holding a `pub fn poll` used only from its own `#[cfg(test)]`
module; `cargo clippy --all-targets -- -D warnings` → GATE-RED,
`error: function \`poll\` is never used` / `error: could not compile \`dctest\` (bin "dctest")`]*

`[measured: same crate, after moving `pub mod ctrl;` behind a new `src/lib.rs` and
leaving `src/main.rs` as a bare `fn main`; `cargo clippy --all-targets -- -D warnings`
→ GATE-GREEN]`

`[measured: same crate with an explicit `[[bin]] name = "dcbin" path = "src/main.rs"`
alongside the auto-detected `src/lib.rs`; `cargo metadata --no-deps` →
`[('dctest', ['lib'], 'lib.rs'), ('dcbin', ['bin'], 'main.rs')]`; clippy → GATE-GREEN]`
— so `crates/game`'s existing explicit `[[bin]] name = "graphite-gp" path = "src/main.rs"`
block does **not** suppress lib auto-detection, and no `[lib]` section is needed.

Rejected alternatives:

| Alternative | Why rejected |
|---|---|
| Keep bin-only, reference the module from `main.rs` | Violates R2-Q2 (`main.rs` behaviour unchanged) or adds a dead call. |
| `#![allow(dead_code)]` on the module | AGENTS.md § *Rust Test Conventions*: no `#[allow(dead_code)]` unless unavoidable — and it **is** avoidable. |
| `#[cfg(test)] mod controller;` | Makes the #43-facing seam test-only code; #43 would have to un-gate it, and AC1/AC6 require documented **public** items. |

**Shape:**

```
crates/game/src/lib.rs            (new)  //! crate docs + `pub mod controller;`
crates/game/src/controller/mod.rs (new)  Controller, PollContext, FrameInput, Roster
crates/game/src/controller/keys.rs(new)  KEY_ORDER, action_for_key, keyboard_action
crates/game/src/controller/player.rs(new) PlayerController
crates/game/src/main.rs           (unchanged)  keeps its own `mod config;` subtree
```

Multi-file module directory mirrors the crate's own `src/config/{mod,cli,echo,error}.rs`
precedent `[measured: `find crates/game -name '*.rs'` → `src/config/{cli,echo,error,mod}.rs`,
`src/main.rs`, `tests/cli.rs`]`. `main.rs` does **not** `use gp_game::…`; the bin keeps its
independent module tree, so all 42 existing in-bin tests keep running
`[measured: `grep -rn '#\[test\]' crates/game/src | wc -l` → 42; per-file:
`config/cli.rs` 18, `config/mod.rs` 18, `config/error.rs` 5, `config/echo.rs` 1,
`main.rs` 0]`.

**Naming:** trait `Controller`, method `poll` (the owner's R1-Q1 word), seat
`PlayerController`, collection `Roster`.

**Consequence to handle:** `crates/game/tests/cli.rs`'s module doc currently asserts
*"a `gp-game` lib target buys nothing else"* `[measured: `sed -n '1,10p' crates/game/tests/cli.rs`
→ lines 5-6 `since a`gp-game` lib target buys nothing else (design § *Resolved spec
hand-offs* #1)`]`. That LIVE claim is refuted by this design and must be corrected in
the same PR (AGENTS.md § *Propagation Rule* step 4). Only the `//!` prose changes — the
three `#[test]` fns stay byte-identical, so AC12 holds. The history surface
(`ai-docs/plans/done/**`) is deliberately **not** touched.

**AC12 scope ruling (settled at `design-review`; recorded here so it is not re-litigated
at `self-review`).** AC12's clause *"`crates/game/tests/cli.rs`'s three process-level tests
pass unmodified"* scopes to **the three `#[test]` fns**, **not** to the whole file. A
`//!`-module-doc-only edit that leaves every `#[test]` body byte-identical **satisfies**
AC12 — the tests are unmodified in the sense AC12 means, and they still pass. Consequently
this prose correction is **NOT a spec amendment**: it triggers no `/task` Step 6 → 7
re-run, no AC edit, and no owner escalation. `self-review` should verify the *mechanical*
form of the claim (the `#[test]` bodies are byte-identical and the three tests pass) rather
than reopening the scope question, which is closed. The mechanical check is AC12's row in
§ *Test Design* → subtask 7.

#### Q1 — roster ownership: **the `Roster` type lives in `gp-game`'s controller module**

AC8 requires a test that *constructs* a roster mixing `PlayerController` with a
non-player stub **now**. #43 does not exist, so "assembled by #43's loop" leaves AC8
with nothing but an ad-hoc `Vec<Box<dyn Controller>>` in a test — which #43 and #158
would then each re-invent. One definition, owned next to the trait.

Kept deliberately minimal (YAGNI): a newtype over `Vec<Box<dyn Controller>>` with
`new` / `push` / `poll(index, ctx)` / `len` / `is_empty`. **No** turn order, no
scheduling, no per-seat state, no seat-kind enum — all of that is #43's. The newtype
(rather than a `pub type` alias) exists for one concrete reason: `poll` uses
`self.seats.get_mut(index)` and returns `Option<Action>`, so an out-of-range index is
**total** rather than a panicking `roster[i]`. `ai-docs/panic-index.md` covers *all*
production code, not just `gp-core` `[measured: `head -4 ai-docs/panic-index.md` →
"Every intentional panicking call … in **production** code (outside `#[cfg(test)]`) …
Kept in sync by `self-review` … and the `panic-gate` hook"]`, so a `pub type` alias
would export the panicking-index idiom to #43's call site.

**No new panic-index rows.** The whole `gp-game` addition is free of `unwrap` /
`expect` / `panic!` / `debug_assert!` / indexing.

#### Q2 — same-frame input precedence: **one uniform "first legal candidate wins" scan**

One rule, applied at two levels, evaluated in this documented order per frame:

| Step | Rule |
|---|---|
| 0 | **Singleton-`{Coast}` auto-resolve** (AC5): if `legal == BitFlags::from(Action::Coast)`, return `Some(Action::Coast)` **before** consulting any input. |
| 1 | Candidate sources are scanned in order `[shell_action, key_action]`; the **first `Some(a)` whose `a ∈ legal`** wins. |
| 2 | If no source yields a legal action → `None` ("no answer yet, ask again next frame"). |

Sub-orders, both already fixed and both left untouched:

- **Inside `shell_action`** — gp-render's existing precedence is preserved verbatim: a
  "Coast (·)" button click wins over a `MovePad` change `[measured: `crates/render/src/screens/race.rs:252-258`
  → `let action = if coast_response.clicked() { Some(Action::Coast) } else if
  movepad_response.changed { movepad_response.selected } else { None };`]`. `race.rs`
  is **not edited** (AC11).
- **Inside `key_action`** — `KEY_ORDER` is scanned in **`Action` declaration order**
  (`Coast, East, West, North, South`), arrow key before letter key within a pair. The
  crate already treats `Action` declaration order as the single ordering convention:
  `MovePad::MOVES` is documented as "the plus-layout table, in `Action` declaration
  order" `[measured: `crates/render/src/widgets/movepad.rs:40-41`]` and `legal_mask` is
  documented as "the legal-action mask for `s`, **in `Action` declaration order**"
  `[measured: `crates/core/src/sim/mod.rs:109-113`]`.

**Why shell before keyboard:** it makes the documented total order a strict *extension*
of gp-render's existing order — one rule appended, none reordered — so `race.rs` needs
no edit (AC11), and a pointer click is a single hit-tested event per frame whereas a
keypress can arrive with `repeat`. *Rejected:* keyboard-first (a held key would
override a deliberate click and would invert the existing order's spirit).

**Why "first *legal* candidate" rather than "first candidate, then mask":** AC3 says an
illegal input is a **no-op**, and a no-op must not *suppress* a valid one. The
fall-through form is also the exact mirror of gp-render's structural masking — an
illegal `MovePad` cell never receives `Sense::click`, so it never becomes a candidate
in the first place `[measured: `crates/render/src/widgets/movepad.rs:250-256` — the
`for cell in &MOVES` loop `continue`s on `!self.legal.contains(cell.action)` at **:251**
before reaching the `ui.interact(…, Sense::click())` at **:256**]`.
*(Locator note: the spec cited both `:244` and `:251` for this fact. `:244` is
`pub fn show`'s signature line and `:237-238` is the doc sentence; the load-bearing
guard is the `continue` at **:251**, and the `Sense::click()` call it guards is at
**:256**. Cite `movepad.rs:251` — that is the correct locator.)*

**Masking the shell action is NOT redundant.** The "Coast (·)" `Button` is **not**
gated by `legal`: `draw_your_move(ui, legal)` passes `legal` only to `MovePad::new(legal)`
and builds the Coast `Button` unconditionally `[measured: `crates/render/src/screens/race.rs:372-405`
— `Button::new("Coast (·)").variant(…).size(…).full_width(true).show(ui)`, no `legal`
reference]`, and `race.rs:252` maps its click to `Some(Action::Coast)` **unconditionally**.
So `RaceResponse.action` — and therefore `ShellResponse.action` — **can carry an illegal
`Coast`**, and AC2 is only satisfiable if `PlayerController` masks it. This is a real
path, not a hypothetical.

#### Q3 — how AC10 is discharged: **`egui_kittest` interaction test, Miri-gated**

The Miri-clean structural route was evaluated first and **rejected on evidence**: it
cannot be made non-tautological without adding production surface to `gp-render` beyond
the owner-ruled "exactly ONE non-drawing plumbing edit" (R2-Q1) — the forwarding *is* a
single field copy, so a pure helper to unit-test would have to be invented for the test's
benefit. The interaction route keeps `gp-render`'s production diff at exactly
`ShellResponse.action` + its forwarding.

The reason an interaction test looked hard, and how it is solved:

- `ShellResponse` today is `{ screen: Screen, advance_rect: Rect }`
  `[measured: `crates/render/src/app.rs:375-383`]`, and for `Screen::Race` `advance_rect`
  is the **Finish** button's rect `[measured: `app.rs:311-314` → `(resp.finish.then_some(Nav::Finish), resp.finish_response.rect)`]`
  — so the shell exposes no handle on the `MovePad`.
- `gp-render` sets **no** `WidgetInfo`/AccessKit labels anywhere, so kittest cannot find
  either Race control by role or label
  `[measured: `grep -rn "widget_info\|WidgetInfo\|accesskit\|get_by_label\|get_by_role" crates/render/src --include=*.rs`
  → no match]` — which is exactly why the two existing interaction tests capture rects
  through `Rc<Cell<Rect>>` instead.
- egui's `Context` exposes **no** public widget-rect enumeration — only
  `read_response(id)` for an `Id` you already hold
  `[measured: `grep -n "pub fn read_response\|pub fn widget_rects\|pub fn pass_state" egui-0.35.0/src/context.rs`
  → only `1287: pub fn read_response`]`, and the pad's cell `Id`s are
  `response.id.with(cell.action)` off an auto-generated `Ui` id `[measured: `movepad.rs:256`]`.
- AC11 forbids editing `crates/render/src/{widgets,screens,tokens}/**`, so exposing the
  rect from `race.rs`/`movepad.rs` is off the table.

**Solution — a layout probe in a second `egui::Context`.** The test captures the shell's
own `ui.max_rect()` from the harness's rest frame, re-derives the body rect with the
shell's **own** `pub(crate) const TOP_BAR_H` `[measured: `crates/render/src/app.rs:32`
→ `pub(crate) const TOP_BAR_H: f32 = HEADER_PAD_Y * 2.0 + NAV_ITEM_H;` and `app.rs:231-232`
→ `let body_rect = Rect::from_min_max(Pos2::new(full.min.x, top_bar_rect.max.y), full.max);`]`,
and draws the **real** `RaceScreen` under that rect in a fresh `Context` to read
`movepad_response.rect`. **No layout constant is duplicated** — the pad rect comes from
the production widget code; only the one-line body-rect derivation is replayed, from the
shell's own constant. A drift in either produces a loud miss, not a silent pass.

Clicking the pad's centre selects `Coast`: the **production** `MOVES` table's first entry is
`MoveCell { action: Action::Coast, glyph: "·", row: 1, col: 1 }` — row 1, col 1 of the 3×3
`[measured: `grep -n "MOVES" crates/render/src/widgets/movepad.rs` → `45:const MOVES: [MoveCell; 5] = [`;
`sed -n '45,51p' crates/render/src/widgets/movepad.rs` → the `MOVES[0]` literal spans **:46-51**,
with `action: Action::Coast` at **:48**, `row: 1` at **:50**, `col: 1` at **:51**]`.
*(Locator correction: an earlier draft cited `movepad.rs:327` and a 4-tuple
`(Action::Coast, "·", (0,0), (1,1))`. That line is inside the `#[cfg(test)]`
`let expected: [ExpectedCell; 5]` **mirror** table, whose tuple shape is
`(action, glyph, accel, (row, col))`; the production `MoveCell` struct carries **no**
`accel` field. The fact is unchanged — only the locator and the shape are corrected to the
production definition at `movepad.rs:45-51`.)* And
the already-green `race_gallery.rs::race_screen_coast_and_movepad_emit_action` pins
exactly this (`MovePad` rect centre click → `RaceResponse.action == Some(Action::Coast)`)
`[measured: `crates/render/src/screens/race_gallery.rs:272-277`]`. That existing test
covers the *MovePad → `RaceResponse.action`* link; the new test covers the *new*
`RaceResponse.action → ShellResponse.action` link, completing AC10's chain.

Miri gating is **inherited, with the correct per-test cause**: `Harness::builder()`
calls `getcwd` via `egui_kittest`'s `kittest.toml` lookup — the same cause the two
existing no-`render()` interaction tests document, and explicitly **not** the golden's
Vulkan-`dlopen` cause `[measured: `crates/render/src/app_gallery.rs:356-362` and
`crates/render/src/screens/race_gallery.rs:194-199`]`.

### The `gp-game` seam

```rust
// controller/mod.rs
pub struct FrameInput { pub shell_action: Option<Action>, pub key_action: Option<Action> }
pub struct PollContext<'a> {
    pub track: &'a TrackArtifact,   // AC1 "track context"; #158's feature source
    pub state: CarState,            // AC1
    pub legal: BitFlags<Action>,    // caller-supplied; NON-EMPTY precondition (AC6)
    pub input: FrameInput,          // a seat that reads no UI input ignores it (AC8)
}
pub trait Controller { fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action>; }
pub struct Roster { seats: Vec<Box<dyn Controller>> }
```

**Why `legal` is caller-supplied rather than recomputed inside `poll`:** the scrub tick's
mask is **not** `legal_mask(corridor, state)` — it is `CrashOutcome::action_mask`, which
returns `BitFlags::from(Action::Coast)` while `scrub` holds `[measured: `crates/core/src/sim/mod.rs:389-395`]`.
AC5 requires that mask to flow through the seam unchanged, so recomputation would break it.

**Why `PollContext` carries `input` for every seat:** AC8 demands one uniform call site
with **no seat-kind branching**. An out-of-band setter on `PlayerController` would force
the caller to branch. An AI seat simply never reads `input`.

**AC7 compliance:** the controller layer computes no legality of its own. `legal.contains(a)`
is a membership test on a `BitFlags` produced by `gp_core::sim` — not an independent rule.
No `step`, no `legal_move` re-implementation, no seat-kind relaxation.

**AC1 ⇒ `legal_move` for the scrub tick too:** during a scrub tick the mask is `{Coast}`,
and `resolve_crash` guarantees that forced `Coast` is legal from the respawn state — "If
the forced `Coast` from `L` at the quenched velocity is still illegal, the fail-safe halves
the whole vector (toward zero) and rechecks, terminating at `v=(0,0)` in the limit"
`[measured: `crates/core/src/sim/mod.rs:432-437`]`. So the AC5 test can assert
`legal_move` on the returned action without contradiction.

`PlayerController::decide` (the pure, `&self`-free core; `poll` delegates to it):

```
if legal == BitFlags::from(Action::Coast) { return Some(Action::Coast) }   // AC5
[input.shell_action, input.key_action].into_iter().flatten()
    .find(|&a| legal.contains(a))                                          // AC2/AC3
```

An **empty** mask (out of contract, AC6) makes `find` vacuously `None` — total, no panic,
never an illegal `Some`.

**Scoping the AC5 rule to `{Coast}` only, not "any singleton".** Auto-resolving *any*
singleton mask (e.g. a forced `{West}` brake) would take a turn away from the player
without an owner ruling; Scope §4 and AC5 both name `{Coast}` specifically. Recorded as a
deliberate narrow reading, not an oversight.

`controller/keys.rs`:

```
pub const KEY_ORDER: [(Key, Action); 9] = [
    (Key::Space, Action::Coast),
    (Key::ArrowRight, Action::East),  (Key::D, Action::East),
    (Key::ArrowLeft,  Action::West),  (Key::A, Action::West),
    (Key::ArrowUp,    Action::North), (Key::W, Action::North),
    (Key::ArrowDown,  Action::South), (Key::S, Action::South),
];
pub fn action_for_key(key: Key) -> Option<Action>;                                 // AC4 table
pub fn keyboard_action(legal: BitFlags<Action>, pressed: impl Fn(Key) -> bool) -> Option<Action>;
```

`keyboard_action` takes a **predicate**, not an `&egui::InputState`. That keeps AC4's
table test Miri-clean with **no** `egui::Context`, no `InputState` construction, no GPU
and no kittest harness, and it leaves #43's production adapter a one-liner —
`ui.input(|i| keyboard_action(legal, |k| i.key_pressed(k)))`, one input-lock acquisition
per frame. All nine `Key` variants exist in egui 0.35 `[measured:
`grep -n "ArrowDown\|ArrowLeft\|ArrowRight\|ArrowUp\|    Space,\|^    W,\|^    A,\|^    S,\|^    D," egui-0.35.0/src/data/key.rs`
→ `10:ArrowDown 11:ArrowLeft 12:ArrowRight 13:ArrowUp 19:Space 122:A 125:D 140:S 144:W`]`,
and `key_pressed` exists on `InputState` `[measured: `egui-0.35.0/src/input_state/mod.rs:743`
→ `pub fn key_pressed(&self, desired_key: Key) -> bool`]`.

**No new `egui` dependency.** `gp-game` reaches `Key` through `eframe::egui`, exactly as
`main.rs` already does `[measured: `crates/game/src/main.rs:27` → `use eframe::egui;`;
`eframe-0.35.0/src/lib.rs:156` → `pub use {egui, egui::emath, egui::epaint};`]`. Adding a
second direct `egui` edge would only create a version-drift surface.

### Lint-posture verification of every proposed shape

The entire proposed API — trait + `PollContext` + `FrameInput` + `Roster` + `decide`'s
final body + `KEY_ORDER` + `action_for_key` + `keyboard_action` + the 3-tuple
`show_body` return — was compiled against a scratch crate reproducing the workspace lint
table **verbatim** (`missing_docs = deny`, clippy `pedantic`/`nursery` deny groups,
`arithmetic_side_effects = deny`, the three targeted allows) with `gp-core` as a path dep:

`[measured: <scratch>/lintprobe, `cargo clippy --all-targets -- -D warnings` → GATE-GREEN
(no error, no warning)]`

Notable individual results this discharges:

| Lint risk | Result |
|---|---|
| `clippy::unused_self` on `Controller::poll(&mut self, …)` for the stateless player seat | Does not fire — trait-impl method. GATE-GREEN. |
| `clippy::missing_const_for_fn` forcing `const fn` on `decide` | Does not fire. `decide`'s scan uses an iterator closure; the earlier `.then_some(…)` variant is likewise blocked because `bool::then_some` is `#[rustc_const_unstable(feature = "const_bool", issue = "151531")]` `[measured: `core/src/bool.rs:33-36`]` — the same class of blocker as the in-tree `Rect::index` counter-example. |
| `missing_const_for_fn` on `Roster::new` | **Does** fire (`Vec::new` is const) — `Roster::new` must be `pub const fn`. Confirmed green with `const`. |
| `clippy::type_complexity` on `show_body`'s new `(Option<Nav>, Rect, Option<Action>)` | Does not fire. GATE-GREEN. |

### The `gp-render` edit (exactly one, non-drawing)

1. `crates/render/src/app.rs`: add `use gp_core::sim::Action;` (the file currently imports
   only `gp_core::track::TrackArtifact` `[measured: `app.rs:26`]`).
2. Add the documented field to `ShellResponse` (`missing_docs = deny` ⇒ `///` required).
3. Widen `show_body`'s return from `(Option<Nav>, Rect)` to `(Option<Nav>, Rect, Option<Action>)`
   `[measured: `app.rs:269` → `fn show_body(&mut self, ui: &mut Ui, session: ShellSession<'_>) -> (Option<Nav>, Rect)`]`;
   `Screen::Race` yields `resp.action`, the other three arms yield `None`.
4. `AppShell::show` populates `ShellResponse { screen, advance_rect, action }` `[app.rs:259-262]`.

No pixel-producing statement changes, so every wgpu golden stays byte-identical (AC11).
A `pub` field on a lib-crate `pub` type re-exported at the crate root `[measured:
`crates/render/src/lib.rs:31` → `pub use app::{AppShell, Screen, ShellResponse, ShellSession};`]`
is reachable, so `dead_code` does not fire — and `gp-render` is lib-only `[measured:
`cat crates/render/Cargo.toml` → no `[[bin]]` and no `[lib]` section]`.

### The `gp-ai` edge removal

`crates/game/Cargo.toml:21` `gp-ai = { workspace = true }` is the **only** `gp-ai`/`gp_ai`
reference anywhere under `crates/game/` `[measured: `grep -rn 'gp_ai\|gp-ai' crates/game/`
→ exactly one hit, `crates/game/Cargo.toml:21`]`, and `gp-ai` is currently in the
resolved graph `[measured: `cargo tree -p gp-game | grep gp-ai` → `├── gp-ai v0.1.0 (…/crates/ai)`]`.
The workspace-root `[workspace.dependencies] gp-ai = { path = "crates/ai" }` and the
`crates/ai` member entry both stay `[measured: `cat Cargo.toml` → both present]`.

Lock-file handling: this is a genuine dep-graph delta, so AGENTS.md § *Dependency Versions*
applies. Use `cargo update --workspace` (**"Only update the workspace packages"**
`[measured: `cargo update --help` → `-w, --workspace  Only update the workspace packages`]`)
so no unrelated transitive bumps enter the lock, then `cargo build`, then confirm
`git diff --stat Cargo.lock` shows only `gp-game` losing its `gp-ai` edge.

---

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Remove `gp-ai = { workspace = true }` from `[dependencies]`. **This is a dependency-graph delta, so the full AGENTS.md § *Dependency Versions* procedure is MANDATORY and MUST NOT be collapsed into a bare `cargo build`:** (i) *name the changed constraint out loud* — "removed edge `gp-game → gp-ai` from `crates/game/Cargo.toml` `[dependencies]`"; (ii) `cargo update --workspace`; (iii) `cargo build`; (iv) **gate on `git diff --stat Cargo.lock` BEFORE `git add`**, confirming the only delta is `gp-game` losing its `gp-ai` edge. AC13(a)/(b)/(c). Full contract: § *Test Design* → subtask 1. | `crates/game/Cargo.toml`, `Cargo.lock` | — |
| 2 | Add the lib target: `src/lib.rs` (`//!` crate docs + `pub mod controller;`). Author `controller/mod.rs` — `Controller`, `PollContext<'a>`, `FrameInput`, `Roster` — with the non-empty-mask precondition documented on `PollContext::legal`, `Controller::poll` and `Roster::poll`. `Roster::new` is `pub const fn`. Tests: AC1 (stub seat, `legal_move` composition), AC6 (empty mask ⇒ `None`), AC7 (no `step`/`legal_move` re-implementation — assert the module names neither). | `crates/game/src/lib.rs`, `crates/game/src/controller/mod.rs` | 1 |
| 3 | `controller/keys.rs` — `KEY_ORDER` (9 entries, `Action` declaration order, arrow-before-letter), `action_for_key`, `keyboard_action(legal, pressed)`. Tests: AC4 full table + mask gating + multi-key precedence. | `crates/game/src/controller/keys.rs`, `crates/game/src/controller/mod.rs` (add `pub mod keys;`) | 2 |
| 4 | `controller/player.rs` — `PlayerController` + `decide` + `impl Controller`. Tests: AC2 (`CarState` table), AC3 (both paths), AC5 (`{Coast}` + scrub), AC9 (replay determinism). Append the AC8 heterogeneous-roster test to `controller/mod.rs`'s test module. | `crates/game/src/controller/player.rs`, `crates/game/src/controller/mod.rs` | 2, 3 |
| 5 | `gp-render` plumbing: `use gp_core::sim::Action;`, documented `pub action: Option<Action>` on `ShellResponse`, `show_body` → 3-tuple, `Screen::Race` forwards `resp.action`, other three arms `None`, `AppShell::show` populates the field. | `crates/render/src/app.rs` | — |
| 6 | AC10 tests in `app_gallery.rs`: `shell_race_arm_forwards_movepad_action` (probe + click) and `shell_non_race_screen_yields_no_action`. Both `#[cfg_attr(miri, ignore = "…getcwd…")]`. | `crates/render/src/app_gallery.rs` | 5 |
| 7 | Correct the refuted lib-target claim in `crates/game/tests/cli.rs`'s `//!` prose (test bodies untouched). Full gate sweep + mechanical AC checks (see § Test Design → *Gate sweep*). Re-run each `-D warnings` gate after the first clean pass. | `crates/game/tests/cli.rs` | 1–6 |

M = 7.

---

## Handoff plan

Per § Rules → handoff-grouping. Every subtask's diff is **code** — Rust `*.rs` plus the
crate's own `Cargo.toml`/`Cargo.lock` build manifest, which belongs with the code bucket
(it is not `*.md`, `.claude/**`, `AGENTS.md`, or `ai-docs/**`). Subtask 7's `cli.rs`
prose edit is inside a `.rs` file. The group is therefore homogeneous.

- **Entry into Group A:** spawn `/context-reset` per
  [`.claude/skills/context-reset/SKILL.md`](../../../.claude/skills/context-reset/SKILL.md)
  § *Compaction recovery (re-entry)* — mandatory at the start of **every** design-defined
  group, including the first.
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (`model: sonnet` + `effort: medium` are frontmatter-pinned; no
  inline `model=`/effort override), 1M-token window — subtasks **1–7** (code change-type:
  `*.rs` + `Cargo.toml`/`Cargo.lock`). **Terminal group** (7 subtasks; within the `1..=10`
  range and at or below the size cap of 10).

**Per-subtask reporting floor inside Group A.** Two subtasks carry an explicit
verbatim-evidence requirement that the group implementor must honour in its return summary,
because both are claims the orchestrator would otherwise have to take on trust:

- **Subtask 6** (the two-`egui::Context` layout probe — the hardest item in this `sonnet`
  group) — quote the literal `cargo test -p gp-render shell_race_arm_forwards_movepad_action`
  output (`test result:` line + `GATE-GREEN`/`GATE-RED` marker) from a saved log; a bare
  "PASS" is **not** acceptable. Full contract in § *Test Design* → subtask 6 →
  *Reporting requirement*.
- **Subtask 1** (the dependency-edge change) — name the changed constraint, then quote the
  literal `git diff --stat Cargo.lock` output **before** `git add`. Full contract in
  § *Test Design* → subtask 1.

Per AGENTS.md § *Workflow*, the orchestrator reconciles both against the durable record
(the commit, `git log`, the saved gate logs) rather than accepting the summary's assertion.

Group count: **1** (minimized — a single change-type, no dependency-forced split, within
the default max of 4, so no user gate is needed). No inter-group handoff exists; the
single group completes /task Step 8 inside its own `/context-reset` subagent.

The `design`, `design-review`, `self-review` and `spec-writer` subagents stay on Opus
regardless of this marker — only the implementor model/effort varies.

---

## Risks

- **`dead_code` hard-errors the whole controller module if the lib target is skipped.**
  Mitigation: subtask 2 adds `src/lib.rs` in the same commit as `controller/mod.rs`, so the
  module is never momentarily bin-only — `[measured: <scratch>/dctest bin-only →
  `cargo clippy --all-targets -- -D warnings` GATE-RED `error: function \`poll\` is never used`;
  same crate with `src/lib.rs` → GATE-GREEN]`.
- **The AC10 probe rect could diverge from the shell's real `MovePad` rect.** Mitigation:
  the probe derives its body rect from the shell's own `pub(crate) TOP_BAR_H` and from the
  `ui.max_rect()` captured out of the live harness frame, and it draws the **real**
  `RaceScreen` (no duplicated layout constants). Divergence produces a click that misses and
  a failed assertion, never a silent pass — `[derived → subtask 6's own assertion
  `assert_eq!(seen_action, Some(Action::Coast))`]`.
- **`Coast` might not be legal for `app_gallery`'s active fixture car** (`CarState { x: 10,
  y: 3, vx: 1, vy: 0 }` on `scene_track_with_metrics`'s corridor `[measured:
  `crates/render/src/app_gallery.rs:63-77` and `crates/render/src/track/test_support.rs:121-131`]`),
  in which case the pad's centre cell gets no `Sense::click` and the click is a no-op.
  Mitigation: subtask 6 opens with an explicit precondition guard
  `assert!(crate::screens::race::active_legal_mask(&track, &cars, 0).contains(Action::Coast), …)`
  so a fixture drift fails with a legible message instead of a mysterious `None`
  — `[derived → subtask 6's precondition assert]`.
- **`crates/render/src/app.rs`'s production region is already over the 500-line soft cap**
  — 524 production lines `[measured: `wc -l crates/render/src/app.rs` → 671;
  `grep -n "^#\[cfg(test)\]" crates/render/src/app.rs` → 525]`. The +~8 lines this design
  adds keeps it far below the 1000-line hard cap. A split is **out of scope**: R2-Q1 caps
  `gp-render` at one non-drawing plumbing edit. If `self-review` raises it, file a deferred
  item rather than widening this PR.
- **`cargo update` could pull unrelated transitive bumps into `Cargo.lock`.** Mitigation:
  `cargo update --workspace` restricts the refresh to workspace packages
  `[measured: `cargo update --help` → `-w, --workspace  Only update the workspace packages`]`,
  and subtask 1 gates on `git diff --stat Cargo.lock` before staging.
- **A `-D warnings` gate aborts on the first failure, masking later ones.** Mitigation:
  subtask 7 re-runs `cargo clippy --workspace --all-targets -- -D warnings` **after** the
  first clean pass; any newly-revealed out-of-contract class is surfaced to the orchestrator
  as a blocker, not absorbed — `[derived → subtask 7's second clippy run]`.
- **New doc-tests.** The new lib target makes `///` fenced Rust examples executable under
  `cargo test --workspace`. Mitigation: keep rustdoc examples on the seam either genuinely
  runnable or `text`-fenced; never write one that would need a fixture track
  — `[derived → `cargo test --workspace` in subtask 7]`.
- **Residual, accepted:** the single argument position `resp.action → show_body`'s third
  tuple slot is covered end-to-end only by subtask 6's Miri-gated interaction test; there is
  no Miri-clean assertion on it. Accepted rather than papered over with a helper fn invented
  for the test's sake, because R2-Q1 caps the `gp-render` production edit at one field.

---

## Test Design

All `gp-game` tests are in-crate `#[cfg(test)] mod tests` blocks in the lib target. They
are pure integer/enum logic — **Miri-clean and NOT gated**. No local Miri run is specced;
CI owns that signal ([`ai-docs/miri-gate.md`](../../miri-gate.md)).

Shared `gp-game` test fixture (module-private in `controller/mod.rs`'s test module, reused
by `player.rs` via `use super::super::tests::…` or a `pub(crate) #[cfg(test)]` helper):
a small `Corridor` giving (a) an all-legal mid-corridor state, (b) a wall-adjacent state
with a restricted mask, (c) a fast-approach state whose mask **excludes** `Coast`, and (d)
a state reached via `resolve_crash` whose `action_mask` is the `{Coast}` singleton.
`(c)`'s existence is guaranteed by the spec's Key-decisions row *"Is `Action::Coast` always
legal? **No.**"*, which is itself backed by `legal_move`'s supercover predicate `[measured:
`crates/core/src/sim/mod.rs:89-107`]`; each fixture state's mask is asserted at
construction so a fixture that fails to produce the intended mask fails loudly.

### Subtask 1 — dependency-edge change (procedure, not a test)

Subtask 1 changes a **dependency constraint** (it removes the `gp-game → gp-ai` edge), so
AGENTS.md § *Dependency Versions* binds it. The four steps below are a **contract**, not a
suggestion; they **MUST NOT** be collapsed into a bare `cargo build`, and the `git diff
--stat Cargo.lock` inspection **MUST** happen **before** `git add`, not after:

| # | Step | Why it cannot be skipped |
|---|---|---|
| i | **Name the changed constraint explicitly** before running anything: *"removed the edge `gp-game → gp-ai` from `crates/game/Cargo.toml` `[dependencies]`."* | AGENTS.md requires naming which constraint changed *before* running `cargo update` — it is what distinguishes a real dep-graph delta (this case, `cargo update` warranted) from a metadata-only edit (no `cargo update`, `cargo build` alone). Skipping the naming is how the two cases get conflated. |
| ii | `cargo update --workspace` | `-w, --workspace` = **"Only update the workspace packages"** `[measured: `cargo update --help` → `-w, --workspace  Only update the workspace packages`]`. A bare `cargo update` would pull unrelated transitive bumps into the lockfile. |
| iii | `cargo build` | Refreshes `Cargo.lock` and proves the crate still builds without the edge. |
| iv | **`git diff --stat Cargo.lock` — inspected BEFORE `git add`** | The gate. The only acceptable delta is `gp-game` losing its `gp-ai` edge. Any other lockfile churn is a **stop-and-surface**, not something to stage and explain in the PR body. The implementor quotes this command's literal output in its return summary (§ *Handoff plan* → *Per-subtask reporting floor*). |

Steps ii and iv are precisely the parts a "just run `cargo build`" shortcut deletes, which
is why they are enumerated here rather than left to the Decomposition row alone.

### Subtask 2 — `crates/game/src/controller/mod.rs` `#[cfg(test)] mod tests`

- **AC1** `poll_yields_only_legal_actions_for_the_state_it_was_asked_about` — a stub seat
  returning a fixed action, driven over every fixture state; for every `Some(a)` assert
  `gp_core::sim::legal_move(&track.corridor, state, a)`.
- **AC6** `empty_mask_yields_none_and_never_an_illegal_some` — `legal = BitFlags::empty()`
  with every combination of `FrameInput`; assert `None` every time, no panic. Plus a
  doc-presence guard is unnecessary — the `///` precondition text is enforced by review.
- **AC7** `controller_module_calls_no_physics` — assert the production source of
  `controller/{mod,keys,player}.rs` contains no `sim::step` / `legal_move` /
  `supercover` call, via `include_str!` on the three files and a substring scan over the
  pre-`#[cfg(test)]` region. Entry point: the `include_str!`ed text. (A structural test,
  not a behavioural one — AC7 is a *negative* about implementation, so it needs a
  source-level assertion to be checkable at all.)

  **Mandatory line filter — the scan is over CODE lines only.** The pre-`#[cfg(test)]`
  region also holds the `///` / `//!` docs that AC1 and AC6 **require**, and those docs
  will very plausibly name `legal_move` / `legal_mask` (e.g. the intra-doc link
  ``[`gp_core::sim::legal_mask`]`` in `PollContext::legal`'s precondition text). Scanning
  them unfiltered would red the test on correct code, or silently pressure the author into
  contorted doc prose to dodge a substring. The test therefore **MUST** build its haystack
  by dropping, in this order:

  1. everything from the first line whose trimmed form starts with `#[cfg(test)]` onward
     (the existing pre-`#[cfg(test)]` truncation);
  2. every line whose **trimmed** form starts with `///`, `//!`, or `//` — all doc and
     ordinary comment lines;
  3. every line whose **trimmed** form starts with `use ` — the import block, so that
     `use gp_core::sim::{Action, legal_move};`-style imports (or a plain
     `use gp_core::sim;`) are not themselves the match.

  Only the surviving lines are joined and substring-scanned. This filter is **pinned in
  the test body itself** (a small `fn code_lines(src: &str) -> String` helper in the test
  module, applied to all three files) — it is part of the contract, not an implementation
  liberty; an implementation that scans the raw `include_str!` text is non-conforming.
  Corollary the filter makes explicit: because comment and `use` lines are stripped, a
  **call** such as `sim::step(` or `legal_move(` on a bare code line is still caught,
  which is exactly the negative AC7 asserts. `[derived → subtask 2's own green
  `cargo test -p gp-game controller_module_calls_no_physics`, run against the real docs
  authored in the same subtask]`

### Subtask 3 — `crates/game/src/controller/keys.rs` `#[cfg(test)] mod tests`

- **AC4** `key_map_table` — the full nine-row table, one assertion per row:
  `action_for_key(Key::ArrowUp) == Some(Action::North)` … `action_for_key(Key::Space) == Some(Action::Coast)`.
  Plus `action_for_key(Key::Escape) == None` (an unmapped key). No `egui::Context`, no
  harness.
- `keyboard_action_masks_illegal_keys` — a legal mask excluding `North`; `pressed` returns
  true only for `Key::ArrowUp`; assert `None` (AC3's keyboard half).
- `keyboard_action_scans_in_action_declaration_order` — `pressed` true for both
  `Key::Space` and `Key::ArrowUp` with an all-legal mask; assert `Some(Action::Coast)`
  (pins the documented order).
- `keyboard_action_skips_illegal_and_takes_the_next_legal_key` — `Space` + `ArrowUp`
  pressed, mask excludes `Coast`; assert `Some(Action::North)`.

### Subtask 4 — `crates/game/src/controller/player.rs` `#[cfg(test)] mod tests`

- **AC2** `never_yields_an_action_outside_the_mask` — `rstest`-style table over the four
  fixture states × `{shell_action, key_action}` ∈ all five `Action`s ∪ `None`; for every
  `Some(a)` assert `legal.contains(a)` **and** `legal_move(&corridor, state, a)`.
  **REQUIRED case — not optional, do not drop as redundant:** the fixture state `(c)` whose
  mask **excludes** `Coast`, fed an **unmasked Coast-button** `shell_action`
  (`FrameInput { shell_action: Some(Action::Coast), key_action: None }`), **MUST** assert
  `poll` returns `None`. This case is load-bearing, not a duplicate of the generic table
  sweep: the "Coast (·)" `Button` in `race.rs` is built **unconditionally** (it never sees
  `legal`) `[measured: `crates/render/src/screens/race.rs:372-405`]` and `race.rs:252` maps
  its click to `Some(Action::Coast)` **regardless of the legal mask**, so
  `ShellResponse.action` can genuinely carry an **illegal** `Coast`. AC2 holds *only*
  because `PlayerController` masks it — this row is the assertion that pins that. An
  implementation that drops it leaves the single real illegal-input path in the product
  untested.
- **AC3** `illegal_inputs_are_no_ops_on_both_paths` — illegal `shell_action` alone ⇒ `None`;
  illegal `key_action` alone ⇒ `None`; illegal `shell_action` + legal `key_action` ⇒ the
  key's action (documented fall-through).

  **Coverage boundary for AC3's first clause — recorded so it does not read as a gap.**
  AC3's *"an illegal MovePad cell is not selectable"* half is discharged by **pre-existing,
  shipped `gp-render` behaviour**, not by any new test in this task: an illegal cell is
  `continue`d over before it ever receives `Sense::click`
  `[measured: `crates/render/src/widgets/movepad.rs:251` guard, `:256` the guarded
  `ui.interact(…, Sense::click())`]`, and that structural masking is covered by
  `movepad.rs`'s **own** `#[cfg(test)]` tests. AC11 forbids editing
  `crates/render/src/widgets/**`, so this task neither re-tests nor can re-test it. What
  **this** task pins is the other two halves: the **shell-action** half (the illegal
  `Coast` that the unconditional Coast `Button` can emit — the REQUIRED AC2 case above)
  and the **keyboard** half (`keyboard_action_masks_illegal_keys`, subtask 3). Together
  the three halves cover AC3; only the latter two are new work here.
- **AC5** `singleton_coast_mask_resolves_on_the_first_poll` — two scenarios: a hand-built
  `BitFlags::from(Action::Coast)`, and the real
  `resolve_crash(&corridor, crash_state).action_mask(&corridor)`. Both with
  `FrameInput::default()` (no click, no keypress); assert `Some(Action::Coast)` on the
  **first** poll, and assert `legal_move` holds for the outcome state.
- **AC9** `replaying_the_same_inputs_yields_the_same_actions` — a fixed `Vec<(CarState,
  BitFlags<Action>, FrameInput)>` script driven twice through two freshly-constructed
  `PlayerController`s; assert the two `Vec<Option<Action>>` results are equal, and that
  the sequence is non-trivial (contains at least one `Some` and one `None`).
- **AC8** (appended to `controller/mod.rs`'s test module)
  `heterogeneous_roster_drives_every_seat_through_one_call_site` — a `Roster` holding
  `Box::new(PlayerController::default())` and `Box::new(AlwaysCoastStub)`; a single
  `for index in 0..roster.len() { roster.poll(index, ctx) }` loop with **no** downcast, no
  `match` on seat kind, no `if index == 0`. Assert both seats answered and that each
  `Some(a)` satisfies `legal_move`.

Fixture/helper needed: `AlwaysCoastStub`, a `#[cfg(test)]` unit struct implementing
`Controller` by returning `ctx.legal.contains(Action::Coast).then_some(Action::Coast)` —
deliberately mask-respecting so AC7's "no relaxation for any seat kind" is exercised by
the stub too.

### Subtask 6 — `crates/render/src/app_gallery.rs` (AC10)

`app_gallery.rs` is a `#[cfg(test)]`-only module `[measured: `crates/render/src/lib.rs:17-18`
→ `#[cfg(test)] mod app_gallery;`]` and lives **outside** AC11's protected
`crates/render/src/{widgets,screens,tokens}/**`, so adding tests there is AC11-compatible.
Reuses the file's own owner-approved fixtures — `fixture_track` (which is
`track::test_support::scene_track_with_metrics`), `fixture_race_cars`, `fixture_standings`,
`shell_session`, `CANVAS_SIZE`, `FIXED_CONFIG` — rather than hand-rolling new ones.

- Location: `crates/render/src/app_gallery.rs` `#[cfg(test)] mod tests`.
- Entry point: `AppShell::show` (through `ShellResponse.action`).
- Both new tests carry
  `#[cfg_attr(miri, ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml lookup, unsupported under Miri isolation (no render() here, so not the golden's Vulkan-dlopen cause)")]`
  — the *own-cause* wording the two existing no-`render()` interaction tests use.
- **No golden, no PNG, no `SnapshotOptions`** — these are interaction tests, so the
  golden-threshold rule does not engage.

**Reporting requirement for subtask 6 (mandatory).** The two-`egui::Context` layout probe
is materially the hardest item in this design's `sonnet` group: it is the one place where
a wrong `body_rect`, a missed font pass, or an off-centre click yields a *plausible-looking*
failure that is easy to hand-wave. Per AGENTS.md § *Workflow* — *"a subagent's RETURN
SUMMARY is a claim, not a record"* — the implementor's return summary for subtask 6 **MUST
quote the actual test-runner output verbatim**, captured from a saved log:

```
cargo test -p gp-render shell_race_arm_forwards_movepad_action > t6.log 2>&1 \
  && echo GATE-GREEN || echo GATE-RED
grep -E "test result:|^error|panicked" t6.log
```

The summary quotes the `test result: ...` line (and the `GATE-GREEN`/`GATE-RED` marker)
literally. A bare **"PASS"**, "verified", or "test green" assertion **does not satisfy**
this subtask's reporting contract and must be treated by the orchestrator as an
unreconciled claim — re-run the command in-thread before accepting the subtask. Never pipe
the gate itself (AGENTS.md § *Build & Test*: `cargo test … | tail` reports `tail`'s exit
status). The same verbatim-quote requirement applies to
`shell_non_race_screen_yields_no_action`.

`shell_race_arm_forwards_movepad_action`:

1. Build `track` / `geometry` / `trails: [[Point; 2]; 4]` / `cars = fixture_race_cars(&trails)`
   / `standings`. Precondition guard:
   `assert!(crate::screens::race::active_legal_mask(&track, &cars, 0).contains(Action::Coast), …)`.
2. `let mut shell = AppShell::new(FIXED_CONFIG); shell.apply(Nav::Generate); shell.apply(Nav::TestLap);`
   then `assert_eq!(shell.screen(), Screen::Race)`. Both `Nav` and `AppShell::apply` are
   public `[measured: `crates/render/src/app.rs:97` `pub enum Nav`, `:199` `pub const fn apply`,
   `:207` `Nav::TestLap | Nav::Again => self.screen = Screen::Race`]`.
3. `Harness::builder().with_size(CANVAS_SIZE).build_ui(…)` following the file's existing
   fonts-install-then-draw dance; the closure captures `ui.max_rect()` and
   `shell.show(ui, session).action` into `Rc<Cell<_>>`s. `harness.run_steps(1)` (fonts),
   `harness.run_steps(1)` (first real draw). Assert the rest frame yielded `action == None`.
4. **Probe** in a fresh `egui::Context`: `set_fonts(crate::fonts::definitions())`, then one
   `run_ui` with `screen_rect: Some(full)`; inside, `scope_builder(UiBuilder::new().max_rect(body_rect))`
   where `body_rect = Rect::from_min_max(Pos2::new(full.min.x, full.min.y + TOP_BAR_H), full.max)`;
   draw `RaceScreen::new(RaceInput { scene: Scene { track, geometry, cars, reduced_motion: false,
   overlays: Overlays::default() }, active: 0, laps_done: 0, total_laps: 1 })` and record
   `movepad_response.rect`.
5. Click the pad rect's centre with the file's existing `hover_at`/`step`/`drag_at`/`step`/
   `drop_at`/`step` idiom; assert the captured action is `Some(Action::Coast)`.

`shell_non_race_screen_yields_no_action`: a fresh `AppShell::new(FIXED_CONFIG)` (on
`Screen::Setup`), same harness shape, `run_steps(2)`; assert `screen == Screen::Setup` and
`action.is_none()`. Repeat once after `shell.apply(Nav::Generate)` (`Screen::Lab`).

### Subtask 7 — gate sweep + mechanical AC verification

Capture each gate to a file and grep the saved log; never pipe a gate whose exit code is
load-bearing (AGENTS.md § *Build & Test*).

| AC | Check |
|---|---|
| AC11 | `git diff --stat <base>..HEAD` shows **no** entry under `crates/render/src/widgets/`, `crates/render/src/screens/`, `crates/render/src/tokens/`, and no `*.png`. Then `cargo test -p gp-render` green (goldens byte-identical). |
| AC12 | `git diff <base>..HEAD -- crates/game/src/main.rs` is empty; `git diff <base>..HEAD -- crates/game/tests/cli.rs` touches **only** the `//!` block — i.e. every hunk lies above the first `#[test]`, so all three `#[test]` fn bodies are byte-identical; `cargo test -p gp-game --test cli` → 3 passed. Per the **AC12 scope ruling** (§ *Approach* → Q4): AC12 scopes to the three `#[test]` fns, not the whole file, so a `//!`-only edit satisfies it and is not a spec amendment. |
| AC13 | (a) `cargo tree -p gp-game > tree.log; grep -c 'gp-ai' tree.log` → 0. (b) `grep -rn 'gp_ai\|gp-ai' crates/game/` → no match. (c) `cargo build -p gp-game` and `cargo test -p gp-game` green. (d) The subtask-1 lockfile gate already ran at its own commit — `git diff --stat Cargo.lock` inspected **before** `git add`, delta limited to `gp-game`'s lost `gp-ai` edge (§ *Test Design* → subtask 1, step iv). |
| AC14 | `cargo build`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings` (twice — see § Risks); `cargo fmt --check`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`. |

No `.github/workflows/*.yml` file is touched by this design, so the `actionlint` gate does
not engage `[measured: the Decomposition table's Files column lists no `.yml` path]`.

---

## Open questions

None. All four spec-delegated questions are settled above (Q1 roster ownership → §
*Approach* → Q1; Q2 precedence → Q2; Q3 AC10 discharge → Q3; Q4 module placement + lib
target → Q4). No product-owner input is required to begin implementation.
