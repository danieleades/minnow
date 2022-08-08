use proc_macro2::TokenStream;
use quote::quote;

use crate::parse::{self, Model};

pub struct Variant {
    pub ident: syn::Ident,
    pub style: Style,
}

#[allow(clippy::large_enum_variant)]
pub enum Style {
    Tuple(Tuple),
    // Struct(Struct),
    Unit,
}

pub struct Tuple {
    pub ty: syn::Type,
    pub model: Option<Model>,
}

impl Tuple {
    pub fn model(&self) -> TokenStream {
        match self.model {
            Some(Model::Float {
                min,
                max,
                precision,
            }) => quote! { minnow::FloatModel::new( #min ..= #max, #precision ) },
            Some(Model::String { max_length }) => {
                quote! { minnow::StringModel::new( #max_length ) }
            }
            None => quote! {()},
        }
    }
}

// pub struct Struct {
//     pub fields: Vec<parse::Field>,
// }

impl From<parse::Variant> for Variant {
    fn from(input: parse::Variant) -> Self {
        match input.fields.style {
            darling::ast::Style::Tuple => {
                let tuple = Tuple {
                    ty: input.fields.fields[0].ty.clone(),
                    model: input.options,
                };

                let style = Style::Tuple(tuple);

                Variant {
                    ident: input.ident,
                    style,
                }
            }
            darling::ast::Style::Struct => todo!(),
            darling::ast::Style::Unit => Variant {
                ident: input.ident,
                style: Style::Unit,
            },
        }
    }
}

pub struct EnumData {
    pub ident: syn::Ident,
    pub variants: Vec<Variant>,
}
