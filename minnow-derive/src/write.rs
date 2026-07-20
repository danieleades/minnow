//! Lowering from a processed [`Data`] to the generated impls: a
//! `minnow::Encodeable` codec impl always, plus a `minnow::Bounded`
//! size-reporting impl unless the container opts out with
//! `#[encode(unbounded)]`.
//!
//! A struct's fields and an enum variant's payload are both, mathematically,
//! an anonymous *product* type (see `src/weight.rs` in the `minnow` crate):
//! weight is the product of field weights, worst/best-case bits are the sum
//! of field bits, and the report is a `SizeReport::product` of field reports.
//! [`FieldSpec`]/[`Shape`]/[`product_fields`]/[`product_metrics`] capture that
//! once, and both [`write_struct`] and [`write_enum`] build on top of it, so
//! struct codegen and enum-variant-payload codegen cannot drift apart.
//!
//! Every reference to the runtime crate is spelled through the `minnow`
//! parameter (resolved at expansion time from the invoking crate's manifest —
//! see `crate::minnow_crate_path`), so the derive works when the dependency
//! is renamed.

use std::collections::HashSet;

use darling::export::syn;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::{
    parse,
    process::{Data, EnumData, EnumVariant, StructData, StructStyle},
};

pub fn write(receiver: Data, minnow: &TokenStream) -> TokenStream {
    match receiver {
        Data::Struct(struct_data) => write_struct(struct_data, minnow),
        Data::Enum(enum_data) => write_enum(enum_data, minnow),
    }
}

/// How a product's fields are written back into source.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A tuple constructor / pattern: `Self(a, b)`.
    Positional,
    /// A struct constructor / pattern: `Self { a, b }`.
    Named,
    /// No fields at all: `Self`.
    Unit,
}

/// One field of a product type — a struct's fields, a tuple struct's fields,
/// or an enum variant's payload fields — abstracted over how the field is
/// named/bound so struct and enum-variant codegen can share one
/// implementation.
struct FieldSpec {
    ty: syn::Type,
    /// The runtime config expression passed to `encode_with_config` /
    /// `decode_with_config` (see [`parse::Model::config_tokens`]).
    model: TokenStream,
    /// Whether the field carried an explicit `#[encode(...)]` attribute, as
    /// opposed to falling back to `Default::default()`. Determines which
    /// generic bounds a generic field needs — see [`add_generic_bounds`].
    has_explicit_model: bool,
    /// The name shown in the size-report tree, and used as the decode-side
    /// struct-field key: the field's own identifier, or its positional
    /// index.
    name: String,
    /// The identifier bound to this field's value: the field's own
    /// identifier for named fields, else a synthesised `field_N`.
    binding: syn::Ident,
}

impl FieldSpec {
    fn new(index: usize, field: &parse::Field, minnow: &TokenStream) -> Self {
        let name = field
            .ident
            .as_ref()
            .map_or_else(|| index.to_string(), ToString::to_string);
        let binding = field
            .ident
            .clone()
            .unwrap_or_else(|| format_ident!("field_{index}"));

        Self {
            ty: field.ty.clone(),
            model: field.model(minnow),
            has_explicit_model: field.model.is_some(),
            name,
            binding,
        }
    }
}

/// Lower a [`StructStyle`] (a struct's fields, or an enum variant's payload)
/// to its uniform [`Shape`] and [`FieldSpec`] list.
fn product_fields(style: &StructStyle, minnow: &TokenStream) -> (Shape, Vec<FieldSpec>) {
    match style {
        StructStyle::Tuple(fields) => (
            Shape::Positional,
            fields
                .iter()
                .enumerate()
                .map(|(i, f)| FieldSpec::new(i, f, minnow))
                .collect(),
        ),
        StructStyle::Struct(fields) => (
            Shape::Named,
            fields
                .iter()
                .enumerate()
                .map(|(i, f)| FieldSpec::new(i, f, minnow))
                .collect(),
        ),
        StructStyle::Unit => (Shape::Unit, Vec::new()),
    }
}

/// The `weight`, `worst_case_bits`, `best_case_bits`, and `report`
/// expressions for a product of fields.
struct ProductMetrics {
    /// A `minnow::Weight` expression: the product rule (`∏ W(fieldᵢ)`).
    weight: TokenStream,
    /// An `f64` expression: the sum of per-field worst-case bits.
    worst_case_bits: TokenStream,
    /// An `f64` expression: the sum of per-field best-case bits.
    best_case_bits: TokenStream,
    /// A `minnow::SizeReport` expression: `SizeReport::product` of the
    /// per-field reports, each named.
    report: TokenStream,
}

fn product_metrics(fields: &[FieldSpec], minnow: &TokenStream) -> ProductMetrics {
    let weight_terms = fields.iter().map(|f| {
        let ty = &f.ty;
        let model = &f.model;
        quote_spanned! {ty.span()=>
            * <#ty as #minnow::Bounded>::weight(&(#model))
        }
    });
    let bits_terms = fields.iter().map(|f| {
        let ty = &f.ty;
        let model = &f.model;
        quote_spanned! {ty.span()=>
            + <#ty as #minnow::Bounded>::worst_case_bits(&(#model))
        }
    });
    let best_bits_terms = fields.iter().map(|f| {
        let ty = &f.ty;
        let model = &f.model;
        quote_spanned! {ty.span()=>
            + <#ty as #minnow::Bounded>::best_case_bits(&(#model))
        }
    });
    let report_children = fields.iter().map(|f| {
        let ty = &f.ty;
        let model = &f.model;
        let name = &f.name;
        quote_spanned! {ty.span()=>
            <#ty as #minnow::Bounded>::report(&(#model)).with_name(#name)
        }
    });

    ProductMetrics {
        // Product rule: the type's cardinality is the product of its fields'.
        weight: quote! { #minnow::Weight::ONE #( #weight_terms )* },
        worst_case_bits: quote! { 0.0_f64 #( #bits_terms )* },
        best_case_bits: quote! { 0.0_f64 #( #best_bits_terms )* },
        report: quote! { #minnow::SizeReport::product(::std::vec![ #( #report_children ),* ]) },
    }
}

/// Build the encode statements for a product's fields, given a closure that
/// produces the `&FieldType` expression accessing field `i`.
fn encode_stmts(
    fields: &[FieldSpec],
    minnow: &TokenStream,
    accessor: impl Fn(usize, &FieldSpec) -> TokenStream,
) -> TokenStream {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let ty = &f.ty;
            let model = &f.model;
            let access = accessor(i, f);
            quote_spanned! {ty.span()=>
                #minnow::Encodeable::encode_with_config(#access, visitor, #model)?;
            }
        })
        .collect()
}

/// Build the constructor expression for decoding a product: `path`,
/// `path(...)`, or `path { ... }` depending on `shape`, decoding each field
/// in order.
fn decode_ctor(
    path: &TokenStream,
    shape: Shape,
    fields: &[FieldSpec],
    minnow: &TokenStream,
) -> TokenStream {
    let decoded = fields.iter().map(|f| {
        let ty = &f.ty;
        let model = &f.model;
        quote_spanned! {ty.span()=>
            <#ty as #minnow::Encodeable>::decode_with_config(visitor, #model)?
        }
    });
    match shape {
        Shape::Unit => quote! { #path },
        Shape::Positional => quote! { #path( #( #decoded ),* ) },
        Shape::Named => {
            let bindings = fields.iter().map(|f| &f.binding);
            quote! { #path { #( #bindings: #decoded ),* } }
        }
    }
}

/// Which trait a generated impl needs its generic field types to implement.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldBound {
    /// The codec alone: `T: minnow::Encodeable`.
    Encodeable,
    /// The codec plus size reporting: `T: minnow::Bounded` (which implies
    /// `Encodeable`). Needed by every `Bounded` impl, and by enum codec impls
    /// for automatically-weighted variants, whose shared discriminant model
    /// is built from payload cardinalities.
    Bounded,
}

/// Add the trait bounds a generated impl needs on its type parameters.
///
/// For every type parameter used as *some field's type directly* (`inner:
/// T`), this adds `T: minnow::Encodeable` or `T: minnow::Bounded` (per
/// `bound`), plus `<T as minnow::Encodeable>::Config: Default` when that
/// field carries no explicit `#[encode(...)]` attribute (its config then
/// lowers to `Default::default()` — see [`parse::Model::config_tokens`]).
///
/// `seen` is shared across calls targeting the same impl so a parameter is
/// bounded at most once; callers that need a mix of bound strengths (see
/// [`write_enum`]) must pass the `Bounded`-requiring fields first, since the
/// first bound recorded for a parameter wins.
///
/// This only recognises a field whose type is *exactly* a bare type
/// parameter; a field that merely mentions one (e.g. `Option<T>`) needs its
/// bound spelled out by hand on the type declaration, which the where-clause
/// this appends to still carries through untouched.
fn add_generic_bounds(
    generics: &mut syn::Generics,
    seen: &mut HashSet<syn::Ident>,
    fields: &[FieldSpec],
    bound: FieldBound,
    minnow: &TokenStream,
) {
    let type_params: HashSet<syn::Ident> =
        generics.type_params().map(|tp| tp.ident.clone()).collect();
    if type_params.is_empty() {
        return;
    }

    let where_clause = generics.make_where_clause();
    for field in fields {
        let syn::Type::Path(type_path) = &field.ty else {
            continue;
        };
        let Some(ident) = type_path.path.get_ident() else {
            continue;
        };
        if !type_params.contains(ident) || !seen.insert(ident.clone()) {
            continue;
        }

        match bound {
            FieldBound::Encodeable => where_clause
                .predicates
                .push(syn::parse_quote! { #ident: #minnow::Encodeable }),
            FieldBound::Bounded => where_clause
                .predicates
                .push(syn::parse_quote! { #ident: #minnow::Bounded }),
        }
        if !field.has_explicit_model {
            where_clause.predicates.push(syn::parse_quote! {
                <#ident as #minnow::Encodeable>::Config: ::core::default::Default
            });
        }
    }
}

fn write_struct(struct_data: StructData, minnow: &TokenStream) -> TokenStream {
    let (shape, fields) = product_fields(&struct_data.fields, minnow);
    let metrics = product_metrics(&fields, minnow);

    let encode_body = encode_stmts(&fields, minnow, |i, f| match shape {
        Shape::Named => {
            let ident = &f.binding;
            quote! { &self.#ident }
        }
        Shape::Positional => {
            let idx = syn::Index::from(i);
            quote! { &self.#idx }
        }
        Shape::Unit => TokenStream::new(),
    });

    let decode_ctor_expr = decode_ctor(&quote! { Self }, shape, &fields, minnow);

    let mut codec_generics = struct_data.generics.clone();
    add_generic_bounds(
        &mut codec_generics,
        &mut HashSet::new(),
        &fields,
        FieldBound::Encodeable,
        minnow,
    );
    let (impl_generics, ty_generics, where_clause) = codec_generics.split_for_impl();

    let ident = struct_data.ident;

    let encodeable_impl = quote! {
        impl #impl_generics #minnow::Encodeable for #ident #ty_generics #where_clause {
            type Config = ();

            fn encode_with_config<W>(&self, visitor: &mut #minnow::EncodeVisitor<W>, _config: ()) -> ::core::result::Result<(), #minnow::EncodeError>
            where
                W: #minnow::__private::BitWrite,
            {
                #encode_body
                Ok(())
            }

            fn decode_with_config<R>(visitor: &mut #minnow::DecodeVisitor<R>, _config: ()) -> ::core::result::Result<Self, #minnow::DecodeError>
            where
                R: #minnow::__private::BitRead,
                Self: Sized,
            {
                Ok(#decode_ctor_expr)
            }
        }
    };

    let bounded_impl = if struct_data.unbounded {
        TokenStream::new()
    } else {
        let mut bounded_generics = struct_data.generics;
        add_generic_bounds(
            &mut bounded_generics,
            &mut HashSet::new(),
            &fields,
            FieldBound::Bounded,
            minnow,
        );
        let (impl_generics, ty_generics, where_clause) = bounded_generics.split_for_impl();

        let weight_body = metrics.weight;
        let worst_case_body = metrics.worst_case_bits;
        let best_case_body = metrics.best_case_bits;
        let report_body = metrics.report;

        quote! {
            impl #impl_generics #minnow::Bounded for #ident #ty_generics #where_clause {
                fn weight(_config: &Self::Config) -> #minnow::Weight {
                    #weight_body
                }

                fn worst_case_bits(_config: &Self::Config) -> f64 {
                    #worst_case_body
                }

                fn best_case_bits(_config: &Self::Config) -> f64 {
                    #best_case_body
                }

                fn report(_config: &Self::Config) -> #minnow::SizeReport {
                    #report_body
                }
            }
        }
    };

    quote! {
        #encodeable_impl
        #bounded_impl
    }
}

/// The per-variant expressions needed to generate an enum's `Encodeable` and
/// `Bounded` impls.
struct VariantParts {
    /// Encode match arm.
    encode_arm: TokenStream,
    /// Decode match arm.
    decode_arm: TokenStream,
    /// Interval width (a `u128`) for the weighted discriminant model. Uses the
    /// `#[encode(weight = N)]` override when present, else the payload
    /// cardinality.
    discriminant_weight: TokenStream,
    /// The variant's true cardinality (a [`minnow::Weight`]), used for the
    /// type's `weight()`. Ignores any discriminant override — an override is a
    /// coding choice, not a change to the number of distinct values.
    cardinality: TokenStream,
    /// The payload's worst-case bits (an `f64`), zero for unit variants.
    payload_bits: TokenStream,
    /// The payload's best-case bits (an `f64`), zero for unit variants.
    payload_best_bits: TokenStream,
    /// A [`minnow::SizeReport`] node for the variant.
    report_child: TokenStream,
}

fn variant_parts(index: usize, variant: &EnumVariant, minnow: &TokenStream) -> VariantParts {
    let ident = &variant.ident;
    // A checked conversion rather than `as`: a panic in a proc macro is a
    // compile error, so an absurd variant count fails loudly instead of
    // silently truncating the discriminant space.
    let symbol = u32::try_from(index).expect("more than u32::MAX enum variants");
    let idx = syn::Index::from(index);
    let name = ident.to_string();

    let (shape, fields) = product_fields(&variant.style, minnow);
    let metrics = product_metrics(&fields, minnow);

    let path = quote! { Self::#ident };
    let pattern = match shape {
        Shape::Unit => path.clone(),
        Shape::Positional => {
            let bindings = fields.iter().map(|f| &f.binding);
            quote! { #path( #( #bindings ),* ) }
        }
        Shape::Named => {
            let bindings = fields.iter().map(|f| &f.binding);
            quote! { #path { #( #bindings ),* } }
        }
    };

    // Under `match self { ... }` with `self: &Self`, every bound field name is
    // already a `&FieldType` thanks to match ergonomics, so it can be passed
    // straight through as the encode accessor.
    let encode_field_stmts = encode_stmts(&fields, minnow, |_, f| {
        let binding = &f.binding;
        quote! { #binding }
    });
    let encode_arm = quote! {
        #pattern => {
            visitor.encode_one(model, &#symbol)?;
            #encode_field_stmts
            Ok(())
        }
    };

    let decode_ctor_expr = decode_ctor(&path, shape, &fields, minnow);
    let decode_arm = quote! {
        #symbol => ::core::result::Result::Ok(#decode_ctor_expr),
    };

    // The manual override, as a `u128` literal, if present; else the
    // payload's true cardinality (wrapped in parens: `payload_weight` is a
    // `*`/`+` expression tree, and `.get()` must apply to the whole thing).
    let payload_weight = &metrics.weight;
    let discriminant_weight = variant
        .weight_override
        .map_or_else(|| quote! { (#payload_weight).get() }, |w| quote! { #w });

    let payload_report = &metrics.report;
    let report_child = quote! {
        #minnow::SizeReport::enum_variant(
            #name,
            model.discriminant_bits(#idx),
            #payload_report,
        )
    };

    VariantParts {
        encode_arm,
        decode_arm,
        discriminant_weight,
        cardinality: metrics.weight,
        payload_bits: metrics.worst_case_bits,
        payload_best_bits: metrics.best_case_bits,
        report_child,
    }
}

fn write_enum(enum_data: EnumData, minnow: &TokenStream) -> TokenStream {
    let parts: Vec<VariantParts> = enum_data
        .variants
        .iter()
        .enumerate()
        .map(|(i, v)| variant_parts(i, v, minnow))
        .collect();

    let ident = enum_data.ident;

    let all_fields: Vec<FieldSpec> = enum_data
        .variants
        .iter()
        .flat_map(|v| product_fields(&v.style, minnow).1)
        .collect();
    // Payload fields of *automatically-weighted* variants: the shared
    // discriminant model uses their cardinalities, so even the codec impl
    // needs `Bounded` for them. (An `#[encode(unbounded)]` enum has an
    // explicit weight on every variant — enforced at parse time — so this is
    // empty there and the codec needs only `Encodeable`.)
    let auto_weighted_fields: Vec<FieldSpec> = enum_data
        .variants
        .iter()
        .filter(|v| v.weight_override.is_none())
        .flat_map(|v| product_fields(&v.style, minnow).1)
        .collect();

    let mut codec_generics = enum_data.generics.clone();
    {
        let mut seen = HashSet::new();
        add_generic_bounds(
            &mut codec_generics,
            &mut seen,
            &auto_weighted_fields,
            FieldBound::Bounded,
            minnow,
        );
        add_generic_bounds(
            &mut codec_generics,
            &mut seen,
            &all_fields,
            FieldBound::Encodeable,
            minnow,
        );
    }
    let (impl_generics, ty_generics, where_clause) = codec_generics.split_for_impl();

    let encode_arms = parts.iter().map(|p| &p.encode_arm);
    let decode_arms = parts.iter().map(|p| &p.decode_arm);

    // The discriminant-model constructor is built once and interpolated into
    // every generated method body, so encode, decode, and the bit-accounting
    // methods cannot drift apart in how they weight the discriminant.
    let discriminant_weights = parts.iter().map(|p| &p.discriminant_weight);
    let model_ctor = quote! {
        #minnow::WeightedModel::new([ #( #discriminant_weights ),* ])
    };

    let encodeable_impl = quote! {
        impl #impl_generics #minnow::Encodeable for #ident #ty_generics #where_clause {
            type Config = ();

            fn encode_with_config<W>(&self, visitor: &mut #minnow::EncodeVisitor<W>, _config: ()) -> ::core::result::Result<(), #minnow::EncodeError>
            where
                W: #minnow::__private::BitWrite {
                let model = #model_ctor;
                match self {
                    #( #encode_arms )*
                }
            }

            fn decode_with_config<R>(visitor: &mut #minnow::DecodeVisitor<R>, _config: ()) -> ::core::result::Result<Self, #minnow::DecodeError>
            where
                R: #minnow::__private::BitRead,
                Self: Sized {
                let model = #model_ctor;
                match visitor.decode_one(model)? {
                    #( #decode_arms )*
                    other => ::core::result::Result::Err(#minnow::DecodeError::InvalidSymbol { symbol: u128::from(other) }),
                }
            }
        }
    };

    let bounded_impl = if enum_data.unbounded {
        TokenStream::new()
    } else {
        enum_bounded_impl(
            enum_data.generics,
            &ident,
            &parts,
            &all_fields,
            &model_ctor,
            minnow,
        )
    };

    quote! {
        #encodeable_impl
        #bounded_impl
    }
}

/// The generated `minnow::Bounded` impl for an enum: the sum-rule `weight`,
/// max/min-over-variants bit bounds, and the per-variant report — all built
/// around the same `model_ctor` the codec impl uses, so the two cannot drift
/// apart in how they weight the discriminant.
fn enum_bounded_impl(
    generics: syn::Generics,
    ident: &syn::Ident,
    parts: &[VariantParts],
    all_fields: &[FieldSpec],
    model_ctor: &TokenStream,
    minnow: &TokenStream,
) -> TokenStream {
    let mut generics = generics;
    add_generic_bounds(
        &mut generics,
        &mut HashSet::new(),
        all_fields,
        FieldBound::Bounded,
        minnow,
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let cardinalities = parts.iter().map(|p| &p.cardinality);

    // `worst_case_bits`: max over variants of (discriminant bits + payload bits).
    let worst_case_terms = parts.iter().enumerate().map(|(i, p)| {
        let idx = syn::Index::from(i);
        let payload_bits = &p.payload_bits;
        quote! {
            worst = f64::max(worst, model.discriminant_bits(#idx) + #payload_bits);
        }
    });

    // `best_case_bits`: min over variants of (discriminant bits + payload bits).
    let best_case_terms = parts.iter().enumerate().map(|(i, p)| {
        let idx = syn::Index::from(i);
        let payload_best = &p.payload_best_bits;
        quote! {
            best = f64::min(best, model.discriminant_bits(#idx) + #payload_best);
        }
    });

    let report_children = parts.iter().map(|p| &p.report_child);

    quote! {
        impl #impl_generics #minnow::Bounded for #ident #ty_generics #where_clause {
            fn weight(_config: &Self::Config) -> #minnow::Weight {
                // Sum rule: the type's cardinality is the sum of its variants'.
                #minnow::Weight::ZERO #( + #cardinalities )*
            }

            fn worst_case_bits(_config: &Self::Config) -> f64 {
                let model = #model_ctor;
                let mut worst = f64::NEG_INFINITY;
                #( #worst_case_terms )*
                worst
            }

            fn best_case_bits(_config: &Self::Config) -> f64 {
                let model = #model_ctor;
                let mut best = f64::INFINITY;
                #( #best_case_terms )*
                best
            }

            fn report(_config: &Self::Config) -> #minnow::SizeReport {
                let model = #model_ctor;
                #minnow::SizeReport::sum(::std::vec![ #( #report_children ),* ])
            }
        }
    }
}
