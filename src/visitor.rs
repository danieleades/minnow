use std::io;

use arithmetic_coding::{Decoder, Encoder, decoder, encoder::State, one_shot};
use bitstream_io::{BitRead, BitWrite};

use crate::DecodeError;

/// A visitor that encodes the fields of a struct into a writer
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct EncodeVisitor<'a, W>
where
    W: BitWrite,
{
    state: Option<State<'a, u128, W>>,
}

impl<'a, W> EncodeVisitor<'a, W>
where
    W: BitWrite,
{
    /// Create a new [`EncodeVisitor`].
    pub fn new(precision: u32, writer: &'a mut W) -> Self {
        Self {
            state: Some(State::new(precision, writer)),
        }
    }

    /// Encode a single symbol.
    ///
    /// # Errors
    ///
    /// This method can fail if the underlying writer cannot be written to.
    /// This will generally be infallible, as in normal use the writer is a
    /// `Vec<u8>`.
    ///
    /// # Panics
    ///
    /// Panics if called after the internal state has been consumed, which
    /// cannot happen through the public API.
    pub fn encode_one<M>(&mut self, model: M, value: &M::Symbol) -> io::Result<()>
    where
        M: one_shot::Model<B = u128>,
    {
        let state = self.state.take().expect("encoder state is always present");
        let mut encoder = Encoder::with_state(state, one_shot::Wrapper::new(model));
        let result = encoder.encode(Some(value));
        let (_model, state) = encoder.into_inner();
        self.state = Some(state);

        match result {
            Ok(()) => Ok(()),
            Err(arithmetic_coding::Error::Io(e)) => Err(e),
            // A `ValueError` here would mean the caller asked to encode a symbol
            // the model rejects, which is a programming error rather than an I/O
            // failure. Surface it as invalid data rather than panicking.
            Err(arithmetic_coding::Error::ValueError(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "attempted to encode a symbol that is invalid for its model",
            )),
        }
    }

    /// Flush the internal buffer of the [`EncodeVisitor`].
    ///
    /// It is necessary to flush the visitor once, after all fields have been
    /// encoded.
    ///
    /// # Errors
    ///
    /// This method can fail if the underlying writer cannot be written to.
    ///
    /// # Panics
    ///
    /// Panics if called after the internal state has been consumed, which
    /// cannot happen through the public API.
    pub fn flush(&mut self) -> io::Result<()> {
        self.state
            .as_mut()
            .expect("encoder state is always present")
            .flush()
    }
}

/// A visitor that decodes the fields of a struct from a reader
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct DecodeVisitor<R>
where
    R: BitRead,
{
    state: Option<decoder::State<u128, R>>,
}

impl<R> DecodeVisitor<R>
where
    R: BitRead,
{
    /// Create a new [`DecodeVisitor`].
    pub fn new(precision: u32, reader: R) -> Self {
        Self {
            state: Some(decoder::State::new(precision, reader)),
        }
    }

    /// Decode a single symbol.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Io`] if the underlying reader fails, or
    /// [`DecodeError::InvalidSymbol`] if the decoder yields no symbol.
    ///
    /// Note that a truncated stream does *not* reliably produce an error: the
    /// decoder treats missing bits as zeros (legitimate streams end in
    /// implicit zero padding), so truncated input may decode to a wrong value.
    /// Integrity requires outer framing; see
    /// [`Encodeable::decode_bytes`](crate::Encodeable::decode_bytes).
    ///
    /// # Panics
    ///
    /// Panics if called after the internal state has been consumed, which
    /// cannot happen through the public API.
    pub fn decode_one<M>(&mut self, model: M) -> Result<M::Symbol, DecodeError>
    where
        M: one_shot::Model<B = u128>,
    {
        let state = self.state.take().expect("decoder state is always present");
        let mut decoder = Decoder::with_state(state, one_shot::Wrapper::new(model));
        let result = decoder.decode();
        let (_model, state) = decoder.into_inner();
        self.state = Some(state);

        // A one-shot wrapper always yields exactly one symbol; `None` here means
        // the stream was exhausted before that symbol could be produced.
        result?.ok_or(DecodeError::InvalidSymbol { symbol: 0 })
    }
}
