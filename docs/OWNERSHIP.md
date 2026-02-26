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

- Generic UI primitives/components/rendering.
- Platform adapters and rendering infrastructure.

Must not own:

- OpenAgents product workflows.
- App-specific business logic from `apps/autopilot-desktop`.
- Dependencies on app crates.

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
