use darling::{FromField, FromVariant};

use super::{parse_attributes, Model};

#[derive(FromField)]
pub struct Field {
    pub ty: syn::Type,
}

pub struct Variant {
    pub ident: syn::Ident,
    pub options: Option<Model>,
    pub fields: darling::ast::Fields<Field>,
}

impl FromVariant for Variant {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        let mut errors = darling::Error::accumulator();

        let options = errors.handle(parse_attributes(&variant.attrs));
        let fields = errors.handle(darling::ast::Fields::try_from(&variant.fields));

        errors.finish()?;

        Ok(Self {
            ident: variant.ident.clone(),
            fields: fields.unwrap(),
            options: options.unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use darling::FromDeriveInput;
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
    fn parse(tokens: TokenStream) {
        let parsed = syn::parse_str(&tokens.to_string()).unwrap();
        let _receiver = Receiver::from_derive_input(&parsed).unwrap();
    }
}
