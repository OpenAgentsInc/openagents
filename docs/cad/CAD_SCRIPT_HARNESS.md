# CAD Headless Script Harness

This harness runs deterministic `CadIntent` and pane-action sequences against
`CadDemoPaneState` without opening a UI window.

## Location

- Harness implementation:
  - `apps/autopilot-desktop/src/input/reducers/cad.rs` (test module)
- Script fixtures:
  - `apps/autopilot-desktop/tests/scripts/cad_demo_canonical_script.json`
  - `apps/autopilot-desktop/tests/scripts/cad_demo_failure_paths_script.json`

## Script Format

Top-level fields:

- `script_id` (string): stable identifier for reporting.
- `seed` (u64): deterministic randomness seed.
- `thread_id` (string): chat-thread binding used for intent dispatch.
- `timing.max_total_duration_ms` (optional u64): total runtime budget.
- `steps` (array): ordered operations.
- `expect` (object): explicit final assertions.

Supported step kinds:

1. `intent_json`
   - Input: `payload` (strict CadIntent JSON string)
   - Outcome: `applied`, `rejected_parse`, or `rejected_dispatch`
   - Supports explicit `expect` assertions per step.

2. `cycle_variant`
   - Runs `CadDemoPaneAction::CycleVariant`
   - Supports fixed `count` or deterministic `randomized { min, max }`
   - Waits for rebuild receipts and exposes receipt hashes for assertions.

3. `inject_rebuild_failure`
   - Injects a `CadRebuildResponse::Failed` path headlessly.
   - Used to validate boolean-failure style error handling.

4. `assert_warning_escalation`
   - Asserts warning and critical-warning minimum counts.

Per-step optional timing:

- `max_duration_ms` (u64)

## Determinism Requirements Encoded

- Script fixtures declare explicit expected rebuild/mesh hashes in `expect.receipts`.
- Final `expect` section validates:
  - state revision
  - active variant
  - receipt hashes
  - warning codes/severity counts
  - analysis metadata

## Run

```bash
cargo test -p autopilot-desktop cad_headless_script_harness_ --quiet
```

Release-gate reliability path (reuses canonical script fixture):

```bash
cargo test -p autopilot-desktop cad_release_gate_reliability_reuses_canonical_script_fixture --quiet
```

Strict lint/check lane integration:

```bash
scripts/cad/headless-script-ci.sh
```
