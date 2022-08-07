use std::io;

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::{
    encodeable_custom::EncodeableCustom,
    visitor::{DecodeVisitor, EncodeVisitor},
};

/// Structs that implement [`EncodeableCustom`] can be encoded and decoded.
///
/// For structs that offer configurable encoding/decoding, [`EncodeableCustom`]
/// should be used instead. [`Encodeable`] is automatically derived for structs
/// that implement [`EncodeableCustom`] where the config type implements
/// [`Default`].
pub trait Encodeable {
    /// Encode the struct using the provided [`EncodeVisitor`].
    ///
    /// # Errors
    ///
    /// This method can fail if the [`EncodeVisitor`]'s underlying writer cannot
    /// be written to.
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> io::Result<()>
    where
        W: BitWrite;

    /// Encode the struct into a [`Vec<u8>`].
    fn encode_bytes(&self) -> Vec<u8> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(32, &mut bit_writer);

        self.encode(&mut encoder).unwrap();
        encoder.flush().unwrap();
        bit_writer.byte_align().unwrap();
        bit_writer.flush().unwrap();

        bit_writer.into_writer()
    }

    /// Decode the struct using the provided [`DecodeVisitor`].
    ///
    /// # Errors
    ///
    /// This method can fail if the [`DecodeVisitor`]'s underlying reader cannot
    /// be read from.
    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized;

    /// Decode the struct from a [`[u8]`].
    #[must_use]
    fn decode_bytes(bytes: &[u8]) -> Self
    where
        Self: Sized,
    {
        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(32, bit_reader);

        Self::decode(&mut decoder).unwrap()
    }
}

impl<T, C> Encodeable for T
where
    T: EncodeableCustom<Config = C>,
    C: Default,
{
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> io::Result<()>
    where
        W: BitWrite,
    {
        let config = C::default();
        self.encode_with_config(visitor, config)
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        let config = C::default();
        Self::decode_with_config(visitor, config)
    }
}
