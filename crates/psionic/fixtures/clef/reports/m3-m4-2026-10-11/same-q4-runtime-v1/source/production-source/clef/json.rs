//! JSON for the Clef prompt: parsed the way Python's `json.loads` reads a
//! request body, rendered the way Cloudflare's reference `render` writes it
//! (`json.dumps(value, ensure_ascii=False, separators=(",", ":"),
//! sort_keys=True)`).
//!
//! The prompt is built from these bytes, so they must match Python's exactly:
//!
//! - object keys keep request order (questions and choice criteria are read
//!   in request order) and a repeated key keeps its first position with the
//!   last value, as a Python `dict` does;
//! - a number keeps its source spelling until it is rendered: an integer
//!   (no `.`, `e`, `E`) is exact at any size, and anything else is an IEEE
//!   double printed with Python's `repr` rules (`1.0`, `1e-05`, `1e+16`,
//!   `Infinity` for an overflow);
//! - strings are written with Python's escapes and no ASCII escaping.

use std::fmt::Write as _;

/// A JSON value with object order and number spelling kept.
#[derive(Clone, Debug, PartialEq)]
pub enum ClefJson {
    Null,
    Bool(bool),
    /// The number as it was written in the request.
    Number(String),
    String(String),
    Array(Vec<ClefJson>),
    /// Members in first-seen key order.
    Object(Vec<(String, ClefJson)>),
}

impl ClefJson {
    /// The member `key` of an object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ClefJson> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Python truthiness: `None`, `False`, `0`, `0.0`, `""`, `[]` and `{}`
    /// are false.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(value) => *value,
            Self::Number(text) => match classify_number(text) {
                NumberKind::Integer => text.trim_start_matches('-').bytes().any(|b| b != b'0'),
                NumberKind::Float => text.parse::<f64>().is_ok_and(|value| value != 0.0),
            },
            Self::String(value) => !value.is_empty(),
            Self::Array(values) => !values.is_empty(),
            Self::Object(members) => !members.is_empty(),
        }
    }

    /// Python `str(value)` for the values a question id or option key can
    /// be (strings and integers); other values render as JSON.
    #[must_use]
    pub fn to_python_str(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            other => render(other),
        }
    }

    /// The value as a `serde_json::Value`, for echoing it in an answer
    /// (`legend`). Integers past 64 bits and non-finite floats become
    /// their rendered text.
    #[must_use]
    pub fn to_serde(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(*value),
            Self::Number(text) => {
                let rendered = render_number(text);
                if let Ok(value) = rendered.parse::<i64>() {
                    serde_json::Value::from(value)
                } else if let Ok(value) = rendered.parse::<u64>() {
                    serde_json::Value::from(value)
                } else if let Some(number) = rendered
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                {
                    serde_json::Value::Number(number)
                } else {
                    serde_json::Value::String(rendered)
                }
            }
            Self::String(value) => serde_json::Value::String(value.clone()),
            Self::Array(values) => {
                serde_json::Value::Array(values.iter().map(Self::to_serde).collect())
            }
            Self::Object(members) => serde_json::Value::Object(
                members
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_serde()))
                    .collect(),
            ),
        }
    }
}

/// Why a body is not JSON this server reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClefJsonError {
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for ClefJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid JSON at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ClefJsonError {}

const MAX_DEPTH: usize = 512;

/// Parses one JSON document (RFC 8259). `NaN`/`Infinity` literals and lone
/// UTF-16 surrogates are refused: the first is not JSON, and the second
/// cannot be tokenized.
pub fn parse(text: &str) -> Result<ClefJson, ClefJsonError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        text,
        position: 0,
    };
    parser.skip_whitespace();
    let value = parser.value(0)?;
    parser.skip_whitespace();
    if parser.position != parser.bytes.len() {
        return Err(parser.error("trailing characters after the JSON value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    position: usize,
}

impl Parser<'_> {
    fn error(&self, message: impl Into<String>) -> ClefJsonError {
        ClefJsonError {
            offset: self.position,
            message: message.into(),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.bytes.get(self.position) {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn expect_literal(
        &mut self,
        literal: &str,
        value: ClefJson,
    ) -> Result<ClefJson, ClefJsonError> {
        if self.bytes[self.position..].starts_with(literal.as_bytes()) {
            self.position += literal.len();
            Ok(value)
        } else {
            Err(self.error("unexpected character"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<ClefJson, ClefJsonError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting is too deep"));
        }
        match self.bytes.get(self.position) {
            None => Err(self.error("unexpected end of input")),
            Some(b'n') => self.expect_literal("null", ClefJson::Null),
            Some(b't') => self.expect_literal("true", ClefJson::Bool(true)),
            Some(b'f') => self.expect_literal("false", ClefJson::Bool(false)),
            Some(b'"') => self.string().map(ClefJson::String),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<ClefJson, ClefJsonError> {
        let start = self.position;
        if self.bytes.get(self.position) == Some(&b'-') {
            self.position += 1;
        }
        match self.bytes.get(self.position) {
            Some(b'0') => self.position += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.bytes.get(self.position), Some(b'0'..=b'9')) {
                    self.position += 1;
                }
            }
            _ => return Err(self.error("invalid number")),
        }
        if self.bytes.get(self.position) == Some(&b'.') {
            self.position += 1;
            if !matches!(self.bytes.get(self.position), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number fraction"));
            }
            while matches!(self.bytes.get(self.position), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        if matches!(self.bytes.get(self.position), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.bytes.get(self.position), Some(b'+' | b'-')) {
                self.position += 1;
            }
            if !matches!(self.bytes.get(self.position), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number exponent"));
            }
            while matches!(self.bytes.get(self.position), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        Ok(ClefJson::Number(
            self.text[start..self.position].to_string(),
        ))
    }

    fn hex4(&mut self) -> Result<u32, ClefJsonError> {
        let digits = self
            .bytes
            .get(self.position..self.position + 4)
            .ok_or_else(|| self.error("truncated \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.error("invalid \\u escape"))?;
        let value = u32::from_str_radix(text, 16).map_err(|_| self.error("invalid \\u escape"))?;
        self.position += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, ClefJsonError> {
        self.position += 1;
        let mut out = String::new();
        loop {
            let run_start = self.position;
            while let Some(byte) = self.bytes.get(self.position) {
                if *byte == b'"' || *byte == b'\\' || *byte < 0x20 {
                    break;
                }
                self.position += 1;
            }
            out.push_str(&self.text[run_start..self.position]);
            match self.bytes.get(self.position) {
                None => return Err(self.error("unterminated string")),
                Some(b'"') => {
                    self.position += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.position += 1;
                    let escape = *self
                        .bytes
                        .get(self.position)
                        .ok_or_else(|| self.error("truncated escape"))?;
                    self.position += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                if self.bytes.get(self.position..self.position + 2)
                                    != Some(b"\\u".as_slice())
                                {
                                    return Err(self.error("lone UTF-16 surrogate in string"));
                                }
                                self.position += 2;
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(self.error("lone UTF-16 surrogate in string"));
                                }
                                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&first) {
                                return Err(self.error("lone UTF-16 surrogate in string"));
                            } else {
                                first
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| self.error("invalid \\u escape"))?,
                            );
                        }
                        _ => return Err(self.error("invalid escape")),
                    }
                }
                Some(_) => return Err(self.error("control character in string")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<ClefJson, ClefJsonError> {
        self.position += 1;
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.bytes.get(self.position) == Some(&b']') {
            self.position += 1;
            return Ok(ClefJson::Array(values));
        }
        loop {
            self.skip_whitespace();
            values.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.bytes.get(self.position) {
                Some(b',') => self.position += 1,
                Some(b']') => {
                    self.position += 1;
                    return Ok(ClefJson::Array(values));
                }
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<ClefJson, ClefJsonError> {
        self.position += 1;
        let mut members: Vec<(String, ClefJson)> = Vec::new();
        self.skip_whitespace();
        if self.bytes.get(self.position) == Some(&b'}') {
            self.position += 1;
            return Ok(ClefJson::Object(members));
        }
        loop {
            self.skip_whitespace();
            if self.bytes.get(self.position) != Some(&b'"') {
                return Err(self.error("expected a string key"));
            }
            let key = self.string()?;
            self.skip_whitespace();
            if self.bytes.get(self.position) != Some(&b':') {
                return Err(self.error("expected `:`"));
            }
            self.position += 1;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            // A Python dict keeps a repeated key where it first appeared,
            // with the last value.
            if let Some(slot) = members.iter_mut().find(|(name, _)| *name == key) {
                slot.1 = value;
            } else {
                members.push((key, value));
            }
            self.skip_whitespace();
            match self.bytes.get(self.position) {
                Some(b',') => self.position += 1,
                Some(b'}') => {
                    self.position += 1;
                    return Ok(ClefJson::Object(members));
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }
}

enum NumberKind {
    Integer,
    Float,
}

fn classify_number(text: &str) -> NumberKind {
    if text.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
        NumberKind::Float
    } else {
        NumberKind::Integer
    }
}

/// Python's `json.dumps` spelling of a JSON number lexeme after
/// `json.loads`: an integer exactly (`-0` is `0`), anything else as
/// `repr(float(text))`.
#[must_use]
pub fn render_number(text: &str) -> String {
    match classify_number(text) {
        NumberKind::Integer => {
            let (negative, digits) = match text.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, text),
            };
            if digits.bytes().all(|byte| byte == b'0') {
                String::from("0")
            } else if negative {
                format!("-{digits}")
            } else {
                digits.to_string()
            }
        }
        NumberKind::Float => python_float_repr(text.parse::<f64>().unwrap_or(f64::NAN)),
    }
}

/// `repr(float)` as `json.dumps` writes it: the shortest round-trip digits,
/// positional notation for decimal exponents in `-4 < decpt <= 16`,
/// otherwise `d.ddde±XX`; `NaN`, `Infinity` and `-Infinity` as Python's
/// encoder spells them.
#[must_use]
pub fn python_float_repr(value: f64) -> String {
    if value.is_nan() {
        return String::from("NaN");
    }
    if value.is_infinite() {
        return String::from(if value > 0.0 { "Infinity" } else { "-Infinity" });
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value == 0.0 {
        return format!("{sign}0.0");
    }
    // Rust's `{:e}` is the shortest round-trip digit string, as Python's
    // repr (David Gay's dtoa mode 0) is.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let decpt = exponent + 1;
    let mut out = String::from(sign);
    if decpt <= -4 || decpt > 16 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let _ = write!(
            out,
            "e{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.unsigned_abs()
        );
    } else if decpt <= 0 {
        out.push_str("0.");
        for _ in 0..(-decpt) {
            out.push('0');
        }
        out.push_str(digits);
    } else {
        let decpt = decpt as usize;
        if digits.len() <= decpt {
            out.push_str(digits);
            for _ in digits.len()..decpt {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..decpt]);
            out.push('.');
            out.push_str(&digits[decpt..]);
        }
    }
    out
}

/// Python `json.dumps(value, ensure_ascii=False)` string spelling.
pub fn write_python_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            control if (control as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

fn write_dumps(out: &mut String, value: &ClefJson) {
    match value {
        ClefJson::Null => out.push_str("null"),
        ClefJson::Bool(true) => out.push_str("true"),
        ClefJson::Bool(false) => out.push_str("false"),
        ClefJson::Number(text) => out.push_str(&render_number(text)),
        ClefJson::String(text) => write_python_string(out, text),
        ClefJson::Array(values) => {
            out.push('[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_dumps(out, item);
            }
            out.push(']');
        }
        ClefJson::Object(members) => {
            // `sort_keys=True`: Python orders str keys by code point, which
            // is UTF-8 byte order.
            let mut sorted: Vec<&(String, ClefJson)> = members.iter().collect();
            sorted.sort_by(|left, right| left.0.cmp(&right.0));
            out.push('{');
            for (index, (key, item)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_python_string(out, key);
                out.push(':');
                write_dumps(out, item);
            }
            out.push('}');
        }
    }
}

/// `json.dumps(value, ensure_ascii=False, separators=(",", ":"),
/// sort_keys=True)`.
#[must_use]
pub fn dumps(value: &ClefJson) -> String {
    let mut out = String::new();
    write_dumps(&mut out, value);
    out
}

/// The reference `render`: a string as it is, anything else as compact
/// sorted JSON.
#[must_use]
pub fn render(value: &ClefJson) -> String {
    match value {
        ClefJson::String(text) => text.clone(),
        other => dumps(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_follow_python_repr() {
        let cases = [
            ("1.0", "1.0"),
            ("1.50", "1.5"),
            ("-2.25", "-2.25"),
            ("0.1", "0.1"),
            ("0.0001", "0.0001"),
            ("0.00001", "1e-05"),
            ("1e-5", "1e-05"),
            ("1E2", "100.0"),
            ("1e16", "1e+16"),
            ("1e15", "1000000000000000.0"),
            ("1e22", "1e+22"),
            ("1.5e300", "1.5e+300"),
            ("1e400", "Infinity"),
            ("-1e400", "-Infinity"),
            ("5e-324", "5e-324"),
            ("1.7976931348623157e308", "1.7976931348623157e+308"),
            ("3.141592653589793", "3.141592653589793"),
            ("123456789.123456789", "123456789.12345679"),
            ("0.30000000000000004", "0.30000000000000004"),
            ("-0.0", "-0.0"),
            ("1234.5e-2", "12.345"),
            ("6.02214076e23", "6.02214076e+23"),
            ("1.1e16", "1.1e+16"),
            ("999999999999999.9", "999999999999999.9"),
            ("9999999999999998.0", "9999999999999998.0"),
            ("0.000123", "0.000123"),
            ("1.0e+3", "1000.0"),
        ];
        for (input, expected) in cases {
            assert_eq!(render_number(input), expected, "{input}");
        }
    }

    #[test]
    fn integers_stay_exact() {
        assert_eq!(render_number("-0"), "0");
        assert_eq!(render_number("0"), "0");
        assert_eq!(
            render_number("123456789012345678901234567890"),
            "123456789012345678901234567890"
        );
        assert_eq!(
            render_number("-9223372036854775809"),
            "-9223372036854775809"
        );
    }

    #[test]
    fn dumps_sorts_keys_and_escapes_like_python() {
        let value =
            parse(r#"{"b": [1, 2.0, "x\u0001\n\"é"], "a": {"z": null, "Ä": true, "B": false}}"#)
                .expect("json");
        assert_eq!(
            dumps(&value),
            "{\"a\":{\"B\":false,\"z\":null,\"Ä\":true},\"b\":[1,2.0,\"x\\u0001\\n\\\"é\"]}"
        );
        assert_eq!(
            render(&ClefJson::String(String::from("plain é"))),
            "plain é"
        );
    }

    #[test]
    fn repeated_keys_keep_first_position_and_last_value() {
        let value = parse(r#"{"a": 1, "b": 2, "a": 3}"#).expect("json");
        assert_eq!(
            value,
            ClefJson::Object(vec![
                (String::from("a"), ClefJson::Number(String::from("3"))),
                (String::from("b"), ClefJson::Number(String::from("2"))),
            ])
        );
    }

    #[test]
    fn refuses_what_python_would_not_round_trip() {
        assert!(parse(r#""\ud800""#).is_err());
        assert!(parse("NaN").is_err());
        assert!(parse("[1,]").is_err());
        assert_eq!(
            parse(r#""🚀""#).expect("pair"),
            ClefJson::String(String::from("🚀"))
        );
    }

    #[test]
    fn truthiness_matches_python() {
        for falsy in ["null", "false", "0", "-0", "0.0", "\"\"", "[]", "{}"] {
            assert!(!parse(falsy).expect("json").is_truthy(), "{falsy}");
        }
        for truthy in ["true", "1", "0.5", "\"x\"", "[0]", "{\"a\":0}"] {
            assert!(parse(truthy).expect("json").is_truthy(), "{truthy}");
        }
    }
}
