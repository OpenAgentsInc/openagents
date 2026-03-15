# Apple `.fmadapter` Package Spec

> Status: canonical repo-owned `.fmadapter` package reference for the retained
> OpenAgents repo as of 2026-03-14.

## Why This Doc Exists

The Apple exporter is the strongest concrete reference in the external adapter
toolkit, but OpenAgents cannot keep treating `.fmadapter` as an opaque future
detail.

This doc freezes the package inventory and metadata shape we intend to support
in Rust so `psionic-adapters`, `psionic-train`, the Apple bridge lane, and
later authority surfaces can all depend on one in-repo contract.

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

- This doc does not claim the Rust parser or writer already exists.
- This doc does not claim draft-model distillation is implemented.
- This doc does not let product docs market adapter hosting or training yet.
