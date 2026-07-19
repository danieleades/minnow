// A float model whose denominator exceeds the arithmetic coder's precision
// bound must be rejected at macro-expansion time (mirroring
// `FloatModel::new`), not silently corrupt round-trips at runtime.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Bad {
    #[encode(float(min = 0.0, max = 9223372036854775808.0, precision = 0))]
    x: f64,
}

fn main() {}
