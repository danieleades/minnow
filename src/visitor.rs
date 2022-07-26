use std::io;

use arithmetic_coding::{
    decoder, encoder::State, fixed_length::Wrapper, one_shot, Decoder, Encoder,
};
use bitstream_io::{BitRead, BitWrite};

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
    pub fn new(precision: u32, writer: &'a mut W) -> Self {
        Self {
            state: Some(State::new(precision, writer)),
        }
    }

    pub fn encode_one<M>(&mut self, model: M, value: &M::Symbol) -> io::Result<()>
    where
        M: one_shot::Model<B = u128>,
    {
        let mut encoder = Encoder::with_state(self.state.take().unwrap(), Wrapper::new(model));
        encoder.encode(Some(value)).unwrap();
        let (_model, state) = encoder.into_inner();
        self.state = Some(state);
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.state.as_mut().unwrap().flush()
    }
}

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
    pub fn new(precision: u32, reader: R) -> Self {
        Self {
            state: Some(decoder::State::new(precision, reader)),
        }
    }

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