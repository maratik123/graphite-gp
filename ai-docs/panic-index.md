# Panic index

Every intentional panicking call (`panic!`, `unwrap`, `expect`, panicking index/slice, release-overflowing arithmetic) in **production** code (outside `#[cfg(test)]`), each with a one-line justification that it is genuinely unreachable/unrecoverable. Kept in sync by `self-review` (`.claude/agents/self-review.md`) and the `panic-gate` hook.

`gp-core` targets zero production panics, and **currently holds it** — the table below has no `crates/core/` row. PR [#171](https://github.com/maratik123/graphite-gp/pull/171) briefly broke it with two `i32::try_from(..).expect(..)` bounds in `supercover`'s interval solver; reverting that rewrite (it measured slower at every production velocity — see `crates/core/benches/supercover.rs`) removed them. Treat any new `sim`/`geom` panic-class call as a red flag and prefer a total form (`checked_*` / `try_from(..).unwrap_or(sentinel)`).

| File:line | Call | Why it cannot fire (or is unrecoverable) |
|---|---|---|
| `crates/render/src/screens/setup.rs:229` | `u32::try_from(cars).expect("cars is clamped to [2,6] — always fits u32")` | `cars` is `.clamp(MIN_CARS, MAX_CARS)` = `[2, 6]` immediately above, so the `i32 → u32` conversion always succeeds. |
| `crates/render/src/screens/setup.rs:230` | `u32::try_from(laps).expect("laps is clamped to [1,9] — always fits u32")` | `laps` is `.clamp(MIN_LAPS, MAX_LAPS)` = `[1, 9]` immediately above, so the `i32 → u32` conversion always succeeds. |
| `crates/render/src/screens/race.rs:403` | `movepad_response.expect("Card::show always invokes add_contents")` | `Card::show` (`crates/render/src/widgets/card.rs:220`, `ui.vertical(add_contents)` at `:243`) unconditionally calls `add_contents` exactly once before returning, and `draw_your_move`'s `add_contents` closure unconditionally assigns `movepad_response = Some(..)` as its first statement — so the `Option` is always `Some` by the time `show` returns. |
| `crates/render/src/screens/race.rs:404` | `coast_response.expect("Card::show always invokes add_contents")` | Same invariant as the row above — `Card::show` always invokes `add_contents`, whose closure unconditionally assigns `coast_response = Some(..)` before returning. |
| `crates/gen/src/phase7.rs:337` | `order.last().expect("order is never empty")` | `order` is initialised `vec![start]` (one element, `phase7.rs:332`) and the enclosing `walk_cycle` loop only ever `order.push(next)`s (`:357`) — it never pops — so `order.last()` is always `Some` on every iteration. Render-only centerline walk; unreachable by construction. |
| `crates/gen/src/phase1.rs:258` | `RaceDir::VARIANTS.choose(rng).copied().expect("enum variants should not be empty")` | `RaceDir` should not be empty |
