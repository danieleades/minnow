use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::spanned::Spanned;

use crate::process::{Data, EnumData, EnumStyle, StructData, StructStyle};



pub fn write(receiver: Data) -> TokenStream {

    match receiver {
        Data::Struct(struct_data) => write_struct(struct_data),
        Data::Enum(enum_data) => write_enum(enum_data),
    }
}

fn write_enum(enum_data: EnumData) -> TokenStream {
    let len = enum_data.variants.len() as u32;

    let encode_block: TokenStream = enum_data.variants.iter().enumerate().map(|(i, variant)| {

        let ident = &variant.ident;
        let symbol = i as u32;

        match &variant.style {
            EnumStyle::Tuple(tuple) => {
                let inner_model = tuple.model();
                let ty = &tuple.ty;
                let ty_span = ty.span();

                quote_spanned! {ty_span=>
                     Self:: #ident (x) => {
                        visitor.encode_one(model, & #symbol)?;
                        x.encode_with_config(visitor, #inner_model)
                     }
                 }
            }
            EnumStyle::Struct(_) => todo!(),
            EnumStyle::Unit => {
                quote! {
                     Self:: #ident => visitor.encode_one(model, & #symbol),
                 }
            },
        }
    }).collect();

    let decode_block: TokenStream = enum_data.variants.iter().enumerate().map(|(i, variant)| {

        let ident = &variant.ident;
        let symbol = i as u32;

        match &variant.style {
            EnumStyle::Tuple(tuple) => {
                let inner_ty = &tuple.ty;
                let ty_span = inner_ty.span();
                let inner_model = tuple.model();
                quote_spanned! {ty_span=>
                    #symbol => Ok(Self:: #ident (<#inner_ty> ::decode_with_config(visitor, #inner_model)?)),
                }
            }
            EnumStyle::Struct(_) => todo!(),
            EnumStyle::Unit => {
                quote! {
                    #symbol => Ok(Self:: #ident),
                }
            },
        }
    }).collect();

    let ident = enum_data.ident;

    quote! {
        impl guppy::EncodeableCustom for #ident {
            type Config = ();
            fn encode_with_config<W>(&self, visitor: &mut guppy::EncodeVisitor<W>, _config: ()) -> std::io::Result<()>
            where
                W: bitstream_io::BitWrite {
        
                let model = guppy::OneShot::< #len >::default();
                match self {
                    #encode_block
                }
            }
        
            fn decode_with_config<R>(visitor: &mut guppy::DecodeVisitor<R>, _config: ()) -> std::io::Result<Self>
            where
                R: bitstream_io::BitRead,
                Self: Sized {
                    let model = guppy::OneShot::< #len >::default();
                    match visitor.decode_one(model)? {
                        #decode_block
                        _ => unreachable!(),
                    }
            }
        }
    }
}

fn write_struct(struct_data: StructData) -> TokenStream {

    let encode_block: TokenStream = match &struct_data.fields {
        StructStyle::Tuple(fields) => {
            let encode_fields: TokenStream = fields.iter().enumerate().map(|(i, field)| {
                let model = field.model();
                let i = syn::Index::from(i);
                quote! {
                    self. #i .encode_with_config(visitor, #model)?;
                }
            }).collect();
            quote! {
                fn encode_with_config<W>(&self, visitor: &mut guppy::EncodeVisitor<W>, _config: ()) -> std::io::Result<()>
                where
                    W: bitstream_io::BitWrite,
                {
                    #encode_fields
                    Ok(())
                }
            }

        },
        StructStyle::Struct(fields) => {
            let encode_fields: TokenStream = fields.iter().map(|field| {
                let ident = field.ident.as_ref().unwrap();
                let model = field.model();
                quote! {
                    self. #ident .encode_with_config(visitor, #model)?;
                }
            }).collect();
            quote! {
                fn encode_with_config<W>(&self, visitor: &mut guppy::EncodeVisitor<W>, _config: ()) -> std::io::Result<()>
                where
                    W: bitstream_io::BitWrite,
                {
                    #encode_fields
                    Ok(())
                }
            }
        },
        StructStyle::Unit => TokenStream::default(),
    };

    let decode_block: TokenStream = match &struct_data.fields {
        StructStyle::Tuple(fields) => {
            let decode_fields: TokenStream = fields.iter().map(|field| {
                let model = field.model();
                let ty = &field.ty;
                quote! {
                    <#ty>::decode_with_config(visitor, #model)?,
                }
            }).collect();

            quote! {
                fn decode_with_config<R>(visitor: &mut guppy::DecodeVisitor<R>, config: ()) -> std::io::Result<Self>
                where
                    R: bitstream_io::BitRead,
                    Self: Sized,
                {
                    Ok(Self (
                        #decode_fields
                    ))
                }
            }
        },
        StructStyle::Struct(fields) => {
            let decode_fields: TokenStream = fields.iter().map(|field| {
                let ident = field.ident.as_ref().unwrap();
                let ty = &field.ty;
                let model = field.model();
                quote! {
                    #ident : <#ty>::decode_with_config(visitor, #model )?,
                }
            }).collect();

            quote! {
                fn decode_with_config<R>(visitor: &mut guppy::DecodeVisitor<R>, config: ()) -> std::io::Result<Self>
                where
                    R: bitstream_io::BitRead,
                    Self: Sized,
                {
                    Ok(Self {
                        #decode_fields
                    })
                }
            }
        },
        StructStyle::Unit => todo!(),
    };

    let ident = struct_data.ident;
    let generics = struct_data.generics;

    quote! {
        impl guppy::EncodeableCustom for #ident #generics {
            type Config = ();
            
            #encode_block
        
            #decode_block
        }
    }
}