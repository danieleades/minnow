// An `#[encode(unbounded)]` enum cannot use automatic discriminant
// weighting (it needs payload cardinalities, which an unbounded schema does
// not have), so every variant must carry an explicit `#[encode(weight = N)]`.
use minnow_derive::Encodeable;

#[derive(Encodeable)]
#[encode(unbounded)]
pub enum Bad {
    #[encode(weight = 3)]
    Weighted,
    Unweighted,
}

fn main() {}
