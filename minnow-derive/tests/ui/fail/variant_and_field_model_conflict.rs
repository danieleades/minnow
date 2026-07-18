// Combining the legacy variant-level model with a field-level attribute on
// the same (sole) field is ambiguous and must be rejected, not silently pick
// one.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub enum Bad {
    #[encode(float(min = 0.0, max = 1.0, precision = 0))]
    A(#[encode(float(min = 0.0, max = 2.0, precision = 0))] f64),
}

fn main() {}
