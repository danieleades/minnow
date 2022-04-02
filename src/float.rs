use std::ops::Range;

use arithmetic_coding::{
    fixed_length::{self, Wrapper},
    one_shot, Model,
};

use crate::bounded_model::BoundedModel;

#[derive(Debug, thiserror::Error)]
#[error("value is out of bounds")]
pub struct Error {}

pub struct FloatModel;

impl one_shot::Model for FloatModel {
    type B = u128;
    type Symbol = f64;
    type ValueError = Error;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        todo!()
    }

    fn max_denominator(&self) -> Self::B {
        todo!()
    }

    fn symbol(&self, value: Self::B) -> Self::Symbol {
        todo!()
    }
}
