use minnow::{Encodeable, EncodeableCustom, FloatModel, SizeReport, Weight};

#[derive(Debug)]
pub enum MyEnum {
    A { x: f64, y: f64 },
    B,
}

/// The models are named once so that encode, decode, and the size report
/// cannot drift apart.
fn x_model() -> FloatModel<f64> {
    FloatModel::new(-10_000.0..=10_000.0, 1).expect("bounds are valid")
}

fn y_model() -> FloatModel<f64> {
    FloatModel::new(0.0..=5_000.0, 0).expect("bounds are valid")
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
                x.encode_with_config(visitor, x_model())?;
                y.encode_with_config(visitor, y_model())?;
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
                let x = visitor.decode_one(x_model())?;
                let y = visitor.decode_one(y_model())?;
                Ok(MyEnum::A { x, y })
            }
            1 => Ok(MyEnum::B),
            other => Err(minnow::DecodeError::InvalidSymbol {
                symbol: u128::from(other),
            }),
        }
    }

    fn size_report() -> SizeReport {
        // This impl encodes the discriminant with a *uniform* `OneShot::<2>`
        // model, so each variant's discriminant costs exactly 1 bit, and the
        // report must say so. (A derived impl would use a weighted
        // discriminant instead.)
        let x_bits = Weight::new(x_model().denominator()).log2();
        let y_bits = Weight::new(y_model().denominator()).log2();
        SizeReport::sum(vec![
            SizeReport::enum_variant(
                "A",
                1.0,
                SizeReport::product(vec![
                    SizeReport::leaf(x_bits).with_name("x"),
                    SizeReport::leaf(y_bits).with_name("y"),
                ]),
            ),
            SizeReport::enum_variant("B", 1.0, SizeReport::leaf(0.0)),
        ])
    }
}

fn main() {
    println!("{}", MyEnum::size_report());

    for input in [MyEnum::A { x: -5.0, y: 15.0 }, MyEnum::B] {
        println!("input: {input:?}");

        let compressed = input.encode_bytes();
        println!("bytes: {}", compressed.len());

        let output = MyEnum::decode_bytes(&compressed).expect("round-trip should succeed");
        println!("output: {output:?}");
    }
}
