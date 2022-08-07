use std::io;

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::visitor::{DecodeVisitor, EncodeVisitor};

/// Structs that implement [`EncodeableCustom`] can be encoded and decoded using
/// custom configuration.
///
/// For structs that do not offer configurable encoding/decoding, [`Encodeable`]
/// should be used instead. [`Encodeable`] is automatically derived for structs
/// that implement [`EncodeableCustom`] where the config type implements
/// [`Default`].
pub trait EncodeableCustom {
    /// The type of the configuration used to customise the encoding/decoding.
    type Config;

    /// Encode the struct using the provided configuration and
    /// [`EncodeVisitor`].
    ///
    /// # Errors
    ///
    /// This method can fail if the [`EncodeVisitor`]'s underlying writer cannot
    /// be written to.
    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: Self::Config,
    ) -> io::Result<()>
    where
        W: BitWrite;

    /// Encode the struct into a [`Vec<u8>`] using the provided configuration.
    fn encode_bytes_with_config(&self, config: Self::Config) -> Vec<u8> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(32, &mut bit_writer);

        self.encode_with_config(&mut encoder, config)
            .expect("can't get io errors when writing to Vec<u8>");
        encoder
            .flush()
            .expect("can't get io errors when writing to Vec<u8>");
        bit_writer
            .byte_align()
            .expect("can't get io errors when writing to Vec<u8>");
        bit_writer
            .flush()
            .expect("can't get io errors when writing to Vec<u8>");

        bit_writer.into_writer()
    }

    /// Decode the struct using the provided configuration and
    /// [`DecodeVisitor`].
    ///
    /// # Errors
    ///
    /// This method can fail if the [`DecodeVisitor`]'s underlying reader cannot
    /// be read from.
    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: Self::Config,
    ) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized;

    /// Decode the struct from a [`[u8]`] using the provided configuration.
    fn decode_bytes_with_config(bytes: &[u8], config: Self::Config) -> io::Result<Self>
    where
        Self: Sized,
    {
        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(32, bit_reader);

        Self::decode_with_config(&mut decoder, config)
    }
}
