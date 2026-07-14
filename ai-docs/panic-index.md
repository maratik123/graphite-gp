# Panic index

Every intentional panicking call (`panic!`, `unwrap`, `expect`, panicking index/slice, release-overflowing arithmetic) in **production** code (outside `#[cfg(test)]`), each with a one-line justification that it is genuinely unreachable/unrecoverable. Kept in sync by `self-review` (`.claude/agents/self-review.md`) and the `panic-gate` hook. `gp-core` targets zero production panics.

| File:line | Call | Why it cannot fire (or is unrecoverable) |
|-----------|------|------------------------------------------|
| `crates/core/src/geom.rs:81` (`Corridor::new`) | `assert!(width >= 0 && height >= 0)` | Caller-contract precondition — corridor dimensions must be non-negative. Firing means a caller passed a negative `width`/`height`, a programming error rather than recoverable runtime state. Not `Result`: a fallible constructor would push validation onto every call site for an invariant all callers already uphold. Documented via `# Panics`; pre-existing assert surfaced + documented by the `clippy::missing_panics_doc` deny. Preferred future fix: a validated non-negative dimension newtype. |
