// The legacy variant-level payload model (`#[encode(float(...))] Foo(f64)`)
// only makes sense for a single-field tuple variant. On a multi-field
// tuple variant it used to be silently applied to just the first field and
// ignore the rest; now it must be a clean compile error instead, pointing
// users at putting `#[encode]` attributes on the individual fields.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub enum Bad {
    #[encode(float(min = 0.0, max = 1.0, precision = 0))]
    A(f64, bool),
}

fn main() {}
