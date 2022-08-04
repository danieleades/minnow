use std::{io, mem::MaybeUninit};

use bitstream_io::{BitRead, BitWrite};

use self::one_shot::OneShot;
use crate::{float::FloatModel, DecodeVisitor, EncodeVisitor, Encodeable, encodeable_custom::EncodeableCustom};

pub mod one_shot;

impl<T> EncodeableCustom for Option<T>
where
    T: EncodeableCustom,
{
    fn encode_with_config<W>(&self, visitor: &mut EncodeVisitor<W>, config: T::Config) -> io::Result<()>
    where
        W: BitWrite,
    {
        match self {
            Some(x) => {
                Option__::Some.encode(visitor)?;
                x.encode_with_config(visitor, config)
            }
            None => Option__::None.encode(visitor),
        }
    }

    fn decode_with_config<R>(visitor: &mut DecodeVisitor<R>, config: T::Config) -> io::Result<Self>
    where
        R: BitRead,
    {
        match Option__::decode(visitor)? {
            Option__::Some => {
                let x = T::decode_with_config(visitor, config)?;
                Ok(Some(x))
            }
            Option__::None => Ok(Option::None),
        }
    }

    type Config = T::Config;
}

pub enum Option__ {
    Some,
    None,
}

impl Encodeable for Option__ {
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> io::Result<()>
    where
        W: BitWrite,
    {
        let value = match self {
            Option__::Some => 0,
            Option__::None => 1,
        };
        let model = OneShot::<2>::default();
        visitor.encode_one(model, &value)
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        let model = OneShot::<2>::default();
        match visitor.decode_one(model)? {
            0 => Ok(Option__::Some),
            1 => Ok(Option__::None),
            _ => unreachable!(),
        }
    }
}

impl EncodeableCustom for f64 {
    fn encode_with_config<W>(&self, visitor: &mut EncodeVisitor<W>, config: Self::Config) -> io::Result<()>
    where
        W: BitWrite,
    {
        visitor.encode_one(config, self)
    }

    fn decode_with_config<R>(visitor: &mut DecodeVisitor<R>, config: Self::Config) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        visitor.decode_one(config)
    }

    type Config = FloatModel<f64>;
}

impl EncodeableCustom for bool {
    fn encode_with_config<W>(&self, visitor: &mut EncodeVisitor<W>, _config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        let model = OneShot::<2>::default();
        let value = u32::from(*self);
        visitor.encode_one(model, &value)
    }

    fn decode_with_config<R>(visitor: &mut DecodeVisitor<R>, _config: ()) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        let model = OneShot::<2>::default();
        match visitor.decode_one(model)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => unreachable!(),
        }
    }

    type Config = ();
}

impl<T, const N: usize> EncodeableCustom for [T; N]
where
    T: EncodeableCustom,
    T::Config: Clone,
{
    fn encode_with_config<W>(&self, visitor: &mut EncodeVisitor<W>, config: T::Config) -> io::Result<()>
    where
        W: BitWrite,
    {
        self.iter()
            .try_for_each(|x| x.encode_with_config(visitor, config.clone()))
    }

    fn decode_with_config<R>(visitor: &mut DecodeVisitor<R>, config: T::Config) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        #[allow(clippy::uninit_assumed_init)]
        let mut array = unsafe { MaybeUninit::<[T; N]>::uninit().assume_init() };

        for elem in &mut array[..] {
            *elem = T::decode_with_config(visitor, config.clone())?;
        }

        Ok(array)
    }

    type Config = T::Config;
}

#[cfg(test)]
mod tests {
    use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
    use test_case::test_case;

    use crate::{float::FloatModel, DecodeVisitor, EncodeVisitor, Encodeable, encodeable_custom::EncodeableCustom};

    #[test_case(Option::Some(true))]
    #[test_case(Option::Some(false))]
    #[test_case(true)]
    #[test_case(false)]
    fn round_trip<T>(input: T)
    where
        T: Encodeable + std::fmt::Debug + PartialEq,
    {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);

        let mut encoder = EncodeVisitor::new(32, &mut bit_writer);

        input.encode(&mut encoder).unwrap();
        encoder.flush().unwrap();
        bit_writer.byte_align().unwrap();
        bit_writer.flush().unwrap();

        let compressed = bit_writer.into_writer();

        let bit_reader = BitReader::endian(compressed.as_slice(), BigEndian);

        let mut decoder = DecodeVisitor::new(32, bit_reader);

        let output = T::decode(&mut decoder).unwrap();

        assert_eq!(input, output);
    }

    #[test_case(Option::Some(true), ())]
    #[test_case(Option::Some(false), ())]
    #[test_case(true, ())]
    #[test_case(false, ())]
    #[test_case(450.0_f64, FloatModel::new(-10000.0..=10000.0, 1))]
    #[test_case(550.0_f64, FloatModel::new(-10000.0..=10000.0, 1))]
    #[test_case(-100.0_f64, FloatModel::new(-5000.0..=0.0, 0))]
    fn round_trip_with_config<T>(input: T, config: T::Config)
    where
        T: EncodeableCustom + std::fmt::Debug + PartialEq,
        T::Config: Clone,
    {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);

        let mut encoder = EncodeVisitor::new(32, &mut bit_writer);

        input.encode_with_config(&mut encoder, config.clone()).unwrap();
        encoder.flush().unwrap();
        bit_writer.byte_align().unwrap();
        bit_writer.flush().unwrap();

        let compressed = bit_writer.into_writer();

        let bit_reader = BitReader::endian(compressed.as_slice(), BigEndian);

        let mut decoder = DecodeVisitor::new(32, bit_reader);

        let output = T::decode_with_config(&mut decoder, config).unwrap();

        assert_eq!(input, output);
    }
}
