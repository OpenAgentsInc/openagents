//! One renderer, shared by every path.
//!
//! A state, an instruction, and a criteria value each arrive as a string, an
//! object, or an array. They reach the model as labelled text. Training, if
//! it ever happens, and serving go through this function, so the model never
//! meets a shape at inference it did not meet before.

use serde_json::Value;

/// Renders one value as labelled text.
#[must_use]
pub fn render(value: &Value) -> String {
    let mut out = String::new();
    write(value, 0, &mut out);
    out.trim_end().to_string()
}

/// Renders a value that may be absent, returning an empty string for null.
#[must_use]
pub fn render_opt(value: Option<&Value>) -> String {
    value.map_or_else(String::new, render)
}

fn write(value: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Null => {}
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::String(text) => out.push_str(text),
        Value::Array(items) => {
            for item in items {
                if item.is_null() {
                    continue;
                }
                out.push_str(&pad);
                out.push_str("- ");
                if item.is_object() || item.is_array() {
                    out.push('\n');
                    write(item, depth + 1, out);
                } else {
                    write(item, depth, out);
                }
                out.push('\n');
            }
        }
        Value::Object(fields) => {
            for (key, field) in fields {
                if field.is_null() {
                    continue;
                }
                out.push_str(&pad);
                out.push_str(key);
                out.push(':');
                if field.is_object() || field.is_array() {
                    out.push('\n');
                    write(field, depth + 1, out);
                } else {
                    out.push(' ');
                    write(field, depth, out);
                    out.push('\n');
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_string_renders_as_itself() {
        assert_eq!(render(&json!("the order was charged twice")), "the order was charged twice");
    }

    #[test]
    fn an_object_renders_as_labelled_lines() {
        let value = json!({"subject": "double charge", "plan": "pro"});
        assert_eq!(render(&value), "subject: double charge\nplan: pro");
    }

    #[test]
    fn nesting_indents_and_nulls_drop_out() {
        let value = json!({"ticket": {"id": 7, "note": null}, "tags": ["billing", "urgent"]});
        assert_eq!(render(&value), "ticket:\n  id: 7\ntags:\n  - billing\n  - urgent");
    }

    #[test]
    fn field_order_survives() {
        let value = json!({"b": 1, "a": 2});
        assert_eq!(render(&value), "b: 1\na: 2");
    }
}
