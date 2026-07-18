// `config = <expr>` accepts arbitrary Rust expressions, so it cannot be
// validated at macro-expansion time — but genuinely malformed token trees
// must still fail cleanly (a `syn`/`darling` parse error), not panic the
// macro.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
pub struct Bad {
    #[encode(config = +++)]
    x: f64,
}

fn main() {}
