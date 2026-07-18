// `weight = 0` would make a variant unencodable (a zero-width interval);
// must be a clean compile error.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub enum Bad {
    #[encode(weight = 0)]
    A,
    B,
}

fn main() {}
