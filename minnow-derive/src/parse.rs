use darling::{Error, FromDeriveInput, FromMeta, ast};
use syn::Attribute;

pub mod parse_enum;
pub mod parse_struct;

pub use parse_enum::Variant;
pub use parse_struct::Field;

/// The largest denominator the arithmetic coder can encode at the default
/// precision. Mirrors `minnow::MAX_DENOMINATOR` (`2^(PRECISION - 2) - 1` with
/// `PRECISION = 64`; the `- 1` because a denominator of exactly `2^62` needs
/// 65 bits of precision under the coder's `frequency_bits + 2` rule);
/// duplicated here because the derive macro cannot depend on the `minnow`
/// crate at expansion time.
const MAX_DENOMINATOR: u128 = (1 << (64 - 2)) - 1;

#[derive(FromDeriveInput)]
pub struct Receiver {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: ast::Data<Variant, Field>,
}

/// A signed number parsed from attribute meta.
///
/// darling rejects bare negative literals in attributes (it expects them
/// quoted), which would break the ergonomic `min = -10_000.0` syntax. This
/// wrapper supports them by peeling a leading unary `-`.
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
// `i128` is wide enough to hold the full range of every integer type
// `IntModel` supports (`u8..=u64`, `i8..=i64`), so `int(min = ..., max =
// ...)` parses both bounds through this one signed representation regardless
// of the field's actual integer type; see `Model::config_tokens`.
impl_signed_number!(i128);

#[derive(FromMeta)]
pub enum Model {
    Float {
        min: Number<f64>,
        max: Number<f64>,
        precision: Number<i8>,
        /// `clamping` opts into coercing out-of-range values to the nearest
        /// bound instead of an `EncodeError::OutOfRange`.
        #[darling(default)]
        clamping: bool,
    },
    /// `#[encode(int(min = a, max = b))]` — a bounded integer, uniform over
    /// `a..=b`. Lowers to `minnow::IntModel::new(a..=b)`; the target integer
    /// type is inferred from the field, not from this attribute (see
    /// [`Model::config_tokens`]).
    Int {
        min: Number<i128>,
        max: Number<i128>,
        /// `clamping` opts into coercing out-of-range values to the nearest
        /// bound instead of an `EncodeError::OutOfRange`.
        #[darling(default)]
        clamping: bool,
    },
    String {
        max_length: usize,
    },
    /// `#[encode(seq(max_len = N, elem = <expr>))]` — a bounded `Vec<T>`,
    /// length-prefixed over `0..=N`. `elem` is the per-element config
    /// expression; it may be omitted when `T::Config: Default`, in which case
    /// it lowers to `Default::default()` like an unannotated field would.
    Seq {
        max_len: u32,
        #[darling(default)]
        elem: Option<syn::Expr>,
    },
    /// `#[encode(config = <expr>)]` — an arbitrary Rust expression evaluated
    /// as the field's runtime config. This is the escape hatch every other
    /// attribute form is sugar for (see [`Model::config_tokens`]); it accepts
    /// any expression, so it **cannot** be validated at macro-expansion time.
    /// An invalid or mistyped expression surfaces as an ordinary compile
    /// error at its interpolation site (e.g. a type mismatch against
    /// `<FieldType as EncodeableCustom>::Config`) rather than a clean
    /// macro-time diagnostic.
    Config(syn::Expr),
}

impl Model {
    fn from_attribute(attr: &Attribute) -> darling::Result<Self> {
        Self::from_meta(&attr.meta)
    }

    /// Lower an optional parsed model to the config expression the generated
    /// code passes to `encode_with_config`/`decode_with_config`.
    ///
    /// This is the single source of truth for that lowering — struct fields and
    /// enum-variant payloads must generate identical config expressions, or the
    /// two paths drift.
    pub fn config_tokens(model: Option<&Self>) -> proc_macro2::TokenStream {
        use quote::quote;
        match model {
            Some(Self::Float {
                min: Number(min),
                max: Number(max),
                precision: Number(precision),
                clamping,
            }) => {
                let clamping = clamping.then(|| quote! { .clamping() });
                quote! {
                    minnow::FloatModel::new( #min ..= #max, #precision )
                        .expect("model bounds validated at compile time")
                        #clamping
                }
            }
            Some(Self::Int {
                min: Number(min),
                max: Number(max),
                clamping,
            }) => {
                let clamping = clamping.then(|| quote! { .clamping() });
                // Emit unsuffixed integer literals so their type is inferred
                // from context (the field's concrete integer type, via
                // `IntModel<T>`'s `T`) rather than fixed to `i128`.
                let min = proc_macro2::Literal::i128_unsuffixed(*min);
                let max = proc_macro2::Literal::i128_unsuffixed(*max);
                quote! {
                    minnow::IntModel::new( #min ..= #max )
                        .expect("model bounds validated at compile time")
                        #clamping
                }
            }
            Some(Self::String { max_length }) => {
                quote! {
                    minnow::StringModel::new( #max_length )
                        .expect("model bounds validated at compile time")
                }
            }
            Some(Self::Seq { max_len, elem }) => {
                let elem = elem.as_ref().map_or_else(
                    || quote! { ::core::default::Default::default() },
                    |expr| quote! { #expr },
                );
                quote! {
                    minnow::SeqModel { max_len: #max_len, elem: #elem }
                }
            }
            Some(Self::Config(expr)) => quote! { #expr },
            // Fall back to `Default::default()` rather than a literal `()` so
            // that a field whose type is a generic parameter with a
            // non-`()` `Config` (constrained by a `Default` bound the derive
            // adds — see `write.rs`) still works, not just concrete types
            // that happen to use `Config = ()`.
            None => quote! { ::core::default::Default::default() },
        }
    }

    /// Validate a model's parameters at macro-expansion time so that invalid
    /// bounds become a clean compile error rather than a runtime panic.
    fn validate(&self) -> Result<(), String> {
        match self {
            Model::Float {
                min: Number(min),
                max: Number(max),
                precision: Number(precision),
                clamping: _,
            } => {
                if !min.is_finite() || !max.is_finite() {
                    return Err(
                        "float model bounds must be finite (neither NaN nor infinite)".into(),
                    );
                }
                if min > max {
                    return Err(format!(
                        "float model lower bound ({min}) must not exceed the upper bound ({max})"
                    ));
                }
                // This must mirror `FloatModel::new` exactly (same f64
                // arithmetic, then the same *integer* comparison) so that
                // anything accepted here is also accepted at runtime.
                // Comparing in f64 instead would disagree near the boundary,
                // where `steps + 1.0` rounds back to `MAX_DENOMINATOR`.
                let multiplier = 10_f64.powi(i32::from(*precision));
                let steps = ((max - min) * multiplier).round();
                let steps = num_traits::ToPrimitive::to_u128(&steps).ok_or_else(|| {
                    format!(
                        "float model denominator exceeds the maximum ({MAX_DENOMINATOR}) \
                         permitted at precision 64; narrow the range or reduce the precision"
                    )
                })?;
                // `denominator = steps + 1`; reject before the `+ 1` can
                // overflow or exceed the bound.
                if steps >= MAX_DENOMINATOR {
                    return Err(format!(
                        "float model denominator ({}) exceeds the maximum ({MAX_DENOMINATOR}) \
                         permitted at precision 64; narrow the range or reduce the precision",
                        steps.saturating_add(1)
                    ));
                }
                Ok(())
            }
            Model::Int {
                min: Number(min),
                max: Number(max),
                clamping: _,
            } => {
                if min > max {
                    return Err(format!(
                        "int model lower bound ({min}) must not exceed the upper bound ({max})"
                    ));
                }
                // This must mirror `IntModel::new` exactly: an `i128`
                // difference, then the same integer comparison against
                // `MAX_DENOMINATOR`.
                let width = max.checked_sub(*min).ok_or_else(|| {
                    "int model range width overflows i128; narrow the range".to_string()
                })?;
                let width = u128::try_from(width).map_err(|_| {
                    "int model range width overflows i128; narrow the range".to_string()
                })?;
                // `denominator = width + 1`; reject before the `+ 1` can
                // overflow or exceed the bound.
                if width >= MAX_DENOMINATOR {
                    return Err(format!(
                        "int model denominator ({}) exceeds the maximum ({MAX_DENOMINATOR}) \
                         permitted at precision 64; narrow the range",
                        width.saturating_add(1)
                    ));
                }
                Ok(())
            }
            Model::String { max_length } => {
                // Mirrors `StringModel::new`: the only way construction can
                // fail is `max_length` not fitting in the `u32` that backs
                // `SeqModel::max_len` — a `u32`-bounded length model's
                // denominator (`max_length + 1 <= 2^32`) is always far below
                // `MAX_DENOMINATOR` (`~2^62`), so that's the only check
                // needed here.
                if u32::try_from(*max_length).is_err() {
                    return Err(format!(
                        "string max_length ({max_length}) exceeds the maximum representable \
                         length ({})",
                        u32::MAX
                    ));
                }
                Ok(())
            }
            // `Seq`'s `max_len` is already a `u32` by construction (parsed as
            // one), and a `u32`-bounded length model's denominator can never
            // exceed `MAX_DENOMINATOR` (see the `String` case above), so
            // there is nothing further to validate here. The `elem`
            // expression, like `Config`, cannot be checked at macro-expansion
            // time.
            Model::Seq { .. } | Model::Config(_) => Ok(()),
        }
    }
}

/// Parse the `#[encode(...)]` attributes on an enum *variant*.
///
/// A variant may carry an optional payload [`Model`] (as for a struct field)
/// and/or an optional manual discriminant `#[encode(weight = N)]` override. The
/// two are distinguished by shape: `weight = N` is a name/value pair,
/// everything else is a model. Keeping this separate from [`parse_attributes`]
/// leaves struct-field parsing untouched; full attribute unification is
/// deferred.
fn parse_variant_attributes(
    attrs: &[syn::Attribute],
) -> darling::Result<(Option<Model>, Option<u128>)> {
    let mut errors = darling::Error::accumulator();
    let mut model: Option<Model> = None;
    let mut weight: Option<u128> = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("encode")) {
        // `#[encode(weight = N)]` parses as a single name/value pair; anything
        // else (e.g. `float(...)`) does not, and falls through to `Model`.
        if let Ok(name_value) = attr.parse_args::<syn::MetaNameValue>() {
            if name_value.path.is_ident("weight") {
                match parse_u128_literal(&name_value.value) {
                    Ok(value) => {
                        if value < 1 {
                            errors
                                .push(Error::custom("`weight` must be at least 1").with_span(attr));
                        }
                        if weight.is_some() {
                            errors.push(
                                Error::custom("duplicate `weight` attribute").with_span(attr),
                            );
                        }
                        weight = Some(value);
                    }
                    Err(e) => errors.push(e.with_span(attr)),
                }
                continue;
            }
        }

        if model.is_some() {
            errors.push(Error::custom("duplicate payload model attribute").with_span(attr));
        }
        if let Some(parsed) = errors.handle(Model::from_attribute(attr)) {
            if let Err(message) = parsed.validate() {
                errors.push(Error::custom(message).with_span(attr));
            }
            model = Some(parsed);
        }
    }

    errors.finish()?;

    Ok((model, weight))
}

/// Parse a non-negative integer literal from an attribute value expression.
fn parse_u128_literal(expr: &syn::Expr) -> darling::Result<u128> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(int),
        ..
    }) = expr
    {
        int.base10_parse::<u128>().map_err(Error::from)
    } else {
        Err(Error::custom(
            "`weight` must be an unsigned integer literal",
        ))
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
    #[test_case(
        quote! {
            #[derive(Encodeable)]
            pub struct Reading {
                #[encode(int(min = -10, max = 10))]
                pub x: i32,
            }
        }
        ; "int"
    )]
    #[test_case(
        quote! {
            #[derive(Encodeable)]
            pub struct Reading {
                #[encode(seq(max_len = 8, elem = minnow::FloatModel::new(0.0..=1.0, 1).unwrap()))]
                pub samples: Vec<f64>,
            }
        }
        ; "seq with elem"
    )]
    #[test_case(
        quote! {
            #[derive(Encodeable)]
            pub struct Reading {
                #[encode(seq(max_len = 8))]
                pub flags: Vec<bool>,
            }
        }
        ; "seq without elem"
    )]
    fn parse(tokens: TokenStream) {
        let parsed: syn::DeriveInput = syn::parse2(tokens).unwrap();
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
            clamping: false,
        };
        model.validate().is_ok()
    }

    /// The macro-time integer bounds check must agree with `IntModel::new`
    /// exactly at the `MAX_DENOMINATOR` boundary. Unlike the float case this
    /// is exact-integer arithmetic on both sides, so agreement is
    /// straightforward — but still worth pinning down with a test.
    #[test_case(4_611_686_018_427_387_902 => true; "largest width below the boundary is accepted")]
    #[test_case(4_611_686_018_427_387_903 => false; "width equal to MAX_DENOMINATOR is rejected")]
    fn int_boundary_agrees_with_runtime(max: i128) -> bool {
        use super::{Model, Number};
        let model = Model::Int {
            min: Number(0),
            max: Number(max),
            clamping: false,
        };
        model.validate().is_ok()
    }

    #[test]
    fn int_rejects_inverted_bounds() {
        use super::{Model, Number};
        let model = Model::Int {
            min: Number(10),
            max: Number(-10),
            clamping: false,
        };
        assert!(model.validate().is_err());
    }

    #[test]
    fn int_accepts_negative_range() {
        use super::{Model, Number};
        let model = Model::Int {
            min: Number(-10),
            max: Number(10),
            clamping: false,
        };
        assert!(model.validate().is_ok());
    }

    #[test]
    fn string_rejects_max_length_over_u32() {
        use super::Model;
        let model = Model::String {
            max_length: u32::MAX as usize + 1,
        };
        assert!(model.validate().is_err());
    }

    #[test]
    fn string_accepts_max_length_at_u32_max() {
        use super::Model;
        let model = Model::String {
            max_length: u32::MAX as usize,
        };
        assert!(model.validate().is_ok());
    }
}
