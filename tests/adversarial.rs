//! Adversarial decode tests.
//!
//! Decoding runs on untrusted input, so it must never panic — every byte slice
//! must decode to `Ok` or `Err`, never a panic. These tests feed many random
//! and truncated byte strings through `decode_bytes` for a few representative
//! types and assert only that the process does not panic.

use minnow::{Encodeable, EncodeableCustom};

#[derive(Debug, Encodeable, PartialEq)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

#[derive(Debug, Encodeable, PartialEq)]
pub struct Report {
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    pub x: f64,
    pub vehicle_class: Option<VehicleClass>,
    pub battery_ok: Option<bool>,
}

/// A fixture exercising the variable-length models: a bounded `Vec` (issue
/// #5) and a bounded `String` (issue #3), both of which decode a length
/// prefix from untrusted input before looping — exactly the code path that
/// must reject a corrupt/oversized decoded length rather than trying to
/// allocate or read past `max_len`/`max_length`.
#[derive(Debug, Encodeable, PartialEq)]
pub struct WithSequences {
    #[encode(seq(max_len = 6))]
    pub flags: Vec<bool>,
    #[encode(string(max_length = 12))]
    pub label: String,
}

/// A tiny deterministic PRNG (`xorshift64*`) so the test is reproducible
/// without pulling in an external dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid the all-zero state, which xorshift cannot escape.
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn fill(&mut self, buf: &mut [u8]) {
        for byte in buf.iter_mut() {
            *byte = (self.next_u64() & 0xff) as u8;
        }
    }
}

/// Decode `bytes` as `T` and assert the call returns (never panics).
fn assert_no_panic<T: Encodeable>(bytes: &[u8]) {
    // The result is deliberately ignored: either outcome (`Ok`/`Err`) is
    // acceptable, we only require that decoding terminates without panicking.
    let _ = T::decode_bytes(bytes);
}

#[test]
fn random_bytes_never_panic() {
    let mut rng = Rng::new(0xdead_beef);

    for _ in 0..10_000 {
        let len = (rng.next_u64() % 24) as usize;
        let mut buf = vec![0u8; len];
        rng.fill(&mut buf);

        assert_no_panic::<bool>(&buf);
        assert_no_panic::<Option<bool>>(&buf);
        assert_no_panic::<VehicleClass>(&buf);
        assert_no_panic::<Report>(&buf);
        assert_no_panic::<WithSequences>(&buf);

        // `Vec`/`String` have no `Default` config (there is no sensible
        // universal `max_len`/`max_length`), so they aren't `Encodeable` and
        // must be exercised through `decode_bytes_with_config` directly.
        let _ = <Vec<bool> as minnow::EncodeableCustom>::decode_bytes_with_config(
            &buf,
            minnow::SeqModel {
                max_len: 6,
                elem: (),
            },
        );
        let _ = <String as minnow::EncodeableCustom>::decode_bytes_with_config(
            &buf,
            minnow::StringModel::new(12).unwrap(),
        );
    }
}

#[test]
fn truncated_valid_encodings_never_panic() {
    // Start from a valid encoding, then feed every truncation of it.
    let report = Report {
        x: 1234.5,
        vehicle_class: Some(VehicleClass::Ship),
        battery_ok: Some(false),
    };
    let encoded = report.encode_bytes().unwrap();

    for len in 0..=encoded.len() {
        assert_no_panic::<Report>(&encoded[..len]);
    }

    // Also exercise the simple leaf/​sum types.
    for value in [true, false] {
        let bytes = value.encode_bytes().unwrap();
        for len in 0..=bytes.len() {
            assert_no_panic::<bool>(&bytes[..len]);
        }
    }

    for value in [Some(true), Some(false), None] {
        let bytes = value.encode_bytes().unwrap();
        for len in 0..=bytes.len() {
            assert_no_panic::<Option<bool>>(&bytes[..len]);
        }
    }

    // And a fixture with variable-length (`Vec`/`String`) fields: truncating
    // mid-way through the length prefix or the element/byte payload must
    // still never panic.
    let with_sequences = WithSequences {
        flags: vec![true, false, true, true],
        label: "hello".to_string(),
    };
    let bytes = with_sequences.encode_bytes().unwrap();
    for len in 0..=bytes.len() {
        assert_no_panic::<WithSequences>(&bytes[..len]);
    }
}

#[test]
fn corrupt_and_truncated_sequences_never_panic_or_oom() {
    // Every byte slice — random garbage or a truncation of a real encoding —
    // fed through a `Vec`/`String` decode must terminate in `Ok`/`Err`, never
    // panic and never attempt to allocate/read beyond `max_len`/`max_length`.
    let seq_config = minnow::SeqModel {
        max_len: 6_u32,
        elem: (),
    };
    let string_config = minnow::StringModel::new(12).unwrap();

    let valid_vec: Vec<bool> = vec![true, false, true];
    let valid_vec_bytes = valid_vec.encode_bytes_with_config(seq_config).unwrap();
    let valid_string = "hello!".to_string();
    let valid_string_bytes = valid_string
        .encode_bytes_with_config(string_config)
        .unwrap();

    let mut rng = Rng::new(0xc0ff_ee00);
    for _ in 0..5_000 {
        let len = (rng.next_u64() % 16) as usize;
        let mut buf = vec![0u8; len];
        rng.fill(&mut buf);

        if let Ok(decoded) =
            <Vec<bool> as minnow::EncodeableCustom>::decode_bytes_with_config(&buf, seq_config)
        {
            assert!(decoded.len() <= seq_config.max_len as usize);
        }
        if let Ok(decoded) =
            <String as minnow::EncodeableCustom>::decode_bytes_with_config(&buf, string_config)
        {
            assert!(decoded.len() <= string_config.max_length());
        }
    }

    for len in 0..=valid_vec_bytes.len() {
        let _ = <Vec<bool> as minnow::EncodeableCustom>::decode_bytes_with_config(
            &valid_vec_bytes[..len],
            seq_config,
        );
    }
    for len in 0..=valid_string_bytes.len() {
        let _ = <String as minnow::EncodeableCustom>::decode_bytes_with_config(
            &valid_string_bytes[..len],
            string_config,
        );
    }
}

#[test]
fn truncations_are_rejected() {
    // Under uniform weighting `Report` has a single exact encoded length, so the
    // up-front length check rejects every truncation with `DecodeError::Length`
    // rather than silently decoding a zero-padded, wrong value.
    let report = Report {
        x: 1234.5,
        vehicle_class: Some(VehicleClass::Ship),
        battery_ok: Some(false),
    };
    let encoded = report.encode_bytes().unwrap();

    assert!(
        Report::decode_bytes(&encoded).is_ok(),
        "full length must decode"
    );

    for len in 0..encoded.len() {
        let err = Report::decode_bytes(&encoded[..len]).unwrap_err();
        assert!(
            matches!(err, minnow::DecodeError::Length { .. }),
            "truncation to {len} bytes should be a Length error, got {err:?}",
        );
    }

    // Trailing padding is equally impossible for the schema.
    let mut padded = encoded.clone();
    padded.push(0);
    assert!(matches!(
        Report::decode_bytes(&padded).unwrap_err(),
        minnow::DecodeError::Length { .. }
    ));
}

#[test]
fn valid_round_trip_still_works() {
    // Sanity check that the fixture type round-trips, so the adversarial tests
    // are exercising a real codec.
    let report = Report {
        x: -42.5,
        vehicle_class: Some(VehicleClass::Auv),
        battery_ok: None,
    };
    let encoded = report.encode_bytes().unwrap();
    let decoded = Report::decode_bytes(&encoded).unwrap();
    assert_eq!(report, decoded);
}
