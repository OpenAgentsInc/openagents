# CAD CLI Commands

Issue coverage: `VCAD-PARITY-084`

## Scope

In scope:
- Implement `openagents-cad-cli` command behaviors for:
  - `export`
  - `import`
  - `info`
- Reuse deterministic CAD IO APIs from `openagents-cad` (`stl`, `glb`, `step`, `step_import`).
- Keep scaffold compatibility from `VCAD-PARITY-083` for bare command invocations.

Out of scope:
- Full `.vcad` document compatibility.
- TUI/interactive CLI surfaces.
- Non-IO command families.

## Command Contract

### `export`

Usage:
- `openagents-cad-cli export <input_mesh_json> <output.stl|output.glb|output.step|output.stp>`

Behavior:
- Reads `CadMeshPayload` JSON.
- Writes deterministic output bytes by extension:
  - `.stl` via `export_stl_from_mesh`
  - `.glb` via `export_glb_from_mesh`
  - `.step`/`.stp` via `export_step_from_mesh`
- Success message format:
  - `Exported <FORMAT> to <path>`

### `import`

Usage:
- `openagents-cad-cli import <input.stl|input.step|input.stp|input.cad0|input.vcadc> <output_json> [--name <id>]`

Behavior:
- `.stl` input:
  - parses via `import_stl_to_mesh`
  - writes `CadMeshPayload` JSON
- `.step` / `.stp` input:
  - parses via `import_step_text_to_document`
  - writes `CadDocument` JSON
- `.cad0` / `.vcadc` input:
  - parses via compact IR parser
  - writes `CadMcpDocument` JSON
- Success message format:
  - `Imported STL to <path>`
  - `Imported STEP to <path>`
  - `Imported compact IR to <path>`

### `info`

Usage:
- `openagents-cad-cli info <input_json|input.step|input.stp>`

Behavior:
- If JSON parses as `CadMeshPayload`: prints mesh summary.
- Else if JSON parses as `CadDocument`: prints document summary.
- Else if payload parses as compact IR text: prints compact IR summary.
- Else if STEP text: prints STEP checker/entity summary.

### Scaffold Compatibility

- Bare command invocations (no args), e.g. `openagents-cad-cli export`, remain scaffold stubs from `VCAD-PARITY-083`:
  - exit code `3`
  - message: `export command scaffold is present; implementation lands in VCAD-PARITY-084`

## Error Codes

- `0` success
- `1` runtime IO/parse/export/import failure
- `2` usage/argument/format errors
- `3` scaffold-only bare command invocation
