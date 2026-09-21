# Repository source references

Ordinary Coder repository context includes deterministic search excerpts with
`openagents.repository-source.v1` references. Each reference records the
repository-relative path, one-based line number, complete observed file-content
digest, excerpt digest, and whether the line was shortened. `base` is the
observed Git commit when available. It does not imply a clean working tree:
the source digest identifies the bytes read, including uncommitted changes.

The content and excerpt digests use `atif::digest` over their JSON strings.
They are not the raw-file checksum printed by `sha256sum`. The collector reads
one complete UTF-8 file before selecting lines, so all excerpts from that file
share the same observed contents. This does not claim a transactionally
consistent snapshot across files or concurrent writers.

Search uses up to four draft terms and eight tracked paths. Each admitted
source file is at most 1 MiB; an oversized file is excluded as an atomic input
rather than silently read as a prefix. The collector excludes non-relative
paths, sources resolving outside the repository, and non-file sources.
Git commands run under the shared process supervisor, with two-second deadlines
and bounded capture. A truncated search result is unavailable, not a complete
candidate list.

Up to ten matching lines contribute at most 2 KiB of quoted source text.
Each line prefix ends at a UTF-8 boundary within 160 bytes. References have a
separate budget, and the entire rendered search-evidence block is at most
8 KiB, including reserved coverage diagnostics. Diagnostics distinguish
unavailable search/read results, excluded sources, and omitted paths or
excerpts. The collector does not claim exhaustive recall.

Each generation collects its prompt and evidence together. Its ATIF instruction
step retains `repository_context` with schema `openagents.repository-context.v1`,
search terms, only the source references actually rendered, coverage diagnostics,
and digests of the rendered search block and repository card. Recording uses
that captured object without rereading files or parsing source text as metadata.
The prompt and metadata jointly determine trace deduplication. The card digest
identifies its existing document prefixes; it does not imply complete source
coverage for those prefixes. Disabling trace recording still disables this record.

These references are consumed by the existing repository context path; they
are not a persistent evidence store or a relevance model. The repository card
and its document prefixes still have their existing behavior. Complete binding
instruction retention, host read/disclosure policy, complete context manifests
and their interface, diff/test evidence, and measured semantic selection
remain part of #9513. Path checks here are not an operating-system read sandbox.
