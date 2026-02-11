# OpenAgents Repository Overview

This document maps the active codebase after the Rust deprecation cleanup.

## Product Surface

- `apps/web/`  
  Main OpenAgents web product. Includes homepage chat panes, auth flows, deployment scripts, and docs specific to web operations.

- `apps/autopilot-worker/`  
  Worker-focused runtime pieces used by the web product and automation paths.

- `apps/expo/`  
  Mobile app surface.

## Shared Packages

- `packages/effuse/`  
  Effect-oriented runtime and orchestration utilities used across app surfaces.

- `packages/effuse-panes/`  
  Pane/window primitives and controllers.

- `packages/effuse-test/`  
  E2E and runtime test tooling.

- `packages/effuse-ui/` and `packages/effuse-flow/`  
  UI and flow helpers used by product surfaces.

- `packages/dse/` and `packages/hud/`  
  DSE visualization and HUD components.

## Docs and Operational Runbooks

- `docs/autopilot/`  
  Primary operational docs for the web product (production E2E, stream testing, trace retrieval, debugging).

- `docs/moltbook/` and `docs/MOLTBOOK.md`  
  Moltbook policy, strategy, and engagement guidance.

- `docs/openclaw/`, `docs/cloudflare/`, `docs/local/`  
  Deployment notes, integration plans, and local development workflows.

## Historical Code and Docs

Rust code and Rust-era docs were removed from this repo and archived to backroom:

- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/`
- `~/code/backroom/openagents-docs-rust-archive-2026-02-11/docs/`

If a legacy document references `crates/*`, `Cargo.toml`, `apps/api/`, or `apps/autopilot-desktop/`, treat it as historical unless it has been explicitly rewritten for the current web stack.
