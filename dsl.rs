// draft of potential DSL

#[derive(Debug, Encodeable)]
pub struct NavigationReport {
    #[encode(float(min = -10_000, max = 10_000, precision = 1))]
    pub x: f64,
    #[encode(float(min = -10_000, max = 10_000, precision = 1))]
    pub y: f64,
    #[encode(model = FloatModel::new(-5000.0..=0.0, 0))]
    pub z: f64,
    pub vehicle_class: Option<VehicleClass>,
    pub battery_ok: Option<bool>,
}