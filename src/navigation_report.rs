use std::io;

use bitstream_io::{BitRead, BitWrite};

use crate::{
    float::FloatModel, impls::one_shot::EnumModel, DecodeVisitor, EncodeVisitor, Encodeable,
};

#[derive(Debug)]
pub struct NavigationReport {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vehicle_class: Option<VehicleClass>,
    pub battery_ok: Option<bool>,
}

#[derive(Debug)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

impl Encodeable<()> for VehicleClass {
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>, _config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        let value = match self {
            Self::Auv => 0,
            Self::Usv => 1,
            Self::Ship => 2,
        };
        let model = EnumModel::<3>::default();
        visitor.encode_one(model, &value)
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>, _config: ()) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        let model = EnumModel::<3>::default();
        match visitor.decode_one(model)? {
            0 => Ok(Self::Auv),
            1 => Ok(Self::Usv),
            2 => Ok(Self::Ship),
            _ => unreachable!(),
        }
    }
}

impl Encodeable<()> for NavigationReport {
    fn encode<W>(&self, visitor: &mut EncodeVisitor<W>, _config: ()) -> io::Result<()>
    where
        W: BitWrite,
    {
        self.x
            .encode(visitor, FloatModel::new(-10000.0..=10000.0, 1))?;
        self.y
            .encode(visitor, FloatModel::new(-10000.0..=10000.0, 1))?;
        self.z.encode(visitor, FloatModel::new(-5000.0..=0.0, 0))?;
        self.vehicle_class.encode(visitor, ())?;
        self.battery_ok.encode(visitor, ())?;

        Ok(())
    }

    fn decode<R>(visitor: &mut DecodeVisitor<R>, config: ()) -> io::Result<Self>
    where
        R: BitRead,
        Self: Sized,
    {
        Ok(Self {
            x: f64::decode(visitor, FloatModel::new(-10000.0..=10000.0, 1))?,
            y: f64::decode(visitor, FloatModel::new(-10000.0..=10000.0, 1))?,
            z: f64::decode(visitor, FloatModel::new(-5000.0..=0.0, 0))?,
            vehicle_class: Option::decode(visitor, config)?,
            battery_ok: Option::decode(visitor, config)?,
        })
    }
}
