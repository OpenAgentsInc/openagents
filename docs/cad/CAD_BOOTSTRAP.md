# CAD Demo Bootstrap Runbook

## Objective

Provide a deterministic one-click reset path for the CAD demo pane so recordings and scripted checks can always return to a known baseline.

## Behavior

- UI action button: `Bootstrap Demo`
- Command palette action: `cad.demo.bootstrap`
- Reducer actions:
  - `CadDemoPaneAction::BootstrapDemo` (primary)
  - `CadDemoPaneAction::ResetSession` (legacy alias, equivalent behavior)
- Both actions:
  - rebuild `CadDemoPaneState` from deterministic defaults
  - queue a fresh rebuild cycle with trigger `bootstrap-demo`
  - set status text to `CAD demo bootstrapped to deterministic baseline`
  - emit `CadEventKind::DocumentCreated`

## Determinism Contract

- Repeating bootstrap any number of times yields the same baseline state signature:
  - document/session IDs
  - active variant (`variant.baseline`)
  - camera/snap/projection defaults
  - warning filters reset to `all`
  - queued rebuild request id reset to `1` for the new bootstrap state instance
- `ResetSession` remains supported as a compatibility alias and must match `BootstrapDemo` output.

## Verification

Run:

```bash
cargo test -p autopilot-desktop bootstrap_demo_action_is_idempotent_and_reset_alias_compatible --quiet
cargo test -p autopilot-desktop cad_palette_command_specs_are_unique_and_resolve_actions --quiet
cargo check -p autopilot-desktop --quiet
```

Expected results:

- idempotence test passes on repeated bootstrap and reset alias.
- command palette test resolves `cad.demo.bootstrap` to `CadDemoPaneAction::BootstrapDemo`.
- crate compiles without CAD action match regressions.
