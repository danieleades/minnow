use std::io;

use arithmetic_coding::{decoder::State, fixed_length::Wrapper, one_shot, Decoder};
use bitstream_io::BitRead;

use crate::float::FloatModel;

pub struct Visitor<R>
where
    R: BitRead,
{
    state: Option<State<u128, R>>,
}

impl<R> Visitor<R>
where
    R: BitRead,
{
    pub fn new(precision: u32, reader: R) -> Self {
        Self {
            state: Some(State::new(precision, reader)),
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

pub trait Decodeable<Config> {
    fn decode<R>(visitor: &mut Visitor<R>, config: Config) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized;
}

impl<T, M> Decodeable<M> for Option<T>
where
    T: Decodeable<M>,
{
    fn decode<R>(visitor: &mut Visitor<R>, config: M) -> io::Result<Self>
    where
        R: BitRead,
    {
        pub enum Option__ {
            Some,
            None,
        }

        impl Decodeable<()> for Option__ {
            fn decode<R>(visitor: &mut Visitor<R>, config: ()) -> io::Result<Self>
            where
                R: BitRead,
                Self: Sized,
            {
                todo!()
            }
        }

        match Option__::decode(visitor, ())? {
            Option__::Some => Ok(Some(T::decode(visitor, config)?)),
            Option__::None => Ok(Option::None),
        }
    }
}

impl<M> Decodeable<M> for f64
where
    M: one_shot::Model<Symbol = f64, B = u128>,
{
    fn decode<R>(visitor: &mut Visitor<R>, config: M) -> io::Result<Self>
    where
        R: BitRead,
    {
        visitor.decode_one(config)
    }
}

impl Decodeable<()> for bool {
    fn decode<R>(visitor: &mut Visitor<R>, config: ()) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        todo!()
    }
}

pub struct NavigationReport {
    x: f64,
    y: f64,
    z: f64,
    vehicle_class: Option<VehicleClass>,
    battery_ok: Option<bool>,
}

pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

impl Decodeable<()> for VehicleClass {
    fn decode<R>(visitor: &mut Visitor<R>, config: ()) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        todo!()
    }
}

impl Decodeable<()> for NavigationReport {
    fn decode<R>(visitor: &mut Visitor<R>, config: ()) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        Ok(Self {
            x: f64::decode(visitor, FloatModel)?,
            y: f64::decode(visitor, FloatModel)?,
            z: f64::decode(visitor, FloatModel)?,
            vehicle_class: Option::decode(visitor, config)?,
            battery_ok: Option::decode(visitor, config)?,
        })
    }
}
