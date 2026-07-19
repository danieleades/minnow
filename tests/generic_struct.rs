//! Generic structs (and enums): `generics.split_for_impl()` plus derive-added
//! `Encodeable`/`Bounded` bounds, exercised end-to-end.

use minnow::{Bounded, Encodeable};

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct Wrapper<T> {
    pub inner: T,
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct Pair<A, B> {
    pub first: A,
    pub second: B,
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum Either<A, B> {
    Left(A),
    Right(B),
}

#[test]
fn single_type_param_round_trips() {
    for value in [Wrapper { inner: true }, Wrapper { inner: false }] {
        let bytes = value.encode_bytes().unwrap();
        let decoded = Wrapper::<bool>::decode_bytes(&bytes).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn two_type_params_round_trip() {
    let value = Pair {
        first: true,
        second: false,
    };
    let bytes = value.encode_bytes().unwrap();
    let decoded = Pair::<bool, bool>::decode_bytes(&bytes).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn generic_enum_round_trips() {
    for value in [Either::<bool, bool>::Left(true), Either::Right(false)] {
        let bytes = value.encode_bytes().unwrap();
        let decoded = Either::<bool, bool>::decode_bytes(&bytes).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn generic_struct_weight_matches_field_weight() {
    // `Wrapper<bool>` has exactly as many values as `bool` itself.
    assert_eq!(
        <Wrapper<bool> as Bounded>::worst_case_bits(&()),
        <bool as Bounded>::worst_case_bits(&())
    );
}
