//! Round-trip and size-law tests for bounded integers (`IntModel`, issue #3's
//! numeric sibling), both hand-configured and via the `#[encode(int(...))]`
//! derive sugar.

use minnow::{Encodeable, EncodeableCustom, IntModel};

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct Reading {
    // A small signed range, well within a single byte.
    #[encode(int(min = -10, max = 10))]
    pub delta: i32,
    // `u8`'s `Default` config spans its full native range.
    pub raw: u8,
    // `u64` has no `Default`, so it always needs an explicit range.
    #[encode(int(min = 0, max = 1_000_000))]
    pub counter: u64,
}

// --- Round-trips at the extremes --------------------------------------------

#[test]
fn round_trips_at_extremes() {
    for delta in [-10, -1, 0, 1, 10] {
        for raw in [u8::MIN, 128, u8::MAX] {
            for counter in [0, 500_000, 1_000_000] {
                let value = Reading {
                    delta,
                    raw,
                    counter,
                };
                let bytes = value.encode_bytes().unwrap();
                let decoded = Reading::decode_bytes(&bytes).unwrap();
                assert_eq!(decoded, value);
            }
        }
    }
}

#[test]
fn negative_only_range_round_trips() {
    let config = IntModel::new(-1000_i32..=-1).unwrap();
    for value in [-1000, -500, -1] {
        let bytes = value.encode_bytes_with_config(config).unwrap();
        let decoded = i32::decode_bytes_with_config(&bytes, config).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn single_value_range_costs_zero_bits() {
    // A range with min == max has exactly one encodable value: weight 1,
    // zero payload bits (matching a unit struct).
    let config = IntModel::new(42_i32..=42).unwrap();
    assert_eq!(<i32 as EncodeableCustom>::weight(&config).get(), 1);
    assert_eq!(<i32 as EncodeableCustom>::worst_case_bits(&config), 0.0);
    let bytes = 42_i32.encode_bytes_with_config(config).unwrap();
    assert_eq!(i32::decode_bytes_with_config(&bytes, config).unwrap(), 42);
}

#[test]
fn i64_and_u64_extremes_round_trip() {
    let signed = IntModel::new(i64::MIN..=i64::MIN + 1_000_000).unwrap();
    for value in [i64::MIN, i64::MIN + 500_000, i64::MIN + 1_000_000] {
        let bytes = value.encode_bytes_with_config(signed).unwrap();
        assert_eq!(
            i64::decode_bytes_with_config(&bytes, signed).unwrap(),
            value
        );
    }

    let unsigned = IntModel::new(u64::MAX - 1_000_000..=u64::MAX).unwrap();
    for value in [u64::MAX - 1_000_000, u64::MAX - 500_000, u64::MAX] {
        let bytes = value.encode_bytes_with_config(unsigned).unwrap();
        assert_eq!(
            u64::decode_bytes_with_config(&bytes, unsigned).unwrap(),
            value
        );
    }
}

// --- Size law ----------------------------------------------------------------

#[test]
fn size_law_holds() {
    let upper = Reading::size_report().total_bytes();
    let mut lengths = Vec::new();
    for delta in [-10, 0, 10] {
        for raw in [0, 255] {
            for counter in [0, 1_000_000] {
                let value = Reading {
                    delta,
                    raw,
                    counter,
                };
                let bytes = value.encode_bytes().unwrap();
                assert!(bytes.len() <= upper);
                lengths.push(bytes.len());
            }
        }
    }
    // Uniform weighting: every value should encode to (near enough) the same
    // length.
    let min = *lengths.iter().min().unwrap();
    let max = *lengths.iter().max().unwrap();
    assert!(max - min <= 1, "min={min} max={max}");
}

#[test]
fn worst_case_bits_matches_formula() {
    // delta: 21 values -> log2(21); raw: 256 values -> log2(256) = 8;
    // counter: 1_000_001 values -> log2(1_000_001).
    let expected = 21_f64.log2() + 256_f64.log2() + 1_000_001_f64.log2();
    let bits = <Reading as Encodeable>::worst_case_bits();
    assert!(
        (bits - expected).abs() < 1e-9,
        "expected {expected}, got {bits}"
    );
}

/// Encode-domain semantics through the derive: out-of-range values error by
/// default and clamp only under the explicit `clamping` flag.
#[test]
fn out_of_range_errors_by_default_and_clamps_on_opt_in() {
    #[derive(Debug, Encodeable, PartialEq, Eq)]
    struct Strict {
        #[encode(int(min = 0, max = 10))]
        value: u8,
    }

    #[derive(Debug, Encodeable, PartialEq, Eq)]
    struct Clamped {
        #[encode(int(min = 0, max = 10, clamping))]
        value: u8,
    }

    assert!(matches!(
        Strict { value: 200 }.encode_bytes(),
        Err(minnow::EncodeError::OutOfRange { .. })
    ));

    let encoded = Clamped { value: 200 }.encode_bytes().unwrap();
    assert_eq!(
        Clamped::decode_bytes(&encoded).unwrap(),
        Clamped { value: 10 }
    );

    // In-range values are untouched in either mode.
    let encoded = Strict { value: 7 }.encode_bytes().unwrap();
    assert_eq!(Strict::decode_bytes(&encoded).unwrap(), Strict { value: 7 });
}
