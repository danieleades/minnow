use std::io;

use arithmetic_coding::{encoder::State, fixed_length::Wrapper, one_shot, Encoder};
use bitstream_io::BitWrite;

use crate::float::FloatModel;

pub struct Visitor<'a, W>
where
    W: BitWrite,
{
    state: Option<State<'a, u128, W>>,
}

impl<'a, W> Visitor<'a, W>
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
}

pub trait Encodeable<Config> {
    fn accept<W>(&self, visitor: &mut Visitor<W>, config: Config) -> io::Result<()>
    where
        W: BitWrite;
}

impl<T, M> Encodeable<M> for Option<T>
where
    T: Encodeable<M>,
{
    fn accept<W>(&self, visitor: &mut Visitor<W>, config: M) -> io::Result<()>
    where
        W: BitWrite,
    {
        pub enum Option__ {
            Some,
            None,
        }

        impl Encodeable<()> for Option__ {
            fn accept<W>(&self, visitor: &mut Visitor<W>, _config: ()) -> io::Result<()>
            where
                W: BitWrite,
            {
                todo!()
            }
        }

        match self {
            Some(x) => {
                Option__::Some.accept(visitor, ())?;
                x.accept(visitor, config)
            }
            None => Option__::None.accept(visitor, ()),
        }
    }
}

impl<M> Encodeable<M> for f64
where
    M: one_shot::Model<Symbol = f64, B = u128>,
{
    fn accept<W>(&self, visitor: &mut Visitor<W>, config: M) -> io::Result<()>
    where
        W: BitWrite,
    {
        visitor.encode_one(config, self)
    }
}

impl Encodeable<()> for bool {
    fn accept<W>(&self, visitor: &mut Visitor<W>, config: ()) -> io::Result<()>
    where
        W: BitWrite,
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

impl Encodeable<()> for VehicleClass {
    fn accept<W>(&self, visitor: &mut Visitor<W>, config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        todo!()
    }
}

impl Encodeable<()> for NavigationReport {
    fn accept<W>(&self, visitor: &mut Visitor<W>, _config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        self.x.accept(visitor, FloatModel)?;
        self.y.accept(visitor, FloatModel)?;
        self.z.accept(visitor, FloatModel)?;
        self.vehicle_class.accept(visitor, ())?;
        self.battery_ok.accept(visitor, ())?;

        Ok(())
    }
}
