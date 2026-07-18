//! Error types produced while decoding.

/// An error that can occur while decoding a value from a stream of bits.
///
/// Decoding operates on untrusted input, so it must never panic. Any problem
/// — a truncated or corrupt stream — is surfaced as one of these variants
/// instead.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The underlying reader could not be read from.
    #[error("i/o error while decoding: {0}")]
    Io(#[from] std::io::Error),

    /// The stream decoded to a symbol outside the range of valid symbols for
    /// the type being decoded.
    ///
    /// This indicates the input is corrupt (for example, a discriminant that
    /// does not correspond to any enum variant).
    #[error("decoded an out-of-range symbol ({symbol}); the input stream is corrupt")]
    InvalidSymbol {
        /// The offending symbol value.
        symbol: u128,
    },

    /// The input length is impossible for the schema.
    ///
    /// Under Minnow's uniform (automatic) weighting every value of a schema
    /// encodes to the same number of bytes, so the valid length is pinned to a
    /// window at most one byte wide — from `ceil(best_case_bits / 8)` up to
    /// [`SizeReport::total_bytes`]. An input outside that window is truncated,
    /// padded, or belongs to a different schema. Length is validated up front
    /// (before decoding) because the arithmetic decoder silently zero-pads
    /// exhausted input, which would otherwise let a truncated message decode to
    /// a plausible-but-wrong value.
    ///
    /// Manual `#[encode(weight = …)]` overrides make weighting non-uniform, so
    /// values vary in length and the window widens accordingly; the check still
    /// never rejects a genuinely valid encoding.
    ///
    /// [`SizeReport::total_bytes`]: crate::SizeReport::total_bytes
    #[error("input is {actual} bytes but the schema requires at most {expected}")]
    Length {
        /// The maximum number of bytes a value of this schema can occupy
        /// ([`SizeReport::total_bytes`](crate::SizeReport::total_bytes)).
        expected: usize,
        /// The number of bytes actually supplied.
        actual: usize,
    },

    /// The decoded byte sequence for a [`String`] field was not valid UTF-8.
    ///
    /// [`String`]'s wire format (see [`crate::StringModel`]) is a bounded
    /// sequence of raw bytes; any byte value round-trips, but not every byte
    /// sequence is valid UTF-8. A corrupt or malicious stream can decode to a
    /// byte sequence that isn't, which this variant reports rather than
    /// panicking on the `unwrap` a naive implementation might reach for.
    #[error("decoded bytes are not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}
