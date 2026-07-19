//! Struct-style, multi-field-tuple, and mixed unit/tuple/struct enum
//! variants (issue #4). Verifies round-tripping *and* the size law: every
//! value of the schema encodes to a length bounded by
//! `size_report().total_bytes()`.

use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum Shape {
    /// Unit variant.
    Point,
    /// Multi-field tuple variant — attributes live on the fields now.
    Circle(
        #[encode(float(min = 0.0, max = 1_000.0, precision = 1))] f64,
        #[encode(float(min = -1_000.0, max = 1_000.0, precision = 0))] f64,
    ),
    /// Struct variant.
    Rectangle {
        #[encode(float(min = 0.0, max = 1_000.0, precision = 1))]
        width: f64,
        height: bool,
    },
}

// --- Tiny deterministic PRNG, matching the convention in tests/size_law.rs -

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
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

    #[allow(clippy::cast_precision_loss)]
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        let t = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        lo + t * (hi - lo)
    }

    fn shape(&mut self) -> Shape {
        match self.next_u64() % 3 {
            0 => Shape::Point,
            1 => Shape::Circle(self.range(0.0, 1_000.0), self.range(-1_000.0, 1_000.0)),
            _ => Shape::Rectangle {
                width: self.range(0.0, 1_000.0),
                height: self.next_u64().is_multiple_of(2),
            },
        }
    }
}

#[test]
fn round_trip_every_variant_kind() {
    let values = [
        Shape::Point,
        Shape::Circle(12.5, -300.0),
        Shape::Rectangle {
            width: 42.0,
            height: true,
        },
    ];
    for value in values {
        let bytes = value.encode_bytes().unwrap();
        let decoded = Shape::decode_bytes(&bytes).expect("valid encoding must decode");
        assert_eq!(decoded, value);
    }
}

#[test]
fn size_law_holds_across_variant_kinds() {
    let mut rng = Rng::new(42);
    let upper = Shape::size_report().total_bytes();

    for _ in 0..5_000 {
        let value = rng.shape();
        let bytes = value.encode_bytes().unwrap();
        assert!(
            bytes.len() <= upper,
            "{value:?} encoded to {} bytes, exceeding size_report().total_bytes() = {upper}",
            bytes.len(),
        );
        let decoded = Shape::decode_bytes(&bytes).expect("a valid encoding must decode");
        assert_eq!(
            decoded.encode_bytes().unwrap(),
            bytes,
            "re-encoding must be stable"
        );
    }
}

#[test]
fn rectangle_field_breakdown_is_named() {
    let report = Shape::size_report();
    let rendered = report.to_string();
    // The `Rectangle` variant's payload children must carry the real field
    // names, not positional indices, proving struct-variant fields flow
    // through to the report.
    assert!(rendered.contains("width"), "{rendered}");
    assert!(rendered.contains("height"), "{rendered}");
}
