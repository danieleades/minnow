//! The size law: under default (automatic) weighting every value of a schema
//! encodes to (essentially) the same length, and that length is bounded by
//! `size_report().total_bytes()`.

use std::fmt::Debug;

use minnow::Encodeable;

// --- Fixtures ---------------------------------------------------------------

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum UnitEnum {
    A,
    B,
    C,
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum FloatEnum {
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    A(f64),
    #[encode(float(min = 0.0, max = 5_000.0, precision = 0))]
    B(f64),
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct TupleStruct(
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
);

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct ArrayStruct {
    #[encode(float(min = 0.0, max = 5_000.0, precision = 0))]
    three_vec: [f64; 3],
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct NavigationReport {
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    pub x: f64,
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    pub y: f64,
    #[encode(float(min = -5_000.0, max = 0.0, precision = 0))]
    pub z: f64,
    pub vehicle_class: Option<VehicleClass>,
    pub battery_ok: Option<bool>,
}

// --- Tiny deterministic PRNG ------------------------------------------------

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

    fn vehicle(&mut self) -> Option<VehicleClass> {
        match self.next_u64() % 4 {
            0 => None,
            1 => Some(VehicleClass::Auv),
            2 => Some(VehicleClass::Usv),
            _ => Some(VehicleClass::Ship),
        }
    }
}

// --- The size law ------------------------------------------------------------

/// Assert, for a set of sample values of `T`:
/// * every encoding fits within `size_report().total_bytes()`;
/// * every encoding decodes through `decode_bytes` (which validates length) to
///   a stable fixed point (re-encoding reproduces the bytes — the right
///   round-trip property for a lossy float codec);
/// * all encodings are within one byte of each other (uniform weighting).
fn assert_size_law<T>(values: &[T])
where
    T: Encodeable + Debug,
{
    let upper = T::size_report().total_bytes();
    let mut min = usize::MAX;
    let mut max = 0usize;

    for value in values {
        let bytes = value.encode_bytes().unwrap();
        let len = bytes.len();
        assert!(
            len <= upper,
            "{value:?} encoded to {len} bytes, exceeding size_report().total_bytes() = {upper}",
        );
        let decoded = T::decode_bytes(&bytes).expect("a valid encoding must decode");
        assert_eq!(
            decoded.encode_bytes().unwrap(),
            bytes,
            "re-encoding the decoded value must be stable",
        );
        min = min.min(len);
        max = max.max(len);
    }

    assert!(
        max - min <= 1,
        "under default weighting all values should be within one byte: min={min} max={max}",
    );
}

#[test]
fn unit_enum_size_law() {
    assert_size_law(&[UnitEnum::A, UnitEnum::B, UnitEnum::C]);
}

#[test]
fn float_enum_size_law() {
    let mut rng = Rng::new(1);
    let values: Vec<FloatEnum> = (0..5_000)
        .map(|i| {
            if i % 2 == 0 {
                FloatEnum::A(rng.range(-10_000.0, 10_000.0))
            } else {
                FloatEnum::B(rng.range(0.0, 5_000.0))
            }
        })
        .collect();
    assert_size_law(&values);
}

#[test]
fn tuple_struct_size_law() {
    let mut rng = Rng::new(2);
    let values: Vec<TupleStruct> = (0..5_000)
        .map(|_| {
            TupleStruct(
                rng.range(-10_000.0, 10_000.0),
                rng.range(-10_000.0, 10_000.0),
            )
        })
        .collect();
    assert_size_law(&values);
}

#[test]
fn array_struct_size_law() {
    let mut rng = Rng::new(3);
    let values: Vec<ArrayStruct> = (0..5_000)
        .map(|_| ArrayStruct {
            three_vec: [
                rng.range(0.0, 5_000.0),
                rng.range(0.0, 5_000.0),
                rng.range(0.0, 5_000.0),
            ],
        })
        .collect();
    assert_size_law(&values);
}

#[test]
fn navigation_report_size_law() {
    let mut rng = Rng::new(4);
    let values: Vec<NavigationReport> = (0..20_000)
        .map(|_| NavigationReport {
            x: rng.range(-10_000.0, 10_000.0),
            y: rng.range(-10_000.0, 10_000.0),
            z: rng.range(-5_000.0, 0.0),
            vehicle_class: rng.vehicle(),
            battery_ok: match rng.next_u64() % 3 {
                0 => None,
                1 => Some(true),
                _ => Some(false),
            },
        })
        .collect();
    assert_size_law(&values);
}

// --- Option<VehicleClass>: exactly 2.0 bits ---------------------------------

#[test]
fn option_vehicle_class_is_exactly_two_bits() {
    // weights {None: 1, Some: 3}, W = 4, log2(4) = 2.0 exactly.
    let bits = <Option<VehicleClass>>::worst_case_bits();
    assert!((bits - 2.0).abs() < 1e-9, "expected 2.0 bits, got {bits}");
    assert_eq!(<Option<VehicleClass>>::size_report().total_bytes(), 1);

    let values = [
        None,
        Some(VehicleClass::Auv),
        Some(VehicleClass::Usv),
        Some(VehicleClass::Ship),
    ];
    let mut lengths = Vec::new();
    for value in values {
        let bytes = value.encode_bytes().unwrap();
        assert!(bytes.len() <= 1, "should fit in a single byte");
        assert_eq!(Option::<VehicleClass>::decode_bytes(&bytes).unwrap(), value);
        lengths.push(bytes.len());
    }
    // Every value encodes to an identical length.
    assert!(lengths.iter().all(|&l| l == lengths[0]));
}

// --- NavigationReport worst case --------------------------------------------

#[test]
fn navigation_report_worst_case_bits() {
    // 17.6096 (x) + 17.6096 (y) + 12.2882 (z) + 2.0 (vehicle) + 1.585 (battery)
    let expected = 17.6096 + 17.6096 + 12.2882 + 2.0 + 1.585;
    let bits = NavigationReport::worst_case_bits();
    assert!(
        (bits - expected).abs() < 0.01,
        "expected ~{expected} bits, got {bits}",
    );
    assert!(NavigationReport::size_report().total_bytes() <= 7);
}

// --- SizeReport Display snapshot --------------------------------------------

#[test]
fn navigation_report_display_snapshot() {
    let rendered = NavigationReport::size_report().to_string();
    let expected = "\
total: 51.09 bits (7 bytes)
  x: 17.61 bits
  y: 17.61 bits
  z: 12.29 bits
  vehicle_class: 2.00 bits
  battery_ok: 1.58 bits
";
    assert_eq!(rendered, expected);
}
