#![deny(clippy::all, clippy::cargo)]
#![warn(clippy::pedantic)]
#![feature(associated_type_defaults)]
#![feature(never_type)]
#![doc = include_str!("../README.md")]

mod bounded_model;
mod encodeable;
mod encodeable_custom;
mod float;
mod impls;
mod visitor;

pub use encodeable::Encodeable;
pub use encodeable_custom::EncodeableCustom;
pub use float::FloatModel;
pub use impls::one_shot::OneShot;
pub use minnow_derive::Encodeable;
pub use visitor::{DecodeVisitor, EncodeVisitor};
