# OpenAgents: Agent Contract (READ THIS FIRST)

This file is a **map**, not a manual. Start here, then follow links for deeper specs/runbooks.

## Map

- Progressive disclosure entry points: `docs/AGENT_MAP.md`, `docs/README.md`
- Repo map / ownership: `docs/PROJECT_OVERVIEW.md`
- Priorities: `docs/ROADMAP.md`
- Vocabulary (terminology authority): `docs/GLOSSARY.md`
- Architecture decisions (invariants/contracts): `docs/adr/`

## Authority (Non-Negotiable)

1. If documentation conflicts with behavior: **CODE WINS**
2. If terminology conflicts across docs: **GLOSSARY WINS**
3. If architecture intent conflicts (invariants/interfaces/contracts): **ADRs WIN**
4. If implementation status conflicts across docs: prefer the active codebase + `docs/PROJECT_OVERVIEW.md`
5. If sequencing conflicts: **ROADMAP WINS**

## Engineering Invariants (Ship-Quality Rules)

- Verification first: do not claim success without running the relevant harness (lint/test/build/e2e as appropriate).
- No stubs: do not land TODO-only placeholders or mock implementations in production paths.
- Typed contracts: decision points become **Signatures**; tools have JSON schemas validated by the runtime.
- Everything is logged + replayable: deterministic hashes, receipts, and replay events are required.

Canonical specs:
- Execution artifacts: `docs/execution/ARTIFACTS.md`
- Replay format: `docs/execution/REPLAY.md`
- Protocol field semantics: `docs/protocol/PROTOCOL_SURFACE.md`

## Effect Best Practices (Non-Negotiable)

Always consult `effect-solutions` before writing Effect code:

1. `effect-solutions list`
2. `effect-solutions show <topic>...`
3. Reference implementations: `.reference/effect/` (run `effect-solutions setup` first)

For Effect packages (`packages/dse`, `packages/effuse`, `packages/effuse-test`): run `npm run effect:patch` after install for diagnostics.

## Git Hygiene (Non-Negotiable)

- Do not create `git worktree`s without explicit user approval.
- Do not use `git stash` without explicit user approval.
- If the worktree is dirty and you need a clean state, present options: commit, discard, or (with approval) temporary worktree.

## Where To Change Things

- **Core web app (incoming):** `apps/openagents.com/` — Laravel 12 + Inertia + React (TypeScript), target replacement for the current web stack. See `docs/plans/active/laravel-rebuild.md`.
- **Web product surface (current/legacy until cutover):** `apps/web/` — Effuse/Cloudflare/Convex stack.
- Autopilot worker surface: `apps/autopilot-worker/`
- Mobile surface: `apps/mobile/`
- Desktop surface: `apps/desktop/`
- Shared packages: `packages/*`
- Canonical docs/contracts: `docs/` (start with `docs/README.md`)

## Verification Entry Points

- **Laravel web (incoming core):** `apps/openagents.com/README.md` (or app root; see `docs/plans/active/laravel-rebuild.md`).
- Web (current): `apps/web/README.md`
- Autopilot worker: `apps/autopilot-worker/README.md`
- Mobile: `apps/mobile/README.md`
- Desktop: `apps/desktop/README.md`

Production debugging and request correlation:
- `docs/autopilot/testing/PROD_E2E_TESTING.md`

## Output Expectations (Agent Runs)

The canonical output of an autonomous run is the **Verified Patch Bundle**:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

See:
- `docs/adr/ADR-0002-verified-patch-bundle.md`
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`

