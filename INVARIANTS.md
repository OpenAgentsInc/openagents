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
- The `openagents.com` deploy topology guard must keep the main product,
  Worker, shared packages, and Foldkit runtime on the tracked Effect v4 line.
  A separate isolated app may carry an older third-party Effect dependency only
  when the guard names that exact package chain, documents the exception, and
  prevents it from becoming OpenAgents.com runtime, settlement, payout, Forum,
  Pylon assignment, or product-promise authority. The current isolated
  exception is `apps/nostr-relay` through `nostr-effect@0.0.12` only.

## No GitHub-Hosted CI / Cloud Actions

- Never add GitHub Actions workflows or any GitHub-hosted CI to this
  repository. `.github/workflows/` must contain no workflow files
  (no `on: push`, `on: schedule`, `on: pull_request`, or any other
  GitHub-runner automation).
- CI, scheduled jobs, freshness re-runs (e.g. study-packet restudy), and any
  recurring automation run on OpenAgents-owned infrastructure (our GCE / cloud
  runners and cron), not on GitHub-hosted compute.
- Rationale: keep build, test, scheduling, and automation on owned infra —
  consistent with the no-Expo/EAS-cloud mobile policy — and avoid handing repo
  automation, secrets, or scheduling to third-party GitHub-hosted runners.

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
- Pylon local supervised danger modes (Codex `danger-full-access`, Claude
  `bypassPermissions`) are explicit owner-local opt-ins only: local composer
  and authenticated local control sessions may honor the local dev overlay;
  every public command, assignment, labor, and provider path rejects them with
  a typed blocker, and the assignment-safe config loaders never read a
  permissive mode.
- Secrets, wallet material, raw prompts, private repo content, provider
  payloads, and private customer data must not be committed or written into
  docs, tests, fixtures, logs, or public projections.

## Verse World Projection

- Live Verse world work is moving to `apps/openagents-world/`, a Cloudflare
  Worker + Region Durable Object service written in TypeScript, Effect, and
  Effect Schema. Durable Objects are the coordination atoms for live presence,
  local interaction, interest-scoped fanout, hibernatable WebSockets, handshake
  buffering, sequence acknowledgements, TTL expiry, and per-region world state.
- `packages/world-contract/` owns public-safe world schemas and command/delta
  contracts. `packages/world-client/` owns the desktop/web client projection
  that mirrors snapshots and deltas into a read-only `WorldReadModel`.
- The world service and client projection do not own settlement, payout,
  training truth, product promises, receipt validation, accepted-work authority,
  wallet state, provider credentials, private prompts, private repo content, or
  customer-private data.
- Public world rows and deltas may expose only public-safe refs, labels,
  positions, timestamps, staleness metadata, movement caveats, moderation state,
  and dereferenceable proof URLs that are already safe for public OpenAgents
  surfaces.
- Browser/user commands may update only explicitly modeled interaction state,
  such as joining/leaving a region, bounded avatar pose, focus, local chat,
  emotes, and ephemeral intent. Service-only commands that create or mutate run,
  entity, edge, proof, settlement, event, cursor, bridge-health, or projection
  rows must require an allowlisted service identity.
- `/tassadar` authority remains the Worker/D1 public summary path until a later
  invariant change explicitly promotes a different authority. The Verse world
  service may enrich or animate the scene only from public refs or timestamped
  projection transitions.
- The deleted self-hosted world module is historical source material only. Do
  not reintroduce it for production world behavior; port useful schema or
  reducer ideas into the Cloudflare/Effect world service.

## Public Projection Staleness

- Every public projection in this workspace carries `generatedAt` (or an
  equivalent rebuild timestamp) plus a declared staleness contract, and either
  rebuilds on the state transitions that matter or composes live at read. A
  projection that cannot meet its declared staleness must say so in the
  payload rather than serve stale data as current.
- The `openagents.com` worker-surface contract vocabulary, the enumerated
  projection inventory, and the enforcing check tooling live in
  `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness
  Declaration") and `apps/openagents.com/scripts/check-zero-debt-architecture.mjs`
  (epic #4751).

## Product Promise Claims

- User-facing and agent-facing product claims belong in the product-promises
  system under `docs/promises/` before copy broadens beyond implementation
  notes.
- A product promise is green only when its evidence refs, authority boundary,
  projection safety, freshness, and copy gate are all satisfied for the exact
  claim being made.
- Planned, partial, stale, blocked, manually gated, or canary-only behavior
  must stay red, yellow, degraded, or explicitly scoped in public and
  agent-readable copy.
- Product promise mismatch reports from users and agents are Forum-first. The
  default public intake is the Product Promises Forum at
  `https://openagents.com/forum/f/product-promises`.
- GitHub issues may be opened only for concrete, reproducible bugs that
  satisfy the strict bug report template. Blank issues are disabled, and
  malformed, broad, or loose reports should be rejected by the issue form or
  moved back to the Forum rather than becoming normal product-promise intake.
- This initial promise system is documentation-backed. Runtime enforcement must
  be added before treating the registry as an automated product gate; until
  then, `docs/promises/checks-and-gates.md` is the model-boundary record.

## Commit Metadata Privacy

- Commit messages, commit trailers, and other committed metadata must not
  include individual people’s names unless the user explicitly requests a
  legally or historically required attribution.
- Prefer neutral product, team, source, operator, reporter, maintainer, or role
  wording in commits and committed process records.
