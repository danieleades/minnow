// The derive only supports structs and enums; a `union` must fail cleanly
// (rejected by `darling`'s `FromDeriveInput`), not panic.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub union Bad {
    x: f64,
    y: u32,
}

fn main() {}
