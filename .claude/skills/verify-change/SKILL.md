---
name: verify-change
description: "Run cargo test. Pass an optional filter to run a subset of tests."
argument-hint: "[test-filter]"
disable-model-invocation: true
allowed-tools: Bash(cargo test *)
---

> Near-stateless: no `.progress.md` discipline applies; re-entry consists of re-invoking the skill.

Run `cargo test $ARGUMENTS`. If no arguments, runs the full test suite. Report any failures.
