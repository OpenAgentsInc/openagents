# Forensic prior-work authority

Date: 2026-08-03

Omega issue 229 admits the forensic prior-work authority as a native All Work
history service. The authority is implemented beside the generated All Work
boundary without changing its generated definition. OpenAgents issue 9305 owns
that generator and remains collision-free.

## Identity

An occurrence identity binds the repository, revision, normalized path, symbol,
line range, and source-window digest. A rename, move, or revision change creates
a new occurrence. Relations can state `related`, `supersedes`, `split_from`, or
`merged_into` with explicit confidence. The service does not invent continuity.

A root-cause identity binds the versioned mechanism class, normalized causal
mechanism, affected behavior, and security boundary. It deliberately excludes
the focal path and source-window digest. Thus, one causal defect can own several
occurrences, while two different mechanisms with colliding source windows stay
separate.

Both algorithms are versioned hot contracts. Identity changes require a new
version and explicit reconciliation rather than silent rekeying.

## Durable Work history

The Effect authority persists under `all-work/forensic-prior-work.v1.json` with
an atomic replace and revision compare. It retains:

- every stable Work ref and occurrence;
- prompt, source, and evidence refs;
- the causal-chain summary;
- append-only disposition and relation events;
- first-identification and update times;
- idempotency command digests; and
- bounded query receipts and cursors.

Exact replay is idempotent. Conflicting replay fails. Concurrent same-cause
submissions retry revision conflicts and converge into one root-cause record
without losing either occurrence or the earliest identification time.

## Query and privacy

Exact lookup accepts a Work, occurrence, root-cause, or record ref. Bounded
semantic search ranks normalized mechanism and causal-chain terms. The
disposition filter includes confirmed, dismissed, rejected, inconclusive,
expired, superseded, corrected, duplicate, and retained Work.

Audience filtering runs before exact or semantic matching. An unauthorized
query receives no inaccessible record, count, timing, or existence signal. The
receipt reports completeness only for the caller-authorized population.

Omega consumes this authority through its prior-work projection. It does not
store a second private duplicate database in the UI.
