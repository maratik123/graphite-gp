//! Throwaway probe for #76: red under Miri only, to empirically confirm that a
//! failing `Miri` required status check blocks a PR merge. Lives on a disposable
//! branch and is never merged — it is deleted with the branch.

#[test]
fn miri_only_deliberate_failure() {
    // Green under build/test/clippy/docs (`cfg(miri)` is false there); under
    // `cargo miri test --workspace` the cfg'd panic fires and reddens the
    // required `Miri` gate.
    #[cfg(miri)]
    panic!("deliberate red Miri — #76 branch-protection gate probe (throwaway)");
}
