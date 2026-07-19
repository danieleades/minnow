use darling::export::syn;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::process::{Data, EnumData, EnumStyle, StructData, StructStyle};

pub fn write(receiver: Data) -> TokenStream {
    match receiver {
        Data::Struct(struct_data) => write_struct(struct_data),
        Data::Enum(enum_data) => write_enum(enum_data),
    }
}

/// The per-variant expressions needed to generate an enum's `EncodeableCustom`
/// impl.
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

fn variant_parts(index: usize, variant: &crate::process::EnumVariant) -> VariantParts {
    let ident = &variant.ident;
    // A checked conversion rather than `as`: a panic in a proc macro is a
    // compile error, so an absurd variant count fails loudly instead of
    // silently truncating the discriminant space.
    let symbol = u32::try_from(index).expect("more than u32::MAX enum variants");
    let idx = syn::Index::from(index);
    let name = ident.to_string();

    // The manual override, as a `u128` literal, if present.
    let override_weight = variant.weight_override.map(|w| quote! { #w });

    match &variant.style {
        EnumStyle::Tuple(tuple) => {
            let ty = &tuple.ty;
            let ty_span = ty.span();
            let model = tuple.model();

            let payload_weight = quote! {
                <#ty as minnow::EncodeableCustom>::weight(&(#model))
            };
            let discriminant_weight =
                override_weight.unwrap_or_else(|| quote! { #payload_weight.get() });

            VariantParts {
                encode_arm: quote_spanned! {ty_span=>
                    Self::#ident(x) => {
                        visitor.encode_one(model, &#symbol)?;
                        minnow::EncodeableCustom::encode_with_config(x, visitor, #model)
                    }
                },
                decode_arm: quote_spanned! {ty_span=>
                    #symbol => ::core::result::Result::Ok(Self::#ident(
                        <#ty as minnow::EncodeableCustom>::decode_with_config(visitor, #model)?,
                    )),
                },
                discriminant_weight,
                cardinality: payload_weight,
                payload_bits: quote! {
                    <#ty as minnow::EncodeableCustom>::worst_case_bits(&(#model))
                },
                payload_best_bits: quote! {
                    <#ty as minnow::EncodeableCustom>::best_case_bits(&(#model))
                },
                report_child: quote! {
                    minnow::SizeReport::enum_variant(
                        #name,
                        model.discriminant_bits(#idx),
                        <#ty as minnow::EncodeableCustom>::report(&(#model)),
                    )
                },
            }
        }
        EnumStyle::Unit => {
            let discriminant_weight = override_weight.unwrap_or_else(|| quote! { 1u128 });

            VariantParts {
                encode_arm: quote! {
                    Self::#ident => visitor.encode_one(model, &#symbol),
                },
                decode_arm: quote! {
                    #symbol => ::core::result::Result::Ok(Self::#ident),
                },
                discriminant_weight,
                // A unit variant always has exactly one value, regardless of any
                // discriminant-weight override.
                cardinality: quote! { minnow::Weight::ONE },
                payload_bits: quote! { 0.0_f64 },
                payload_best_bits: quote! { 0.0_f64 },
                report_child: quote! {
                    minnow::SizeReport::leaf(model.discriminant_bits(#idx)).with_name(#name)
                },
            }
        }
    }
}

fn write_enum(enum_data: EnumData) -> TokenStream {
    let parts: Vec<VariantParts> = enum_data
        .variants
        .iter()
        .enumerate()
        .map(|(i, v)| variant_parts(i, v))
        .collect();

    let ident = enum_data.ident;

    let encode_arms = parts.iter().map(|p| &p.encode_arm);
    let decode_arms = parts.iter().map(|p| &p.decode_arm);
    let cardinalities = parts.iter().map(|p| &p.cardinality);

    // The discriminant-model constructor is built once and interpolated into
    // every generated method body, so encode, decode, and the bit-accounting
    // methods cannot drift apart in how they weight the discriminant.
    let discriminant_weights = parts.iter().map(|p| &p.discriminant_weight);
    let model_ctor = quote! {
        minnow::WeightedModel::new([ #( #discriminant_weights ),* ])
    };

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
        impl minnow::EncodeableCustom for #ident {
            type Config = ();

            fn weight(_config: &Self::Config) -> minnow::Weight {
                // Sum rule: the type's cardinality is the sum of its variants'.
                minnow::Weight::ZERO #( + #cardinalities )*
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

            fn report(_config: &Self::Config) -> minnow::SizeReport {
                let model = #model_ctor;
                minnow::SizeReport::sum(::std::vec![ #( #report_children ),* ])
            }

            fn encode_with_config<W>(&self, visitor: &mut minnow::EncodeVisitor<W>, _config: ()) -> std::io::Result<()>
            where
                W: bitstream_io::BitWrite {
                let model = #model_ctor;
                match self {
                    #( #encode_arms )*
                }
            }

            fn decode_with_config<R>(visitor: &mut minnow::DecodeVisitor<R>, _config: ()) -> ::core::result::Result<Self, minnow::DecodeError>
            where
                R: bitstream_io::BitRead,
                Self: Sized {
                let model = #model_ctor;
                match visitor.decode_one(model)? {
                    #( #decode_arms )*
                    other => ::core::result::Result::Err(minnow::DecodeError::InvalidSymbol { symbol: u128::from(other) }),
                }
            }
        }
    }
}

/// The `weight`, `worst_case_bits`, `best_case_bits` and `report` method bodies
/// for a struct.
struct StructMetrics {
    weight: TokenStream,
    worst_case_bits: TokenStream,
    best_case_bits: TokenStream,
    report: TokenStream,
}

fn struct_metrics(fields: &StructStyle) -> StructMetrics {
    // Collect per-field `(type, model, name)` for the metric methods.
    let entries: Vec<(TokenStream, TokenStream, String)> = match fields {
        StructStyle::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let ty = &field.ty;
                (quote! { #ty }, field.model(), i.to_string())
            })
            .collect(),
        StructStyle::Struct(fields) => fields
            .iter()
            .map(|field| {
                let ty = &field.ty;
                let name = field.ident.as_ref().unwrap().to_string();
                (quote! { #ty }, field.model(), name)
            })
            .collect(),
        StructStyle::Unit => Vec::new(),
    };

    let weight_terms = entries.iter().map(|(ty, model, _)| {
        quote! { * <#ty as minnow::EncodeableCustom>::weight(&(#model)) }
    });
    let bits_terms = entries.iter().map(|(ty, model, _)| {
        quote! { + <#ty as minnow::EncodeableCustom>::worst_case_bits(&(#model)) }
    });
    let best_bits_terms = entries.iter().map(|(ty, model, _)| {
        quote! { + <#ty as minnow::EncodeableCustom>::best_case_bits(&(#model)) }
    });
    let report_children = entries.iter().map(|(ty, model, name)| {
        quote! {
            <#ty as minnow::EncodeableCustom>::report(&(#model)).with_name(#name)
        }
    });

    StructMetrics {
        // Product rule: the type's cardinality is the product of its fields'.
        weight: quote! { minnow::Weight::ONE #( #weight_terms )* },
        worst_case_bits: quote! { 0.0_f64 #( #bits_terms )* },
        best_case_bits: quote! { 0.0_f64 #( #best_bits_terms )* },
        report: quote! { minnow::SizeReport::product(::std::vec![ #( #report_children ),* ]) },
    }
}

#[allow(clippy::too_many_lines)]
fn write_struct(struct_data: StructData) -> TokenStream {
    let encode_block: TokenStream = match &struct_data.fields {
        StructStyle::Tuple(fields) => {
            let encode_fields: TokenStream = fields
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let model = field.model();
                    let i = syn::Index::from(i);
                    quote! {
                        minnow::EncodeableCustom::encode_with_config(& self. #i, visitor, #model)?;
                    }
                })
                .collect();
            quote! {
                fn encode_with_config<W>(&self, visitor: &mut minnow::EncodeVisitor<W>, _config: ()) -> std::io::Result<()>
                where
                    W: bitstream_io::BitWrite,
                {
                    #encode_fields
                    Ok(())
                }
            }
        }
        StructStyle::Struct(fields) => {
            let encode_fields: TokenStream = fields
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let model = field.model();
                    quote! {
                        minnow::EncodeableCustom::encode_with_config(& self. #ident, visitor, #model)?;
                    }
                })
                .collect();
            quote! {
                fn encode_with_config<W>(&self, visitor: &mut minnow::EncodeVisitor<W>, _config: ()) -> std::io::Result<()>
                where
                    W: bitstream_io::BitWrite,
                {
                    #encode_fields
                    Ok(())
                }
            }
        }
        StructStyle::Unit => TokenStream::default(),
    };

    let decode_block: TokenStream = match &struct_data.fields {
        StructStyle::Tuple(fields) => {
            let decode_fields: TokenStream = fields
                .iter()
                .map(|field| {
                    let model = field.model();
                    let ty = &field.ty;
                    quote! {
                        <#ty as minnow::EncodeableCustom>::decode_with_config(visitor, #model)?,
                    }
                })
                .collect();

            quote! {
                fn decode_with_config<R>(visitor: &mut minnow::DecodeVisitor<R>, config: ()) -> ::core::result::Result<Self, minnow::DecodeError>
                where
                    R: bitstream_io::BitRead,
                    Self: Sized,
                {
                    Ok(Self (
                        #decode_fields
                    ))
                }
            }
        }
        StructStyle::Struct(fields) => {
            let decode_fields: TokenStream = fields
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let ty = &field.ty;
                    let model = field.model();
                    quote! {
                        #ident : <#ty as minnow::EncodeableCustom>::decode_with_config(visitor, #model )?,
                    }
                })
                .collect();

            quote! {
                fn decode_with_config<R>(visitor: &mut minnow::DecodeVisitor<R>, config: ()) -> ::core::result::Result<Self, minnow::DecodeError>
                where
                    R: bitstream_io::BitRead,
                    Self: Sized,
                {
                    Ok(Self {
                        #decode_fields
                    })
                }
            }
        }
        StructStyle::Unit => todo!(),
    };

    let metrics = struct_metrics(&struct_data.fields);
    let weight_body = metrics.weight;
    let worst_case_body = metrics.worst_case_bits;
    let best_case_body = metrics.best_case_bits;
    let report_body = metrics.report;

    let ident = struct_data.ident;
    let generics = struct_data.generics;

    quote! {
        impl minnow::EncodeableCustom for #ident #generics {
            type Config = ();

            fn weight(_config: &Self::Config) -> minnow::Weight {
                #weight_body
            }

            fn worst_case_bits(_config: &Self::Config) -> f64 {
                #worst_case_body
            }

            fn best_case_bits(_config: &Self::Config) -> f64 {
                #best_case_body
            }

            fn report(_config: &Self::Config) -> minnow::SizeReport {
                #report_body
            }

            #encode_block

            #decode_block
        }
    }
}
