# CUT-17 workspace capability foundation receipt

- Date: 2026-07-11
- Issue: [#8697](https://github.com/OpenAgentsInc/openagents/issues/8697)
- Status: capability core, tree/watch/search/mutation host bridge, cancellable
  search worker, mutation core, scale/lifecycle receipt, and standalone bounded
  Effect Native browser projection plus typed bridge handler loop landed;
  shell/boot composition and legacy projection migration remain open
- Implementations: `4bbf0c7758`, `37372f30e2`, `efe7738ff1`, `36725a91df`,
  `57488904c5`, `9f957a6d76`, `60369f3009`, `96692a6672`, `e6b2469e2e`

## Landed boundary

The existing Desktop workspace service still begins only after an explicit
native directory-picker choice and remains owned by the replaceable
main-process WorkContext. CUT-17 adds a separate typed capability surface over
that selected root:

- one opaque `workspace.grant.*` ref;
- root-relative directory/file refs with traversal and symlink containment;
- lazy directory pages capped at 200 entries;
- path and text search capped at 100 matches, 20,000 visited entries, 1 MiB per
  searched file, and 240 characters per preview;
- hidden names, `node_modules`, Git-ignored paths, secret-shaped filenames,
  binary files, unreadable entries, and secret-shaped text withheld;
- SHA-derived entry revisions and cache keys that contain neither the selected
  root nor the raw search query;
- cache facts naming key, epoch, and current freshness;
- one recursive watcher opened only while at least one subscriber exists;
- change, explicit refresh, and unlocated/overflow events that advance the
  epoch and clear both tree and search caches; and
- idempotent subscriber close and WorkContext disposal, with the underlying
  watcher closed exactly once.

The fixed host bridge now schema-decodes tree requests/responses plus refresh,
watch subscribe/unsubscribe, and change events. Main refuses non-top-level or
non-bundled senders, keeps one workspace subscription per webContents, rebinds
active subscribers when explicit workspace selection replaces the WorkContext,
and closes with the registered window/app lifecycle. Preload multiplexes local
listeners over one reference-counted decoded event handler. No root, handle,
arbitrary channel, or filesystem API crosses. The current legacy root summary/
read/save/Git renderer path is intentionally unchanged until UI migration.

Bounded path/content search now executes in one isolated worker per request.
The WorkContext owns every task, terminates stale work before a watch or refresh
epoch advances, caches only a result from the unchanged epoch, and settles
cancel/error/exit/project-close races exactly once. The worker result crosses a
bounded schema decoder and contains only the opaque grant plus relative refs.
Fixed search/start-cancel operations now cross the same trusted main/preload
boundary. Main owns one active request per webContents; replacement cancels the
prior task, exact cancellation is fenced by both owner and request ref, and
window/app teardown closes the owner. No renderer search UI is claimed.

The root-private mutation core now supports create file/directory,
revision-bound rename, revision-bound non-recursive delete, and host-injected
reveal. It rejects traversal, symlinks, hidden/secret/Git-ignored names, stale
revisions, existing targets, non-empty directory deletion, and permission loss
with typed outcomes. Only confirmed mutations advance the WorkContext epoch.
Fixed decoded create/rename/delete/reveal operations now cross the trusted
main/preload boundary. Electron main injects reveal authority when it opens a
WorkContext; no absolute path returns. No mutation UI is claimed.

The shell-independent workspace-browser projection now expresses that boundary
through the shared Effect Native catalog. It includes a lazy virtualized tree,
bounded path/content search results, explicit idle/loading/unavailable/empty/
truncated states, and inline create/rename/two-step delete/reveal affordances.
The pure state transitions discard prior pages and search results when root
authority ends, reject child pages or results from a different grant, dedupe
paged entries, and cap the visible hierarchy at 500 rows with in-flow
disclosure.

The typed handler loop now decodes every host result before state, owns one
exact renderer search request at a time, cancels and fences late results, and
preserves/dedupes only matching paged results. Lazy tree pages, explicit manual
refresh, watcher-triggered reload without recursive refresh events, and
create/rename/delete/reveal calls use the same bridge boundary. Renderer guards
refuse unseen create parents and stale rename/delete revisions before dispatch;
confirmed mutations reload the root before projecting a receipt. Neither the
handler nor the view is composed into the shell yet, so this is still not
evidence that Desktop users can reach the new boundary.

## Verification

- focused search/worker/workspace/electron suite: 41 pass, 0 fail, 509 assertions;
- focused mutation/workspace/topology suite: 35 pass, 0 fail, 270 assertions;
- focused mutation/electron bridge suite: 37 pass, 0 fail, 522 assertions;
- full integrated Desktop suite: 644 pass, 0 fail, 3,563 assertions, with 11 existing
  capability-gap skips retained and named;
- Desktop typecheck: pass;
- Desktop production bundle: pass;
- real bundled-worker test: built `workspace-search-worker.js`, searched a
  temporary fixture at epoch 7, returned the relative `README.md` match, and
  exposed no selected root;
- built Electron smoke: relative tree page passed through preload, explicit
  refresh produced a decoded newer-epoch event, search returned the relative
  `session_index.jsonl` match at that epoch, a foreign cancel was refused,
  unsubscribe completed, every existing EP250 step remained green, and
  lifecycle teardown reported `active: 0`;
- fixtures cover lexical traversal, file and directory symlink escape,
  `.gitignore`, hidden/secret filenames, secret-shaped content, binary data,
  large-root pagination, cache identity/invalidation, watcher overflow,
  duplicate close, and terminal disposal;
- every new tree/search projection is asserted not to contain the selected
  absolute root; and
- named scale/lifecycle receipt: the real bundled worker searched a synthetic
  20,000-file repository to the declared visit cap in 1.41 seconds, returned a
  truncated relative-ref-only result, replayed the current-epoch cache in
  0.03 ms, then project close settled the pending worker, closed the watcher
  once, and reported zero active searches. The integrated suite with this test
  is 645 pass / 3,573 assertions plus the 11 existing named skips.
- focused standalone browser projection: 13 pass, 53 assertions, covering grant
  fencing, lazy depth-first pagination and deduplication, the 500-row disclosure
  bound, virtualized tree/search lists, relative-only labels, and inline
  mutation confirmation; and
- post-rebase integrated Desktop suite at `96692a6672`: 685 pass / 3,781
  assertions, 10 named capability-gap skips, typecheck, and production build.
- focused typed browser handler loop at `e6b2469e2e`: 19 pass / 80 assertions;
  post-rebase integrated Desktop suite: 707 pass / 3,864 assertions with seven
  named capability-gap skips, plus typecheck and production build.

No composed or user-reachable tree/search/mutation UI is claimed.

## Remaining before CUT-17 closure

- compose that browser into the Files workspace and remove the prior flat
  root/read/save projection from its renderer consumers;
- remove the legacy absolute-root renderer projection only after its consumers
  have migrated.
