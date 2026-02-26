# Workspace Lint Adoption (Issue #2311)

Date: 2026-02-26  
Scope: `apps/autopilot-desktop`, `crates/wgpui`

## Summary

Enabled `lints.workspace = true` in both workspace crates so lint policy is inherited consistently instead of only being defined at workspace root.

## Changes

- Added workspace lint inheritance:
  - `apps/autopilot-desktop/Cargo.toml`
  - `crates/wgpui/Cargo.toml`
- Added explicit temporary lint expectations in `wgpui` crate root:
  - `clippy::unwrap_used`
  - `clippy::expect_used`
  - `clippy::panic`
- Added explicit temporary lint expectations in all example crate roots for demo/setup fail-fast paths:
  - `clippy::unwrap_used`
  - `clippy::expect_used`
  - `clippy::panic`
- Refreshed clippy warning baseline after policy activation:
  - `scripts/lint/clippy-baseline.toml`

## Why Baseline Increased

Before this change, workspace lint policy existed but was not inherited by all crates.
Once inheritance was enabled, the full workspace policy started applying to lib/test/example lanes, which surfaced previously hidden pedantic and deny-level findings.

## Temporary Exceptions (Explicit)

The temporary expectations are intentional and tracked debt, not silent ignores:

- Production crate root expectations are temporary scaffolding while line-by-line cleanup lands.
- Example lane expectations are limited to demo binaries where explicit fail-fast setup remains acceptable.

Cleanup follow-ups:

- `#2312` (argument-heavy helper refactors)
- `#2313` (`eprintln!` removal/gating)
- Further lint burn-down issues after this rollout.
