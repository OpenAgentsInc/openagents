# Saved harness history

`coder-history` reads saved Codex and Claude conversations from explicitly
selected desktop directories. It provides catalog pages and complete source
bytes in bounded transcript pages. It does not start, resume, interrupt, or
modify a harness. It has no model or network client.

The default `host` feature supports macOS and Linux. Set
`default-features = false` to use the serializable request, cursor, page, and
readable-record types on a client without filesystem access. See
[`src/lib.rs`](src/lib.rs) for the portable contract and
[`src/host.rs`](src/host.rs) for `Config` and `History`.

## Read a history

A desktop operator selects absolute `Config.codex` and `Config.claude` roots.
`History::open(config)` opens those roots. `catalog(CatalogRequest)` enumerates
saved sources, and `transcript(TranscriptRequest)` reads a source selected by
its opaque `source_id`. Reopening `History` preserves valid cursors for the
same roots and files.

The Codex adapter reads `session_index.jsonl`, `sessions/`, and
`archived_sessions/`. The latest complete index row supplies a title and its
reported update time. An indexed conversation without a matching source stays
in the catalog with `status: missing`. The Claude adapter reads JSONL sources
under `projects/`, including separately labeled `subagents` sources. It uses
first-record title metadata when present; it does not reconstruct Claude's
full title history or sort conversations by their last activity.

These adapters follow locally observed saved-file structures, not a guaranteed
provider API. Unknown records remain available. The reader does not inspect
credentials or unrelated configuration files. Catalog requests rescan the
selected source trees, so newly saved chats appear without restarting the host.

## Preserve the complete transcript

`RecordChunk.raw_base64` is the exact source byte sequence, including its newline
when present. Reassemble chunks in byte-offset order and validate contiguous
offsets, source identity, incarnation, record identity, and base64 decoding.
A record can span pages; `complete` becomes true only at its newline. Unknown
JSON, malformed JSON, invalid UTF-8, and oversized records retain their bytes.

`Readable` is a convenience projection with a bounded preview, role, native
record ID, and tool metadata when recognized. It is not the full transcript.
`readable_record_full` gives a client the full recognized text after local
reassembly, without the 1 KiB host preview limit. `readable_record` keeps the
preview behavior. Both parse at most 256 KiB and return `None` for larger records; show or save those raw chunks
without claiming they were omitted. A message can contain multiple tool blocks;
the single tool metadata fields describe the first recognized block, while raw
JSON retains all blocks.

Retain `TranscriptPage.next` at EOF and reuse it when polling. A partial final
line is returned with `pending_line: true`; an append completes the same record
ID without repeating its previous bytes. EOF describes the observed file cut,
not whether an agent has finished. Each page records its observed source length.

Catalog pagination uses stable opaque ordering and a digest of membership, not
mutable titles or modification times. Title changes do not invalidate a page
cursor. Added, removed, or moved sources require a fresh catalog pagination.
`Chat.id` follows a native conversation ID when available; `source_id` identifies
a particular path under an admitted root. Moving a chat into the archive can
preserve its chat ID while changing its source ID. Clients must not silently
concatenate old and new sources.

## Bounds and refusals

| Resource | Bound |
| --- | --- |
| Catalog entries per response | 32 |
| Source-tree entries visited | 100,000 |
| Source path depth | 16 components |
| Notices per catalog | 128 |
| Codex title index | 32 MiB |
| One transcript source | 512 MiB |
| Raw bytes per transcript page | 32 KiB |
| Raw bytes per chunk | 8 KiB |
| Chunks per page | 128 |
| Serialized response | 112 KiB |
| Parsed record | 256 KiB |
| Readable text preview | 1 KiB per record; 1 KiB total per host page |

A limit returns `ResourceLimit` rather than a successful truncated catalog.
Source errors and ignored title-index records have explicit status or notices.
Readable previews can be shortened or deferred to satisfy the response bound;
the original transcript bytes remain available through chunks.

The host walks paths using held directory descriptors and `openat`, refuses
symlinks inside source trees, and opens only regular files. Callers submit opaque
source IDs, never paths. Roots are an operator trust decision: this is not a
sandbox against a privileged process, a mount change, or an operator placing a
sensitive regular file or hard link inside an admitted history tree.

A transcript cursor binds the file incarnation and a SHA-256 digest of every
previously consumed byte. The host verifies the prefix again before returning a
page. Replacement, consumed-prefix rewrite, truncation, and forged cursor
positions fail explicitly. The current implementation streams the prefix again
for each page rather than maintaining a trusted persistent index. Memory stays
bounded, but reading a very large conversation takes repeated prefix I/O; a
cache optimization must preserve these identity checks.

Opening a root grants local read access only. A caller must separately authorize
any device or relay disclosure, expiry, revocation, recipients, and encrypted
storage. Transcript contents can contain secrets and untrusted instructions;
this crate neither interprets them as commands nor redacts their raw bytes.

## Verification

[`src/host/tests.rs`](src/host/tests.rs) uses synthetic temporary histories for
archive and title discovery, missing sources, catalog changes, Unicode and
unknown records, partial appends, large-record chunking, file replacement,
truncation, malformed indexes, source confinement, and Claude subagent records.
No private conversation is a fixture. The tests exercise local file reading;
they do not claim provider compatibility across every version, phone rendering,
relay delivery, or a live harness session.

Run the focused checks with the repository's pinned toolchain:

```sh
cargo test -p coder-history --lib
cargo clippy -p coder-history --all-targets -- -D warnings
cargo check -p coder-history --no-default-features
```
