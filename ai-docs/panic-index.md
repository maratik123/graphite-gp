# Panic index

Every intentional panicking call (`panic!`, `unwrap`, `expect`, panicking index/slice, release-overflowing arithmetic) in **production** code (outside `#[cfg(test)]`), each with a one-line justification that it is genuinely unreachable/unrecoverable. Kept in sync by `self-review` (`.claude/agents/self-review.md`) and the `panic-gate` hook. `gp-core` targets zero production panics.

| File:line | Call | Why it cannot fire (or is unrecoverable) |
|-----------|------|------------------------------------------|
| `crates/render/src/screens/setup.rs:187` | `.expect("the button row unconditionally runs inside show")` | The `Generate track` button is drawn unconditionally inside the card closure earlier in the same `show`, so `generate_response` is always `Some` by this line. |
| `crates/render/src/screens/setup.rs:225` | `u32::try_from(cars).expect("cars is clamped to [2,6] — always fits u32")` | `cars` is `.clamp(MIN_CARS, MAX_CARS)` = `[2, 6]` immediately above, so the `i32 → u32` conversion always succeeds. |
| `crates/render/src/screens/setup.rs:226` | `u32::try_from(laps).expect("laps is clamped to [1,9] — always fits u32")` | `laps` is `.clamp(MIN_LAPS, MAX_LAPS)` = `[1, 9]` immediately above, so the `i32 → u32` conversion always succeeds. |
