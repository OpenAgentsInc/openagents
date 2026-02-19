# Canonical Hashing

This document explains the canonical JSON serialization and SHA-256 hashing used in the protocol crate.

## Why Deterministic Hashing?

Deterministic hashing is critical for:

1. **Job Identification**: Every job request has a unique hash that identifies it
2. **Deduplication**: Identical requests produce identical hashes
3. **Verification**: Providers can verify they received the correct request
4. **Audit Trails**: Hashes link requests to responses in provenance
5. **Cross-Implementation Compatibility**: Same hash in Rust, TypeScript, Python

## Canonical JSON (RFC 8785 JCS)

The protocol follows RFC 8785 (JSON Canonicalization Scheme) principles:

### 1. Object Keys Sorted Lexicographically

```json
// Input (any order)
{"zebra": 1, "apple": 2, "mango": 3}

// Canonical output (sorted)
{"apple":2,"mango":3,"zebra":1}
```

### 2. No Whitespace

```json
// Input (formatted)
{
  "name": "test",
  "value": 42
}

// Canonical output (no whitespace)
{"name":"test","value":42}
```

### 3. Numbers in Shortest Form

- No leading zeros
- No trailing zeros after decimal
- No positive sign prefix

```json
// Input
{"value": 1.0, "count": 007}

// Canonical output
{"count":7,"value":1}
```

### 4. Nested Objects Recursively Sorted

```rust
#[derive(Serialize)]
struct Outer {
    z_field: Inner,
    a_field: i32,
}

#[derive(Serialize)]
struct Inner {
    z_inner: String,
    a_inner: i32,
}

let value = Outer {
    z_field: Inner {
        z_inner: "z".into(),
        a_inner: 1,
    },
    a_field: 2,
};

let json = canonical_json(&value).unwrap();
// {"a_field":2,"z_field":{"a_inner":1,"z_inner":"z"}}
```

### 5. Array Order Preserved

Arrays maintain their original order (not sorted):

```rust
let value = vec![3, 1, 2];
let json = canonical_json(&value).unwrap();
// [3,1,2]  -- order preserved
```

## API Reference

### canonical_json

Serialize a value to canonical JSON.

```rust
use protocol::hash::canonical_json;
use serde::Serialize;

#[derive(Serialize)]
struct MyData {
    name: String,
    value: i32,
}

let data = MyData {
    name: "test".into(),
    value: 42,
};

let json = canonical_json(&data).unwrap();
assert_eq!(json, r#"{"name":"test","value":42}"#);
```

### canonical_hash

Compute SHA-256 hash of canonical JSON.

```rust
use protocol::hash::canonical_hash;

let hash = canonical_hash(&data).unwrap();
assert_eq!(hash.len(), 64);  // 32 bytes = 64 hex chars
```

### Hashable Trait

All serializable types automatically implement `Hashable`:

```rust
use protocol::hash::Hashable;

let hash = data.compute_hash().unwrap();
```

For job requests, use the `JobRequest` trait method:

```rust
use protocol::jobs::JobRequest;

let request = ChunkAnalysisRequest { ... };
let hash = request.compute_hash().unwrap();
```

## Implementation Details

### Algorithm

1. Serialize to `serde_json::Value`
2. Recursively sort all object keys
3. Serialize to string without whitespace
4. Compute SHA-256 hash
5. Encode as lowercase hex

### Code

```rust
pub fn canonical_hash<T: Serialize + ?Sized>(value: &T) -> Result<String, HashError> {
    let json = canonical_json(value)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted: Vec<(String, Value)> = map
                .into_iter()
                .map(|(k, v)| (k, canonicalize_value(v)))
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(canonicalize_value).collect())
        }
        other => other,
    }
}
```

## Cross-Implementation Compatibility

To ensure the same hash across implementations:

### TypeScript

```typescript
function canonicalJson(obj: any): string {
  if (Array.isArray(obj)) {
    return '[' + obj.map(canonicalJson).join(',') + ']';
  }
  if (obj !== null && typeof obj === 'object') {
    const keys = Object.keys(obj).sort();
    return '{' + keys.map(k =>
      JSON.stringify(k) + ':' + canonicalJson(obj[k])
    ).join(',') + '}';
  }
  return JSON.stringify(obj);
}

async function canonicalHash(obj: any): Promise<string> {
  const json = canonicalJson(obj);
  const encoder = new TextEncoder();
  const data = encoder.encode(json);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  return Array.from(new Uint8Array(hashBuffer))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}
```

### Python

```python
import json
import hashlib

def canonical_json(obj):
    if isinstance(obj, dict):
        sorted_items = sorted(obj.items(), key=lambda x: x[0])
        return '{' + ','.join(
            f'{json.dumps(k)}:{canonical_json(v)}'
            for k, v in sorted_items
        ) + '}'
    elif isinstance(obj, list):
        return '[' + ','.join(canonical_json(item) for item in obj) + ']'
    else:
        return json.dumps(obj)

def canonical_hash(obj):
    json_str = canonical_json(obj)
    return hashlib.sha256(json_str.encode()).hexdigest()
```

## Testing Hash Compatibility

To verify cross-implementation compatibility:

```rust
#[test]
fn test_known_hash() {
    // This hash should match across all implementations
    let data = serde_json::json!({
        "zebra": "z",
        "apple": 1,
        "mango": true
    });

    let hash = canonical_hash(&data).unwrap();

    // Verify this matches TypeScript/Python implementations
    assert_eq!(
        hash,
        "expected_hash_here"
    );
}
```

## Error Handling

```rust
use protocol::hash::{canonical_hash, HashError};

match canonical_hash(&data) {
    Ok(hash) => println!("Hash: {}", hash),
    Err(HashError::Serialization(msg)) => {
        eprintln!("Serialization failed: {}", msg);
    }
}
```

## Best Practices

1. **Always use typed structs**: Avoid raw JSON to prevent serialization inconsistencies
2. **Verify hashes on receipt**: Providers should verify the job hash matches
3. **Store hashes in provenance**: Link request hash to response for audit
4. **Test cross-implementation**: Verify hashes match across languages

## Security Considerations

- SHA-256 is cryptographically secure for content addressing
- Hashes do not prove authenticity (use signatures for that)
- Hashes are deterministic but not reversible
