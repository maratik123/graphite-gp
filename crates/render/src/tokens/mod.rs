//! Design-token constants ported from `docs/design-system/tokens/*.css`
//! (design: `2026-07-17-render-design-tokens`).
//!
//! Skeleton for now: subtask 5 completes this module's doc with the
//! unit-semantics table and the AC1 disposition table, and adds the AC8
//! inventory test.

pub mod color;
pub mod spacing;
pub mod typography;

/// Shared CSS-parsing test infrastructure, used by every token submodule's
/// `#[cfg(test)] mod tests` (`color`, `spacing`, `typography`, `effects`).
///
/// Hoisted here so there is exactly one copy of the parser and exactly one
/// `#[allow(clippy::float_cmp)]` site (`assert_f32`) for the whole crate —
/// see `ai-docs/plans/2026-07-17-render-design-tokens.design.md` § *Remedy*.
#[cfg(test)]
pub(crate) mod css {
    /// The value text between `:` and the terminating `;`, for the declaration of
    /// `name` that starts a line (finding 6 + the prefix-collision clause).
    ///
    /// Anchored by SEARCHING the occurrences, not by taking the first: `--bg-grid`
    /// occurs in comment prose at `effects.css:27` before its real declaration at
    /// line 29, so a `split_once` anchor binds to the comment and dies. Deliberately
    /// free of index arithmetic — `clippy::arithmetic_side_effects` is a workspace
    /// `deny` and fires on the `colon + 1` spelling of this same function.
    pub(crate) fn value_of<'a>(css: &'a str, name: &str) -> &'a str {
        let rest = css
            .match_indices(name)
            .find_map(|(idx, _)| {
                let (before, at) = css.split_at(idx);
                let after = at.strip_prefix(name)?;
                // Rule 1b: the next non-space char must be `:` — `--cell` vs `--cell-sm`.
                let value = after.trim_start().strip_prefix(':')?;
                // Rule 1a: the declaration must start a line.
                before
                    .lines()
                    .next_back()
                    .is_none_or(|l| l.trim().is_empty())
                    .then_some(value)
            })
            .unwrap_or_else(|| panic!("token {name}: no declaration starts a line"));
        rest.split_once(';')
            .unwrap_or_else(|| panic!("token {name}: value has no terminating ';'"))
            .0
            .trim()
    }

    /// The ONLY float-comparison site in the crate.
    ///
    /// NOTE: must NOT be named `*_eq` / `eq_*` — `clippy::float_cmp` silently skips
    /// such fns, which would make the `#[allow]` below inert and the suppression
    /// accidental rather than declared.
    #[allow(
        clippy::float_cmp,
        reason = "CSS text and the const are two spellings of one decimal; Rust's \
                  float parsing and float literals are both correctly rounded, so \
                  they yield bit-identical f32 even for values like 1.05 that are \
                  inexact in binary. Exact equality is the intended contract - an \
                  epsilon would mask the token drift AC8 exists to catch."
    )]
    pub(crate) fn assert_f32(label: &str, got: f32, want: f32) {
        assert_eq!(got, want, "{label}: CSS value != const");
    }

    /// Element-wise `f32` comparison, naming the differing index on failure.
    pub(crate) fn assert_f32_slice(label: &str, got: &[f32], want: &[f32]) {
        assert_eq!(got.len(), want.len(), "{label}: length mismatch");
        for (index, (g, w)) in got.iter().zip(want.iter()).enumerate() {
            assert_f32(&format!("{label}[{index}]"), *g, *w);
        }
    }

    /// Parses a `px`/`em`/bare-numeric token value out of `css` and compares it
    /// against `want` via `assert_f32`.
    pub(crate) fn assert_token(css: &str, name: &str, want: f32) {
        let raw = value_of(css, name);
        let numeric = raw
            .strip_suffix("px")
            .or_else(|| raw.strip_suffix("em"))
            .unwrap_or(raw);
        let got: f32 = numeric
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("token {name}: unhandled unit in {raw:?}"));
        assert_f32(name, got, want);
    }

    /// Parses a `cubic-bezier(a, b, c, d)` token value and compares its four
    /// control points element-wise against `want`.
    pub(crate) fn assert_cubic_bezier(css: &str, name: &str, want: [f32; 4]) {
        let raw = value_of(css, name);
        let Some(inner) = raw
            .strip_prefix("cubic-bezier(")
            .and_then(|s| s.strip_suffix(')'))
        else {
            panic!("token {name}: not a cubic-bezier(): {raw:?}");
        };
        let got: Vec<f32> = inner
            .split(',')
            .map(|p| {
                p.trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("token {name}: bad control point {p:?}"))
            })
            .collect();
        assert_f32_slice(name, &got, &want);
    }

    /// Extracts the `--x` target name out of a `var(--x)` token value.
    pub(crate) fn var_target<'a>(css: &'a str, name: &str) -> &'a str {
        let raw = value_of(css, name);
        let Some(inner) = raw.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) else {
            panic!("token {name}: not a var(): {raw:?}");
        };
        inner
    }

    /// Direct coverage of the shared parser contract itself — independent of
    /// which token submodule ends up exercising each helper, so this module's
    /// helpers are never dead code between the subtask that defines them and
    /// the subtask that first consumes them.
    #[cfg(test)]
    mod tests {
        use super::{assert_cubic_bezier, assert_f32, assert_f32_slice, assert_token, var_target};

        #[test]
        fn assert_f32_accepts_a_css_parsed_equal_value() {
            let got: f32 = "1.05".parse().expect("valid float literal");
            assert_f32("probe", got, 1.05);
        }

        #[test]
        fn assert_f32_slice_accepts_equal_arrays() {
            assert_f32_slice("probe", &[0.2, 0.0, 0.1, 1.0], &[0.2, 0.0, 0.1, 1.0]);
        }

        #[test]
        fn assert_token_parses_px_em_and_bare_numbers() {
            // Indented like the real CSS files: `value_of`'s "starts a line"
            // rule checks that only whitespace precedes the match on its own
            // line, which a column-0 fixture (no indentation at all) cannot
            // exercise the same way the real, indented CSS does.
            const CSS: &str = "  --a: 24px;\n  --b: -0.02em;\n  --c: 700;\n";
            assert_token(CSS, "--a", 24.0);
            assert_token(CSS, "--b", -0.02);
            assert_token(CSS, "--c", 700.0);
        }

        #[test]
        fn assert_cubic_bezier_parses_control_points() {
            const CSS: &str = "  --ease: cubic-bezier(0.2, 0, 0.1, 1);\n";
            assert_cubic_bezier(CSS, "--ease", [0.2, 0.0, 0.1, 1.0]);
        }

        #[test]
        fn var_target_extracts_the_referenced_name() {
            const CSS: &str = "  --alias: var(--base);\n";
            assert_eq!(var_target(CSS, "--alias"), "--base");
        }
    }
}
