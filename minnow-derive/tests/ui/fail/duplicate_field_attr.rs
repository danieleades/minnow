// A field with two `#[encode(...)]` attributes is ambiguous: which model
// applies? Must be a clean compile error, not a silent "last one wins" or a
// panic.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Bad {
    #[encode(float(min = 0.0, max = 1.0, precision = 0))]
    #[encode(float(min = 0.0, max = 2.0, precision = 0))]
    x: f64,
}

fn main() {}
