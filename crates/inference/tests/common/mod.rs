//! Helpers shared by the integration tests.
#![allow(dead_code)]

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// A value with every `null` object field removed, recursively, and
/// integral floats made integers: the comparison "nothing was lost" uses,
/// since an absent optional field and a `null` one say the same thing.
pub fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| (key.clone(), canonical(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        Value::Number(number) => match number.as_f64() {
            Some(float) if number.is_f64() && float.fract() == 0.0 && float.abs() < 1e15 =>
            {
                #[allow(clippy::cast_possible_truncation)]
                Value::from(float as i64)
            }
            _ => value.clone(),
        },
        other => other.clone(),
    }
}

/// Every field of `part` is in `whole` with the same value. The encoder
/// may add fields (a required field the input left out, at its default);
/// it may not drop or change one.
pub fn subset(part: &Value, whole: &Value, path: String) -> Result<(), String> {
    match (part, whole) {
        (Value::Object(part), Value::Object(whole)) => {
            for (key, value) in part {
                let at = format!("{path}.{key}");
                match whole.get(key) {
                    Some(other) => subset(value, other, at)?,
                    None => return Err(at),
                }
            }
            Ok(())
        }
        (Value::Array(part), Value::Array(whole)) if part.len() == whole.len() => {
            for (index, (value, other)) in part.iter().zip(whole).enumerate() {
                subset(value, other, format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        (part, whole) if part == whole => Ok(()),
        _ => Err(path),
    }
}

/// Decodes `value` as `T`, re-encodes it, and asserts nothing was lost;
/// returns the decoded value.
pub fn lossless<T: Serialize + DeserializeOwned>(value: &Value, what: &str) -> T {
    let decoded: T = serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{what}: does not decode: {error}\n{value:#}"));
    let encoded = serde_json::to_value(&decoded).expect("encodes");
    if let Err(path) = subset(&canonical(value), &canonical(&encoded), String::new()) {
        panic!("{what}: re-encoding lost or changed {path}\nsent: {value}\ngot:  {encoded}");
    }
    let again: T = serde_json::from_value(encoded).expect("re-decodes");
    assert_eq!(
        serde_json::to_value(&again).expect("encodes"),
        serde_json::to_value(&decoded).expect("encodes"),
        "{what}: decode is not stable"
    );
    decoded
}

/// A recorded upstream stream from the chat worker's fixtures.
pub fn recorded(name: &str) -> String {
    let path = format!(
        "{}/../coder/fixtures/gateway/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// A spec example from `fixtures/spec/`.
pub fn spec(name: &str) -> Value {
    let path = format!("{}/fixtures/spec/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    serde_json::from_str(&text).expect("fixture is JSON")
}

/// The recorded streams, by file name.
pub const RECORDED: [&str; 3] = [
    "google-gemini-3.8-flash.sse",
    "zai-glm-5.3-flash.sse",
    "stealth-space-bunny-alpha.sse",
];

/// A small deterministic generator, for splitting streams at arbitrary
/// points without a dependency.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self, below: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap_or(0) % below.max(1)
    }
}
