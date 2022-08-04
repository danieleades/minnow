use darling::{ast, Error, FromDeriveInput, FromMeta};
use syn::Attribute;

pub mod parse_enum;
pub mod parse_struct;

pub use parse_enum::Variant;
pub use parse_struct::Field;

#[derive(FromDeriveInput)]
pub struct Receiver {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: ast::Data<Variant, Field>,
}

#[derive(FromMeta)]
pub enum Model {
    Float { min: f64, max: f64, precision: i8 },
    String { max_length: usize },
}

impl Model {
    fn from_attribute(attr: &Attribute) -> darling::Result<Self> {
        attr.parse_meta()
            .map_err(darling::Error::from)
            .and_then(|f| Self::from_meta(&f))
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> darling::Result<Option<Model>> {
    let mut errors = darling::Error::accumulator();

    let encode_attrs = attrs
        .iter()
        .filter(|attr| attr.path.is_ident("encode"))
        .collect::<Vec<_>>();

    // Make sure we have exactly one `#[encode]` attribute to avoid conflicting
    // definitions
    let options = match encode_attrs.len() {
        0 => None,
        1 => {
            errors.handle(Model::from_attribute(encode_attrs[0]))
        }
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
    fn parse(tokens: TokenStream) {
        let parsed = syn::parse_str(&tokens.to_string()).unwrap();
        let _receiver = Receiver::from_derive_input(&parsed).unwrap();
    }
}
