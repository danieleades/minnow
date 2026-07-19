use minnow::{Bounded, Encodeable, FloatModel, SizeReport, Weight};

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
    type Config = ();

    fn encode_with_config<W>(
        &self,
        visitor: &mut minnow::EncodeVisitor<W>,
        _config: (),
    ) -> Result<(), minnow::EncodeError>
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

    fn decode_with_config<R>(
        visitor: &mut minnow::DecodeVisitor<R>,
        _config: (),
    ) -> Result<Self, minnow::DecodeError>
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
}

/// Implementing [`Bounded`] is the promise that every value fits a static
/// budget; `weight` is the only required method, but this impl encodes the
/// discriminant with a *uniform* `OneShot::<2>` model (1 bit per variant, not
/// the cardinality-weighted split a derived impl would use), so the bit
/// bounds and the report must be overridden to say what the codec actually
/// does.
impl Bounded for MyEnum {
    fn weight(_config: &Self::Config) -> Weight {
        // The true cardinality — sum over variants, product over fields —
        // regardless of how the discriminant is weighted on the wire.
        let a = Weight::new(x_model().denominator()) * Weight::new(y_model().denominator());
        a + Weight::ONE
    }

    fn worst_case_bits(_config: &Self::Config) -> f64 {
        // Variant `A`: 1 discriminant bit plus both payload fields.
        1.0 + Weight::new(x_model().denominator()).log2()
            + Weight::new(y_model().denominator()).log2()
    }

    fn best_case_bits(_config: &Self::Config) -> f64 {
        // Variant `B`: just the discriminant bit.
        1.0
    }

    fn report(_config: &Self::Config) -> SizeReport {
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

        let compressed = input.encode_bytes().unwrap();
        println!("bytes: {}", compressed.len());

        let output = MyEnum::decode_bytes(&compressed).expect("round-trip should succeed");
        println!("output: {output:?}");
    }
}
