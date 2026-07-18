//! Round-trip and size-law tests for `Vec<T>` (issue #5), both hand-configured
//! (`SeqModel`) and via the `#[encode(seq(...))]` derive sugar.

use minnow::{Encodeable, EncodeableCustom, FloatModel, SeqModel};

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum Flag {
    Red,
    Green,
    Blue,
}

#[derive(Debug, Encodeable, PartialEq, Clone)]
pub struct Samples {
    #[encode(seq(max_len = 5, elem = minnow::FloatModel::new(0.0..=10.0, 1).unwrap()))]
    pub readings: Vec<f64>,
    #[encode(seq(max_len = 5))]
    pub flags: Vec<Flag>,
}

fn as_f64(i: usize) -> f64 {
    f64::from(u32::try_from(i).unwrap())
}

// --- Round-trips at lengths {0, 1, max} -------------------------------------

#[test]
fn vec_f64_round_trips_at_length_extremes() {
    let elem = FloatModel::new(0.0..=10.0, 1).unwrap();
    for len in [0, 1, 5] {
        let config = SeqModel {
            max_len: 5,
            elem: elem.clone(),
        };
        let value: Vec<f64> = (0..len).map(as_f64).collect();
        let bytes = value.encode_bytes_with_config(config.clone());
        let decoded = <Vec<f64>>::decode_bytes_with_config(&bytes, config).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn vec_enum_round_trips_at_length_extremes() {
    let config = SeqModel {
        max_len: 4,
        elem: (),
    };
    let all = [Flag::Red, Flag::Green, Flag::Blue, Flag::Red];
    for len in [0, 1, 4] {
        let value: Vec<Flag> = all[..len].to_vec();
        let bytes = value.encode_bytes_with_config(config);
        let decoded = <Vec<Flag>>::decode_bytes_with_config(&bytes, config).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn derive_seq_sugar_round_trips() {
    for len in [0, 1, 5] {
        let value = Samples {
            readings: (0..len).map(as_f64).collect(),
            flags: vec![Flag::Blue; len],
        };
        let bytes = value.encode_bytes();
        let decoded = Samples::decode_bytes(&bytes).unwrap();
        assert_eq!(decoded, value);
    }
}

// --- Size law: the tight per-length formula ---------------------------------
//
// For a variable-length sequence the size law is per-*value*, not a single
// figure: a sequence of length `l` costs `log2(max_len + 1) + l * elem_bits`
// payload bits, plus `TERMINATION_BITS` (2.0) of coder overhead, rounded up
// to whole bytes.

#[test]
fn per_length_size_law_matches_formula() {
    // For a *uniform* leaf, every value costs the same ideal (`f64`) number
    // of bits, but the real arithmetic coder's actual output is an integer
    // number of bits whose relationship to that ideal figure includes
    // renormalisation/flush rounding (`TERMINATION_BITS` is an *upper* bound
    // on termination cost, "at most two bits" — see `crate::TERMINATION_BITS`
    // docs, not always exactly two). So the achieved byte length is pinned to
    // within one byte of `ceil((length_bits + l * elem_bits +
    // TERMINATION_BITS) / 8)` — the same one-byte tolerance
    // `tests/size_law.rs` uses for fixed-size schemas — rather than always
    // equal to it; and it must never exceed that figure, which is a genuine
    // upper bound (`SizeReport::total_bytes`'s contract).
    let elem = FloatModel::new(0.0..=10.0, 1).unwrap();
    let elem_bits = <f64 as EncodeableCustom>::worst_case_bits(&elem);
    let max_len = 5_u32;

    for len in 0..=max_len {
        let config = SeqModel {
            max_len,
            elem: elem.clone(),
        };
        let value: Vec<f64> = (0..len).map(|i| as_f64(i as usize)).collect();
        let bytes = value.encode_bytes_with_config(config);

        let expected_bits = (f64::from(max_len) + 1.0).log2()
            + f64::from(len) * elem_bits
            + minnow::TERMINATION_BITS;
        let expected_bytes = (expected_bits / 8.0).ceil() as usize;

        assert!(
            bytes.len() <= expected_bytes && bytes.len() + 1 >= expected_bytes,
            "length {len}: expected {expected_bytes} bytes (±1), got {}",
            bytes.len()
        );
    }
}

#[test]
fn best_and_worst_case_bits_match_formula() {
    let elem = FloatModel::new(0.0..=10.0, 1).unwrap();
    let elem_bits = <f64 as EncodeableCustom>::worst_case_bits(&elem);
    let max_len = 5_u32;
    let config = SeqModel { max_len, elem };

    let length_bits = (f64::from(max_len) + 1.0).log2();
    assert!((<Vec<f64> as EncodeableCustom>::best_case_bits(&config) - length_bits).abs() < 1e-9);
    let expected_worst = length_bits + f64::from(max_len) * elem_bits;
    assert!(
        (<Vec<f64> as EncodeableCustom>::worst_case_bits(&config) - expected_worst).abs() < 1e-9
    );
}

// --- Weight formula ----------------------------------------------------------

#[test]
fn weight_matches_geometric_sum_formula() {
    // elem weight 3 (Flag has 3 variants), max_len 3:
    // 1 + 3 + 9 + 27 = 40.
    let config = SeqModel {
        max_len: 3,
        elem: (),
    };
    assert_eq!(
        <Vec<Flag> as EncodeableCustom>::weight(&config).get(),
        1 + 3 + 9 + 27
    );
}
