extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(DynamicScript)]
pub fn _derive_dynamic_script(input: TokenStream) -> TokenStream {
    derive_dynamic_script(input)
}

fn derive_dynamic_script(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    TokenStream::from(quote! {
        #[no_mangle]
        pub fn _create_script() -> *mut dyn bevytor_script::Script {
            let object = #struct_name {};
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    })
}
