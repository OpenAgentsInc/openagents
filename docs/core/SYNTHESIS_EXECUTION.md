# Synthesis Execution Status

This file tracks current execution posture for the Rust-era system.

## Scope

- Current implementation truth: codebase + runtime behavior.
- Contract authority: `docs/execution/`, `docs/protocol/`.
- Architecture authority: `docs/core/ARCHITECTURE.md`.

## Canonical Contract Docs

- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
- `docs/protocol/PROTOCOL_SURFACE.md`

## Execution Guidance

1. Treat code as source of truth for what is currently wired.
2. Keep docs aligned to current Rust behavior and active strategy.
3. Archive stale/historical docs to backroom, not in `docs/`.
4. Require verification evidence (tests/build/runbooks) for architecture-affecting changes.
