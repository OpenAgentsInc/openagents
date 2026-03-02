# Assembly UI Selection + Editing Parity (`VCAD-PARITY-063`)

`VCAD-PARITY-063` implements assembly pane selection/editing parity for deterministic UI workflows.

## Scope

- Added UI state model in `openagents-cad`:
- `CadAssemblyUiState` (`selected_instance_id`, `selected_joint_id`, `last_error`)
- Added UI-driven mutation APIs:
- `select_instance`, `select_joint`
- `rename_selected_instance`
- `set_selected_joint_state`
- `sync_with_schema`
- Integrated pane-level assembly selection/editing state in Autopilot desktop:
- `CadDemoPaneState::assembly_schema`
- `CadDemoPaneState::assembly_ui_state`
- Added deterministic inspector rendering for selected assembly instance/joint details.

## vcad References

- `~/code/vcad/packages/core/src/stores/document-store.ts`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_ui_selection_edit_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_ui_selection_edit_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-ui-selection-edit -- --check`
- CI script: `scripts/cad/parity-assembly-ui-selection-edit-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_ui_selection_edit_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-ui-selection-edit`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_ui_selection_edit_vcad_reference.json`
