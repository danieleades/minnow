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
}
