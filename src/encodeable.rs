use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::{
    DecodeError, EncodeError, PRECISION, SizeReport,
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
    /// Returns [`EncodeError`] if a value lies outside its model's configured
    /// domain, or if the underlying writer cannot be written to.
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> Result<(), EncodeError>
    where
        W: BitWrite;

    /// The worst-case number of bits needed to encode any value of this type,
    /// using its [`Default`] configuration.
    ///
    /// Under Minnow's uniform weighting this is the *exact* fractional size of
    /// every value; see [`crate::SizeReport`].
    ///
    /// The default implementation reads it off
    /// [`size_report`](Encodeable::size_report); the blanket implementation
    /// over [`EncodeableCustom`] overrides both.
    #[must_use]
    fn worst_case_bits() -> f64
    where
        Self: Sized,
    {
        Self::size_report().total_bits()
    }

    /// A [`SizeReport`] describing the worst-case encoded size of this type,
    /// using its [`Default`] configuration.
    ///
    /// This method is deliberately *required*: a default (such as an empty
    /// report) would silently claim "zero bits" for any hand-written impl that
    /// forgot to override it, poisoning capacity planning. The blanket
    /// implementation over [`EncodeableCustom`] provides the real per-field
    /// breakdown for derived types; hand-written impls must state their own
    /// (see `examples/struct_enum.rs`).
    #[must_use]
    fn size_report() -> SizeReport
    where
        Self: Sized;

    /// Encode the struct into a [`Vec<u8>`].
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] if a value lies outside its model's configured
    /// domain. Writing to the `Vec<u8>` itself cannot fail.
    fn encode_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(PRECISION, &mut bit_writer);

        self.encode(&mut encoder)?;
        encoder.flush()?;
        bit_writer.byte_align()?;
        bit_writer.flush()?;

        Ok(bit_writer.into_writer())
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
    /// Returns a [`DecodeError`] if the underlying reader fails or a decoded
    /// symbol falls outside its model. This method never panics, regardless
    /// of the input.
    ///
    /// # Integrity
    ///
    /// The blanket implementation over [`EncodeableCustom`] validates the input
    /// length against the schema before decoding (returning
    /// [`DecodeError::Length`] on a mismatch), which rejects truncated input
    /// that the arithmetic decoder would otherwise mask by zero-padding. This
    /// default implementation — used only by hand-written [`Encodeable`] impls
    /// — does **not**: arithmetic-coded streams are not self-delimiting, so
    /// almost every byte string of a plausible length decodes to *some* valid
    /// value. Even with the length check, a corrupt-but-correctly-sized stream
    /// can decode to `Ok` with a wrong value. Callers that need full integrity
    /// must add outer framing (a checksum) around the encoded bytes.
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
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> Result<(), EncodeError>
    where
        W: BitWrite,
    {
        let config = C::default();
        self.encode_with_config(visitor, config)
    }

    fn worst_case_bits() -> f64 {
        <T as EncodeableCustom>::worst_case_bits(&C::default())
    }

    fn size_report() -> SizeReport {
        <T as EncodeableCustom>::report(&C::default())
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        let config = C::default();
        Self::decode_with_config(visitor, config)
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        // Route through the config-aware path so the up-front schema length
        // check (see `DecodeError::Length`) applies to derived types too.
        Self::decode_bytes_with_config(bytes, C::default())
    }
}
