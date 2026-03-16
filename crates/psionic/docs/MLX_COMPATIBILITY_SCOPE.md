# Psionic MLX Compatibility Scope

> Status: canonical `PMLX-002` / `#3830` reference record, updated 2026-03-16
> after freezing the initial upstream MLX version window and claim-language
> contract in `crates/psionic/psionic-compat/src/lib.rs`.

This document records the current honest claim boundary for Psionic's MLX lane.

## Canonical Runner

Run the scope check from the repo root:

```bash
scripts/release/check-psionic-mlx-compatibility-scope.sh
```

## What Landed

`psionic-compat` now exposes:

- `MlxUpstreamVersionWindow`
- `MlxCompatibilityTerm`
- `MlxCompatibilityScopeReport`
- `builtin_mlx_compatibility_scope_report()`

The report freezes one bounded upstream oracle window and one explicit
claim-language split so MLX work does not drift into vague "MLX-like" or
"MLX-compatible" wording.

## Frozen Upstream Window

The current report freezes the initial bounded upstream oracle to:

- upstream repository: `ml-explore/mlx`
- inclusive release window: `v0.31.0` through `v0.31.1`
- informative audit checkout: `ea91bd02cf0671f3fe6ddaf746812c27bf05154e`
- informative audit describe string: `v0.31.1-7-gea91bd02`
- informative audit date: `2026-03-16`

That local checkout informed roadmap review, but it does not widen Psionic's
claim window beyond the bounded `v0.31.x` release family.

## Claim-Language Split

`MLX-class` means:

- a Rust-native Psionic-owned framework surface inside `crates/psionic/*`
- parity or adoption claims tied to the frozen upstream window above
- reuse of Psionic runtime, compiler, train, artifact, receipt, and refusal
  truth instead of bypassing them through wrappers or sidecars

`MLX-compatible` means:

- a later bounded facade, migration surface, naming shim, or import/export
  path above the native Psionic MLX-class substrate
- explicit supported, convertible, and unsupported behavior for the same
  frozen upstream window
- typed refusal or lossy-conversion truth instead of implied full upstream
  closure

## Forbidden Shortcuts

The report now makes these review rules explicit:

- do not call a wrapper around upstream MLX, Python, or FFI glue an
  `MLX-class` implementation
- do not use `MLX-compatible` wording without naming the bounded upstream
  window
- do not let a compatibility shell stand in for missing native array,
  transform, `nn`, export, distributed, or backend semantics
- do not imply tip-of-tree MLX coverage from one local checkout, demo, or
  notebook port
- do not imply `MLX-identical` behavior anywhere in roadmap or review language

## Why This Matters

This closes the first governance requirement in `ROADMAP_MLX.md`:

- one named upstream MLX version window is frozen
- compatibility language is explicit and bounded

The remaining Epic 0 work still needs:

- a parity harness entrypoint
- a supported, convertible, and unsupported matrix
