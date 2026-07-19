//! A model for `String` (issue #3): a bounded sequence of UTF-8 bytes.

use bitstream_io::{BitRead, BitWrite};

use crate::{
    DecodeError, DecodeVisitor, EncodeError, EncodeVisitor, EncodeableCustom, IntModel, ModelError,
    SeqModel, SizeReport, Weight,
};

/// The configuration for a [`String`] field: a maximum length **in bytes**.
///
/// `String` is modelled as a bounded sequence of bytes (see
/// [`crate::SeqModel`]), each byte uniform over all 256 values — i.e.
/// `StringModel` is exactly `SeqModel<IntModel<u8>>` under the hood, with the
/// byte model fixed to its full-range [`Default`]. Restricting the alphabet
/// (e.g. printable ASCII, à la DCCL) would shrink the per-byte cost below 8
/// bits, but is not implemented here — a documented follow-up.
///
/// Every value round-trips as bytes; not every byte sequence is valid UTF-8,
/// so decoding a corrupt stream can fail with
/// [`DecodeError::InvalidUtf8`](crate::DecodeError::InvalidUtf8) rather than
/// panicking.
#[derive(Debug, Clone, Copy)]
pub struct StringModel {
    bytes: SeqModel<IntModel<u8>>,
}

impl StringModel {
    /// Create a new [`StringModel`] with the given maximum length, in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::DenominatorTooLarge`] if `max_length` does not
    /// fit in a `u32` (the length prefix's underlying representation — see
    /// [`SeqModel::max_len`](crate::SeqModel)); a `u32`-bounded length always
    /// stays comfortably within
    /// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR), so this is the only way
    /// construction can fail.
    pub fn new(max_length: usize) -> Result<Self, ModelError> {
        let max_len = u32::try_from(max_length).map_err(|_| ModelError::DenominatorTooLarge {
            denominator: u128::try_from(max_length).unwrap_or(u128::MAX),
            max: u128::from(u32::MAX),
            precision: crate::PRECISION,
        })?;
        Ok(Self {
            bytes: SeqModel {
                max_len,
                elem: IntModel::<u8>::default(),
            },
        })
    }

    /// The maximum length, in bytes, a [`String`] configured with this model
    /// may occupy.
    #[must_use]
    pub fn max_length(&self) -> usize {
        self.bytes.max_len as usize
    }
}

impl EncodeableCustom for String {
    type Config = StringModel;

    fn weight(config: &Self::Config) -> Weight {
        <Vec<u8> as EncodeableCustom>::weight(&config.bytes)
    }

    fn worst_case_bits(config: &Self::Config) -> f64 {
        <Vec<u8> as EncodeableCustom>::worst_case_bits(&config.bytes)
    }

    fn best_case_bits(config: &Self::Config) -> f64 {
        <Vec<u8> as EncodeableCustom>::best_case_bits(&config.bytes)
    }

    fn report(config: &Self::Config) -> SizeReport {
        <Vec<u8> as EncodeableCustom>::report(&config.bytes)
    }

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: Self::Config,
    ) -> Result<(), EncodeError>
    where
        W: BitWrite,
    {
        let bytes = self.as_bytes().to_vec();
        bytes.encode_with_config(visitor, config.bytes)
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: Self::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        let bytes = <Vec<u8> as EncodeableCustom>::decode_with_config(visitor, config.bytes)?;
        String::from_utf8(bytes).map_err(DecodeError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::StringModel;
    use crate::{DecodeError, EncodeableCustom};

    #[test]
    fn max_length_round_trips() {
        let model = StringModel::new(100).unwrap();
        assert_eq!(model.max_length(), 100);
    }

    #[test]
    fn round_trips_ascii_and_multibyte() {
        for s in [
            "",
            "hello",
            "héllo wörld",
            "😀 emoji",
            "the quick brown fox",
        ] {
            let config = StringModel::new(64).unwrap();
            let bytes = s.to_string().encode_bytes_with_config(config).unwrap();
            let decoded = String::decode_bytes_with_config(&bytes, config).unwrap();
            assert_eq!(decoded, s);
        }
    }

    #[test]
    fn empty_and_max_length_round_trip() {
        let config = StringModel::new(5).unwrap();

        let empty = String::new();
        let bytes = empty.encode_bytes_with_config(config).unwrap();
        assert_eq!(
            String::decode_bytes_with_config(&bytes, config).unwrap(),
            empty
        );

        let max = "abcde".to_string();
        let bytes = max.encode_bytes_with_config(config).unwrap();
        assert_eq!(
            String::decode_bytes_with_config(&bytes, config).unwrap(),
            max
        );
    }

    #[test]
    fn encoding_over_max_length_is_rejected() {
        let config = StringModel::new(2).unwrap();
        let too_long = "abc".to_string();
        let mut writer = bitstream_io::BitWriter::endian(Vec::new(), bitstream_io::BigEndian);
        let mut encoder = crate::EncodeVisitor::new(crate::PRECISION, &mut writer);
        let err = too_long
            .encode_with_config(&mut encoder, config)
            .unwrap_err();
        assert!(matches!(err, crate::EncodeError::TooLong { .. }));
    }

    #[test]
    fn invalid_utf8_bytes_are_rejected_not_panicked() {
        // Encode a raw `Vec<u8>` containing an invalid UTF-8 sequence (a lone
        // continuation byte) using the same byte model `StringModel` uses
        // internally, then decode it as a `String` and confirm it errors
        // cleanly.
        let config = StringModel::new(4).unwrap();
        let invalid_bytes: Vec<u8> = vec![0xff, 0xfe];
        let bytes = invalid_bytes
            .encode_bytes_with_config(crate::SeqModel {
                max_len: u32::try_from(config.max_length()).unwrap(),
                elem: crate::IntModel::<u8>::default(),
            })
            .unwrap();

        let err = String::decode_bytes_with_config(&bytes, config).unwrap_err();
        assert!(matches!(err, DecodeError::InvalidUtf8(_)));
    }

    #[test]
    fn rejects_max_length_over_u32() {
        let err = StringModel::new(usize::MAX).unwrap_err();
        assert!(matches!(err, crate::ModelError::DenominatorTooLarge { .. }));
    }
}
