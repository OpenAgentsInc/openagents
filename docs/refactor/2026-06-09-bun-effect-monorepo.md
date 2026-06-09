# Bun Effect Monorepo Reset

Date: 2026-06-09

## Decision

Reset the `OpenAgentsInc/openagents` main repo as the Bun and Effect monorepo
for the active OpenAgents product stack.

The old Cargo/Tauri/pruned-MVP tree has been removed from the current working
tree. The retained historical material is `docs/transcripts/`. The Git history
remains the archive for deleted tracked files.

## Imported Sources

- `apps/openagents.com/` from the previous product-surface clone
  at `1637c91dde13`.
- `apps/pylon/` from `/Users/christopherdavid/work/pylon` at
  `cafce1ac73c6`.
- `packages/probe/` from `/Users/christopherdavid/work/probe` at
  `e0c78a764ba8`.

Only tracked `HEAD` snapshots were imported. Local caches, build products,
secrets, and the untracked Probe `docs/playground.md` file were excluded.

## Target Layout

- `apps/openagents.com/`: `openagents.com` product, Cloudflare Worker,
  Foldkit browser app, and existing product packages.
- `apps/forum/`: forum extraction target. The live `/forum` surface remains
  served by the `apps/openagents.com` Worker until its schema, auth, and
  payment boundaries are separated cleanly.
- `apps/pylon/`: contributor Pylon CLI/TUI and runtime packages.
- `packages/probe/`: Probe runtime and benchmark/agent execution packages.
- `docs/transcripts/`: preserved transcript archive.
- `docs/refactor/`: architecture plans, migration checkpoints, and cutover
  records.

## Workspace Plan

1. Keep the first commit as a mechanical reset plus source import.
2. Stabilize root `bun install`, root typecheck, and root test delegates.
3. Decide whether the nested product workspace should remain nested or be flattened
   into root `apps/*`, `workers/*`, and `packages/*` members.
4. Move Probe integration that was started in Pylon behind explicit package
   imports from `packages/probe`.
5. Build the forum as a separate Effect/Foldkit surface with a stable `/forum`
   base path and no hidden coupling to product page state.
6. Move cross-surface schemas into root `packages/*` only when two or more apps
   need the same contract.
7. Retire standalone repo release paths after this monorepo has equivalent
   install, test, deploy, and rollback evidence.

## Authority Notes

- `apps/openagents.com` keeps its imported invariant ledger.
- Forum code must not own settlement, payout, public claim, or runtime
  promotion authority.
- Probe remains evidence-producing runtime code. It does not authorize deploys,
  spend, provider mutation, or public claim promotion by itself.
- Pylon earning and assignment claims must stay receipt-backed and explicit
  about payment state.

## Immediate Verification

- `bun run --cwd apps/forum test`
- `bun run --cwd apps/forum typecheck`
- `bun run --cwd apps/pylon test`
- `bun run --cwd packages/probe test`

The full product check remains delegated to `apps/openagents.com` because it was
imported as its existing workspace in this reset commit.
