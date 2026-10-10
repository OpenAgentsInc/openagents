//! JSON Canonicalization (RFC 8785) for the values vault formats use:
//! objects, arrays, strings, booleans, null, and integers. Keys are sorted
//! by their UTF-16 code units, as RFC 8785 requires, whatever map type
//! `serde_json` was built with. Floats are refused: no vault format has one.

use serde::Serialize;
use serde_json::Value;

use crate::{Error, Result};

/// The canonical JSON text of `value`.
pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    let value =
        serde_json::to_value(value).map_err(|_| Error::Format("A value can't be written."))?;
    let mut out = String::new();
    write(&value, &mut out)?;
    Ok(out)
}

/// The canonical JSON bytes of `value`.
pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    to_string(value).map(String::into_bytes)
}

fn write(value: &Value, out: &mut String) -> Result<()> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => {
            if let Some(n) = number.as_u64() {
                out.push_str(&n.to_string());
            } else if let Some(n) = number.as_i64() {
                out.push_str(&n.to_string());
            } else {
                return Err(Error::Format("Vault formats carry integers only."));
            }
        }
        Value::String(text) => string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                string(key, out);
                out.push(':');
                write(&map[key.as_str()], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// A string as ECMAScript `JSON.stringify` writes it.
fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
