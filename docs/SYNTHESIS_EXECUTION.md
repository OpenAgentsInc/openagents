# Synthesis: Implementation Status

This document is a lightweight pointer to **what is wired today** in the active TypeScript/Effect codebase.

Authority note:
- This file tracks *implementation status*, not architectural intent. ADRs still win for intent/invariants.

## Start Here

- Active repo map: `docs/PROJECT_OVERVIEW.md`
- Roadmap/priorities: `docs/ROADMAP.md`
- Autopilot behavior spec (web): `docs/autopilot/spec.md`

## Contracts (Accepted vs Wired)

Some contracts are **Accepted** by ADR, but may be partially implemented.

Canonical contracts:
- Verified Patch Bundle: `docs/plans/archived/adr-legacy-2026-02-21/ADR-0002-verified-patch-bundle.md`
- Replay format target: `docs/plans/archived/adr-legacy-2026-02-21/ADR-0003-replay-formats.md`
- Tool execution contract: `docs/plans/archived/adr-legacy-2026-02-21/ADR-0007-tool-execution-contract.md`

Canonical specs:
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
- `docs/protocol/PROTOCOL_SURFACE.md`

Implementation guidance:
- Treat code as the source of truth for what is currently wired.
- When wiring a previously "spec-only" contract, update the relevant docs/ADRs and add verification coverage.

