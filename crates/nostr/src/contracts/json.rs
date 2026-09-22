//! Strict JSON parsing and RFC 8785 canonicalization.
//!
//! Parsing rejects duplicate keys, invalid Unicode, non-finite numbers,
//! and integers outside the safe range before a value is built. Canonical
//! bytes are a separate encoding: object keys sort by UTF-16 code unit,
//! and numbers follow ECMAScript `JSON.stringify`. A byte-artifact digest
//! hashes the original bytes, including whitespace. It is not this encoding.

use ryu_js::Buffer;
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

use super::error::{ContractError, RefusalCode};

/// The common decoded-body ceiling, in bytes.
pub const MAX_BODY_BYTES: usize = 1_048_576;
/// The deepest JSON array or object nesting a body may use.
pub const MAX_DEPTH: usize = 64;
/// The largest integer magnitude JSON numbers in these contracts may use.
pub const SAFE_INTEGER: i128 = 9_007_199_254_740_991;

/// Parse one JSON value under the shared ceilings.
///
/// # Errors
///
/// Returns [`RefusalCode::LimitExceeded`] when the body or its nesting is
/// over the ceiling, and [`RefusalCode::Malformed`] for every other
/// encoding failure, including duplicate keys and unsafe integers.
pub fn parse_strict(input: &[u8]) -> Result<Value, ContractError> {
    if input.len() > MAX_BODY_BYTES {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "body exceeds 1048576 bytes",
        ));
    }
    if input.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(malformed("leading byte order mark"));
    }
    let mut parser = Parser {
        input,
        index: 0,
        depth: 0,
    };
    parser.skip_ws()?;
    if parser.done() {
        return Err(malformed("empty body"));
    }
    let value = parser.parse_value()?;
    parser.skip_ws()?;
    if !parser.done() {
        return Err(malformed("trailing data"));
    }
    Ok(value)
}

/// RFC 8785 canonical JSON for `value`.
///
/// # Errors
///
/// Returns [`RefusalCode::Malformed`] when a number is non-finite.
pub fn jcs(value: &Value) -> Result<Vec<u8>, ContractError> {
    let mut out = String::new();
    write_jcs(&mut out, value)?;
    Ok(out.into_bytes())
}

/// `sha256:` plus the lowercase hex digest of `bytes`.
#[must_use]
pub fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::from("sha256:");
    for byte in digest {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

/// `sha256:` plus the digest of the JCS encoding of `value`.
///
/// # Errors
///
/// Returns the same errors as [`jcs`].
pub fn digest_value(value: &Value) -> Result<String, ContractError> {
    Ok(digest_bytes(&jcs(value)?))
}

fn write_jcs(out: &mut String, value: &Value) -> Result<(), ContractError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&format_number(number)?),
        Value::String(text) => push_string(out, text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_jcs(out, item)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|left, right| utf16_cmp(left, right));
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_string(out, key);
                out.push(':');
                write_jcs(out, &map[*key])?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn format_number(number: &Number) -> Result<String, ContractError> {
    let value = number
        .as_f64()
        .or_else(|| number.as_i64().map(|value| value as f64))
        .or_else(|| number.as_u64().map(|value| value as f64))
        .ok_or_else(|| malformed("number is not a finite JSON number"))?;
    if !value.is_finite() {
        return Err(malformed("non-finite number"));
    }
    let mut buffer = Buffer::new();
    Ok(buffer.format_finite(value).to_owned())
}

fn push_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

fn utf16_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left = left.encode_utf16();
    let mut right = right.encode_utf16();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(a), Some(b)) if a != b => return a.cmp(&b),
            (Some(_), Some(_)) => {}
        }
    }
}

struct Parser<'a> {
    input: &'a [u8],
    index: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn done(&self) -> bool {
        self.index >= self.input.len()
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.index).copied()
    }

    fn bump(&mut self) -> Result<u8, ContractError> {
        let byte = self.peek().ok_or_else(|| malformed("truncated json"))?;
        self.index += 1;
        Ok(byte)
    }

    fn skip_ws(&mut self) -> Result<(), ContractError> {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.index += 1;
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), ContractError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ContractError::new(
                RefusalCode::LimitExceeded,
                "json nesting exceeds 64",
            ));
        }
        Ok(())
    }

    fn parse_value(&mut self) -> Result<Value, ContractError> {
        match self.peek() {
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(malformed("unexpected byte")),
            None => Err(malformed("truncated json")),
        }
    }

    fn literal(&mut self, expected: &[u8], value: Value) -> Result<Value, ContractError> {
        for byte in expected {
            if self.bump()? != *byte {
                return Err(malformed("invalid literal"));
            }
        }
        Ok(value)
    }

    fn parse_array(&mut self) -> Result<Value, ContractError> {
        self.enter()?;
        self.bump()?;
        let mut items = Vec::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b']') {
                self.bump()?;
                self.depth -= 1;
                return Ok(Value::Array(items));
            }
            if !items.is_empty() {
                if self.bump()? != b',' {
                    return Err(malformed("array element was not comma-separated"));
                }
                self.skip_ws()?;
            }
            items.push(self.parse_value()?);
        }
    }

    fn parse_object(&mut self) -> Result<Value, ContractError> {
        self.enter()?;
        self.bump()?;
        let mut map = Map::new();
        let mut seen = Vec::<String>::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b'}') {
                self.bump()?;
                self.depth -= 1;
                return Ok(Value::Object(map));
            }
            if !seen.is_empty() {
                if self.bump()? != b',' {
                    return Err(malformed("object entry was not comma-separated"));
                }
                self.skip_ws()?;
            }
            if self.peek() != Some(b'"') {
                return Err(malformed("object key is not a string"));
            }
            let key = self.parse_string()?;
            if seen.iter().any(|existing| existing == &key) {
                return Err(malformed("duplicate object key"));
            }
            seen.push(key.clone());
            self.skip_ws()?;
            if self.bump()? != b':' {
                return Err(malformed("object entry is missing a colon"));
            }
            self.skip_ws()?;
            let value = self.parse_value()?;
            map.insert(key, value);
        }
    }

    fn parse_string(&mut self) -> Result<String, ContractError> {
        if self.bump()? != b'"' {
            return Err(malformed("string did not open"));
        }
        let mut out = String::new();
        loop {
            match self.bump()? {
                b'"' => return Ok(out),
                b'\\' => out.push(self.parse_escape()?),
                byte if byte < 0x20 => return Err(malformed("raw control character in string")),
                byte if byte < 0x80 => out.push(char::from(byte)),
                byte => out.push(self.parse_utf8(byte)?),
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char, ContractError> {
        match self.bump()? {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{0008}'),
            b'f' => Ok('\u{000c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.parse_unicode(),
            _ => Err(malformed("invalid string escape")),
        }
    }

    fn parse_unicode(&mut self) -> Result<char, ContractError> {
        let unit = self.hex4()?;
        let value = match unit {
            0xD800..=0xDBFF => {
                if self.bump()? != b'\\' || self.bump()? != b'u' {
                    return Err(malformed("unpaired surrogate"));
                }
                let low = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(malformed("unpaired surrogate"));
                }
                let code = 0x10000 + (((unit - 0xD800) as u32) << 10) + (low - 0xDC00) as u32;
                char::from_u32(code).ok_or_else(|| malformed("invalid unicode scalar"))?
            }
            0xDC00..=0xDFFF => return Err(malformed("unpaired surrogate")),
            other => char::from_u32(u32::from(other))
                .ok_or_else(|| malformed("invalid unicode scalar"))?,
        };
        Ok(value)
    }

    fn hex4(&mut self) -> Result<u16, ContractError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let nibble = match self.bump()? {
                byte @ b'0'..=b'9' => byte - b'0',
                byte @ b'a'..=b'f' => byte - b'a' + 10,
                byte @ b'A'..=b'F' => byte - b'A' + 10,
                _ => return Err(malformed("invalid unicode escape")),
            };
            value = (value << 4) | u16::from(nibble);
        }
        Ok(value)
    }

    fn parse_utf8(&mut self, first: u8) -> Result<char, ContractError> {
        let width = if first & 0xE0 == 0xC0 {
            2
        } else if first & 0xF0 == 0xE0 {
            3
        } else if first & 0xF8 == 0xF0 {
            4
        } else {
            return Err(malformed("invalid utf-8"));
        };
        let mut bytes = [first, 0, 0, 0];
        for slot in &mut bytes[1..width] {
            let next = self.bump()?;
            if next & 0xC0 != 0x80 {
                return Err(malformed("invalid utf-8"));
            }
            *slot = next;
        }
        let text = std::str::from_utf8(&bytes[..width]).map_err(|_| malformed("invalid utf-8"))?;
        text.chars()
            .next()
            .ok_or_else(|| malformed("invalid utf-8"))
    }

    fn parse_number(&mut self) -> Result<Value, ContractError> {
        let start = self.index;
        if self.peek() == Some(b'-') {
            self.bump()?;
        }
        match self.bump()? {
            b'0' => {}
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump()?;
                }
            }
            _ => return Err(malformed("invalid number")),
        }
        let mut integer = true;
        if self.peek() == Some(b'.') {
            integer = false;
            self.bump()?;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(malformed("invalid fraction"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump()?;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            integer = false;
            self.bump()?;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump()?;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(malformed("invalid exponent"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump()?;
            }
        }
        let token = std::str::from_utf8(&self.input[start..self.index])
            .map_err(|_| malformed("invalid number"))?;
        if integer {
            let digits = token.trim_start_matches('-');
            if digits.len() > 38 {
                return Err(malformed("integer outside the safe range"));
            }
            let magnitude: i128 = digits
                .parse()
                .map_err(|_| malformed("integer outside the safe range"))?;
            let signed = if token.starts_with('-') {
                -magnitude
            } else {
                magnitude
            };
            if !(-SAFE_INTEGER..=SAFE_INTEGER).contains(&signed) {
                return Err(malformed("integer outside the safe range"));
            }
            let number = if signed >= 0 {
                Number::from(u64::try_from(signed).expect("safe integer fits u64"))
            } else {
                Number::from(i64::try_from(signed).expect("safe integer fits i64"))
            };
            return Ok(Value::Number(number));
        }
        // `serde_json`'s parser rounds some decimals one ulp away from the
        // ECMAScript conversion RFC 8785 requires. Rust's `f64` parser
        // matches that conversion on the published vectors.
        let value: f64 = token.parse().map_err(|_| malformed("invalid number"))?;
        let number = Number::from_f64(value).ok_or_else(|| malformed("non-finite number"))?;
        Ok(Value::Number(number))
    }
}

fn malformed(detail: &str) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_the_published_cross_language_sample() {
        let input = concat!(
            r#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,1e-27],"#,
            r#""string":"\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/","#,
            r#""literals":[null,true,false]}"#
        );
        let value = parse_strict(input.as_bytes()).expect("sample parses");
        let encoded = jcs(&value).expect("sample canonicalizes");
        let expected = concat!(
            r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"#,
            r#""string":"€$\u000f\nA'B\"\\\\\"/"}"#
        );
        assert_eq!(String::from_utf8(encoded).expect("utf-8"), expected);
    }

    #[test]
    fn sorts_object_keys_by_utf16_code_unit() {
        let input = "{\"b\":1,\"a\":2,\"é\":3,\"𐀀\":6,\"😀\":4,\"￿\":5}";
        let value = parse_strict(input.as_bytes()).expect("keys parse");
        let encoded = String::from_utf8(jcs(&value).expect("jcs")).expect("utf-8");
        assert_eq!(
            encoded,
            "{\"a\":2,\"b\":1,\"é\":3,\"𐀀\":6,\"😀\":4,\"￿\":5}"
        );
    }

    #[test]
    fn rejects_duplicate_keys_unsafe_integers_and_deep_nesting() {
        assert_eq!(
            parse_strict(br#"{"a":1,"a":2}"#).unwrap_err().code,
            RefusalCode::Malformed
        );
        assert_eq!(
            parse_strict(b"9007199254740992").unwrap_err().code,
            RefusalCode::Malformed
        );
        assert_eq!(
            parse_strict(br#""\uD800""#).unwrap_err().code,
            RefusalCode::Malformed
        );
        let deep = format!("{}1{}", "[".repeat(65), "]".repeat(65));
        assert_eq!(
            parse_strict(deep.as_bytes()).unwrap_err().code,
            RefusalCode::LimitExceeded
        );
        let huge = vec![b' '; MAX_BODY_BYTES + 1];
        assert_eq!(
            parse_strict(&huge).unwrap_err().code,
            RefusalCode::LimitExceeded
        );
    }

    #[test]
    fn byte_digest_keeps_whitespace_and_jcs_digest_does_not() {
        let raw = br#"{ "a" : 1 }"#;
        let parsed = parse_strict(raw).expect("parses");
        assert_ne!(digest_bytes(raw), digest_value(&parsed).expect("digest"));
        assert_eq!(
            digest_value(&parsed).expect("digest"),
            digest_bytes(br#"{"a":1}"#)
        );
    }
}
