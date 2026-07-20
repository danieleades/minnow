//! Lowers a parsed enum's variants: each variant's payload is reduced to a
//! [`StructStyle`], exactly like a struct's fields (see
//! `crate::process::process_struct`), so a struct-style or multi-field tuple
//! variant is just an anonymous struct to [`crate::write`].

use darling::export::syn;

use crate::{parse, process::StructStyle};

pub struct Variant {
    pub ident: syn::Ident,
    /// The variant's payload, treated exactly like an anonymous struct's
    /// fields — a unit variant has [`StructStyle::Unit`], a tuple variant
    /// [`StructStyle::Tuple`], a struct variant [`StructStyle::Struct`].
    pub style: StructStyle,
    /// A manual `#[encode(weight = N)]` discriminant weight, overriding the
    /// automatic (payload-cardinality) weight for this variant.
    pub weight_override: Option<u128>,
}

impl From<parse::Variant> for Variant {
    fn from(input: parse::Variant) -> Self {
        Self {
            ident: input.ident,
            style: StructStyle::new(input.fields),
            weight_override: input.weight,
        }
    }
}

pub struct EnumData {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub variants: Vec<Variant>,
    /// `#[encode(unbounded)]`: skip the generated `Bounded` impl.
    pub unbounded: bool,
}
