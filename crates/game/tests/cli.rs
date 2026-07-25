//! Process-level exit-contract tests for the `graphite-gp` binary (issue
//! #41, AC15/AC16). Spawns the built binary via
//! `env!("CARGO_BIN_EXE_graphite-gp")` — the only reach a `gp-game` test
//! needs beyond the in-crate `cargo test` coverage in `config/`, since a
//! `gp-game` lib target buys nothing else (design § *Resolved spec
//! hand-offs* #1). All three tests terminate before `eframe::run_native` is
//! called, so none opens a window.

use std::process::Command;

/// A fresh [`Command`] for the built `graphite-gp` binary.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graphite-gp"))
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn invalid_cars_exits_nonzero_with_range_in_stderr() {
    // AC15: an out-of-range --cars is rejected before the window opens.
    let output = bin()
        .args(["--cars", "9"])
        .output()
        .expect("failed to spawn the built graphite-gp binary");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--cars"), "{stderr}");
    assert!(stderr.contains('9'), "{stderr}");
    assert!(stderr.contains("2..=6"), "{stderr}");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn cross_field_block_size_below_floor_exits_nonzero_with_values_in_stderr() {
    // Fix 3 (self-review round 1, minor): `ConfigError::exit`'s cross-field
    // arm (`config/error.rs`'s `other @ Self::BlockSizeBelowWidthFloor {
    // .. }`, rendered via `Cli::command().error(ValueValidation, ..)`) had
    // no coverage at any level — `invalid_cars_exits_nonzero_with_range_in_
    // stderr` above only ever drives the `Self::Cli(err) => err.exit()`
    // arm. `--cars 6 --block-size 2` is below the `ceil(cars/2) = 3`
    // corridor-width floor, so it takes the cross-field arm instead.
    let output = bin()
        .args(["--cars", "6", "--block-size", "2"])
        .output()
        .expect("failed to spawn the built graphite-gp binary");
    // `Some(2)`, not `assert_ne!(.., Some(0))` — measured, and it gives
    // sibling parity with the clap-sourced case above.
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--block-size 2"), "{stderr}");
    assert!(stderr.contains("= 3"), "{stderr}");
    assert!(stderr.contains("--cars 6"), "{stderr}");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn help_exits_zero() {
    // AC16: --help exits 0 without opening a window.
    let output = bin()
        .arg("--help")
        .output()
        .expect("failed to spawn the built graphite-gp binary");
    assert_eq!(output.status.code(), Some(0));
}
