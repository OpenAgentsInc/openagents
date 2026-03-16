# Extension Contract Semantics

> Status: canonical `PLIB-217` / `#3732` reference record, updated 2026-03-16
> after landing the first bounded extension-contract report in
> `crates/psionic/psionic-ir/src/lib.rs`.

This document records the current bounded extension-contract semantics surface
for Psionic.

## Canonical Runner

Run the extension harness from the repo root:

```bash
scripts/release/check-psionic-extension-contract-semantics.sh
```

## What Landed

`psionic-ir` now exposes:

- `CustomOpExtensionContract`
- `CustomKernelExtensionContract`
- `CustomAutogradExtensionContract`
- `BackendPluginExtensionContract`
- `QuantizerPluginExtensionContract`
- `ExtensionContractSemanticsReport`
- `builtin_extension_contract_semantics_report()`

## Current Honest Posture

Today Psionic has a first-class typed extension-contract surface, but it does
**not** claim a full dynamic plugin ecosystem yet.

The bounded seeded surface now makes these seams explicit:

- custom ops must use declared-output schema contracts
- custom kernels must sit on validated custom schemas
- custom autograd must declare functional-input requirements explicitly
- backend plugins are typed bundles of schemas and kernel registrations
- quantizer plugins are typed capability bundles above raw decode

The report also keeps unsupported cases machine-legible, including custom ops
that bypass declared-output posture and quantizer plugins that fail to declare
non-dense modes.

## Why This Matters

This report prevents two failure modes:

- treating the extensible registry as if it were a full user-facing plugin API
- inventing separate plugin vocabularies for ops, kernels, autograd, backends,
  and quantizers in downstream crates

The point of this issue is to publish one reusable extension-contract layer
that later distribution, export, and runtime loading work can extend honestly.
