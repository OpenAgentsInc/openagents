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
digests of the rendered search block and repository card, the full candidate
manifest the observation declared, and the selection record when a decision
door ranked it. Recording uses that captured object without rereading files or
parsing source text as metadata. The prompt and metadata jointly determine
trace deduplication. The card digest identifies its existing document prefixes;
it does not imply complete source coverage for those prefixes. Disabling trace
recording still disables this record.

## Evidence selection

When a decision door is configured and the observation names at least two
distinct paths, the `repository/select` site asks the
`openagents.evidence-relevance.v1` question set which candidates the task
wants. The request carries the task and each candidate's observable record —
path, span, readness, byte count, content digest, and the reason any refused
candidate withheld — never the content itself. The gate's options are exactly
the observed candidates plus the set's own `none`, so the model cannot invent
a path and abstention is an honest answer.

A chosen ranking reorders the excerpts the bounded block renders first; every
observation still renders within the same budgets, so a judgment reorders
evidence rather than erasing it. An abstention, a typed refusal, a transport
failure, and an answer naming no offered option all leave the deterministic
order standing — the failure is recorded as the `repository/select` decision
call in the trace, and no selection record lands in the manifest. A set
naming one path asks nothing at all.

A ranking answers again only while every identity still matches: the evidence
digest over the full candidate records and omissions, the question-set
wording digest, the bound policy digest, and the model artifact the door
reported. A new user message drops the cached ranking; a changed byte under
the same path is different evidence and is asked again.

Relevance is a judgment and disclosure is a grant. The selection manifest
records the bound each candidate sat under — `path-only`, `path-span`, or
`content` — decided by the resolved profile's locality and the host's read
grants, not by the score. A hosted profile sees at most what a read already
admitted, a refused candidate shows its path under every profile, and text
inside repository content carries no disclosure authority. The selection
record (`repository_context.selection`) keeps the verdict, the resolved pick,
the supplied distribution, the `any_relevant` and `coverage` probabilities,
the unranked options, the listed omissions, and every identity the judgment
is attributable to.

These references are consumed by the existing repository context path; they
are not a persistent evidence store. The repository card and its document
prefixes still have their existing behavior. Complete binding instruction
retention, diff/test evidence surfaces, and the downstream task-quality
measurement remain open. Path checks here are not an operating-system read
sandbox.
