//! `Vec<NavigationReport>`-style nesting: a derived struct (whose own
//! `Config` is always `()`, so it needs no `elem` expression in the `seq(...)`
//! sugar) collected into a bounded `Vec`. This is the shape Phase 5's DCCL
//! comparison example builds on (a fleet of reports in one message).

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

/// A bounded fleet of navigation reports: `NavigationReport::Config == ()`,
/// which implements `Default`, so `elem` is omitted entirely — the same
/// sugar as an unannotated leaf field.
#[derive(Debug, Encodeable, PartialEq, Clone)]
pub struct Fleet {
    #[encode(seq(max_len = 10))]
    pub reports: Vec<NavigationReport>,
}

fn sample_report(seed: u32) -> NavigationReport {
    #[allow(clippy::cast_precision_loss)]
    let seed = f64::from(seed);
    NavigationReport {
        x: (seed * 137.0) % 10_000.0 - 5_000.0,
        y: (seed * 91.0) % 10_000.0 - 5_000.0,
        z: -((seed * 53.0) % 5_000.0),
        vehicle_class: match seed as u32 % 4 {
            0 => None,
            1 => Some(VehicleClass::Auv),
            2 => Some(VehicleClass::Usv),
            _ => Some(VehicleClass::Ship),
        },
        battery_ok: Some((seed as u32).is_multiple_of(2)),
    }
}

#[test]
fn round_trips_at_length_extremes() {
    for len in [0, 1, 10] {
        let fleet = Fleet {
            reports: (0..len).map(sample_report).collect(),
        };
        let bytes = fleet.encode_bytes().unwrap();
        let decoded = Fleet::decode_bytes(&bytes).unwrap();
        assert_eq!(decoded, fleet);
    }
}

#[test]
fn size_matches_length_prefix_plus_reports() {
    // Each report's worst-case size: `NavigationReport`'s own report total
    // (17.6096*2 + 12.2882 + 2.0 + 1.585, see tests/size_law.rs), plus the
    // uniform length prefix over 0..=10 (log2(11) bits), plus coder
    // termination — matching `SeqModel`'s documented
    // `worst_case_bits = log2(L+1) + L * worst_case_bits(elem)` formula.
    let report_bits = <NavigationReport as Encodeable>::worst_case_bits();
    let length_bits = 11_f64.log2();
    let expected_worst = length_bits + 10.0 * report_bits;

    let worst = <Fleet as Encodeable>::worst_case_bits();
    assert!(
        (worst - expected_worst).abs() < 1e-6,
        "expected {expected_worst}, got {worst}"
    );

    let upper = Fleet::size_report().total_bytes();
    let full_fleet = Fleet {
        reports: (0..10).map(sample_report).collect(),
    };
    let bytes = full_fleet.encode_bytes().unwrap();
    assert!(bytes.len() <= upper);
}

#[test]
fn beats_dccl_per_report_bit_budget() {
    // DCCL3's default codec costs 18+18+13+2+2 = 53 bits per report
    // (see the crate-level docs / Phase 5's DCCL comparison). Minnow's
    // per-field float quantisation plus weighted `Option` discriminants
    // already beats that per-report; this is the nested-`Vec` analogue of
    // that comparison, checked directly against `NavigationReport`'s own
    // measured worst-case bits.
    let report_bits = <NavigationReport as Encodeable>::worst_case_bits();
    assert!(
        report_bits < 53.0,
        "expected < 53 bits/report, got {report_bits}"
    );
}
