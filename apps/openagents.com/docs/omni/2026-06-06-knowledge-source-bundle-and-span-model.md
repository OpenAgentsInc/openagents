# Knowledge Source Bundle And Span Model

Status: implemented for issue #369 / `OPENAGENTS-LATE-009`.

## Purpose

Knowledge workrooms need source-backed records, not summaries pretending to be
evidence. This contract separates source bundles, extracted spans, generated
summary refs, provenance, rights, digests, and redaction policy.

Implementation:

- `workers/api/src/omni-knowledge-source-bundles.ts`
- `workers/api/src/omni-knowledge-source-bundles.test.ts`

## Source Records

Supported source kinds:

- connector reads;
- data packages;
- files;
- links;
- repository refs;
- tables; and
- transcripts.

Each source record carries:

- source ref;
- locator ref;
- title ref;
- digest algorithm and digest ref;
- rights state and rights refs;
- provenance refs;
- caveat refs;
- redaction policy refs;
- data classification; and
- trust tier.

Generated summaries are intentionally not source records. They are attached
through `generatedSummaryRefs` so retrieval, proof, and review surfaces can
distinguish cited source material from agent-written summaries.

## Extracted Spans

Supported span kinds:

- page;
- row;
- table cell;
- transcript;
- code; and
- file range.

Each span points back to a source in the same bundle and carries a content
digest ref, excerpt ref, fact candidate refs, provenance refs, rights refs,
redaction policy refs, classification, and trust tier.

Kind-specific validation prevents vague citations:

- page spans require a positive page number;
- row spans require an ordered row range;
- table-cell spans require an ordered row range and column refs;
- transcript spans require an ordered time range;
- code spans require an ordered line range; and
- file-range spans require either line or byte ranges.

## Projection Audiences

The first projections are:

- `public`;
- `team`; and
- `operator`.

Public projections remove private source, span, digest, locator, excerpt,
summary, rights, and workroom refs. Team projections can retain more team-safe
refs but still remove private source archives and private source/span refs.
Operator projections can see the full safe ref set.

Visible source and span counts count only records still visible after
redaction.

## Authority Boundaries

Knowledge source bundles are read-only. They cannot:

- mutate connectors;
- mutate generated summaries;
- upgrade public claims;
- copy raw source archives; or
- mutate rights.

If a packet relaxes any of those boundaries, projection throws
`OmniKnowledgeSourceBundleUnsafe`.

## Safety Filters

The contract rejects:

- private customer/provider/wallet/payment material;
- raw connector, source, file, repo, transcript, text, email, runner, prompt,
  archive, payment, payout, and webhook material;
- private repository refs;
- generated summary text masquerading as a ref;
- source records that use generated-summary refs as source refs;
- secret-shaped values; and
- raw timestamps.

Projection leak checks inspect string values, not object keys, so authority flag
names such as `rawSourceArchiveCopyAllowed` do not create false positives.

## How It Feeds Retrieval And Proof

The bundle model is the source layer for later retrieval trace and
graph-curated context work:

- source bundles identify what material was eligible;
- extracted spans identify exactly what page, row, time range, line range, or
  table cell was used;
- generated summaries remain separate artifacts;
- fact candidates can point at spans before being promoted into workroom
  objects; and
- proof surfaces can cite span refs without exposing raw private content.

Writes that promote extracted facts into CRM, project, legal, support, finance,
or investor objects still require a later approval-gated path.

## Tests

Coverage includes:

- valid projection of source records and extracted spans;
- page, row, table-cell, transcript, code, and file-range validation;
- provenance, digest, rights, active-rights, and read-only authority
  requirements;
- public redaction of private refs; and
- generated-summary separation plus unsafe raw/source material rejection.
