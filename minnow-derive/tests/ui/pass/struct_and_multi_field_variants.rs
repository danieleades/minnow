// Struct-style and multi-field tuple enum variants, with per-field
// attributes.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub enum Good {
    Unit,
    Tuple(
        #[encode(float(min = 0.0, max = 1.0, precision = 0))] f64,
        bool,
    ),
    Struct {
        #[encode(float(min = 0.0, max = 1.0, precision = 0))]
        x: f64,
        y: bool,
    },
}

fn main() {}
