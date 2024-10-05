extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn process_video(_input: TokenStream) -> TokenStream {}
