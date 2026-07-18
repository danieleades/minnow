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
    // workspace's stable-channel toolchain. This workspace's CI `test` job
    // also runs the pinned MSRV toolchain, whose diagnostic formatting can
    // differ — skip the stderr-sensitive checks there rather than fail for
    // reasons unrelated to the macro's correctness. If `rustc --version`
    // can't be determined, fail open and run the checks anyway (the
    // behaviour every non-CI/local run gets).
    if !is_msrv_toolchain() {
        t.compile_fail("tests/ui/fail/*.rs");
    }

    // The pass fixtures carry no stderr snapshot, so they must keep
    // compiling on every toolchain in the matrix.
    t.pass("tests/ui/pass/*.rs");
}

/// Best-effort detection of this workspace's pinned MSRV toolchain (see the
/// `rust-version` key in `Cargo.toml`), so the stderr-snapshotted half of
/// [`ui`] can be skipped there. Returns `false` (fail open) if the version
/// can't be determined.
fn is_msrv_toolchain() -> bool {
    const MSRV: &str = "1.85";

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let Ok(output) = std::process::Command::new(rustc).arg("--version").output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout).contains(MSRV)
}
