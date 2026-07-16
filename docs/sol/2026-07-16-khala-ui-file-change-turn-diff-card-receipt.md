# Khala UI file-change and turn-diff card receipt

Issue: [#8862](https://github.com/OpenAgentsInc/openagents/issues/8862)

## Result

Codex file-change items now retain their bounded typed `changes[]` payload
through the Desktop event, durable-note, history, and renderer boundaries.
Each file renders its path, `[ADD]`/`[DEL]`/`[MOD]` tag, addition/deletion
tally, and an independently expandable unified diff. The patch lifecycle is
visible in the header and truncated diffs identify the display bound.

Live `item/fileChange/patchUpdated` notifications reconcile by provider
`itemId`. The latest `turn/diff/updated` aggregate parses into one stable
turn-scoped card and closes with the turn. Retained raw and JSON-wrapped
`apply_patch` rollout records parse into the same typed component instead of a
generic tool row.

`/components/workbench` now mounts running turn-diff, completed patch, failed
patch, and capped-diff variants alongside the command stories.

## Theme boundary

Khala is the sole mounted theme and owns the blue background/surface hierarchy.
Autopilot contributes only the compact mono tally/readout grammar. File status
and diff lines resolve through Khala accent, success, danger, warning, border,
and surface roles; this change adds no raw donor-palette literals or competing
theme mount.

## Bounds and invariants

- At most 64 file changes cross the typed boundary.
- Each path is bounded to 1,024 characters and each diff to 20,000 characters.
- Per-file truncation is explicit via `diffCapReached`.
- Live patch identity is provider `itemId`; aggregate identity is provider
  turn id. Repeated updates replace a row rather than append duplicate cards.
- Current camelCase file changes and retained `apply_patch` history share one
  schema and shared component.

## Verification

- Focused Desktop/web contract, event, history, renderer, and story suites:
  55 passed.
- Desktop full suite: 167 files; 1,587 passed, 39 skipped.
- Desktop, shared UI, and Start TypeScript checks: passed.
- Desktop production build: passed.
- Start full suite: 50 files; 217 passed.
- Start production build and Cloud Run bundle: passed.
- Sol documentation policy/manifest checks: passed.

In-app visual inspection was attempted against the local Start preview, but
the browser-control runtime failed during bootstrap with `Cannot redefine
property: process`. No screenshot or interactive-browser acceptance is
claimed. Server-rendered story assertions prove all four real shared card
variants are mounted; visual owner acceptance remains separate.
