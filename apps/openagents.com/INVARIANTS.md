# INVARIANTS

This is the invariant ledger for `openagents`.

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
- Release credits the provider balance and debits the requester held claim
  exactly once. Refund releases the hold without debiting the requester.
  Release-after-refund, refund-after-release, double-release, and
  double-refund must not move balances.
- Reserve, release, and refund each require public-safe receipt rows carrying
  refs and amounts only. Escrowed or credited amounts are not settled bitcoin
  until the later payout path records settlement evidence.
- Regression coverage for this policy lives in
  `workers/api/src/artanis-labor-requester.test.ts`,
  `workers/api/src/forum-routes.test.ts`,
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

## Provider Capacity Marketplace Gate

- ChatGPT/Codex account connection is not resale authorization and must not be
  described as a live subscription-capacity marketplace by itself.
- Provider capacity monetization claims are provider-specific. ChatGPT/Codex is
  the first modeled provider; unsupported prepaid providers must remain planned or blocked
  until provider schemas, secret handling, route policy, metering, pricing, ToS
  boundaries, dispatch, assignment receipts, and settlement receipts exist for
  that provider.
- Provider tokens, raw quota payloads, subscription cookies, provider-account
  grants, raw metering, raw pricing, payment material, wallet material, customer
  data, and timestamps must not enter public refs.
- Pricing must distinguish agentic work or accepted outcomes from base
  inference resale. Base inference resale remains blocked unless a future
  policy explicitly authorizes it with tests.
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
- Regression coverage for this policy lives in
  `workers/api/src/provider-capacity-marketplace-gate.test.ts`.

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
