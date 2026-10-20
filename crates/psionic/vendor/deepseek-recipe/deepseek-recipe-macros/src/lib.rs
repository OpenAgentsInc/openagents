//! Procedural macros for deepseek-recipe: [`prompt!`](macro@prompt), which
//! builds prompts for prompt rendering, and the
//! [`UntaggedDeserialize`](derive@UntaggedDeserialize) derive, which
//! deserializes untagged enums by inspecting the JSON value.

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod prompt;
mod untagged;

/// Build a `Prompt` from a format string. Template text and string arguments
/// are plain text; a `Special` argument inserts an explicit special token.
///
/// Format placeholders follow `format!`:
///
/// - `{}` takes the next positional argument.
/// - `{name}` takes the named argument `name = ...`, or captures the variable
///   `name` from the enclosing scope when no such argument exists.
/// - `{{` and `}}` write literal braces.
///
/// Format specifiers are not supported. Every argument must be used. The
/// expansion evaluates each argument exactly once, from left to right, before
/// it appends any text. Argument types dispatch through
/// `deepseek_recipe_core::prompt::PromptFragment`:
/// `str` and `String` are plain text, `Special<T>` is an explicit special
/// token, and `Prompt` splices in with its own explicit special tokens. A
/// special-token spelling in plain text is not an explicit special token.
///
/// Re-exported as `deepseek_recipe_core::prompt`.
#[proc_macro]
pub fn prompt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as prompt::PromptInput);
    match prompt::expand(input) {
        Ok(expanded) => TokenStream::from(expanded),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive `Deserialize` for an untagged enum by inspecting the JSON value.
///
/// The enum attribute `#[untagged_deserialize(error = "...")]` sets the error
/// for a value that matches no variant. Each variant selects values with
/// `#[untagged_deserialize(...)]`:
///
/// - `field = "name"`: the object has the field;
/// - `pattern = Variant(_)`: the value, or the selected field, matches a
///   `serde_json::Value` pattern;
/// - `value = "text"`: the value, or the selected field, equals a JSON value.
///
/// The generated `deserialize` checks the variant conditions in declaration
/// order and deserializes the first variant whose condition matches. It
/// returns that variant's result, including a deserialization error, without
/// trying later variants. A unit variant is constructed directly. A tuple
/// variant deserializes the whole value. A struct variant deserializes each
/// field from the field of the same name, reporting a failure as
/// ``field `name`: {error}``; a missing field deserializes from `null`.
///
/// The field name follows a field's `#[serde(rename = "...")]` and a variant's
/// `#[serde(rename_all = "...")]` with the `snake_case`, `lowercase`, or
/// `camelCase` rule; other `rename_all` rules are not supported. The derive
/// ignores other `serde` attributes. The expansion uses the `serde` and
/// `serde_json` crates.
#[proc_macro_derive(UntaggedDeserialize, attributes(untagged_deserialize, serde))]
pub fn untagged_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match untagged::expand(input) {
        Ok(expanded) => TokenStream::from(expanded),
        Err(err) => err.to_compile_error().into(),
    }
}
