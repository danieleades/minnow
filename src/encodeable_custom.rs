use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

use crate::{
    DecodeError, EncodeError, PRECISION, SizeReport, Weight,
    visitor::{DecodeVisitor, EncodeVisitor},
};

/// Structs that implement [`EncodeableCustom`] can be encoded and decoded using
/// custom configuration.
///
/// For structs that do not offer configurable encoding/decoding,
/// [`Encodeable`](crate::Encodeable) should be used instead.
/// [`Encodeable`](crate::Encodeable) is automatically derived for structs that
/// implement [`EncodeableCustom`] where the config type implements [`Default`].
pub trait EncodeableCustom {
    /// The type of the configuration used to customise the encoding/decoding.
    type Config;

    /// The number of distinct values this type can encode under `config` — its
    /// [`Weight`].
    ///
    /// This is the cardinality of the value set: the product of field weights
    /// for structs/arrays, the sum of variant weights for enums/[`Option`], and
    /// the model denominator for leaves. See [`Weight`] for the semiring this
    /// belongs to.
    fn weight(config: &Self::Config) -> Weight;

    /// The worst-case number of bits needed to encode any value of this type
    /// under `config`.
    ///
    /// The default is `log₂` of the [`weight`](EncodeableCustom::weight), which
    /// is exact for uniform leaves and for sums/products whose weight has not
    /// saturated. Containers override this to accumulate `f64` bit counts
    /// directly (sum over fields; max over enum variants), which stays accurate
    /// even when the weight product saturates — see [`crate::SizeReport`].
    fn worst_case_bits(config: &Self::Config) -> f64 {
        Self::weight(config).log2()
    }

    /// The *best*-case number of bits — the size of the cheapest value of this
    /// type under `config`.
    ///
    /// The dual of [`worst_case_bits`](EncodeableCustom::worst_case_bits): a
    /// sum over fields but a **min** over enum variants. For uniform
    /// (automatic) weighting every value costs the same, so this equals
    /// `worst_case_bits`; with manual `#[encode(weight = …)]` overrides the
    /// cheapest and dearest values differ, and this is the lower one.
    ///
    /// It is used to bound the encoded length from below when validating input
    /// on decode (see [`crate::DecodeError::Length`]); the default matches the
    /// uniform-leaf case.
    fn best_case_bits(config: &Self::Config) -> f64 {
        Self::weight(config).log2()
    }

    /// A [`SizeReport`] tree describing the worst-case encoded size of this
    /// type under `config`.
    ///
    /// The default is an unnamed leaf carrying
    /// [`worst_case_bits`](EncodeableCustom::worst_case_bits); containers
    /// override it to expose a per-field / per-variant breakdown.
    fn report(config: &Self::Config) -> SizeReport {
        SizeReport::leaf(Self::worst_case_bits(config))
    }

    /// Encode the struct using the provided configuration and
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

    /// Encode the struct into a [`Vec<u8>`] using the provided configuration.
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

    /// Decode the struct using the provided configuration and
    /// [`DecodeVisitor`].
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

    /// Decode the struct from a `[u8]` using the provided configuration.
    ///
    /// The input length is validated against the schema *before* decoding: a
    /// slice outside the schema's length window (see
    /// [`DecodeError::Length`]) is rejected without the arithmetic decoder ever
    /// running. This catches truncation, which the decoder would otherwise mask
    /// by zero-padding the exhausted stream.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Length`] if the input cannot be a valid encoding
    /// of this schema, [`DecodeError::Io`] if the reader fails, or
    /// [`DecodeError::InvalidSymbol`] if a decoded symbol falls outside its
    /// model. A length-valid but internally corrupt stream may still decode to
    /// `Ok` with a wrong value — see
    /// [`Encodeable::decode_bytes`](crate::Encodeable::decode_bytes) for the
    /// residual integrity caveats. This method never panics, regardless of the
    /// input.
    fn decode_bytes_with_config(bytes: &[u8], config: Self::Config) -> Result<Self, DecodeError>
    where
        Self: Sized,
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

        let bit_reader = BitReader::endian(bytes, BigEndian);
        let mut decoder = DecodeVisitor::new(PRECISION, bit_reader);

        Self::decode_with_config(&mut decoder, config)
    }
}
