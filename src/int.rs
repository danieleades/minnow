//! A model for bounded integers (issue #3's sibling: the numeric leaf that
//! [`crate::StringModel`]/[`crate::SeqModel`] build on for byte alphabets).
//!
//! [`IntModel<T>`] encodes a value of an integer type `T` uniformly over an
//! inclusive `min..=max` range: weight `max − min + 1`, one symbol per value.
//! It is implemented for every primitive integer type from `u8`/`i8` up to
//! `u64`/`i64`.

use std::{
    convert::Infallible,
    ops::{Range, RangeInclusive},
};

use arithmetic_coding::one_shot;
use num_traits::PrimInt;

use crate::{
    DecodeError, DecodeVisitor, EncodeVisitor, EncodeableCustom, MAX_DENOMINATOR, ModelError,
    PRECISION, Weight,
};

/// Convert a supported integer type to `i128`.
///
/// Every type this module supports (`u8..=u64`, `i8..=i64`) fits losslessly
/// in `i128`, so this never fails in practice.
fn to_i128<T: PrimInt>(value: T) -> i128 {
    num_traits::ToPrimitive::to_i128(&value)
        .expect("every type IntModel supports fits losslessly in i128")
}

/// Convert an `i128` back to a supported integer type.
///
/// Callers must ensure `value` is within `T`'s range; [`IntModel`] only ever
/// calls this with offsets bounded by its own `min..=max`, so the conversion
/// always succeeds.
fn from_i128<T: PrimInt>(value: i128) -> T {
    <T as num_traits::NumCast>::from(value)
        .expect("value is within the configured range, which is within T's range")
}

/// The number of distinct values in `min..=max`, as a `u128`.
fn width<T: PrimInt>(min: T, max: T) -> u128 {
    let diff = to_i128(max) - to_i128(min);
    // `max >= min` is checked by every caller before this runs, so `diff` is
    // never negative.
    u128::try_from(diff).expect("max >= min, so the difference is non-negative")
}

/// A [`Model`](arithmetic_coding::one_shot::Model) which encodes and decodes
/// a bounded integer, uniform over `min..=max`.
///
/// The weight (see [`crate::Weight`]) of an `IntModel` is `max − min + 1`:
/// every value in the range is equally likely and costs the same fractional
/// number of bits.
#[derive(Clone, Copy, Debug)]
pub struct IntModel<T> {
    min: T,
    max: T,
}

impl<T> IntModel<T>
where
    T: PrimInt + std::fmt::Debug,
{
    /// Create a new [`IntModel`] over the given inclusive range.
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::InvertedBounds`] if `min > max`, or
    /// [`ModelError::DenominatorTooLarge`] if the number of distinguishable
    /// values in the range (`max − min + 1`) would exceed
    /// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR).
    pub fn new(range: RangeInclusive<T>) -> Result<Self, ModelError> {
        let min = *range.start();
        let max = *range.end();

        if min > max {
            return Err(ModelError::InvertedBounds);
        }

        let width = width(min, max);
        // `denominator = width + 1`; reject before the `+ 1` can overflow or
        // exceed the bound (mirrors `FloatModel::new`).
        if width >= MAX_DENOMINATOR {
            return Err(ModelError::DenominatorTooLarge {
                denominator: width.saturating_add(1),
                max: MAX_DENOMINATOR,
                precision: PRECISION,
            });
        }

        Ok(Self { min, max })
    }

    /// The number of distinct values this model can encode — its
    /// [`Weight`](crate::Weight): `max − min + 1`.
    #[must_use]
    pub fn denominator(&self) -> u128 {
        width(self.min, self.max) + 1
    }

    /// The offset of `value` from `min`, as a `u128` in `0..denominator()`.
    fn offset(&self, value: T) -> u128 {
        let diff = to_i128(value) - to_i128(self.min);
        // Encoding a value outside `min..=max` is a programming error (the
        // symbol would not belong to this model); `IntModel` is only ever fed
        // values of the concrete integer type it models, so this cannot
        // actually go negative in practice.
        u128::try_from(diff).expect("value is within the configured min..=max range")
    }

    /// The inverse of [`IntModel::offset`].
    fn unscale(&self, offset: u128) -> T {
        let offset = i128::try_from(offset).expect("offset is bounded by the denominator");
        from_i128(to_i128(self.min) + offset)
    }
}

impl<T> one_shot::Model for IntModel<T>
where
    T: PrimInt + std::fmt::Debug,
{
    type B = u128;
    type Symbol = T;
    type ValueError = Infallible;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        let offset = self.offset(*symbol);
        #[allow(clippy::range_plus_one)]
        Ok(offset..offset + 1)
    }

    fn max_denominator(&self) -> Self::B {
        self.denominator()
    }

    fn symbol(&self, value: Self::B) -> Self::Symbol {
        self.unscale(value)
    }
}

/// Implement [`EncodeableCustom`] for a primitive integer type `$t`, whose
/// config is `IntModel<$t>`.
///
/// The `@default` variant additionally gives the type a [`Default`]
/// `IntModel` spanning its entire native range (`$t::MIN..=$t::MAX`), which
/// is only safe for types whose full range fits within
/// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR) — `u64`/`i64` do not (their
/// full-range denominator, `2^64`, exceeds the ~`2^62` bound), so those two
/// types require an explicit [`IntModel::new`] range and have no `Default`.
macro_rules! impl_int_leaf {
    (@default $t:ty) => {
        impl Default for IntModel<$t> {
            fn default() -> Self {
                Self::new(<$t>::MIN..=<$t>::MAX)
                    .expect("the full range of this type fits within MAX_DENOMINATOR")
            }
        }
        impl_int_leaf!(@no_default $t);
    };
    (@no_default $t:ty) => {
        impl EncodeableCustom for $t {
            type Config = IntModel<$t>;

            fn weight(config: &Self::Config) -> Weight {
                Weight::new(config.denominator())
            }

            fn encode_with_config<W>(
                &self,
                visitor: &mut EncodeVisitor<W>,
                config: Self::Config,
            ) -> std::io::Result<()>
            where
                W: bitstream_io::BitWrite,
            {
                visitor.encode_one(config, self)
            }

            fn decode_with_config<R>(
                visitor: &mut DecodeVisitor<R>,
                config: Self::Config,
            ) -> Result<Self, DecodeError>
            where
                R: bitstream_io::BitRead,
                Self: Sized,
            {
                visitor.decode_one(config)
            }
        }
    };
}

impl_int_leaf!(@default u8);
impl_int_leaf!(@default u16);
impl_int_leaf!(@default u32);
impl_int_leaf!(@default i8);
impl_int_leaf!(@default i16);
impl_int_leaf!(@default i32);
// `u64`/`i64` have no `Default` `IntModel`: their full native range's
// denominator (`2^64`) exceeds `MAX_DENOMINATOR` (`~2^62`), so callers must
// supply an explicit bounded range.
impl_int_leaf!(@no_default u64);
impl_int_leaf!(@no_default i64);

#[cfg(test)]
mod tests {
    use arithmetic_coding::one_shot::Model;
    use test_case::test_case;

    use super::IntModel;
    use crate::ModelError;

    #[test]
    fn denominator() {
        let model = IntModel::new(-5_i32..=5).unwrap();
        assert_eq!(model.denominator(), 11);
    }

    #[test_case(0 => 0..1)]
    #[test_case(5 => 5..6)]
    #[test_case(-5 => -5..-4; "negative")]
    fn probability(input: i32) -> std::ops::Range<i32> {
        let model = IntModel::new(-5_i32..=5).unwrap();
        // Translate the returned `u128` interval back to an `i32` range for a
        // readable assertion.
        let range = model.probability(&input).unwrap();
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let start = range.start as i32 - 5;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let end = range.end as i32 - 5;
        start..end
    }

    #[test]
    fn round_trips_symbol() {
        let model = IntModel::new(-1000_i32..=1000).unwrap();
        for value in [-1000, -1, 0, 1, 1000] {
            let range = model.probability(&value).unwrap();
            assert_eq!(model.symbol(range.start), value);
        }
    }

    #[test]
    fn rejects_inverted_bounds() {
        let min = 5_i32;
        let max = -5_i32;
        assert_eq!(
            IntModel::new(min..=max).unwrap_err(),
            ModelError::InvertedBounds
        );
    }

    #[test]
    fn rejects_oversized_denominator() {
        let err = IntModel::new(0_i64..=i64::MAX).unwrap_err();
        assert!(matches!(err, ModelError::DenominatorTooLarge { .. }));
    }

    #[test]
    fn accepts_full_u64_minus_epsilon() {
        // The largest range whose weight (width + 1) is still <=
        // MAX_DENOMINATOR.
        let max_denominator = u64::try_from(crate::MAX_DENOMINATOR).unwrap();
        assert!(IntModel::new(0_u64..=(max_denominator - 1)).is_ok());
        assert!(IntModel::new(0_u64..=max_denominator).is_err());
    }

    #[test]
    fn u8_default_spans_full_range() {
        let model = IntModel::<u8>::default();
        assert_eq!(model.denominator(), 256);
    }

    #[test]
    fn i8_default_spans_full_range() {
        let model = IntModel::<i8>::default();
        assert_eq!(model.denominator(), 256);
    }
}
