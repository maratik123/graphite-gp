# Panic index

Every intentional panicking call (`panic!`, `unwrap`, `expect`, panicking index/slice, release-overflowing arithmetic) in **production** code (outside `#[cfg(test)]`), each with a one-line justification that it is genuinely unreachable/unrecoverable. Kept in sync by `self-review` (`.claude/agents/self-review.md`) and the `panic-gate` hook. `gp-core` targets zero production panics.

| File:line | Call | Why it cannot fire (or is unrecoverable) |
|-----------|------|------------------------------------------|
| `crates/render/src/screens/setup.rs:229` | `u32::try_from(cars).expect("cars is clamped to [2,6] — always fits u32")` | `cars` is `.clamp(MIN_CARS, MAX_CARS)` = `[2, 6]` immediately above, so the `i32 → u32` conversion always succeeds. |
| `crates/render/src/screens/setup.rs:230` | `u32::try_from(laps).expect("laps is clamped to [1,9] — always fits u32")` | `laps` is `.clamp(MIN_LAPS, MAX_LAPS)` = `[1, 9]` immediately above, so the `i32 → u32` conversion always succeeds. |
| `crates/render/src/screens/race.rs:417` | `movepad_response.expect("Card::show always invokes add_contents")` | `Card::show` (`crates/render/src/widgets/card.rs:220-247`) unconditionally calls `add_contents` exactly once before returning, and `draw_your_move`'s `add_contents` closure unconditionally assigns `movepad_response = Some(..)` as its first statement — so the `Option` is always `Some` by the time `show` returns. |
| `crates/render/src/screens/race.rs:418` | `coast_response.expect("Card::show always invokes add_contents")` | Same invariant as the row above — `Card::show` always invokes `add_contents`, whose closure unconditionally assigns `coast_response = Some(..)` before returning. |
| `crates/game/src/main.rs:111` | `i32::try_from(STARTUP_CONFIG.laps).expect("STARTUP_CONFIG.laps is a small const literal — always fits i32")` | `STARTUP_CONFIG` is a module-level `const RaceConfig` with `laps: 5` (`crates/game/src/main.rs:35-37`); the value is a fixed small literal, so the conversion to `i32` always succeeds. Binary-only fixture wiring. |
