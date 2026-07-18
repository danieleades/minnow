//! Adversarial decode tests.
//!
//! Decoding runs on untrusted input, so it must never panic — every byte slice
//! must decode to `Ok` or `Err`, never a panic. These tests feed many random
//! and truncated byte strings through `decode_bytes` for a few representative
//! types and assert only that the process does not panic.

use minnow::Encodeable;

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
    let encoded = report.encode_bytes();

    for len in 0..=encoded.len() {
        assert_no_panic::<Report>(&encoded[..len]);
    }

    // Also exercise the simple leaf/​sum types.
    for value in [true, false] {
        let bytes = value.encode_bytes();
        for len in 0..=bytes.len() {
            assert_no_panic::<bool>(&bytes[..len]);
        }
    }

    for value in [Some(true), Some(false), None] {
        let bytes = value.encode_bytes();
        for len in 0..=bytes.len() {
            assert_no_panic::<Option<bool>>(&bytes[..len]);
        }
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
    let encoded = report.encode_bytes();

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
    let encoded = report.encode_bytes();
    let decoded = Report::decode_bytes(&encoded).unwrap();
    assert_eq!(report, decoded);
}
