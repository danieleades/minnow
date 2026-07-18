//! Pins down `examples/dccl_comparison.rs`'s headline numbers as a regression
//! test: Minnow's actual encoded bytes for a bounded `Vec<NavigationReport>`
//! must never exceed DCCL3's documented worst-case formula for the same
//! schema, at every table row the example prints.
//!
//! See `examples/dccl_comparison.rs` for the derivation of the DCCL formula
//! (per-field `ceil(log2(N))` whole-bit packing, `18+18+13+2+2 = 53`
//! bits/report, a `ceil(log2(max_repeat+1))`-bit count prefix, a 1-byte
//! message ID, byte-padded once) and the caveat that the DCCL side is
//! formula-based, not measured against a real `libdccl` build.

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

#[derive(Debug, Encodeable, PartialEq, Clone)]
pub struct Fleet {
    #[encode(seq(max_len = 16))]
    pub reports: Vec<NavigationReport>,
}

const MAX_REPEAT: u32 = 16;
const DCCL_BITS_PER_REPORT: u32 = 18 + 18 + 13 + 2 + 2;

fn dccl_bytes(n: u32) -> u32 {
    let message_id_bits = 8;
    let count_bits = (MAX_REPEAT + 1).ilog2() + 1;
    let total_bits = message_id_bits + count_bits + n * DCCL_BITS_PER_REPORT;
    total_bits.div_ceil(8)
}

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

#[test]
fn minnow_beats_dccl_worst_case_at_every_table_row() {
    // Matches the table `examples/dccl_comparison.rs` prints.
    let expected_dccl = [(1, 9), (5, 35), (10, 68), (16, 108)];

    for (n, expected_dccl_bytes) in expected_dccl {
        let dccl = dccl_bytes(n);
        assert_eq!(dccl, expected_dccl_bytes, "DCCL formula drifted for n={n}");

        let fleet = Fleet {
            reports: (0..n).map(sample_report).collect(),
        };
        let minnow_bytes = fleet.encode_bytes().unwrap().len();

        assert!(
            (minnow_bytes as u32) <= dccl,
            "minnow ({minnow_bytes} bytes) should never exceed DCCL's worst case ({dccl} bytes) \
             at n={n}"
        );

        let decoded = Fleet::decode_bytes(&fleet.encode_bytes().unwrap()).unwrap();
        assert_eq!(decoded, fleet);
    }
}

#[test]
fn ten_reports_matches_plan_headline_figures() {
    // The specific numbers called out in the design docs: DCCL 68 bytes vs
    // minnow 65 for a 10-report fleet.
    let fleet = Fleet {
        reports: (0..10).map(sample_report).collect(),
    };
    assert_eq!(fleet.encode_bytes().unwrap().len(), 65);
    assert_eq!(dccl_bytes(10), 68);
}
