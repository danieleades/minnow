// `#[encode(config = <expr>)]` accepts an arbitrary expression producing the
// field's config.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Good {
    #[encode(config = minnow::FloatModel::new(-1.0..=1.0, 1).unwrap())]
    x: f64,
}

fn main() {}
