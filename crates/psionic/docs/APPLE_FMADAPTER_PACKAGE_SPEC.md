# Apple `.fmadapter` Package Spec

> Status: canonical repo-owned `.fmadapter` package reference for the retained
> OpenAgents repo as of 2026-03-16.

## Why This Doc Exists

The Apple exporter is the strongest concrete reference in the external adapter
toolkit, but OpenAgents cannot keep treating `.fmadapter` as an opaque future
detail.

This doc freezes the package inventory and metadata shape we intend to support
in Rust so `psionic-adapters`, `psionic-train`, the Apple bridge lane, and
later authority surfaces can all depend on one in-repo contract.

`psionic-adapters` now implements the first Rust-native reader, writer,
inventory validator, lineage extractor, and generic manifest bridge for this
package family.

## Authority

- This doc is the canonical repo-owned `.fmadapter` package-layout spec.
- `crates/psionic/fixtures/apple_adapter/packages/` is the canonical fixture
  corpus for positive and negative package-shape tests.
- The external Apple exporter remains reference material only.

## Source Inputs Used To Freeze This Spec

- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/constants.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/export_fmadapter.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/export_utils.py`
- `crates/psionic/psionic-adapters/src/lib.rs`

## Canonical Package Root

- A package is a directory with the suffix `.fmadapter`.
- The directory name is the stable artifact label exposed to operators.
- OpenAgents should treat the package as a structured artifact family, not as a
  generic `safetensors` bundle.

## Required Inventory

Every valid package must contain:

- `metadata.json`
- `adapter_weights.bin`

## `adapter_weights.bin` Payload Contract

For the live Apple adapter lane, `adapter_weights.bin` is not just raw
concatenated fp16 bytes.

It is a Core ML blob-storage container:

- the file starts with one 64-byte file header
- bytes `0..4` are the tensor-record count as little-endian `u32`
- bytes `4..8` are the current blob-storage version as little-endian `u32`
  (`2` in the retained toolkit oracle and current Rust-native writer)
- each tensor record starts at a 64-byte-aligned offset
- each tensor record begins with one 64-byte header:
  - bytes `0..4`: magic `0xdeadbeef` as little-endian `u32`
  - bytes `4..8`: blob kind (`1` for fp16 payloads in the current adapter lane)
  - bytes `8..16`: payload byte length as little-endian `u64`
  - bytes `16..24`: absolute payload offset as little-endian `u64`
- tensor payload bytes are fp16 little-endian data
- records are padded to the next 64-byte boundary between payloads

Bridge acceptance depends on that container shape. A package with correct
metadata and correct tensor values can still be rejected if
`adapter_weights.bin` is emitted as raw bytes instead of this blob-storage
layout.

## Optional Draft-Model Inventory

Draft-model payloads are optional, but if present they must be complete:

- `draft.mil`
- `draft_weights.bin`

OpenAgents should treat partial draft payloads as invalid. `draft.mil` without
`draft_weights.bin`, or the reverse, is not a valid package state.

## Metadata Shape

The Apple exporter writes camelCase JSON keys. OpenAgents should preserve that
shape on import and export.

Required metadata keys:

- `adapterIdentifier`
- `baseModelSignature`
- `loraRank`

Common exporter keys:

- `author`
- `description`
- `license`
- `speculativeDecodingDraftTokenCount`
- `creatorDefined`

## Metadata Rules Frozen For OpenAgents

### `adapterIdentifier`

- Must be a non-empty string.
- Apple reference exporters derive it as
  `fmadapter-<adapter_name>-<base_signature_prefix>`.
- OpenAgents should preserve that convention for compatibility and operator
  legibility.

### `baseModelSignature`

- Must be a 40-character lowercase hex string in the current fixture corpus.
- It is the primary compatibility anchor between the adapter package and the
  Apple base model family.
- Package import should refuse obviously malformed signatures.
- The current live Apple reference lane in this repo uses
  `9799725ff8e851184037110b422d891ad3b92ec1` when the runtime does not surface
  a more specific explicit signature.

### `loraRank`

- Must be a positive integer.
- Rank is part of artifact comparability and should not be hidden in raw JSON.

### `creatorDefined`

- Is reserved for developer-defined or OpenAgents-defined lineage extensions.
- Unknown `creatorDefined` fields should roundtrip.
- OpenAgents should use this map for package-local lineage values such as:
  - `packageFormatVersion`
  - `tokenizerDigest`
  - `templateDigest`
  - `trainingEnvironmentRef`
  - `benchmarkRefs`
  - `validatorPolicyRef`
  - `draftModelPresent`

## Inventory Validation Rules

A valid package must satisfy all of the following:

- package root ends with `.fmadapter`
- `metadata.json` exists and parses as JSON
- `adapter_weights.bin` exists
- required metadata keys are present
- `baseModelSignature` is well formed
- `loraRank` is positive
- draft files are either both absent or both present

OpenAgents should surface typed incompatibility or malformed-package errors
rather than collapsing everything into generic IO failure.

## Fixture Corpus

Positive fixtures:

- `crates/psionic/fixtures/apple_adapter/packages/minimal_chat_adapter.fmadapter/`
- `crates/psionic/fixtures/apple_adapter/packages/draft_chat_adapter.fmadapter/`

Expectation sidecars:

- `crates/psionic/fixtures/apple_adapter/packages/minimal_chat_adapter.expected.json`
- `crates/psionic/fixtures/apple_adapter/packages/draft_chat_adapter.expected.json`

Negative fixtures:

- `crates/psionic/fixtures/apple_adapter/packages/invalid_missing_metadata.fmadapter/`
- `crates/psionic/fixtures/apple_adapter/packages/invalid_bad_base_signature.fmadapter/`
- `crates/psionic/fixtures/apple_adapter/packages/invalid_draft_pairing.fmadapter/`

The payload files in these fixtures are intentionally small placeholder blobs so
the repo can freeze package inventory, metadata shape, and digest behavior
without checking in heavyweight Apple-exported binaries.

## Non-Goals

- This doc is not the execution authority for the draft-model lane; the
  repo-owned reference distillation path now lives in
  `crates/psionic/psionic-train/src/apple_adapter.rs`.
- This doc does not let product docs market adapter hosting or training yet.
