//! Shared test-only float-comparison helpers, used by every `#[cfg(test)]`
//! module in the crate — both `tokens::css`'s CSS-token-parity tests and the
//! `track`/`widgets` modules' hand-computed geometry/style expectations.
//!
//! Hoisted here so there is exactly one copy and exactly one
//! `#[allow(clippy::float_cmp)]` site (`assert_f32`) for the whole crate —
//! see design `2026-07-17-render-design-tokens` § *Remedy*.

/// The ONLY float-comparison site in the crate.
///
/// NOTE: must NOT be named `*_eq` / `eq_*` — `clippy::float_cmp` silently skips
/// such fns, which would make the `#[allow]` below inert and the suppression
/// accidental rather than declared.
#[allow(
    clippy::float_cmp,
    reason = "Every call site compares two values that are expected to be \
              bit-identical by construction: either a CSS-parsed value \
              against the const it was ported from (Rust's float parsing \
              and float literals are both correctly rounded, so they yield \
              bit-identical f32 even for values like 1.05 that are inexact \
              in binary), or a deterministic arithmetic result against a \
              hand-computed literal for the same fixed test inputs. Exact \
              equality is the intended contract in both cases - an epsilon \
              would mask real drift (CSS token drift AC8 exists to catch,  \
              or a regression in the arithmetic under test)."
)]
pub(crate) fn assert_f32(label: &str, got: f32, want: f32) {
    assert_eq!(got, want, "{label}: value != expected");
}

/// Element-wise `f32` comparison, naming the differing index on failure.
pub(crate) fn assert_f32_slice(label: &str, got: &[f32], want: &[f32]) {
    assert_eq!(got.len(), want.len(), "{label}: length mismatch");
    for (index, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_f32(&format!("{label}[{index}]"), *g, *w);
    }
}

#[cfg(test)]
mod tests {
    use super::{assert_f32, assert_f32_slice};

    #[test]
    fn assert_f32_accepts_an_equal_value() {
        let got: f32 = "1.05".parse().expect("valid float literal");
        assert_f32("probe", got, 1.05);
    }

    #[test]
    fn assert_f32_slice_accepts_equal_arrays() {
        assert_f32_slice("probe", &[0.2, 0.0, 0.1, 1.0], &[0.2, 0.0, 0.1, 1.0]);
    }
}
