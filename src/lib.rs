#![feature(associated_type_defaults)]
#![feature(never_type)]

mod bounded_model;
mod encodeable;
mod encodeable_custom;
mod float;
mod impls;
mod visitor;
// mod navigation_report;

pub use encodeable::{Encodeable};
pub use encodeable_custom::EncodeableCustom;
pub use visitor::{EncodeVisitor, DecodeVisitor};
pub use float::FloatModel;
pub use guppy_derive::Encodeable;
pub use impls::one_shot::OneShot;
