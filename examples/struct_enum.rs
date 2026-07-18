use minnow::{Encodeable, EncodeableCustom};

#[derive(Debug)]
pub enum MyEnum {
    A { x: f64, y: f64 },
    B,
}

impl Encodeable for MyEnum {
    fn encode<W>(&self, visitor: &mut minnow::EncodeVisitor<W>) -> std::io::Result<()>
    where
        W: bitstream_io::BitWrite,
    {
        let model = minnow::OneShot::<2>;
        match self {
            MyEnum::A { x, y } => {
                visitor.encode_one(model, &0)?;
                x.encode_with_config(
                    visitor,
                    minnow::FloatModel::new(-10_000.0..=10_000.0, 1).unwrap(),
                )?;
                y.encode_with_config(visitor, minnow::FloatModel::new(0.0..=5_000.0, 0).unwrap())?;
            }
            MyEnum::B => {
                visitor.encode_one(model, &1)?;
            }
        }

        Ok(())
    }

    fn decode<R>(visitor: &mut minnow::DecodeVisitor<R>) -> Result<Self, minnow::DecodeError>
    where
        R: bitstream_io::BitRead,
        Self: Sized,
    {
        let model = minnow::OneShot::<2>;
        match visitor.decode_one(model)? {
            0 => {
                let x =
                    visitor.decode_one(minnow::FloatModel::new(-10_000.0..=10_000.0, 1).unwrap())?;
                let y = visitor.decode_one(minnow::FloatModel::new(0.0..=5_000.0, 0).unwrap())?;
                Ok(MyEnum::A { x, y })
            }
            1 => Ok(MyEnum::B),
            other => Err(minnow::DecodeError::InvalidSymbol {
                symbol: u128::from(other),
            }),
        }
    }
}

fn main() {
    for input in [MyEnum::A { x: -5.0, y: 15.0 }, MyEnum::B] {
        println!("input: {input:?}");

        let compressed = input.encode_bytes();
        println!("bytes: {}", compressed.len());

        let output = MyEnum::decode_bytes(&compressed).expect("round-trip should succeed");
        println!("output: {output:?}");
    }
}
