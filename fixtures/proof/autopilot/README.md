# Autopilot Proof Projection Fixtures

These reduced fixtures describe the JSON contract consumed by
`apps/autopilot`. They are not production proof artifacts. They are stable,
redacted examples for UI and projection tests.

- `pylon-status-projection.json` is the Tauri-side Pylon status projection.
- `proof-run-projection.json` is the Tauri-side proof-flow projection assembled
  from `run-report.json`, `authority-state-trace.json`, `proof-summary.json`,
  and `object-trace.jsonl`.

Keep these fixtures small. Full proof evidence belongs under the runtime
namespace in `~/.openagents/pylon/proof/namespaces/<namespace>/`.
