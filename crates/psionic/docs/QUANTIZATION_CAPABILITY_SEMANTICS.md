# Quantization Capability Semantics

> Status: canonical `PLIB-215` / `#3730` reference record, updated 2026-03-16
> after landing the first bounded quantization capability report in
> `crates/psionic/psionic-core/src/lib.rs`.

This document records the current bounded quantization semantics surface for
Psionic above raw file-format decode.

## Canonical Runner

Run the quantization harness from the repo root:

```bash
scripts/release/check-psionic-quantization-capability-semantics.sh
```

## What Landed

`psionic-core` now exposes:

- `QuantizationCapabilityStage`
- `QuantizationCalibrationMode`
- `QuantizationGranularity`
- `QuantizationConfig`
- `QuantizationCapabilityCaseResult`
- `QuantizationCapabilitySemanticsReport`
- `builtin_quantization_capability_semantics_report()`

## Current Honest Posture

Today Psionic has a first-class quantization capability vocabulary, but it does
**not** claim blanket quantization closure across all flows or model families.

The bounded seeded surface now makes these seams explicit:

- PTQ support for symmetric int8 weights with calibration metadata
- QAT support for bounded observer-driven symmetric int8 flows
- quantized runtime execution semantics for seeded `ggml_q4_0` matmul above
  raw file-format decode
- compiler-lowering semantics that preserve quantized intent instead of hiding
  it inside one loader
- export-aware quantization intent on the meta surface before deployment
  artifact contracts land

The report also keeps unsupported paths machine-legible, including block-quant
QAT and broader activation-dtype closure for runtime execution.

## Why This Matters

This report prevents two failure modes:

- treating GGUF decode as if it were full quantization support
- smuggling PTQ, QAT, compiler, or export semantics through ad hoc loader
  behavior instead of one reusable capability surface

The point of this issue is to make quantization a library-owned semantics
program that later plugin, export, and deployment work can extend honestly.
