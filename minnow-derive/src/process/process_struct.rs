//! Lowers a parsed struct's fields into the shape [`crate::write`] codegens
//! from: [`Style`] classifies it as unit / tuple / struct, matching the same
//! classification used for an enum variant's payload (see
//! `crate::process::process_enum`), so the two share one codegen path.

use darling::{ast, export::syn};

use crate::parse;

pub struct StructData {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub fields: Style,
    /// `#[encode(unbounded)]`: skip the generated `Bounded` impl.
    pub unbounded: bool,
}

impl StructData {
    pub fn new(
        ident: syn::Ident,
        generics: syn::Generics,
        fields: ast::Fields<parse::Field>,
        unbounded: bool,
    ) -> Self {
        let fields = Style::new(fields);

        Self {
            ident,
            generics,
            fields,
            unbounded,
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
