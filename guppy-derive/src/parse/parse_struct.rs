use darling::{FromField};
use proc_macro2::TokenStream;
use quote::quote;

use super::{parse_attributes, Model};

pub struct Field {
    pub ident: Option<syn::Ident>,
    pub ty: syn::Type,
    pub model: Option<Model>,
}

impl Field {
    pub fn model(&self) -> TokenStream {
        match self.model {
            Some(Model::Float { min, max, precision }) => quote!{ guppy::FloatModel::new( #min ..= #max, #precision ) },
            Some(Model::String { max_length }) => quote!{ guppy::StringModel::new( #max_length ) },
            None => quote!{()},
        }
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
