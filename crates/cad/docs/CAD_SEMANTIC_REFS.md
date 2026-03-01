# CAD Semantic Reference Registry

This document defines the deterministic semantic reference registry in
`crates/cad/src/semantic_refs.rs`.

## Goal

Provide stable semantic names for downstream ops and AI commands, for example:

- `rack_outer_face`
- `mount_hole_pattern`
- `vent_face_set`

## Registry Model

`CadSemanticRefRegistry` stores semantic refs in a deterministic `BTreeMap` with:

- semantic ref key
- current entity id
- source feature id
- status (`valid`, `expired`, `rebound`)

Status meanings:

- `valid`: semantic ref resolves to current entity id
- `expired`: semantic ref no longer resolves after rebuild/reconcile
- `rebound`: semantic ref still exists but remapped to a new entity id

## Determinism Rules

- Semantic ref names are validated (lowercase ASCII + `_.-` only).
- Reconcile behavior is deterministic for a given stable-id map.
- Serialization uses sorted map ordering.

## `.apcad` Persistence

`.apcad` persists the registry as `stable_ids` (`semantic_ref -> entity_id`).

Helpers in `crates/cad/src/format.rs`:

- `set_semantic_ref_registry(...)`
- `semantic_ref_registry(...)`

Persistence behavior:

- `valid` and `rebound` refs are persisted
- `expired` refs are not persisted in `stable_ids`

## Out of Scope

- Full topological naming for arbitrary B-Rep edits
- Automatic semantic inference from arbitrary user geometry
