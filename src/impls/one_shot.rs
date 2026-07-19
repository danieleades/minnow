use std::{convert::Infallible, ops::Range};

use arithmetic_coding::one_shot;

/// A convenience model for encoding a single symbol from a fixed set of
/// possible symbols.
///
/// This model has a constant value `N` representing the total number of
/// possible symbols. The distribution is uniform, so each symbol costs exactly
/// `log2(N)` bits (fractionally).
#[derive(Default, Debug)]
pub struct OneShot<const N: u32>;

impl<const N: u32> one_shot::Model for OneShot<N> {
    type B = u128;
    type Symbol = u32;
    type ValueError = Infallible;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        debug_assert!(
            *symbol < N,
            "symbol {symbol} is out of range for a OneShot model over {N} symbols",
        );
        Ok((*symbol).into()..(symbol + 1).into())
    }

    fn max_denominator(&self) -> Self::B {
        N.into()
    }

    fn symbol(&self, value: Self::B) -> Self::Symbol {
        value.try_into().unwrap()
    }
}
