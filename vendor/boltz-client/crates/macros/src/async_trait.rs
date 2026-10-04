use proc_macro2::TokenStream;
use quote::quote;

pub fn async_trait(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(quote! { #[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), async_trait::async_trait)] });
    out.extend(quote! { #[cfg_attr(all(target_family = "wasm", target_os = "unknown"), async_trait::async_trait(?Send))] });
    out.extend(TokenStream::from(input));
    out.into()
}
