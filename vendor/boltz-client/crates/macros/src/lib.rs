mod async_trait;
mod derive_from;
mod derive_into;
mod testing;
mod wasm_bindgen;

use proc_macro::TokenStream;

/// Attribute macro switch `async_trait` usage depending on WASM target
#[proc_macro_attribute]
pub fn async_trait(args: TokenStream, input: TokenStream) -> TokenStream {
    async_trait::async_trait(args, input)
}

/// Attribute macro to mirror the external struct/enum in WASM
///
/// ```rust,ignore
/// #[macros::extern_wasm_bindgen(sdk_common::prelude::RouteHint)]
/// pub struct RouteHint {
///     pub hops: Vec<RouteHintHop>,
/// }
/// ```
/// Generates in WASM typescript:
/// ```typescript
/// export interface RouteHint {
///     hops: RouteHintHop[];
/// }
/// ```
#[proc_macro_attribute]
pub fn extern_wasm_bindgen(args: TokenStream, input: TokenStream) -> TokenStream {
    wasm_bindgen::extern_wasm_bindgen(args, input)
}

#[proc_macro_attribute]
pub fn derive_from(attr: TokenStream, item: TokenStream) -> TokenStream {
    derive_from::derive_from(attr, item)
}

#[proc_macro_attribute]
pub fn derive_into(attr: TokenStream, item: TokenStream) -> TokenStream {
    derive_into::derive_into(attr, item)
}

/// Attribute macro to async test only non-WASM targets
#[proc_macro_attribute]
pub fn async_test_not_wasm(args: TokenStream, input: TokenStream) -> TokenStream {
    testing::async_test_not_wasm(args, input)
}

/// Attribute macro to async test only WASM targets
#[proc_macro_attribute]
pub fn async_test_wasm(args: TokenStream, input: TokenStream) -> TokenStream {
    testing::async_test_wasm(args, input)
}

/// Attribute macro to async test all targets
#[proc_macro_attribute]
pub fn async_test_all(args: TokenStream, input: TokenStream) -> TokenStream {
    testing::async_test_all(args, input)
}

/// Attribute macro to test only non-WASM targets
#[proc_macro_attribute]
pub fn test_not_wasm(args: TokenStream, input: TokenStream) -> TokenStream {
    testing::test_not_wasm(args, input)
}

/// Attribute macro to test only WASM targets
#[proc_macro_attribute]
pub fn test_wasm(args: TokenStream, input: TokenStream) -> TokenStream {
    testing::test_wasm(args, input)
}

/// Attribute macro to test all targets
#[proc_macro_attribute]
pub fn test_all(args: TokenStream, input: TokenStream) -> TokenStream {
    testing::test_all(args, input)
}
