//! # gp-game — Block 3b: game loop & orchestration (design doc §3b, §6)
//!
//! The runnable binary. Wires generation (block 1) → the physics core (block 3a)
//! → rendering (block 2), with player and AI controllers driving the same engine.
//! Kept separate from block 3a so training (thousands of headless envs) and live
//! play share one, non-diverging physics implementation.

fn main() {
    println!("graphite-gp — grid racing game (scaffold)");
    println!("architecture: [1 gen] · [2 render] · [3a core] · [3b game] · [4 ai]");
    println!("see docs/design.md for the full spec; build order 3a -> (1 || 2) -> 4");
    // TODO(3b): input, timing, orchestration, UX.
}
