# CUT-13 canonical coding-session catalog receipt

- Date: 2026-07-11
- Issue: [#8693](https://github.com/OpenAgentsInc/openagents/issues/8693)
- Status: shared contract and bounded restart resolver active; server projection,
  Desktop navigation/persistence, and built-host restart receipt remain pending
- Contract schema: `openagents.coding_catalog.v1`

## Durable product identities

`@openagentsinc/khala-sync` now defines separate full post-image shapes for:

- `coding_project` — owner scope, stable project ref, display name, opaque
  aliases, and active/archive state;
- `coding_repository` — stable repository/project/owner refs, former-name
  aliases, pinned base, availability, and explicit grant state;
- `coding_worktree` — stable worktree/repository/project/owner refs, opaque
  host alias refs, pinned base, availability, and explicit grant state;
- `coding_session` — stable product session, WorkContext, thread,
  conversation, optional run/fleet/attachment/checkpoint/topology refs,
  canonical event/activity cursors, provider/runtime facts, lifecycle, and
  authorization state; and
- `coding_navigation` — persisted project/repository/worktree/session
  selection, bounded open tabs, and typed conversation/editor/terminal/agent
  focus.

Session identity cannot contain a raw path, host name, process id, vendor
session id, credential, or transport handle. Placement remains an optional
attachment beneath the session. Unknown provider/runtime/grant facts stay
explicit instead of being filled from a tab, filesystem path, transcript, or
local process.

## Bounded restart resolution

The shared resolver validates the catalog before restoring a selection. It:

- verifies exact owner/project/repository/worktree relationships;
- canonicalizes former-name and opaque checkout aliases to stable refs;
- rejects alias collisions instead of choosing by array order;
- collapses duplicate open tabs while preserving typed focus;
- returns named recovery states for missing session/repository/worktree,
  archived state, owner mismatch, revoked grant, or grant truth that was not
  projected; and
- returns an honest empty state rather than inventing a current session.

The catalog query helper accepts structured project/repository/state/time
fields and applies a stable recent-first sort. It deliberately implements no
keyword matching. User-facing text retrieval must later use the workspace's
central semantic or structured query path.

## Verification

- Focused schema/resolver suite: 9 pass, 0 fail, 94 expectations.
- Bounded authority model: all 64 combinations of project/session archive,
  repository/worktree loss, and session/worktree revocation agree with the
  fail-closed eligibility law.
- Full `@openagentsinc/khala-sync`: 187 pass, 0 fail, 2,695 expectations.
- Package typecheck: pass.

## Residual

CUT-13 remains open. The next tranche must register owner-scoped server
projection/migration and confirmed client reads for these post-images. Desktop
then needs typed create/open/archive/recover navigation, host-owned persistence,
process-restart restoration, and a built Electron recovery receipt. This
shared-contract tranche does not claim runtime placement or remote movement.
