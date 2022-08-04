use darling::ast;

use crate::parse;


pub struct StructData {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub fields: Style,
}

impl StructData {
    pub fn new(ident: syn::Ident, generics: syn::Generics, fields: ast::Fields<parse::Field>) -> Self {
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
    fn new(fields: ast::Fields<parse::Field>) -> Self {
        match fields.style {
            ast::Style::Tuple => Self::Tuple(fields.fields),
            ast::Style::Struct => Self::Struct(fields.fields),
            ast::Style::Unit => Self::Unit,
        }
    }
}
