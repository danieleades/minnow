//! Compares Minnow's actual encoded size for a bounded `Vec<NavigationReport>`
//! against DCCL3's documented worst-case size for the same schema.
//!
//! The `NavigationReport`/`VehicleClass` shape mirrors `tests/nested_vec.rs`
//! (a fleet of reports bounded to `max_len = 16`).
//!
//! # The DCCL side: formula, not measurement
//!
//! The DCCL figures below are **computed from DCCL3's documented default
//! codec rules**, not measured against a real `libdccl` build linked into
//! this repository — there is no DCCL installation here to compare against.
//! Per the default-codec docs (<https://libdccl.org/3.0/>), each field is
//! packed into a *whole* number of bits, `ceil(log2(N))` for a field with `N`
//! distinguishable values:
//!
//! * `x`, `y`: `ceil(log2(200_001)) = 18` bits each (range ±10,000 at 0.1
//!   precision),
//! * `z`: `ceil(log2(5_001)) = 13` bits (range \[-5,000, 0\] at 1.0 precision),
//! * `vehicle_class` (`Option<VehicleClass>`, 4 values including `None`):
//!   `ceil(log2(4)) = 2` bits,
//! * `battery_ok` (`Option<bool>`, 3 values including `None`): `ceil(log2(3)) =
//!   2` bits,
//!
//! for a fixed **53 bits/report** (`18+18+13+2+2`). A repeated field also
//! costs a `ceil(log2(max_repeat + 1))`-bit count prefix, and DCCL messages
//! carry a 1-byte message ID. The whole message is then byte-padded once
//! (not once per field, not once per repeated element) — this is the
//! "byte-packed" scheme that makes DCCL simple to implement but leaves
//! sub-bit redundancy on the table, which is exactly what Minnow's
//! fractional arithmetic coding recovers.
//!
//! # The Minnow side: measured, not estimated
//!
//! Minnow's byte counts below are the **actual `encode_bytes().len()`** of a
//! real `Fleet` value for each `n` — not a formula. (Under Minnow's uniform
//! weighting every value of a fixed schema encodes to the same length; see
//! `tests/size_law.rs`.)

use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
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

/// A bounded fleet of navigation reports, `max_len = 16`.
#[derive(Debug, Encodeable, PartialEq, Clone)]
pub struct Fleet {
    #[encode(seq(max_len = 16))]
    pub reports: Vec<NavigationReport>,
}

const MAX_REPEAT: u32 = 16;

/// DCCL3's documented worst-case bits per report: `18 + 18 + 13 + 2 + 2`
/// (whole-bit `ceil(log2(N))` packing per field — see the module docs).
const DCCL_BITS_PER_REPORT: u32 = 18 + 18 + 13 + 2 + 2;

/// DCCL3's documented worst-case size, in bytes, for a message containing
/// `n` repeated `NavigationReport`s: a 1-byte message ID, plus a
/// `ceil(log2(max_repeat + 1))`-bit repetition count, plus `n` reports at
/// [`DCCL_BITS_PER_REPORT`] bits each — all packed into whole bits and
/// byte-padded exactly once at the end.
fn dccl_bytes(n: u32) -> u32 {
    let message_id_bits = 8;
    let count_bits = (MAX_REPEAT + 1).ilog2() + 1; // ceil(log2(max_repeat + 1))
    let total_bits = message_id_bits + count_bits + n * DCCL_BITS_PER_REPORT;
    total_bits.div_ceil(8)
}

/// A small deterministic generator for sample reports, so the table doesn't
/// depend on Minnow's compression happening to favour one particular value.
fn sample_report(seed: u32) -> NavigationReport {
    #[allow(clippy::cast_precision_loss)]
    let seed_f = f64::from(seed);
    NavigationReport {
        x: (seed_f * 137.0) % 10_000.0 - 5_000.0,
        y: (seed_f * 91.0) % 10_000.0 - 5_000.0,
        z: -((seed_f * 53.0) % 5_000.0),
        vehicle_class: match seed % 4 {
            0 => None,
            1 => Some(VehicleClass::Auv),
            2 => Some(VehicleClass::Usv),
            _ => Some(VehicleClass::Ship),
        },
        battery_ok: Some(seed.is_multiple_of(2)),
    }
}

fn main() {
    println!(
        "{:>3}  {:>11}  {:>13}",
        "n", "dccl (bytes)", "minnow (bytes)"
    );
    for n in [1_u32, 5, 10, 16] {
        let fleet = Fleet {
            reports: (0..n).map(sample_report).collect(),
        };
        let minnow_bytes = fleet.encode_bytes().unwrap().len();
        let dccl = dccl_bytes(n);

        println!("{n:>3}  {dccl:>11}  {minnow_bytes:>13}");

        assert!(
            (minnow_bytes as u32) <= dccl,
            "minnow ({minnow_bytes} bytes) should never exceed DCCL's worst case ({dccl} bytes) \
             at n={n}"
        );
    }
}
