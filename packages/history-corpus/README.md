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
- **`recall.ts`** — the `HistoryRecall` contract schemas. The request names
  a corpus, one question, and the budget caps. The response holds the cited
  answer spans, the required honesty record, and the cost record.
- **`recall-tier-d.ts`** — the Tier D recall engine. It answers each
  question with a pure traversal of the corpus. It makes zero model calls.

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

## Tier D recall

Tier D is the deterministic recall engine of RLM-02 (issue #9138). It is one
of the two tiers in the `HistoryRecall` service design. It ships first.

- **Questions.** The request carries one question. The question kinds are
  `Grep`, `CursorSlice`, `TimeSlice`, `KeyTurns`, and `TurnSummary`. Each
  kind is a pure operation over the ordered corpus.
- **Cited spans.** Each answer is a cursor span. A span names the scope ref,
  the turn id, the inclusive sequence range, a bounded excerpt, and the
  entry kind. The cursor is the citation. The engine does not paraphrase.
- **Caps truncate. They do not fail.** The caps are `maxSpans`,
  `maxEntriesScanned`, and `maxCharsPerSpan`. The scan stops at the entry
  cap. The answer list stops at the span cap. The excerpt stops at the
  character cap. Each hit cap goes into the honesty record. Only invalid
  input fails, with a typed `HistoryRecallError`. An example is a grep
  pattern that does not compile.
- **Honesty is required.** Each response states the scanned entry count,
  the total entry count, the truncation state, the hit caps, and the corpus
  coverage note. The response must state what the scan could not see.
- **Zero model calls.** The cost record reports `modelCalls: 0` for each
  Tier D answer. The tests assert this for each question kind.
- **Service shape.** `HistoryRecall` is the Effect service tag with one
  verb, `recall`. `historyRecallTierDLayer` builds the Tier D layer from a
  corpus provider. A request can also carry a prebuilt corpus inline.
- **Recall output is a cited candidate. It is not authority.**

## Boundaries

- The corpus is a read artifact for owner-local recall. It is not authority.
- Raw history does not leave owner-local execution. Public projections
  receive nothing from this package.
- The builder does not dereference refs. Prompt refs, result refs, and raw
  sidecar refs stay refs.

## Host tool (RLM-03)

`host-tool.ts` exposes `history_recall` as an Effect AI `Tool` / `Toolkit`
over `HistoryRecall`. The wire form is registered in
`@openagentsinc/agent-harness-contract` (`historyRecallHostToolSpec`). STREAM-07
projects the Tool onto that wire form. Dispatch resolves through the Toolkit
handler Layer and re-enters the neutral stream as `tool.call` / `tool.result`
(payload stays on the host-tool result path as a `resultRef`). Desktop main
wires the tool against owner-local stores.

## Recursive recall (RLM-04, Effect-native)

`recursive-recall.ts` is the Effect-native recursive recall engine. It replaces
the paper's Python REPL with a bounded typed-operation agent loop. The root
model is an injected `LanguageModel` from `effect/unstable/ai`. Each iteration
the model emits one typed operation over the corpus — `Grep`, `CursorSlice`,
`TimeSlice`, `TurnSummary`, `Subcall`, or `Answer` — decoded fail-closed with
Effect Schema. Caps bound depth, iterations, subcalls, tokens, and time. The
result is `Completed`, `Partial` with an honest reason, or `Failed` with a
typed failure class. Partial-answer honesty is mandatory. There is no Python
runtime and no code execution anywhere.
