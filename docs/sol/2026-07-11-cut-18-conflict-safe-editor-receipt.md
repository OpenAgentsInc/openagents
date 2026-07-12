# CUT-18 conflict-safe editor closure receipt

- Date: 2026-07-11
- Issue: [#8698](https://github.com/OpenAgentsInc/openagents/issues/8698)
- Status: complete on `main`
- Closing commit: `091574d5bc`
- Parents: [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566), [#8574](https://github.com/OpenAgentsInc/openagents/issues/8574)

## Outcome

OpenAgents Desktop now has a practical Effect Native-owned document lifecycle
behind a replaceable `CodeEditor` foreign host. The editor widget receives only
bounded serializable props and emits typed runtime-bound events; it owns no
workspace, project, session, IPC, credential, or arbitrary filesystem
authority.

The landed stack provides:

- bounded tabs, language modes, authoritative selections, find, undo/redo,
  dirty state, save state, inline close confirmation, and familiar Save As;
- grant-scoped relative-ref open/save/save-as operations decoded in main and
  preload, with bounded UTF-8 documents and content revision receipts;
- atomic stale-revision refusal and create-only Save As that never overwrites,
  including an `EEXIST` race;
- explicit missing, directory, binary, too-large, encoding, permission,
  revoked-grant, external-change, deletion, and conflict states;
- confirmed file and folder rename reconciliation that retargets matching open
  tabs and descendants without dropping local drafts; and
- renderer-reload recovery keyed by an opaque coding-session ref. The recovery
  snapshot contains only bounded relative path refs, expected revisions, and
  drafts—never a workspace root or grant—and reopens through the current grant
  before classifying unchanged, changed, or missing files.

This is host-local crash/reload recovery. It is not Khala Sync authority,
portable workspace materialization, or a claim that provider process memory can
move across hosts.

## Commit stack

- `774c74f836` — grant-scoped document core
- `37bd3f2c0b` — decoded document bridge
- `65ffaac4e1` — conflict-safe editor lifecycle
- `bf3b609cf6` — replaceable code-editor adapter hardening
- `4478204c18` — selected-range projection
- `2485d9adde` — Files/editor shell composition
- `091574d5bc` — create-only Save As, typed host runtime binding, rename
  reconciliation, and exact-session reload recovery

## Verification

Post-rebase validation on the closing commit:

- `bun run --cwd apps/openagents-desktop typecheck` — pass
- editor, shell, Electron-boundary, and workspace-registry suites — 123 pass,
  0 fail, 996 expectations
- full built Electron smoke — pass, including real textarea input, inline Save
  As, current-session recovery storage, renderer reload, dirty-draft restore,
  existing EP250/#8712 journeys, real Git status, and lifecycle teardown with
  `active: 0`

The broader Bun workspace-service file reports 20 passing document/tree/search/
mutation tests and two legacy Git status/diff fixture failures because Bun
1.3.11 does not return those child-process stdout bytes in that test context.
The production Electron/Node path remains covered by the green built-host
`git-review-panel-real-status` journey. No assertion was weakened or skipped to
hide that harness limitation.

## Boundary evidence

- The renderer never receives an absolute root, ambient path, credential,
  process handle, or generic IPC method.
- Recovery persists no WorkContext grant and must reconcile through the current
  main-owned grant after reload.
- Save As is exclusive-create only; conflict is a visible result, never an
  overwrite fallback.
- The editor adapter remains substitutable and its mount/update/unmount
  lifecycle is renderer Scope-owned with exact disposal coverage.
- Window/application teardown closes workspace subscriptions and workers with
  zero active owners in the built smoke.

## Remaining ownership

CUT-19 owns typed Git review, review actions, and composer diff/context capture.
CUT-20 owns workspace-bounded PTY and preview lifecycle. Language-server and
full IDE breadth remain outside CUT-18.
