use std::ops::Range;

use arithmetic_coding::Model;

use crate::bounded_model::BoundedModel;


#[derive(Debug)]
pub struct FloatModel {
    range: Range<u64>,
    precision: u32,
}

impl Default for FloatModel {
    fn default() -> Self {
    let range = u64::MIN..u64::MAX;
    let precision = 0;
    Self {
        range, precision
    }
    }
}

#[must_use]
#[derive(Debug, Default)]
pub struct Builder {
    model: FloatModel,
}

impl Builder {
    pub fn min(mut self, min: f32) -> Self {
        self.model.range.start = min;
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        self.model.range.end = max;
        self
    }

    pub fn precision(mut self, precision: u32) -> Self {
        self.model.precision = precision;
        self
    }

    pub fn build(self) -> FloatModel {
        self.model
    }
}

impl FloatModel {
    fn values(&self) -> u32 {
        let start = (self.range.start * 10.0_f32.powf(self.precision as f32)).round() as u32;
        let end = (self.range.end * 10.0_f32.powf(self.precision as f32)).round() as u32;
        end - start
    }
}

#[derive(Debug, thiserror::Error)]
#[error("value is out of bounds")]
pub struct OutOfBoundsError;

impl Model for FloatModel {
    type Symbol = f32;

    type ValueError = OutOfBoundsError;

    type B = u32;

    fn probability(
        &self,
        symbol: Option<&Self::Symbol>,
    ) -> Result<Range<Self::B>, Self::ValueError> {
        todo!()
    }

    fn max_denominator(&self) -> Self::B {
        todo!()
    }

    fn symbol(&self, value: Self::B) -> Option<Self::Symbol> {
        todo!()
    }
}

impl BoundedModel for FloatModel {
    fn worst_case(&self) -> f32 {
        todo!()
    }
}
