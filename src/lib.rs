#![deny(clippy::all, clippy::cargo)]
#![warn(clippy::pedantic)]

#![feature(associated_type_defaults)]
#![feature(never_type)]

mod bounded_model;
mod encodeable;
mod encodeable_custom;
mod float;
mod impls;
mod visitor;

pub use encodeable::{Encodeable};
pub use encodeable_custom::EncodeableCustom;
pub use visitor::{EncodeVisitor, DecodeVisitor};
pub use float::FloatModel;
pub use minnow_derive::Encodeable;
pub use impls::one_shot::OneShot;
