//! The shared error type for fallible leaf-model constructors.

/// An error returned when a model cannot be constructed because its
/// parameters violate the arithmetic coder's precision invariant (see
/// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR)) or are otherwise invalid.
///
/// Every leaf model with a fallible constructor
/// ([`FloatModel::new`](crate::FloatModel::new),
/// [`IntModel::new`](crate::IntModel::new),
/// [`StringModel::new`](crate::StringModel::new)) shares this one error type,
/// so callers only need to handle a single error shape regardless of which
/// model they are building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ModelError {
    /// One or both of the supplied bounds was `NaN` or infinite.
    #[error("model bounds must be finite (neither NaN nor infinite)")]
    NonFiniteBounds,

    /// The lower bound was greater than the upper bound.
    #[error("model lower bound must not exceed the upper bound")]
    InvertedBounds,

    /// The resulting denominator exceeds
    /// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR).
    #[error(
        "model denominator ({denominator}) exceeds the maximum ({max}) permitted at precision \
         {precision}; narrow the range or reduce the precision"
    )]
    DenominatorTooLarge {
        /// The denominator that was requested.
        denominator: u128,
        /// The maximum permissible denominator.
        max: u128,
        /// The arithmetic coder precision in bits.
        precision: u32,
    },
}
