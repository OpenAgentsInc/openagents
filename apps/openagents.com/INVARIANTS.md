# INVARIANTS

This is the invariant ledger for `openagents`.

## Login Surface

- The `/login` SPA page (`LoginRoute` / `loginRouter`, `apps/web/src/page/login.ts`)
  is a **branded launcher into the real OpenAuth flow only**: it links to
  `/login/github` (GitHub OAuth) and `/login/email` (OpenAuth `CodeProvider`
  one-time email code). It must never reintroduce a **simulated / in-app** auth
  flow (no client-side `SaveSession` / `SimulateAuthRequest` / fake session
  issuance). Those removed simulated-auth symbols remain banned by
  `scripts/check-zero-debt-architecture.mjs` ("deleted simulated login auth
  symbols").
- Login only authenticates. Authorization stays gated downstream
  (`authHasCoreTeamAccess` / `isAdmin` / onboarding in
  `apps/web/src/product-policy.ts`); email sign-in does not widen product access.
- Background context: `docs/auth/2026-06-16-login-and-auth-audit.md`.

## Clean Public URLs

- First-party product routes must not carry auth, connection, payment,
  checkout, runner, deployment, or account-result state in query parameters or
  URL fragments.
- OAuth/OIDC callback routes may receive provider-required parameters such as
  `code`, `state`, and `error`, but those parameters must be consumed at the
  callback boundary and followed by a clean redirect to a first-party route.
- Connection success/failure should be represented by server-side durable
  state, authenticated API re-fetches, cookies, session storage, local storage,
  or in-memory Foldkit state, not by public URLs like
  `/?github_write=connected`.
- Product routes that are directly visited with stale result query parameters
  must redirect to their clean canonical URL before rendering.
- Regression coverage for this policy lives in
  `workers/api/src/redirect-policy.test.ts`.

## Data Trace Correctness Gate

- Data-trace marketplace projections must keep submission, redaction,
  semantic planning, correctness, valuation, purchase, entitlement, payout
  contract, and settlement evidence as distinct refs.
- Valuation evidence must not advance a data trace to `valued` unless a
  public-safe correctness receipt is present. Correctness receipts are separate
  from valuation refs and settlement refs.
- General data-contribution correctness receipts must be minted by the
  verifier only after public-safe provenance refs, duplicate checks, and
  deterministic derived-trace replay agree with the claimed trace digest.
  Tampered, unprovenanced, or duplicate contributions must remain non-payable
  even if valuation, purchase, entitlement, payout-contract, or settlement refs
  are present.
- Studied-knowledge verification reports may provide correctness receipts for
  deterministic link/span replay. Non-deterministic remainder must surface as
  validator-review refs and must not pass the correctness gate until review
  produces a correctness receipt.
- Regression coverage for this policy lives in
  `workers/api/src/data-trace-marketplace-gate.test.ts` and
  `packages/probe/packages/runtime/tests/openagents-study-verification.test.ts`.

## Artanis Work Direction Labor Requests

- Artanis construction/data work-direction requests must stay operator-enabled
  by default-off configuration and must use the existing labor requester,
  NIP-LBR lifecycle, and labor escrow rail. Do not add a parallel settlement or
  payout workflow for these directions.
- Program-authorship requests must carry V1 construction verification commands
  and may release escrow only after the compiled module construction/replay
  verdict passes without real-bitcoin movement. Dataset-curation requests must
  carry V3 data-correctness verification commands and may release escrow only
  after the data contribution correctness gate passes.
- Contributor work-routing proposals must enter the requester surface through
  typed proposal records with selector refs and explicit direction kinds. Do
  not infer funded direction kinds from keywords in Forum post text.
- Regression coverage for this policy lives in
  `workers/api/src/artanis-work-directions.test.ts`.

## Tassadar Module Library Demand Ranking

- Demand-price signals, module-library ranking, and dedupe projections are
  read-only economic evidence. They must not mutate marketplace listings,
  request budgets, payout state, ranking state, or settlement state by
  themselves.
- Near-duplicate compiled-module entries must collapse by typed dedupe keys to
  a verified canonical entry. Unverified duplicates may be recorded as collapsed
  refs but must not displace the replay/composition/link-verified canonical
  module.
- Artanis requester surfaces may consume a demand-price signal only as a
  public-safe request budget/source-ref input and only through the existing
  operator-enabled work-direction gate. Do not infer demand or module direction
  from keywords in Forum text.
- Regression coverage for this policy lives in
  `workers/api/src/tassadar-module-library.test.ts` and
  `workers/api/src/tassadar-compiled-module-marketplace.test.ts`.

## Tassadar Adversarial Verification Market

- Adversarial-verification work requests are a typed Artanis direction, not a
  Forum keyword inference path. They must carry explicit module, corpus, spec,
  implementation, input, psionic evidence, and verifier command refs.
- A divergence claim becomes payable only when an independent validator device
  reproduces the exact claimed input and behavior divergence. Same-device,
  non-reproduced, non-divergent, digest-mismatched, or missing-near-miss-refusal
  claims must refund/reject and must not release escrow.
- Confirmed defects reuse the V1 compiled-module construction settlement path.
  Public code must not add a separate payout rail, bypass labor escrow, bypass
  the owner real-settlement gate, widen spend caps, or publish raw inputs,
  traces, private source material, wallet material, or payment material.
- Regression coverage for this policy lives in
  `workers/api/src/tassadar-adversarial-verification-market.test.ts` and
  `workers/api/src/artanis-work-directions.test.ts`.

## Tassadar Gradient Window Hybrid Ring

- Public learned-interface gradient windows are candidate updates only. A
  submission may enter a quarantine checkpoint, but it must not mutate a
  canonical checkpoint directly.
- Promotion requires explicit construction-substrate refs, verification-substrate
  refs, curated-data refs, quarantine receipts, deterministic recompute receipts,
  replicated matching update digests, Baseline-D-grade canary metrics, and a
  public promotion decision ref. Missing or failed recompute, replication,
  canary, or promotion evidence blocks canonical mutation.
- The compiled exact core remains frozen across the whole window. Any changed
  frozen-core digest, trainable compiled-core scope, direct compiled-core gradient
  target, missing frozen-core scope, or trace-not-in-forward-pass claim blocks
  the window.
- The gradient-window gate does not settle payouts, arm real Bitcoin, widen
  spend caps, publish raw gradients/traces/prompts/private data, or grant live
  public decentralized-training claims by itself.
- Regression coverage for this policy lives in
  `workers/api/src/tassadar-gradient-window-regime.test.ts`.

## Foldkit-Owned Browser Navigation

- Production browser app code must not call raw History APIs such as
  `window.history.pushState`, `window.history.replaceState`, `history.pushState`,
  or `history.replaceState`.
- App navigation must go through Foldkit navigation commands such as `pushUrl`,
  `replaceUrl`, or `load` so the router receives the URL change and can update
  the model.
- Test setup may use browser history APIs to arrange location state, but runtime
  app source must not.
- Regression coverage for this policy lives in
  `apps/web/src/navigation-policy.test.ts`, which is included in
  `bun run check:deploy`.

## Generated Icon Catalog

- Product UI icons must render through the generated Fireball Apps SDK icon
  catalog in `apps/web/src/icon.ts`.
- UI primitives must type icon props as `IconName`; browser app code must use
  `iconView` or `IconService` instead of ad hoc inline SVG, Unicode/text icon
  stand-ins, image icon URLs, icon fonts, lucide/react-icons, Iconify, or new
  icon dependencies.
- If the product surface needs a missing product icon, update the upstream Fireball catalog
  first, regenerate with `bun run sync:icons`, and keep the icon tests
  passing.
- Regression coverage for this policy lives in `apps/web/src/icon.test.ts` and
  `apps/web/src/icon-policy.test.ts`, both included in `bun run check:deploy`.

## Three-Effect Proof Replay Visual Ownership

- Training-run and proof-replay visual renderers are owned by
  `@openagentsinc/three-effect` and the visual taxonomy in
  `docs/launch/2026-06-17-tassadar-training-run-visual-language.md` plus the
  `/animations` route studies.
- `@openagentsinc/proof-replay` owns deterministic replay bundles, timeline
  clocks, source coverage, shipment gates, and render-plan data only. It must
  not become a parallel DOM, CSS, canvas, WebGL, actor, camera, zap, avatar, or
  particle renderer.
- `apps/web` and `apps/autopilot-desktop` may adapt public-safe replay bundles
  into `three-effect` visualization options and may render Foldkit controls,
  inspectors, event lists, transcripts, loading states, and accessibility
  mirrors. They must not add new app-local replay stage/world visuals.
- The existing `apps/web/src/scene/tassadarProofReplayElement.ts` is a
  temporary legacy bridge for the first public replay route. It may receive
  interaction, loading, source-inspection, accessibility, and migration glue
  fixes, but new replay visual language must first land in
  `/Users/christopherdavid/work/three-effect` and then be consumed here.
- Regression coverage starts in
  `apps/web/src/scene/tassadarProofReplayElement.test.ts` and
  `scripts/check-zero-debt-architecture.mjs` ("app-local proof replay visual
  renderers outside three-effect").

## Explicit Team Autopilot File Selection

- Team Autopilot command parsing is bounded to exact `@autopilot` command
  forms: leading command plus a following space, trailing command preceded by a
  space, or a standalone trimmed command line.
- Team Autopilot context files must be selected by explicit authorized file IDs
  from the request or stored message metadata.
- Do not infer selected files from prompt keywords such as "pdf", "file",
  "attachment", or similar wording.
- Prompt text may still become the Autopilot goal, but it must not decide which
  uploaded files are included in hidden dispatch context unless a typed
  semantic selector is explicitly modeled and tested.
- Regression coverage for this policy lives in
  `workers/api/src/team-autopilot.test.ts`.

## Prefilled Workspace Public And Private Access

- Prefilled workspace rows are explicitly labeled by `accessMode`:
  `public_safe` rows keep the original public-source-only onboarding invariant,
  while `private_team` rows may hold private project material and must be
  denied by default.
- The first-signed-in-holder claim path applies only to `public_safe` invited
  workspaces. It must not claim, bind, or reveal `private_team` workspace
  material.
- `private_team` holder reads and engagement writes require an authenticated
  user with active membership in the workspace's private team, and active
  project validity when a private project ref is present. Operator reads may
  inspect the operator view, but public/holder projections must not expose raw
  invite tokens, holder refs, team-private refs, or operator-only metadata.
- Regression coverage for this policy lives in
  `workers/api/src/prefilled-workspace-routes.test.ts`.

## Agent Runtime Kernel Event Projection

- Worker ingestion of Agent Runtime Kernel events must schema-decode
  `AgentRuntimeEvent` at the boundary and append by `runId` plus exact
  sequence. Duplicate or non-append events must fail closed before
  persistence.
- Ingestion must not parse adapter-specific transcript dialects. Fixture,
  Codex, Claude, OpenCode, hosted, SHC, Hermes-reserved, and native loops must
  project into the shared kernel contract before the Worker sees them.
- Runtime events are evidence only. Ingestion and public projection must not
  grant accepted-work authority, payout authority, public-claim authority,
  provider-account mutation, or spend authority.
- Public runtime projections must rebuild from public-visible kernel events,
  carry `generatedAt` and the declared staleness contract, and expose the
  storage/projection visibility split.
- Public runtime projections must reject raw prompts, raw logs, provider
  payloads, secrets, private paths, wallet material, and customer/private data
  before persistence or projection.
- Regression coverage for this policy lives in
  `workers/api/src/agent-runtime-kernel.test.ts`.

## Pack A Autopilot Runtime Supervision

- Pack A task and schedule supervision events must schema-decode
  `PackARuntimeEvent` at the boundary and append by subject ref plus exact
  sequence. Duplicate event ids, sequence gaps, duplicate fired schedule
  occurrences, and duplicate delivered task notifications must fail closed
  before persistence.
- Task, schedule, and continuation status must be replayable from typed events.
  Derived web, Pylon, agent/API, companion, and public-safe views are
  projections, not separate authority stores.
- Pack A task events may record public-safe task refs, run refs, schedule refs,
  output refs, artifact refs, usage refs, blocker refs, notification refs,
  cursors, truncation/redaction state, and terminal state. They must not store
  raw prompts, raw shell logs, provider payloads, secrets, private repo paths,
  wallet/payment material, or customer/private data in public events.
- Schedule fired, skipped, failed, cancelled, and continuation-queued
  occurrences must emit receipt refs and must not create payout, accepted-work,
  settlement, or public-claim authority by themselves.
- Regression coverage for this policy lives in
  `workers/api/src/autopilot-pack-a-runtime-supervision.test.ts`.

## Pack A Supervision Contract

- Background, scheduled, companion, Pylon, and agent/API Autopilot surfaces
  must consume the shared Pack A supervision contract for attention events,
  companion projections, permission decisions, and non-interactive structured
  output before adding new approval or notification behavior.
- Attention events must dedupe/fold by typed keys, clear through typed
  invalidation or resolution events, and record delivery-failure receipts
  without failing or retrying the underlying run indefinitely.
- Companion projections are read/action projections only. They may expose
  status, waiting decision refs, action refs, public-safe artifact refs,
  budget refs, caveats, `generatedAt`, and the staleness contract, but they do
  not grant deploy, spend, provider mutation, payout, settlement, or accepted
  work authority.
- Permission decisions must fail closed when prompts are unavailable and no
  remote approval resolver exists. Deny rules and hard safety checks beat saved
  allow rules, and background approvals become typed waiting states instead of
  hidden prompts.
- Non-interactive, JSON, CI, and headless modes must return schema-validated
  public-safe envelopes with stable exit codes, blocker refs, receipt refs,
  generatedAt, and caveats rather than color-only or TUI-only status.
- Regression coverage for this policy starts in
  `workers/api/src/autopilot-pack-a-supervision.test.ts`.

## Pack A Ledger And Settlement Evidence

- Pack A artifacts, lifecycle receipts, usage budget events, team spend joins,
  buyer-payment receipts, conversion refs, escrow holds, acceptance records,
  and settlement receipts must remain distinct typed records. A payment receipt
  cannot satisfy delivery, review, accepted-work, or settled-bitcoin
  requirements, and accepted work cannot imply payout settlement by itself.
- Public and agent-readable Ledger projections may expose refs, summaries,
  caveats, `generatedAt`, and the declared staleness contract only. They must
  reject raw prompts, raw shell logs, private repo data, provider payloads,
  wallet material, payment preimages or hashes, payout targets, local paths,
  and secrets before projection.
- Unknown pricing is an explicit caveated state, never zero cost. Own-Pylon
  free-lane work may report zero credit debit only while still preserving
  local/provider usage refs where available.
- Team spend-to-evidence joins must link ledger entries to mission refs,
  artifact refs, and receipt refs without widening artifact visibility or
  leaking provider/customer/private material.
- Labor-lane payout visibility is a release gate. P4/P6/P7 live paid-labor
  claims require every payout-ladder rung used by that job to be
  dereferenceable for recipient, public receipt, and auditor surfaces, or to
  return a typed-absent reason.
- Regression coverage for this policy starts in
  `workers/api/src/autopilot-pack-a-ledger.test.ts`.

## Autopilot Aperture Mission, Scope, And Writeback Contract

- Every agent/API, web, terminal, Forum, autonomic, or workroom Autopilot
  entry point that declares a per-mission data scope must keep repo refs,
  bounded path prefixes, and tool refs explicit in the typed work request.
  The declared repo scope must cover every task repository before a request
  can be accepted.
- Mission records and work orders must converge on a single canonical
  `mission_work_order.*` linkage ref. Briefings, decision actions, artifact
  refs, receipts, placement refs, data-scope refs, and continuation refs hang
  off that shared key rather than creating orphan mission-only or work-order
  only paths.
- Placement explanations are projections over repo placement records. They
  carry `generatedAt` and a declared staleness contract, redact non-public
  repo refs outside operator-safe contexts, and never grant runtime, spend,
  provider mutation, payout, settlement, accepted-work, or public-claim
  authority by themselves.
- Work-order writeback to PR drafts must route through artifact records and
  explicit GitHub writeback authority receipts. Missing approval, missing
  grants, expired grants, unusable connections, unsupported repositories, and
  missing authority receipts become typed blocked artifacts or validation
  errors; human/maintainer merge authority is not delegated to the market or
  agent runtime.
- Regression coverage starts in
  `workers/api/src/autopilot-aperture-contracts.test.ts` and
  `workers/api/src/autopilot-work-request.test.ts`.

## Pack C Repo Scope, Delivery, And Evidence

- Pack C repository/worktree identity snapshots are evidence records only.
  They may carry repository refs, host, owner/name, visibility, trust tier,
  default branch, pinned commit refs, remote digest refs, data-scope refs,
  workspace refs, worktree refs, branch refs, base/head refs, cleanliness,
  sandbox profile refs, retention refs, caveats, freshness metadata, and typed
  blockers. They do not grant writeback, merge, acceptance, payout,
  settlement, provider mutation, or public-claim authority.
- Public and agent-readable repo/worktree identity projections must reject
  private remotes, private repo content, raw prompts, raw shell material, local
  filesystem paths, credentials, provider payloads, wallet/payment material,
  and customer-private data before projection. Branch refs must be parseable
  safe Git refs and must not contain shell fragments, path traversal,
  lockfile suffixes, or ambiguous Git ref syntax.
- Pack C change captures may expose change refs, repository/worktree refs,
  base/head refs, file summary refs, patch digest refs, verification refs,
  diagnostic refs, review caveat refs, authority receipt refs, file counts,
  visibility, public-safety state, freshness metadata, and typed blockers.
  They must not expose raw patches, raw file contents, raw shell logs, private
  repo data, local filesystem paths, provider payloads, credentials,
  wallet/payment material, customer-private data, or raw prompts. Missing
  verification, missing patch digest, missing writeback authority, stale or
  blocked worktree identity, and unsafe public visibility must become typed
  blockers before a change capture can be treated as review-ready.
- Pack C workspace/file/shell evidence projections may expose workspace refs,
  sandbox profile refs, expected sandbox refs, operation kind, command intent
  refs, allowed command refs, allowed path refs, touched path refs, approval
  refs, timeout refs, cancellation refs, redaction class, redaction receipt
  refs, and typed blockers only. Out-of-scope paths, missing approval,
  disallowed command intent, sandbox mismatch, timeout, cancellation, and
  missing public redaction must become typed blockers. Public and
  agent-readable workspace evidence must reject raw shell logs, raw commands,
  raw prompts, local filesystem paths, private repo content, provider payloads,
  credentials, wallet/payment material, and customer-private data before
  projection.
- Pack C delivery readiness projections may expose delivery refs,
  repository/worktree identity refs, change capture refs, verification refs,
  GitHub writeback authority refs, review refs, human-merge caveat refs,
  delivery receipt refs, market and agent delivery refs, acceptance receipt
  refs, settlement receipt refs, public-safety state, freshness metadata,
  caveat refs, and typed blockers only. Missing change capture, missing
  writeback authority, missing verification, missing review refs, missing
  human-merge caveats, stale or blocked repository/worktree identity, stale or
  blocked change captures, stale projection freshness, and unsafe public
  visibility must become typed blockers. Market and agent delivery refs are
  evidence only and must not satisfy maintainer merge, acceptance, settlement,
  payout, or public-claim authority. Public and agent-readable delivery
  readiness projections must reject raw patches, raw file contents, raw shell
  logs, raw commands, raw prompts, private repo data, local filesystem paths,
  provider payloads, credentials, wallet/payment material, and
  customer-private data before projection.
- Regression coverage for Pack C repository/worktree identity snapshots lives
  in `workers/api/src/pack-c-repo-worktree-identity.test.ts`.
- Regression coverage for Pack C change capture and diff review artifacts
  lives in `workers/api/src/pack-c-change-capture.test.ts`.
- Regression coverage for Pack C workspace/file/shell authority evidence
  lives in `workers/api/src/pack-c-workspace-authority.test.ts`.
- Regression coverage for Pack C delivery readiness receipts lives in
  `workers/api/src/pack-c-delivery-readiness.test.ts`.

## Autopilot Bridge API Parity And Work-Creation Contract

- Every MVP browser/workroom Autopilot capability must have a registered-agent
  API peer in the checked-in parity matrix, or carry an explicit waiver with an
  issue ref and reason ref. A new user-facing route without an API peer or
  waiver is a contract failure, not a documentation gap.
- The Bridge parity projection is public-safe evidence only. It carries
  `generatedAt`, the shared staleness contract, web/API/proof/test refs, and
  waiver refs; it must not grant runtime, spend, provider mutation, payout,
  settlement, accepted-work, or public-claim authority by itself.
- Forum `request-coding` and work-request interactions may spawn Autopilot
  coding orders only as ref-only links between the registered agent, Forum
  thread/action, mission, work order, budget/payment mode, and lifecycle
  receipts. Raw prompts, private repo material, provider payloads, wallet
  material, payment secrets, customer data, and raw runner logs must not cross
  the Forum boundary.
- Autonomic `request_coding_work` ticks are proposal records gated by
  operator enablement, per-tick budget, funded payment authority, validator
  re-execution, and repo-authority review. The autonomic may propose a work
  order draft and reserve intent; it must not self-execute, self-accept, or
  bypass human review where repo authority is involved.
- Regression coverage starts in
  `workers/api/src/autopilot-bridge-contracts.test.ts`.

## Autopilot Gate Proof Authority

- MVP and Pack A proof issues may close only from a Gate decision record that
  names the exact claim, issue refs, source commit refs, required receipt
  kinds, smoke receipt authority refs, live evidence refs, missing evidence,
  and any accepted deferrals.
- Smoke receipt authority records are evidence-only records backed by
  `smoke_passed` or `smoke_failed` Pack A receipts plus artifact and verifier
  refs. They never grant runtime, spend, provider mutation, payout,
  settlement, accepted-work, or public-claim authority by themselves.
- #4768 M10 and #4772 M14 require typed Pack A lifecycle receipts, smoke
  authority, required child-issue status, and live evidence refs before Gate
  may mark them ready. Missing live two-account rotation, non-Codex run,
  overnight unattended proof, or usage-budget evidence must remain a typed
  blocker/deferred boundary, not prose acceptance.
- #4786 and #4813 parent closeout must preserve the exact open tail. Post-MVP
  market issues, W3 evaluation work, and credentialed live legs may be
  explicitly deferred only when the parent decision records the issue refs and
  prevents broad MVP/public-readiness claims from depending on them.
- #4749 W3 remains a separate research/evaluation track. It must not be used
  as MVP readiness evidence, and it stays blocked until the four-baseline
  report and hypothesis verdict refs exist.
- Regression coverage starts in
  `workers/api/src/autopilot-gate-proof-authority.test.ts`.

## Typed Email Side Effects

- Production email sends must pass through `EmailService`; route handlers and
  product services must not call Resend, Gmail, or another mail provider
  directly.
- Every external email send must carry a typed email kind, an idempotency key,
  source-authority metadata, rendered text and HTML, and a durable
  `email_messages` record before provider delivery is attempted.
- Provider delivery attempts must be recorded in `email_deliveries` with a
  classified, length-limited error summary. Do not store raw provider payloads,
  provider secrets, or unbounded diagnostics in delivery records, logs, source
  exports, issue comments, or docs.
- Human-owned Gmail draft tooling remains local/operator-owned unless OpenAgents product surface
  gains an explicit provider-account product surface. The Worker may record
  Gmail draft identifiers through the email ledger, but it must not shell out
  to `gws` or store Gmail OAuth tokens as a shortcut.
- WorkOS or other auth-code email flows are not product email sends and should
  stay behind their own auth service boundary if they are added later.
- Regression coverage for this policy starts in `workers/api/src/email.test.ts`
  and the D1 backing store starts in
  `workers/api/migrations/0026_email_ledger.sql`.

## Canonical Token Usage Ledger

- Cross-system token usage accounting must persist through
  `token_usage_events` with a stable event id and idempotency key before Stats
  dashboards, issue comments, or leaderboards treat it as durable usage.
- Token usage events may store producer/source route, safe actor/team/account
  refs, anonymized source refs, run/session/task/repository refs, provider,
  model, backend profile, bucketed token counts, usage truth, cost, currency,
  and leaderboard/privacy flags.
- Token usage events must not store raw prompts, completions, provider payloads,
  API keys, bearer/callback/OAuth material, tool args, raw source, private repo
  paths, local filesystem paths, or customer/private material. Unsafe fields
  must be rejected before persistence, not hidden after schema decode.
- Leaderboard privacy flags are accounting policy. Opted-out events remain in
  global aggregate totals but must be excluded or anonymized by leaderboard
  projections.
- Regression coverage for this policy lives in
  `workers/api/src/token-usage-ledger.test.ts` and
  `workers/api/src/token-usage-ledger-routes.test.ts`; the D1 backing store
  starts in `workers/api/migrations/0137_token_usage_events.sql`.

## Blueprint Program Run Evidence Authority

- Probe-submitted Blueprint Program Run records are evidence only. They must
  not authorize deploys, emails, spend, source mutation, direct business
  mutation, public claim promotion, or provider-account side effects.
- Probe-submitted Blueprint Action Submission proposals are review records, not
  executor calls. They must be stored as pending approval with direct execution
  disabled until a separate reviewed executor path records approval and
  execution receipts.
- Probe-submitted Blueprint Signature Contribution and Developer Package
  Contribution records are release-gate evidence, not self-promoting runtime
  authority. Candidate runtime use is dogfood-scoped only, and production
  runtime eligibility requires a promoted ref with approved review, target refs,
  release gate refs, fixture refs, retained failure refs, no rejection, no
  runtime authority, no self-promotion, and no production authority embedded in
  the contribution.
- Program Run evidence intake must reject raw prompts, callback URLs or tokens,
  provider payloads, private file content, private repo refs, wallet material,
  customer private data, provider secrets, raw run logs, and source archives
  before schema stripping can hide unknown fields.
- Action Submission proposal intake must reject Probe-local sandbox effects,
  direct Program Run execution attempts, model-confidence bypasses, completed
  execution claims, raw emails, payment material, callback material, provider
  payloads, private source material, wallet material, and customer private data.
- Contribution intake must reject self-promotion, runtime authority, raw
  prompts, source archives, runner logs, provider material, private repo refs,
  callback material, wallet or payment secrets, customer private data, raw
  timestamps, and secret-shaped refs before schema stripping can hide unknown
  fields.
- Accepted Program Runs may appear in operator-safe Blueprint registry
  projections only as refs and safe detail fields. Raw typed output and
  metadata remain repository-private unless a future projection explicitly
  models and tests their redaction boundary.
- Regression coverage for this policy lives in
  `workers/api/src/blueprint-routes.test.ts` and
  `workers/api/src/blueprint-probe-contribution-routes.test.ts` and
  `workers/api/src/blueprint/repositories/action-submissions.test.ts` and
  `workers/api/src/blueprint/repositories/probe-contributions.test.ts` and
  `workers/api/src/blueprint/repositories/program-runs.test.ts`.

## Pylon GEPA Metric-Call Assignment Claims

- Probe GEPA metric-call assignments are benchmark work-slice evidence. They
  must not imply runtime promotion, public benchmark leaderboard claims, wallet
  spend, payout dispatch, or settled payout unless a separate approved
  settlement path records explicit settlement evidence.
- Every GEPA metric-call assignment must carry an explicit payment mode:
  `unpaid_smoke`, `operator_credit`, `payable_pending_settlement`,
  `settled_bitcoin`, or `rejected_no_pay`.
- Accepted work and settled payout are separate claims. Accepted work requires
  submitted artifact refs, proof bundle refs, closeout refs, verifier result
  refs, and resource usage refs. Payable work additionally requires payment
  receipt refs. Settled bitcoin additionally requires `settled_bitcoin` mode,
  payment receipt refs, and settlement receipt refs.
- Probe GEPA settlement readiness is a separate batch-level accounting gate.
  `unpaid_smoke` batches may finish with no payment or settlement refs and no
  payout claim. `operator_credit` and `payable_pending_settlement` require
  accepted closeout refs, resource refs, proof refs, verifier refs, batch
  operator-accounting refs, and payment or credit receipt refs. `settled_bitcoin`
  additionally requires settlement receipt refs before any public settlement
  claim.
- Public-safe assignment, progress, closeout, coordinator-import, and settlement
  refs must reject private data, provider secrets, raw runner logs, wallet
  material, payment preimages or hashes, payout targets, private repo refs, and
  raw timestamps.
- Regression coverage for this policy lives in
  `workers/api/src/pylon-gepa-metric-call-assignments.test.ts` and
  `workers/api/src/probe-gepa-settlement-readiness.test.ts`.

## Public Pylon Earning Counter Gate

- Public Pylon stats must expose a deterministic earning launch gate before
  any public surface may present broad Pylon earning copy.
- The public earning gate is blocked unless fresh online, wallet-ready, and
  assignment-ready counters are all nonzero for the current public cohort.
- Stale heartbeats must drop online, wallet-ready, assignment-ready, and
  sellable counters back toward zero. A Pylon seen in the last 24 hours is not
  necessarily online now.
- Wallet-ready means receive/readiness evidence only. It must not imply send
  authority, outbound liquidity, accepted work, payout dispatch, or settlement.
- Assignment-ready requires public compute readiness evidence in addition to
  wallet readiness. It must not imply assignment acceptance, accepted work,
  payout dispatch, or settlement.
- Zero or unavailable counters must expose blocker refs and blocked public
  claim refs that dashboards can render without inferring from raw numbers.
- Regression coverage for this policy lives in
  `workers/api/src/public-pylon-stats.test.ts`,
  `workers/api/src/artanis-public-report.test.ts`,
  `apps/web/src/page/loggedOut/page/login.scene.test.ts`, and
  `apps/web/src/docs-blog-route.test.ts`.

## Receipt-Backed Public Pylon Paid-Work Totals

- Public Pylon accepted-work sats must be derived from public Nexus/Pylon
  settlement receipts that prove real bitcoin movement and settled public
  projection state.
- Simulation receipts, payment-only receipts, rejected reconciliation events,
  missing settlement events, missing accepted-work refs, unsupported amount
  denominations, and private payment material must not count toward public
  paid-work totals.
- Duplicate receipt retries for the same payout intent must count at most once.
- Receipt-ledger unavailable is distinct from zero settled receipts. When the
  settlement receipt store is unavailable, accepted-work totals must be `null`
  and the settlement gate state must be `unavailable`. When the store is
  available but no qualifying settled receipts exist, totals may be zero and
  the gate remains blocked.
- Public Artanis and Pylon surfaces that display accepted-work bitcoin totals
  must expose exact public receipt refs or an explicit blocked/unavailable
  settlement-gate state.
- Legacy aggregate sats without public settlement receipt refs must not be
  upgraded into accepted-work public totals.
- Regression coverage for this policy lives in
  `workers/api/src/public-pylon-stats.test.ts`,
  `workers/api/src/artanis-public-report.test.ts`,
  `workers/api/src/artanis-nexus-pylon-adapters.test.ts`,
  `apps/web/src/page/loggedOut/page/login.scene.test.ts`, and
  `apps/web/src/docs-blog-route.test.ts`.

## Pylon Install-To-Bitcoin Launch Smoke

- The broad "install Pylon and earn bitcoin" launch claim requires one retained
  install-to-bitcoin smoke bundle before public launch copy may present the flow
  as live.
- The bundle must include public-safe refs for install, registration,
  heartbeat, MDK wallet readiness, payout readiness, assignment lease,
  accepted-work closeout, payment, settlement, and public projection.
- CI no-spend mode and sandbox fake-payment mode may prove wiring and retained
  evidence shape, but they must not allow live wallet spend or settled-bitcoin
  claims.
- Live small-sats mode requires an explicit spend cap, operator approval,
  original funded MDK wallet-home send-readiness, non-stale assignment lease,
  payout readiness, payment receipt refs, settlement receipt refs, and public
  projection refs.
- Retained smoke evidence must reject raw invoices, payment hashes, preimages,
  mnemonics, wallet paths, raw payout targets, provider secrets, private data,
  raw logs, and raw timestamps.
- Regression coverage for this policy lives in
  `workers/api/src/pylon-install-to-bitcoin-smoke.test.ts` and the checklist
  script lives at `scripts/pylon-install-to-bitcoin-smoke.mjs`.

## MDK Agent-Wallet Send Readiness

- Any Pylon, Forum, or Site flow that may call
  `@moneydevkit/agent-wallet send` must pass the shared MDK send-readiness
  preflight before issuing the send command.
- Positive wallet balance is not send-ready evidence. Receive readiness is not
  send readiness.
- Until MDK documents a repair or restore procedure that preserves outbound
  capacity, mnemonic-only restore is not accepted as send-ready evidence. Live
  sends must use an explicitly original funded wallet home or remain blocked.
- MDK send failures that mention insufficient outbound capacity must normalize
  to a stable wallet-readiness blocker instead of a generic provider failure.
- Public and operator docs must keep original-wallet-home, mnemonic restore,
  balance, receive readiness, send readiness, payout dispatch, and settlement
  as separate states.
- Regression coverage for this policy lives in
  `workers/api/src/treasury-payment-mdk-agent-wallet-adapter.test.ts` and
  `workers/api/src/mdk-agent-wallet-smoke-fixture.test.ts`.

## Forum Tip Payment Truth

- Ordinary Forum tips are content rewards, not accepted-work payouts.
- A Forum post is tip-eligible only when the post author has a public-safe
  ready recipient-wallet projection. Posts without ready recipient wallets must
  not receive tip challenges.
- Public Forum paid totals, creator earnings rows, and receipt wording may
  count confirmed payer-side payment events as `paid` evidence only.
- Public Forum settled totals and leaderboards may count only payment events
  whose public projection carries `recipient_wallet_direct` settlement
  authority.
- Pending, previewed, failed, refunded, reversed, staged, sandbox, demo, or
  unconfirmed payment evidence must not be shown as paid tip value.
- Recipient self-claims and settlement-claim rows are optional auxiliary
  evidence for legacy/audit compatibility. They must not be required before a
  confirmed MDK Forum reward is shown as paid, and they must not convert a
  hosted payer-side payment into settled creator spendable value.
- Confirmed ordinary Forum tips may be described as paid creator tip value, but
  must not be described as accepted-work settlement, provider payout evidence,
  Treasury accepted-work authority, or proof that unrelated Pylon/Site payout
  gates are green.
- Regression coverage for this policy lives in
  `workers/api/src/forum/tip-settlement.test.ts`,
  `workers/api/src/forum-routes.test.ts`, and
  `workers/api/src/forum/paid-actions.test.ts`.

## Labor Escrow Credit Ledger

- Labor escrow is a held claim on the existing 1:1 buffer-backed
  `agent_balances` ledger, not a parallel ledger and not external money.
- `balance_msat` remains the total backed claim. `held_msat` is the
  non-sweepable reserved portion, and available balance is
  `balance_msat - held_msat`.
- Reserve may only move available requester balance into held state and must
  fail closed when available balance is insufficient. Held labor funds must not
  be spent through tips or swept to a wallet.
- Release requires public-safe acceptance evidence from the requester or a
  validator policy. Workers and providers cannot self-release escrow.
- Forum work-request acceptance is requester-authenticated and single-winner:
  only the original requester actor may accept a quote, at most one quote may
  be accepted for a work request, and the accepted quote amount must not exceed
  the request budget.
- Artanis labor requests are disabled by default. When enabled by explicit
  operator configuration, the tick action may only propose ref-only requests
  that pass schema validation, the per-tick labor budget gate, and the seeded
  balance ceiling. Delivered Artanis-requested work releases escrow only on a
  passing validator re-execution; failing validator verdicts refund escrow.
- Artanis studying contribution requests are data-direction labor, not compiled
  module construction or settled-bitcoin evidence. Requests and deliveries carry
  study-packet, graph, contribution, and S3 verification refs only; delivered
  work may reach `accepted`/`settled` in the Forum work-request lifecycle only
  after the S3 studied-knowledge correctness report passes. Rejected or
  validator-review-required S3 verdicts must refund escrow and must not record
  accepted or settled lifecycle states.
- Release credits the provider balance and debits the requester held claim
  exactly once. Refund releases the hold without debiting the requester.
  Release-after-refund, refund-after-release, double-release, and
  double-refund must not move balances.
- Reserve, release, and refund each require public-safe receipt rows carrying
  refs and amounts only. Escrowed or credited amounts are not settled bitcoin
  until the later payout path records settlement evidence.
- Regression coverage for this policy lives in
  `workers/api/src/artanis-labor-requester.test.ts`,
  `workers/api/src/artanis-studying-labor.test.ts`,
  `workers/api/src/forum-routes.test.ts`,
  `workers/api/src/labor-live-rehearsal.test.ts`,
  `workers/api/src/labor-escrow.test.ts`,
  `workers/api/src/payments-ledger.test.ts`,
  `workers/api/src/tips-sweep.test.ts`, and
  `workers/api/src/tip-ladder.test.ts`.

## MDK Payout Mode Declaration

- Pylon, Site, Forum, and Artanis public surfaces must declare the active MDK
  payout mode before any payout claim: `hosted_mdk_direct_payout`,
  `local_mdk_agent_wallet_bridge`, or `disabled`.
- Hosted MDK direct payout claims require both explicit programmatic-payout
  enablement and verified funded-key evidence. Hosted sandbox evidence must
  remain separate from live payout authority.
- When hosted direct programmatic payouts are disabled, successful Pylon
  settlement evidence may only be claimed through the local MDK agent-wallet
  bridge, and public surfaces must expose a stable hosted-direct blocker ref.
- Local bridge claims require send-readiness evidence, original funded wallet
  home evidence, live authority refs, and checked payment-material redaction.
- Public release and dashboard projections must not collapse hosted direct
  payout, local bridge payout, disabled payout, dispatch acceptance, and
  terminal settlement into one generic "MDK works" state.
- Regression coverage for this policy lives in
  `workers/api/src/mdk-payout-mode-gate.test.ts`,
  `workers/api/src/site-payment-manifest.test.ts`,
  `workers/api/src/mdk-agent-wallet-smoke-fixture.test.ts`,
  `workers/api/src/pylon-v02-openagents-release-gate.test.ts`,
  `workers/api/src/artanis-public-report.test.ts`, and
  `apps/web/src/docs-blog-route.test.ts`.

## Site Referral Bitcoin Withdrawal Gate

- Site referral capture is attribution evidence only. Raw signup attribution,
  referral cookies, claimed users, linked order refs, or credits must not create
  Bitcoin payout eligibility by themselves.
- Reward eligibility requires attribution plus a paid-activity workflow ref and
  no active policy blocker.
- Self-referral, duplicate-account, dispute, cap, chargeback/refund, clawback,
  reversal, held-review, and operator-review policy blockers must block or
  adjust payout eligibility before any public reward claim can advance.
- Credits and account balances are not Bitcoin liabilities unless a separate
  receipt-backed payout path records settlement evidence.
- Bitcoin stream, withdrawal, or settled-reward copy is blocked until public
  settlement receipt refs exist and the reward gate is not policy-blocked.
- Public referral reward projections must reject raw signup, customer, payment,
  wallet, payout, provider, secret, and timestamp material before rendering.
- Regression coverage for this policy lives in
  `workers/api/src/site-referral-reward-gate.test.ts`,
  `workers/api/src/site-referral-inspection.test.ts`,
  `workers/api/src/site-referral-workflow-events.test.ts`,
  `workers/api/src/site-referral-policy.test.ts`, and
  `workers/api/src/site-referral-attribution-consumption.test.ts`.

## Credit<->Bitcoin Asset Boundary (live guard)

- The credit<->Bitcoin asset-boundary invariant is enforced by the SHARED guard
  `workers/api/src/asset-bitcoin-boundary.ts` (`validateAssetBoundary`), not by
  scattered inline checks. The invariant: Bitcoin revenue may create a
  withdrawable Bitcoin revenue share; credit/USD (Stripe-credit) revenue creates
  a credit revenue share only and may NOT create a withdrawable Bitcoin
  liability; free/promotional spend creates no withdrawable Bitcoin share at all.
- This guard is wired onto the LIVE value-movement paths (RL-3, #5460), so the
  invariant is enforced at runtime, not just documented:
  - the RL-1 referral payout dispatch (`site-referral-payout-dispatch.ts`) fails
    closed before calling any payout adapter for non-Bitcoin revenue;
  - the RL-1 referral eligibility feed (`site-referral-payout-feed.ts`) records a
    revenue-matched revshare and never a credit-funded Bitcoin eligibility;
  - the RL-2 firm-up Bitcoin settlement decision
    (`firmup-bitcoin-settlement.ts` / `-routes.ts`) refuses a credit/USD/free-
    funded escrow before any money moves; and
  - the read-only commerce revenue-share projection
    (`site-commerce-revenue-share.ts`) delegates to the same guard.
- A boundary denial is public-safe and reason-qualified. Regression coverage:
  `workers/api/src/asset-bitcoin-boundary.test.ts`,
  `workers/api/src/site-referral-payout-wire.test.ts`,
  `workers/api/src/firmup-bitcoin-settlement.test.ts`, and
  `workers/api/src/firmup-bitcoin-settlement-routes.test.ts`.

## Referral Payout Settlement Adapter (owner-armed) + Cross-Category Accrual

- The referral payout dispatcher (`site-referral-payout-dispatch.ts`) invokes the
  production settlement adapter `workers/api/src/site-referral-payout-adapter.ts`
  (`makeSiteReferralPayoutAdapter`) to record a `settled` payout. The adapter
  wraps the hosted-MDK programmatic-payout rail and returns a sha256-redacted
  `receipt.site_referral_payout.hosted_mdk.<hash>` ref — never raw payment
  material (preimage/hash/invoice) and never a fabricated receipt (a receipt
  exists only after the rail confirms a non-FAILED payout). It refuses a
  non-reusable (single-use bolt11) destination so an idempotent retry cannot
  double-pay.
- The first real referral payout stays OWNER-ARMED (#5511/#5512). Two
  independent gates keep the production wiring inert: the injected readiness gate
  (`hostedMdkDirectPayoutDisabledGate` -> `livePayoutClaimAllowed: false`) refuses
  before the dispatcher reaches the adapter, AND the adapter is wired with a null
  payout client + a null-returning destination resolver, so it FAILS CLOSED
  (throws a tagged `SiteReferralPayoutAdapterError`, no money moves, NO settled
  state recorded) even if reached. Arming requires the owner to enable a live
  payout mode AND configure a funded client + a registered referrer destination.
  Do NOT relax either gate or flip `referral.refer_once_earn_forever.v1` /
  `sites.referral_bitcoin_stream.v1` green without an owner-signed, receipt-first
  upgrade per `proof.claim_upgrade_receipts.v1`.
- Refer-once-earn-forever is permanent and cross-category: the referrer<->referee
  binding is the consume-once attribution spine (`user_referral_attributions` /
  `agent_referral_attributions` joined to `site_referral_sources`), which names
  the referrer for that account regardless of purchase category. The category-
  agnostic accrual primitive `workers/api/src/referral-cross-category-accrual.ts`
  (`accrueCrossCategoryReferral`) is the single entry every category routes
  through to feed the ONE RL-1 payout ledger. It is usage-funded only (a real
  metered paid event; never on signups; a zero/below-1-sat-cut event accrues
  nothing), idempotent per `(category, eventId)`, and enforces the same shared
  asset boundary (Bitcoin->Bitcoin, credit/USD->credit, never a credit-funded
  Bitcoin liability). It records eligibility only — it moves no money.
- Regression coverage: `workers/api/src/site-referral-payout-adapter.test.ts` and
  `workers/api/src/referral-cross-category-accrual.test.ts`.
- USD->msat inference-credit bridge (#5497): a card (Stripe) USD purchase may be
  converted into an inference-spendable msat `agent_balances` credit
  (`usd_credit_grant` pay-in) via `POST /api/billing/inference-credit`, but the
  granted msat is tagged USD-origin in `agent_balances.usd_credit_msat`. That
  USD-origin balance is inference-spendable (the gateway gate + metering hook
  read `balance_msat`/`availableMsat`) but is NOT Bitcoin-withdrawable: the
  Lightning sweep (`tips-sweep.ts`, the live Bitcoin-withdrawal path) subtracts
  `usd_credit_msat` from the sweepable amount, so a card dollar can never leave
  as real Bitcoin. The bridge debits the USD `billing_ledger_entries` and grants
  the equivalent msat atomically (one D1 batch), idempotent per grant ref, and
  bounded by the available USD balance; the conversion is the single-source rate
  in `workers/api/src/inference/usd-msat-conversion.ts`. Regression coverage:
  `workers/api/src/inference/usd-credit-bridge.test.ts`.

## Provider Capacity Marketplace Gate

- ChatGPT/Codex account connection is not subscription-account resale
  authorization and must not be described as a live subscription-capacity
  marketplace by itself.
- Provider capacity monetization claims are provider-specific. ChatGPT/Codex is
  the first modeled provider; unsupported prepaid providers must remain planned or blocked
  until provider schemas, secret handling, route policy, metering, pricing, ToS
  boundaries, dispatch, assignment receipts, and settlement receipts exist for
  that provider.
- Provider tokens, raw quota payloads, subscription cookies, provider-account
  grants, raw metering, raw pricing, payment material, wallet material, customer
  data, and timestamps must not enter public refs.
- Pricing must distinguish agentic work or accepted outcomes from API-inference
  gateway resale. Converting a consumer subscription login into resale remains
  blocked; API-inference gateway resale is allowed only through an explicit
  policy path such as this gate, with tests.
- The authorizing policy required by the previous clause is
  `workers/api/src/inference-resale-authorization.ts`
  (`authorizeInferenceMonetization`, tests in
  `workers/api/src/inference-resale-authorization.test.ts`). API-inference
  gateway resale (Model 2, the cost-plus-10% gateway on OpenAgents' OWN
  API-key/commercial accounts) is authorized only when the full ref chain is
  present — provider grant, route policy, metering receipt, pricing policy,
  ToS boundary, dispatch, assignment receipt, and settlement receipt — and is
  refused on a subscription-auth account. `subscription_capacity_resale` is
  blocked unconditionally (non-waivable). This authorizes the mechanism only;
  it does not relax the public-copy gate in the clauses below.
- This no-resale gate is now enforced as a LIVE guard, not just a tested
  primitive (RL-3, #5460): the firm-up Bitcoin settlement decision
  (`firmup-bitcoin-settlement.ts`) calls `authorizeInferenceMonetization` and
  fails closed before any money moves. Firm-up is agent labor (`agentic_work`,
  allowed); a `subscription_capacity_resale` is refused unconditionally. The
  no-resale rule stays scoped to consumer SUBSCRIPTION accounts only —
  API-inference gateway resale on an API-key account is NOT over-blocked.
  Coverage: `workers/api/src/firmup-bitcoin-settlement.test.ts`.
- Assignment dispatch, assignment receipt, and Bitcoin settlement are separate
  states. Assignment evidence does not imply paid settlement.
- Public capacity marketplace or Bitcoin monetization copy remains blocked
  until the specific provider has safe grant refs, route policy refs, metering
  receipt refs, dispatch refs, pricing policy refs, ToS boundary refs,
  assignment receipt refs, and settlement receipt refs.
- Provider connector dashboards must expose the per-provider ladder explicitly:
  `unsupported`, `configured`, `healthy`, `assignable`, `payable`, and
  `settled`.
- A provider must not be listed as sellable capacity until typed account schema
  refs, secret-ref policy refs, connector health refs, quota evidence refs, and
  assignment-mode/policy refs are present for that provider.
- Provider selection must use the typed provider union, not generic provider
  string routing.
- Anthropic (`anthropic_claude`) and Google Gemini (`google_gemini`) provider
  peers connect by API-key BYOK only (`authMode: 'api_key'`), per the dated
  ToS-compliance review in
  `docs/autopilot-coder/2026-06-11-provider-peer-tos-compliance-review.md`
  (monorepo `docs/` root). Subscription-account connect shapes
  (Claude.ai/Pro/Max login or OAuth-token capture, Google account OAuth /
  Code Assist / AI Pro/Ultra) are forbidden by those providers' terms and
  must not be added without a new dated review superseding that document.
- Connected provider API keys are user-scoped: lease selection uses
  provider-tagged candidates from the requesting user's own connected
  accounts and never pools credentials across customers. Raw key material
  lives only in the auth KV under `provider-auth:<providerAccountRef>`;
  durable rows, events, grants, and projections carry secret refs only.
- Pack B credential-boundary projections may join provider accounts, auth
  grants, active lease refs, artifact refs, and receipt refs only as refs.
  Revoked grants, expired grants, disconnected accounts, reauth-required
  health, deleted accounts, and missing credential refs must produce typed
  blocker refs plus credential-cache invalidation refs before dependent work
  can claim lease authority. Raw API keys, OAuth material, provider payloads,
  private repo data, raw prompts, and shell logs must be rejected from those
  joins before projection.
- Pack B settings/configuration decisions must resolve through explicit
  precedence (`default`, `environment`, `organization`, `team`, `repository`,
  `user`, `device`, `runtime`) into immutable effective config refs. Provider,
  budget, approval, telemetry, retention, and routing decisions must fail as
  typed blockers when required settings are missing or invalid. Projections may
  expose config refs, caveat refs, source layers, and value tags only; raw
  environment values, API keys, OAuth material, private repo data, raw prompts,
  shell logs, and provider payloads must be rejected before projection.
- Pack B provider-peer security reviews must cite ToS review refs, credential
  boundary refs, threat-model refs, telemetry/privacy refs, retention-policy
  refs, redaction fixture refs, and revocation/stale-lease fixture refs before
  broad provider-peer claims can close. High-risk provider-account flows must
  also carry approval, denial, rollback, incident-boundary, and debug-boundary
  refs. Scoped exceptions may keep a narrow existing slice alive, but must
  preserve visible blocker refs and cannot broaden provider-peer readiness.
- Pack B provider-account telemetry projections may expose refs, counters,
  durations, statuses, provider ids, provider-account classes, aggregates,
  caveat refs, and freshness metadata only. Account-health, rate-limit,
  low-credit, cooldown, reset-hint, and reconnect telemetry must cite redaction
  fixture refs or produce typed blockers. `local_only` projections are ref-only
  outside the local/debug boundary, `off` projections remain disabled, and raw
  prompts, transcripts, shell output, private repo data, raw provider responses,
  provider credentials, OAuth material, wallet/payment material, and customer
  data must be rejected before telemetry projection.
- Pack B provider-account retention policies must declare retention class,
  deletion behavior, and projection invalidation behavior for credentials,
  account leases, account-health telemetry, provider-routing decisions, policy
  snapshots, reconnect state, debug/support records, artifacts, and receipts.
  Credential revocation or account deletion must invalidate dependent leases,
  emit typed dependent blockers and cache invalidation refs, and produce
  reconnect action refs where applicable. Tombstones, deletion receipts,
  retained audit refs, artifacts, and receipt refs must remain ref-only and
  must reject raw credentials, OAuth material, raw prompts, transcripts, shell
  output, private repo data, raw provider responses, wallet/payment material,
  customer-private data, and local paths before projection.
- Pack B managed-policy snapshots are minimal evidence records, not a broad
  enterprise admin product. Provider/account and team-budget decisions must
  resolve organization, team, repository, user, device/local, provider, budget,
  retention, and telemetry policy refs into a stable `effectivePolicyRef`.
  Denials for provider allowlists, approved-user gates, budget limits,
  retention blocks, stale policy, or unknown policy must be typed refs visible
  to web and agent/API surfaces without exposing raw policy internals, raw
  credentials, OAuth material, raw prompts, transcripts, shell output, private
  repo data, raw provider responses, wallet/payment material, customer-private
  data, or local paths. Policy caveats and allow/deny refs may attach to runs,
  leases, work orders, and receipts, but do not override the credential,
  security-review, telemetry, retention, or ToS gates.
- Regression coverage for the API-key connect boundary lives in
  `workers/api/src/provider-account-api-key.test.ts` and
  `workers/api/src/provider-account-lease-policy.test.ts`.
- Regression coverage for the Pack B credential-boundary projection lives in
  `workers/api/src/provider-account-credential-boundary.test.ts`.
- Regression coverage for Pack B effective config snapshots lives in
  `workers/api/src/provider-account-effective-config.test.ts`.
- Regression coverage for Pack B provider-peer security review gates lives in
  `workers/api/src/provider-account-security-review.test.ts`.
- Regression coverage for Pack B provider-account telemetry/privacy fixtures
  lives in `workers/api/src/provider-account-telemetry-privacy.test.ts`.
- Regression coverage for Pack B provider-account retention/deletion policy
  lives in `workers/api/src/provider-account-retention-policy.test.ts`.
- Regression coverage for Pack B managed-policy snapshots lives in
  `workers/api/src/provider-account-managed-policy.test.ts`.
- Regression coverage for this policy lives in
  `workers/api/src/provider-capacity-marketplace-gate.test.ts`.

## Open Labor Market Pylon Gates

- A GitHub issue may be routed through either an in-house Autopilot work order
  or an open-market work request, not both at once. Backlog faucet filings must
  use ref-only objective, repository, verification command, budget, and
  deadline fields; issue bodies, raw prompts, private repo material, provider
  payloads, wallet/payment material, local paths, secrets, and customer data
  must not enter the market filing or lifecycle comments.
- Open-market listing comments are mirrors over Forum/NIP-LBR records. They
  cannot grant acceptance, settlement, payout, merge, writeback, or public
  claim authority by themselves.
- Backlog faucet records reach `approved_for_publication` only through the
  typed operator approval transition carrying an `operator.*` ref and an
  integer spend cap covering the filing budget; `published` additionally
  requires a relay-accepted publish receipt with a real job event id. The
  generic market-state transition cannot reach either gated state, the
  dry-run path never publishes or escrows, and the `delivered → accepted`
  transition requires a validator verdict ref on top of the receipt ref.
- Spare-capacity provider mode is default-off. A Pylon may serve other
  people's jobs only after explicit owner consent, a public provider ref,
  pricing policy, min/max sats policy, declared capability refs, own-work
  preemption, payout/settlement readiness, and earnings visibility refs exist.
  Owned work preempts market serving.
- Lane C fanout is opt-in and public-tier only. Owned capacity wins when it is
  available; market fanout requires customer opt-in, public or public-beta
  privacy tier, mission/work-order unification, USD-to-sats settlement
  readiness, market inventory, artifact authority, validator policy, budget
  cap compliance, and a provider trust tier at the public floor.
- P1/P5/P6/P7 live paid-labor claims remain blocked until the corresponding
  real Forum/relay/quote/acceptance/execution/validator/release/settlement
  receipts are public and dereferenceable.
- Regression coverage for this policy lives in
  `workers/api/src/backlog-faucet.test.ts`,
  `workers/api/src/market-provider-policy.test.ts`, and
  `workers/api/src/lane-c-fanout-policy.test.ts`.

## Data Trace Marketplace Gate

- Local trace submission is marketplace evidence only. It must not imply a
  sale, data revenue, payout, entitlement, or settlement.
- Local trace/data revenue claims must remain blocked until a public-safe gate
  records trace submission, redaction, semantic planner or structured query
  planner, valuation, purchase receipt, buyer entitlement, payout contract, and
  settlement receipt refs.
- Raw traces, prompts, private repo or source content, provider payloads,
  customer material, wallet/payment material, payout targets, secrets, and raw
  timestamps must not enter public data-market refs.
- Data-market lookup/routing must use a typed semantic selector,
  cosine/embedding search, or structured query planner. Keyword-route fixtures
  may exist only as explicit denial tests.
- Valuation is not payout. Purchase is not settlement. Entitlement is not
  settlement. Public data-revenue copy requires a settled public-safe sale
  smoke with receipt refs, and those caveats must remain visible in public-safe
  gate projections.
- Regression coverage for this policy lives in
  `workers/api/src/data-trace-marketplace-gate.test.ts`.

## Signature Marketplace Revenue Gate

- Signature package validation is read-only evidence. It must not imply package
  install, runtime activation, marketplace listing, payment mutation, payout, or
  settlement.
- Candidate acceptance is not runtime activation. Runtime activation requires a
  separate future activation path with explicit authority and tests.
- Signature/plugin usage revenue copy is blocked until public-safe refs exist
  for package validation, package refs, program signature refs, usage event
  refs, usage idempotency refs, exact usage subject refs, attribution, pricing
  policy, revenue projection, gross revenue, payout eligibility, contributor
  payable amount, fork policy, license policy, dispute policy, refund policy,
  revenue-share split policy, and settlement receipts.
- A public-safe usage event may project pending revenue only after exact
  metering, attribution, pricing, revenue projection, and gross revenue evidence
  exist. That projection must not allow payout or settlement claims until payout
  eligibility, contributor payable amount, policy refs, and settlement receipt
  refs exist.
- Usage meters must bind to exact package, version, route or usage subject,
  usage event, and idempotency refs. Aggregate or inferred usage must not create
  payout eligibility.
- Private package source, raw prompts, provider payloads, raw usage/metering,
  customer data, wallet or payment material, payout targets, secrets, and raw
  timestamps must not enter public signature marketplace refs.
- Regression coverage for this policy lives in
  `workers/api/src/signature-marketplace-revenue-gate.test.ts` and
  `workers/api/src/signature-package-validation.test.ts`.

## Forum Tip Wallet Onboarding Gate

- Self-serve Forum tipping copy must remain gated until recipient wallet
  receive readiness, payer wallet configuration, payer funding evidence, payer
  send readiness, spend-cap checks, and a guarded signet or approved
  live-small-sats smoke are all visible through public-safe launch or product
  projections.
- Recipient readiness and payer readiness are separate. A recipient
  `ready`/receive-ready projection must not imply payer wallet setup, payer
  balance, send authority, payment dispatch, or settlement.
- Payer configured, funded, and send-ready states are separate. Positive
  balance or receive capability must not be upgraded into send readiness.
- Forum post/tip surfaces must expose missing recipient readiness,
  receive-ready recipient readiness, paid-pending-settlement, and settled
  states without collapsing payer-side payment evidence into creator spendable
  settlement.
- Public Forum wallet onboarding projections must reject raw wallet paths,
  balances, invoices, payment hashes, preimages, mnemonics, provider material,
  payout targets, bearer tokens, private customer data, and raw timestamps.
- Regression coverage for this policy lives in
  `workers/api/src/forum/payer-wallet-readiness.test.ts`,
  `workers/api/src/forum/recipient-wallet-readiness.test.ts`,
  `workers/api/src/forum/tip-smoke.test.ts`,
  `workers/api/src/forum/tip-settlement.test.ts`,
  `workers/api/src/forum/launch-gates.test.ts`, and
  `workers/api/src/forum-routes.test.ts`.

## Forum Tip Paid-Versus-Settled Claims

- Forum `paid` means payer-side content-reward payment evidence only. It must
  not imply creator spendable balance, recipient wallet settlement,
  accepted-work payout evidence, or payout dispatch.
- `totalPaidSats` and `totalSettledSats` are separate public totals. Paid sats
  may count confirmed payer-side payment events. Settled sats may count only
  receipts whose recipient settlement refs are present and whose settlement
  projection is `settled`.
- Forum receipt pages, post `tipStats`, creator earnings, reconciliation views,
  and leaderboards must label paid and settled totals separately.
- Refunds, reversals, failed payments, unverified evidence, and
  payment-required states must not contribute to paid or settled totals.
- Ordinary Forum tips must never become accepted-work payout evidence.
- Regression coverage for this policy lives in
  `workers/api/src/forum/tip-settlement.test.ts`,
  `workers/api/src/forum-routes.test.ts`,
  `apps/web/src/forum-route.test.ts`, and
  `apps/web/src/page/forum-tip-ui.test.ts`.

## Agent Claim Promotional Reward Ledger

- The X verification-tweet reward is a promotional claim incentive, not Forum
  tipping, Pylon accepted work, accepted-outcome payout, creator spendable
  settlement, or proof that an agent earned bitcoin.
- Reward amount is fixed at 1000 sats for
  `campaign.agent_claim.x_tweet_1000_sats.v1`.
- One active reward may exist per campaign/X account, per campaign owner, and
  per campaign agent claim unless a future operator policy explicitly models a
  broader allowance.
- Public reward receipts may expose only receipt ref, campaign ref, claim refs,
  owner ref, X account ref, tweet ref, state, fixed amount, destination kind,
  redacted destination ref, payout intent ref, dispatch attempt ref, settlement
  ref, caveat refs, and policy refs.
- Public reward receipts must reject raw X OAuth tokens, raw email addresses,
  raw payout destinations, raw invoices, payment hashes, preimages, wallet
  state or balances, mnemonics, provider payloads, raw fraud signals, IP/device
  fingerprints, raw timestamps, and bearer tokens.
- Public copy must distinguish approved, payout-intent-created, dispatched, and
  settled states. `settled` cannot be projected without a settlement ref.
- Regression coverage for this policy lives in
  `workers/api/src/agent-claim-reward-ledger.test.ts`.

## Debt Receipt Hygiene Settlement

- Codebase hygiene work is payable only as a funded debt receipt: a public-safe
  source ref, baseline metric, target metric, touched scope, budget cap, stop
  condition, verifier, accepted-work evidence, measured hygiene delta, and
  settlement approval.
- Discovery is inventory, not spend. A worker, churn probe, or agent may
  propose debt, but a distinct owner, allocator, reviewer, or market policy must
  convert it into a funded receipt before payout eligibility exists.
- Workers must not receive spend authority, settlement authority, deployment
  authority, or authority to mint payable follow-up debt from their own work.
- Merged PRs, review approvals, and verifier passes are accepted-work evidence
  only. Without an owner-funded receipt or approved batch plus settlement
  authority, accepted hygiene work is recognition/credit-class work, not a
  pending Bitcoin payout. `payable_pending_settlement` requires settlement
  approval or escrow/payout processing evidence. The lane state `settled`
  requires settlement receipt refs; Bitcoin-specific payment-mode projections
  may spell that terminal state as `settled_bitcoin` only when they also prove
  real Bitcoin movement.
- Payment follows verified delta, not churn: behavior or benchmark parity must
  be green, the named hygiene metric must improve, and no equal-or-worse debt
  may be introduced elsewhere in the scoped receipt.
- For hygiene work that depends on tests or typechecks, "green" must be
  dereferenceable as an OpenAgents-owned runner check-run or an independent
  verifier replay before it can support payout. GitHub-hosted Actions are not
  an allowed trust anchor for this lane. Local worker assertions are useful
  progress notes, but they are not settlement evidence.
- Duplicate/novelty is enforced by typed fingerprint keys, not loose ref
  matching: `DebtReceiptKey = sha256(debtReceiptRef | repoBaselineRef |
scopeDigest | objectiveDigest)` and `PatchNoveltyKey = sha256(DebtReceiptKey |
normalizedPatchDigest | behaviorReceiptDigest)`. Exactly one accepted
  settlement is allowed per `DebtReceiptKey`, then it retires; a near-duplicate
  patch carrying an already-retired `DebtReceiptKey` is a duplicate replay and
  is not payable.
- When a debt receipt requires studied-codebase evidence, the cited study
  packet, studied-knowledge graph, and studied-knowledge verification refs are
  an evidence-only gate. The gate must use public refs only and pass correctness
  with zero rejected claims and no pending validator review before the receipt
  can become verified/payable. A studied-knowledge source that is present but
  invalid fails the gate closed even when the gate is optional: bad optional
  evidence may not leave a contribution payable while attaching blockers.
  Studied knowledge does not grant mutation, spend, deployment, settlement, or
  self-review authority. Artanis studying labor is a recognized hygiene work
  type only through this evidence-only source mapping.
- After repeated rejected or revision-required attempts on the same
  receipt/scope, the receipt becomes human-review-only until the benchmark,
  budget, or scope changes.
- Public debt-receipt projections must reject raw diffs, prompts, generated
  fixtures, provider payloads, customer data, private repo data, payment or
  wallet material, payout targets, secrets, and raw timestamps.
- Hygiene settlement uses an HONEST verification basis. The hygiene-lane
  settlement dispatch (`POST /api/hygiene-lane/settlement-receipt`) settles a
  merged hygiene debt receipt through the SAME owner gate
  (`OPENAGENTS_REAL_SETTLEMENT_GATE`, scoped by a `run.hygiene.lane.<YYYYMMDD>`
  run ref) and the SAME Spark treasury payout rail as the Tassadar run
  settlement, but its receipt projection states the basis is
  `hygiene_merged_reviewed` (merged PR + reviewer acceptance + debt receipt).
  It must NEVER emit an `exact_trace_replay` verdict or a
  `verificationChallengeRef` for hygiene work, and must not route hygiene work
  through the Tassadar lease/challenge settle endpoint (which would force a
  fabricated trace-replay verification receipt). The debt-receipt projection is
  the source of truth for payability; an operator cannot assert payability
  through the request body, and an absent projection fails closed.
- Hygiene settlement is fail-closed and idempotent: only a `payable`
  (non-duplicate, non-blocked) debt receipt with a payable computed amount
  (`computeHygieneLaneSettlementSats`, in [1, 100]) under the gate cap may move
  real Bitcoin; the gate-disabled / over-cap / not-allowlisted cases record the
  honest simulation chain (`moneyMovement:'none'`, `realBitcoinMoved:false`)
  rather than paying. One settlement per `DebtReceiptKey`, and a retry on the
  same `idempotencyRef` dispatches at most once. No registered Spark
  destination means no send. Receipt projections remain public-projection-safe
  (redacted `payout.spark.<digest>`-style refs only; no raw `spark1…`,
  preimages, invoices, or payout targets).
- A payable debt receipt must exist in a durable store before it can settle.
  The requester / settlement-authority creates it through the admin-only
  create endpoint (`POST /api/hygiene-lane/debt-receipts`), which reprojects the
  supplied public-safe evidence through the debt-receipt policy and persists it
  ONLY when it reaches the `payable` state, keyed by its `DebtReceiptKey`
  (`hygiene_debt_receipts` D1 table, one row per key). Create is idempotent on
  the key. The settle route resolves payability ONLY from this store: an absent
  row fails closed (`debt_receipt_not_found`), and a retired row reprojects to
  `duplicate_replay`. Once real Bitcoin moves, the settle route marks the key
  `retired` (recording the settlement receipt ref), so a second settle on the
  same key is a duplicate replay and never re-pays; a retired key also cannot be
  re-created. The simulation chain (gate OFF) does NOT retire the receipt, since
  nothing was paid. Every stored column is a public-safe ref or a bounded
  integer, and the policy re-validates ref safety on every reprojection, so the
  store never relies on itself to keep secrets out.
- Regression coverage for this policy lives in the OpenAgents-owned
  verification runner / independent replay plan plus
  `workers/api/src/debt-receipt-key.test.ts`,
  `workers/api/src/debt-receipt-policy.test.ts`,
  `workers/api/src/debt-receipt-work-request.test.ts`,
  `workers/api/src/hygiene-lane-settlement.test.ts`,
  `workers/api/src/hygiene-lane-settlement-routes.test.ts`,
  `workers/api/src/hygiene-debt-receipt-store.test.ts`,
  `workers/api/src/hygiene-lane-debt-receipt-create-routes.test.ts`, and
  `workers/api/src/artanis-studying-labor.test.ts`; the durable backing store
  starts in `workers/api/migrations/0207_hygiene_debt_receipts.sql`, and the
  buyer-facing lane packet lives in
  `docs/labor/2026-06-18-debt-receipt-hygiene-lane.md`.

## User-Facing Live Data Integrity

- User-facing product surfaces must not render dummy, example, fixture, seed,
  placeholder, mock, or static snapshot values as live facts.
- User-facing money, payout, tip, revenue-share, settlement, launch-gate,
  availability, leaderboard, and network-stat values must come from live
  public-safe projections or render an explicit empty, unavailable, gated, or
  error state.
- Documentation specs may describe schemas and example field names, but they
  must not prescribe hard-coded user-facing totals, fake creators, fake tips,
  fake payouts, fake balances, fake revenue-share amounts, or static snapshots
  for implementation.
- Regression coverage for public homepage money and status panels must assert
  that visible values are endpoint-derived or explicitly unavailable, not
  embedded examples.

## Generated Site Checkout Evidence Gate

- Generated Site payment fixtures must expose a public receipt bundle before
  any live checkout claim. The bundle requires checkout intent refs, payment
  proof refs, receipt refs, active entitlement refs, and matched
  reconciliation refs.
- Checkout returns are never payout authority. Client success pages must not
  create receipts, entitlements, payout intents, or settlement claims.
- Verified buyer payment and active entitlement are checkout evidence only
  unless a live-provider checkout gate is explicit. Checkout evidence is still
  not accepted-work payout evidence.
- Public payout or settlement copy requires separate accepted-work refs, payout
  target approval, fresh wallet readiness, spend cap, payout bridge readiness,
  and settlement receipt refs. Without settlement receipt refs, generated Site
  public copy must stay in checkout-evidence-only mode.
- Generated Site payment projections must reject raw MDK credentials, raw
  invoices, webhook payloads, payment hashes, preimages, wallet material,
  customer private data, and raw payout targets.
- Regression coverage for this policy lives in
  `workers/api/src/generated-site-payment-smoke-fixture.test.ts`,
  `workers/api/src/site-commerce-routes.test.ts`,
  `workers/api/src/site-payment-proof.test.ts`, and
  `workers/api/src/site-payment-to-payout-bridge.test.ts`.

## Controlled Pylon Assignment Dispatch

- Operator Pylon assignment creation must pass
  `gate.public.pylon.assignment_dispatch.controlled.v1` before any new
  assignment lease is persisted.
- The dispatch gate requires campaign policy refs, selection policy refs,
  explicit payment mode, idempotency refs, pause policy refs, rollback path
  refs, closeout path refs, no-duplicate refs, no-Forum-publish refs, required
  capability refs, an explicit unpaused campaign state, and an explicit
  `forumAutoPublishAllowed:false` state.
- Assignment dispatch must deny missing Pylons, non-active Pylons,
  wallet-not-ready Pylons, offline Pylons, stale heartbeat Pylons,
  below-minimum client versions, wrong capability refs, and duplicate
  unexpired active assignments.
- Paid assignment modes require public-safe spend-cap refs at dispatch time.
  The assignment route still must not spend bitcoin, dispatch payouts, settle
  work, mutate provider accounts, or publish Forum posts.
- Idempotency replay may return the original assignment response, but it must
  not create a second lease or use wallet readiness as spend authority.
- Regression coverage for this policy lives in
  `workers/api/src/pylon-api-routes.test.ts`.

## Spark Address Payout Target Registration

- A Pylon may register its OWN Spark address as a payout target so the platform
  can pay it natively over Spark (#5252). `spark_address` is an admitted
  `PayoutTargetKind` whose only public projection is the redacted
  `payout.spark.<digest>` ref (digest of the raw address), mirroring the
  existing `bolt12_offer → payout.bolt12.*` allow-prefix pattern.
- The raw `spark1…` is PAYMENT MATERIAL. It rides only the authenticated
  `POST /api/pylons/:ref/spark-payout-target` request body, is stored in the
  private operator store `pylon_spark_payout_targets` keyed to the agent's
  `pylonRef`, and must never enter a public projection, a public Pylon event
  body, a tracked file, a commit, a log, or normal output. Public surfaces carry
  only the redacted digest ref. `assertPublicProjectionSafe` and
  `admitPayoutTarget` must continue to reject a raw `spark1…` in any public ref,
  and `JSON.stringify` of a projection must never contain a `spark1…`.
- Registration is auth-scoped: an agent may register a Spark target only on a
  Pylon it owns (`ownerAgentUserId` from the bearer-token session), and the
  declared `payout.spark.<digest>` ref must match the server-recomputed digest of
  the submitted raw address. A mismatch fails closed as a validation error.
- Registration is idempotent: re-registering the same address for the same Pylon
  is a no-op upsert (private store keyed by `pylonRef`, public event
  idempotency-keyed), not a duplicate.
- The settlement payout destination resolver looks up the recipient's registered
  raw Spark address from the private store and returns it as the native Spark
  send destination for the gated real-settlement path (#5232) and native send
  (#5225). It fails closed (returns `undefined`, no send) when the recipient has
  no registered Spark target or the private store read fails; the raw
  destination never enters any receipt projection. Registration grants no
  payout, settlement, spend, accepted-work, or public-claim authority by itself.
- Regression coverage for this policy lives in
  `workers/api/src/pylon-api-routes.test.ts` and `apps/pylon/tests/wallet.test.ts`;
  the private D1 backing store starts in
  `workers/api/migrations/0202_pylon_spark_payout_targets.sql`.

## Tassadar Auto-Stream Settlement And Daily Budget Cap

- Auto-streaming real settlement (#5309/#5310) is hands-off: when the worker →
  validator verdict path finalizes a `Verified` `exact_trace_replay` fixture
  pair, BOTH legs settle automatically — the per-window rate of 5 sats to the
  worker (`lease.pylonRef`) AND 5 sats to the validator (its registered Spark
  payout target) — through the same proven #5232 `spark_treasury` rail, with NO
  operator POST. The per-window rate is fixed at
  `TassadarPerWindowWorkerRewardSats = 5` and
  `TassadarPerWindowValidatorRewardSats = 5`.
- Auto-settlement is INERT by default everywhere: every leg resolves to skip
  while `OPENAGENTS_REAL_SETTLEMENT_GATE` is OFF (the default). It is gated by
  the same gate as the admin path (enabled + `spark_treasury` adapter + run
  allowlist + per-payout `maxPayoutSats`) PLUS the new cumulative daily cap.
- Auto-settlement is RECEIPT-FIRST, IDEMPOTENT, and FAIL-SOFT. Each leg derives
  a deterministic settlement receipt ref keyed by challenge + party, so a retry
  of the same Verified pair pays AT MOST ONCE per challenge per party. A
  blocked/failed/over-budget settlement NEVER breaks verification or the
  heartbeat: the verdict route fires the hook fire-and-forget under a catch. No
  real-settled receipt exists without a confirmed dispatch + matched
  reconciliation, preserving every #5232 safety + redaction guard
  (`assertPublicProjectionSafe`; no raw `spark1…`, invoice, preimage, or wallet
  material in any projection).
- The validator leg SKIPS cleanly (no error, `skipped: no_payout_destination`)
  when the validator has no registered Spark payout target; the worker leg is
  unaffected.
- The validator-leg payout destination MUST resolve from the validator's OWN
  registered Spark target with NO operator step (#5310/#5306/#5394). The worker
  leg's contributorRef is the verified registered `pylonRef` (so it resolves
  directly), but the validator submits its verdict with its DEVICE-ref (its
  nodeId, `pylon_<hash>`), which is NOT a `pylonRef` and matches no registration
  directly. The owner resolver MUST therefore map a device-ref to the most
  recent `pylon_ref` that device acted as a WORKER under (from
  `training_trace_contributions`), then resolve THAT pylon's owner, then use the
  owner-scoped `readByOwner` Spark target. This binding is to the device's own
  historical worker pylon and its own owning agent only; it MUST NOT cross agent
  ownership, MUST fail closed (device never recorded a worker contribution, or
  the resolved owner has no target → `no_payout_destination`), and grants no new
  authority. Without this backstop the validator leg always skipped and the
  worker/validator pair could not auto-settle hands-off. Regression coverage:
  `workers/api/src/tassadar-auto-settlement-validator-resolution.test.ts`.
- The optional gate field `maxDailyPayoutSats` is the cumulative daily real
  budget. It is BACKWARD-COMPATIBLE: an existing armed gate value WITHOUT the
  field still decodes and behaves exactly as before (per-payout-only, no
  aggregate ceiling). When present it is the maximum real sats that may be
  auto-settled per UTC calendar day, itself clamped to the module hard daily
  ceiling `TassadarRunSettlementHardDailyCapSats`. The daily total is
  RECEIPT-FIRST: it is the sum of `settlement_recorded` receipts whose
  projection is `state: settled` and `moneyMovement: real_bitcoin` on the
  current UTC day (simulation receipts never consume real budget). It FAILS
  CLOSED: once a day's real total would exceed the cap, further auto-settlements
  fall back to skip until 00:00:00Z resets the window. The boundary is UTC by
  deliberate choice for deterministic reset.
- The optional gate field `runScopedStreaming` widens eligibility so streaming
  does not require a hand-maintained per-agent allowlist. When absent or false,
  only `allowedContributorRefs` recipients are eligible (prior behavior). When
  true, ANY contributor with a registered Spark payout target on an allowlisted
  run is eligible — still bounded by the per-payout cap, the daily cap, and the
  independent worker≠validator replay verification (the trust anchor). The run
  must always be allowlisted even under run-scoped streaming; the explicit
  `allowedContributorRefs` path keeps working alongside it. No adapter is
  broadened and no existing guard is removed.
- Compiled-module construction settlement (#5326) is a separate payment leg
  from the 5+5 worker/validator auto-stream. It is eligible only for a
  `Verified` `exact_trace_replay` challenge tied to a digest-pinned compiled
  module contribution, and its settlement receipt must carry explicit
  construction metadata (`constructionSettlement: true`,
  `settlementSource: compiled_module_construction`, public module kind, module
  digest, contribution ref). It must not treat ordinary replay work,
  marketplace listing, purchase, entitlement, or composition metadata as
  construction-payment authority.
- Construction settlement records simulation receipts by default in
  `unpaid_smoke` mode (`adapterKind: simulation`, `moneyMovement: none`,
  `realBitcoinMoved: false`) so a clean checkout can prove the construct →
  verify → pay loop without moving sats. Real Spark settlement remains
  owner-gated by the existing `OPENAGENTS_REAL_SETTLEMENT_GATE`, per-payout
  cap, daily cap, run allowlist, contributor eligibility, registered payout
  target, and receipt-first dispatch/reconciliation chain. An over-cap,
  non-verified, unsafe-ref, missing-target, or dispatch-failed construction
  settlement must fail soft with no real-settled receipt.
- Regression coverage for this policy lives in
  `workers/api/src/tassadar-auto-settlement.test.ts` and
  `workers/api/src/tassadar-run-settlement-gate.test.ts`.

## Tassadar Compiled-Module Composition Verification

- A linked compiled-module listing is not settlement-eligible merely because
  the composed dense payload replays to the expected digest. The Worker replay
  path and marketplace projection must require a first-class composition
  verdict: every constituent dense bank replays to its source trace digest, the
  psionic link-resolution compatibility evidence conformance-checks
  (requested/selected refs, trust posture, claim class, compatibility digests,
  dependency graph digest, resolution digest, and dependency edge), and the
  composed exact replay digest matches end-to-end.
- Injected link incompatibility, dependency-graph drift, resolution-digest
  drift, or tampered constituent evidence must reject the linked-module replay
  even when the composed trace digest itself still matches. Marketplace
  purchase settlement remains blocked until
  `compositionVerificationCleared: true`; purchase refs and settlement refs do
  not override a failed composition verdict.
- Public compiled-module marketplace projections may expose only digest-pinned
  refs, receipt refs, blocker refs, and caveats. They must not expose raw
  module install authority, runtime activation, private traces, wallet/payment
  material, or real-settlement authority.
- Regression coverage for this policy lives in
  `packages/tassadar-executor/src/linked-dense-module.test.ts`,
  `workers/api/src/tassadar-replay-validator.test.ts`, and
  `workers/api/src/tassadar-compiled-module-marketplace.test.ts`.

## Probe GEPA Campaign Public Projection

- Artanis/Probe GEPA campaign projections are public-safe summaries of
  refs. They must not contain raw prompts, raw traces, raw benchmark fixtures,
  provider credentials, account refs, bearer material, wallet material,
  invoices/preimages, private repo paths, or local filesystem paths.
- Campaign claim state cannot advance beyond `none` without matching evidence
  refs: retained claim states require retained result refs, validation claim
  states require validation result refs, and holdout claim states require
  holdout result refs.
- Public Pylon work can be visible without implying payout. Settled payout
  claims require public receipt refs and settlement receipt refs.
- Regression coverage for this policy lives in
  `workers/api/src/probe-gepa-campaign-projection.test.ts`.

## Probe GEPA Stage 0 No-Spend Campaign Gate

- Probe GEPA Stage 0 is a no-spend campaign gate only. Accepted imports must
  use `unpaid_smoke`; rejected closeouts may use `rejected_no_pay`. Stage 0
  must not carry payment receipts, settlement receipts, payout claims, public
  Terminal-Bench score claims, model-training claims, or runtime candidate
  activation claims.
- Stage 0 dashboard green requires multiple distinct Pylons plus public-safe
  assignment refs, accepted closeout refs, rejected closeout refs, artifact
  refs, proof bundle refs, resource usage refs, verifier result refs, Probe
  closeout import refs, Psionic import dry-run refs, and Artanis summary refs.
- Accepted and rejected closeouts must both be represented before Stage 0 can
  clear. A single-Pylon canary or accepted-only bundle remains blocked.
- Public-safe Stage 0 bundles must reject raw benchmark data, raw prompts, raw
  traces, provider payloads, customer data, wallet/payment material, model
  weights, private repo/source refs, secrets, and raw timestamps.
- Clearing Stage 0 does not authorize paid GEPA modes. Paid, payable, and
  settled-bitcoin campaign claims remain blocked until a later gate supplies
  payment and settlement evidence.
- Regression coverage for this policy lives in
  `workers/api/src/probe-gepa-stage0-no-spend-campaign.test.ts`,
  `workers/api/src/pylon-gepa-metric-call-assignments.test.ts`, and
  `workers/api/src/probe-gepa-campaign-projection.test.ts`.

## Probe GEPA Paid-Mode Campaign Ladder

- Probe GEPA paid campaign copy must clear the paid-mode ladder after Stage 0
  is green. The ladder requires ready settlement-readiness results for
  `unpaid_smoke`, `payable_pending_settlement`, and `settled_bitcoin`.
- Payable-work claims require payment receipt refs. Settled-bitcoin campaign
  claims additionally require settlement receipt refs, wallet send-readiness
  refs, outbound liquidity refs, and a live-small-sats smoke ref.
- The public ladder projection must expose aggregate campaign payment mode,
  per-assignment payment modes, payment receipt refs, public settlement receipt
  refs, readiness decision refs, bridge attempt refs, and blocker refs.
- Duplicate bridge attempts must not double-settle. A replay is safe only when
  it points at the original bridge attempt and carries no fresh payment or
  settlement receipt refs. Multiple accepted settled-bitcoin bridge attempts
  for the same assignment remain blocked.
- Clearing payable mode does not imply settled bitcoin. Clearing settled
  bitcoin does not imply public Terminal-Bench score claims, model training,
  runtime candidate activation, automatic dispatch, or automatic payout
  authority beyond the modeled receipt-backed claim.
- Regression coverage for this policy lives in
  `workers/api/src/probe-gepa-paid-mode-ladder.test.ts`,
  `workers/api/src/probe-gepa-settlement-readiness.test.ts`, and
  `workers/api/src/probe-gepa-stage0-no-spend-campaign.test.ts`.

## Qwen 3.6 Remote Pylon Fine-Tune Claims

- Public Qwen 3.6 fine-tune copy must stay blocked until a remote Pylon training
  run report has at least two distinct remote worker refs, signed worker receipt
  refs, required shard receipt refs, artifact refs, merge refs, eval refs,
  adapter admission refs, payment refs, settlement refs for settled claims, and
  public projection refs.
- Public projections must expose bounded remote Qwen training/adaptation,
  full-transformer fine-tune, remote-device, Harvey private-benchmark, payable,
  and settled-bitcoin claims as separate booleans.
- Local loopback workers, local Psionic rehearsals, weight-load reports, and
  sampled-projection LoRA are not sufficient evidence for a full remote Qwen 3.6
  transformer backprop fine-tune on people's devices.
- Public Harvey replay evidence must not be upgraded into private benchmark
  performance. Payable or deferred payment state must not be upgraded into
  settled bitcoin.
- Bad or quarantined shard refs block the public fine-tune claim until the run
  report identifies replacement shard receipts and passes merge/eval/admission.
- Regression coverage for this policy lives in
  `workers/api/src/qwen-remote-pylon-finetune-gate.test.ts`.

## Public Launch Copy Claim Gate

- Public launch copy in AGENTS text, manifests, OpenAPI descriptions, Forum seed
  text, Artanis summaries, launch announcements, templates, pages, and
  dashboards must not use unsafe live/earning/settlement phrases unless the
  matching claim gate is green and the surface carries matching public evidence
  refs.
- The unsafe phrase policy covers broad Pylon earning, full GEPA network live,
  Qwen 3.6 remote fine-tuning live, provider provider-capacity marketplace
  live, referral sats streams, hosted MDK direct payouts, creator spendable
  settlement, and unbounded Artanis autonomy.
- Stale health blocks unsafe launch copy even when an evidence gate otherwise
  reports ready.
- Prohibition and caveat language such as "do not claim X" is allowed, but the
  same phrase in affirmative launch copy must fail until the evidence gate and
  evidence refs are present.
- Launch-critical public and registered-agent routes claimed in the public
  agent sheet must remain covered by `docs/live/AGENTS.md`, the capability
  manifest, and OpenAPI. Planned broad scoped API keys must stay non-callable
  and absent from OpenAPI until implemented with separate authority gates.
- Public agent onboarding docs must not point agents at stale repository-internal
  source paths. Critical onboarding links, including the Episode 230 founder
  open-letter transcript, must be checked before deploy; the transcript check
  must verify that the URL returns the expected transcript body, not an HTML
  fallback page.
- The public launch dashboard must include every numbered source-transcript
  promise from the source-conversation gap audit exactly once with red/yellow/green state,
  evidence refs, blocker refs, safe copy, and unsafe-copy boundaries. Stale
  endpoint data must not leave stale-sensitive rows green.
- Regression coverage for this policy lives in
  `workers/api/src/public-launch-copy-gate.test.ts`,
  `workers/api/src/public-launch-dashboard.test.ts`, and
  `workers/api/src/openagents-agent-sheet-route-coverage.test.ts`; critical
  onboarding link coverage lives in `scripts/check-live-agent-doc-links.mjs`
  via `bun run check:agent-doc-links`.

## Artanis Probe GEPA Production Smoke

- Artanis production-equivalent Probe GEPA/Pylon smoke is retained evidence,
  not runtime authority. It may clear the `production_e2e_smoke` launch-gate
  blocker only when it carries SHC/Harbor refs, Probe closeout bundle refs,
  accepted and rejected Pylon closeout refs, artifact/proof/resource/verifier
  refs, route scorecard refs, Psionic import refs, explicit `unpaid_smoke`
  mode, and a public-safe Forum summary ref.
- The retained smoke must deny wallet spend, settlement mutation, provider
  mutation, model training, automatic candidate promotion, public benchmark
  score claims, payout claims, and automatic Forum posting.
- Clearing `production_e2e_smoke` does not by itself clear
  `scheduled_runner`, does not by itself allow continuous-autonomy copy, and
  does not authorize public Terminal-Bench or paid-work settlement claims. A
  separate bounded scheduled-runner proof must own that gate.
- Regression coverage for this policy lives in
  `workers/api/src/artanis-gepa-production-smoke.test.ts`,
  `workers/api/src/artanis-production-launch-gate.test.ts`, and
  `workers/api/src/artanis-public-report.test.ts`.

## Artanis Bounded GEPA Scheduled Runner

- Artanis scheduled-runner evidence is bounded GEPA status-projection
  evidence. It may clear the `scheduled_runner` launch-gate blocker only when
  the Probe GEPA/Pylon production smoke has passed, the runner is explicitly
  enabled, public health/staleness refs exist, closeout receipts exist,
  idempotency refs exist, no-duplicate assignment and Forum post refs exist,
  Pylon selection policy refs exist, and pause/disable/rollback refs exist.
- The bounded runner must deny assignment dispatch, duplicate assignment,
  duplicate Forum post, automatic Forum publishing, model training, provider
  mutation, runtime promotion, settlement mutation, and wallet spend
  authority.
- Clearing `scheduled_runner` allows public copy about bounded continuous
  Artanis status operation only. It does not authorize unbounded production
  administration, public Pylon release claims, Terminal-Bench score claims,
  Probe candidate activation, accepted-work payout claims, settlement claims,
  provider mutation, or wallet spend.
- Regression coverage for this policy lives in
  `workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts`,
  `workers/api/src/artanis-production-launch-gate.test.ts`,
  `workers/api/src/artanis-public-report.test.ts`, and
  `workers/api/src/artanis-scheduled-runner.test.ts`.

## Artanis Public Report Authority Split

- The Artanis public report must expose separate booleans for status projection,
  dispatch authority, spend authority, settlement authority, provider mutation
  authority, and Forum auto-publish authority.
- Bounded scheduled-runner evidence may allow status-projection copy. It must
  not imply dispatch, wallet spend, settlement, provider mutation, runtime
  promotion, or automatic Forum publishing.
- Stale, blocked, degraded, unavailable, or unknown Artanis health must block
  green launch copy even when status-projection evidence is retained.
- The public report must expose stable blocker refs, launch runbook command refs,
  and Forum intent idempotency refs for pause, disable, revoke, and no-duplicate
  publication checks without exposing private runner state.
- Regression coverage for this policy lives in
  `workers/api/src/artanis-public-report.test.ts`,
  `workers/api/src/artanis-production-launch-gate.test.ts`, and
  `workers/api/src/artanis-retained-launch-smoke.test.ts`.

## Probe GEPA Forum Summary Drafts

- Probe GEPA Forum summaries are regenerated from public-safe refs. They must
  not include raw prompts, raw traces, raw benchmark fixtures, provider
  credentials, account refs, bearer material, wallet material,
  invoices/preimages, private repo paths, local filesystem paths, raw logs, or
  raw timestamps.
- Generated copy must use exact claim-state language. Retained evidence must
  not be described as a public benchmark score, and validation evidence must not
  be described as frozen holdout performance.
- Probe may prepare public-safe copy or post only as its own registered agent.
  Posting as Artanis requires the existing OpenAgents product surface/operator authority path; Probe
  summaries must not invoke an Artanis bridge.
- Artanis Probe GEPA public summaries require explicit operator authority refs
  and projection authority refs. Generated copy may describe GEPA as
  Pylon-distributed rollout optimization, not distributed neural-network
  training, and must not claim public benchmark score, paid work, settlement,
  active production, or release-candidate state.
- Regression coverage for this policy lives in
  `workers/api/src/probe-gepa-forum-summary.test.ts` and
  `workers/api/src/artanis-probe-gepa-benchmark-summary.test.ts`.

## Probe GEPA Outcome Metrics

- Probe benchmark wins are not product wins unless they are connected to
  accepted coding outcome refs and public/private proof refs.
- The Probe GEPA Stage 1 benchmark promotion gate may emit `shadow` or rejected
  benchmark-only state only. It must not emit `active` or `release_candidate`;
  those states require a separate explicit OpenAgents product surface/Blueprint production gate.
- The product surface may display a Probe GEPA candidate as `benchmark_only`, `shadow`,
  `release_candidate`, or `active`, but `active` requires accepted outcome refs
  plus proof refs. Benchmark validation alone is not active-product authority.
- Product before/after metrics must carry route scorecard refs and validation
  refs. Claim text must distinguish benchmark validation from paid customer
  outcome improvement.
- Regression coverage for this policy lives in
  `workers/api/src/probe-gepa-outcome-metrics.test.ts` and
  `workers/api/src/probe-gepa-stage1-shadow-promotion-gate.test.ts`.

## Public Projection Staleness Declaration

- Every public projection carries `generatedAt` (or `lastRebuiltAt`; numeric
  `generatedAtUnixMs` where the surface's safety scan bans raw ISO timestamps
  in string fields) plus a declared staleness contract, and either rebuilds on
  the state transitions that matter (event-driven invalidation at the write
  site) or composes live at read. A projection that cannot meet its own
  declared staleness must say so in the payload rather than serve stale data
  as current. (Epic #4751; instances #4744, #4745, #4746, #4747, #4735,
  #4752, #4753, #4754.)
- The shared contract vocabulary is
  `workers/api/src/public-projection-staleness.ts`
  (`projection_staleness.v1`: `composition` of `live_at_read`,
  `rebuilt_on_transition`, or `stored_snapshot`, plus `maxStalenessSeconds`
  and the `rebuildsOn` transition set). It deliberately extends the shape
  frozen by the Tassadar trace factory's day-0 `projection_rebuild.v0.1`
  contract (#4748,
  `workers/api/src/tassadar-trace-factory/projection-rebuild.ts`); do not
  invent a second staleness vocabulary.
- New public projections must ship compliant. The zero-debt architecture check
  (`scripts/check-zero-debt-architecture.mjs`, run by `bun run
check:architecture` inside `check:deploy`) discovers `/api/public/...`
  route literals, fails any route missing from its projection-surface ledger,
  greps `staleness_declared` modules for the shared contract, and freezes the
  legacy count as an exact ratchet budget that may only shrink as retrofits
  land.
- Projection inventory (staleness mode → compliance as of epic #4751):
  - `GET /api/public/marketing-agency/receipts/{receiptRef}` — live at read
    over the static mocked delivery-receipt fixture. `staleness_declared`.
  - `GET /api/public/artanis/report` — live at read over tick rows rebuilt on
    closeout — compliant (`generatedAtUnixMs`, report + loop contracts, stale
    and example-fallback flags with caveat refs).
  - `GET /api/public/artanis/tick-streak` — live at read over the
    tick-decision and exact-replay closeout-verdict ledgers — compliant
    (`generatedAt`, `staleness` contract `projection_staleness.v1`
    live_at_read, the consecutive unattended-tick-streak counter for
    artanis.tassadar_evolution_loop.v1: currentStreak/longestStreak/
    targetReached against streakTarget 10, each currentStreak assignment
    dereferenceable as an artanis_admin_closeout receipt).
  - `GET /api/public/artanis/tassadar-distillation-dataset` — live at read over
    the tick-decision and exact-replay closeout-verdict ledgers — compliant
    (`generatedAt`, `staleness` contract `projection_staleness.v1`
    live_at_read, a refs-only Tassadar distillation dataset-curation receipt
    for artanis.tassadar_evolution_loop.v1: receiptState, datasetRef,
    required/source verified trace counts, digest prefixes, closeout receipt
    refs, and blocker-clearing refs; no raw trace bodies, runner logs,
    settlement, model-training, or model-promotion authority).
  - `GET /api/public/artanis/responder-support` — live at read over the
    Artanis responder-action ledger — compliant (`generatedAt`, `staleness`
    contract `projection_staleness.v1` live_at_read, the external-contributor
    Pylon-support flow projection for artanis.pylon_support_responder.v1:
    per-asker-provenance answered counts, externalContributorFlowProven, and
    each external interaction's dereferenceable reply-post ref).
  - `GET /api/public/artanis/labor-receipts` — live at read over the durable
    Artanis unattended-labor receipt store — compliant (`generatedAt`,
    `staleness` contract `projection_staleness.v1` live_at_read; folds the
    content-addressed, public-safe consolidated request receipts into a feed:
    per-terminal-state counts, placed-request count, and each receipt's
    lifecycle refs re-verified against the ref it is keyed under).
  - `GET /api/forum/tip-leaderboards` — live at read — compliant
    (`generatedAt`, contract, ranked-creator ladder credited/swept sats,
    honesty caveat refs).
  - `GET /api/forum/moderation/tip-earnings` — live at read — compliant.
  - `GET /api/forum/actors/{actorRef}/tip-earnings` — live at read —
    compliant.
  - Forum post `tipStats` blocks (topic detail, post detail, post list) —
    live at read — compliant (each block carries the contract).
  - `GET /api/agents/profiles/{profileRef}` and
    `GET /api/forum/actors/{actorRef}/profile` — live at read over
    registration, approved owner claims, and verified X-proof challenges —
    compliant (`generatedAt`, contract, `x_verified_agent` state).
  - `GET /api/agents/claims/rewards` and `/{rewardRef}` — live at read —
    compliant (#4754).
  - `GET /api/provider-accounts/pool` — live at read over provider-account,
    lease, and failover state — compliant (`generatedAt`, contract, typed
    eligibility reasons and reconnect nudges; lease refs and typed state only,
    no provider secret material) (#4766).
  - `GET /api/public/metrics/accepted-outcomes-per-kwh` — live at read over
    the receipt-backed accepted-outcome seed, metric definition, and product
    promise registry — compliant (`generatedAt`, contract, evidence-state
    labels, caveats, and modeled-vs-measured gate).
  - `GET /api/public/payments/contributor-accrual-bundle?economicsId=<id>` —
    live at read over one persisted accepted-outcome economics record (promise
    `payments.accepted_outcome_economics.v1`, red;
    `blocker.product_promises.contributor_ledger_missing`) — compliant
    (`generatedAt`, `live_at_read` contract). Read-only public projection of the
    reconciled gross-margin receipt + contributor accrual ledger: lifecycle and
    evidence labels stay visible, internal monetary cents are dropped, and every
    contributor entry's payable/settlement state stays honestly
    `not_yet_evidenced`. No dispatch, spend, settlement, or payout authority.
  - `GET /api/public/demand-provenance` — live at read over revenue-bearing
    public surfaces that carry typed internal/external demand splits — compliant
    (`generatedAt`, `projection_staleness.v1` live-at-read contract, AO/kWh
    surface summary, internal/external/unlabeled counts, zero external-demand
    claim, remaining coverage refs, and authority boundary). It is proof-copy
    discipline only: no revenue, demand, payout, settlement, reporting, or
    public-claim upgrade authority.
  - `GET /api/public/marketplace/composed-products` — live at read over the
    INERT compose-and-list listing store (EPIC #5510, #5515; promise
    `marketplace.compose_and_list_products.v1`, planned) — compliant
    (`generatedAt`, `live_at_read` contract). The surface is flag-gated
    (`MARKETPLACE_COMPOSE_AND_LIST_ENABLED`, default off => empty store) and the
    payload always reports `inert: true` / `promiseState: 'planned'`; it makes
    no billing, fulfillment, or live-product claim.
  - `GET /api/public/autopilot/composed-runs` — live at read over the INERT
    Autopilot all-in-one composed-run store (EPIC #5510, #5519; promises
    `autopilot.all_in_one_business_system.v1` + `cloud.primitives_suite.v1`, both
    planned) — compliant (`generatedAt`, `live_at_read` contract). The surface is
    flag-gated (`AUTOPILOT_COMPOSED_RUN_ENABLED`, default off => empty store) and
    the payload always reports `inert: true` / `promiseState: 'planned'` over both
    capstone promise ids, with the three composition blockers surfaced as
    `unclearedBlockerRefs`. It shows the composition SHAPE only (one balance ref,
    one receipt envelope referencing per-component receipt refs derived from the
    inference / fine-tuning / sandbox primitive scaffolds); it provisions no
    primitive, debits no balance, and makes no billing, settlement, or
    live-business claim.
  - `GET /api/public/autopilot/labor-products` — live at read over the INERT
    agentic labor-product flow store (promise
    `autopilot.agentic_labor_products.v1`, yellow) — compliant (`generatedAt`,
    `live_at_read` contract). The surface is flag-gated
    (`AGENTIC_LABOR_PRODUCTS_ENABLED`, default off => empty store) and the
    payload always reports `inert: true` / `promiseState: 'yellow'` with the two
    labor blockers surfaced as `unclearedBlockerRefs`. It shows the end-to-end
    labor-product flow shape only (post -> order -> dispatch -> deliver ->
    settle, with the settlement receipt ref derived from the shared
    cloud-metering helper). The settlement seam (`settleLaborProductOrder`) is
    flag-gated INERT and owner-gated and is unreachable from this read-only
    route; the surface debits no balance and makes no live-sale claim.
  - `GET /api/public/autopilot/self-serve-fanout` — live at read over the INERT
    self-serve fanout plan store (promise
    `autopilot.control_center_fanout_marketplace.v1`, yellow) — compliant
    (`generatedAt`, `live_at_read` contract). The surface is flag-gated
    (`SELF_SERVE_FANOUT_ENABLED`, default off => empty store) and the payload
    always reports `inert: true` / `promiseState: 'yellow'` / `selfServe: true`
    / `workClass: 'code_task'`, surfacing the cleared self-serve blocker and the
    still-uncleared plugin-marketplace blocker. It models a customer-initiated
    single-action fanout plan (the lane-C gate decision plus the linked market
    work-request the fanout would list) over the existing server-side lane-C
    gate. The dispatch seam (`dispatchSelfServeFanout`) is flag-gated INERT and
    is unreachable from this read-only route; the surface lists nothing on the
    market and moves no money.
  - `GET /api/public/autopilot/marketplace-work-classes` — live at read over the
    in-module marketplace work-class catalog (promise
    `autopilot.control_center_fanout_marketplace.v1`, yellow) — compliant
    (`generatedAt`, `live_at_read` contract). Read-only registry view: it lists
    every registered work class with its `status`, names the single live class
    (`code_task`), and always reports `pluginMarketplaceBeyondCodeTaskLive: false`
    with the still-uncleared plugin-marketplace-beyond-code_task blocker in
    `unclearedBlockerRefs`. Optional `?workClass=` narrows to one class. No flag
    and no store: `assertCatalogInvariants` (inside the projection) throws rather
    than let any plugin class silently flip live, so the surface can never
    over-claim a live plugin marketplace; it lists nothing executable and moves no
    money.
  - `GET /api/public/markets/signature-monetization/metering` — live at read
    over the INERT signature usage-metering store (promise
    `marketplace.signature_monetization.v1`, red) — compliant (`generatedAt`,
    `live_at_read` contract). The surface is flag-gated
    (`SIGNATURE_USAGE_METERING_ENABLED`, default off => empty store) and the
    payload always reports `inert: true` / `promiseState: 'red'`, surfacing the
    public-safe usage-evidence refs (`usageEventRefs`, `usageIdempotencyRefs`,
    `exactUsageSubjectRefs`) the signature revenue gate consumes. Metering
    PRODUCES those refs (clearing
    `blocker.product_promises.signature_usage_metering_missing`) and drives the
    revenue gate to its `metered` rung; it meters no payload, prices nothing,
    debits no balance, and settles nothing — `signature_settlement_missing`
    stays owner-gated and is surfaced as `remainingOwnerGatedBlocker` (#5529).
  - `GET /api/public/pylon/multi-earning-node` — live at read over the INERT
    Pylon multi-earning store (promise `pylon.v0_3_multi_earning_node.v1`, red)
    — compliant (`generatedAt`, `live_at_read` contract). The surface is
    flag-gated (`PYLON_MULTI_EARNING_PROJECTION_ENABLED`, default off => empty
    store) and the payload always reports `inert: true` / `promiseState: 'red'`.
    It distinguishes the five amount classes (`modeled` / `observed` /
    `pending` / `paid` / `settled`) per earning mode and reports the
    `settledModeCount` against the `>=2`-modes-in-one-install bar for green, but
    never asserts the bar met as authority and never reports a settled mode
    without a public-safe `settlementReceiptRef`. It is the safe public
    projection deliverable (clearing
    `blocker.product_promises.safe_public_projection_missing`); the three
    install/receipt/settlement blockers
    (`pylon_v1_default_install_not_fully_closed`,
    `multi_earning_mode_receipts_missing`,
    `multi_earning_settlement_refs_missing`) stay owner-gated and are surfaced
    as `remainingOwnerGatedBlockers`. It records no real earnings, moves no
    money, and admits no install as closed (#5527).
  - `GET /api/public/omni/client-delivery-projection` — live at read over the
    INERT client-delivery workroom store (promise
    `workrooms.omni_client_delivery_workrooms.v1`, yellow) — compliant
    (`generatedAt`, `live_at_read` contract). The surface is flag-gated
    (`OMNI_CLIENT_DELIVERY_PROJECTION_ENABLED`, default off => empty store) and
    the payload always reports `effectsApplied: false`. When armed it projects
    the existing source-authorized business-object delivery seam
    (`buildOmniBusinessObjectDeliveryPlan`) over an injected workroom store:
    per-write approval-gated decisions plus the integration gate verdict. It
    applies no business-object write, sends nothing, settles nothing, spends
    nothing, mutates no connector, notifies nobody, launches no runner, and
    upgrades no public claim. It is the read-only delivery-projection
    deliverable (clearing
    `blocker.product_promises.omni_client_delivery_projection_missing`); the
    live-integration, owner-sign-off, and closeout-receipt blockers
    (`integration_inert_disabled`, `owner_sign_off_missing`,
    `closeout_receipt_missing`) stay owner-gated and are surfaced as
    `remainingBlockers`, so the promise stays yellow (DE-9 / #5532).
  - `GET /api/public/customer-one-cohort` — live at read over Customer #1
    cohort source rows and privacy-review evidence — compliant (`generatedAt`,
    contract, evidence-only opaque cohort refs, generic labels, counts, blockers,
    caveats, and three-completion D3 gate).
  - `GET /api/public/markets/open-markets` — live at read over the six
    Episode 213 markets with honest per-market state — compliant (`generatedAt`,
    contract, evidence-only per-market state/receipt flags, counts, blockers, and
    unsafe-copy guards; no market-making, settlement, or claim authority) (#5514).
  - `GET /api/public/markets/liquidity/skeleton` — live at read over the inert
    liquidity market skeleton — compliant (`generatedAt`, contract,
    `state="skeleton"`, `inert=true`, `moneyMovement="none"`,
    `settledTransactionCount=0`, `promiseGreen=false`; documents protocol shapes
    only, moves no money) (#5514).
  - `GET /api/public/markets/risk/skeleton` — live at read over the inert
    risk market skeleton (incl. the agentic-insurance-policy primitive) —
    compliant (`generatedAt`, contract, `state="skeleton"`, `inert=true`,
    `moneyMovement="none"`, `settledTransactionCount=0`, `promiseGreen=false`;
    documents protocol shapes only, binds/underwrites/pays nothing) (#5514).
  - `POST /api/public/business-signup` — live-at-write intake receipt over the
    inserted business signup row — compliant (`generatedAt`, contract,
    public-safe request id/status only; no email, phone, website, or freeform
    request text echoed). A converted signup may bind to the referral
    attribution spine (issue #5809): a bounded inbound `?ref=`/`referralCode`
    (a `site_referral_sources.public_source_ref`) or an already-captured
    `oa_pending_referral_attribution` cookie resolves a pending attribution that
    is consumed exactly once into `business_signup_referral_attributions` (keyed
    on the signup id; PRIMARY KEY + pending->claimed guard prevent
    double-credit) and recorded on the row as `referral_attribution_id`. This is
    ATTRIBUTION ELIGIBILITY ONLY — it moves no money and accrues no payout
    (refer-once-earn-forever payout stays usage-funded via
    `accrueCrossCategoryReferral`, never on signups). The public response echoes
    only a `referralAttributed` boolean — never the code or the internal
    attribution id. A referral resolution failure never fails the intake.
  - `GET /api/public/training/runs/{trainingRunRef}` — live at read over the
    Worker-authoritative training run, window, lease, verification challenge,
    and provider-confirmed settlement receipt rows — compliant (`generatedAt`,
    top-level contract, public-safe run projection, source refs, and
    provenance-labeled summary metrics only; no admin token, private logs,
    wallet material, pending-as-paid payout, or write authority).
  - `GET /api/public/training/runs/{trainingRunRef}/settlements` — public alias
    for the per-run settlements feed (#5403), serving the identical public-safe
    `routeReadRunSettlements` handler as the non-`/public/` path over
    provider-confirmed settlement receipts — compliant (`generatedAt`,
    top-level contract, public-safe settlement rows with `movementMode` and
    `realBitcoinMoved` flagged so simulation rows never count as real Bitcoin;
    no seeds, raw addresses, payment hashes, or write authority).
  - `GET /api/public/training/verification-challenges/{challengeRef}` — live at
    read standalone per-challenge dereference (#5403) over the
    Worker-authoritative verification challenge row, serving the same
    public-safe `publicTrainingVerificationChallengeProjection` (worker,
    validator, verdict, and challenge refs; the two compared sha256 digests;
    public-safe failure codes) exposed inside the run summary — compliant
    (`generatedAt`, top-level contract; no payloads, seeds, payment material,
    raw traces, or write authority).
  - `GET /api/public/tassadar-run-summary` — live at read compatibility feed
    for the live Tassadar spatial view over the same Worker-authoritative
    training run/window/lease/challenge rows — compliant (`generatedAt`,
    top-level contract, public-safe summary metrics, and honest idle envelope
    only; no admin token, private logs, wallet material, pending-as-paid
    payout, or write authority).
  - `GET /api/public/activity-timeline` — live at read over public-safe Pylon,
    training, verification, settlement receipt, Forum, Artanis, and capacity
    source families — compliant (`generatedAt`, top-level
    `projection_staleness.v1` contract, source-lag rows with stale/unavailable
    statuses and blocker/caveat refs, cursor-ordered events, projection gaps for
    unreadable source families, simulation-vs-real Bitcoin separation, and no
    wallet material, raw logs, payment preimages, provider/customer payloads, or
    write authority).
  - `GET /api/public/tassadar/compiled-module-marketplace` — live at read over
    the committed psionic linked-dense fixture and local composition gate —
    compliant (`generatedAt`, top-level contract, digest-pinned linked module
    listing, source-bank replay/conformance refs, psionic link-compatibility
    receipt refs, purchase/settlement blockers, and explicit no mutation/no
    real-settlement authority).
  - `GET /api/public/proof-replays` — live at read proof replay resolver over
    public Worker-authoritative proof, run, pylon, and settlement refs —
    compliant (`generatedAt`, top-level contract, public-safe source refs,
    replay bundle claim scope, explicit gaps/caveats, and no wallet material,
    raw logs, service tokens, payment preimages, or private operator payloads).
  - `GET /api/public/tassadar-replays/first-real-settlement` — live at read
    compatibility replay bundle for the first real Tassadar settlement —
    compliant (`generatedAt`, top-level contract, public-safe first-settlement
    proof/payment refs, failed-closed markers, simulation-vs-real payment
    distinction, and no wallet material, raw logs, service tokens, payment
    preimages, or private operator payloads).
  - `POST|GET /api/public/replay-clips` and
    `GET /api/public/replay-clips/{jobRef}` — live at read over the
    `replay_clip_jobs` D1 store (EPIC #5411, issue #5432) — compliant
    (`generatedAt`, top-level `projection_staleness.v1` `live_at_read`
    contract, public-safe job projection with claim scope
    `evidence_presentation_only`, public source/caveat/blocker refs, and the
    finished manifest URL only). The Worker creates `queued` jobs and reads
    records only; it never renders frames or runs native binaries (rendering
    is the owned render box's job, issue #5431). Grants no settlement, payout,
    deployment, accepted-work, provider, wallet, or public-claim authority.
    Regression coverage:
    `workers/api/src/replay-clip-job-routes.test.ts`.
  - `GET /api/public/site-referral-payouts` — live at read over the latest
    non-archived RL-1 Sites referral payout ledger entry per payout ref (#5458)
    — compliant (`generatedAt`, top-level `projection_staleness.v1`
    `live_at_read` contract; count-only per-state counts/sats plus real settled
    figures, the policy shape, campaign/policy refs, caveat/blocker refs, and a
    `ledgerWiredInSource` source-wiring flag). It selects ONLY `state` and
    `amount_sats` from the ledger — no user id, attribution id, payout ref,
    qualifying event ref, address, preimage, or invoice leaves the Worker — and
    is honest that no real referral payout has settled (`settledCount` /
    `settledSats` expected `0` while the wiring is present). The surface grants
    no attribution, accrual, eligibility, payout, or settlement authority and
    flips no promise; the `referral.refer_once_earn_forever.v1` promise stays
    red/owner-gated. Regression coverage:
    `workers/api/src/site-referral-payout-public-projection.test.ts`.
  - `GET /api/public/product-promises/audit` — live at read over the live
    product-promise registry joined against the promise transition-receipt feed
    (proof.claim_upgrade_receipts.v1) — compliant (`generatedAt`, top-level
    `projection_staleness.v1` `live_at_read` contract, `maxStalenessSeconds` 0,
    `rebuildsOn` registry/receipt transitions). Read-only enterprise audit
    surface: per promise it projects promiseId, productArea, currentState,
    lastVerifiedAt, blockerRefs, and the backing transition receipts (from->to
    state, registryVersion, receiptRef, result, evidence refs, owner signoff),
    plus a registry-wide summary listing any green promises with no recorded
    green-flip receipt. Filterable by promiseId/state/greenOnly. It re-projects
    only already-public data, exposes no private data, moves no money, grants no
    authority, and flips no promise. Regression coverage:
    `workers/api/src/promise-transition-audit-routes.test.ts`.
- GREEN-FLIP SIGN-OFF DELEGATION (owner-authorized 2026-06-20). Per-flip owner
  sign-off for yellow/red -> green transitions is DELEGATED to the operating
  agent. The operating agent MAY record a green transition (transition receipt +
  registry state change + deploy) when it is genuinely satisfied the promise is
  KEPT, with a dereferenceable receipt and all green gates honestly met. This does
  NOT waive the dereferenceable-receipt or gates-met requirements
  (`proof.claim_upgrade_receipts.v1`), and does NOT relax any money-arming,
  spend-enablement, or live-payout gate — those remain OWNER-gated. NEVER record a
  green flip for a promise whose green criteria require a live event (real
  payment, real external user, signed installer, settled receipt) that has not
  actually occurred. The audit panel records the operating agent as the signoff
  actor; fabricating deservingness is a hard violation.
  - `GET /api/public/training/ablation-derisking-ledger` — live at read over
    the candidate-only training ablation derisking ledger (promise
    `training.ablation_system.v1`, planned) — compliant (`generatedAt`,
    top-level `projection_staleness.v1` `live_at_read` contract, explicit gate
    with `publicProjectionAvailable=true` and `greenGateSatisfied=false`,
    public-safe candidate entries, one-delta manifest-harness evidence, a
    retained checkpoint-eval reproduction receipt, remaining blocker refs, and
    no private training, provider, payment, wallet, or customer material). The
    surface clears the projection, one-delta harness, and eval-reproduction
    blockers; `blocker.product_promises.paid_ablation_dispatch_missing`
    remains. It grants no dispatch, assignment, spend, settlement,
    model-promotion, verdict, or public-claim authority and flips no promise.
    Regression coverage:
    `workers/api/src/training-ablation-derisking-ledger.test.ts`.
  - `GET /api/public/training/post-training-arc/instruct-sft-lane` — live at
    read over the bounded Psionic fixture-scale instruct SFT lane receipt
    (promise `training.post_training_arc.v1`, planned) — compliant
    (`generatedAt`, top-level `projection_staleness.v1` `live_at_read`
    contract, explicit gate with `instructSftLaneAvailable=true`,
    `instructSftPaidDispatchAvailable=false`,
    `preferenceRolloutWorkAvailable=false`,
    `vibeTestArtifactAvailable=false`, and `greenGateSatisfied=false`). The
    surface exposes public-safe refs and digests for the owned chat template,
    assistant-token generation mask, repo-owned example corpus, deterministic
    smoke run, bit-exact resume drill, and committed report fixture synchronized
    with deterministic generator output. It clears
    `blocker.product_promises.instruct_sft_lane_missing` and
    `blocker.product_promises.instruct_sft_fixture_sync_missing`; paid dispatch,
    preference rollout, and vibe-test blockers remain. It grants no assignment, spend,
    settlement, model promotion, model-service, fine-tuning-service, or
    public-claim authority and flips no promise. Regression coverage:
    `workers/api/src/training-post-training-instruct-sft.test.ts`.
  - `GET /api/public/accepted-outcome/settlement/{economicsId}` — live at read
    over one accepted outcome's INERT settlement bundle (promise
    `payments.accepted_outcome_economics.v1`, planned) — compliant
    (`generatedAt`, top-level `projection_staleness.v1` `live_at_read` contract
    re-derived from the source economics row on every read). The surface
    projects the eight ordered settlement states (with honest evidence labels
    and `movedMoney` flags), the contributor accrual ledger, and the
    gross-margin receipt lifecycle, all with internal monetary figures dropped.
    The bundle is built disarmed (`dispatchArmed=false`): producing a complete
    bundle MOVES NO MONEY and is NOT a green flip. It grants no dispatch,
    assignment, spend, settlement, payout, or public-claim authority and flips
    no promise. Regression coverage:
    `workers/api/src/public-accepted-outcome-settlement-routes.test.ts`.
  - `GET /api/public/inference/receipts/{receiptRef}` — live at read over
    `pay_ins.public_receipt_ref` for `receipt.inference.charge.*` and
    `receipt.inference.usd_credit_grant.*` ledger rows — compliant
    (`generatedAt`, top-level `projection_staleness.v1` `live_at_read` contract,
    paid-ledger-row proof only). It exposes the receipt ref, kind, paid state,
    caveats, and source refs while omitting account ids, amounts, idempotency
    keys, Stripe session ids, invoices, preimages, wallet material, provider
    payloads, and raw prompts. Read-only; grants no spend, refund, payout,
    checkout, settlement, provider, public-claim, or registry authority.
    Regression coverage:
    `workers/api/src/public-inference-receipt-routes.test.ts`.
  - `GET /api/public/billing/stripe-checkout-receipts/{receiptRef}` — live at
    read over `stripe_checkout_sessions` plus `billing_ledger_entries` for
    `receipt.billing.stripe_checkout.*` — compliant (`generatedAt`, top-level
    `projection_staleness.v1` `live_at_read` contract, pending/invalid/ok
    resolution). It proves only that a stored Checkout Session is paid and
    fulfilled and that the webhook-created positive Stripe checkout credit row
    exists. It omits customer ids, checkout URLs, email, raw Stripe payloads,
    secrets, ledger ids, invoices, payment material, wallet material, and raw
    account data. Read-only; grants no checkout, spend, refund, payout,
    settlement, provider, public-claim, or registry authority. Regression
    coverage: `workers/api/src/public-stripe-checkout-receipt-routes.test.ts`.
  - `GET /api/public/site-referral-payout-receipts/{receiptRef}` — live at
    read over `site_referral_payout_ledger_entries` for
    `receipt.site_referral_payout.*` settlement evidence refs — compliant
    (`generatedAt`, top-level `projection_staleness.v1` `live_at_read`
    contract, settled-row proof only). It resolves only a settled referral
    payout row that cites the exact public-safe receipt ref, exposing settlement
    state, amount sats, qualifying-event kind, policy refs, caveats, and
    public-safe evidence refs while omitting payout refs, user ids, attribution
    ids, referral source or invite ids, payout destinations, invoices, payment
    hashes, preimages, raw provider payloads, wallet material, and ledger ids.
    Read-only; grants no attribution, invite, checkout, spend, refund, payout,
    settlement, wallet, provider, public-claim, or registry authority.
    Regression coverage:
    `workers/api/src/public-site-referral-payout-receipt-routes.test.ts`.
  - `GET /api/public/partner-payout-receipts/{receiptRef}` — live at read over
    `partner_payout_ledger_entries` for `receipt.partner_payout.*` settlement
    evidence refs — compliant (`generatedAt`, top-level
    `projection_staleness.v1` `live_at_read` contract, settled-row proof only).
    It resolves only a settled partner payout row that cites the exact
    public-safe receipt ref, exposing settlement state, amount, asset,
    qualifying-event kind, policy refs, caveats, and public-safe evidence refs
    while omitting partner refs, user ids, payout refs, qualifying-event refs,
    payout destinations, invoices, payment hashes, preimages, provider payloads,
    wallet material, and ledger ids. Read-only; grants no attribution,
    eligibility, checkout, spend, refund, payout, settlement, wallet, provider,
    revenue, public-claim, or registry authority. Regression coverage:
    `workers/api/src/public-partner-payout-receipt-routes.test.ts`.
  - `GET /api/public/inference/card-credit-spend-receipts/{receiptRef}` — live
    at read over the card-credit paid-loop ledger chain for
    `receipt.inference.card_credit_spend.*` — compliant (`generatedAt`,
    top-level `projection_staleness.v1` `live_at_read` contract,
    pending/invalid/ok resolution). It reads the Stripe checkout credit row,
    the card-origin `usd_credit_grant` row, and future inference charge rows
    with served-model/token context. Pending means the paid loop is not proven;
    invalid means the stored chain violates conservation/provenance. Read-only;
    grants no checkout, spend, refund, payout, settlement, provider,
    public-claim, or registry authority. Regression coverage:
    `workers/api/src/public-card-credit-spend-receipt-routes.test.ts`.
  - `GET /api/public/home` — static discovery document, exempt (not a state
    projection).
  - `GET /api/public/product-promises` — live at read over the versioned
    product-promise registry — compliant (`generatedAt`, `registryVersion`,
    `maxStalenessSeconds`, top-level `projection_staleness.v1`
    `live_at_read` contract, and blocker/evidence verification summary). It
    grants no write, deploy, spend, settlement, or public-claim authority and
    flips no promise by itself.
  - `GET /api/public/product-promises/transitions` — stored receipt rows,
    served with live registry context — compliant (`generatedAt`,
    `registryVersion`, `registryGeneratedAt`, and the top-level
    `projection_staleness.v1` `live_at_read` contract; receipt rows remain
    transition evidence only and do not mutate registry state).
  - `GET /api/public/proof/otec` — stored snapshot — NON-COMPLIANT (no
    freshness fields at all).
  - `GET /api/public/pylon-stats` — live at read — NON-COMPLIANT
    (`asOfUnixMs` + `counterWindows` from #4735, but no declared
    `maxStaleness` contract).
  - `GET /api/public/pylon-capacity-funnel` — live at read — NON-COMPLIANT
    (`generatedAt` only).
  - `GET /api/public/pylon-capacity-funnel/history` — stored snapshots —
    NON-COMPLIANT (`generatedAt` + per-snapshot times, no declared bound).
  - `GET /api/public/launch-dashboard` — live at read — NON-COMPLIANT
    (`generatedAt` only; internal 10-minute freshness gate is undeclared).
  - `GET /api/public/treasury/launch-status` — live at read — NON-COMPLIANT
    (no freshness fields).
  - `GET /api/public/treasury` — live proxy — NON-COMPLIANT.
  - `GET /api/public/artanis/admin-ticks` — live at read — NON-COMPLIANT.
  - `GET /api/public/nexus-pylon/receipts/{receiptRef}` — stored receipt
    projection — NON-COMPLIANT (per-receipt times only).
  - `GET /api/public/nip90-market/receipts/{receiptRef}` — stored receipt
    projection — NON-COMPLIANT.
  - `GET /api/public/adjutant/activity` — live at read — NON-COMPLIANT
    (per-item `updatedAt` only).
  - `GET /api/public/goals/{goalId}`, `/api/public/goals/{goalId}/snapshot`,
    and `/api/public/agents/{agentRef}/goal` — live at read — NON-COMPLIANT.
  - `GET /api/forum/launch-status` — live at read — NON-COMPLIANT.
  - `GET /api/forum/receipts/{receiptRef}` — live at read over pay-ins and
    receipts — NON-COMPLIANT (no payload-level declaration).
  - `GET /api/training/...` window/leaderboard/eval surfaces
    (`training-run-window-routes.ts`) — mixed — NON-COMPLIANT (file owned by
    an in-flight lane; retrofit owed on #4751).
  - `GET /api/openapi.json` — static contract document — exempt from the
    payload rule, but its route inventory must track shipped routes (#4752,
    file owned by an in-flight lane).
- Regression coverage for this policy lives in
  `workers/api/src/public-projection-staleness.test.ts`,
  `workers/api/src/artanis-public-report.test.ts`,
  `workers/api/src/forum/tip-leaderboards-staleness.test.ts`,
  `workers/api/src/forum-routes.test.ts`,
  `workers/api/src/x-claim-reward-eligibility-routes.test.ts`, and the
  ledger/ratchet in `scripts/check-zero-debt-architecture.mjs`.

## Customer #1 Cohort Public Projection

- Customer #1 cohort source rows are private operator evidence by default.
  Public surfaces may receive only an explicit projection that emits opaque
  cohort refs, generic display labels, state, counts, blocker refs, caveat refs,
  `generatedAt`, and the shared public-projection staleness contract.
- Private Customer #1 cohort row intake and storage must stay behind the
  operator/admin boundary at `/api/operator/customer-one-cohort/rows`. Stored
  rows are source evidence only; the public route may expose only the
  `CustomerOneCohortProjection` output.
- The Customer #1 cohort projection must reject raw prompts, shell logs, private
  repo or local filesystem refs, URLs, email addresses, provider payloads,
  provider secrets, wallet material, payment hashes/preimages, invoices, and
  customer private data before schema stripping can hide unknown fields.
- A `loop_completed` cohort row counts toward D3 completion only when it has
  both a completion-bundle ref and a privacy-review ref. Missing evidence must
  emit blocker refs rather than silently count, and an empty or partial cohort
  must not fabricate Customer #1 completion progress.
- The projection is `evidence_only`. It must not grant runtime authority,
  deployment authority, merge authority, accepted-work authority, payout or
  settlement authority, provider authority, or a broad public customer-success
  claim.
- Regression coverage for this policy lives in
  `workers/api/src/customer-one-cohort-projection.test.ts`.

## Mullet Simulation Runner Authority

- The `/mullet` surface and `/api/mullet/*` routes are private operator-only
  simulation tools for the confirmed `chris@openagents.com` account.
- The misspelled `chris@openaegnts.com` account is not an authority and must
  not appear in runtime allowlists, tests as an accepted user, seed data, or
  documentation except as a denied typo note.
- Mullet scenarios, simulation runs, candidate-mode records, dispatch outputs,
  proof references, energy telemetry references, market-memory records, and
  exports are private simulation evidence only.
- Mullet records must not authorize live Pylon assignment, provider mutation,
  wallet spend, invoice payment, Bitcoin settlement, accepted-work closeout,
  public claim promotion, Forum posting, deployment, email sends, or other
  production side effects.
- Mullet modeled, measured, verified, accepted, paid, and settled states are
  separate claims. A modeled scenario must not become measured energy,
  accepted work, payable work, or settled payout without matching evidence refs
  from the appropriate runtime authority.
- Browser route gating is not sufficient authority. Every `/api/mullet/*`
  handler must require a browser session and repeat the server-side email
  allowlist check.
- Mullet exports are private by default and must reject raw prompts, raw
  traces, customer data, private artifacts, private repo refs, wallet material,
  payment preimages, invoices, provider secrets, raw logs, and raw timestamps.
- Regression coverage begins in `workers/api/src/admin-access.test.ts` for the
  confirmed operator email and denied typo. Route, API, export, and redaction
  coverage must be added with the implementation slices that introduce those
  surfaces.
