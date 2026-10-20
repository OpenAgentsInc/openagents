use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataEnum, DeriveInput, Fields, LitStr, Token};

/// The variant condition of `#[untagged_deserialize(...)]`.
struct VariantCondition {
    field: Option<LitStr>,
    pattern: Option<TokenStream>,
    value: Option<TokenStream>,
}

/// Convert a snake_case field name with a serde `rename_all` rule. Only the
/// rules the protocol types use are supported.
///
/// # Errors
///
/// Returns an error spanned at `rule` for an unsupported rule.
fn rename_field(field: &str, rule: Option<&LitStr>) -> syn::Result<String> {
    let Some(rule) = rule else {
        return Ok(field.to_owned());
    };
    match rule.value().as_str() {
        "snake_case" | "lowercase" => Ok(field.to_owned()),
        "camelCase" => {
            let mut renamed = String::new();
            let mut capitalize = false;
            for ch in field.chars() {
                if ch == '_' {
                    capitalize = true;
                } else if capitalize {
                    renamed.push(ch.to_ascii_uppercase());
                    capitalize = false;
                } else {
                    renamed.push(ch);
                }
            }
            Ok(renamed)
        }
        unsupported => Err(syn::Error::new_spanned(
            rule,
            format!("unsupported rename_all rule `{unsupported}`"),
        )),
    }
}

pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let enum_name = &input.ident;
    let Data::Enum(DataEnum { variants, .. }) = input.data else {
        return Err(syn::Error::new_spanned(
            enum_name,
            "UntaggedDeserialize can only be derived for enums",
        ));
    };

    let mut error_message = None::<LitStr>;
    for attr in &input.attrs {
        if attr.path().is_ident("untagged_deserialize") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("error") {
                    meta.input.parse::<Token![=]>()?;
                    error_message = Some(meta.input.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("expected `error`"))
                }
            })?;
        }
    }
    let error_message = error_message.ok_or_else(|| {
        syn::Error::new_spanned(
            enum_name,
            "`#[untagged_deserialize(error = ...)]` is required",
        )
    })?;

    let mut arms = Vec::new();
    for variant in variants {
        let variant_ident = variant.ident;
        let mut condition = VariantCondition {
            field: None,
            pattern: None,
            value: None,
        };
        let mut rename_rule = None::<LitStr>;
        for attr in &variant.attrs {
            if attr.path().is_ident("untagged_deserialize") {
                attr.parse_nested_meta(|meta| {
                    meta.input.parse::<Token![=]>()?;
                    if meta.path.is_ident("field") {
                        condition.field = Some(meta.input.parse()?);
                    } else if meta.path.is_ident("pattern") {
                        let pattern = meta.input.call(syn::Pat::parse_single)?;
                        condition.pattern = Some(quote!(#pattern));
                    } else if meta.path.is_ident("value") {
                        let value = meta.input.parse::<syn::Expr>()?;
                        condition.value = Some(quote!(#value));
                    } else {
                        return Err(meta.error("expected `field`, `pattern`, or `value`"));
                    }
                    Ok(())
                })?;
            } else if attr.path().is_ident("serde") {
                // This derive reads only `rename_all` here and `rename` on
                // fields; it ignores other serde attributes.
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename_all") {
                        meta.input.parse::<Token![=]>()?;
                        rename_rule = Some(meta.input.parse()?);
                    } else if meta.input.peek(Token![=]) {
                        meta.input.parse::<Token![=]>()?;
                        meta.input.parse::<syn::Expr>()?;
                    }
                    Ok(())
                });
            }
        }
        let check = match (condition.field, condition.pattern, condition.value) {
            (None, Some(pattern), None) => quote!(matches!(value, serde_json::Value::#pattern)),
            (None, None, Some(expected)) => quote!(value == serde_json::json!(#expected)),
            (Some(field), None, None) => quote!(value.get(#field).is_some()),
            (Some(field), Some(pattern), None) => {
                quote!(matches!(value.get(#field), Some(serde_json::Value::#pattern)))
            }
            (Some(field), None, Some(expected)) => {
                quote!(value.get(#field) == Some(&serde_json::json!(#expected)))
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    &variant_ident,
                    "`#[untagged_deserialize(...)]` needs `field`, `pattern`, or `value`, \
                     and not both `pattern` and `value`",
                ));
            }
        };
        let construct = match variant.fields {
            Fields::Named(fields) => {
                let mut assignments = Vec::new();
                for field in fields.named {
                    let field_ident = field.ident.expect("named fields have identifiers");
                    let mut rename = None::<LitStr>;
                    for attr in &field.attrs {
                        if attr.path().is_ident("serde") {
                            let _ = attr.parse_nested_meta(|meta| {
                                if meta.path.is_ident("rename") {
                                    meta.input.parse::<Token![=]>()?;
                                    rename = Some(meta.input.parse()?);
                                } else if meta.input.peek(Token![=]) {
                                    meta.input.parse::<Token![=]>()?;
                                    meta.input.parse::<syn::Expr>()?;
                                }
                                Ok(())
                            });
                        }
                    }
                    let field_name = match rename {
                        Some(rename) => rename.value(),
                        None => {
                            let name = field_ident.to_string();
                            let name = name.strip_prefix("r#").unwrap_or(&name);
                            rename_field(name, rename_rule.as_ref())?
                        }
                    };
                    assignments.push(quote! {
                        #field_ident: serde_json::from_value(
                            value.get(#field_name).cloned().unwrap_or(serde_json::Value::Null)
                        ).map_err(|err| serde::de::Error::custom(
                            format!("field `{}`: {}", #field_name, err)
                        ))?,
                    });
                }
                quote!(return Ok(#enum_name::#variant_ident { #(#assignments)* });)
            }
            Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        &variant_ident,
                        "a tuple variant must have exactly one field",
                    ));
                }
                quote! {
                    return serde_json::from_value(value)
                        .map(#enum_name::#variant_ident)
                        .map_err(serde::de::Error::custom);
                }
            }
            Fields::Unit => quote!(return Ok(#enum_name::#variant_ident);),
        };
        arms.push(quote! {
            if #check {
                #construct
            }
        });
    }

    Ok(quote! {
        impl<'de> serde::Deserialize<'de> for #enum_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
                #(#arms)*
                Err(serde::de::Error::custom(#error_message))
            }
        }
    })
}
