# Environment Registry Reference

> Status: canonical `#3578` environment-registry record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-environment-registry.sh`.

This document records the first Psionic-native environment registry and
composition layer.

## What Landed

The issue widened `psionic-environments` with:

- typed `EnvironmentInstallRequest` and `EnvironmentInstallRecord` for package
  install receipts
- digest-bound `EnvironmentPackagePin` aliases so train and eval code resolve
  immutable versions instead of free-form environment refs
- mixed-surface `EnvironmentCompositionGroup` and
  `EnvironmentCompositionMember` contracts for train, eval, and benchmark
  groups
- `EnvironmentRegistry` resolution over installed packages, dependencies, pin
  aliases, and group members
- `EnvironmentEvalParityReceipt` proving train and eval reuse the same pinned
  package and digest for shared group members

Kernel and Nexus still own canonical registry and package authority truth.
This issue lands the Psionic-side runtime and resolution contract that local
orchestrators can execute against without bespoke per-environment glue.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-environment-registry.sh
```

## Reference Flow

The current reference path proves a minimal but real mixed-environment setup:

1. install one train/eval environment package plus one benchmark package
2. pin the train/eval package alias to a specific immutable version and digest
3. define one mixed-surface group that reuses the same member across train and
   eval, while keeping a benchmark-only member separate
4. resolve the group for both train and eval surfaces
5. emit a parity receipt showing that shared members reuse the same package
   key and digest

## Pass Criteria

The registry layer is green only if all of the following are true:

- installs validate package contracts and dependency presence
- pin aliases hold an immutable package key plus stable package digest
- group resolution is machine-legible and surface-scoped
- benchmark-profile requirements are refused when the pinned package does not
  declare them
- train and eval parity is proved explicitly rather than inferred from logs

## Expected Signals

The current harness should prove:

- version pinning survives the presence of a newer installed package version
- benchmark-only members can live in the same group without train/eval
  bespoke code
- retiring a package makes future resolutions fail explicitly
- train and eval resolve the exact same package key and digest for shared
  members

## Current Limitations

This issue intentionally does not claim:

- persistent on-disk registry storage
- kernel-authority synchronization or package publication workflows
- eval-run execution itself beyond the package/group contract reuse
- sandbox pooling or training-stage orchestration built on top of the registry
