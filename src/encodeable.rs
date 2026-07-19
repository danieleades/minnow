//! The core codec trait: [`Encodeable`].
//!
//! `Encodeable` is deliberately *silent about size*: it says a type can be
//! encoded and decoded, nothing more. The static size guarantees — weights,
//! worst-case bits, size reports, and the pre-decode length window — live on
//! the [`Bounded`] subtrait (see `src/bounded.rs`), so that models with no
//! finite worst case (open-ended varints, unbounded sequences, adaptive
//! models) are expressible without weakening the budgets of those that have
//! one.
//!
//! The split follows the standard library's `Iterator` /
//! `ExactSizeIterator` / `DoubleEndedIterator` pattern: one core trait, with
//! convenience methods provided directly on it but gated by `where` clauses
//! (`Self::Config: Default` for the default-config entry points, `Self:
//! Bounded` for the length-validated ones).

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::{
    Bounded, DecodeError, EncodeError, PRECISION,
    visitor::{DecodeVisitor, EncodeVisitor},
};

/// A type that can be encoded to, and decoded from, an arithmetic-coded
/// stream.
///
/// This is the codec tier: implementing it says nothing about *how large* the
/// encoding can get. Types whose encoded size has a finite worst case should
/// additionally implement [`Bounded`], which unlocks the size-reporting
/// methods and the length-validated [`decode_bytes`](Encodeable::decode_bytes)
/// entry points. A schema built from bounded parts is bounded; a schema
/// containing even one unbounded field is not, and the type system tracks
/// that distinction automatically for derived types.
///
/// # Configuration
///
/// Every codec is parameterised by a [`Config`](Encodeable::Config) — the
/// model driving the arithmetic coder (value ranges, precisions, length
/// bounds, …). Types with a canonical model implement `Default` on their
/// config, which enables the convenience methods without the `_with_config`
/// suffix; `#[derive(Encodeable)]` uses `Config = ()`.
pub trait Encodeable {
    /// The type of the configuration used to customise the encoding/decoding.
    type Config;

    /// Encode the value using the provided configuration and
    /// [`EncodeVisitor`].
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] if a value lies outside its model's configured
    /// domain (see [`EncodeError::OutOfRange`] and the saturation opt-in
    /// documented there), or if the underlying writer cannot be written to.
    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: Self::Config,
    ) -> Result<(), EncodeError>
    where
        W: BitWrite;

    /// Decode a value using the provided configuration and [`DecodeVisitor`].
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] if the underlying reader fails or the stream
    /// is corrupt. Decoding untrusted bytes must never panic.
    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: Self::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized;

    /// Encode the value into a [`Vec<u8>`] using the provided configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] if a value lies outside its model's configured
    /// domain. Writing to the `Vec<u8>` itself cannot fail.
    fn encode_bytes_with_config(&self, config: Self::Config) -> Result<Vec<u8>, EncodeError> {
        let mut bit_writer = BitWriter::endian(Vec::new(), BigEndian);
        let mut encoder = EncodeVisitor::new(PRECISION, &mut bit_writer);

        self.encode_with_config(&mut encoder, config)?;
        encoder.flush()?;
        bit_writer.byte_align()?;
        bit_writer.flush()?;

        Ok(bit_writer.into_writer())
    }

    /// Encode the value using its [`Default`] configuration and the provided
    /// [`EncodeVisitor`].
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] if a value lies outside its model's configured
    /// domain, or if the underlying writer cannot be written to.
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>) -> Result<(), EncodeError>
    where
        W: BitWrite,
        Self::Config: Default,
    {
        self.encode_with_config(visitor, Self::Config::default())
    }

    /// Encode the value into a [`Vec<u8>`] using its [`Default`]
    /// configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] if a value lies outside its model's configured
    /// domain. Writing to the `Vec<u8>` itself cannot fail.
    fn encode_bytes(&self) -> Result<Vec<u8>, EncodeError>
    where
        Self::Config: Default,
    {
        self.encode_bytes_with_config(Self::Config::default())
    }

    /// Decode a value using its [`Default`] configuration and the provided
    /// [`DecodeVisitor`].
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] if the underlying reader fails or the stream
    /// is corrupt. This method never panics, regardless of the input.
    fn decode<R>(visitor: &mut DecodeVisitor<R>) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
        Self::Config: Default,
    {
        Self::decode_with_config(visitor, Self::Config::default())
    }

    /// Decode a value from a `[u8]` using the provided configuration, with
    /// the input length validated against the schema *before* decoding.
    ///
    /// Every value of a [`Bounded`] schema encodes to a length within a
    /// provable window (see [`DecodeError::Length`]); input outside that
    /// window is rejected without the arithmetic decoder ever running. This
    /// catches truncation, which the decoder would otherwise mask by
    /// zero-padding the exhausted stream. The check is what makes this method
    /// a `Bounded` privilege — an unbounded schema has no window to check, and
    /// must use
    /// [`decode_bytes_unvalidated_with_config`](Encodeable::decode_bytes_unvalidated_with_config)
    /// instead.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Length`] if the input cannot be a valid encoding
    /// of this schema, [`DecodeError::Io`] if the reader fails, or
    /// [`DecodeError::InvalidSymbol`] if a decoded symbol falls outside its
    /// model. A length-valid but internally corrupt stream may still decode to
    /// `Ok` with a wrong value — see [`decode_bytes`](Encodeable::decode_bytes)
    /// for the residual integrity caveats. This method never panics,
    /// regardless of the input.
    fn decode_bytes_with_config(bytes: &[u8], config: Self::Config) -> Result<Self, DecodeError>
    where
        Self: Sized + Bounded,
    {
        // Every value of the schema encodes to a length in
        // `[ceil(best_case_bits / 8), total_bytes]`. For uniform (automatic)
        // weighting the two ends coincide, pinning the length exactly and
        // catching truncation; manual `#[encode(weight)]` overrides widen the
        // window but it still never rejects a genuinely valid encoding.
        // Computed from the scalar bit bounds directly — building the full
        // `SizeReport` tree here would be wasted work on every decode.
        let expected =
            crate::report::bytes_for(Self::worst_case_bits(&config) + crate::TERMINATION_BITS);
        let lower = crate::report::bytes_for(Self::best_case_bits(&config));
        let actual = bytes.len();
        if actual < lower || actual > expected {
            return Err(DecodeError::Length { expected, actual });
        }

        Self::decode_bytes_unvalidated_with_config(bytes, config)
    }

    /// Decode a value from a `[u8]` using its [`Default`] configuration, with
    /// the input length validated against the schema *before* decoding.
    ///
    /// See
    /// [`decode_bytes_with_config`](Encodeable::decode_bytes_with_config) for
    /// the validation this performs, and why it requires [`Bounded`].
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Length`] if the input cannot be a valid
    /// encoding of this schema, or any other [`DecodeError`] the decoder
    /// produces. This method never panics, regardless of the input.
    ///
    /// # Integrity
    ///
    /// The length window rejects truncated input, but arithmetic-coded
    /// streams are not self-delimiting: almost every byte string of a valid
    /// length decodes to *some* value, so a corrupt-but-correctly-sized
    /// stream can decode to `Ok` with a wrong value. Callers that need full
    /// integrity must add outer framing (a checksum) around the encoded
    /// bytes.
    fn decode_bytes(bytes: &[u8]) -> Result<Self, DecodeError>
    where
        Self: Sized + Bounded,
        Self::Config: Default,
    {
        Self::decode_bytes_with_config(bytes, Self::Config::default())
    }

    /// Decode a value from a `[u8]` using the provided configuration,
    /// **without** validating the input length first.
    ///
    /// This is the from-bytes entry point for schemas that do not implement
    /// [`Bounded`] — with no finite worst case there is no length window to
    /// check. Bounded schemas may also call it to opt out of the window.
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] if the underlying reader fails or a decoded
    /// symbol falls outside its model. This method never panics, regardless
    /// of the input.
    ///
    /// # Integrity
    ///
    /// Without the length window, truncation is *not* reliably detected: the
    /// decoder treats missing bits as zeros (legitimate streams end in
    /// implicit zero padding), so truncated input may decode to a wrong
    /// value. Callers must add outer framing (an explicit length and/or a
    /// checksum) around the encoded bytes.
    fn decode_bytes_unvalidated_with_config(
        bytes: &[u8],
        config: Self::Config,
    ) -> Result<Self, DecodeError>
    where
        Self: Sized,
    {
        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(PRECISION, bit_reader);

        Self::decode_with_config(&mut decoder, config)
    }

    /// Decode a value from a `[u8]` using its [`Default`] configuration,
    /// **without** validating the input length first.
    ///
    /// See
    /// [`decode_bytes_unvalidated_with_config`](Encodeable::decode_bytes_unvalidated_with_config)
    /// for when this is appropriate and its integrity caveats.
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] if the underlying reader fails or a decoded
    /// symbol falls outside its model. This method never panics, regardless
    /// of the input.
    fn decode_bytes_unvalidated(bytes: &[u8]) -> Result<Self, DecodeError>
    where
        Self: Sized,
        Self::Config: Default,
    {
        Self::decode_bytes_unvalidated_with_config(bytes, Self::Config::default())
    }
}
