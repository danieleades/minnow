#![deny(
    clippy::all,
    clippy::cargo,
    missing_docs,
    missing_copy_implementations,
    missing_debug_implementations
)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]
// `arithmetic-coding-core` 0.4 depends on `thiserror` 1, while this crate uses
// `thiserror` 2. The two versions coexist only as transitive/direct build
// dependencies; there is nothing we can do about the upstream pin, so this
// otherwise-useful lint is silenced.
#![allow(clippy::multiple_crate_versions)]
#![doc = include_str!("../README.md")]

mod encodeable;
mod encodeable_custom;
mod error;
mod float;
mod impls;
mod int;
mod model_error;
mod report;
mod seq;
mod string;
mod visitor;
mod weight;

pub use encodeable::Encodeable;
pub use encodeable_custom::EncodeableCustom;
pub use error::DecodeError;
pub use float::FloatModel;
pub use impls::{one_shot::OneShot, weighted::WeightedModel};
pub use int::IntModel;
pub use minnow_derive::Encodeable;
pub use model_error::ModelError;
pub use report::{SizeReport, TERMINATION_BITS};
pub use seq::SeqModel;
pub use string::StringModel;
pub use visitor::{DecodeVisitor, EncodeVisitor};
pub use weight::Weight;

/// The precision (in bits) of the arithmetic coder used throughout this crate.
///
/// Every value is encoded through a single coder state configured with this
/// precision. It is fixed rather than derived per-model so that many one-shot
/// models can be chained through one shared state.
///
/// # Why 64?
///
/// The arithmetic coder's internal state is stored in a `u128` (`B = u128`).
/// The output *length* of an arithmetic code is independent of the precision —
/// `P` only sizes the fixed-point state used to track the coding interval, so
/// we pick the largest `P` that keeps that state's arithmetic from overflowing
/// a `u128`, which makes the [`MAX_DENOMINATOR`] bound effectively irrelevant
/// for realistic models.
///
/// The interval is initialised to `[0, 2^P)` and renormalised so its width
/// never exceeds `2^P`. Encoding scales the interval by
/// `range * probability / denominator`, and decoding forms
/// `(x - low + 1) * denominator / range` — both of which multiply a value up to
/// `range ≤ 2^P + 1` by a value up to `denominator ≤ D`. The product must fit
/// in a `u128`:
///
/// ```text
/// (2^P + 1) * D <= 2^128 - 1
/// ```
///
/// With `D = 2^(P - 2)` (see [`MAX_DENOMINATOR`]) this is
/// `2^(2P - 2) + 2^(P - 2) <= 2^128 - 1`. `P = 65` would need
/// `2^128 + 2^63`, which overflows, whereas `P = 64` needs only
/// `2^126 + 2^62`, comfortably inside a `u128`. Hence `P = 64` is the largest
/// safe precision for `B = u128`.
pub const PRECISION: u32 = 64;

/// The largest model denominator that can be encoded safely at [`PRECISION`].
///
/// # The invariant
///
/// This is `arithmetic-coding`'s own guard, restated: the coder computes
/// `frequency_bits = floor(log2(denominator)) + 1` and requires
///
/// ```text
/// PRECISION >= frequency_bits + 2
/// ```
///
/// (see `Encoder::with_precision` in `arithmetic-coding` 0.4 — a
/// `debug_assert` there, which the `with_state` construction used by Minnow's
/// visitors bypasses entirely, so Minnow must enforce it itself). At
/// `PRECISION = 64` that admits `frequency_bits <= 62`, i.e.
///
/// ```text
/// D = 2^(PRECISION - 2) - 1 = 2^62 - 1 = 4_611_686_018_427_387_903
/// ```
///
/// Note the `- 1`: a denominator of *exactly* `2^62` has
/// `frequency_bits = 63` and would need 65 bits of precision. Exceeding `D`
/// risks silent round-trip corruption, so every model constructor validates
/// its denominator against this bound — though at `~2^62` (over four
/// quintillion distinguishable values) the bound is unreachable for any
/// practical model.
pub const MAX_DENOMINATOR: u128 = (1 << (PRECISION - 2)) - 1;

#[cfg(test)]
mod precision_tests {
    use super::{MAX_DENOMINATOR, PRECISION};

    /// `frequency_bits` exactly as `arithmetic-coding` 0.4 computes it.
    fn frequency_bits(denominator: u128) -> u32 {
        denominator.ilog2() + 1
    }

    /// [`MAX_DENOMINATOR`] is the *largest* denominator satisfying the
    /// upstream coder invariant `PRECISION >= frequency_bits + 2` — the bound
    /// is safe and tight.
    #[test]
    fn max_denominator_is_tight() {
        assert!(PRECISION >= frequency_bits(MAX_DENOMINATOR) + 2);
        assert!(PRECISION < frequency_bits(MAX_DENOMINATOR + 1) + 2);
    }
}
