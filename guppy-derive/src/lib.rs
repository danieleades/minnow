use darling::FromDeriveInput;
use proc_macro::{TokenStream};
use process::process;
use syn::parse_macro_input;

mod parse;
mod process;
mod write;

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