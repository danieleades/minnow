use std::io;

use bitstream_io::{BitRead, BitWrite};

use self::one_shot::OneShot;
use crate::{
    encodeable_custom::EncodeableCustom, float::FloatModel, DecodeError, DecodeVisitor,
    EncodeVisitor, Encodeable,
};

pub mod one_shot;

impl<T> EncodeableCustom for Option<T>
where
    T: EncodeableCustom,
{
    type Config = T::Config;

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: T::Config,
    ) -> io::Result<()>
    where
        W: BitWrite,
    {
        match self {
            Some(x) => {
                OptionDiscriminant::Some.encode(visitor)?;
                x.encode_with_config(visitor, config)
            }
            None => OptionDiscriminant::None.encode(visitor),
        }
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: T::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
    {
        match OptionDiscriminant::decode(visitor)? {
            OptionDiscriminant::Some => {
                let x = T::decode_with_config(visitor, config)?;
                Ok(Some(x))
            }
            OptionDiscriminant::None => Ok(Option::None),
        }
    }
}

/// The wire discriminant for [`Option`], encoded before any payload.
///
/// The wire order is `None = 0`, `Some = 1`.
#[derive(Debug, Clone, Copy)]
pub enum OptionDiscriminant {
    /// Corresponds to [`Option::None`].
    None,
    /// Corresponds to [`Option::Some`].
    Some,
}

impl Encodeable for OptionDiscriminant {
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> io::Result<()>
    where
        W: BitWrite,
    {
        let value = match self {
            OptionDiscriminant::None => 0,
            OptionDiscriminant::Some => 1,
        };
        let model = OneShot::<2>;
        visitor.encode_one(model, &value)
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        let model = OneShot::<2>;
        match visitor.decode_one(model)? {
            0 => Ok(OptionDiscriminant::None),
            1 => Ok(OptionDiscriminant::Some),
            other => Err(DecodeError::InvalidSymbol {
                symbol: u128::from(other),
            }),
        }
    }
}

impl EncodeableCustom for f64 {
    type Config = FloatModel<f64>;

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
        // through does not require conjuring a `T` out of thin air (as the old
        // `MaybeUninit::assume_init` did — instant undefined behaviour). Once
        // every element has decoded successfully, unwrap them into `[T; N]`.
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
        encodeable_custom::EncodeableCustom, float::FloatModel, DecodeVisitor, EncodeVisitor,
        Encodeable, PRECISION,
    };

    #[test_case(&Option::Some(true))]
    #[test_case(&Option::Some(false))]
    #[test_case(&true)]
    #[test_case(&false)]
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
}
