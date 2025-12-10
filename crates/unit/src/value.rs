//! Value: Dynamic value type with JS-like coercion
//!
//! Provides a universal value type that can represent any data flowing
//! through the Unit graph. Supports JSON-compatible types with JavaScript-like
//! type coercion for flexible dataflow programming.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Dynamic value type for Unit dataflow
///
/// Represents any value that can flow through pins and connections.
/// Follows JSON semantics with JavaScript-like coercion rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Null/undefined value
    Null,
    /// Boolean value
    Boolean(bool),
    /// Numeric value (IEEE 754 f64)
    Number(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<Value>),
    /// Object/map of string keys to values
    Object(HashMap<String, Value>),
}

impl Value {
    // ========== Constructors ==========

    /// Create a null value
    pub fn null() -> Self {
        Self::Null
    }

    /// Create a boolean value
    pub fn boolean(b: bool) -> Self {
        Self::Boolean(b)
    }

    /// Create a number value
    pub fn number(n: impl Into<f64>) -> Self {
        Self::Number(n.into())
    }

    /// Create a string value
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    /// Create an array value
    pub fn array(items: impl IntoIterator<Item = Value>) -> Self {
        Self::Array(items.into_iter().collect())
    }

    /// Create an object value
    pub fn object(pairs: impl IntoIterator<Item = (impl Into<String>, Value)>) -> Self {
        Self::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    // ========== Type Checking ==========

    /// Get the type name of this value
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Check if value is boolean
    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Boolean(_))
    }

    /// Check if value is number
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    /// Check if value is string
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// Check if value is array
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// Check if value is object
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    // ========== Accessors (non-coercing) ==========

    /// Get as boolean if it is one
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as number if it is one
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as string if it is one
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as array if it is one
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Get as mutable array if it is one
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Get as object if it is one
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Get as mutable object if it is one
    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, Value>> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    // ========== Coercion (JS-like) ==========

    /// Convert to boolean (JS-like truthiness)
    ///
    /// Falsy: null, false, 0, NaN, ""
    /// Truthy: everything else
    pub fn to_boolean(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Boolean(b) => *b,
            Self::Number(n) => *n != 0.0 && !n.is_nan(),
            Self::String(s) => !s.is_empty(),
            Self::Array(_) => true,
            Self::Object(_) => true,
        }
    }

    /// Convert to number (JS-like coercion)
    ///
    /// - null -> 0
    /// - true -> 1, false -> 0
    /// - string -> parsed or NaN
    /// - array/object -> NaN
    pub fn to_number(&self) -> f64 {
        match self {
            Self::Null => 0.0,
            Self::Boolean(true) => 1.0,
            Self::Boolean(false) => 0.0,
            Self::Number(n) => *n,
            Self::String(s) => s.trim().parse().unwrap_or(f64::NAN),
            Self::Array(_) | Self::Object(_) => f64::NAN,
        }
    }

    /// Convert to string (JS-like coercion)
    pub fn to_string_value(&self) -> String {
        match self {
            Self::Null => "null".to_string(),
            Self::Boolean(true) => "true".to_string(),
            Self::Boolean(false) => "false".to_string(),
            Self::Number(n) => {
                if n.is_nan() {
                    "NaN".to_string()
                } else if n.is_infinite() {
                    if *n > 0.0 {
                        "Infinity".to_string()
                    } else {
                        "-Infinity".to_string()
                    }
                } else if *n == n.trunc() && n.abs() < 1e15 {
                    // Integer-like numbers without decimal
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Self::String(s) => s.clone(),
            Self::Array(arr) => arr
                .iter()
                .map(|v| v.to_string_value())
                .collect::<Vec<_>>()
                .join(","),
            Self::Object(_) => "[object Object]".to_string(),
        }
    }

    // ========== Object/Array Access ==========

    /// Get a value by key (for objects) or index (for arrays)
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(obj) => obj.get(key),
            Self::Array(arr) => key.parse::<usize>().ok().and_then(|i| arr.get(i)),
            _ => None,
        }
    }

    /// Get a mutable value by key (for objects) or index (for arrays)
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Self::Object(obj) => obj.get_mut(key),
            Self::Array(arr) => key.parse::<usize>().ok().and_then(|i| arr.get_mut(i)),
            _ => None,
        }
    }

    /// Set a value by key (for objects) or index (for arrays)
    pub fn set(&mut self, key: &str, value: Value) -> bool {
        match self {
            Self::Object(obj) => {
                obj.insert(key.to_string(), value);
                true
            }
            Self::Array(arr) => {
                if let Ok(i) = key.parse::<usize>() {
                    if i < arr.len() {
                        arr[i] = value;
                        return true;
                    } else if i == arr.len() {
                        arr.push(value);
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Deep get using dot notation path (e.g., "foo.bar.0.baz")
    pub fn deep_get(&self, path: &str) -> Option<&Value> {
        let mut current = self;
        for key in path.split('.') {
            current = current.get(key)?;
        }
        Some(current)
    }

    /// Deep set using dot notation path
    pub fn deep_set(&mut self, path: &str, value: Value) -> bool {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return false;
        }

        if parts.len() == 1 {
            return self.set(parts[0], value);
        }

        let mut current = self;
        for key in &parts[..parts.len() - 1] {
            current = match current.get_mut(key) {
                Some(v) => v,
                None => return false,
            };
        }

        current.set(parts[parts.len() - 1], value)
    }

    /// Get array length or object key count
    pub fn len(&self) -> Option<usize> {
        match self {
            Self::Array(arr) => Some(arr.len()),
            Self::Object(obj) => Some(obj.len()),
            Self::String(s) => Some(s.len()),
            _ => None,
        }
    }

    /// Check if array/object/string is empty
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|l| l == 0)
    }
}

// ========== Default ==========

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

// ========== Display ==========

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_value())
    }
}

// ========== From implementations ==========

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Self::Number(n as f64)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<f32> for Value {
    fn from(n: f32) -> Self {
        Self::Number(n as f64)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(|x| x.into()).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

// ========== serde_json interop ==========

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Boolean(b),
            serde_json::Value::Number(n) => Self::Number(n.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(arr) => Self::Array(arr.into_iter().map(Value::from).collect()),
            serde_json::Value::Object(obj) => {
                Self::Object(obj.into_iter().map(|(k, v)| (k, Value::from(v))).collect())
            }
        }
    }
}

impl From<Value> for serde_json::Value {
    fn from(v: Value) -> Self {
        match v {
            Value::Null => serde_json::Value::Null,
            Value::Boolean(b) => serde_json::Value::Bool(b),
            Value::Number(n) => serde_json::json!(n),
            Value::String(s) => serde_json::Value::String(s),
            Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            Value::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
        }
    }
}

// ========== Tests ==========

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructors() {
        assert!(Value::null().is_null());
        assert!(Value::boolean(true).is_boolean());
        assert!(Value::number(42).is_number());
        assert!(Value::string("hello").is_string());
        assert!(Value::array([Value::from(1), Value::from(2)]).is_array());
        assert!(Value::object([("a", Value::from(1))]).is_object());
    }

    #[test]
    fn test_type_name() {
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Boolean(true).type_name(), "boolean");
        assert_eq!(Value::Number(1.0).type_name(), "number");
        assert_eq!(Value::String("".into()).type_name(), "string");
        assert_eq!(Value::Array(vec![]).type_name(), "array");
        assert_eq!(Value::Object(HashMap::new()).type_name(), "object");
    }

    #[test]
    fn test_to_boolean() {
        // Falsy values
        assert!(!Value::Null.to_boolean());
        assert!(!Value::Boolean(false).to_boolean());
        assert!(!Value::Number(0.0).to_boolean());
        assert!(!Value::Number(f64::NAN).to_boolean());
        assert!(!Value::String("".into()).to_boolean());

        // Truthy values
        assert!(Value::Boolean(true).to_boolean());
        assert!(Value::Number(1.0).to_boolean());
        assert!(Value::Number(-1.0).to_boolean());
        assert!(Value::String("hello".into()).to_boolean());
        assert!(Value::Array(vec![]).to_boolean()); // Empty array is truthy in JS
        assert!(Value::Object(HashMap::new()).to_boolean()); // Empty object is truthy
    }

    #[test]
    fn test_to_number() {
        assert_eq!(Value::Null.to_number(), 0.0);
        assert_eq!(Value::Boolean(true).to_number(), 1.0);
        assert_eq!(Value::Boolean(false).to_number(), 0.0);
        assert_eq!(Value::Number(42.5).to_number(), 42.5);
        assert_eq!(Value::String("123".into()).to_number(), 123.0);
        assert_eq!(Value::String("  456  ".into()).to_number(), 456.0);
        assert!(Value::String("not a number".into()).to_number().is_nan());
        assert!(Value::Array(vec![]).to_number().is_nan());
    }

    #[test]
    fn test_to_string() {
        assert_eq!(Value::Null.to_string_value(), "null");
        assert_eq!(Value::Boolean(true).to_string_value(), "true");
        assert_eq!(Value::Boolean(false).to_string_value(), "false");
        assert_eq!(Value::Number(42.0).to_string_value(), "42");
        assert_eq!(Value::Number(3.14).to_string_value(), "3.14");
        assert_eq!(Value::Number(f64::NAN).to_string_value(), "NaN");
        assert_eq!(Value::Number(f64::INFINITY).to_string_value(), "Infinity");
        assert_eq!(Value::String("hello".into()).to_string_value(), "hello");
        assert_eq!(
            Value::Array(vec![Value::from(1), Value::from(2)]).to_string_value(),
            "1,2"
        );
        assert_eq!(
            Value::Object(HashMap::new()).to_string_value(),
            "[object Object]"
        );
    }

    #[test]
    fn test_object_access() {
        let mut obj = Value::object([("name", Value::from("Alice")), ("age", Value::from(30))]);

        assert_eq!(obj.get("name"), Some(&Value::from("Alice")));
        assert_eq!(obj.get("age"), Some(&Value::from(30)));
        assert_eq!(obj.get("missing"), None);

        obj.set("age", Value::from(31));
        assert_eq!(obj.get("age"), Some(&Value::from(31)));
    }

    #[test]
    fn test_array_access() {
        let mut arr = Value::array([Value::from(10), Value::from(20), Value::from(30)]);

        assert_eq!(arr.get("0"), Some(&Value::from(10)));
        assert_eq!(arr.get("2"), Some(&Value::from(30)));
        assert_eq!(arr.get("5"), None);

        arr.set("1", Value::from(25));
        assert_eq!(arr.get("1"), Some(&Value::from(25)));
    }

    #[test]
    fn test_deep_get() {
        let data = Value::object([
            ("user", Value::object([("name", Value::from("Bob"))])),
            (
                "items",
                Value::array([Value::from("a"), Value::from("b")]),
            ),
        ]);

        assert_eq!(
            data.deep_get("user.name"),
            Some(&Value::String("Bob".into()))
        );
        assert_eq!(data.deep_get("items.0"), Some(&Value::String("a".into())));
        assert_eq!(data.deep_get("items.1"), Some(&Value::String("b".into())));
        assert_eq!(data.deep_get("missing.path"), None);
    }

    #[test]
    fn test_deep_set() {
        let mut data = Value::object([("user", Value::object([("name", Value::from("Bob"))]))]);

        assert!(data.deep_set("user.name", Value::from("Carol")));
        assert_eq!(
            data.deep_get("user.name"),
            Some(&Value::String("Carol".into()))
        );
    }

    #[test]
    fn test_from_conversions() {
        assert_eq!(Value::from(true), Value::Boolean(true));
        assert_eq!(Value::from(42i32), Value::Number(42.0));
        assert_eq!(Value::from(42i64), Value::Number(42.0));
        assert_eq!(Value::from(3.14f32), Value::Number(3.14f32 as f64));
        assert_eq!(Value::from(3.14f64), Value::Number(3.14));
        assert_eq!(Value::from("hello"), Value::String("hello".into()));
        assert_eq!(
            Value::from(vec![1, 2, 3]),
            Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0)
            ])
        );
        assert_eq!(Value::from(None::<i32>), Value::Null);
        assert_eq!(Value::from(Some(42)), Value::Number(42.0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = Value::object([
            ("name", Value::from("test")),
            ("count", Value::from(42)),
            ("active", Value::from(true)),
            ("items", Value::array([Value::from(1), Value::from(2)])),
        ]);

        let json = serde_json::to_string(&original).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(original.get("name"), parsed.get("name"));
        assert_eq!(original.get("count"), parsed.get("count"));
        assert_eq!(original.get("active"), parsed.get("active"));
    }

    #[test]
    fn test_serde_json_interop() {
        let json_value = serde_json::json!({
            "name": "test",
            "count": 42,
            "nested": { "a": 1 }
        });

        let value: Value = json_value.clone().into();
        assert_eq!(value.get("name"), Some(&Value::from("test")));
        assert_eq!(value.get("count"), Some(&Value::from(42)));

        let back: serde_json::Value = value.into();
        // Numbers may differ in representation (42 vs 42.0), so compare semantically
        assert_eq!(back["name"], json_value["name"]);
        assert_eq!(back["count"].as_f64(), json_value["count"].as_f64());
        assert_eq!(back["nested"]["a"].as_f64(), json_value["nested"]["a"].as_f64());
    }

    #[test]
    fn test_len() {
        assert_eq!(Value::Array(vec![Value::from(1), Value::from(2)]).len(), Some(2));
        assert_eq!(Value::Object(HashMap::new()).len(), Some(0));
        assert_eq!(Value::String("hello".into()).len(), Some(5));
        assert_eq!(Value::Number(42.0).len(), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Value::from(42)), "42");
        assert_eq!(format!("{}", Value::from("hello")), "hello");
        assert_eq!(format!("{}", Value::Null), "null");
    }
}
