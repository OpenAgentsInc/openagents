# CAD Mesh Contract

`crates/cad::mesh` defines the renderer-facing mesh payload contract for CAD panes.

## Scope

- In scope:
  - stable payload types for vertices, triangle indices, material slots, edges, and bounds
  - explicit validation for index/material/bounds failures
  - deterministic little-endian binary sections for render backends
- Out of scope:
  - tessellation from solids (tracked by backlog item 34 / issue #2485)
  - WGPUI mesh primitive/render-pass integration (tracked by issues #2486 and #2487)

## Contract Notes

- Binary contract version is `CAD_MESH_BINARY_CONTRACT_VERSION = 1`.
- Vertex/material/edge structs are `#[repr(C)]` and tested for stable size/alignment.
- `CadMeshPayload::validate_contract` returns explicit `CadError::InvalidPrimitive` failures.
- `CadMeshPayload::to_binary_payload` emits deterministic LE byte sections plus a stable hash.

## Verification

- `cargo test -p openagents-cad mesh::tests::binary_contract_layout_is_stable`
- `cargo test -p openagents-cad mesh::tests::mesh_payload_binary_encoding_is_deterministic`
- `cargo test -p openagents-cad`
