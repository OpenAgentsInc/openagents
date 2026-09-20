//! Helpers the integration tests share: fixture loading and hex codecs.

#![allow(dead_code)]

use std::path::PathBuf;

pub fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

pub fn fixture_json(relative: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(fixture_path(relative))
        .unwrap_or_else(|error| panic!("read fixture {relative}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parse fixture {relative}: {error}"))
}

pub fn hex_bytes(value: &str) -> Vec<u8> {
    assert!(
        value.len().is_multiple_of(2),
        "hex value has odd length: {value:?}"
    );
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_digit(pair[0]) << 4) | hex_digit(pair[1]))
        .collect()
}

pub fn hex_array<const N: usize>(value: &str) -> [u8; N] {
    let bytes = hex_bytes(value);
    <[u8; N]>::try_from(bytes.as_slice())
        .unwrap_or_else(|_| panic!("expected {N} bytes of hex, received {value:?}"))
}

pub fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_digit(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        other => panic!("invalid hex digit {other:?}"),
    }
}
