# `.apcad` Format Specification (Wave 1)

Related issue: [#2457](https://github.com/OpenAgentsInc/openagents/issues/2457)

## Purpose

Define a deterministic, versioned CAD document envelope for local-first persistence and diffable workflows.

## Versioning

- `header.format`: `apcad`
- `header.version`: `1` (Wave 1 baseline)
- `header.canonical_unit`: `mm`

## Canonical Envelope

Top-level shape:

```json
{
  "header": {
    "format": "apcad",
    "version": 1,
    "canonical_unit": "mm"
  },
  "document_id": "doc-uuid-or-stable-id",
  "stable_ids": {
    "feature.base": "sid-001"
  },
  "metadata": {
    "title": "Rack"
  },
  "analysis_cache": {
    "weight_kg": "2.71"
  }
}
```

## Deterministic Ordering Rules

1. Use deterministic map ordering for serialized key-value fields.
2. Stable IDs must preserve semantic labels and deterministic value mapping.
3. Repeated serialization of equivalent payloads must produce identical JSON bytes.
4. Optional `analysis_cache` is non-authoritative and may be omitted without invalidating document semantics.

## Stable IDs

- Stable IDs map semantic references to deterministic identifiers.
- Consumers must not derive behavior from insertion order; key order is lexical.
- Stable IDs are the persisted view of semantic refs from `CadSemanticRefRegistry`.
- `valid` and `rebound` refs are persisted; `expired` refs are excluded from `stable_ids`.

## Error Handling

- Parse/encode failures surface as explicit CAD serialization errors.
- Invalid version/format handling is reserved for upcoming migration support.

## Code References

- `crates/cad/src/format.rs`
- `crates/cad/src/lib.rs` (`CadError::Serialization`)
