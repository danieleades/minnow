use minnow::Encodeable;

#[derive(Debug, Encodeable)]
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

#[derive(Debug, Encodeable)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

fn main() {
    let input = NavigationReport {
        x: 450.0,
        y: 550.0,
        z: -100.0,
        vehicle_class: Some(VehicleClass::Auv),
        battery_ok: Some(true),
    };

    println!("input: {input:?}");

    // The worst-case size report (issue #6): a per-field breakdown of the
    // fractional bit cost, plus the byte figure including coder termination.
    // Weighting the optional discriminants brings this to 51.09 bits (7 bytes),
    // one bit better than uniform discriminants would give, and beating DCCL's
    // 53 bits. See <https://libdccl.org/3.0/>.
    println!("\nsize report:\n{}", NavigationReport::size_report());

    let compressed = input.encode_bytes().unwrap();

    println!("bytes: {:x?}, length: {}", compressed, compressed.len());

    let output = NavigationReport::decode_bytes(&compressed).expect("round-trip should succeed");
    println!("output: {output:?}");
}
