# Panic index

Every intentional panicking call (`panic!`, `unwrap`, `expect`, panicking index/slice, release-overflowing arithmetic) in **production** code (outside `#[cfg(test)]`), each with a one-line justification that it is genuinely unreachable/unrecoverable. Kept in sync by `self-review` (`.claude/agents/self-review.md`) and the `panic-gate` hook. `gp-core` targets zero production panics.

| File:line | Call | Why it cannot fire (or is unrecoverable) |
|-----------|------|------------------------------------------|
| _(none yet)_ | | |
