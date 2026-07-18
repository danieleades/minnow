//! Derive macro for the [Minnow](https://github.com/danieleades/minnow) crate

#![deny(clippy::all, clippy::cargo)]
#![warn(clippy::pedantic)]

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use process::process;
use syn::parse_macro_input;

mod parse;
mod process;
mod write;

/// Derives [`EncodeableCustom`](https://docs.rs/minnow/latest/minnow/trait.EncodeableCustom.html)
/// (and, through it, `Encodeable`) for a struct or enum.
///
/// Supports plain, tuple, and struct-style structs; and unit, tuple
/// (single- or multi-field), and struct-style enum variants. A struct's
/// fields, and an enum variant's payload fields, are encoded/decoded in
/// declaration order.
///
/// # Field attributes
///
/// A field (struct or enum-variant) may carry one `#[encode(...)]` attribute
/// naming its runtime config:
///
/// * `#[encode(float(min = ..., max = ..., precision = ...))]` — sugar for a [`FloatModel`](https://docs.rs/minnow/latest/minnow/struct.FloatModel.html);
///   `min`/`max`/`precision` are validated at macro-expansion time (mirroring
///   `FloatModel::new`), so an out-of-range or inverted bound is a compile
///   error, not a runtime panic.
/// * `#[encode(string(max_length = ...))]` — sugar for a `StringModel`.
/// * `#[encode(config = <expr>)]` — the general escape hatch every other form
///   is sugar for: an arbitrary Rust expression, evaluated at runtime and
///   passed as the field's config. Because the expression is arbitrary, it
///   **cannot** be validated at macro-expansion time — a bad expression (bad
///   syntax, or a value of the wrong `Config` type) surfaces as an ordinary
///   compile error at its interpolation site inside the generated impl, rather
///   than a clean macro diagnostic pointing at the attribute.
/// * No attribute — the field's config is `Default::default()` (works for any
///   `Config: Default`, most commonly `Config = ()`).
///
/// For backward compatibility, a single-field tuple variant may instead carry
/// its payload model directly on the variant (`#[encode(float(...))]
/// Foo(f64)`); this is equivalent to putting the attribute on the sole field,
/// and cannot be combined with a field-level attribute on the same field.
///
/// # Enum discriminant weighting
///
/// An enum variant may also carry `#[encode(weight = N)]`, overriding its
/// automatic (payload-cardinality) discriminant weight — see the `minnow`
/// crate documentation for the weighted-enum theory.
///
/// # Generics
///
/// A generic type parameter used directly as a field's type (`inner: T`)
/// automatically gets an `EncodeableCustom` bound (plus a `Config: Default`
/// bound, if that field has no explicit `#[encode(...)]` attribute) added to
/// the generated impl's where-clause. A type parameter used only indirectly
/// (e.g. inside `Option<T>`) needs its bounds spelled out by hand on the type
/// declaration.
#[proc_macro_derive(Encodeable, attributes(encode))]
pub fn derive(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as syn::DeriveInput);

    let receiver = match parse::Receiver::from_derive_input(&derive_input) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };

    let processed = process(receiver);

    write::write(processed).into()
}
