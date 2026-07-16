# Khala UI command execution card receipt

Issue: [#8861](https://github.com/OpenAgentsInc/openagents/issues/8861)

## Result

Codex `item/commandExecution/outputDelta` notifications now cross the Desktop
event boundary as bounded, typed progress facts keyed by provider `itemId`.
The live renderer and durable local thread store update the original command
note in place; terminal results close the same invocation. Concurrent Bash
commands reconcile by identity instead of tool name or FIFO proximity.

The shared `DesktopCommandCard` now renders the real typed payload in Desktop,
history, dispatch-table, splash, and component-library contexts: command,
running/completed/failed status, right-aligned exit code and duration, CWD,
source, bounded output tail, and an explicit notice when earlier output was
discarded. Current camelCase and retained snake_case rollout records project
through the same component.

`/components/workbench` contains real mounted streaming, completed, failed,
and capped-output variants. The tokens inventory identifies `khalaTheme` as
the sole mounted product theme; Autopilot remains a donor grammar normalized
through Khala roles.

## Bounds and invariants

- Output retains at most `WORKBENCH_OUTPUT_TAIL_LIMIT` (4,000 characters).
- A cap flag survives live progress and history projection.
- Progress-note identity is `turnRef + itemRef`; completion has a distinct
  deterministic result key and folds into the started row.
- Tool traces retain the FIFO fallback only for historical producers without
  an item identity.
- No new palette literal or competing theme mount was added.

## Verification

- Desktop focused event/projection/history/UI suites: 108 passed.
- Desktop full suite, isolated: 167 files; 1,566 passed, 39 skipped.
- Desktop, shared UI, and Start TypeScript checks: passed.
- Desktop production build: passed.
- Start full suite: 50 files; 220 passed.
- Start production build and Cloud Run bundle: passed.
- `/components/workbench` SSR contract: four real command variants.

One concurrent full-suite run exceeded the existing 500-row React benchmark's
wall-clock threshold while four builds/tests competed for the machine. The
isolated rerun passed the complete Desktop suite. The requested in-app visual
inspection could not start because the browser-control runtime failed during
bootstrap with `Cannot redefine property: process`; no screenshot is claimed.
