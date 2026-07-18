//! Issue #9's optimisation, in miniature: a weighted enum discriminant makes
//! every value of `Option<VehicleClass>` cost the *same*, minimal number of
//! bits, rather than paying a fixed 1-bit tax for the `Option` regardless of
//! how few values the payload has.
//!
//! `VehicleClass` has three variants (weight 3); wrapped in `Option` that's
//! `{None: 1, Some: 3}`, total weight `W = 1 + 3 = 4`.
//!
//! * **Naive / uniform discriminant** (a plain 50/50 `None`-vs-`Some` bit, then
//!   the payload if present): `1 + log2(3) ≈ 2.58` bits worst case — the
//!   discriminant bit is "wasted" because `None` and `Some` aren't equally
//!   likely outcomes of a 4-value type.
//! * **Weighted discriminant** (interval widths proportional to `{1, 3}`, what
//!   Minnow's derive does automatically — see the crate-level "How Minnow
//!   compresses" docs): exactly `log2(4) = 2.0` bits, for *every* value, no
//!   annotation required. This is the information-theoretic minimum for a
//!   4-valued type.

use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

fn main() {
    // `Option<VehicleClass>` is exactly the shape issue #9 asked for: no
    // `#[encode(weight = ...)]` needed, the derive weights `None`/`Some`
    // automatically by payload cardinality.
    println!("size report:\n{}", <Option<VehicleClass>>::size_report());

    let bits = <Option<VehicleClass>>::worst_case_bits();
    let naive_bits = 1.0 + 3_f64.log2();
    println!(
        "\nweighted worst case: {bits:.2} bits (naive uniform discriminant would cost \
         {naive_bits:.2} bits)"
    );
    assert!(
        (bits - 2.0).abs() < 1e-9,
        "Option<VehicleClass> should cost exactly 2.0 bits, got {bits}"
    );

    for input in [
        None,
        Some(VehicleClass::Auv),
        Some(VehicleClass::Usv),
        Some(VehicleClass::Ship),
    ] {
        let compressed = input.encode_bytes().unwrap();
        let output =
            <Option<VehicleClass>>::decode_bytes(&compressed).expect("round-trip should succeed");
        assert_eq!(input, output);
        println!("input: {input:?}, bytes: {}", compressed.len());
    }
}
