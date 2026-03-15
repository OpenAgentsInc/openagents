# Apple Adapter Lineage Spec

> Status: canonical OpenAgents lineage and metadata reference for Apple adapter
> training/export artifacts as of 2026-03-14.

## Why This Doc Exists

The raw Apple exporter metadata is too thin to support truthful training,
evaluation, provider, and market claims on its own.

This doc freezes the lineage fields OpenAgents must capture around Apple
adapter jobs so later `psionic-adapters`, `psionic-train`, kernel authority,
and provider-substrate work can converge on one field set.

## Authority

- This doc is the canonical Apple adapter lineage reference.
- `crates/psionic/fixtures/apple_adapter/lineage/` is the canonical fixture
  corpus for expected lineage payloads.
- `crates/psionic/docs/TRAIN_SYSTEM.md` remains the canonical train-system
  architecture doc; this doc freezes Apple-specific metadata fields.

## Required Lineage Fields

OpenAgents should capture all of the following for Apple adapter artifacts.

### Base-model compatibility

- `baseModelSignature`
- stable base-model identifier when one exists
- stable base-model revision when one exists

For the current repo-owned Apple lane, `baseModelSignature` is no longer a
hard-coded desktop constant. It is derived from the live Apple runtime profile
the bridge exposes today: model id, use-case, and guardrail posture. That is
the compatibility anchor the operator path now carries through training,
export, runtime smoke, and kernel acceptance.

### Tokenizer and prompt shaping

- tokenizer family
- tokenizer digest
- special-token digest when relevant
- template or prompt-shaping digest
- locale-sensitive default-instruction posture when relevant

For the current repo-owned Apple lane, those values come from one repo-owned
compatibility preprocessing profile, not from ad hoc placeholders:

- tokenizer digest is derived from the current runtime profile plus the
  captured locale/default-instruction posture
- template or prompt-shaping digest is derived from the same profile plus the
  repo-owned Apple transcript-shaping rules
- token counts for packing are derived from that transcript-aware preprocessing
  path, including prior assistant turns, tool definitions, and response-schema
  attachments, rather than from character-count estimates

### Dataset and environment provenance

- dataset refs and versions
- split identifiers
- record-encoding family
- environment ref and version
- environment workload class

### Validation posture

- benchmark refs and versions
- validator policy ref
- runtime smoke-validation posture

### Package and artifact identity

- package format family
- package format version
- package digest
- adapter weights digest
- adapter identifier
- LoRA rank
- draft-model presence
- draft payload digests when present

## Where These Fields Belong

### Inside `.fmadapter`

Only the package-local subset belongs in `metadata.json`, typically through
exporter-native fields plus `creatorDefined`.

At minimum, OpenAgents should preserve or inject:

- `adapterIdentifier`
- `baseModelSignature`
- `loraRank`
- `creatorDefined.packageFormatVersion`
- `creatorDefined.tokenizerDigest`
- `creatorDefined.templateDigest`
- `creatorDefined.trainingEnvironmentRef`
- `creatorDefined.benchmarkRefs`
- `creatorDefined.validatorPolicyRef`
- `creatorDefined.draftModelPresent`

### In future `psionic-adapters` typed manifests

All of the above should later become typed fields on the Rust package layer so
serving, eval, and authority code do not need to inspect raw JSON.

### In training authority objects

The kernel and Nexus path should later retain:

- dataset and environment refs
- benchmark refs
- validator policy ref
- package digest
- draft-model presence
- compatibility anchors needed for accepted outcomes

### In accepted outcomes and provider receipts

Any later accepted outcome or hosted-serving receipt must preserve enough
identity to answer:

- what base signature was this adapter for?
- which tokenizer and prompt template shaped it?
- which exact package digest was accepted or served?
- was a draft-model payload involved?

The current runtime-smoke acceptance path now also checks the expected
base-model, tokenizer, and template digests against the exported package before
the app publishes acceptance.

## Fixture Corpus

Positive lineage fixture:

- `crates/psionic/fixtures/apple_adapter/lineage/training_metadata_v1.json`

Negative lineage fixture:

- `crates/psionic/fixtures/apple_adapter/lineage/training_metadata_missing_tokenizer.json`

## Non-Goals

- This doc does not define the final wire format for kernel/Nexus receipts.
- This doc does not claim those authority or provider surfaces are implemented.
- This doc exists so later implementation work does not invent lineage fields
  ad hoc in UI code or issue comments.
