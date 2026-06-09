# OpenAgents Invariants

This is the root invariant ledger for the rebuilt `openagents` Bun workspace.
More specific invariant ledgers apply inside imported apps and packages.

## Preserved Transcript Archive

- `docs/transcripts/` is retained historical material and must not be deleted,
  renamed, rewritten, or used as runtime private data.
- New refactor docs belong in `docs/refactor/`; do not mix migration planning
  into the transcript archive.

## Effect Workspace Boundary

- New production TypeScript code in this repo must use Bun and Effect.
- External boundaries must be modeled with typed data structures or Effect
  Schema. Do not add ad hoc keyword routing for user intent, CRM/database
  query routing, retrieval routing, or tool selection.
- Shared runtime contracts belong in `packages/*`. App-specific UI, Worker,
  CLI, or deployment composition belongs in `apps/*`.

## Product Surface Ownership

- `apps/openagents.com/` owns the `openagents.com` product surface and retains
  its local invariant ledger.
- `apps/forum/` owns forum-specific code and must mount under `/forum` when it
  is served by `openagents.com`.
- `apps/pylon/` owns contributor-node UX, CLI, local runtime orchestration, and
  contributor-facing payment evidence.
- `packages/probe/` owns Probe runtime code and evidence submission helpers.

## Authority Boundaries

- Public UI does not own settlement, payout, runtime promotion, or accepted
  outcome authority.
- Probe evidence does not authorize deployment, spend, provider mutation, or
  public claim promotion without a separate approved authority path.
- Pylon payment, assignment, and earning claims must remain receipt-backed and
  explicit about unsettled, rejected, unpaid, credited, and settled states.
- Secrets, wallet material, raw prompts, private repo content, provider
  payloads, and private customer data must not be committed or written into
  docs, tests, fixtures, logs, or public projections.
