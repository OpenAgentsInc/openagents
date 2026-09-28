# Saved harness history

`coder-history` reads saved Codex and Claude conversations and Coder task
transcripts from explicitly selected desktop directories. It provides catalog pages and complete source
bytes in bounded transcript pages. It does not start, resume, interrupt, or
modify a harness. It has no model or network client.

The default `host` feature supports macOS and Linux. Set
`default-features = false` to use the serializable request, cursor, page, and
readable-record types on a client without filesystem access. See
[`src/lib.rs`](src/lib.rs) for the portable contract and
[`src/host.rs`](src/host.rs) for `Config` and `History`.

## Read a history

A desktop operator selects absolute `Config.codex`, `Config.claude`, and
`Config.coder` roots.
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

The Coder adapter reads Coder's task directory, such as `~/.openagents/tasks`.
It lists only the `<task>.<attempt>.atif.jsonl` files directly inside that
directory, one chat per task attempt, newest first by last write. Their native
ID is the 64-hex task ID, and their title is the first line of the first
`User` step. It ignores subdirectories and every other file there.

The OpenCode adapter reads the host's mirror of OpenCode's sessions, such
as `~/.openagents/opencode/mirror`. OpenCode 1.2 and later keep sessions in
a SQLite database, so `opencode::mirror` (feature `opencode`) copies each
session into `ses_<id>.jsonl` there: an `opencode.session` header, then one
`opencode.part` line per finished part in OpenCode's order, and an
`opencode.error` line after a reply that failed. It opens the database
read-only and writes only the mirror. The catalog lists the `ses_*.jsonl`
files directly inside the mirror; titles, update times, and archiving come
from its `session_index.jsonl`, and a session with a parent lists as a
subagent. `coder host` runs a pass every five seconds. A part projects as a
`message` (text, by the message's role), `reasoning`, `tool_call` (the
tool's title or input, then its output or error), or an `adapter` record
with no text; an error line is a `system` message.

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

A Coder transcript line (`crates/atif`) projects as follows:

| Record | `kind` | `role` | `text` |
| --- | --- | --- | --- |
| `session` | `session` | none | empty; `native_id` is the session ID |
| `User` step | `message` | `user` | the message |
| `System` step with no extensions | `message` | `system` | the message |
| `System` step, loop event `generated` with an `Ok` action | `message` | `assistant` | the rationale, then `Finished.` on its own line when finished |
| `System` step, loop event `generated` with an `Err` | `message` | `system` | `The model call failed: ` and the error's first line, at most 300 bytes |
| `System` step, loop event `ran` | `tool_call` (`shell`) | none | the command, then `exit N`, `timed out`, or `ended by a signal` when it failed, then a blank line and the output |
| `System` step, loop event `tested` | `tool_call` (`tests`) | none | one `COMMAND: exit N` line per test, by each command's first line |
| `System` step, loop event `ended` | `message` | `system` | `Coder finished in N steps.`, or `Coder stopped: ` and the reason |
| Any other `System` step with extensions | `adapter` | none | empty |
| `Agent` step with a message | `message` | `assistant` | the message, then `Tool: NAME ARGS` for each call after a blank line |
| `Agent` step with only a call | `tool_call` | none | the argument summary, then the call's result after a blank line |
| Step with only an observation | `tool_result` | `tool` | the result content |
| `Agent` step with only reasoning | `reasoning` | none | the reasoning |
| `end` | `end` | none | the end state |

A loop event is `extensions.microcoder.event`, the `run::Event` that
`crates/microcoder` reports. Admission, effect, decision, summary, and fault
evidence, and the loop's other events, such as `judged` and `gated`, are
`adapter` records: readers can skip them, and their raw bytes stay available.
A stop reason other than `bad_replies` is its name in words, such as
`step limit`; `bad_replies` shows the first line of its detail.

`tool_name` and `call_id` name a step's first call. The argument summary is a
shell command's text or compact JSON, at most 240 bytes. The reader accepts both
the log's `call` field and the exported document's `tool_calls` and
`observation`. A record with another shape stays an explicit unknown
projection.

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
