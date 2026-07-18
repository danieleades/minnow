// A unit struct: weight 1, zero bits, no-op encode/decode.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Good;

fn main() {}
