use proc_macro2::TokenStream;
use quote::quote;

pub fn async_test_not_wasm(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(quote! { #[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), tokio::test)] });
    out.extend(TokenStream::from(input));
    out.into()
}

pub fn async_test_wasm(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(quote! { #[cfg_attr(all(target_family = "wasm", target_os = "unknown"), wasm_bindgen_test::wasm_bindgen_test)] });
    out.extend(TokenStream::from(input));
    out.into()
}

pub fn async_test_all(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(quote! { #[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), tokio::test)] });
    out.extend(quote! { #[cfg_attr(all(target_family = "wasm", target_os = "unknown"), wasm_bindgen_test::wasm_bindgen_test)] });
    out.extend(TokenStream::from(input));
    out.into()
}

pub fn test_not_wasm(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(
        quote! { #[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)] },
    );
    out.extend(TokenStream::from(input));
    out.into()
}

pub fn test_wasm(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(
        quote! { #[cfg_attr(all(target_family = "wasm", target_os = "unknown"), wasm_bindgen_test::wasm_bindgen_test)] },
    );
    out.extend(TokenStream::from(input));
    out.into()
}

pub fn test_all(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut out = TokenStream::new();
    out.extend(
        quote! { #[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)] },
    );
    out.extend(quote! { #[cfg_attr(all(target_family = "wasm", target_os = "unknown"), wasm_bindgen_test::wasm_bindgen_test)] });
    out.extend(TokenStream::from(input));
    out.into()
}
