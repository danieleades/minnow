use std::io;

use arithmetic_coding::{
    decoder, encoder::State, fixed_length::Wrapper, one_shot, Decoder, Encoder,
};
use bitstream_io::{BitRead, BitWrite};

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
    /// This will generally by infallible, as in normal use the writer is a
    /// `Vec<u8>`.
    pub fn encode_one<M>(&mut self, model: M, value: &M::Symbol) -> io::Result<()>
    where
        M: one_shot::Model<B = u128, ValueError = !>,
    {
        #![allow(clippy::missing_panics_doc)]
        let mut encoder = Encoder::with_state(self.state.take().unwrap(), Wrapper::new(model));
        encoder.encode(Some(value)).unwrap();
        let (_model, state) = encoder.into_inner();
        self.state = Some(state);
        Ok(())
    }

    /// Flush the internal buffer of the [`EncodeVisitor`].
    ///
    /// It is necessary to flush the visitor once, after all fields have been
    /// encoded.
    ///
    /// # Errors
    ///
    /// This method can fail if the underlying writer cannot be written to.
    pub fn flush(&mut self) -> io::Result<()> {
        #![allow(clippy::missing_panics_doc)]
        self.state.as_mut().unwrap().flush()
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
    /// This method can fail if the underlying reader cannot be read from.
    pub fn decode_one<M>(&mut self, model: M) -> io::Result<M::Symbol>
    where
        M: one_shot::Model<B = u128>,
    {
        let mut decoder = Decoder::with_state(self.state.take().unwrap(), Wrapper::new(model));
        let symbol = decoder.decode().unwrap().unwrap();
        let (_model, state) = decoder.into_inner();
        self.state = Some(state);
        Ok(symbol)
    }
}
