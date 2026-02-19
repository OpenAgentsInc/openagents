# Schema Versioning

Every job type in the protocol has a schema version that follows semantic versioning (semver). This document explains the versioning rules and how to handle schema changes.

## Version Format

Versions follow the `MAJOR.MINOR.PATCH` format:

```
oa.code_chunk_analysis.v1  →  schema_version: 1.0.0
```

## Version Bump Rules

### Patch (1.0.0 → 1.0.1)

**When to use:** Bug fixes, documentation updates, no schema changes.

Examples:
- Fix a typo in a field description
- Clarify validation rules without changing them
- Fix a serialization bug

**Compatibility:** Fully backward and forward compatible.

### Minor (1.0.0 → 1.1.0)

**When to use:** New optional fields, backward compatible additions.

Examples:
- Add a new optional field to the request
- Add a new optional field to the response
- Add new enum variants that are backward compatible

**Compatibility:**
- Older clients can read newer responses (ignore unknown fields)
- Newer clients can read older responses (use defaults for missing fields)

### Major (v1 → v2)

**When to use:** Breaking changes that require a new job type.

Examples:
- Remove a required field
- Change the type of an existing field
- Rename a field
- Change the meaning of a field

**Compatibility:** Incompatible. Use a new job type name:
```
oa.code_chunk_analysis.v1  →  oa.code_chunk_analysis.v2
```

## Compatibility Checks

The `SchemaVersion` type provides methods for checking compatibility:

```rust
use protocol::version::SchemaVersion;

let v100 = SchemaVersion::new(1, 0, 0);
let v110 = SchemaVersion::new(1, 1, 0);
let v200 = SchemaVersion::new(2, 0, 0);

// Same major version = compatible
assert!(v100.is_compatible_with(&v110));  // true
assert!(v100.is_compatible_with(&v200));  // false

// Newer version can read older data
assert!(v110.can_read(&v100));  // true
assert!(v100.can_read(&v110));  // false
```

## Adding New Fields

When adding a new optional field:

1. Add the field with `#[serde(skip_serializing_if = "Option::is_none")]`
2. Provide a sensible default
3. Bump the minor version

```rust
#[derive(Serialize, Deserialize)]
pub struct MyRequest {
    pub existing_field: String,

    // New in 1.1.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_optional_field: Option<String>,
}
```

## Deprecating Fields

When deprecating a field:

1. Add a doc comment explaining the deprecation
2. Keep the field for at least one major version
3. Document the migration path

```rust
#[derive(Serialize, Deserialize)]
pub struct MyRequest {
    /// DEPRECATED in 1.2.0: Use `new_field` instead.
    /// Will be removed in 2.0.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_field: Option<String>,

    /// Replacement for `old_field`. Added in 1.2.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_field: Option<String>,
}
```

## Hash Stability

The canonical hash of a job request must remain stable:
- Same input → same hash, always
- Across versions (if compatible)
- Across implementations (Rust, TypeScript, Python)

This means:
- Field ordering is canonicalized (sorted alphabetically)
- No whitespace in JSON
- Numbers use shortest representation
- Strings use minimal escaping

## Job Type Naming

Job types follow the pattern:
```
oa.<category>_<name>.v<major>
```

Examples:
- `oa.code_chunk_analysis.v1`
- `oa.retrieval_rerank.v1`
- `oa.sandbox_run.v1`

When creating a new major version:
- Keep the old job type (for backward compatibility)
- Create a new job type with incremented major version
- Document migration between versions
