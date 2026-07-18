// `min` above `max` is nonsensical and must be rejected at macro-expansion
// time, not produce a model that panics at runtime.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Bad {
    #[encode(float(min = 10.0, max = -10.0, precision = 0))]
    x: f64,
}

fn main() {}
