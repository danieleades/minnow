//! Derive macro for the [Minnow](https://github.com/danieleades/minnow) crate

#![deny(clippy::all, clippy::cargo)]
#![warn(clippy::pedantic)]

use darling::{FromDeriveInput, export::syn};
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
///
///   ```rust,ignore
///   #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
///   pub x: f64,
///   ```
///
/// * `#[encode(int(min = ..., max = ...))]` — sugar for an [`IntModel`](https://docs.rs/minnow/latest/minnow/struct.IntModel.html);
///   the target integer type is inferred from the field, and bounds are
///   validated at macro-expansion time exactly like `float`.
///
///   ```rust,ignore
///   #[encode(int(min = 0, max = 200))]
///   pub age: u8,
///   ```
///
/// * `#[encode(string(max_length = ...))]` — sugar for a [`StringModel`](https://docs.rs/minnow/latest/minnow/struct.StringModel.html)
///   (`max_length` in bytes).
///
///   ```rust,ignore
///   #[encode(string(max_length = 64))]
///   pub name: String,
///   ```
///
/// * `#[encode(seq(max_len = ..., elem = <expr>))]` — sugar for a [`SeqModel`](https://docs.rs/minnow/latest/minnow/struct.SeqModel.html)
///   wrapping a bounded `Vec<T>`. `elem` is the per-element config expression
///   and may be omitted when `T::Config: Default`.
///
///   ```rust,ignore
///   #[encode(seq(max_len = 16))]
///   pub flags: Vec<bool>,
///
///   #[encode(seq(max_len = 8, elem = float_model()))]
///   pub samples: Vec<f64>,
///   ```
///
/// * `#[encode(config = <expr>)]` — the general escape hatch every other form
///   is sugar for: an arbitrary Rust expression, evaluated at runtime and
///   passed as the field's config. Because the expression is arbitrary, it
///   **cannot** be validated at macro-expansion time — a bad expression (bad
///   syntax, or a value of the wrong `Config` type) surfaces as an ordinary
///   compile error at its interpolation site inside the generated impl, rather
///   than a clean macro diagnostic pointing at the attribute.
///
///   ```rust,ignore
///   #[encode(config = my_crate::custom_model())]
///   pub reading: MyType,
///   ```
///
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
/// automatic (payload-cardinality) discriminant weight:
///
/// ```rust,ignore
/// #[derive(Encodeable)]
/// enum Reading {
///     #[encode(weight = 1000)]
///     Common,
///     Rare,
/// }
/// ```
///
/// See the `minnow` crate documentation ("How Minnow compresses" in the
/// README) for the weighted-enum theory: this is the same mechanism behind
/// automatic weighting, so it composes with both the worst-case-optimal
/// default and a manually-supplied prior.
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
