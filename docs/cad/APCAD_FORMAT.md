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
  },
  "sketch": {
    "planes": {
      "plane.front": {
        "id": "plane.front",
        "name": "Front",
        "origin_mm": [0, 0, 0],
        "normal": [0, 0, 1],
        "x_axis": [1, 0, 0],
        "y_axis": [0, 1, 0]
      }
    },
    "entities": {
      "entity.line.001": {
        "kind": "line",
        "id": "entity.line.001",
        "plane_id": "plane.front",
        "start_mm": [0, 0],
        "end_mm": [120, 0],
        "anchor_ids": ["anchor.l.start", "anchor.l.end"],
        "construction": false
      }
    }
  }
}
```

## Deterministic Ordering Rules

1. Use deterministic map ordering for serialized key-value fields.
2. Stable IDs must preserve semantic labels and deterministic value mapping.
3. Repeated serialization of equivalent payloads must produce identical JSON bytes.
4. Optional `analysis_cache` is non-authoritative and may be omitted without invalidating document semantics.
5. Optional `sketch` block stores deterministic sketch planes/entities keyed by stable IDs.

## Sketch Model (Wave 2 Kickoff)

- `sketch.planes` is a deterministic map keyed by stable `plane_id`.
- `sketch.entities` is a deterministic map keyed by stable `entity_id`.
- Entity payloads support `line`, `arc`, and `point`.
- Every entity references an existing `plane_id` and explicit `anchor_ids`.
- Envelope parse validates sketch references; invalid cross-refs fail fast.

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
- `crates/cad/src/sketch.rs`
- `crates/cad/src/lib.rs` (`CadError::Serialization`)
