# NIP Sources

This directory holds our copies of the Nostr protocol specifications
(NIPs). These copies are the source of truth for the Rust implementation
in this repository.

## The two sources

| Lane | Upstream | Content |
| --- | --- | --- |
| `nips/official/` | [nostr-protocol/nips](https://github.com/nostr-protocol/nips) | The standard NIPs |
| `nips/block/` | [block/buzz](https://github.com/block/buzz/tree/main/docs/nips) | The Buzz extension NIPs |
| `nips/coder/` | this repository | Coder product specifications, authored here |

`nips/coder/` is not synced: its files are the source of truth and the
manifest does not track them.

`nips/manifest.json` records the exact upstream commit for each synced lane, with
a `tree_url` link to browse that commit. Use those links to see the
upstream history for any file.

A lane may carry a repo-owned `README.md` that summarizes its specs when
the upstream does not publish one (currently `nips/block/README.md`).
The sync script preserves that file, and an upstream-provided README
always replaces the local copy. The manifest file count records upstream
files only.

## Implementation mandate

The specifications pinned in these lanes are the relay's implementation
target. The lane is not a reading list. We implement each applicable spec
across the `crates/nostr` domain and `crates/nostr-relay` server surfaces.
If an upstream NIP is client-only, completion means a fixture-backed
client implementation rather than pretending the relay serves it. If a
feature is optional or configuration-dependent, it stays fail-closed and
absent from NIP-11 until its configured path is executable.

Pinned deprecated or unrecommended NIPs are still implemented for exact
compatibility and regression coverage. They do not become the foundation
for new protocol design.

## How we sync

1. `./scripts/sync-nips.sh` clones each pinned upstream, copies the
   specification files into the lane directory, and writes the upstream
   commit hashes to `nips/manifest.json`.
2. Review the diff. A specification change is a protocol policy change.
   Do not commit a sync without reading what changed.
3. Commit the sync as its own commit.

Run the sync at a regular interval and before each new NIP
implementation starts.

## How we verify

1. We implement each NIP from the specification text in this directory,
   from scratch, in Rust.
2. Each implemented NIP gets a fixture corpus in this repository. A
   protocol change without a fixture update is not complete.
3. Where the state space is small and the property matters, we add formal
   verification and keep the model next to the fixtures.
4. A synced upstream change becomes normative for the implementation only
   after review and a fixture update. The sync itself never changes the
   implementation.
5. Keep an explicit ledger for every pinned specification. No file is silently
   ignored because its role is client-side, optional, deprecated, or not yet
   represented by a server handler.

## Precedence

The lanes stay separate. The implementation tracks each NIP by lane and
identifier. If the same identifier exists in more than one lane, the
`official` lane wins unless the build plan names an exact exception.
