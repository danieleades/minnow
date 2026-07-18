use darling::ast;

use crate::parse;

pub struct StructData {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub fields: Style,
}

impl StructData {
    pub fn new(
        ident: syn::Ident,
        generics: syn::Generics,
        fields: ast::Fields<parse::Field>,
    ) -> Self {
        let fields = Style::new(fields);

        Self {
            ident,
            generics,
            fields,
        }
    }
}

pub enum Style {
    Tuple(Vec<parse::Field>),
    Struct(Vec<parse::Field>),
    Unit,
}

impl Style {
    /// Lower a set of parsed fields to their [`Style`] — also used for an
    /// enum variant's payload fields, which are treated as an anonymous
    /// product exactly like a struct's (see `process_enum.rs`).
    pub(crate) fn new(fields: ast::Fields<parse::Field>) -> Self {
        match fields.style {
            ast::Style::Tuple => Self::Tuple(fields.fields),
            ast::Style::Struct => Self::Struct(fields.fields),
            ast::Style::Unit => Self::Unit,
        }
    }
}
