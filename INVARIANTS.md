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
- **Enforced** by `check:no-github-actions` (in `check:deploy`): it fails if any
  `.github/workflows/*.yml` exists. PR-evidence / autonomous-QA on a PR is **agent- or
  manually-triggered** — an agent runs `apps/qa-runner/src/pr-comment-run.ts` (or the
  `qa-runner` directly) and posts the verdict/trace/video comment itself (e.g. PR #6224) —
  never a `pull_request`/`push`/`schedule` workflow.

## Product Surface Ownership

- `apps/openagents.com/` owns the `openagents.com` product surface and retains
  its local invariant ledger.
- `apps/forum/` owns forum-specific code and must mount under `/forum` when it
  is served by `openagents.com`.
- `apps/forge/` owns the separate `forge.openagents.com` UI surface. It may
  consume shared `@openagentsinc/ui` primitives/tokens and Forge API contracts,
  but it does not own runtime promotion, settlement, payout, accepted-work
  authority, or the main `openagents.com` logged-in route tree.
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
  and authenticated local control sessions may honor the local dev overlay.
  Caller-owned Khala -> Pylon -> Codex coding delegation is also owner-local:
  when the caller is routed to their own linked Pylon, the local Codex executor
  uses the SDK equivalent of
  `--dangerously-bypass-approvals-and-sandbox` (`danger-full-access` plus
  approval policy `never`) so real GitHub/worktree operations can complete.
  Untrusted labor, provider, and public command paths still reject caller-
  supplied danger flags with a typed blocker, and assignment-safe config
  loaders never read a permissive mode from public wire/config.
- Real Khala fleet-run coding dispatch must use named, isolated account refs
  from the caller-owned Pylon account registry. Automatic real-work fanout must
  not route through the display/default Codex account or omit `--account-ref`;
  that would write rollouts under the operator's default `~/.codex` home.
  Local Pylon control sessions must also prefer healthy connected Codex account
  registry entries before falling back to the default Codex home. Provider
  usage exhaustion, rate limiting, and auth revocation are typed account-health
  failures; they must update the local health/quota ledger, surface as
  account-specific failure classes, and retry another healthy connected Codex
  account when one exists.
  Fleet-run supervisor ticks are serialized per Pylon/run handle so startup
  status reads and the background loop cannot over-dispatch past the target
  concurrency. Regression coverage lives in
  `clients/khala-code-desktop/tests/fleet-run-supervisor.test.ts` and
  `clients/khala-code-desktop/tests/khala-fleet-tools.test.ts`.
- Secrets, wallet material, raw prompts, private repo content, provider
  payloads, and private customer data must not be committed or written into
  docs, tests, fixtures, logs, or public projections.

## Background Agent Definition Tool Authority

- Harness-agnostic background agents are defined by
  `openagents.agent_definition.v1` in
  `packages/agent-runtime-schema`. The durable definition owns the standing
  workflow contract: name, goal, harness hint, lane, triggers, budget,
  escalation, source refs, and the explicit toolset.
- The harness field is never authority. Codex, Claude Code, Khala, hosted,
  custom, or fixture adapters may execute only after their local or cloud
  tool boundary compiles and enforces the definition's toolset.
- `decideAgentDefinitionToolAuthority` is the shared deny-by-default contract:
  explicit deny rules beat ask and allow rules, ask rules create an operator
  escalation record without authorizing execution, allow rules authorize only
  the matched tool ref, and unmatched tools are denied.
- `compileAgentDefinitionToolRuntimePolicy` materializes that contract as
  `openagents.agent_definition_tool_runtime_policy.v1` before a lane starts
  executing tools. Local Khala tool execution must enforce the compiled policy
  against both the tool name ref and authority ref before any tool body runs.
- Forge tenant git tokens for definition-backed work compile requested git
  scopes through the same policy boundary. `git:receive-pack`,
  `git:upload-pack`, and `git:admin` map to Forge git tool refs; denied scopes
  are rejected before token mint, and ask scopes create operator escalation
  instead of minting a token.
- Runtime runs may link back to `agentDefinitionId` as evidence that a run was
  definition-backed, but that link alone grants no tool, spend, dispatch,
  payout, settlement, public-claim, provider-account, or external-send
  authority.
- Any Worker, Pylon, desktop, or cloud-workroom executor that claims
  definition-backed tool enforcement must use this contract or a formally
  equivalent compiled policy at the execution boundary, with regression tests
  for deny precedence, ask escalation, allow, and default-deny behavior.
- Regression coverage starts in
  `packages/agent-runtime-schema/src/index.test.ts`,
  `packages/khala-tools/src/dispatcher.test.ts`, and
  `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.test.ts`.

## Connector Authority And Redaction

- Connector sidecars never own workspace, payment, email, membership,
  settlement, identity, or broad provider-account authority. The platform
  remains authoritative for those state changes; connectors may only emit
  source-verified, bounded events and execute explicitly modeled per-connector
  tools.
- Before any connector event reaches model context, session history, logs, or
  outbound provider mutation, provider credentials, authorization headers, raw
  webhook bodies, raw payloads, signatures, and webhook secrets must be
  excluded or redacted. Public or model-visible connector envelopes may carry
  only typed subjects, source refs, redacted refs, booleans, timestamps, and
  blocker/caveat refs.
- Outbound connector mutation must pass an app-owned idempotency gate before
  dispatch. Provider retry keys alone are not enough; the OpenAgents connector
  contract owns the dedupe key and the bounded receipt/projection.
- Generic provider tools are forbidden. Tool authority must name the connector,
  provider, subject kind, and operation, and it must stay bound to the verified
  event subject, such as one issue or one pull request.
- Regression coverage for the BF-6 connector gate lives in
  `packages/connector-sidecar/src/index.test.ts`, including denial cases for
  raw provider material in context/history/logs, missing app-owned idempotency,
  generic provider tools, and platform-authority widening.

## Cloudflare Verse World Service

- Live Verse world work belongs to `apps/openagents-world/`, a Cloudflare
  Worker + Region Durable Object service written in TypeScript, Effect, and
  Effect Schema. Durable Objects are the coordination atoms for live presence,
  local interaction, interest-scoped fanout, hibernatable WebSockets, handshake
  buffering, sequence acknowledgements, TTL expiry, and per-region world state.
- `packages/world-contract/` owns public-safe world schemas and command/delta
  contracts. `packages/world-client/` owns the desktop/web client projection
  that mirrors snapshots and deltas into a read-only `WorldReadModel`.
- Worker/D1 public product surfaces remain authoritative for public training
  truth, product promises, receipt-backed proof claims, settlement/payout
  projection, and Forum/product state. The Verse world service owns only
  public-safe presence, local interaction, interest-scoped fanout, diagnostic
  rows, and replayable projection rows derived from public source refs.
- The world service and client projection do not own settlement, payout,
  training truth, product promises, receipt validation, accepted-work authority,
  wallet state, provider credentials, private prompts, private repo content,
  private customer data, or unpublished provider payloads.
- Public world rows and deltas may expose only public-safe refs, labels,
  positions, timestamps, staleness metadata, movement caveats, moderation state,
  and dereferenceable proof URLs that are already safe for public OpenAgents
  surfaces.
- Browser/user commands may update only explicitly modeled interaction state,
  such as joining/leaving a region, bounded avatar pose, focus, local chat,
  emotes, and ephemeral intent. Service-only commands that create or mutate run,
  entity, edge, proof, settlement, event, cursor, bridge-health, or projection
  rows must require an allowlisted service identity.
- Actor command authority is modeled in
  `docs/game/2026-06-22-cloudflare-world-actor-command-authority-model.md` and
  enforced by `packages/world-contract` plus `apps/openagents-world` command
  tests. Counterexamples must become tests before broadening command authority.
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
