//! The JSON Schema and TypeScript types generated from [`crate::View`] and
//! [`crate::Intent`]. `cargo run -p aethyr-ui -- --write` rewrites both
//! checked-in files; `tests::generated_files_are_current` fails on drift.

use crate::view::{ELEMENT_KINDS, Intent, View};
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value, json};
use std::fmt::Write as _;

/// The checked-in JSON Schema, relative to the repository root.
pub const SCHEMA_PATH: &str = "crates/aethyr-ui/schema/aethyr.ui.v1.schema.json";
/// The checked-in TypeScript types, relative to the repository root.
pub const TYPESCRIPT_PATH: &str = "apps/verse-ui/src/aethyr/protocol.ts";

/// The JSON Schema document: `View` at the root, every type under `$defs`.
pub fn schema() -> Value {
    let mut generator = SchemaSettings::draft2020_12().into_generator();
    generator.subschema_for::<View>();
    generator.subschema_for::<Intent>();
    let defs = generator.take_definitions(true);
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://openagents.com/schemas/aethyr.ui.v1.schema.json",
        "title": crate::SCHEMA,
        "description": "Aethyr UI protocol: a view tree published by a controller (View) and the intents a renderer sends back (Intent). Generated from crates/aethyr-ui; do not edit.",
        "$ref": "#/$defs/View",
        "$defs": defs,
    })
}

/// The schema as checked in: pretty JSON with a trailing newline.
pub fn schema_text() -> String {
    let mut text = serde_json::to_string_pretty(&schema()).expect("schema serializes");
    text.push('\n');
    text
}

/// SHA-256 of `schema_text()`, embedded in the TypeScript so the web package
/// can detect drift without Rust.
pub fn schema_sha256() -> String {
    sha256_hex(schema_text().as_bytes())
}

/// The TypeScript module the controller and renderer import.
pub fn typescript() -> String {
    let schema = schema();
    let defs = schema["$defs"].as_object().expect("$defs");
    let mut out = String::new();
    out.push_str(
        "// Generated from crates/aethyr-ui (the aethyr.ui.v1 schema); do not edit.\n\
         // Regenerate: cargo run -p aethyr-ui -- --write\n\n",
    );
    let _ = writeln!(out, "export const SCHEMA = {:?};", crate::SCHEMA);
    let _ = writeln!(out, "export const SCHEMA_SHA256 = {:?};", schema_sha256());
    let _ = writeln!(out, "export const MAX_NODES = {};", crate::MAX_NODES);
    let _ = writeln!(out, "export const MAX_DEPTH = {};", crate::MAX_DEPTH);
    let _ = writeln!(
        out,
        "export const MAX_VIEW_BYTES = {};",
        crate::MAX_VIEW_BYTES
    );
    let _ = writeln!(out, "export const MAX_ID_BYTES = {};", crate::MAX_ID_BYTES);
    let _ = writeln!(
        out,
        "export const MAX_TEXT_BYTES = {};",
        crate::MAX_TEXT_BYTES
    );
    let _ = writeln!(
        out,
        "export const MAX_FIELD_BYTES = {};",
        crate::MAX_FIELD_BYTES
    );
    let kinds: Vec<String> = ELEMENT_KINDS.iter().map(|k| format!("{k:?}")).collect();
    let _ = writeln!(
        out,
        "export const ELEMENT_KINDS = [{}] as const;",
        kinds.join(", ")
    );
    for (name, def) in defs {
        out.push('\n');
        doc(&mut out, def, "");
        match def.get("properties") {
            Some(_) => {
                let _ = writeln!(out, "export interface {name} {}", object(def, ""));
            }
            None => {
                let text = ty(def, "");
                let sep = if text.starts_with('\n') { "" } else { " " };
                let _ = writeln!(out, "export type {name} ={sep}{text};");
            }
        }
    }
    let _ = write!(
        out,
        "\n/** The JSON Schema these types come from; the web validator decodes with it. */\n\
         export const SCHEMA_DOCUMENT: unknown = {};\n",
        schema_text().trim_end()
    );
    out
}

fn doc(out: &mut String, schema: &Value, indent: &str) {
    if let Some(text) = schema.get("description").and_then(Value::as_str) {
        let _ = writeln!(out, "{indent}/**");
        for line in text.lines() {
            let line = line.replace("*/", "*\\/");
            if line.is_empty() {
                let _ = writeln!(out, "{indent} *");
            } else {
                let _ = writeln!(out, "{indent} * {line}");
            }
        }
        let _ = writeln!(out, "{indent} */");
    }
}

fn object(schema: &Value, indent: &str) -> String {
    let empty = Map::new();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let inner = format!("{indent}  ");
    let mut out = String::from("{\n");
    for (key, property) in properties {
        doc(&mut out, property, &inner);
        let optional = !required.contains(&key.as_str());
        // An optional property is omitted rather than sent as null.
        let text = ty_inner(property, &inner, optional);
        let mark = if optional { "?" } else { "" };
        let _ = writeln!(out, "{inner}readonly {key}{mark}: {text};");
    }
    out.push_str(indent);
    out.push('}');
    out
}

fn ty(schema: &Value, indent: &str) -> String {
    ty_inner(schema, indent, false)
}

fn ty_inner(schema: &Value, indent: &str, drop_null: bool) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return reference
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();
    }
    if let Some(value) = schema.get("const") {
        return value.to_string();
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let parts: Vec<String> = values
            .iter()
            .filter(|v| !(drop_null && v.is_null()))
            .map(Value::to_string)
            .collect();
        return parts.join(" | ");
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(key).and_then(Value::as_array) {
            let parts: Vec<String> = variants
                .iter()
                .filter(|v| !(drop_null && v.get("type") == Some(&json!("null"))))
                .map(|v| {
                    let mut text = String::new();
                    if v.get("properties").is_some() {
                        let mut comment = String::new();
                        doc(&mut comment, v, &format!("{indent}  "));
                        text.push('\n');
                        text.push_str(&comment);
                        let _ = write!(text, "{indent}  | {}", object(v, &format!("{indent}  ")));
                    } else {
                        text = ty(v, indent);
                    }
                    text
                })
                .collect();
            return if parts.iter().any(|p| p.starts_with('\n')) {
                parts.concat()
            } else {
                parts.join(" | ")
            };
        }
    }
    match schema.get("type") {
        Some(Value::Array(types)) => types
            .iter()
            .filter(|t| !(drop_null && t.as_str() == Some("null")))
            .map(|t| {
                let mut single = schema.clone();
                single["type"] = t.clone();
                ty(&single, indent)
            })
            .collect::<Vec<_>>()
            .join(" | "),
        Some(Value::String(name)) => match name.as_str() {
            "string" => "string".into(),
            "integer" | "number" => "number".into(),
            "boolean" => "boolean".into(),
            "null" => "null".into(),
            "array" => {
                let item = schema.get("items").map(|i| ty(i, indent));
                format!("ReadonlyArray<{}>", item.as_deref().unwrap_or("unknown"))
            }
            "object" if schema.get("properties").is_some() => object(schema, indent),
            _ => "unknown".into(),
        },
        _ => "unknown".into(),
    }
}

/// A small SHA-256 so the crate needs no hashing dependency.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for chunk in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(value);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
pub(crate) fn sha256_for_test(data: &[u8]) -> String {
    sha256_hex(data)
}
