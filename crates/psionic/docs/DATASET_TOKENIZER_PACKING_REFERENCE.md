# Dataset, Tokenizer, and Packing Reference

> Status: canonical `#3567` data-contract record, updated 2026-03-15 after
> landing the runnable harness in
> `scripts/release/check-psionic-data-contracts.sh`.

This document records the first Psionic-native data contract layer.

## What Landed

The issue landed the `psionic-data` crate with:

- canonical `dataset_ref@version` identity through `DatasetKey`
- typed `DatasetManifest` objects with tokenizer digests, split declarations,
  and shard-level lineage
- shard contracts that bind directly to `psionic-datastream` manifest refs for
  tokenized corpora
- deterministic streamed iteration contracts with resume-safe cursors and
  optional epoch-shuffled shard order
- sequence-packing and batch-packing policies for long-context or token-budget
  workloads

On 2026-03-15, issue `#3621` extended the same crate with a repo-owned Apple
adapter dataset lane:

- JSONL import into typed Apple message, tool, response-schema, and sample
  records
- fixture-backed validation for Apple role ordering, tool-definition shape, and
  `response_format` schema completeness
- dataset-level tokenizer and prompt-shaping lineage metadata for later
  train/eval parity checks
- deterministic packing over explicit prompt/completion/tool/schema token
  captures, with typed refusal on tokenizer or prompt-shaping drift

The environment ABI now binds versioned dataset keys from this crate instead of
free-form dataset refs, but kernel and Nexus still own any future dataset
registry or authority truth.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-data-contracts.sh
```

## Workload Shape

The current reference path proves one bounded but real data-contract workload:

1. define a versioned dataset manifest
2. attach tokenizer identity and tokenized shard refs
3. validate split totals and shard-level dataset binding truth
4. plan a resume-safe streamed iteration window
5. plan long-context packing or token-budget batches over sequence descriptors

The Apple adapter extension now proves a second bounded workload:

1. import Apple training fixtures into typed Rust records
2. preserve tool and schema attachments at the data-contract layer
3. bind tokenizer and prompt-shaping lineage at dataset scope
4. derive token captures from the repo-owned Apple transcript-preprocessing path
5. plan deterministic packing only when token captures match that lineage

## Pass Criteria

The data layer is green only if all of the following are true:

- dataset identity is versioned and stable-keyed as `dataset_ref@version`
- tokenized shard refs are typed and validated against `psionic-datastream`
- split lineage is explicit at shard, sequence-count, and token-count level
- iteration state is resume-safe and deterministic across epochs
- packing policy is machine-legible rather than hidden in ad hoc loader code

## Expected Signals

The current harness should prove:

- `DatasetManifest::stable_digest()` is deterministic
- shard contracts refuse wrong datastream subjects or mismatched dataset
  bindings
- `DatasetIterationContract` can advance cursors and wrap epochs in repeat mode
- `DatasetPackingPolicy` can both pack short sequences into long-context rows
  and batch rows under a token budget
- `psionic-environments` can now bind versioned datasets through `DatasetKey`
- Apple adapter fixtures import into typed records with explicit refusal paths
  for malformed roles, missing schemas, and invalid tools
- Apple adapter token captures are derived from full transcript context,
  tool/schema attachments, and locale/default-instruction metadata instead of
  character-count estimates
- Apple adapter packing refuses tokenizer drift and prompt-shaping drift instead
  of silently reusing stale token counts

## Current Limitations

This issue intentionally does not claim:

- a kernel-owned dataset registry or marketplace authority surface
- raw-byte dataset ingestion or tokenization pipelines
- curriculum scheduling, freshness policy, or sample-weight policy
- validator-owned benchmark packaging or eval scoring
- full trainer integration with dataset manifests on every step receipt

Those remain later issues. This issue makes the reusable data identity,
lineage, iteration, and packing contracts real first.
