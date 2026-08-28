# test-report

Pure-computation guest plugin: captured test-runner output in, typed
failures out, no imports. Built on `openagents-pdk`. Harvest target 5
from `docs/2026-08-25-plugin-harvest-targets.md` in openagents.com
(OpenAgentsInc/openagents#120).

Recognises pytest, cargo test, jest/vitest, and go test. Auto-detects
when `runner` is omitted. Output names file, test, assertion, and a
bounded traceback, plus passed/failed counts when the runner printed a
summary. Truncation of the input or the failure list is reported, never
hidden.

The parser is unit-tested in `src/tests.rs`. Rebuild from `plugins/`
and update `artifact.digest` in `manifest.json` — see `../README.md`.
