
use crate::parse;

mod process_enum;
use process_enum::Variant;
mod process_struct;
pub use process_enum::EnumData;
pub use process_enum::Style as EnumStyle;
pub use process_struct::StructData;
pub use process_struct::Style as StructStyle;

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
            darling::ast::Data::Enum(variants) => {
                Data::Enum(EnumData {
                    ident: receiver.ident,
                    variants: variants.into_iter().map(Variant::from).collect(),
                })
            },
            darling::ast::Data::Struct(fields) => {
                Data::Struct(StructData::new(receiver.ident, receiver.generics, fields))
            },
        }
    }
}