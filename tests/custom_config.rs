//! `#[encode(config = <expr>)]` (issue #8): an arbitrary Rust expression
//! evaluated at runtime as a field's config. Covers both a builtin model
//! constructed indirectly (proving the sugar and the general form produce
//! the same wire format) and a fully custom, hand-written
//! `Encodeable`/`Bounded` leaf whose config is not `float`/`string` sugar at
//! all.

use minnow::{Bounded, DecodeError, DecodeVisitor, EncodeVisitor, Encodeable, Weight};

// --- A custom leaf type with its own bespoke config -------------------------

/// A value clamped to a small inclusive integer range, encoded as a uniform
/// discriminant. Nothing like `FloatModel`/`StringModel` — this exercises
/// `#[encode(config = ...)]` against a genuinely custom model.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    min: i32,
    max: i32,
}

impl Range {
    pub const fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }

    fn denominator(self) -> u128 {
        u128::from((self.max - self.min).unsigned_abs()) + 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clamped(pub i32);

impl Bounded for Clamped {
    fn weight(config: &Self::Config) -> Weight {
        Weight::new(config.denominator())
    }
}

impl Encodeable for Clamped {
    type Config = Range;

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: Self::Config,
    ) -> Result<(), minnow::EncodeError>
    where
        W: bitstream_io::BitWrite,
    {
        let model = minnow::WeightedModel::new(vec![1_u128; config.denominator() as usize]);
        #[allow(clippy::cast_sign_loss)]
        let symbol = (self.0 - config.min) as u32;
        visitor.encode_one(model, &symbol)?;
        Ok(())
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: Self::Config,
    ) -> Result<Self, DecodeError>
    where
        R: bitstream_io::BitRead,
        Self: Sized,
    {
        let model = minnow::WeightedModel::new(vec![1_u128; config.denominator() as usize]);
        let symbol = visitor.decode_one(model)?;
        Ok(Self(config.min + symbol as i32))
    }
}

#[derive(Debug, Encodeable, PartialEq)]
pub struct WithCustomModel {
    #[encode(config = Range::new(-10, 10))]
    pub level: Clamped,
}

#[test]
fn custom_model_round_trips() {
    let input = WithCustomModel { level: Clamped(-3) };
    let bytes = input.encode_bytes().unwrap();
    let output = WithCustomModel::decode_bytes(&bytes).unwrap();
    assert_eq!(input, output);
}

#[test]
fn custom_model_weight_matches_range() {
    assert_eq!(<Clamped as Bounded>::weight(&Range::new(-10, 10)).get(), 21);
}

// --- `config = <expr>` as a general form of the float sugar -----------------

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct ViaSugar {
    #[encode(float(min = -100.0, max = 100.0, precision = 1))]
    pub x: f64,
}

#[derive(Debug, Encodeable, PartialEq, Clone, Copy)]
pub struct ViaConfigExpr {
    #[encode(config = minnow::FloatModel::new(-100.0..=100.0, 1).unwrap())]
    pub x: f64,
}

#[test]
fn config_expr_matches_float_sugar() {
    let sugar = ViaSugar { x: 42.5 };
    let expr = ViaConfigExpr { x: 42.5 };

    // Same underlying model, so the wire format is identical.
    assert_eq!(sugar.encode_bytes().unwrap(), expr.encode_bytes().unwrap());
    assert_eq!(
        ViaSugar::size_report().total_bytes(),
        ViaConfigExpr::size_report().total_bytes()
    );

    let bytes = expr.encode_bytes().unwrap();
    let decoded = ViaConfigExpr::decode_bytes(&bytes).unwrap();
    assert_eq!(decoded, expr);
}
