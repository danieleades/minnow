//! The unbounded tier (issue #72): types that implement [`Encodeable`] but
//! not [`Bounded`] — no weight, no size report, no length-validated decode —
//! and how they compose with derived schemas via `#[encode(unbounded)]`.

use minnow::{DecodeError, DecodeVisitor, EncodeVisitor, Encodeable, OneShot, SeqModel};

// --- A hand-written unbounded leaf -------------------------------------------

/// An open-ended varint: a `u64` encoded as base-16 digits, least significant
/// first, each followed by a continuation flag. Small values cost a few bits;
/// there is no finite worst case over the type's *model* (the scheme extends
/// to arbitrary-precision integers unchanged), so `Varint` implements
/// [`Encodeable`] only — asking it for a weight or a size report does not
/// compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Varint(pub u64);

impl Encodeable for Varint {
    type Config = ();

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        _config: (),
    ) -> Result<(), minnow::EncodeError>
    where
        W: bitstream_io::BitWrite,
    {
        let mut remaining = self.0;
        loop {
            #[allow(clippy::cast_possible_truncation)]
            let digit = (remaining & 0xf) as u32;
            remaining >>= 4;
            visitor.encode_one(OneShot::<16>, &digit)?;
            visitor.encode_one(OneShot::<2>, &u32::from(remaining != 0))?;
            if remaining == 0 {
                return Ok(());
            }
        }
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        _config: (),
    ) -> Result<Self, DecodeError>
    where
        R: bitstream_io::BitRead,
        Self: Sized,
    {
        let mut value: u64 = 0;
        for shift in (0..).step_by(4) {
            // A corrupt stream may keep asserting "more digits" forever; a
            // `u64` has at most 16, so a 17th is invalid, not an infinite
            // loop.
            if shift >= 64 {
                return Err(DecodeError::InvalidSymbol { symbol: 1 });
            }
            let digit = u64::from(visitor.decode_one(OneShot::<16>)?);
            value |= digit << shift;
            if visitor.decode_one(OneShot::<2>)? == 0 {
                break;
            }
        }
        Ok(Self(value))
    }
}

const SAMPLES: [u64; 8] = [0, 1, 15, 16, 255, 4_096, 1_234_567_890, u64::MAX];

#[test]
fn varint_round_trips_through_unvalidated_decode() {
    for value in SAMPLES {
        let input = Varint(value);
        let bytes = input.encode_bytes().unwrap();
        let output = Varint::decode_bytes_unvalidated(&bytes).unwrap();
        assert_eq!(input, output, "value {value}");
    }
}

#[test]
fn varint_small_values_stay_small() {
    // The point of an unbounded model: cost tracks the value, with no
    // schema-wide worst case to pay for. A digit + flag is ~5 bits.
    assert_eq!(Varint(0).encode_bytes().unwrap().len(), 1);
    assert!(Varint(u64::MAX).encode_bytes().unwrap().len() >= 10);
}

// --- A derived unbounded struct ----------------------------------------------

#[derive(Debug, Encodeable, PartialEq)]
#[encode(unbounded)]
pub struct Telemetry {
    pub healthy: bool,
    pub uptime_seconds: Varint,
}

#[test]
fn unbounded_struct_round_trips() {
    for value in SAMPLES {
        let input = Telemetry {
            healthy: value % 2 == 0,
            uptime_seconds: Varint(value),
        };
        let bytes = input.encode_bytes().unwrap();
        let output = Telemetry::decode_bytes_unvalidated(&bytes).unwrap();
        assert_eq!(input, output);
    }
}

// --- A derived unbounded enum ------------------------------------------------

// An `#[encode(unbounded)]` enum must weight every variant explicitly:
// automatic weighting would need the payloads' cardinalities, which an
// unbounded payload does not have.
#[derive(Debug, Encodeable, PartialEq)]
#[encode(unbounded)]
pub enum Message {
    #[encode(weight = 3)]
    Heartbeat,
    #[encode(weight = 1)]
    Count(Varint),
}

#[test]
fn unbounded_enum_round_trips() {
    let values = [
        Message::Heartbeat,
        Message::Count(Varint(0)),
        Message::Count(Varint(u64::MAX)),
    ];
    for input in values {
        let bytes = input.encode_bytes().unwrap();
        let output = Message::decode_bytes_unvalidated(&bytes).unwrap();
        assert_eq!(input, output);
    }
}

// --- Containers of unbounded elements ----------------------------------------

/// A bounded-length `Vec` of an unbounded element still encodes — its length
/// prefix is uniform, so the codec never needs the element's cardinality. It
/// just has no size budget (`Vec<Varint>` is not `Bounded`).
#[test]
fn bounded_vec_of_unbounded_elements_round_trips() {
    let config = SeqModel {
        max_len: 8,
        elem: (),
    };
    let input: Vec<Varint> = SAMPLES.into_iter().map(Varint).collect();
    let bytes = input.encode_bytes_with_config(config).unwrap();
    let output = <Vec<Varint>>::decode_bytes_unvalidated_with_config(&bytes, config).unwrap();
    assert_eq!(input, output);
}
