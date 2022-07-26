use std::io;
use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::visitor::{EncodeVisitor, DecodeVisitor};

pub trait EncodeableCustom {
    type Config;
    fn encode_with_config<W>(&self, visitor: &mut EncodeVisitor<W>, config: Self::Config) -> io::Result<()>
    where
        W: BitWrite;

    fn encode_bytes_with_config(&self, config: Self::Config) -> io::Result<Vec<u8>> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(32, &mut bit_writer);

        self.encode_with_config(&mut encoder, config)?;
        encoder.flush()?;
        bit_writer.byte_align()?;
        bit_writer.flush()?;

        Ok(bit_writer.into_writer())
    }

    fn decode_with_config<R>(visitor: &mut DecodeVisitor<R>, config: Self::Config) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized;

    fn decode_bytes_with_config(bytes: &[u8], config: Self::Config) -> io::Result<Self>
    where
        Self: Sized,
    {
        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(32, bit_reader);

        Self::decode_with_config(&mut decoder, config)
    }
}
