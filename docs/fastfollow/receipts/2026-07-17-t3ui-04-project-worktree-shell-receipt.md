# T3UI-04 project/worktree shell receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@b6852266992055da79f7f00386e3a891ab449105`
- Scope: device-local project/worktree grouping, status, ordering, selection,
  lifecycle actions, and conversation-header worktree controls

## Implemented

- The mounted session rail now projects the existing device-local coding
  catalog as project groups with exact repository/worktree session rows.
  Active, idle, recovery-required, and archived states remain distinct and a
  project inherits the highest-priority state of its visible worktrees.
- Open, recovery, and archived filters dispatch the existing typed catalog
  filter intent. Recent, name, and local manual ordering never mutate the
  authoritative catalog; manual up/down controls reorder only the current
  presentation.
- Worktree checkboxes support bounded multi-selection. Batch archive or recover
  emits one existing exact-session intent per selected ref. Per-row open,
  archive, recover, delete request, and delete confirmation preserve the host's
  current collision and confirmation boundaries.
- Choose project/worktree and load-more controls use the catalog's admitted
  chooser and paging calls. No renderer path picker or worktree creation logic
  was introduced.
- When an admitted coding session is selected, the conversation header shows
  its repository/worktree identity, observed branch when available, and typed
  Files, Review, and Change-project controls. Narrow headers collapse labels to
  icons while retaining accessible button names.

## Proof

- Project/worktree shell behavior is included in the mounted shell-adapter
  suite, covering grouping, status filters, selection, batch recovery, manual
  order controls, exact open and paging intents, header identity, and Files /
  Review / Change intent dispatch: 29 focused tests passed.
- Desktop TypeScript passed. The full serial Desktop suite passed 207 files and
  2,015 tests with 39 skipped; the production build, classic and React Electron
  fixture smokes passed.
- The 16-state visual lane was deliberately rebaselined for the new Projects
  section and then passed at zero pixel drift. The Sol manifest, policy/link
  guard, and 19-test Sol suite passed.

## Boundaries

This packet adds no Git mutation, path, shell, credential, remote placement, or
worktree-creation authority. Project choice and worktree lifecycle remain owned
by the existing main-process coding catalog. Manual ordering is deliberately
renderer presentation state until a typed durable preference contract exists.
The tab/surface manager, rich files/diff, PTY, preview, settings, remote/mobile,
responsive closure, component census, installed signed-build evidence, and T3
parity remain later packets.
