use std::io;

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::{
    DecodeError, PRECISION,
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
    ///
    /// # Panics
    ///
    /// This method is infallible in practice: writing to a `Vec<u8>` cannot
    /// produce an I/O error. It will only panic if that invariant is somehow
    /// violated.
    fn encode_bytes(&self) -> Vec<u8> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(PRECISION, &mut bit_writer);

        // Writing to a `Vec<u8>` is infallible, so these operations cannot fail.
        self.encode(&mut encoder)
            .expect("writing to Vec<u8> is infallible");
        encoder.flush().expect("writing to Vec<u8> is infallible");
        bit_writer
            .byte_align()
            .expect("writing to Vec<u8> is infallible");
        bit_writer
            .flush()
            .expect("writing to Vec<u8> is infallible");

        bit_writer.into_writer()
    }

    /// Decode the struct using the provided [`DecodeVisitor`].
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] if the underlying reader fails or the stream
    /// is corrupt. Decoding untrusted bytes must never panic.
    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized;

    /// Decode the struct from a `[u8]`.
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] if the bytes are truncated or corrupt. This
    /// method never panics, regardless of the input.
    fn decode_bytes(bytes: &[u8]) -> Result<Self, DecodeError>
    where
        Self: Sized,
    {
        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(PRECISION, bit_reader);

        Self::decode(&mut decoder)
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

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        let config = C::default();
        Self::decode_with_config(visitor, config)
    }
}
