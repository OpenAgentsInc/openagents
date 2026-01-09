pub mod adapter;
pub mod callbacks;
pub mod core;
pub mod data;
pub mod evaluate;
pub mod manifest;
pub mod optimizer;
pub mod predictors;
pub mod trace;
pub mod utils;

pub use adapter::chat::*;
pub use callbacks::*;
pub use core::*;
pub use data::*;
pub use evaluate::*;
pub use manifest::*;
pub use optimizer::*;
pub use predictors::*;
pub use utils::*;

pub use dsrs_macros::*;

#[macro_export]
macro_rules! example {
    // Pattern: { "key": <__dsrs_field_type>: "value", ... }
    { $($key:literal : $field_type:literal => $value:expr),* $(,)? } => {{
        use std::collections::HashMap;
        use dsrs::data::example::Example;
        use dsrs::trace::{NodeType, record_node};

        let mut input_keys = vec![];
        let mut output_keys = vec![];
        let mut fields = HashMap::new();
        let mut mappings = vec![];
        let mut parent_ids = vec![];

        $(
            if $field_type == "input" {
                input_keys.push($key.to_string());
            } else {
                output_keys.push($key.to_string());
            }

            let tracked = {
                use dsrs::trace::IntoTracked;
                $value.into_tracked()
            };

            fields.insert($key.to_string(), tracked.value);

            if let Some((node_id, source_key)) = tracked.source {
                mappings.push(($key.to_string(), (node_id, source_key)));
                if !parent_ids.contains(&node_id) {
                    parent_ids.push(node_id);
                }
            }
        )*

        let mut example = Example::new(
            fields,
            input_keys,
            output_keys,
        );

        // If we found mappings and we are tracing, record a Map node
        if !mappings.is_empty() {
             if let Some(map_node_id) = record_node(
                NodeType::Map { mapping: mappings },
                parent_ids,
                None
             ) {
                example.node_id = Some(map_node_id);
             }
        }

        example
    }};

    // Pattern without field type (defaulting to input usually? or implicit?)
    // The previous macro definition had a second pattern which was slightly different.
    // Wait, the original macro only had the first pattern for `example!`.
    // The `prediction!` macro was separate.

    // Original pattern from lib.rs:22
    // { $($key:literal : $field_type:literal => $value:expr),* $(,)? }

    // Wait, I should also support the simpler syntax if user uses it, but looking at lib.rs, `example!` only has one pattern.
}

#[macro_export]
macro_rules! prediction {
    { $($key:literal => $value:expr),* $(,)? } => {{
        use std::collections::HashMap;
        use dsrs::{Prediction, LmUsage};

        let mut fields = HashMap::new();
        $(
            fields.insert($key.to_string(), serde_json::to_value($value).unwrap());
        )*

        Prediction::new(fields, LmUsage::default())
    }};
}

#[macro_export]
macro_rules! field {
    // Example Usage: field! {
    //   input["Description"] => question: String
    // }
    //
    // Example Output:
    //
    // {
    //   "question": {
    //     "type": "String",
    //     "desc": "Description",
    //     "schema": ""
    //   },
    //   ...
    // }

    // Pattern for field definitions with descriptions
    { $($field_type:ident[$desc:literal] => $field_name:ident : $field_ty:ty),* $(,)? } => {{
        use serde_json::json;

        let mut result = serde_json::Map::new();

        $(
            let type_str = stringify!($field_ty);
            let schema = {
                let schema = schemars::schema_for!($field_ty);
                let schema_json = serde_json::to_value(schema).unwrap();
                // Extract just the properties if it's an object schema
                if let Some(obj) = schema_json.as_object() {
                    if obj.contains_key("properties") {
                        schema_json["properties"].clone()
                    } else {
                        "".to_string().into()
                    }
                } else {
                    "".to_string().into()
                }
            };
            result.insert(
                stringify!($field_name).to_string(),
                json!({
                    "type": type_str,
                    "desc": $desc,
                    "schema": schema,
                    "__dsrs_field_type": stringify!($field_type)
                })
            );
        )*

        serde_json::Value::Object(result)
    }};

    // Pattern for field definitions without descriptions
    { $($field_type:ident => $field_name:ident : $field_ty:ty),* $(,)? } => {{
        use serde_json::json;

        let mut result = serde_json::Map::new();

        $(
            let type_str = stringify!($field_ty);
            let schema = {
                let schema = schemars::schema_for!($field_ty);
                let schema_json = serde_json::to_value(schema).unwrap();
                // Extract just the properties if it's an object schema
                if let Some(obj) = schema_json.as_object() {
                    if obj.contains_key("properties") {
                        schema_json["properties"].clone()
                    } else {
                        "".to_string().into()
                    }
                } else {
                    "".to_string().into()
                }
            };
            result.insert(
                stringify!($field_name).to_string(),
                json!({
                    "type": type_str,
                    "desc": "",
                    "schema": schema,
                    "__dsrs_field_type": stringify!($field_type)
                })
            );
        )*

        serde_json::Value::Object(result)
    }};
}

#[macro_export]
macro_rules! sign {
    // Example Usage: signature! {
    //     question: String, random: bool -> answer: String
    // }
    //
    // Example Output:
    //
    // #[derive(Signature)]
    // struct InlineSignature {
    //     question: In<String>,
    //     random: In<bool>,
    //     answer: Out<String>,
    // }
    //
    // InlineSignature::new()

    // Pattern: input fields -> output fields
    { ($($input_name:ident : $input_type:ty),* $(,)?) -> $($output_name:ident : $output_type:ty),* $(,)? } => {{
        use dsrs::Signature;
        let mut input_fields = serde_json::Map::new();
        let mut output_fields = serde_json::Map::new();

        #[Signature]
        struct InlineSignature {
            $(
                #[input]
                $input_name: $input_type,
            )*
            $(
                #[output]
                $output_name: $output_type,
            )*
        }

        InlineSignature::new()
    }};
}

/// Source: https://github.com/wholesome-ghoul/hashmap_macro/blob/master/src/lib.rs
/// Author: https://github.com/wholesome-ghoul
/// License: MIT
/// Description: This macro creates a HashMap from a list of key-value pairs.
/// Reason for Reuse: Want to avoid adding a dependency for a simple macro.
#[macro_export]
macro_rules! hashmap {
    () => {
        ::std::collections::HashMap::new()
    };

    ($($key:expr => $value:expr),+ $(,)?) => {
        ::std::collections::HashMap::from([ $(($key, $value)),* ])
    };
}
