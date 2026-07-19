//! Manual `#[encode(weight = N)]` discriminant overrides bias the code toward
//! the heavier variants (fewer bits) at the expense of the lighter ones.

use minnow::{Encodeable, EncodeableCustom};

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum Biased {
    #[encode(weight = 1000)]
    Common,
    Rare,
}

#[test]
fn override_makes_the_heavy_variant_cheaper() {
    // The override only changes the *coding* weights, not the cardinality: the
    // type still has exactly two distinct values.
    assert_eq!(<Biased as EncodeableCustom>::weight(&()).get(), 2);

    // ...but the worst case is now far above `log2(2) = 1` bit, because `Rare`
    // pays for `Common`'s cheapness.
    let worst = <Biased as EncodeableCustom>::worst_case_bits(&());
    let best = <Biased as EncodeableCustom>::best_case_bits(&());
    assert!(
        worst > 9.0,
        "Rare should cost ~log2(1001) bits, got {worst}"
    );
    assert!(best < 0.01, "Common should be nearly free, got {best}");
}

#[test]
fn repeated_heavy_variant_encodes_smaller() {
    // Amplify the per-symbol difference by repeating each variant many times.
    let commons = [Biased::Common; 64];
    let rares = [Biased::Rare; 64];

    let common_len = commons.encode_bytes().unwrap().len();
    let rare_len = rares.encode_bytes().unwrap().len();

    assert!(
        common_len < rare_len,
        "biasing toward Common should shrink an all-Common array ({common_len} vs {rare_len})",
    );

    // Both still round-trip, even though their lengths differ wildly: the decode
    // length check must not reject the (very short) all-Common encoding.
    let commons_bytes = commons.encode_bytes().unwrap();
    let rares_bytes = rares.encode_bytes().unwrap();
    assert_eq!(
        <[Biased; 64]>::decode_bytes(&commons_bytes).unwrap(),
        commons
    );
    assert_eq!(<[Biased; 64]>::decode_bytes(&rares_bytes).unwrap(), rares);
}
