# Apple Adapter Dataset Spec

> Status: canonical Apple-adapter training-data reference for the retained
> OpenAgents repo as of 2026-03-14.

## Why This Doc Exists

The Apple adapter reference repos describe the data shape we need, but they are
not the runtime contract for OpenAgents.

This doc freezes the Apple-adapter dataset shape we intend to implement in
Rust so later `psionic-data`, `psionic-apple-fm`, `psionic-environments`, and
`psionic-train` work can code against a stable in-repo spec and fixture corpus.

The external Apple repos remain reference inputs only.

## Authority

- This doc is the canonical repo-owned Apple adapter dataset spec.
- `crates/psionic/fixtures/apple_adapter/datasets/` is the canonical fixture
  corpus for positive and negative dataset examples.
- `crates/psionic/docs/TRAIN_SYSTEM.md` remains the canonical train-system
  architecture doc; this doc only freezes the Apple-specific data shape.

## Source Inputs Used To Freeze This Spec

- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/docs/schema.md`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/data.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/messages.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/utils.py`
- `crates/psionic/psionic-data/src/lib.rs`
- `crates/psionic/psionic-apple-fm/src/contract.rs`

## Canonical Record Shape

- Each dataset file is UTF-8 `jsonl`.
- Each line is exactly one training sample.
- Each sample is one JSON array of message objects.
- The raw Apple-style message roles admitted by this spec are:
  - `system`
  - `user`
  - `assistant`

## Message Object Rules

### Shared fields

- `role`: required string.
- `content`: required string.

### `system` messages

- At most one `system` message may appear in a sample.
- If present, it must be the first message.
- `tools` may appear only on the first `system` message.

### `user` messages

- `response_format` may appear only on `user` messages.
- `response_format` is admitted only for guided-generation fixtures and must be
  fully specified when present.

### `assistant` messages

- `assistant` messages must follow a `user` message.
- For training datasets, the final message in the sample must be
  `assistant`.

## Ordering Rules

- A sample must contain at least one `user` / `assistant` pair.
- Consecutive identical roles are invalid.
- `assistant` may not appear before the first `user`.
- A `system` message after index `0` is invalid.

These rules are frozen from the Apple examples and validation logic and are
also compatible with the later `psionic-data` contract work.

## Default Instruction Guidance

The Apple examples consistently rely on the default instruction:

`A conversation between a user and a helpful assistant.`

OpenAgents should treat that as a training-data convention, not as implicit
runtime magic. If a dataset depends on the default instruction, it should be
recorded either:

- directly in the first `system` message, or
- in lineage metadata through the tokenizer/template digest fields described in
  `APPLE_ADAPTER_LINEAGE_SPEC.md`.

## Guided Generation

This spec freezes two Apple-compatible guided-generation modes.

### 1. Schema-free guided generation

- The `assistant.content` payload is a stringified JSON object.
- The canonical fixture format uses exactly one space after each structural
  comma and colon inside that stringified JSON payload.
- No `response_format` object is required when the adapter is trained for one
  stable output shape.

### 2. Full guided-generation schema

- The `user` message carries `response_format`.
- `response_format.type` must currently be `json_schema`.
- `response_format.json_schema.name` is required.
- `response_format.json_schema.schema` is required.
- The `assistant.content` payload is a stringified JSON object that conforms to
  the declared schema.

OpenAgents should preserve the raw schema payload and treat schema compaction or
prompt augmentation as deterministic derived behavior, not as handwritten
desktop-local logic.

## Tool Calling

For Apple training fixtures, tools are frozen in the raw Apple reference shape:

- tools live on the first `system` message
- each tool object uses:
  - `type`
  - `function.name`
  - `function.description`
  - `function.arguments`

This doc freezes the external training-data shape, not the later normalized
Rust runtime tool contract used by `psionic-apple-fm`.

## Tokenizer And Prompt-Shaping Capture

The raw dataset lines do not carry tokenizer digests directly, but OpenAgents
must treat the following as part of the dataset contract:

- tokenizer family
- tokenizer digest
- special-token digest when relevant
- chat-template or prompt-template digest
- locale-sensitive default-instruction behavior when relevant

Those values belong in lineage metadata and fixture expectations rather than in
every dataset row.

## Curated Corpus Overlay

For reviewed real-run corpora, OpenAgents may attach a repo-owned curation
manifest next to the raw JSONL splits without changing the Apple-compatible row
shape itself.

That overlay is where the repo should freeze:

- target identity and scope
- reviewed source inventory
- per-split sample annotations
- task-family tags
- source-provenance tags
- split-leakage policy

The first such corpus now lives under:

- `crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/`

The raw JSONL files remain the canonical Apple-style training records. The
curation overlay adds OpenAgents review and provenance semantics on top of
those rows.

## Fixture Corpus

Positive fixtures:

- `crates/psionic/fixtures/apple_adapter/datasets/minimal_sft_train.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/schema_free_guided_generation_train.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/guided_generation_with_schema_train.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/tool_calling_train.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/train.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/held_out.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/benchmark.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/corpus_manifest.json`

Negative fixtures:

- `crates/psionic/fixtures/apple_adapter/datasets/invalid_system_not_first.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/invalid_assistant_not_last.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/invalid_duplicate_roles.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/invalid_missing_response_schema.jsonl`
- `crates/psionic/fixtures/apple_adapter/datasets/invalid_tool_definition.jsonl`

## Non-Goals

- No Python execution is part of the supported OpenAgents flow.
- No Tk or AFMTrainer workflow is part of this dataset contract.
- This doc does not claim the training runtime already exists; it freezes the
  input shape the runtime must later implement.
