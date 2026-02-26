# Framework Lane Retirement Decision

Date: 2026-02-26  
Scope: `crates/wgpui`

## Decision

Retire the unused `app` / `element` / `window` framework lane from `wgpui` and keep a single active MVP path based on the retained component + scene stack.

## Why

- The lane was not consumed by `apps/autopilot-desktop`.
- Keeping two UI paradigms increased maintenance load with no product gain.
- Current MVP direction favors deletion and simplification over optional dormant lanes.

## Changes

- Removed `crates/wgpui/src/app/`.
- Removed `crates/wgpui/src/element/`.
- Removed `crates/wgpui/src/window/`.
- Removed `experimental-framework` feature from `crates/wgpui/Cargo.toml`.
- Removed framework-lane exports from `crates/wgpui/src/lib.rs`.
- Removed stale lane docs (`crates/wgpui/docs/phase1-foundation.md`) and updated `README`/docs index.

## Result

- One clear default UI model remains in the repository.
- No feature-flag indirection was added for dead code.
- `wgpui` and `autopilot-desktop` build checks remain the verification gate for this decision.
