# Desktop Lexical composer completion receipt

- Class: receipt
- Date: 2026-07-16
- Status: implementation and local verification complete
- Dispatch: no; use [#8851](https://github.com/OpenAgentsInc/openagents/issues/8851)
- Base: `96fa75b907`
- Owner: OpenAgents Desktop

## Result

The Desktop chat composer now uses Lexical as its production plain-text
editing engine without transferring application authority out of Effect
Native. The typed shell state remains the value authority and every Send,
Steer, Queue, Stop, attachment, and mode action still crosses the existing
typed intent boundary exactly once.

The editor provides native contenteditable selection and history, Enter to
submit, Shift+Enter for a newline, IME-safe composition, explicit new-session
focus, image-only submission, bounded attachments, drag/drop, the two-row
composer shell, accessibility metadata, and a registered-node seam for future
file, skill, and terminal context nodes. Persisted prompt content remains plain
text.

## Completion correction

The initial implementation was already present on `main` through
`7cc531d1d7` and `eaa1a3b959`, although the issue remained open with an older
orphaned-commit status. The completion audit found one acceptance gap: a
genuinely different external value rebuilt the editor and selected its end.

External synchronization now snapshots both range endpoints as bounded
plain-text offsets, rebuilds the controlled value, restores the range with
clamping, and applies Lexical's history-merge tag. This prevents a server or
shell hydration from forcing the caret to the end or adding a synthetic undo
checkpoint. Direct regression coverage exercises keyboard undo, keyboard
redo, editor identity, external no-echo behavior, and caret preservation.

## Theme and authority boundary

Khala is the sole mounted visual theme and retains its blue surface hierarchy.
This completion changes no colors, frames, spacing, renderer authority, or
application state ownership. Autopilot is not mounted as a competing theme;
any future condensed or mono donor ideas must continue to resolve through
Khala semantic roles.

## Verification

- Focused composer, focus, and primitive-adapter suites: 3 files and 35 tests
  passed.
- Complete Desktop test inventory: all 167 files and 1,587 tests passed, with
  39 skipped. The command reported five post-suite React scheduler errors from
  `react-review-sheet.test.tsx` after that fixture removed its temporary
  `window`; the suite passed independently (1 file, 1 test). An earlier full
  run's only failure was the known 20 ms missing-Codex transition timing check;
  its isolated suite passed all 27 tests.
- Desktop TypeScript check and production build: passed.
- Packaged React Electron smoke passed its React-exclusive workbench,
  composer-focused new-session, image-only attachment, first-keystroke, turn,
  decision, and lifecycle checks. The wider command later failed at the
  unrelated navigation-history fixture (`forwardChanged: true`); no complete
  navigation-smoke pass is claimed.
- Sol documentation policy and exact generated-manifest suite: 19 tests
  passed.

No deployment is part of this issue.
