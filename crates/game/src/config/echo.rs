//! The `gp-game` startup-echo formatter (issue #41, AC18).
//!
//! Split out of `config/mod.rs` (AGENTS.md's 800-line file-size soft cap) —
//! a pure, self-contained formatting concern with no other dependents.

use super::GameConfig;

/// Decimal places the startup echo renders the pilot temperature at.
const TEMPERATURE_DECIMALS: usize = 2;

/// Renders the resolved configuration for the startup echo (AC18) — a pure
/// formatter, no I/O, so it is testable without a process or a window.
pub fn render_startup_echo(config: &GameConfig) -> String {
    let player_line = format!(
        "graphite-gp: cars {cars}, laps {laps}, V_target {v_target}, difficulty {difficulty} (temperature {temp:.prec$})",
        cars = config.race.cars,
        laps = config.race.laps,
        v_target = config.race.v_target,
        difficulty = config.race.difficulty.label(),
        temp = config.temperature(),
        prec = TEMPERATURE_DECIMALS,
    );
    // v3 (subtask 5): the full `GenParams` `Debug` line — carries all seven
    // fields, including the four nested `Seeds` values. `Debug` cannot
    // silently omit a field, and auto-follows the deferred
    // `v_ceiling` -> `v_target` rename instead of drifting.
    format!("{player_line}\ngraphite-gp: {:?}", config.to_gen_params())
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    // ---- AC18: the startup echo contains every resolved value ----

    #[test]
    fn ac18_echo_contains_every_resolved_value() {
        let config = parse(&[
            "--cars",
            "6",
            "--seed",
            "12345",
            "--seed-ai-learning",
            "999",
        ]);
        let params = config.to_gen_params();
        let rendered = render_startup_echo(&config);

        assert!(
            rendered.contains(&format!(
                "temperature {:.*}",
                TEMPERATURE_DECIMALS,
                config.temperature()
            )),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("cars: {}", params.cars)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("min_straight: {}", params.min_straight)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("v_ceiling: {}", params.v_ceiling)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("block_size: {}", params.block_size)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("seed_budget: {}", params.seed_budget)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("repair_budget: {}", params.repair_budget)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("collision: {}", params.seeds.collision)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("generation: {}", params.seeds.generation)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("ai_learning: {}", params.seeds.ai_learning)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("ai_inference: {}", params.seeds.ai_inference)),
            "{rendered}"
        );

        // Negative control (design § AC18): the player line alone does not
        // satisfy the `GenParams`-half needles.
        let player_line = rendered.lines().next().expect("player line present");
        assert!(!player_line.contains("min_straight: "), "{player_line}");
        assert!(!player_line.contains("collision: "), "{player_line}");
    }
}
