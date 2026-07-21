# @openagentsinc/history-corpus

This package builds a history corpus. A history corpus is a deterministic
export of durable conversation history for one scope. Each unit in the corpus
has a cursor address. The export obeys the visibility and redaction rules of
each source unit. This is RLM-01 of the RLM epic (#9136, issue #9137). The
design source is `docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md`.

## What it contains

- **`corpus.ts`** — the Effect Schemas. `HistoryCorpusEntry` is one bounded,
  safe projection of one neutral runtime event or one thread note.
  `HistoryCorpusScope` selects one thread, one run, or a thread set.
  `HistoryCorpusPolicy` lists the admitted visibilities and redaction
  classes. `HistoryCorpusManifest` records the scope, the build time from the
  input, the entry count, the exact byte length, the coverage statement, and
  the exclusions record. `HistoryCorpusError` is the typed failure.
- **`builder.ts`** — the pure builder `buildHistoryCorpus`. Inputs are the
  scope, an optional durable event-log reader with the turn ids to read, the
  optional neutral thread snapshots, the policy, and the build timestamp.
  Output is the manifest plus the ordered entries.

## Sources

The builder reads two sources. It does not write to them.

- The durable harness event log (`HarnessEventLogStore` in
  `@openagentsinc/agent-harness-contract`). Each event is a neutral
  `KhalaRuntimeEvent` with a `sequence` cursor.
- Neutral thread snapshots. The desktop thread-store shape satisfies the
  `NeutralThreadSnapshot` schema structurally. This package does not import
  the desktop app.

## Guarantees

- **Deterministic.** The same inputs give an identical corpus. The order is
  stable: thread id, then turn id, then sequence. The build time comes from
  the input, not from a clock.
- **Cursor addressed.** For an event entry, `(turnId, sequence)` is the exact
  durable cursor of the source event. A read of the source store at
  `fromCursor = sequence - 1` returns that event first. A thread note gets a
  synthetic address: the note key (or `note.<threadId>.<index>`) plus the
  note index. A duplicate address is a build error.
- **Redaction aware.** An event outside the admitted visibilities or
  redaction classes does not enter the corpus. The manifest counts each
  exclusion. Nothing is dropped without a count. Thread notes carry raw
  owner-local text, so they enter filtering as `private` / `private_ref`.
- **Honest coverage.** The neutral log at HEAD carries only the seven core
  kinds: `turn.started`, `turn.finished`, `turn.interrupted`, `text.delta`,
  `reasoning.delta`, `tool.call`, and `tool.result`. Plan, meter, question,
  child, and notice facts do not reach the neutral log. The manifest
  coverage statement names the kinds that are in the corpus, the kinds that
  are not, and this bound.

## Boundaries

- The corpus is a read artifact for owner-local recall. It is not authority.
- Raw history does not leave owner-local execution. Public projections
  receive nothing from this package.
- The builder does not dereference refs. Prompt refs, result refs, and raw
  sidecar refs stay refs.
