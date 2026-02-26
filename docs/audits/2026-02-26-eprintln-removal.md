# `eprintln!` Production Path Cleanup (Issue #2313)

Date: 2026-02-26  
Scope: `apps/`, `crates/wgpui/src`

## Summary

Removed unconditional `eprintln!` usage from production source paths and replaced it with structured `log` macros.

## Changes

- Converted iOS platform bridge output in `crates/wgpui/src/platform/ios.rs`:
  - setup lifecycle messages -> `log::debug!`
  - benchmark output in test lane -> `log::info!`
  - failure paths -> `log::error!`
- Verified no remaining `eprintln!` callsites under `apps/` or `crates/wgpui/src`.

## Logging Strategy (Brief)

- Use `log::debug!` for verbose lifecycle/progress diagnostics.
- Use `log::info!` for notable non-error operational milestones.
- Use `log::error!` for failure paths that should be visible in runtime logs.
- Avoid direct stderr/stdout macros in production code paths.
