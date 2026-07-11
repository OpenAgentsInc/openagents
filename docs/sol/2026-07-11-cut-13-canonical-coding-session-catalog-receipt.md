# CUT-13 canonical coding-session catalog receipt

- Date: 2026-07-11
- Issue: [#8693](https://github.com/OpenAgentsInc/openagents/issues/8693)
- Status: complete and closed at `0c49648217`; shared contract, bounded restart
  resolver, owner-scoped server projection, confirmed client reads, Desktop
  navigation/persistence, and built-host reload restoration are active
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

## Owner-scoped server projection

`@openagentsinc/khala-sync-server` now validates and appends one bounded catalog
change set through the normal transaction writer. All project, repository,
worktree, session, and navigation post-images must name the exact same user or
team scope. Cross-owner and broken-relationship bundles fail before storage;
raw placement, credential, provider-session, and transport-shaped material is
refused with a public-safe diagnostic.

The writer appends entities sequentially inside one transaction so its first
dense-version allocation cannot race. A real local-Postgres receipt proves all
five entity classes commit at one owner-scope version and the next whole bundle
advances that scope exactly once.

## Confirmed client read model

`@openagentsinc/khala-sync-client` now reads all five entity classes from one
exact authorized user/team scope. It exposes nothing while the scope is
catching up, denied, unavailable, or requires refetch. Once live, it validates
every post-image, requires entity-id and owner-scope agreement, ignores
malformed/pre-contract rows, applies explicit aggregate bounds, selects the
newest navigation post-image, and runs the shared relationship/recovery
resolver. Cached SQLite rows never become authority on their own.

## Desktop host persistence and typed navigation

OpenAgents Desktop now owns a device-local catalog through its existing Khala
Sync SQLite `local_entities` store. Canonical post-images contain stable random
refs and never contain a path. A main-process-only, mode-`0600` binding file
maps the opaque worktree ref to the local checkout path needed for runtime
placement. Device-local authority is explicit in the shared entity contract;
the hosted server projector and confirmed client read model remain restricted
to authenticated user/team scopes.

The fixed IPC contract exposes schema-decoded snapshot, choose, open, archive,
recover, and focus actions. Project Home renders typed active, recovery, and
archived projections; structured `project:`, `repository:`, and `state:`
queries avoid ad hoc intent matching. Opening a session restores its typed
conversation/editor/terminal/agent focus. Duplicate opens retain one stable
session, missing paths become named recovery, and archive/recover operations
retain product identity.

The catalog service test closes the real SQLite connection and private binding
store, creates a new service instance, and proves the same refs, recent order,
focus, archive, duplicate suppression, and missing-worktree recovery. The
built Electron smoke then reloads the renderer and proves Project Home restores
the exact same session key and `This Mac` authority. The existing generic
`local_entities` schema already stores full typed post-images, so CUT-13 needed
no new database migration; its existing migration suite plus the real reopen
and Postgres receipts cover both storage paths.

## Verification

- Focused schema/resolver suite: 9 pass, 0 fail, 94 expectations.
- Bounded authority model: all 64 combinations of project/session archive,
  repository/worktree loss, and session/worktree revocation agree with the
  fail-closed eligibility law.
- Full `@openagentsinc/khala-sync`: 188 pass, 0 fail, 2,697 expectations.
- Package typecheck: pass.
- Server projection focus including real Postgres: 5 pass, 0 fail,
  19 expectations; server typecheck passes.
- Broad server suite reaches 515 pass / 1 unrelated failure. The failing
  `runtime-intents.test.ts` expiry fixture rejects three runtime events and
  reproduces in isolation; no CUT-13 projector test or typecheck fails.
- Confirmed-client focus: 5 pass, 0 fail, 9 expectations; client typecheck
  passes.
- Full `@openagentsinc/khala-sync-client`: 183 pass, 3 opt-in live-smoke skips,
  0 fail, 12,750 expectations.
- Desktop catalog focus: 5 pass, 0 fail, 25 expectations, including a real
  SQLite close/reopen.
- Desktop boundary/catalog/UI focus after concurrent contract integration: 135
  pass, 0 fail, 996 expectations; Desktop typecheck passes.
- Full OpenAgents Desktop: 382 pass, 0 fail, 2,023 expectations; production
  build passes.
- Built Electron smoke: `coding-catalog-host-persistence` and
  `coding-catalog-reload-restoration` pass with the exact same opaque session
  key before and after reload; lifecycle teardown reports zero active handles.

## Close statement

CUT-13 is complete at `0c49648217`. It establishes local device authority and
authenticated hosted projection without claiming remote host movement. Mobile
binding to authenticated repository/session/thread refs remains CUT-14.
