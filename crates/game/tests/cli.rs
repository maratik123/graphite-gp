//! Process-level exit-contract tests for the `graphite-gp` binary (issue
//! #41, AC15/AC16). Spawns the built binary via
//! `env!("CARGO_BIN_EXE_graphite-gp")` — the only reach a `gp-game` test
//! needs beyond the in-crate `cargo test` coverage in `config.rs`, since a
//! `gp-game` lib target buys nothing else (design § *Resolved spec
//! hand-offs* #1). Both tests terminate before `eframe::run_native` is
//! called, so neither opens a window.

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
fn help_exits_zero() {
    // AC16: --help exits 0 without opening a window.
    let output = bin()
        .arg("--help")
        .output()
        .expect("failed to spawn the built graphite-gp binary");
    assert_eq!(output.status.code(), Some(0));
}
