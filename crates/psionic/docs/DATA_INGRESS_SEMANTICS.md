# Data Ingress Semantics

> Status: canonical `PLIB-218` / `#3733` reference record, updated 2026-03-16
> after landing the first bounded data-ingress report in
> `crates/psionic/psionic-data/src/lib.rs`.

This document records the current bounded reusable data-ingress semantics
surface for Psionic.

## Canonical Runner

Run the data-ingress harness from the repo root:

```bash
scripts/release/check-psionic-data-ingress-semantics.sh
```

## What Landed

`psionic-data` now exposes:

- `DatasetSourceContract`
- `DatasetSamplerContract`
- `DatasetBatchSamplerContract`
- `HostDeviceStagingContract`
- `DataIngressContract`
- `DataIngressSemanticsReport`
- `builtin_data_ingress_semantics_report()`

## Current Honest Posture

Today Psionic has a first-class reusable local data-ingress surface, but it
does **not** claim distributed or sharded feed closure yet.

The bounded seeded surface now makes these seams explicit:

- map-style and iterable-streaming dataset access
- sequential and deterministic-shuffle sampling
- batch-sampler contracts above packing policy
- direct-host and pinned-prefetch host-to-device staging
- explicit refusal for weighted or round-robin sampler families

## Why This Matters

This report prevents two failure modes:

- re-implementing dataset, sampler, and staging glue in each training lane
- implying distributed feed semantics are done because local iteration and
  packing exist

The point of this issue is to make local data ingress a reusable library
contract that later distributed feed work can extend honestly.
