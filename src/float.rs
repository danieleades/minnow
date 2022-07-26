use std::ops::{Range, RangeInclusive};

use arithmetic_coding::one_shot;
use num_traits::Float;

#[derive(Clone)]
pub struct FloatModel<F>
where
    F: Float,
{
    min: F,
    max: F,
    precision: i8,
}

impl<F> Default for FloatModel<F>
where
    F: Float + std::fmt::Debug,
{
    fn default() -> Self {
        let min = F::from(-1_000_000).unwrap();
        let max = F::from(1_000_000).unwrap();
        Self::new(min..=max, 0)
    }
}

impl<F> FloatModel<F>
where
    F: Float + std::fmt::Debug,
{
    pub fn new(range: RangeInclusive<F>, precision: i8) -> Self {
        let model = Self {
            min: *range.start(),
            max: *range.end(),
            precision,
        };

        debug_assert!(
            (model.max - model.min) * model.multiplier() < F::max_value(),
            "too many values in range!"
        );
        model
    }

    fn multiplier(&self) -> F {
        F::from(10_u32).unwrap().powi(self.precision.into())
    }

    fn scale(&self, value: F) -> u128 {
        let input = num_traits::clamp(value, self.min, self.max);
        let float = ((input - self.min) * self.multiplier()).round();
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
    type ValueError = !;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        Ok(self.scale(*symbol)..self.scale(*symbol) + 1)
    }

    fn max_denominator(&self) -> Self::B {
        self.scale(self.max) + 1
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

    #[test]
    fn denominator() {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
        };

        assert_eq!(model.denominator(), 11);
    }

    #[test_case(0.0 => 0)]
    #[test_case(0.5 => 5)]
    #[test_case(1.0 => 10)]
    #[test_case(1.1 => 10)]
    fn scale(input: f64) -> u128 {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
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
        };

        model.probability(&input).unwrap()
    }

    #[test_case(0 => 0.0)]
    #[test_case(1 => 0.1)]
    #[test_case(2 => 0.2)]
    #[test_case(10 => 1.0)]
    fn symbol(value: u128) -> f64 {
        let model = FloatModel {
            min: 0.0,
            max: 1.0,
            precision: 1,
        };

        model.symbol(value)
    }

    #[test]
    fn probability_y() {
        let model = FloatModel::new(-10000.0..=10000.0, 1);

        assert_eq!(model.probability(&2.0).unwrap(), 100020..100021);

        assert_eq!(model.symbol(100020), 2.0);
    }
}
