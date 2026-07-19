use std::{
    convert::Infallible,
    ops::{Range, RangeInclusive},
};

use arithmetic_coding::one_shot;
use num_traits::Float;

use crate::{EncodeError, MAX_DENOMINATOR, ModelError};

/// A [`Model`](arithmetic_coding::Model) which (lossily) encodes and decodes
/// floating point values.
#[derive(Clone, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct FloatModel<F>
where
    F: Float,
{
    min: F,
    max: F,
    precision: i8,
    clamping: bool,
}

impl<F> Default for FloatModel<F>
where
    F: Float + std::fmt::Debug,
{
    fn default() -> Self {
        let min = F::from(-1_000_000).unwrap();
        let max = F::from(1_000_000).unwrap();
        Self::new(min..=max, 0).expect("the default FloatModel bounds are valid")
    }
}

impl<F> FloatModel<F>
where
    F: Float + std::fmt::Debug,
{
    /// Create a new [`FloatModel`] with the given range and precision.
    ///
    /// Encoding a value outside this range is an [`EncodeError::OutOfRange`]
    /// unless [clamping](FloatModel::clamping) mode is enabled. The
    /// `precision` is the number of decimal digits retained (it may be
    /// negative to quantise more coarsely than integers).
    ///
    /// # Errors
    ///
    /// Returns [`ModelError`] if either bound is `NaN`/infinite, if `min >
    /// max`, or if the number of distinguishable values in the range would
    /// exceed [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR).
    pub fn new(range: RangeInclusive<F>, precision: i8) -> Result<Self, ModelError> {
        let min = *range.start();
        let max = *range.end();

        if !min.is_finite() || !max.is_finite() {
            return Err(ModelError::NonFiniteBounds);
        }
        if min > max {
            return Err(ModelError::InvertedBounds);
        }

        let model = Self {
            min,
            max,
            precision,
            clamping: false,
        };

        // The denominator is the number of distinguishable values in the range.
        let steps = ((max - min) * model.multiplier()).round();
        let steps = steps.to_u128().ok_or(ModelError::DenominatorTooLarge {
            denominator: u128::MAX,
            max: MAX_DENOMINATOR,
            precision: crate::PRECISION,
        })?;
        // `denominator = steps + 1`; reject before the `+ 1` can overflow or
        // exceed the bound.
        if steps >= MAX_DENOMINATOR {
            return Err(ModelError::DenominatorTooLarge {
                denominator: steps.saturating_add(1),
                max: MAX_DENOMINATOR,
                precision: crate::PRECISION,
            });
        }

        Ok(model)
    }

    /// The number of distinct values this model can encode — its
    /// [`Weight`](crate::Weight).
    ///
    /// This is the model's denominator: `round((max - min) · 10^precision) +
    /// 1`. It is guaranteed by [`FloatModel::new`] to be at most
    /// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR).
    #[must_use]
    pub fn denominator(&self) -> u128 {
        self.scale(self.max) + 1
    }

    /// Enable clamping mode: encoding a value outside `min..=max` clamps it
    /// to the nearest bound instead of returning
    /// [`EncodeError::OutOfRange`].
    ///
    /// In-range quantisation (rounding to the declared precision) always
    /// happens and is not affected — its loss is bounded by half a
    /// quantisation step, which the schema explicitly declares. Saturation
    /// opts into *unbounded* loss at the boundaries, which is the right
    /// policy for naturally saturating sources (sensor channels) and the
    /// wrong one for most everything else — hence opt-in. NaN and infinite
    /// values are an [`EncodeError::NonFinite`] in every mode: no nearest
    /// representable value exists.
    #[must_use]
    pub const fn clamping(mut self) -> Self {
        self.clamping = true;
        self
    }

    /// Validate `value` against this model's domain, returning the value that
    /// will actually be encoded.
    ///
    /// # Errors
    ///
    /// [`EncodeError::NonFinite`] for NaN/infinite values;
    /// [`EncodeError::OutOfRange`] for values outside `min..=max` unless
    /// [clamping](FloatModel::clamping) mode is enabled.
    pub(crate) fn admit(&self, value: F) -> Result<F, EncodeError> {
        if !value.is_finite() {
            return Err(EncodeError::NonFinite {
                value: format!("{value:?}"),
            });
        }
        if value < self.min || value > self.max {
            if self.clamping {
                return Ok(num_traits::clamp(value, self.min, self.max));
            }
            return Err(EncodeError::OutOfRange {
                value: format!("{value:?}"),
                min: format!("{:?}", self.min),
                max: format!("{:?}", self.max),
            });
        }
        Ok(value)
    }

    fn multiplier(&self) -> F {
        F::from(10_u32).unwrap().powi(self.precision.into())
    }

    fn scale(&self, value: F) -> u128 {
        debug_assert!(
            value >= self.min && value <= self.max,
            "scale is only called with admitted (in-range) values"
        );
        let float = ((value - self.min) * self.multiplier()).round();
        num_traits::ToPrimitive::to_u128(&float).unwrap()
    }

    fn unscale(&self, value: u128) -> F {
        let input = F::from(value).unwrap();
        (input / self.multiplier()) + self.min
    }
}

impl<F> one_shot::Model for FloatModel<F>
where
    F: Float + std::fmt::Debug,
{
    type B = u128;
    type Symbol = F;
    type ValueError = Infallible;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        #[allow(clippy::range_plus_one)]
        Ok(self.scale(*symbol)..self.scale(*symbol) + 1)
    }

    fn max_denominator(&self) -> Self::B {
        self.denominator()
    }

    fn symbol(&self, value: Self::B) -> Self::Symbol {
        self.unscale(value)
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use arithmetic_coding::fixed_length::Model;
    use test_case::test_case;

    use super::FloatModel;
    use crate::{EncodeError, MAX_DENOMINATOR, ModelError};

    #[test]
    fn denominator() {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
            clamping: false,
        };

        assert_eq!(model.denominator(), 11);
    }

    #[test_case(0.0 => 0)]
    #[test_case(0.5 => 5)]
    #[test_case(1.0 => 10)]
    fn scale(input: f64) -> u128 {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
            clamping: false,
        };

        model.scale(input)
    }

    #[test_case(0.0 => 0..1)]
    #[test_case(0.1 => 1..2)]
    #[test_case(1.0 => 10..11)]
    fn probability(input: f64) -> Range<u128> {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
            clamping: false,
        };

        model.probability(&input).unwrap()
    }

    #[test_case(0 => 0.0)]
    #[test_case(1 => 0.1)]
    #[test_case(2 => 0.2)]
    #[test_case(10 => 1.0)]
    #[allow(clippy::float_cmp)]
    fn symbol(value: u128) -> f64 {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
            clamping: false,
        };

        model.symbol(value)
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn probability_y() {
        let model = FloatModel::new(-10000.0..=10000.0, 1).unwrap();

        assert_eq!(model.probability(&2.0).unwrap(), 100_020..100_021);

        assert_eq!(model.symbol(100_020), 2.0);
    }

    #[test]
    fn rejects_nan_bounds() {
        assert_eq!(
            FloatModel::new(f64::NAN..=1.0, 0).unwrap_err(),
            ModelError::NonFiniteBounds
        );
    }

    #[test]
    fn rejects_inverted_bounds() {
        assert_eq!(
            FloatModel::new(1.0..=0.0, 0).unwrap_err(),
            ModelError::InvertedBounds
        );
    }

    #[test]
    fn rejects_oversized_denominator() {
        // 10^19 integer steps comfortably exceeds MAX_DENOMINATOR (2^62).
        let err = FloatModel::new(0.0..=1e19, 0).unwrap_err();
        assert!(matches!(err, ModelError::DenominatorTooLarge { .. }));
    }

    #[test]
    fn accepts_large_denominator() {
        // A billion distinguishable values is far below MAX_DENOMINATOR (2^62).
        const _: () = assert!(MAX_DENOMINATOR > 1_000_000_000);
        assert!(FloatModel::new(0.0..=1_000_000_000.0, 0).is_ok());
    }

    /// Out-of-range values are an error by default, clamp only under the
    /// explicit clamping opt-in, and NaN is an error in every mode.
    #[test]
    fn admit_semantics() {
        let strict = FloatModel::new(0.0..=1.0, 1).unwrap();
        assert!(matches!(
            strict.admit(1.1),
            Err(EncodeError::OutOfRange { .. })
        ));
        assert!(matches!(
            strict.admit(f64::NAN),
            Err(EncodeError::NonFinite { .. })
        ));

        let clamping = FloatModel::new(0.0..=1.0, 1).unwrap().clamping();
        assert!((clamping.admit(1.1_f64).unwrap() - 1.0).abs() < f64::EPSILON);
        assert!(clamping.admit(-5.0_f64).unwrap().abs() < f64::EPSILON);
        // No nearest representable value exists for NaN, even when clamping.
        assert!(matches!(
            clamping.admit(f64::NAN),
            Err(EncodeError::NonFinite { .. })
        ));
    }
}
