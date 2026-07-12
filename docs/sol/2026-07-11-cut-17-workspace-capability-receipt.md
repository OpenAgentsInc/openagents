# CUT-17 workspace capability foundation receipt

- Date: 2026-07-11
- Issue: [#8697](https://github.com/OpenAgentsInc/openagents/issues/8697)
- Status: capability core, tree/watch/search host bridge, and cancellable search
  worker landed; UI, mutations, and closure evidence remain open
- Implementations: `4bbf0c7758`, `37372f30e2`, `efe7738ff1`, `36725a91df`

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

## Verification

- focused search/worker/workspace/electron suite: 41 pass, 0 fail, 509 assertions;
- full integrated Desktop suite after rebasing the concurrent Git/GitHub
  surface: 642 pass, 0 fail, 3,515 assertions, with 11 existing
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
  duplicate close, and terminal disposal; and
- every new tree/search projection is asserted not to contain the selected
  absolute root.

No tree/search UI or scale-benchmark receipt is claimed.

## Remaining before CUT-17 closure

- migrate the Files workspace to the new relative-ref boundary and ship the
  recursive accessible Effect Native tree/search experience;
- add reveal plus create/rename/delete capability operations with explicit
  conflict and permission-loss outcomes;
- run the required large-repository scale benchmark and built-host lifecycle
  receipt; and
- remove the legacy absolute-root renderer projection only after its consumers
  have migrated.
