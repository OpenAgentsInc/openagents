# T3UI-06 Files and rich Diff receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@2009c27e09`
- Scope: grant-scoped Files workbench and exact read-only rich Diff

## Implemented

- The Files surface now renders the host-projected relative file tree with
  directory expansion, exact selection, bounded 500-row projection, root
  refresh, name/content search modes, search results, loading, empty, and typed
  unavailable states.
- Selecting a file opens it through `WorkspaceEditorOpenRequested` with the
  current grant ref. Mounted document tabs support activate, close and dirty
  state; the editor supports typed change events, Command/Ctrl-S, save, undo,
  redo, find navigation, close confirmation, external-change conflict, reload
  theirs, and explicit save mine.
- Review now renders exact host status and changed-file groups without Git
  mutation controls. Staged and unstaged entries request a fenced diff through
  `GitPanelDiffRequested`; untracked files remain visibly non-reviewable.
- Exact hunks render in a unified scrollable diff with add/remove/context
  treatment, line-level local review notes, close, and typed attachment to the
  composer. Local notes are presentation state and grant no comment-publishing
  authority.
- Transcript-first composition remains intact: both rich workbenches live in
  the T3UI-05 panel while the conversation stays mounted.

## Proof

- Mounted tests cover grant-relative tree composition, directory toggle,
  document tab/editor rendering, changed-file projection, exact rich hunk
  rendering, typed diff attachment, and line annotation entry. The combined
  focused shell/fixture set passes 36 tests.
- The visual lane now captures both rich Files and rich Diff: `surface-tabs`
  contains an active document and `files-rich-diff` contains changed files and
  an exact unified hunk. All 18 visual states pass after deliberate admission.
- Desktop TypeScript passed. The full serial suite passed 209 files and 2,025
  tests with 39 skipped; production build and both Electron fixture journeys
  passed. Sol guards and publishing are recorded by the landed commit.

## Boundaries

All paths remain root-relative refs under the existing WorkContext grant. The
renderer receives no absolute root and performs no filesystem or Git operation
directly. Diff is read-only; staging, discard, commit, push, branch mutation,
and review publication are absent. Local annotations do not leave the renderer.
Terminal, preview, settings convergence, remote/mobile, closure, component
census, installed signed evidence, and T3 parity remain later packets.
