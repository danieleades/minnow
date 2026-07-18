use proc_macro2::TokenStream;

use crate::parse::{self, Model};

pub struct Variant {
    pub ident: syn::Ident,
    pub style: Style,
    /// A manual `#[encode(weight = N)]` discriminant weight, overriding the
    /// automatic (payload-cardinality) weight for this variant.
    pub weight_override: Option<u128>,
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
        Model::config_tokens(self.model.as_ref())
    }
}

// pub struct Struct {
//     pub fields: Vec<parse::Field>,
// }

impl From<parse::Variant> for Variant {
    fn from(input: parse::Variant) -> Self {
        let weight_override = input.weight;
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
                    weight_override,
                }
            }
            darling::ast::Style::Struct => todo!(),
            darling::ast::Style::Unit => Variant {
                ident: input.ident,
                style: Style::Unit,
                weight_override,
            },
        }
    }
}

pub struct EnumData {
    pub ident: syn::Ident,
    pub variants: Vec<Variant>,
}
