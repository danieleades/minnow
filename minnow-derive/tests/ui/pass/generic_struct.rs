// A generic struct: the derive adds the `Encodeable`/`Bounded`/`Config: Default`
// bounds itself, so the user doesn't have to spell them out.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Good<T> {
    inner: T,
}

fn main() {}
