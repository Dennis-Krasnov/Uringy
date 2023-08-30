use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn start(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);

    let attributes = &item.attrs;
    let visibility = &item.vis;
    let signature = &item.sig;
    let body = &item.block;

    let result = quote! {
        #(#attributes)*
        #visibility #signature {
            ::uringy::runtime::start(move || #body).unwrap();
        }
    };

    result.into()
}
