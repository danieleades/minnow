//! Attribute parsing for a struct's (or enum variant's) individual fields.

use darling::{FromField, export::syn};
use proc_macro2::TokenStream;

use super::{Model, parse_attributes};

pub struct Field {
    pub ident: Option<syn::Ident>,
    pub ty: syn::Type,
    pub model: Option<Model>,
}

impl Field {
    pub fn model(&self) -> TokenStream {
        Model::config_tokens(self.model.as_ref())
    }
}

impl FromField for Field {
    fn from_field(field: &syn::Field) -> darling::Result<Self> {
        let model = parse_attributes(&field.attrs)?;

        // Final assembly; none of these operations should be fallible.
        Ok(Self {
            ident: field.ident.clone(),
            ty: field.ty.clone(),
            model,
        })
    }
}
