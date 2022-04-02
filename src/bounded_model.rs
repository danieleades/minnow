pub trait BoundedModel {
    /// Return the worst-case number of bits required to encode symbols using
    /// this model
    ///
    /// [`Models`](Model) that are capable of estimating an upper bound for the
    /// encoding size will usually do so my using a non-adaptive model, and
    /// upper limits on the number of symbols that can be encoded. This doesn't
    /// need to be a whole number.
    ///
    /// This number must never be exceeded.
    fn worst_case(&self) -> f32;
}

pub trait Codec<M>
where
    M: BoundedModel,
{
    fn model(&self) -> M;
}
