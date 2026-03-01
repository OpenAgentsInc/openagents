# CAD Tessellation Path

`crates/cad::tessellation` converts deterministic rebuild results into renderer-facing mesh payloads.

## Scope

- In scope:
  - deterministic conversion from `DeterministicRebuildResult` + feature graph to `CadMeshPayload`
  - rack demo primitive handlers (`primitive.box.v1`, `cut.hole.v1`, `linear.pattern.v1`, `fillet.placeholder.v1`)
  - explicit failure for unknown operation keys (no silent fallback)
  - structured `CadTessellationReceipt` for mesh observability
- Out of scope:
  - WGPUI primitive/render pass integration (issues #2486, #2487)
  - pane draw integration with real mesh scene data (issue #2488)

## Determinism

- Rebuild order must match graph topo order.
- Mesh IDs are deterministic: `mesh.<variant>.<rebuild_hash>`.
- Binary mesh hash is deterministic from encoded LE sections.
- Golden fixtures pin expected rack-variant mesh signatures:
  - `crates/cad/tests/goldens/tessellation_rack_primitives.json`

## Verification

- `cargo test -p openagents-cad tessellation::tests::tessellation_is_deterministic_for_demo_rack_variants`
- `cargo test -p openagents-cad tessellation::tests::tessellation_rejects_unknown_operation_keys`
- `cargo test -p openagents-cad`
