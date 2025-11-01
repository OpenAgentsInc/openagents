# Maestro Artifacts (Screenshots and Logs)

When a flow fails, Maestro writes artifacts to a timestamped folder under your home directory:

- macOS: `~/.maestro/tests/<YYYY-MM-DD_HHMMSS>`
- Example from local runs: `/Users/<you>/.maestro/tests/2025-11-01_031143`

Each run contains:
- `report.json` — structured run output per step.
- `screenshot.png` — screenshot captured on failure.
- Per-step logs — the CLI summary inlined in the console plus details in the folder.

Tips:
- Open the latest timestamped directory after a failure to quickly see what was on screen.
- If a flow passes locally but fails in automation, compare artifact folders to spot UI timing differences.
- Keep Metro (Expo dev server) running and warmed before invocation to minimize route-render timing differences.
