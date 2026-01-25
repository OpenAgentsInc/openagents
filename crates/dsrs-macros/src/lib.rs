extern crate self as dsrs_macros;

use proc_macro::TokenStream;
use quote::quote;
use serde_json::{Value, json};
use syn::{Attribute, DeriveInput, Lit, MetaNameValue, parse_macro_input};

mod optim;

#[proc_macro_derive(Optimizable, attributes(parameter))]
pub fn derive_optimizable(input: TokenStream) -> TokenStream {
    optim::optimizable_impl(input)
}

#[allow(unused_assignments, non_snake_case)]
#[proc_macro_attribute]
pub fn Signature(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse the attributes (cot, hint, etc.)
    let attr_str = attr.to_string();
    let has_cot = attr_str.contains("cot");
    let has_hint = attr_str.contains("hint");

    let struct_name = &input.ident;
    let visibility = &input.vis;

    let mut signature_instruction = String::new();
    // Store everything as serde Values
    let mut input_schema: Value = json!({});
    let mut output_schema: Value = json!({});

    // Store schema update operations to be performed at runtime
    let mut schema_updates = Vec::new();

    if has_cot {
        output_schema["reasoning"] = json!({
            "type": "String",
            "desc": "Think step by step",
            "schema": "",
            "__dsrs_field_type": "output"
        });
    }
    // Generate schema for the field

    match &input.data {
        syn::Data::Struct(s) => {
            if let syn::Fields::Named(named) = &s.fields {
                let mut found_first_input = false;

                for field in &named.named {
                    let field_name = field.ident.as_ref().unwrap().clone();
                    let field_type = field.ty.clone();

                    // Check for #[input] or #[output] attributes
                    let (is_input, desc) = has_in_attribute(&field.attrs);
                    let (is_output, desc2) = has_out_attribute(&field.attrs);

                    if is_input && is_output {
                        panic!("Field {field_name} cannot be both input and output");
                    }

                    if !is_input && !is_output {
                        panic!(
                            "Field {field_name} must have either #[input] or #[output] attribute"
                        );
                    }

                    let field_desc = if is_input { desc } else { desc2 };

                    // Collect doc comments from first input field as instruction
                    if is_input && !found_first_input {
                        signature_instruction = field
                            .attrs
                            .iter()
                            .filter(|a| a.path().is_ident("doc"))
                            .filter_map(|a| match &a.meta {
                                syn::Meta::NameValue(nv) => match &nv.value {
                                    syn::Expr::Lit(syn::ExprLit {
                                        lit: syn::Lit::Str(s),
                                        ..
                                    }) => Some(s.value()),
                                    _ => None,
                                },
                                _ => None,
                            })
                            .map(|s| s.trim().to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        found_first_input = true;
                    }

                    // Create the field metadata as a serde Value
                    let type_str = quote!(#field_type).to_string();

                    let field_metadata = json!({
                        "type": type_str,
                        "desc": field_desc,
                        "schema": "",
                        "__dsrs_field_type": if is_input { "input" } else { "output" }
                    });

                    if is_input {
                        input_schema[field_name.to_string()] = field_metadata;
                        // Check if type needs schema generation (not primitive types)
                        if !is_primitive_type(&type_str) {
                            let field_name_str = field_name.to_string();
                            schema_updates.push(quote! {
                                {
                                    let schema = schemars::schema_for!(#field_type);
                                    let schema_json = serde_json::to_value(schema).unwrap();
                                    // Extract just the properties if it's an object schema
                                    if let Some(obj) = schema_json.as_object() {
                                        if obj.contains_key("properties") {
                                            input_fields[#field_name_str]["schema"] = schema_json["properties"].clone();
                                        } else {
                                            input_fields[#field_name_str]["schema"] = schema_json;
                                        }
                                    } else {
                                        input_fields[#field_name_str]["schema"] = schema_json;
                                    }
                                }
                            });
                        }
                    } else if is_output {
                        output_schema[field_name.to_string()] = field_metadata;
                        // Check if type needs schema generation (not primitive types)
                        if !is_primitive_type(&type_str) {
                            let field_name_str = field_name.to_string();
                            schema_updates.push(quote! {
                                {
                                    let schema = schemars::schema_for!(#field_type);
                                    let schema_json = serde_json::to_value(schema).unwrap();
                                    // Extract just the properties if it's an object schema
                                    if let Some(obj) = schema_json.as_object() {
                                        if obj.contains_key("properties") {
                                            output_fields[#field_name_str]["schema"] = schema_json["properties"].clone();
                                        } else {
                                            output_fields[#field_name_str]["schema"] = schema_json;
                                        }
                                    } else {
                                        output_fields[#field_name_str]["schema"] = schema_json;
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
        _ => panic!("Signature can only be applied to structs"),
    }

    if has_hint {
        input_schema["hint"] = json!({
            "type": "String",
            "desc": "Hint for the query",
            "schema": "",
            "__dsrs_field_type": "input"
        });
    }

    // Serialize the schemas to strings so we can embed them in the generated code
    let input_schema_str = serde_json::to_string(&input_schema).unwrap();
    let output_schema_str = serde_json::to_string(&output_schema).unwrap();

    let generated = quote! {
        #[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
        #visibility struct #struct_name {
            instruction: String,
            input_fields: serde_json::Value,
            output_fields: serde_json::Value,
            demos: Vec<dsrs::Example>,
        }

        impl #struct_name {
            pub fn new() -> Self {
                let mut input_fields: serde_json::Value = serde_json::from_str(#input_schema_str).unwrap();
                let mut output_fields: serde_json::Value = serde_json::from_str(#output_schema_str).unwrap();

                // Update schemas for complex types
                #(#schema_updates)*

                Self {
                    instruction: #signature_instruction.to_string(),
                    input_fields: input_fields,
                    output_fields: output_fields,
                    demos: vec![],
                }
            }

            pub fn input_fields_len(&self) -> usize {
                self.input_fields.as_object().map_or(0, |obj| obj.len())
            }

            pub fn output_fields_len(&self) -> usize {
                self.output_fields.as_object().map_or(0, |obj| obj.len())
            }
        }

        impl dsrs::core::MetaSignature for #struct_name {
            fn demos(&self) -> Vec<dsrs::Example> {
                self.demos.clone()
            }

            fn set_demos(&mut self, demos: Vec<dsrs::Example>) -> anyhow::Result<()> {
                self.demos = demos;
                Ok(())
            }

            fn instruction(&self) -> String {
                self.instruction.clone()
            }

            fn input_fields(&self) -> serde_json::Value {
                self.input_fields.clone()
            }

            fn output_fields(&self) -> serde_json::Value {
                self.output_fields.clone()
            }

            fn update_instruction(&mut self, instruction: String) -> anyhow::Result<()> {
                self.instruction = instruction;
                Ok(())
            }

            fn append(&mut self, name: &str, field_value: serde_json::Value) -> anyhow::Result<()> {
                match field_value["__dsrs_field_type"].as_str() {
                    Some("input") => {
                        self.input_fields[name] = field_value;
                    }
                    Some("output") => {
                        self.output_fields[name] = field_value;
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Invalid field type: {:?}", field_value["__dsrs_field_type"].as_str()));
                    }
                }
                Ok(())
            }
        }
    };

    generated.into()
}

fn has_in_attribute(attrs: &[Attribute]) -> (bool, String) {
    for attr in attrs {
        if attr.path().is_ident("input") {
            // Try to parse desc parameter
            if let Ok(list) = attr.meta.require_list() {
                let desc = parse_desc_from_tokens(list.tokens.clone());
                return (true, desc);
            } else {
                // Just #[input] without parameters
                return (true, String::new());
            }
        }
    }
    (false, String::new())
}

fn has_out_attribute(attrs: &[Attribute]) -> (bool, String) {
    for attr in attrs {
        if attr.path().is_ident("output") {
            // Try to parse desc parameter
            if let Ok(list) = attr.meta.require_list() {
                let desc = parse_desc_from_tokens(list.tokens.clone());
                return (true, desc);
            } else {
                // Just #[output] without parameters
                return (true, String::new());
            }
        }
    }
    (false, String::new())
}

fn parse_desc_from_tokens(tokens: proc_macro2::TokenStream) -> String {
    if let Ok(nv) = syn::parse2::<MetaNameValue>(tokens)
        && nv.path.is_ident("desc")
        && let syn::Expr::Lit(syn::ExprLit {
            lit: Lit::Str(s), ..
        }) = nv.value
    {
        return s.value();
    }
    String::new()
}

fn is_primitive_type(type_str: &str) -> bool {
    matches!(
        type_str,
        "String"
            | "str"
            | "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "char"
    )
}
