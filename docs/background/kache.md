# The kache compile cache

kache is the `rustc` wrapper for this workspace. It keeps compiled crates in
a content-addressed store, `~/Library/Caches/kache` on macOS, and restores
them into any target directory whose build matches. Its size cap is
`local_max_size` in `~/.config/kache/config.toml`. This page covers how that
cap is enforced on an agent machine and how to reclaim the store
(issue #10758).

## Configuration

```toml
[cache]
local_max_size = "30GiB"
gc_evict_shared = true
```

`gc_evict_shared = true` is required on a machine with long-lived agent
target directories. Without it, the cap isn't enforced:

- The cap counts every blob the store names (`store_bytes` in
  `kache stats --json`), including blobs that target directories also hold as
  APFS clones or hard links.
- By default, the collector keeps every entry that holds the last store
  reference to such a blob, because deleting it frees no disk. It keeps the
  whole entry, so the entry's private blobs stay too.
- Agent target directories under `~/work/openagents-target-agent*` live for
  days and clone most of the store. On 2026-10-06 every one of 18,800 entries
  was kept this way: the store held 287 GB against the 30 GiB cap, 92 GB of
  it private, and each collection finished in under a minute having freed
  almost nothing.

With the setting on, the collector evicts those entries as well. The store,
and so its private bytes, falls under the cap. A target directory keeps its
own clone of an evicted blob, so nothing it holds changes; a new target
directory misses the evicted entries and rebuilds them. The daemon rereads the
configuration within 15 seconds of an edit. To restore kache's default,
delete the line.

## The collection lock

`kache gc` reports "Another GC is already running" when another collector
holds `gc.lock` in the store directory. The lock is an advisory lock on an
open file, so the kernel releases it when its holder exits; a dead holder
can't block a collection, and there is never a stale lock to clear. The pid
written into the file names only the last holder. The message means a live
collector, usually the daemon's own sweep, is running. Don't delete the lock
file: kache warns that unlinking it can let two collectors run at once.

kache's automatic sweeps also back off for up to two hours after a sweep that
leaves the store over its cap (`auto-gc-backoff.json`). `kache gc` ignores
that backoff.

## Reclaiming the store

```sh
openagents background kache            # run kache's collector, report what it freed
openagents background kache --status   # sizes and who holds the lock, changing nothing
```

The command reads `kache stats --json`, waits while a live process holds
`gc.lock` (up to 10 tries, 30 seconds apart), then runs `kache gc --json` and
reports the store's size before and after and the bytes returned. It never
deletes anything under the store itself. Callers in Rust use
`background::kache::Kache::reclaim`, which returns the same report; the disk
cleanup monitor (#10759) schedules it.

What the store can't free, target directories hold. `kache targets` lists
each one with what deleting it frees, and the disk cleanup monitor removes
stale ones.
