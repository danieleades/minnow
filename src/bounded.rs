//! The size-guarantee trait: [`Bounded`].
//!
//! [`Encodeable`] (the codec tier) says a type can be encoded and decoded;
//! `Bounded` adds the DCCL-style budget guarantee: a static, provable
//! worst-case size for every value of the schema. Splitting the two makes
//! unbounded models (open-ended varints, unbounded sequences, adaptive
//! models) expressible as `Encodeable`-only types, while boundedness — and
//! everything that depends on it, like [`size_report`](Bounded::size_report)
//! and the pre-decode length window on
//! [`decode_bytes`](Encodeable::decode_bytes) — propagates through the type
//! system: a derived schema implements `Bounded` exactly when every field
//! does, so asking an unbounded schema for its budget is a *compile* error,
//! not a runtime `None`.

use crate::{Encodeable, SizeReport, Weight};

/// A type whose encoded size has a finite, statically-known worst case.
///
/// This is the budget guarantee that makes Minnow suitable for
/// severely-constrained links: every value of a `Bounded` schema encodes to
/// at most [`worst_case_bits`](Bounded::worst_case_bits) bits (plus the fixed
/// coder-termination overhead — see [`crate::TERMINATION_BITS`]), and
/// [`Encodeable::decode_bytes`] uses the same bounds to reject input outside
/// the schema's provable length window before decoding.
///
/// # Implementing
///
/// [`weight`](Bounded::weight) is the single required method, and everything
/// else defaults from it: for a *uniformly weighted* type (every leaf and
/// every automatically-weighted composite) each value costs exactly
/// `log₂ weight` bits, so the defaults are exact. Override the other methods
/// only when the type's coding scheme makes values cost different amounts —
/// e.g. a manually-weighted enum discriminant, or a uniform (unweighted)
/// discriminant like the hand-written example in `examples/struct_enum.rs` —
/// and keep the overrides consistent with what
/// [`encode_with_config`](Encodeable::encode_with_config) actually emits:
/// these numbers are the crate's core promise.
///
/// A type with no finite worst case simply does not implement this trait —
/// it remains encodeable, but has no budget, no size report, and no
/// length-validated decode.
pub trait Bounded: Encodeable {
    /// The number of distinct values this type can encode under `config` — its
    /// [`Weight`].
    ///
    /// This is the cardinality of the value set: the product of field weights
    /// for structs/arrays, the sum of variant weights for enums/[`Option`], and
    /// the model denominator for leaves. See [`Weight`] for the semiring this
    /// belongs to.
    fn weight(config: &Self::Config) -> Weight;

    /// The worst-case number of bits needed to encode any value of this type
    /// under `config`.
    ///
    /// The default is `log₂` of the [`weight`](Bounded::weight), which is
    /// exact for uniform leaves and for sums/products whose weight has not
    /// saturated. Containers override this to accumulate `f64` bit counts
    /// directly (sum over fields; max over enum variants), which stays
    /// accurate even when the weight product saturates — see
    /// [`crate::SizeReport`].
    fn worst_case_bits(config: &Self::Config) -> f64 {
        Self::weight(config).log2()
    }

    /// The *best*-case number of bits — the size of the cheapest value of this
    /// type under `config`.
    ///
    /// The dual of [`worst_case_bits`](Bounded::worst_case_bits): a sum over
    /// fields but a **min** over enum variants. For uniform (automatic)
    /// weighting every value costs the same, so this equals
    /// `worst_case_bits`; with manual `#[encode(weight = …)]` overrides the
    /// cheapest and dearest values differ, and this is the lower one.
    ///
    /// It is used to bound the encoded length from below when validating input
    /// on decode (see [`crate::DecodeError::Length`]); the default matches the
    /// uniform-leaf case.
    fn best_case_bits(config: &Self::Config) -> f64 {
        Self::weight(config).log2()
    }

    /// A [`SizeReport`] tree describing the worst-case encoded size of this
    /// type under `config`.
    ///
    /// The default is an unnamed leaf carrying
    /// [`worst_case_bits`](Bounded::worst_case_bits); containers override it
    /// to expose a per-field / per-variant breakdown.
    fn report(config: &Self::Config) -> SizeReport {
        SizeReport::leaf(Self::worst_case_bits(config))
    }

    /// A [`SizeReport`] describing the worst-case encoded size of this type,
    /// using its [`Default`] configuration.
    ///
    /// This is the capacity-planning entry point: only a schema that is
    /// `Bounded` all the way down has one, so a schema that silently gained
    /// an unbounded field loses this method at *compile* time rather than
    /// reporting a wrong budget.
    #[must_use]
    fn size_report() -> SizeReport
    where
        Self::Config: Default,
    {
        Self::report(&Self::Config::default())
    }
}
