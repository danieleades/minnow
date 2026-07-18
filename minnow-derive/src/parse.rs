use darling::{Error, FromDeriveInput, FromMeta, ast};
use syn::Attribute;

pub mod parse_enum;
pub mod parse_struct;

pub use parse_enum::Variant;
pub use parse_struct::Field;

/// The largest denominator the arithmetic coder can encode at the default
/// precision. Mirrors `minnow::MAX_DENOMINATOR` (`2^(PRECISION - 2)` with
/// `PRECISION = 64`); duplicated here because the derive macro cannot depend on
/// the `minnow` crate at expansion time.
const MAX_DENOMINATOR: u128 = 1 << (64 - 2);

#[derive(FromDeriveInput)]
pub struct Receiver {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: ast::Data<Variant, Field>,
}

/// A signed number parsed from attribute meta.
///
/// darling 0.20 rejects bare negative literals in attributes (it expects them
/// quoted), which would break the ergonomic `min = -10_000.0` syntax. This
/// wrapper restores support by peeling a leading unary `-`.
#[derive(Debug, Clone, Copy)]
pub struct Number<T>(pub T);

macro_rules! impl_signed_number {
    ($t:ty) => {
        impl FromMeta for Number<$t> {
            fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
                if let syn::Expr::Unary(unary) = expr {
                    if matches!(unary.op, syn::UnOp::Neg(_)) {
                        let Number(value) = <Number<$t>>::from_expr(&unary.expr)?;
                        return Ok(Number(-value));
                    }
                }
                <$t as FromMeta>::from_expr(expr).map(Number)
            }
        }
    };
}

impl_signed_number!(f64);
impl_signed_number!(i8);

#[derive(FromMeta)]
pub enum Model {
    Float {
        min: Number<f64>,
        max: Number<f64>,
        precision: Number<i8>,
    },
    String {
        max_length: usize,
    },
}

impl Model {
    fn from_attribute(attr: &Attribute) -> darling::Result<Self> {
        Self::from_meta(&attr.meta)
    }

    /// Validate a model's parameters at macro-expansion time so that invalid
    /// bounds become a clean compile error rather than a runtime panic.
    fn validate(&self) -> Result<(), String> {
        if let Model::Float {
            min: Number(min),
            max: Number(max),
            precision: Number(precision),
        } = self
        {
            if !min.is_finite() || !max.is_finite() {
                return Err("float model bounds must be finite (neither NaN nor infinite)".into());
            }
            if min > max {
                return Err(format!(
                    "float model lower bound ({min}) must not exceed the upper bound ({max})"
                ));
            }
            // This must mirror `FloatModel::new` exactly (same f64 arithmetic,
            // then the same *integer* comparison) so that anything accepted
            // here is also accepted at runtime. Comparing in f64 instead would
            // disagree near the boundary, where `steps + 1.0` rounds back to
            // `MAX_DENOMINATOR`.
            let multiplier = 10_f64.powi(i32::from(*precision));
            let steps = ((max - min) * multiplier).round();
            let steps = num_traits::ToPrimitive::to_u128(&steps).ok_or_else(|| {
                format!(
                    "float model denominator exceeds the maximum ({MAX_DENOMINATOR}) permitted at \
                     precision 64; narrow the range or reduce the precision"
                )
            })?;
            // `denominator = steps + 1`; reject before the `+ 1` can overflow
            // or exceed the bound.
            if steps >= MAX_DENOMINATOR {
                return Err(format!(
                    "float model denominator ({}) exceeds the maximum ({MAX_DENOMINATOR}) \
                     permitted at precision 64; narrow the range or reduce the precision",
                    steps.saturating_add(1)
                ));
            }
        }
        Ok(())
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> darling::Result<Option<Model>> {
    let mut errors = darling::Error::accumulator();

    let encode_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("encode"))
        .collect::<Vec<_>>();

    // Make sure we have exactly one `#[encode]` attribute to avoid conflicting
    // definitions
    let options = match encode_attrs.len() {
        0 => None,
        1 => errors.handle(Model::from_attribute(encode_attrs[0])),
        _ => {
            errors.handle(Model::from_attribute(encode_attrs[0]));
            for attr in encode_attrs.iter().skip(1) {
                errors.handle(Model::from_attribute(attr));
                errors.push(
                    Error::custom(
                        "Unexpected encode attribute. Each field should have a single attribute \
                         only",
                    )
                    .with_span(attr),
                );
            }
            None
        }
    };

    // Validate the parsed model, attaching the error to the attribute's span.
    if let Some(model) = &options {
        if let Err(message) = model.validate() {
            errors.push(Error::custom(message).with_span(&encode_attrs[0]));
        }
    }

    errors.finish()?;

    Ok(options)
}

#[cfg(test)]
mod tests {
    use darling::FromDeriveInput;
    use proc_macro2::TokenStream;
    use quote::quote;
    use test_case::test_case;

    use super::Receiver;

    #[test_case(
        quote! {
            #[derive(Encodeable)]
            pub struct NavigationReport {
                #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
                pub x: f64,
            }
        }
        ; "float"
    )]
    #[test_case(
        quote! {
            #[derive(Encodeable)]
            pub struct NavigationReport {
                #[encode(string(max_length = 100))]
                pub x: String,
            }
        }
        ; "string"
    )]
    #[test_case(
        quote! {
            #[derive(Encodeable)]
            pub enum VehicleType {
                Auv,
                Usv,
                Ship,
            }
        }
        ; "unit enum"
    )]
    #[allow(clippy::needless_pass_by_value)]
    fn parse(tokens: TokenStream) {
        let parsed = syn::parse_str(&tokens.to_string()).unwrap();
        let _receiver = Receiver::from_derive_input(&parsed).unwrap();
    }

    /// The macro-time bounds check must agree with `FloatModel::new` exactly
    /// at the `MAX_DENOMINATOR` boundary: `steps + 1.0` rounds back to
    /// `2^62` in `f64`, so an `f64` comparison would accept a model that the
    /// runtime constructor rejects, turning the generated
    /// `.expect("validated at compile time")` into a runtime panic.
    #[test_case(4_611_686_018_427_387_904.0 => false; "steps of two to the sixty-two is rejected")]
    #[test_case(4_611_686_018_427_387_392.0 => true; "largest f64 below the boundary is accepted")]
    #[test_case(9_223_372_036_854_775_808.0 => false; "well past the boundary is rejected")]
    fn boundary_agrees_with_runtime(max: f64) -> bool {
        use super::{Model, Number};
        let model = Model::Float {
            min: Number(0.0),
            max: Number(max),
            precision: Number(0),
        };
        model.validate().is_ok()
    }
}
