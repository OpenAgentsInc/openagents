//! JSON serialization with Python `json.dumps` separators.

use std::io;

use serde::Serialize;
use serde_json::ser::{CompactFormatter, Formatter};

/// A formatter that writes `, ` between items and `: ` after keys, as Python
/// `json.dumps` does.
struct SpacedFormatter;

impl Formatter for SpacedFormatter {
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(if first { b"" } else { b", " })
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(if first { b"" } else { b", " })
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b": ")
    }
}

/// A formatter that writes Python `json.dumps` separators and float
/// exponents: a float whose magnitude is below `1e-4` uses exponent notation,
/// and a negative exponent has at least two digits, such as `1e-05`.
struct PythonStyleFormatter;

impl Formatter for PythonStyleFormatter {
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(if first { b"" } else { b", " })
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(if first { b"" } else { b", " })
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b": ")
    }

    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buf = Vec::with_capacity(32);
        CompactFormatter.write_f64(&mut buf, value)?;
        if !buf.contains(&b'e') {
            let (sign, rest): (&[u8], &[u8]) = match buf.strip_prefix(b"-") {
                Some(rest) => (b"-", rest),
                None => (b"", &buf),
            };
            // Python writes a float below `1e-4`, which has at least four zeros
            // after the decimal point, in exponent notation.
            if let Some(fraction) = rest.strip_prefix(b"0.") {
                let zeros = fraction.iter().take_while(|&&b| b == b'0').count();
                if let [first_digit, rest_digits @ ..] = &fraction[zeros..]
                    && zeros >= 4
                {
                    writer.write_all(sign)?;
                    writer.write_all(std::slice::from_ref(first_digit))?;
                    if !rest_digits.is_empty() {
                        writer.write_all(b".")?;
                        writer.write_all(rest_digits)?;
                    }
                    return write!(writer, "e-{:02}", zeros + 1);
                }
            }
        } else if let [mantissa @ .., b'e', b'-', digit] = buf.as_slice() {
            // Python pads a one-digit negative exponent to two digits.
            writer.write_all(mantissa)?;
            return write!(writer, "e-0{}", *digit as char);
        }
        writer.write_all(&buf)
    }
}

/// Serialize JSON with spaced separators and Python-style negative float
/// exponents. Non-ASCII text remains unescaped; other string and number
/// formatting follows `serde_json`.
///
/// The input is a JSON value, whose serialization to memory produces valid
/// JSON.
pub fn stringify_python_style(value: &serde_json::Value) -> String {
    let mut buf = Vec::with_capacity(128);
    let mut ser = serde_json::ser::Serializer::with_formatter(&mut buf, PythonStyleFormatter);
    value
        .serialize(&mut ser)
        .expect("JSON values serialize to memory");
    String::from_utf8(buf).expect("JSON serialization produces UTF-8")
}

/// Serialize JSON with spaced separators. Numbers, strings, and non-ASCII
/// text follow `serde_json`.
pub fn stringify_spaced(value: &serde_json::Value) -> String {
    let mut buf = Vec::with_capacity(128);
    let mut ser = serde_json::ser::Serializer::with_formatter(&mut buf, SpacedFormatter);
    value
        .serialize(&mut ser)
        .expect("JSON values serialize to memory");
    String::from_utf8(buf).expect("JSON serialization produces UTF-8")
}

#[cfg(test)]
mod tests {
    use super::{stringify_python_style, stringify_spaced};

    #[test]
    fn small_floats_differ_between_styles() {
        let value = serde_json::json!({"a": [0.00001, 1e-7]});
        assert_eq!(stringify_python_style(&value), r#"{"a": [1e-05, 1e-07]}"#);
        assert_eq!(stringify_spaced(&value), r#"{"a": [0.00001, 1e-7]}"#);
    }

    #[test]
    fn spaced_separators() {
        let value = serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The location user interested in"
                }
            },
            "required": ["location"],
            "additionalProperties": false
        });

        assert_eq!(
            stringify_python_style(&value),
            r#"{"type": "object", "properties": {"location": {"type": "string", "description": "The location user interested in"}}, "required": ["location"], "additionalProperties": false}"#
        );
    }
}
