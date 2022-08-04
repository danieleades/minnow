use minnow::{Encodeable, EncodeableCustom};

#[derive(Debug)]
pub enum MyEnum {
    //#[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    A {
        //#[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
        x: f64,
        //#[encode(float(min = 0.0, max = 5_000.0, precision = 0))]
        y: f64,
    },
    B,
}

impl Encodeable for MyEnum {
    fn encode<W>(&self, visitor: &mut minnow::EncodeVisitor<W>) -> std::io::Result<()>
    where
        W: bitstream_io::BitWrite,
    {
        let model = minnow::OneShot::<2>::default();
        match self {
            MyEnum::A { x, y } => {
                visitor.encode_one(model, &0)?;
                x.encode_with_config(visitor, minnow::FloatModel::new(-10_000.0..=10_000.0, 1))?;
                y.encode_with_config(visitor, minnow::FloatModel::new(0.0..=5_000.0, 0))?;
            }
            MyEnum::B => todo!(),
        }

        Ok(())
    }

    fn decode<R>(visitor: &mut minnow::DecodeVisitor<R>) -> std::io::Result<Self>
    where
        R: bitstream_io::BitRead,
        Self: Sized,
    {
        let model = minnow::OneShot::<2>::default();
        match visitor.decode_one(model)? {
            0 => {
                let x = visitor.decode_one(minnow::FloatModel::new(-10_000.0..=10_000.0, 1))?;
                let y = visitor.decode_one(minnow::FloatModel::new(0.0..=5_000.0, 0))?;
                Ok(MyEnum::A { x, y })
            }
            1 => Ok(MyEnum::B),
            _ => unreachable!(),
        }
    }
}

fn main() {
    let input = MyEnum::A { x: -5.0, y: 15.0 };

    println!("input: {:?}", input);

    let compressed = input.encode_bytes().unwrap();
    println!("bytes: {}", compressed.len());

    let output = MyEnum::decode_bytes(&compressed).unwrap();
    println!("output: {:?}", output);
}
