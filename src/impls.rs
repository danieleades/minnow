use std::io;

use bitstream_io::{BitRead, BitWrite};

use self::{one_shot::OneShot, weighted::WeightedModel};
use crate::{
    DecodeError, DecodeVisitor, EncodeVisitor, SizeReport, Weight,
    encodeable_custom::EncodeableCustom, float::FloatModel,
};

pub mod one_shot;
pub mod weighted;

/// The weighted discriminant model shared by *every* `Option<T>` code path.
///
/// Wire order `None = 0`, `Some = 1`, with interval widths `{None: 1, Some:
/// W(T)}`. Encode, decode, and the bit-accounting methods must all use this one
/// constructor: if they built their models independently they could drift
/// apart, corrupting either the wire format or the pre-decode length window.
fn option_discriminant<T>(config: &T::Config) -> WeightedModel
where
    T: EncodeableCustom,
{
    WeightedModel::new([1, T::weight(config).get()])
}

impl<T> EncodeableCustom for Option<T>
where
    T: EncodeableCustom,
{
    type Config = T::Config;

    fn weight(config: &Self::Config) -> Weight {
        // Sum rule: `None` contributes one value, `Some(x)` contributes `W(T)`.
        Weight::ONE + T::weight(config)
    }

    fn worst_case_bits(config: &Self::Config) -> f64 {
        // Under exact weighting both variants cost `log₂(1 + W(T))`; the max
        // also stays correct if the discriminant is rescaled.
        let model = option_discriminant::<T>(config);
        let none_bits = model.discriminant_bits(0);
        let some_bits = model.discriminant_bits(1) + T::worst_case_bits(config);
        none_bits.max(some_bits)
    }

    fn best_case_bits(config: &Self::Config) -> f64 {
        let model = option_discriminant::<T>(config);
        let none_bits = model.discriminant_bits(0);
        let some_bits = model.discriminant_bits(1) + T::best_case_bits(config);
        none_bits.min(some_bits)
    }

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: T::Config,
    ) -> io::Result<()>
    where
        W: BitWrite,
    {
        // Every value of `Option<T>` costs exactly `log₂(1 + W(T))` bits under
        // the shared weighted discriminant.
        let model = option_discriminant::<T>(&config);
        match self {
            Some(x) => {
                visitor.encode_one(model, &1_u32)?;
                x.encode_with_config(visitor, config)
            }
            None => visitor.encode_one(model, &0_u32),
        }
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: T::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
    {
        let model = option_discriminant::<T>(&config);
        match visitor.decode_one(model)? {
            0 => Ok(Option::None),
            1 => {
                let x = T::decode_with_config(visitor, config)?;
                Ok(Some(x))
            }
            other => Err(DecodeError::InvalidSymbol {
                symbol: u128::from(other),
            }),
        }
    }
}

impl EncodeableCustom for () {
    type Config = ();

    fn weight(_config: &Self::Config) -> Weight {
        // A single encodable value: the unit itself.
        Weight::ONE
    }

    fn encode_with_config<W>(&self, _visitor: &mut EncodeVisitor<W>, _config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        // Nothing to encode: `weight() == Weight::ONE`, so `()` costs zero
        // bits on the wire.
        Ok(())
    }

    fn decode_with_config<R>(
        _visitor: &mut DecodeVisitor<R>,
        _config: (),
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        Ok(())
    }
}

impl EncodeableCustom for f64 {
    type Config = FloatModel<f64>;

    fn weight(config: &Self::Config) -> Weight {
        Weight::new(config.denominator())
    }

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: Self::Config,
    ) -> io::Result<()>
    where
        W: BitWrite,
    {
        visitor.encode_one(config, self)
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: Self::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        visitor.decode_one(config)
    }
}

impl EncodeableCustom for bool {
    type Config = ();

    fn weight(_config: &Self::Config) -> Weight {
        Weight::new(2)
    }

    fn encode_with_config<W>(&self, visitor: &mut EncodeVisitor<W>, _config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        let model = OneShot::<2>;
        let value = u32::from(*self);
        visitor.encode_one(model, &value)
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        _config: (),
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        let model = OneShot::<2>;
        match visitor.decode_one(model)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(DecodeError::InvalidSymbol {
                symbol: u128::from(other),
            }),
        }
    }
}

impl<T, const N: usize> EncodeableCustom for [T; N]
where
    T: EncodeableCustom,
    T::Config: Clone,
{
    type Config = T::Config;

    fn weight(config: &Self::Config) -> Weight {
        // Product rule: `W([T; N]) = W(T)^N`.
        #[allow(clippy::cast_possible_truncation)]
        let exp = N as u32;
        T::weight(config).pow(exp)
    }

    fn worst_case_bits(config: &Self::Config) -> f64 {
        // Sum the per-element cost in `f64`, which stays exact even when the
        // weight product saturates.
        #[allow(clippy::cast_precision_loss)]
        let n = N as f64;
        n * T::worst_case_bits(config)
    }

    fn best_case_bits(config: &Self::Config) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let n = N as f64;
        n * T::best_case_bits(config)
    }

    fn report(config: &Self::Config) -> SizeReport {
        // A compact tree — one element *template* rather than one node per
        // element (`N` can be large, and this report is built on every
        // `decode_bytes` call). The node's bit total is computed
        // arithmetically instead of summed from children, so it stays exact.
        let element = T::report(config).with_name(format!("element (× {N})"));
        SizeReport {
            name: None,
            bits: Self::worst_case_bits(config),
            children: vec![element],
        }
    }

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: T::Config,
    ) -> io::Result<()>
    where
        W: BitWrite,
    {
        self.iter()
            .try_for_each(|x| x.encode_with_config(visitor, config.clone()))
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: T::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        // Decode into an array of `Option<T>` so that a decode failure part-way
        // through does not require conjuring a `T` out of thin air. Once every
        // element has decoded successfully, unwrap them into `[T; N]`.
        let mut error: Option<DecodeError> = None;
        let decoded: [Option<T>; N] = std::array::from_fn(|_| {
            if error.is_some() {
                return None;
            }
            match T::decode_with_config(visitor, config.clone()) {
                Ok(value) => Some(value),
                Err(e) => {
                    error = Some(e);
                    None
                }
            }
        });

        if let Some(e) = error {
            return Err(e);
        }

        // Every element decoded successfully, so all are `Some`.
        Ok(decoded.map(|value| value.expect("all elements decoded successfully")))
    }
}

#[cfg(test)]
mod tests {
    use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
    use test_case::test_case;

    use crate::{
        DecodeVisitor, EncodeVisitor, Encodeable, PRECISION, encodeable_custom::EncodeableCustom,
        float::FloatModel,
    };

    #[test_case(&Option::Some(true))]
    #[test_case(&Option::Some(false))]
    #[test_case(&true)]
    #[test_case(&false)]
    #[test_case(&())]
    fn round_trip<T>(input: &T)
    where
        T: Encodeable + std::fmt::Debug + PartialEq,
    {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);

        let mut encoder = EncodeVisitor::new(PRECISION, &mut bit_writer);

        input.encode(&mut encoder).unwrap();
        encoder.flush().unwrap();
        bit_writer.byte_align().unwrap();
        bit_writer.flush().unwrap();

        let compressed = bit_writer.into_writer();

        let bit_reader = BitReader::endian(compressed.as_slice(), BigEndian);

        let mut decoder = DecodeVisitor::new(PRECISION, bit_reader);

        let output = T::decode(&mut decoder).unwrap();

        assert_eq!(input, &output);
    }

    #[test_case(&Option::Some(true), ())]
    #[test_case(&Option::Some(false), ())]
    #[test_case(&true, ())]
    #[test_case(&false, ())]
    #[test_case(&(), ())]
    #[test_case(&450.0_f64, FloatModel::new(-10000.0..=10000.0, 1).unwrap())]
    #[test_case(&550.0_f64, FloatModel::new(-10000.0..=10000.0, 1).unwrap())]
    #[test_case(&-100.0_f64, FloatModel::new(-5000.0..=0.0, 0).unwrap())]
    fn round_trip_with_config<T>(input: &T, config: T::Config)
    where
        T: EncodeableCustom + std::fmt::Debug + PartialEq,
        T::Config: Clone,
    {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);

        let mut encoder = EncodeVisitor::new(PRECISION, &mut bit_writer);

        input
            .encode_with_config(&mut encoder, config.clone())
            .unwrap();
        encoder.flush().unwrap();
        bit_writer.byte_align().unwrap();
        bit_writer.flush().unwrap();

        let compressed = bit_writer.into_writer();

        let bit_reader = BitReader::endian(compressed.as_slice(), BigEndian);

        let mut decoder = DecodeVisitor::new(PRECISION, bit_reader);

        let output = T::decode_with_config(&mut decoder, config).unwrap();

        assert_eq!(input, &output);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn unit_type_costs_zero_bits() {
        assert_eq!(<() as EncodeableCustom>::weight(&()), crate::Weight::ONE);
        assert_eq!(<() as EncodeableCustom>::worst_case_bits(&()), 0.0);
        // Zero payload bits still incurs coder-termination overhead, rounded
        // up to a whole byte.
        assert_eq!(().encode_bytes().len(), 1);
        assert_eq!(<() as Encodeable>::size_report().total_bytes(), 1);
    }
}
