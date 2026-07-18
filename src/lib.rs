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
mod visitor;

pub use encodeable::Encodeable;
pub use encodeable_custom::EncodeableCustom;
pub use error::DecodeError;
pub use float::{FloatModel, ModelError};
pub use impls::one_shot::OneShot;
pub use minnow_derive::Encodeable;
pub use visitor::{DecodeVisitor, EncodeVisitor};

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
/// The arithmetic coder renormalises its working interval so its width stays
/// within `[2^(P-2), 2^P]`. For every symbol to map to a *distinct*, non-empty
/// sub-interval after scaling by `probability / denominator`, and — more
/// stringently — for the internal `range * denominator` product to fit in the
/// `u128` state (see [`PRECISION`]), the denominator is capped at
///
/// ```text
/// D = 2^(PRECISION - 2) = 2^62 = 4_611_686_018_427_387_904
/// ```
///
/// At `PRECISION = 64` the governing product `(2^P + 1) * D = 2^126 + 2^62`
/// stays below `2^128 - 1`. Exceeding `D` risks silent round-trip corruption,
/// so every model constructor validates its denominator against this bound —
/// though at `~2^62` (over four quintillion distinguishable values) the bound
/// is unreachable for any practical model.
pub const MAX_DENOMINATOR: u128 = 1 << (PRECISION - 2);
