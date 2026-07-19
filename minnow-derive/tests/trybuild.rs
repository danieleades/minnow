//! UI tests for the derive macro's diagnostics.
//!
//! Each fixture under `tests/ui/fail/` must fail to compile with a clean
//! diagnostic — never a macro panic (`todo!()`, `unwrap()`, `assert!()`
//! reachable from user input). The checked-in `.stderr` snapshots were
//! generated on this workspace's pinned stable toolchain; `trybuild`
//! compares against them exactly, so a toolchain upgrade that changes
//! diagnostic wording (rustc's own, or `darling`'s) will need
//! `TRYBUILD=overwrite cargo test -p minnow-derive --test trybuild` to
//! regenerate them — re-diff the output when you do, to confirm the
//! diagnostic is still a clean one (no "panicked at") rather than rubber-
//! stamping a regression. Run this test only on the stable CI job for that
//! reason; MSRV/nightly jobs should skip it.
//!
//! Fixtures under `tests/ui/pass/` must compile, exercising the newer
//! attribute forms (`config = <expr>`, struct/multi-field-tuple variants,
//! generics) as an extra compile-time smoke test alongside the `tests/*.rs`
//! integration tests.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // The checked-in `.stderr` snapshots pin rustc's exact diagnostic
    // rendering (span underlines, box-drawing characters), generated on this
    // workspace's current stable toolchain. CI also runs the pinned MSRV
    // toolchain (`test (1.85)`) and a nightly (the coverage job), whose
    // diagnostic wording differs — skip the stderr-sensitive checks on any
    // non-current-stable toolchain rather than fail for reasons unrelated to
    // the macro's correctness. If `rustc --version` can't be determined,
    // fail open and run the checks anyway (the behaviour every non-CI/local
    // run gets).
    if is_snapshot_toolchain() {
        t.compile_fail("tests/ui/fail/*.rs");
    }

    // The pass fixtures carry no stderr snapshot, so they must keep
    // compiling on every toolchain in the matrix.
    t.pass("tests/ui/pass/*.rs");
}

/// Best-effort detection of a toolchain whose diagnostics match the
/// checked-in `.stderr` snapshots: a *stable* rustc that is not this
/// workspace's pinned MSRV (see the `rust-version` key in `Cargo.toml`).
/// Nightly/beta compilers and the MSRV word their diagnostics differently.
/// Returns `true` (fail open, run the checks) if the version can't be
/// determined — the behaviour every local run gets.
fn is_snapshot_toolchain() -> bool {
    const MSRV: &str = "1.85";

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let Ok(output) = std::process::Command::new(rustc).arg("--version").output() else {
        return true;
    };
    let version = String::from_utf8_lossy(&output.stdout).into_owned();
    !version.contains("nightly") && !version.contains("beta") && !version.contains(MSRV)
}
