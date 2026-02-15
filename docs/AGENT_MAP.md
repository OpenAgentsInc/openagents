# Agent Map

This repo is designed for **progressive disclosure**: start with a small set of stable entry points, then follow links into deeper specs/runbooks as needed.

## Start Here (Always)

- `AGENTS.md`
  Non-negotiables + pointers to canonical docs.
- `docs/README.md`
  Docs index (what exists, where to look next).
- `docs/PROJECT_OVERVIEW.md`
  Active codebase map and ownership hints.
- `docs/GLOSSARY.md`
  Canonical terminology (use this when words conflict).
- `docs/ROADMAP.md`
  Current sequencing and priorities.

## Architecture + Contracts

- `docs/adr/INDEX.md`
  ADR list and the authority chain (what wins when docs disagree).
- `docs/execution/ARTIFACTS.md`
  Verified Patch Bundle artifact contract (`PR_SUMMARY.md`, `RECEIPT.json`).
- `docs/execution/REPLAY.md`
  REPLAY.jsonl contract (ToolCall/ToolResult/Verification/Session lifecycle).
- `docs/protocol/PROTOCOL_SURFACE.md`
  Protocol-level field semantics (ids, hashes, receipts, payment proofs).

## DSE / Compiler Layer

- `docs/dse/README.md`
  Canonical DSE docs referenced by ADRs.
- `packages/dse/README.md`
  Implementation-level overview and usage.

## Product Surfaces

- `apps/openagents.com/`
  **Incoming core web app** (Laravel 12 + Inertia + React). Plan: `docs/plans/active/laravel-rebuild.md`.
- `apps/web/README.md`
  Current web app dev/test/deploy entry point (Effuse/Cloudflare/Convex; legacy until cutover).
- `docs/autopilot/spec.md`
  Autopilot behavior spec (web surface).
- `apps/autopilot-worker/README.md` (if present)
  Worker surface entry point (tools/runtime).
- `apps/mobile/README.md`
  Mobile app entry point.
- `apps/desktop/README.md`
  Desktop surface entry point.

## Debugging + Verification

- `docs/autopilot/testing/PROD_E2E_TESTING.md`
  Production-safe E2E and request correlation workflow.
- `docs/autopilot/testing/TRACE_RETRIEVAL.md`
  Trace retrieval/debug workflow.
- `docs/audits/README.md`
  Known audits and recommended follow-up work.

