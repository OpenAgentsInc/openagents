# Ownership Boundaries (MVP)

This document defines what each active surface in this pruned repo owns, and what it must not own.

## Purpose

- Keep the MVP codebase understandable.
- Prevent responsibility bleed between product code and reusable crates.
- Enforce stable architecture while we iterate quickly.

## Active Surfaces

## `apps/autopilot-desktop`

Owns:

- App wiring and product behavior.
- Pane orchestration, app-level event routing, and UX flows.
- Composition of reusable crates (`wgpui`, `nostr`, `spark`).

Must not own:

- Reusable protocol/wallet primitives that belong in crates.
- Generic UI toolkit primitives that belong in `wgpui`.

## `crates/wgpui`

Owns:

- Compatibility facade over split WGPUI crates.
- Product-agnostic UI APIs consumed by apps.

Must not own:

- OpenAgents product workflows.
- App-specific business logic from `apps/autopilot-desktop`.
- Dependencies on app crates.

## `crates/wgpui-core`

Owns:

- Core primitives (geometry/color/input/scene/curve).
- Product-agnostic types that render/components build on.

Must not own:

- GPU pipeline orchestration.
- Product/app behavior.

## `crates/wgpui-render`

Owns:

- GPU renderer implementation and SVG rasterization pipeline.
- Render metrics and texture preparation internals.

Must not own:

- Product/app behavior.
- Widget-level business workflows.

## `crates/openagents-ios-domain`

Owns:

- iOS app-domain mission/Codex data models.
- Filtering/severity semantics for mission event views.

Must not own:

- Rendering/GPU platform concerns.
- Shared WGPUI primitives.

## `crates/nostr/core`

Owns:

- Reusable Nostr identity + key derivation primitives.

Must not own:

- App UI logic.
- Spark wallet logic.

## `crates/spark`

Owns:

- Reusable Spark wallet primitives and API wrappers.

Must not own:

- App UI logic.
- Nostr pane/product orchestration logic.

## Dependency Rules

- `apps/*` may depend on crates.
- `crates/*` must not path-depend on `apps/*`.
- Reusable crates must not depend on each other through app layers.

## Guardrail

- Static boundary check: `scripts/lint/ownership-boundary-check.sh`
- This guard is run from `scripts/lint/clippy-regression-check.sh`.

## Review Checklist

- Does this change introduce product-specific behavior into reusable crates?
- Does this change add a crate-to-app dependency?
- Could this logic be moved one layer closer to its true owner?
