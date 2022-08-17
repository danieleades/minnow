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

    let compressed = input.encode_bytes();

    // actual number of bits required is 52.09 bits. [DCCL](https://libdccl.org/3.0/) does it in 53.
    println!("bytes: {:x?}, length: {}", compressed, compressed.len());

    let output = NavigationReport::decode_bytes(&compressed);
    println!("output: {output:?}");
}
