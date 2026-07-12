# CUT-17 workspace capability foundation receipt

- Date: 2026-07-11
- Issue: [#8697](https://github.com/OpenAgentsInc/openagents/issues/8697)
- Status: capability core landed; gateway, UI, mutations, and closure evidence
  remain open
- Implementation: `4bbf0c7758`

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

The current legacy root summary/read/save/Git renderer path is intentionally
unchanged in this foundation commit. Therefore this receipt does not claim that
the new root-private projections cross preload or render yet.

## Verification

- focused adversarial workspace suite: 11 pass, 0 fail, 67 assertions;
- full Desktop suite: 569 pass, 0 fail, 3,186 assertions, with 11 existing
  capability-gap skips retained and named;
- Desktop typecheck: pass;
- Desktop production bundle: pass;
- fixtures cover lexical traversal, file and directory symlink escape,
  `.gitignore`, hidden/secret filenames, secret-shaped content, binary data,
  large-root pagination, cache identity/invalidation, watcher overflow,
  duplicate close, and terminal disposal; and
- every new tree/search projection is asserted not to contain the selected
  absolute root.

No Electron launch was necessary for this main-process-only rung, and no UI or
scale-benchmark receipt is claimed.

## Remaining before CUT-17 closure

- add fixed schema-decoded gateway/preload operations and a bounded event
  subscription for tree, search, refresh, and invalidation;
- migrate the Files workspace to the new relative-ref boundary and ship the
  recursive accessible Effect Native tree/search experience;
- add reveal plus create/rename/delete capability operations with explicit
  conflict and permission-loss outcomes;
- move long-running content search behind a cancellable owned task so project
  close proves every watcher and search task settles exactly once;
- run the required large-repository scale benchmark and built-host lifecycle
  receipt; and
- remove the legacy absolute-root renderer projection only after its consumers
  have migrated.
