# Model IO Reference

> Status: canonical `PSI-282` / `#3587` reference record, updated 2026-03-14
> after landing the first typed model-IO portability layer in
> `crates/psionic/psionic-train/src/model_io.rs`.

This document records the first explicit Rust-native model-IO contract for the
Psionic train stack.

## Canonical Runner

Run the contract harness from the repo root:

```bash
scripts/release/check-psionic-model-io-contracts.sh
```

## What Landed

`psionic-train` now owns a typed model-IO portability layer that sits between
training state and serving-compatible artifact surfaces.

The new typed surfaces include:

- `PortableModelStateDict`
- `ModelStateGroupAssignment`
- `ModelStateTensorManifest`
- `PortableTokenizerBinding`
- `PortableModelBundle`
- `ModelIoArtifactReceipt`
- `ModelAdapterDelta`

## Portability Surfaces

The model-IO layer now supports these explicit portability surfaces:

- Psionic-native in-memory state dict ownership
- dense safetensors export and import with embedded Psionic manifest metadata
- JSON torch-style state-dict compatibility artifacts
- GGUF import with tensor inventory, tokenizer binding, and chat-template digest
- additive adapter merge and unmerge on parameter tensors

## What The Contract Makes Explicit

The model-IO issue was not just "save some tensors to disk." The new contract
makes all of the following typed and inspectable:

- named state-dict traversal records
- Rust model-tree assignment paths for each tensor
- training-group-to-state-dict assignment contracts
- optimizer-state portability alongside train-visible parameters
- checkpoint family and checkpoint reference binding
- tokenizer family, digest, asset version, and special-token posture
- chat-template digest binding
- portability receipts for artifact import and export
- typed adapter delta derivation and reversal

## Current Interop Boundaries

The current portable layer is intentionally specific about what it supports:

- safetensors export and import are for dense `f32` training-state artifacts
- the torch-compatible surface is a typed JSON artifact, not a Python pickle
  or opaque `.pt` loader
- GGUF support is currently import-focused, because that is the relevant
  train-to-serve portability seam in the retained stack
- GGUF-imported quantized tensors are preserved in portable state, but they are
  not re-emitted as safetensors without an explicit dequantization or conversion
  step

Those limits are deliberate. The goal of this issue was to stop trained or
served artifacts from being stranded behind bespoke conversion scripts, not to
pretend every foreign binary format is already supported.

## Pass Criteria

The contract is green only if all of the following remain true:

- portable state dicts can roundtrip training groups without losing optimizer
  state, residency posture, or applied-step truth
- safetensors artifacts carry enough embedded metadata to recover Psionic
  assignment contracts
- JSON state-dict artifacts remain machine-legible and replay-safe
- GGUF import binds tokenizer and chat-template identity instead of treating
  them as detached side files
- adapter deltas can be derived, merged, and unmerged against the same typed
  state-dict surface

## Current Limits

This issue does not claim that the whole train-to-serve conversion story is
finished. It does not yet implement:

- direct Python pickle or opaque `.pt` checkpoint decoding
- full GGUF export
- tokenizer asset re-emission beyond the typed binding contract
- model-family-specific structural assignment for full decoder trees

What it does do is give Psionic one canonical, Rust-owned portability contract
that training and serving work can both target.
