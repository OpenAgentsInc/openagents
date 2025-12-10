# Phase 2: Value Type System - 2025-12-10

## Summary

Implemented Phase 2 of the Unit to GPUI migration plan: Dynamic Value type with JavaScript-like coercion.

## New File: `crates/unit/src/value.rs` (~430 lines)

### Value Enum

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}
```

### Features

#### Type Checking
- `type_name()` - Returns "null", "boolean", "number", "string", "array", "object"
- `is_null()`, `is_boolean()`, `is_number()`, `is_string()`, `is_array()`, `is_object()`

#### Non-Coercing Accessors
- `as_boolean()` -> `Option<bool>`
- `as_number()` -> `Option<f64>`
- `as_str()` -> `Option<&str>`
- `as_array()` / `as_array_mut()` -> `Option<&[Value]>` / `Option<&mut Vec<Value>>`
- `as_object()` / `as_object_mut()` -> `Option<&HashMap<String, Value>>`

#### JS-Like Coercion
- `to_boolean()` - Falsy: null, false, 0, NaN, "". Truthy: everything else
- `to_number()` - null->0, bool->0/1, string->parse or NaN
- `to_string_value()` - JS semantics (array joins with comma, object->"[object Object]")

#### Object/Array Access
- `get(key)` / `get_mut(key)` - Works for both objects and arrays
- `set(key, value)` - Works for both objects and arrays
- `deep_get(path)` - Dot notation (e.g., "user.name", "items.0")
- `deep_set(path, value)` - Dot notation
- `len()` / `is_empty()` - For arrays, objects, and strings

#### From Conversions
- `bool`, `i32`, `i64`, `f32`, `f64`, `&str`, `String`
- `Vec<T>` where `T: Into<Value>`
- `Option<T>` where `T: Into<Value>` (None -> Null)

#### serde_json Interop
- `From<serde_json::Value>` and `From<Value> for serde_json::Value`
- Full round-trip serialization

## Tests Added

14 new tests covering:
- Constructors and type checking
- Boolean/number/string coercion
- Object and array access
- Deep get/set with dot notation
- From conversions
- Serde roundtrip
- serde_json interop
- Display formatting

## Test Results

```
running 72 tests
test result: ok. 72 passed; 0 failed
```

(+14 from Phase 1's 58)

## Usage Example

```rust
use unit::Value;

// Create values
let user = Value::object([
    ("name", Value::from("Alice")),
    ("age", Value::from(30)),
    ("scores", Value::array([Value::from(95), Value::from(87)])),
]);

// Access
assert_eq!(user.get("name"), Some(&Value::from("Alice")));
assert_eq!(user.deep_get("scores.0"), Some(&Value::from(95)));

// Coercion
assert!(user.get("age").unwrap().to_boolean()); // true (non-zero)
assert_eq!(user.get("name").unwrap().to_number(), f64::NAN); // can't parse

// From JSON
let json: serde_json::Value = serde_json::json!({"x": 1});
let value: Value = json.into();
```

## Lines Added

| File | Lines |
|------|-------|
| `value.rs` | ~430 |
| `lib.rs` | +2 |
| **Total** | ~432 |

---

**Status:** Phase 2 complete. Total tests: 72 passing.
