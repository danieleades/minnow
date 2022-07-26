use std::io;
use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::{encodeable_custom::EncodeableCustom, visitor::{EncodeVisitor, DecodeVisitor}};

pub trait Encodeable {
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> io::Result<()>
    where
        W: BitWrite;

    fn encode_bytes(&self) -> io::Result<Vec<u8>> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(32, &mut bit_writer);

        self.encode(&mut encoder)?;
        encoder.flush()?;
        bit_writer.byte_align()?;
        bit_writer.flush()?;

        Ok(bit_writer.into_writer())
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized;

    fn decode_bytes(bytes: &[u8]) -> io::Result<Self>
    where
        Self: Sized,
    {
        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(32, bit_reader);

        Self::decode(&mut decoder)
    }
}

impl<T, C> Encodeable for T where T: EncodeableCustom<Config = C>, C: Default {
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> io::Result<()>
    where
        W: BitWrite {
            let config = C::default();
        self.encode_with_config(visitor, config)
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized {
            let config = C::default();
        Self::decode_with_config(visitor, config)
    }
}