use darling::{FromVariant, ast, export::syn};

use super::{Field, parse_variant_attributes};

pub struct Variant {
    pub ident: syn::Ident,
    pub weight: Option<u128>,
    /// The variant's payload fields. Empty for a unit variant; `style ==
    /// Tuple` for a tuple variant (`Foo(f64, bool)`); `style == Struct` for a
    /// struct variant (`Foo { x: f64, y: bool }`). Each field carries its own
    /// optional payload [`super::Model`] (see [`Field`]).
    pub fields: ast::Fields<Field>,
}

impl FromVariant for Variant {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        let mut errors = darling::Error::accumulator();

        let attributes = errors.handle(parse_variant_attributes(&variant.attrs));
        let fields: Option<ast::Fields<Field>> =
            errors.handle(ast::Fields::try_from(&variant.fields));

        errors.finish()?;

        // `unwrap()` is safe: `errors.finish()` above already returned early
        // if either `handle` call recorded an error, so both are `Some` here.
        let (legacy_model, weight) = attributes.unwrap();
        let mut fields = fields.unwrap();

        // The legacy form puts a payload model directly on the variant
        // (`#[encode(float(...))] Foo(f64)`), which only makes sense for a
        // single-field tuple variant. Fold it into that field so every other
        // stage of the pipeline only ever looks at field-level models.
        if let Some(model) = legacy_model {
            match fields.style {
                ast::Style::Tuple if fields.fields.len() == 1 => {
                    let field = &mut fields.fields[0];
                    if field.model.is_some() {
                        return Err(darling::Error::custom(
                            "cannot combine a payload model attribute on the variant with an \
                             `#[encode]` attribute on its field; put the attribute on the field \
                             alone",
                        )
                        .with_span(&variant.ident));
                    }
                    field.model = Some(model);
                }
                _ => {
                    return Err(darling::Error::custom(
                        "a payload model attribute on the variant itself is only supported for \
                         single-field tuple variants; put `#[encode]` attributes on the payload \
                         fields instead",
                    )
                    .with_span(&variant.ident));
                }
            }
        }

        Ok(Self {
            ident: variant.ident.clone(),
            fields,
            weight,
        })
    }
}

#[cfg(test)]
mod tests {
    use darling::{FromDeriveInput, export::syn};
    use proc_macro2::TokenStream;
    use quote::quote;
    use test_case::test_case;

    use crate::parse::Receiver;

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
            pub enum VehicleType {
                Auv(String),
                Usv(bool),
                Ship(f64),
            }
        }
        ; "tuple enum no model"
    )]
    #[test_case(
        quote! {
            #[derive(Debug, Encodeable)]
            pub enum Ordinate {
                #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
                X(f64),
                #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
                Y(f64),
                #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
                Z(f64),
            }
        }
        ; "tuple enum w model"
    )]
    #[test_case(
        quote! {
            #[derive(Debug, Encodeable)]
            pub enum MyEnum {
                A {
                    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
                    x: f64,
                    y: bool,
                },
                B,
            }
        }
        ; "struct variant"
    )]
    #[test_case(
        quote! {
            #[derive(Debug, Encodeable)]
            pub enum MyEnum {
                A(
                    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
                    bool,
                ),
                B,
            }
        }
        ; "multi field tuple variant"
    )]
    fn parse(tokens: TokenStream) {
        let parsed: syn::DeriveInput = syn::parse2(tokens).unwrap();
        let _receiver = Receiver::from_derive_input(&parsed).unwrap();
    }
}
