//! Stage 2 of the derive pipeline (`parse` → `process` → `write`): lower the
//! parsed [`parse::Receiver`] into a [`Data`] tree shaped for codegen.
//!
//! Where [`crate::parse`] is concerned with attribute *syntax*, this stage is
//! concerned with *structure*: a struct's fields and an enum variant's
//! payload fields are both reduced to the same shape ([`StructStyle`]: unit /
//! tuple / struct), so [`crate::write`] can treat a variant's payload exactly
//! like an anonymous struct rather than special-casing it. This is also where
//! an enum variant's discriminant weight is decided (the manual override from
//! `#[encode(weight = N)]`, or `None` for the automatic payload-cardinality
//! weight `crate::write` derives at codegen time).

use crate::parse;

mod process_enum;
use process_enum::Variant;
mod process_struct;
pub use process_enum::{EnumData, Variant as EnumVariant};
pub use process_struct::{StructData, Style as StructStyle};

pub fn process(receiver: parse::Receiver) -> Data {
    Data::from(receiver)
}

pub enum Data {
    Struct(StructData),
    Enum(EnumData),
}

impl From<parse::Receiver> for Data {
    fn from(receiver: parse::Receiver) -> Self {
        match receiver.data {
            darling::ast::Data::Enum(variants) => Data::Enum(EnumData {
                ident: receiver.ident,
                generics: receiver.generics,
                variants: variants.into_iter().map(Variant::from).collect(),
            }),
            darling::ast::Data::Struct(fields) => {
                Data::Struct(StructData::new(receiver.ident, receiver.generics, fields))
            }
        }
    }
}
