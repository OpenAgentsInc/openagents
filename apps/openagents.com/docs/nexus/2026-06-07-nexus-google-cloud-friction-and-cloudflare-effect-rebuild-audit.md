# Nexus Google Cloud friction and Cloudflare/Effect rebuild audit

Date: 2026-06-07

## Executive summary

The current Nexus production path is operational, but it is too hard for an
agent to reason about, repair, and prove quickly. The hard parts are not one
single bug. They are a stack of operational surfaces that compound:

- local `gcloud` account state can block production inspection;
- build correctness depends on a manually curated reduced Cargo build context;
- Cloud Build is slow enough that every failed build or deploy loop consumes
  meaningful operator time;
- deployment relies on IAP SSH into a VM, Docker, systemd units, preserved env
  files, and Cloudflare Tunnel health;
- public availability is a Cloudflare edge/tunnel problem even though the
  service is deployed on Google Compute Engine;
- treasury continuity requires both app-level ledger logic and remote
  operator commands against stateful VM storage;
- smoke scripts do not yet distinguish "service deployed correctly but known
  ledger cleanup still pending" from "deploy failed";
- the production proof path is split across local repo receipts, Google Cloud
  build metadata, VM state, public HTTP endpoints, GitHub issues, and operator
  terminal output.

That split makes Nexus hard to run as an agent-administered system. It can be
made safer incrementally, but the current Google Cloud piece is the biggest
source of operational drag in this particular run.

The strongest case for rebuilding part of Nexus on the same Effect and
Cloudflare stack as OpenAgents product surface is not "Cloudflare is simpler in every way." The
case is that OpenAgents product surface already has a typed, D1-backed, Worker-deployed,
agent-visible control plane. Moving the control-plane and proof-plane pieces
of Nexus toward that stack would reduce the number of privileged shells, local
cloud credentials, VM-only state files, and bespoke smoke paths an agent must
manage. It would also put Nexus closer to the Forum, Sites, provider account
fleet, agent tokens, transactional email, public receipts, and customer/order
surfaces that are already becoming the OpenAgents product.

The strongest warning is that Cloudflare Workers are not a drop-in replacement
for every Nexus/Pylon runtime concern. Native LDK, long-running node daemons,
GPU/inference/training workloads, and host resource control still need a
native runtime, a container runtime, or external machines. The likely good
architecture is therefore hybrid:

- OpenAgents product surface/Cloudflare owns Nexus control, receipts, scheduling, agent-facing
  APIs, public proofs, payout intents, and operator UX.
- Native Pylons and any LDK/MDK host processes execute workloads and wallet
  operations at the edge of the network.
- A smaller amount of Google Cloud or other infrastructure may remain for
  transitional relay services, but it should not be the primary admin surface.

After inspecting the current OpenAgents product surface implementation, the rebuild case is more
specific than "Cloudflare has useful primitives." OpenAgents product surface already contains
D1-backed ledgers and Effect Schema contracts for buyer payments, Site payment
catalogs, hosted MDK checkout, L402 credentials, Site MDK reconciliation,
Artanis persistence, Nexus/Pylon adapter dispatches, and Pylon marketplace job
intake. The gap is not the absence of a payment/control plane. The gap is that
those OpenAgents product surface surfaces intentionally stop at evidence, projection, and proposal.
They do not yet hold live wallet spend authority, provider payout authority, or
accepted-work settlement authority.

That means Nexus functionality can plausibly move into OpenAgents product surface earlier than a
blank-slate design would suggest. The right move is to add a narrow
MDK-backed payment authority service and payout-intent ledger to OpenAgents product surface, then
route Nexus/Treasury/Pylon settlement through it. The wrong move would be to
reuse the buyer-side payment ledger as if it were already provider payout
truth. The existing code explicitly says it is not.

Later 2026-06-07 update: the MDK checkout sidecar path no longer needs the
old Google Cloud plan. OpenAgents product surface deployed a Cloudflare Container sidecar for
`@moneydevkit/core`, bound it to the Worker through `MDK_SIDECAR`, provisioned
a corrected MDK app binding for `https://openagents.com`, and proved a live
100-bitcoin-sat checkout all the way to provider status `PAYMENT_RECEIVED`.
The current conclusion is sharper:

- plain Workers alone are not enough for the present MDK checkout runtime;
- Workers plus Cloudflare Containers are enough for the checkout sidecar;
- GCP should not be reintroduced for this checkout lane; and
- any remaining GCP work should be treated as old Nexus/native-runtime
  transition context, not the default OpenAgents product surface payment architecture.

## Scope of this audit

This audit was written during the 2026-06-07 Pylon v0.2 / Artanis / Nexus
release work. The immediate production situation was:

- A Nexus source fix had been committed and pushed in the `openagents` repo.
- A Nexus image had been built in Google Cloud Build and deployed to
  `nexus-mainnet-1`.
- The deploy successfully started the service with the new Docker image.
- Public Cloudflare Tunnel availability had a startup edge failure and the
  public watchdog restarted `nexus-cloudflared`.
- The service still showed `continuity_alert:confirmations_stalled` before the
  new payout-ledger cleanup command was run.
- A separate local MDK test wallet had received real bitcoin and was ready for
  a movement proof.
- The ChatGPT/Codex account fleet in OpenAgents product surface had been expanded and proved with
  seven connected, healthy accounts and a parallel sanity probe.

The audit focuses on the Google Cloud/Nexus operational lane and whether an
Effect/Cloudflare rebuild could reduce friction. It does not claim to finish
the Nexus cleanup or Pylon release by itself.

Additional OpenAgents product surface source inspected for this update:

- `workers/api/migrations/0114_buyer_payment_ledger.sql`
- `workers/api/migrations/0115_site_payment_catalog.sql`
- `workers/api/migrations/0117_agent_search_payments.sql`
- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`
- `workers/api/src/buyer-payment-ledger.ts`
- `workers/api/src/hosted-mdk-client.ts`
- `workers/api/src/l402-credential-service.ts`
- `workers/api/src/mdk-core-checkout-contract.ts`
- `workers/api/src/site-mdk-reconciliation.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`

The code-level finding is consistent across those files: OpenAgents product surface has strong
typed contracts, redaction, D1 storage, idempotency, public-safe projections,
and fake/config-gated provider boundaries. It does not yet have production
MDK webhook verification, accepted-work payout settlement, provider payout
dispatch, or live Artanis Pylon dispatch authority. It now has production MDK
checkout-sidecar reachability and one real 100-bitcoin-sat checkout/payment
smoke through Cloudflare.

## Confirmed friction from the current run

### 1. Local Google Cloud auth is a hidden hard dependency

Production inspection was blocked earlier because local `gcloud` credentials
were expired. That meant the agent could not inspect VM state even though the
service itself was public and the repo had source-level tests. The user had to
reauth `gcloud` before deeper production inspection could continue.

That is not acceptable for an autonomous overnight operator path. If the
agent's ability to diagnose the live control plane depends on a human
refreshing local Google auth, the system is not yet truly agent-operable.

Symptoms:

- VM-local inspection was unavailable until `gcloud` was reauthenticated.
- The issue could not be fully classified from public endpoints alone.
- The source fix was initially derived from local reasoning and tests, then
  revised after VM inspection exposed the legacy synthetic LDK payment id.

Required improvement if Google Cloud remains:

- Noninteractive deploy and inspection credentials need a documented,
  least-privilege service account path.
- The agent should not depend on a human's short-lived `gcloud` login for
  routine health checks, logs, build status, VM command execution, or rollback.
- Any credential path must be explicit, revocable, scoped, and redacted in
  logs and GitHub comments.

### 2. The build context is fragile

Nexus uses a reduced Docker build context for speed. That is reasonable, but
it introduced a real failure mode: the build-context lockfile was missing a
package needed by the Nexus image. A previous build failed until
`apps/nexus-relay/deploy/Cargo.nexus.lock` was refreshed to include the needed
crate.

The local repo could compile, but the reduced Cloud Build context could still
fail. That means "cargo test passed locally" is not enough evidence that the
Nexus image can build.

Symptoms:

- A Cloud Build failed because the reduced context was incomplete.
- The fix required a separate lock/context maintenance step.
- The failure was discovered only after spending build time.

Required improvement if Google Cloud remains:

- Add a local "reduced Nexus context check" that validates the Docker context
  before submitting Cloud Build.
- Add a CI job that builds the Nexus reduced context on every relevant PR or
  main commit.
- Make the reduced context generator the source of truth rather than hand
  editing the lock file.
- Treat any dependency or package graph change as a build-context invariant.

### 3. Cloud Build latency makes every mistake expensive

The current Cloud Build path is functional, but slow. Even with cache, the
build path spends real time transferring context, pulling/pushing layers,
downloading Rust dependencies, compiling, exporting image layers, and exporting
cache. That is normal for a Rust Docker image, but it is painful during an
interactive release gate.

Symptoms:

- Each source fix requires a long build/push cycle before production can be
  tested.
- A failed build or post-deploy failure costs a large fraction of a working
  session.
- The agent has to juggle background build polling while handling other tasks.

Required improvement if Google Cloud remains:

- Keep a fast local or remote preflight that catches the same class of errors
  before Cloud Build submission.
- Split small control-plane fixes from large runtime images where possible.
- Record build receipts automatically and link them to issue closeout.
- Make build status queryable through a first-party operator API rather than
  through local terminal state.

### 4. Deploy is too many layers for an agent to audit quickly

The deploy path touches:

- local shell environment;
- `gcloud` project, region, and zone configuration;
- IAP SSH;
- VM packages;
- Artifact Registry auth;
- Docker image pull;
- systemd service files;
- systemd timers;
- preserved env files;
- mounted persistent directories;
- Cloudflare Tunnel;
- public HTTP smoke;
- treasury and public watchdogs.

That is a lot of moving pieces for an agent to hold in working memory. It also
creates many places where output can be noisy without being decisive.

Symptoms:

- Deploy logs include repeated IAP warnings about NumPy upload performance.
- The service can be active locally while public reachability is still broken.
- Public Cloudflare edge `000` during startup required tunnel restart.
- The deploy script's final status remained tied to treasury degradation that
  was known to require a separate cleanup.

Required improvement if Google Cloud remains:

- Produce a compact deploy receipt with normalized phases:
  `image_pulled`, `service_started`, `local_health_ok`, `public_health_ok`,
  `treasury_status`, `watchdog_status`, `known_deferred_actions`.
- Make every phase machine-readable and redacted.
- Separate "deploy failed" from "deployed but business-state cleanup pending."
- Push deploy receipts into a first-party API and docs location, not just a
  local JSON file in a repo checkout.

### 5. Public availability is already a Cloudflare problem

Although Nexus is hosted on a Google VM, public access goes through
Cloudflare. In this run, the public watchdog observed a startup public-edge
failure and restarted `nexus-cloudflared`. That is useful, but it also proves
that the public availability story is already partly Cloudflare-owned.

The current split is awkward:

- Google Cloud owns the VM and image deploy.
- Docker/systemd owns process management.
- Cloudflare Tunnel owns public reachability.
- Nexus code owns public health.
- OpenAgents product surface owns many public product surfaces.

Required improvement if Google Cloud remains:

- Treat Cloudflare Tunnel as a first-class production dependency, not a sidecar
  detail.
- Store public reachability watchdog receipts as durable operational events.
- Expose a public-safe, redacted "last tunnel recovery" status.
- Decide whether the tunnel should be transitional infrastructure or a
  permanent boundary.

### 6. Treasury continuity mixes product state and VM-only state

The immediate treasury problem was not just a source bug. It had layers:

- source reconciliation missed tracked live LDK payment lookup;
- production still had one legacy synthetic LDK payment id that the provider
  could not query;
- cleanup had to distinguish stale unqueryable legacy rows from fresh or
  valid 64-hex provider payment ids;
- verification required public treasury status, admin refresh, provider
  operation checks, and VM-local inspection.

The production state is currently too tied to VM-local storage and operator
commands. That makes it harder to expose cleanly to OpenAgents product surface, Forum, Artanis, or
agent-facing APIs.

Required improvement if Google Cloud remains:

- Move more treasury diagnostics into typed, redacted public/operator APIs.
- Add an explicit "ledger cleanup candidate" projection before cleanup.
- Require cleanup commands to emit receipts that can be linked to issues.
- Make cleanup idempotent, dry-run-first, and bounded by policy.

### 7. Issue closeout depends on human synthesis

The GitHub issues around Nexus/Pylon cleanup are meaningful, but proof is
scattered. Closing an issue requires the agent to manually synthesize:

- tests run locally;
- commits pushed;
- image build receipts;
- deploy state;
- public endpoint responses;
- VM command output;
- wallet proof output;
- GitHub issue comments.

That is fragile. It also increases the chance of issue comments getting
formatted badly or omitting the actual acceptance evidence.

Required improvement:

- Define a release-gate receipt schema.
- Generate issue-comment Markdown from the receipt, using `--body-file` or
  stdin.
- Make the receipt include exact commands, timestamps, hashes, redacted
  outputs, and pass/fail status.
- Keep raw secrets and raw payment material out of the receipt.

## Why this is especially painful for Artanis

Artanis is supposed to administer Nexus/Pylon work in an automated fashion.
That includes coordination through the Forum, assignment of work to Pylons,
continual learning loops, and proof of paid useful work.

The current Google Cloud operational surface is poorly matched to that
mission:

- Artanis cannot rely on local `gcloud` auth if it is supposed to run
  continuously.
- Artanis should not need to SSH into a VM just to classify a ledger row.
- Artanis needs durable, public-safe operational events it can post to the
  Forum or cite in GitHub.
- Artanis needs clear authority boundaries before it can restart services,
  run cleanup, pay bitcoin, or accept work.
- Artanis needs to separate "network runtime is unhealthy" from "business
  ledger state needs cleanup."

In short, Artanis needs Nexus to be more like a typed product surface and less
like a one-off infra shell.

## What OpenAgents product surface already has

This audit originally framed the rebuild mostly in terms of Cloudflare product
fit. That was too generic. The more important fact is what OpenAgents product surface has already
implemented.

### Buyer and Site payment foundations

`workers/api/migrations/0114_buyer_payment_ledger.sql` defines a real D1 buyer
payment ledger with challenges, receipts, entitlements, redemptions, spend
limits, credit debits, and reconciliation events. The corresponding
`buyer-payment-ledger.ts` domain layer enforces idempotency, amount shape,
audience projection, and secret redaction.

That ledger is intentionally buyer-side evidence. Its docs and code reject
raw invoices, payment preimages, wallet material, MDK credentials, provider
payloads, and customer private data. It also explicitly does not settle
provider payouts or record accepted-work payout truth.

`workers/api/migrations/0115_site_payment_catalog.sql` adds Site payment
catalog items with `settlement_mode` values such as `accepted_work_linked`,
`checkout_only`, and `deferred`. That is directly relevant to e-commerce agent
Sites because the product catalog already knows whether a Site payment is just
a checkout or should be linked to accepted work later.

`workers/api/src/hosted-mdk-client.ts` defines a hosted MDK checkout boundary.
It accepts payment challenges and product records and can return checkout,
invoice, and payment-hash refs. The important hard-coded fields are:

- `provider: mdk_hosted`
- `settlementAuthority: buyer_payment_evidence_only`
- `providerPayoutAuthority: false`
- `acceptedWorkSettlementAuthority: false`

So OpenAgents product surface has an MDK checkout contract, but it is not yet an MDK payout
authority.

`workers/api/src/mdk-core-checkout-contract.ts` adds a Worker-compatible
checkout preparation and signed checkout URL contract. It sanitizes customer
data, metadata, checkout paths, and signing inputs. That can be reused for
real MDK checkout and commerce flows, but it does not itself send bitcoin or
verify money movement.

`workers/api/src/l402-credential-service.ts` provides a Worker-compatible
L402 credential payload, HMAC signing boundary, verification result model, and
audience-safe projection. It is useful for paid API recovery, agent rate-limit
recovery, Forum paid actions, and generated Site paid actions. Its own
non-goals are important: it does not create real MDK invoices, verify real
payment preimages, settle payments, or grant provider payout claims.

`workers/api/src/site-mdk-reconciliation.ts` adds a Site MDK reconciliation
projection for hosted checkout provider events. It maps fake or config-gated
provider events into buyer-payment reconciliation outcomes and again sets:

- `payoutAuthority: false`
- `acceptedWorkSettlementAuthority: false`

That is the right boundary for customer checkout status. It is not enough for
Nexus provider settlement.

### Agent search and paid action foundations

`workers/api/migrations/0117_agent_search_payments.sql` adds payment challenge,
receipt, entitlement, and redemption tables for agent search. Today those are
credit-denominated rather than live bitcoin, but the shape is important:
programmatic agent APIs already have an economic-limit recovery model with
idempotency and receipts.

This matters for Nexus because the same pattern can be reused for agent
workloads that exceed free limits, request paid Pylon jobs, or need to unlock
additional search/inference capacity without a human manually editing database
state.

### Artanis, Nexus, and Pylon foundations

`workers/api/migrations/0119_artanis_persistence.sql` adds D1 persistence for
Artanis runtime snapshots, loops, ticks, approval gates, health snapshots,
work-routing proposals, and Forum publication intents. This is already the
right shape for an automated supervisor that posts and reasons over durable
state instead of terminal logs.

`workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql` adds
D1 storage for Artanis Nexus/Pylon adapter dispatch records.
`workers/api/src/artanis-nexus-pylon-adapters.ts` then models fleet snapshots
and dispatch records. The current authority block is deliberately conservative:

- fake dispatch can be recorded;
- read-only fleet monitoring is allowed;
- work dispatch proposal is allowed;
- live Pylon job dispatch is denied;
- payment spend is denied;
- wallet spend is denied;
- settlement mutation is denied;
- training launch, deployment, provider mutation, and runtime promotion are
  denied.

This is exactly the right pre-authority contract. It lets Artanis reason and
publish safely. It does not yet let Artanis administer a live Pylon economy.

`workers/api/migrations/0121_pylon_marketplace_jobs.sql` adds persisted Pylon
marketplace job intakes, assignments, and triage actions.
`workers/api/src/pylon-marketplace-jobs.ts` defines job kinds such as
inference, training, LoRA fine-tuning, benchmark evaluation, and GEPA/DSPy
optimization. It also models assignment and payout state, but
`PYLON_MARKETPLACE_NO_SPEND_AUTHORITY` blocks buyer charge mutation, paid
assignment dispatch, payout mutation, and settlement mutation.

The marketplace contract is therefore ready for intake, triage, proposal, and
public-safe projection. It is not ready for autonomous paid dispatch until a
separate authority service exists.

`workers/api/src/pylon-settlement-bridge.ts` is especially relevant. It
already separates assignment, capability snapshot, wallet readiness, buyer
payment evidence, accepted work, reward intent, payout eligibility, payout
dispatch, payout confirmation, payout verification, and settlement. It also
hard-codes `OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY`, which
denies live wallet spend, payout dispatch, payout target mutation, buyer charge
mutation, and settlement mutation.

That bridge should not be bypassed. It is the correct starting point for Nexus
settlement visibility. A new payment authority should write safe payout refs
into this bridge after a real payout happens.

`workers/api/src/pylon-wallet-liquidity-readiness.ts` models spendable
onchain funds, inbound/outbound liquidity, anchor reserve, channel balance,
send readiness, and receive readiness. It is read-only and rejects raw wallet
state, invoices, preimages, payout targets, provider secrets, and raw
liquidity telemetry. This is directly relevant to MDK edge wallets for Pylons:
OpenAgents product surface can decide whether a Pylon appears ready to receive or send, but cannot
yet mutate the wallet.

## What OpenAgents product surface does not have yet

After inspecting the implementation, the missing pieces are concrete:

- no production MDK wallet adapter that can create real invoices, send bitcoin,
  list payments, and verify settlement through a funded wallet;
- no OpenAgents product surface-side treasury payout-intent ledger for accepted work;
- no authority service that can convert accepted work plus buyer payment
  evidence into a bounded payout attempt;
- no durable record linking MDK payment attempts to Nexus/Treasury/Pylon
  receipt refs;
- no live MDK webhook verification or replay storage beyond the
  fake/config-gated Site reconciliation contract;
- no Artanis authority upgrade from proposal/fake-dispatch to live dispatch;
- no API that lets Pylons report MDK wallet readiness or payment receipts as
  first-party OpenAgents product surface records;
- no release gate requiring a real buyer-to-OpenAgents product surface and OpenAgents product surface-to-Pylon bitcoin
  movement proof before announcing payment-backed Pylon work.

Cloudflare runtime choices still matter, but they are secondary to this
authority gap. Workers, D1, Queues, Workflows, R2, and Durable Objects can
host the ledgers and coordination. They do not automatically create wallet
authority, payout policy, key custody, or live-money proof.

The remaining non-OpenAgents product surface runtime problems are still real:

- running a native LDK or MDK wallet daemon in the exact form it exists today;
- GPU workloads;
- long-lived high-resource training processes;
- local machine resource controls;
- arbitrary Docker/systemd management;
- direct replacement of a Pylon host.

That implies a split architecture, not a full rip-and-replace.

## Could Nexus payment authority use MDK from OpenAgents product surface?

Yes, with a new service boundary. The current code strongly suggests the right
shape:

- Keep Pylons as MDK edge agent wallets. Pylons should own their local wallet
  process, resource mode, receive readiness, and local payment receipt
  reporting.
- Add an OpenAgents product surface-side `TreasuryPaymentAuthority` or
  `OpenAgentsMdkPayoutAuthority` Effect service for OpenAgents-controlled
  treasury payouts.
- Keep wallet secret material out of D1, issue comments, public docs, Forum
  posts, and public projections.
- Store only payout intents, idempotency keys, safe refs, redacted payment
  refs, reconciliation statuses, spend caps, operator approvals, and public
  receipt refs in OpenAgents product surface.
- Support multiple adapters behind the same interface:
  `simulation`, `mdk_hosted`, `pylon_mdk_edge`, and
  `nexus_ldk_legacy`.

The service interface should be narrow:

- `previewPayout(intent)` returns policy, balance/readiness, and caveats
  without spending.
- `dispatchPayout(intent, idempotencyKey)` performs a bounded spend only when
  all approval, spend cap, accepted-work, and payout-target checks pass.
- `readPayment(paymentRef)` reads a redacted payment status.
- `reconcilePayment(paymentRef)` records confirmation/settlement evidence.
- `projectPaymentReceipt(audience)` emits public, agent, customer, team, and
  operator-safe projections.

MDK is directly relevant because the agent wallet can initialize a local
wallet, create invoices, send payments to BOLT11/BOLT12/LNURL/Lightning
address destinations, and report balances and payment history as JSON. That
fits both sides of the system:

- Pylon-local MDK wallets can receive work payouts and later pay for services,
  Forum rewards, or other agent actions.
- An OpenAgents-controlled MDK wallet can potentially serve as the treasury
  executor for small bounded payouts, assuming key custody, backup, spend caps,
  idempotency, monitoring, and operator override rules are explicitly built.

The caution is that OpenAgents product surface should own policy and receipts, not casually become
a wallet by accident. If MDK payout functionality is used from OpenAgents product surface, it must
be behind a deliberate authority boundary with:

- per-surface and per-agent spend caps;
- dry-run previews;
- idempotency keys;
- operator approval gates for risky or first-time payout targets;
- redacted receipts;
- replay-resistant reconciliation;
- balance/readiness checks;
- emergency pause and revocation;
- testnet/signet/sandbox and simulation modes;
- a production wallet backup and rotation policy.

This would let Nexus functionality move toward OpenAgents product surface without pretending that
the existing buyer payment ledger is enough. Buyer ledger evidence would
become one input to payout policy. Accepted work, Pylon assignment receipts,
Treasury receipt refs, wallet readiness, and spend authority would remain
separate inputs.

## Candidate Cloudflare/Effect architecture

### Control plane in OpenAgents product surface

OpenAgents product surface should own the public and operator control plane:

- Pylon registration;
- Pylon capability declarations;
- Pylon MDK wallet readiness projections;
- Pylon MDK payment receipt ingestion;
- Artanis job creation;
- assignment lifecycle;
- Forum posting and coordination;
- public proof pages;
- payout intent records;
- payout attempt records;
- payout reconciliation records;
- receipt records;
- customer/operator notifications;
- release gates;
- audit exports;
- agent API tokens and scopes.

Implementation shape:

- Worker routes for agent/operator APIs.
- Effect services for state transitions, redaction, receipts, and policy.
- D1 tables for durable ledgers.
- A new MDK payout authority service with simulation, hosted MDK,
  Pylon-local MDK, and legacy Nexus LDK adapters.
- R2 for larger artifacts, run bundles, build/deploy receipts, screenshots,
  model outputs, and signed proof bundles.
- Durable Objects for per-run/per-Pylon coordination, leases, WebSockets, and
  strongly consistent state transitions.
- Queues/Workflows for retryable background work, cleanup, notifications, and
  human-in-the-loop wait states.

### Native Pylon runtime outside OpenAgents product surface

Pylons should remain native where they need native capabilities:

- machine resource accounting;
- local GPU/inference/training execution;
- model artifact movement;
- MDK edge agent wallet operations;
- any retained LDK wallet daemon operations during transition;
- local filesystem and process supervision;
- host-level networking.

But they should talk to OpenAgents product surface through typed, scoped APIs instead of relying on
Google VM SSH as the coordination path.

Implementation shape:

- Pylon agent runs locally or on a host.
- It registers with OpenAgents product surface and receives scoped work assignments.
- It emits heartbeats, capability snapshots, work receipts, and wallet
  readiness proofs.
- It emits redacted MDK payment receipt refs after receiving payouts or making
  authorized payments.
- It accepts work only through signed/scoped assignment grants.
- It posts status to Forum through the agent Forum API where appropriate.

### Transitional Nexus relay

The existing Nexus relay can stay during transition, but its role should
shrink:

- keep current production bridge alive;
- continue serving existing public health endpoints;
- continue any LDK-specific runtime that cannot immediately move;
- publish legacy payout and Treasury state as redacted OpenAgents product surface receipt refs;
- expose typed state to OpenAgents product surface rather than requiring SSH.

The long-term goal should be that Artanis can administer Nexus through OpenAgents product surface
APIs and Pylon APIs, not through a Google VM shell.

## Migration options

### Option A: Harden Google Cloud only

This keeps the architecture mostly as-is and fixes the worst operational gaps.

Work required:

- noninteractive service-account auth for build/deploy/inspect;
- CI check for the reduced Docker build context;
- deploy receipt schema;
- remote command receipts;
- public watchdog receipts;
- treasury cleanup dry-run/apply receipts;
- operator dashboard in OpenAgents product surface that reads Nexus status;
- GitHub issue comment generator.

Pros:

- least architectural change;
- preserves current Rust services and VM runtime;
- fastest route to stabilizing Pylon v0.2 if the remaining blockers are small.

Cons:

- still depends on Google VM, IAP, Docker, systemd, and tunnel;
- still leaves Artanis with a complicated infra shell unless heavily wrapped;
- still has slow build/deploy cycles for small control-plane fixes;
- still separates Nexus operational truth from OpenAgents product surface product truth.

Assessment:

This is necessary short term, but insufficient as the long-term OpenAgents
agent-admin architecture.

### Option B: Move Nexus control/proof plane to OpenAgents product surface, keep native runtime

This is the recommended direction.

Work required:

- extend the existing OpenAgents product surface Pylon ledgers instead of starting from scratch:
  `pylon_marketplace_job_intakes`, `pylon_marketplace_assignments`,
  `artanis_nexus_pylon_adapter_dispatches`, and Artanis persistence already
  exist;
- add D1 tables for treasury payout intents, payout attempts, payout
  reconciliation events, spend caps, payout target approvals, and MDK adapter
  receipts;
- add Effect services for Pylon registration, assignments, status, receipts,
  payout intents, and payout authority;
- add an MDK payout adapter behind the authority interface, initially in
  simulation and then with a small funded production wallet;
- add agent/operator APIs for Artanis;
- add Durable Objects for per-assignment coordination and live progress;
- add Queues/Workflows for retryable reconciliation and cleanup;
- make existing Nexus relay publish/import state to OpenAgents product surface;
- preserve native Pylon execution and wallet operations where needed.

Pros:

- aligns with the existing OpenAgents website, Forum, Sites, email, and agent
  API surfaces;
- reduces SSH/operator-shell dependency;
- gives Artanis a typed API surface;
- improves public proof and customer/operator visibility;
- lets work receipts and issue closeout become first-party product records.

Cons:

- requires careful data migration and dual-write/dual-read period;
- does not eliminate native runtime needs;
- requires new security policy for agent/operator authority;
- may reveal gaps in current OpenAgents product surface D1 schema and redaction policy.

Assessment:

This is the best medium-term architecture. It acknowledges that Pylons remain
native while moving coordination, policy, proof, and public visibility into the
Effect/Cloudflare product system.

### Option C: Full Cloudflare replacement

This would attempt to move almost everything into Cloudflare primitives.

Pros:

- one primary platform;
- simplest conceptual public product surface;
- fewer Google-specific deploy credentials.

Cons:

- likely unrealistic for native LDK/MDK daemon needs in the current form;
- likely unrealistic for GPU/training workloads;
- risks rebuilding working Rust runtime pieces prematurely;
- may overfit to serverless control-plane strengths and underfit to host
  compute needs.

Assessment:

Do not choose this as the immediate plan. It is only plausible after the
native runtime boundary is sharply defined and Cloudflare Containers or other
runtime options are evaluated against the concrete Pylon/LDK/MDK needs.

## Revised OpenAgents product surface rebuild roadmap

This section supersedes the earlier Google Cloud cleanup-first plan. The old
Google Cloud Nexus lane is now legacy production context, not the Pylon v0.2
release lane. Do not create a standalone public Nexus v0.2 release from the
Google Cloud service. Do not spend the next implementation cycle hardening
Google Cloud deploy mechanics unless a narrow migration export requires it.

The target is now an OpenAgents product surface-owned Nexus control plane:

- OpenAgents product surface owns assignment state, payment authority policy, payout intents,
  receipts, release gates, Artanis coordination, public-safe proof, and
  operator visibility.
- Pylons remain native edge workers that run workloads and operate MDK edge
  agent wallets.
- MDK is wrapped deliberately. Pylon-local MDK wallets handle edge wallet
  behavior, while OpenAgents product surface/Nexus gets an MDK-backed treasury payout authority
  with strict approval, spend cap, idempotency, and receipt boundaries.
- Pylon v0.2 remains blocked until the OpenAgents product surface/Nexus control plane can prove the
  full Artanis/Pylon/payment flow in production with real bitcoin and
  public-safe OpenAgents product surface receipts.

### Phase 0: Stop the legacy release lane

Required work:

- mark the Google Cloud Nexus release path as legacy in OpenAgents product surface docs and
  runbooks;
- keep any old Google Cloud state as migration evidence only;
- stop using stale VM relay health, in-memory treasury status, or old Nexus
  release issues as the primary Pylon v0.2 gate;
- keep the funded local MDK test wallet as test input for the new OpenAgents product surface
  authority proof, not as evidence that the old Nexus release path is ready;
- make the current release blocker explicit: Pylon v0.2 cannot ship until the
  OpenAgents product surface/Nexus flow is tested end to end.

Acceptance:

- no roadmap item says to finish a standalone Google Cloud Nexus public
  release first;
- the old Google Cloud work is described only as context or transition input;
- all new issue bodies point to OpenAgents product surface implementation files, not VM operator
  shell work.

### Phase 1: Add the OpenAgents product surface treasury payout authority ledger

OpenAgents product surface already has D1 payment foundations:

- `workers/api/migrations/0114_buyer_payment_ledger.sql`;
- `workers/api/migrations/0115_site_payment_catalog.sql`;
- `workers/api/migrations/0117_agent_search_payments.sql`;
- `workers/api/migrations/0119_artanis_persistence.sql`;
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`;
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`.

The rebuild should extend those ledgers with Nexus-specific payment authority
records instead of inventing a separate database:

- treasury payout intents;
- payout attempts;
- payout reconciliation events;
- payout target approvals;
- payment authority receipts;
- release gates.

Acceptance:

- every live payout has a durable intent before any adapter can dispatch it;
- intents link buyer payment evidence, accepted work, assignment refs, payout
  target approval, spend cap policy, and idempotency key;
- attempts and reconciliation events can be rendered into public-safe receipts
  without raw invoice, preimage, mnemonic, wallet state, or private payout
  target material.

### Phase 2: Add the Effect payment authority service

Current payment-related modules are useful starting points:

- `workers/api/src/buyer-payment-ledger.ts`;
- `workers/api/src/hosted-mdk-client.ts`;
- `workers/api/src/l402-credential-service.ts`;
- `workers/api/src/mdk-core-checkout-contract.ts`;
- `workers/api/src/site-mdk-reconciliation.ts`;
- `workers/api/src/pylon-settlement-bridge.ts`;
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`.

The new service should expose explicit operations:

- preview a payout without spending;
- create or reject a payout intent;
- reserve spend capacity;
- dispatch through an adapter;
- record adapter attempts;
- reconcile status;
- project public-safe receipts;
- pause the authority globally or per adapter.

The first adapters should be:

- `simulation`, for deterministic policy and receipt tests without moving
  bitcoin;
- `mdk_agent_wallet`, for a controlled local MDK CLI/daemon boundary;
- `hosted_mdk`, only where hosted MDK checkout/status evidence applies;
- `legacy_nexus_import`, read-only, for importing old Nexus receipts or
  cleanup evidence during transition.

Acceptance:

- the service rejects missing accepted-work refs, missing payout target
  approval, stale wallet readiness, excess spend, replayed idempotency keys,
  paused adapters, and malformed amount/destination data;
- the simulation adapter and MDK adapter share a conformance suite;
- no route handler shells out directly to MDK or stores wallet secrets.

### Phase 3: Add Pylon edge wallet and work APIs

Existing Pylon marketplace and settlement code should become the API-facing
surface for Pylon v0.2:

- `workers/api/src/pylon-marketplace-jobs.ts`;
- `workers/api/src/pylon-settlement-bridge.ts`;
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`.

Required APIs:

- Pylon registration and heartbeat;
- wallet readiness reporting;
- payout target admission;
- assignment acceptance and status updates;
- artifact/proof upload;
- payment receipt upload;
- settlement event projection;
- operator and Artanis reads over the same state.

Acceptance:

- a Pylon can prove it is ready to receive bitcoin before assignment;
- assignment status does not depend on SSH into a machine;
- payment receipts are linked to assignments and payout intents;
- all Pylon write routes require scoped agent or operator authority.

Current implementation status:

- #426 added the D1-backed Pylon agent API for registration, heartbeat, wallet
  readiness, payout-target admission requests, assignment progress, artifact
  refs, payment receipt refs, and settlement status refs.
- #427 added the simulation-only Pylon marketplace payout-flow bridge in
  `workers/api/src/pylon-marketplace-payout-flow.ts`.
- The #427 bridge converts accepted marketplace assignment evidence into a
  Nexus Treasury payout intent, simulation payout attempt, reconciliation
  event, payment-authority receipts, settlement-bridge timeline records, and
  a read-only accepted-work payout row.
- The bridge rejects missing accepted-work evidence before payout intent
  creation and links job, assignment, Artanis dispatch, payout intent, and
  adapter attempt refs in operator-safe records.
- The focused runbook is
  `docs/nexus/2026-06-07-pylon-marketplace-payout-flow-runbook.md`.

Still missing:

- #428 added the first Artanis payment-backed dispatch gate in
  `workers/api/src/artanis-nexus-pylon-adapters.ts`, documented in
  `docs/nexus/2026-06-07-artanis-payment-backed-dispatch-gates.md`.
  Artanis can now run a simulation-backed dispatch through
  `TreasuryPaymentAuthority`, record payment authority state, and project
  public-safe gate status without exposing private payment or wallet material.
- #429 added the first Nexus/Pylon visibility layer in OpenAgents product surface:
  `GET /api/public/nexus-pylon/receipts/{receiptRef}`,
  `/nexus-pylon/receipts/{receiptRef}`,
  `GET /api/operator/nexus-pylon/dashboard`, and
  `GET /api/operator/nexus-pylon/receipts/{receiptRef}`. The first receipt
  projection is simulation-only with `realBitcoinMoved: false`; operator
  status includes Artanis runs, Pylon readiness, assignments, payout intents,
  payout attempts, settlement status, blocked gates, and release-gate evidence
  without requiring SSH.
- Real MDK bitcoin movement through the authority path is still #431.

### Phase 4: Upgrade Artanis from observer to gated supervisor

Current Artanis/Nexus/Pylon code now has a payment-authority-gated simulation
dispatch path, but not live MDK spend authority:

- `workers/api/src/artanis-nexus-pylon-adapters.ts`;
- `workers/api/migrations/0119_artanis_persistence.sql`;
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`.

Artanis can now:

- run a complete simulated Pylon assignment through OpenAgents product surface after accepted-work,
  payout-target approval, wallet-readiness, spend-cap, adapter, and
  idempotency gates pass;
- record payment authority states for proposed, previewed,
  awaiting-approval, assignment-created, wallet-ready, payout-intent-created,
  dispatch-authorized, dispatch-blocked, settlement-pending,
  settlement-complete, and settlement-failed dispatches;
- produce public-safe projection fields for payment state, gate-passed status,
  blocked status, accepted-work refs, payout intent refs, and settlement
  bridge refs; and
- block before payout attempts when accepted work is missing, payout-target
  approval is missing, wallet readiness is stale or absent, idempotency is
  replayed, the adapter is unavailable, spend caps fail, or pause policy
  applies.

Artanis should next be able to:

- propose work;
- publish work to the Forum;
- select or recommend Pylons;
- create assignments;
- request payout previews from operator/browser/API surfaces;
- submit payout intents for approval or auto-approval when policy allows and
  persist those transitions into the operator dashboard;
- publish public-safe status and receipts.

Artanis must not:

- spend bitcoin without the payment authority gates passing;
- announce paid dispatches before wallet readiness and spend caps pass;
- bypass operator approval for first-time payout targets, large payouts, or
  non-simulation adapters.

Acceptance:

- Artanis can run a full simulated Pylon assignment;
- Artanis can run a real small-bitcoin assignment only after approval and
  readiness gates pass;
- Artanis publishes useful Forum updates without exposing private wallet,
  operator, or customer data.

### Phase 5: Prove the MDK flow with simulation, then real bitcoin

The local MDK test wallet already received real bitcoin and can be used as the
source for a later OpenAgents product surface-controlled proof. That proof should not be considered
complete until OpenAgents product surface records the full receipt chain.

Required sequence:

1. Simulation adapter passes all policy and receipt tests.
2. A second isolated MDK wallet is created as the Pylon edge wallet.
3. OpenAgents product surface creates a payout intent for a tiny approved test assignment.
4. The MDK adapter dispatches a small bitcoin payment.
5. OpenAgents product surface records dispatch, confirmation, verification, and settlement events.
6. A public-safe receipt page proves the flow without exposing private
   payment material.

Acceptance:

- a real MDK wallet can pay another MDK wallet through the OpenAgents product surface authority
  path;
- the receipt chain links buyer evidence, accepted work, payout intent,
  dispatch, confirmation, verification, and settlement;
- the public proof page contains no raw invoice, preimage, mnemonic, wallet
  secret, or private payout target.

### Phase 6: Add operator, public, and Forum visibility

Required surfaces:

- operator dashboard for Artanis/Nexus/Pylon state;
- public-safe activity feed for Nexus/Pylon work;
- receipt detail pages;
- Forum bridge for Artanis assignment, incident, release, and payout updates;
- AGENTS.md and API docs explaining what agents can do through OpenAgents product surface.

Acceptance:

- an operator can classify a stuck assignment without SSH;
- a public viewer can inspect safe proof and status;
- an agent can discover live Forum, Pylon, and Sites capabilities without
  being told to use private operator credentials.

### Phase 7: Release Pylon v0.2 only after OpenAgents product surface gates pass

The Pylon v0.2 release gate should require:

- OpenAgents product surface treasury payout ledger migration deployed;
- payment authority service deployed;
- simulation adapter green;
- MDK adapter green against an isolated funded test wallet;
- Pylon registration, heartbeat, wallet readiness, assignment, artifact, and
  receipt APIs green;
- Artanis simulated assignment green;
- Artanis small real-bitcoin assignment green;
- public-safe receipt page green;
- operator dashboard green;
- Forum release/update flow green;
- runbook and AGENTS.md current.

Only after those gates pass should OpenAgents cut or announce Pylon v0.2.

## Exact GitHub issues to create

The following issue titles and bodies are the exact text to create in
`OpenAgentsInc/openagents` via GitHub CLI. The bodies are intentionally
long so the issue itself has enough context for an agent to implement without
re-reading this full audit.

### 1. `[OPENAGENTS-NEXUS] Freeze legacy GCP Nexus lane and mark OpenAgents product surface as the Pylon v0.2 release path`

```markdown
## Context

The old Google Cloud Nexus lane is no longer the release path for Nexus v0.2 or Pylon v0.2. It remains useful as legacy production context and migration evidence, but the current product direction is to rebuild Nexus control, proof, Artanis coordination, and payment authority inside OpenAgents product surface.

This issue exists to prevent future agents from continuing the stale plan of hardening the Google Cloud VM/deploy loop before Pylon v0.2. The new gate is OpenAgents product surface-owned Nexus state and MDK-backed payment authority.

## Current code and docs to inspect

- `docs/nexus/2026-06-07-nexus-google-cloud-friction-and-cloudflare-effect-rebuild-audit.md`
- `docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `docs/live/AGENTS.md`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`

## Required work

- Update relevant OpenAgents product surface docs so they clearly say the Google Cloud Nexus lane is legacy context, not the active Pylon v0.2 release path.
- Remove or demote any roadmap language that says to finish a standalone Google Cloud Nexus public release before rebuilding in OpenAgents product surface.
- Add a short transition note explaining that old Google Cloud receipts, cleanup outputs, or VM state may be imported later, but should not be treated as the live control plane.
- Make the current Pylon v0.2 gate explicit: OpenAgents product surface/Nexus must own assignment state, payment authority policy, public-safe receipts, Artanis coordination, and release evidence.
- Ensure the roadmap names MDK wrapping as the current decision: Pylons use MDK edge agent wallets, and OpenAgents product surface/Nexus adds an MDK-backed treasury payout authority.

## Acceptance criteria

- No active OpenAgents product surface roadmap section still says to ship a standalone Google Cloud Nexus v0.2 release first.
- The Nexus audit says the Google Cloud lane is legacy or transition input only.
- The Pylon v0.2 release gate is documented as an OpenAgents product surface/Nexus gate.
- MDK is described as something OpenAgents wraps deliberately, not merely as an external liquidity pattern.
- Docs avoid exposing secrets, raw payment material, VM-only operational tokens, or private payout targets.

## Out of scope

- Do not delete old Google Cloud code or receipts in this issue.
- Do not deploy or restart the old Nexus VM in this issue.
- Do not create a Pylon v0.2 public release in this issue.
```

### 2. `[OPENAGENTS-NEXUS] Add treasury payout authority D1 ledger`

```markdown
## Context

OpenAgents product surface already has buyer payment, Site payment, agent search payment, Artanis, and Pylon marketplace foundations. Nexus payout authority needs a first-party D1 ledger that links buyer payment evidence, accepted work, Pylon assignment refs, payout targets, spend caps, adapter attempts, reconciliation events, and public-safe receipt projection.

This ledger is the core reason to rebuild Nexus inside OpenAgents product surface. No MDK adapter should be allowed to move bitcoin unless a durable payout intent exists first.

## Current code to inspect

- `workers/api/migrations/0114_buyer_payment_ledger.sql`
- `workers/api/migrations/0115_site_payment_catalog.sql`
- `workers/api/migrations/0117_agent_search_payments.sql`
- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`
- `workers/api/src/buyer-payment-ledger.ts`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`

## Required work

- Add D1 migrations for Nexus/OpenAgents product surface payout authority tables.
- Include, at minimum, tables or equivalent durable records for:
  - treasury payout intents;
  - payout attempts;
  - payout reconciliation events;
  - payout target approvals;
  - spend cap reservations or spend policy snapshots;
  - payment authority receipts;
  - release gate evidence.
- Use stable IDs and idempotency keys.
- Link payout intents to existing buyer payment evidence where available.
- Link payout intents to accepted work, Artanis dispatches, Pylon marketplace assignments, and settlement bridge refs where available.
- Store only redacted references for payment material. Do not store raw invoice text, payment preimage, mnemonic, wallet secret, raw daemon output, or private payout target details.
- Add typed TypeScript helpers or schemas that mirror the migration shape.
- Add focused tests for insert/read/list/projection behavior.

## Acceptance criteria

- A payout intent can be created before any payout adapter dispatches.
- A payout attempt can be linked to exactly one payout intent and adapter.
- Reconciliation events can be appended without mutating historical attempt evidence.
- Public-safe receipt projection can be derived without exposing raw payment material.
- Replay of the same idempotency key does not create duplicate payout intents or attempts.
- Tests cover the success path and at least these rejection cases: missing accepted-work ref, missing payout target approval, amount over spend cap, malformed idempotency key, and attempt without intent.

## Out of scope

- Do not call MDK in this issue.
- Do not implement Artanis dispatch policy in this issue.
- Do not build a browser dashboard in this issue.
```

### 3. `[OPENAGENTS-NEXUS] Implement TreasuryPaymentAuthority Effect service contract`

```markdown
## Context

OpenAgents product surface needs a single Effect service boundary for all Nexus payout authority decisions. Route handlers, Artanis adapters, and Pylon marketplace code must not call MDK directly or perform ad hoc payout policy checks.

The service should own payout preview, intent creation, spend checks, adapter dispatch, reconciliation, receipt projection, pause policy, and rejection reasons.

## Current code to inspect

- `workers/api/src/buyer-payment-ledger.ts`
- `workers/api/src/hosted-mdk-client.ts`
- `workers/api/src/l402-credential-service.ts`
- `workers/api/src/mdk-core-checkout-contract.ts`
- `workers/api/src/site-mdk-reconciliation.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- the migration added by `[OPENAGENTS-NEXUS] Add treasury payout authority D1 ledger`

## Required work

- Add a `TreasuryPaymentAuthority` or equivalent Effect service.
- Define typed request and response schemas for:
  - payout preview;
  - payout intent creation;
  - payout dispatch;
  - payout reconciliation;
  - public-safe receipt projection;
  - authority pause/resume reads.
- Add typed rejection reasons for:
  - missing accepted-work ref;
  - missing payout target approval;
  - stale or absent wallet readiness;
  - spend cap exceeded;
  - replayed idempotency key;
  - paused authority;
  - paused adapter;
  - malformed payout amount;
  - malformed payout target;
  - adapter unavailable.
- Make the service read and write through the D1 ledger from the previous issue.
- Keep adapter execution behind an interface so simulation and MDK adapters can share conformance tests.
- Ensure all route-facing errors are redacted and bounded.

## Acceptance criteria

- Unit tests prove that payout preview never dispatches money.
- Unit tests prove that dispatch requires a previously created valid intent.
- Unit tests prove that all listed rejection reasons are reachable and typed.
- No route handler imports an MDK adapter directly.
- No service writes raw invoice, preimage, mnemonic, wallet state, or private payout target material.
- The public-safe receipt projection includes enough IDs and timestamps for auditability without private payment data.

## Out of scope

- Do not implement the real MDK adapter in this issue.
- Do not create browser UI in this issue.
- Do not add operator approval UI in this issue unless needed for a minimal test fixture.
```

### 4. `[OPENAGENTS-NEXUS] Add simulation payout adapter and conformance tests`

```markdown
## Context

Before OpenAgents product surface moves real bitcoin through MDK, the payment authority needs a deterministic simulation adapter. The simulation adapter should exercise the same policy, idempotency, receipt, and reconciliation paths that the real MDK adapter will use, without moving money.

This is the first green gate for the OpenAgents product surface/Nexus rebuild.

## Current code to inspect

- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- `workers/api/src/site-mdk-reconciliation.ts`
- the `TreasuryPaymentAuthority` service from the prior issue
- the payout authority ledger migration from the prior issue

## Required work

- Add a `simulation` payout adapter behind the payment authority adapter interface.
- Make it deterministic under test.
- Simulate at least these states:
  - dispatch accepted;
  - dispatch rejected;
  - confirmation pending;
  - confirmation succeeded;
  - confirmation failed;
  - reconciliation found duplicate;
  - reconciliation found stale pending attempt.
- Add a shared conformance test suite that the future MDK adapter must also pass.
- Prove idempotent dispatch behavior.
- Prove receipt projection from simulated dispatch, confirmation, verification, and settlement events.
- Add documentation explaining that simulation receipts are policy proofs, not proof of money movement.

## Acceptance criteria

- The simulation adapter can complete a full payout lifecycle without external services.
- The conformance suite runs in normal OpenAgents product surface tests.
- Replaying the same idempotency key returns the existing result rather than creating duplicate attempts.
- Public-safe receipts distinguish simulation from real bitcoin movement.
- The adapter cannot bypass spend caps, payout target approval, wallet readiness, or pause policy.

## Out of scope

- Do not call the MDK CLI or daemon in this issue.
- Do not create real invoices or move bitcoin in this issue.
- Do not publish a Pylon v0.2 release in this issue.
```

### 5. `[OPENAGENTS-NEXUS] Add MDK agent-wallet payout adapter boundary`

```markdown
## Context

The new decision is that OpenAgents should wrap MDK. Pylons should use MDK edge agent wallets, and OpenAgents product surface/Nexus should add an MDK-backed treasury payout authority. The MDK adapter must be implemented behind the payment authority service boundary and must never leak wallet secrets or raw payment material into D1, logs, docs, issue comments, or public receipts.

The MDK agent wallet CLI emits JSON and can run a local daemon. OpenAgents product surface should treat this as a controlled adapter boundary, not as route-level shell plumbing.

## Current code and docs to inspect

- `workers/api/src/hosted-mdk-client.ts`
- `workers/api/src/mdk-core-checkout-contract.ts`
- `workers/api/src/site-mdk-reconciliation.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- `docs/nexus/2026-06-07-nexus-google-cloud-friction-and-cloudflare-effect-rebuild-audit.md`
- MDK agent wallet docs: `init`, `receive`, `send`, `balance`, `payments`, JSON stdout, local daemon, `MDK_WALLET_MNEMONIC`, and `MDK_WALLET_PORT`

## Required work

- Add an `mdk_agent_wallet` adapter behind the payment authority adapter interface.
- Support configuration through Worker-safe bindings or operator-provided local execution only where appropriate. Do not commit secrets.
- Define a strict command/request boundary for:
  - balance/readiness check;
  - receive invoice creation if needed;
  - send payment;
  - payments/history reconciliation.
- Parse JSON output with typed schemas.
- Redact all payment material before ledger writes.
- Store only bounded references such as payment hash, adapter attempt ID, amount, status, timestamps, and redacted destination fingerprint.
- Add adapter tests with mocked MDK command responses.
- Add failure classification for command timeout, invalid JSON, daemon unavailable, insufficient balance, payment failed, and reconciliation mismatch.
- Add a runbook for local real-wallet testing that does not include mnemonic or secret values.

## Acceptance criteria

- The MDK adapter passes the shared payout adapter conformance suite with mocked MDK responses.
- The adapter refuses to run unless the payment authority has approved a payout intent.
- The adapter stores no mnemonic, raw invoice, preimage, wallet config, or private payout target.
- Failures are recorded as bounded classified errors.
- The runbook explains how to test with isolated wallets and real bitcoin without committing or printing secrets.

## Out of scope

- Do not run the real bitcoin movement smoke in this issue unless it is explicitly part of a later gated test issue.
- Do not expose a public endpoint that directly shells out to MDK.
- Do not make MDK the only possible adapter; simulation must remain available.
```

### 6. `[OPENAGENTS-NEXUS] Add payout target approval, spend caps, and emergency pause policy`

```markdown
## Context

OpenAgents product surface/Nexus must not allow Artanis, Pylons, or arbitrary agent tokens to spend bitcoin just because they can create work records. Live payout requires policy: approved payout target, spend caps, idempotency, wallet readiness, adapter status, and emergency pause.

This issue adds the policy layer that makes MDK wrapping safe enough to test.

## Current code to inspect

- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- the payout authority ledger and service from earlier OpenAgents product surface/Nexus issues

## Required work

- Add payout target approval records if not already covered by the ledger issue.
- Add spend cap policy records or snapshots.
- Add emergency pause state for:
  - global payment authority;
  - individual payout adapter;
  - individual payout target;
  - individual Pylon or agent identity.
- Add typed policy evaluation in the payment authority service.
- Require first-time payout target approval before live MDK dispatch.
- Require explicit approval or a documented policy rule for large payouts.
- Add idempotency and replay checks at policy evaluation time.
- Add tests for all policy gates.
- Add docs/runbook entries for pausing and resuming payout authority.

## Acceptance criteria

- A payout intent without payout target approval is rejected.
- A payout over cap is rejected.
- A paused authority, adapter, payout target, Pylon, or agent identity blocks dispatch.
- Replayed idempotency keys do not spend twice.
- Policy decisions are stored as redacted receipt evidence.
- The emergency pause runbook is clear enough for an operator or agent with appropriate authority to follow.

## Out of scope

- Do not build a full browser approval UI in this issue unless necessary for a minimal operator test.
- Do not move bitcoin in this issue.
- Do not grant broad spending authority to all agent bearer tokens.
```

### 7. `[OPENAGENTS-NEXUS] Add Pylon registration, heartbeat, wallet readiness, and receipt APIs`

Status on 2026-06-07: implemented in OpenAgents product surface as the D1-backed Pylon Agent API.
The operator/agent runbook is
`docs/nexus/2026-06-07-pylon-agent-api-runbook.md`.

```markdown
## Context

Pylon v0.2 needs Pylons to interact with OpenAgents product surface directly. A Pylon should be able to register, post heartbeats, report MDK wallet readiness, accept assignments, upload artifacts/proofs, upload payment receipts, and expose status to Artanis and operators without SSH.

Existing Pylon code exists, but it is not yet a complete live API loop.

## Current code to inspect

- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`
- `workers/api/src/openagents-capability-manifest.ts`
- `workers/api/src/openagents-openapi.ts`
- `docs/live/AGENTS.md`

## Required work

- Add or complete API routes for:
  - Pylon registration;
  - heartbeat/status update;
  - wallet readiness report;
  - payout target admission request;
  - assignment acceptance;
  - assignment progress update;
  - artifact/proof upload metadata;
  - payment receipt upload;
  - assignment settlement status.
- Add scoped auth for Pylon/agent writes.
- Ensure public reads are public-safe and private writes are authority-gated.
- Update OpenAPI and capability manifest.
- Update AGENTS.md to explain only the live capabilities.
- Add tests for each route.

## Acceptance criteria

- A registered Pylon can report heartbeat and wallet readiness through OpenAgents product surface.
- Artanis/operator reads can see current Pylon readiness without SSH.
- A Pylon can accept and update an assignment through API.
- A Pylon can attach artifact/proof/payment receipt metadata without exposing private payment data.
- Unauthenticated writes fail.
- Missing or wrong scopes fail.
- Public reads do not expose wallet secrets, raw invoices, preimages, private payout targets, or private operator notes.

## Out of scope

- Do not implement native Pylon workload execution in OpenAgents product surface.
- Do not move GPU/training runtime into Workers.
- Do not create a Pylon release in this issue.
```

### 8. `[OPENAGENTS-NEXUS] Wire Pylon marketplace jobs to payout intents and settlement receipts`

```markdown
## Context

OpenAgents product surface already has Pylon marketplace job scaffolding and settlement bridge code. The rebuild needs that work loop connected to payout intents and settlement receipts so accepted work can become payable work through the payment authority service.

This issue connects job intake, assignment, accepted work, payout intent, payout attempt, and settlement projection.

## Current code to inspect

- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- the payout authority ledger and service from earlier OpenAgents product surface/Nexus issues

## Required work

- Define the accepted-work record shape that can become payout-eligible.
- Link Pylon marketplace assignment IDs to payout intent creation.
- Require accepted-work evidence before payout intent creation.
- Add settlement bridge writes for:
  - payout intent created;
  - payout dispatch attempted;
  - payout confirmation observed;
  - payout verification recorded;
  - settlement complete;
  - settlement failed or paused.
- Add tests for full simulated flow from job intake to settlement receipt.
- Add tests for rejection when accepted-work evidence is missing.

## Acceptance criteria

- A simulated Pylon job can move from intake to assignment to accepted work to payout intent to settlement receipt.
- Missing accepted-work evidence blocks payout intent creation.
- Settlement bridge records link back to job, assignment, Artanis dispatch, payout intent, and adapter attempt where available.
- Public-safe settlement projection can be generated without private payment material.
- The flow is documented in the Nexus audit or a linked runbook.

## Out of scope

- Do not run real MDK payments in this issue.
- Do not build browser dashboard UI in this issue unless needed for minimal proof links.
- Do not add unrelated Site order fulfillment behavior in this issue.
```

Implementation status on 2026-06-07:

- Closed by `workers/api/src/pylon-marketplace-payout-flow.ts` and
  `workers/api/src/pylon-marketplace-payout-flow.test.ts`.
- Added the accepted-work payout evidence record shape.
- Added builders for payout intent, payout attempt, reconciliation event,
  intent-created receipt, settlement-bridge timeline, paused blocked bridge
  record, and accepted-work payout row.
- Verified the simulated path from marketplace intake/assignment evidence to
  payment-authority settlement receipt.
- Verified missing accepted-work evidence blocks payout flow.
- Verified public-safe settlement and payout-row projections do not expose raw
  payment, wallet, private provider, private customer, or timestamp material.
- Documented the flow in
  `docs/nexus/2026-06-07-pylon-marketplace-payout-flow-runbook.md`.

### 9. `[OPENAGENTS-NEXUS] Upgrade Artanis Nexus/Pylon adapters with payment-backed dispatch gates`

```markdown
## Context

Artanis should become the automated supervisor for Nexus/Pylon work, but it must be gated. It can propose work, publish updates, select Pylons, request payout previews, and create assignments. It must not spend bitcoin or announce paid dispatches until payment authority, wallet readiness, spend cap, approval, and idempotency checks pass.

Current Artanis code exists but needs to be connected to the new OpenAgents product surface/Nexus authority model.

## Current code to inspect

- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- the payment authority service and ledger from earlier OpenAgents product surface/Nexus issues

## Required work

- Extend Artanis adapter dispatch records with authority state.
- Add states for proposed, previewed, awaiting approval, assignment created, wallet ready, payout intent created, dispatch authorized, dispatch blocked, settlement pending, settlement complete, and settlement failed.
- Make Artanis call the payment authority preview path before live payout intent creation.
- Make Artanis refuse or pause payment-backed dispatch when:
  - no accepted-work ref exists;
  - no approved payout target exists;
  - wallet readiness is stale;
  - spend cap is exceeded;
  - adapter is paused;
  - idempotency key has already been used.
- Add tests for simulated assignment and gated dispatch.
- Add public-safe status projection for Artanis updates.

## Acceptance criteria

- Artanis can run a complete simulated Pylon assignment through OpenAgents product surface.
- Artanis cannot create a live MDK payout attempt unless the payment authority gates pass.
- Artanis status records are readable by operator surfaces and public-safe projections.
- Failure states are specific enough to guide the next operator or agent action.
- Tests prove that payment-backed dispatch is blocked before approval/readiness and allowed after gates pass.

## Out of scope

- Do not move native workload execution into Artanis.
- Do not create Pylon v0.2 release artifacts in this issue.
- Do not allow Artanis to bypass payment authority service methods.
```

Implementation status on 2026-06-07:

- Closed by updates to `workers/api/src/artanis-nexus-pylon-adapters.ts` and
  `workers/api/src/artanis-nexus-pylon-adapters.test.ts`.
- Added `ArtanisNexusPylonPaymentAuthorityState` and payment state refs to
  dispatch records and projections.
- Added `runArtanisNexusPylonPaymentBackedDispatch`, which calls
  `TreasuryPaymentAuthority.previewPayout`, `createPayoutIntent`, and
  `dispatchPayout` instead of letting Artanis bypass the authority boundary.
- Added blocked dispatch records for missing accepted work, missing payout
  target approval, stale wallet readiness, replayed idempotency, spend-policy
  failure, pause policy, and adapter unavailability.
- Verified public projections expose gate status without payment authority
  internals, payout attempts, wallet readiness refs, private evidence, raw
  payment material, or raw timestamps.
- Documented the contract in
  `docs/nexus/2026-06-07-artanis-payment-backed-dispatch-gates.md`.

### 10. `[OPENAGENTS-NEXUS] Add public-safe Nexus/Pylon receipt pages and operator dashboard`

Status: implemented by #429. The issue body below remains as the original
scope. The delivered endpoints are:

- `GET /api/public/nexus-pylon/receipts/{receiptRef}`;
- `GET /nexus-pylon/receipts/{receiptRef}`;
- `GET /api/operator/nexus-pylon/dashboard`;
- `GET /api/operator/nexus-pylon/receipts/{receiptRef}`.

The first shipped projection is simulation-only and explicitly reports
`realBitcoinMoved: false`. It does not move bitcoin; #431 remains the real
two-wallet MDK movement proof. Public receipt projections also separate
dispatch acceptance from terminal settlement evidence so an accepted payout
call is never presented as settled bitcoin.

```markdown
## Context

The point of rebuilding Nexus inside OpenAgents product surface is not just cleaner code. It is also visibility. Operators should not need SSH to classify a stuck assignment, and public viewers should be able to inspect safe proof for Artanis/Pylon work without seeing private wallet or customer data.

This issue adds the first OpenAgents product surface visibility layer for Nexus/Pylon.

## Current code and docs to inspect

- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/src/public-otec-proof.ts`
- existing Forum topic UI and public proof patterns
- `docs/live/AGENTS.md`

## Required work

- Add public-safe receipt projection routes for Nexus/Pylon work.
- Add operator-only reads for full redacted operational status.
- Add a minimal operator dashboard or API view showing:
  - Artanis runs;
  - Pylon readiness;
  - assignments;
  - payout intents;
  - payout attempts;
  - settlement status;
  - blocked gates;
  - release gate evidence.
- Add public-safe pages or routes for receipt detail.
- Ensure public surfaces do not expose private customer data, wallet secrets, raw invoices, preimages, mnemonics, private payout targets, or operator-only notes.
- Add tests for public/private projection boundaries.

## Acceptance criteria

- An operator can classify the state of a Nexus/Pylon assignment without SSH.
- A public-safe receipt page can show a simulated or real payout proof without private payment material.
- Private fields are absent from public JSON and HTML.
- The dashboard or API clearly distinguishes simulation from real bitcoin movement.
- Tests cover at least one public-safe projection and one operator projection.

## Out of scope

- Do not redesign the entire OpenAgents UI in this issue.
- Do not make private operator data public.
- Do not move money in this issue.
```

### 11. `[OPENAGENTS-NEXUS] Add Forum bridge for Artanis assignment, incident, release, and payout updates`

```markdown
## Context

Artanis is expected to coordinate Nexus/Pylon work primarily through the OpenAgents Forum. The Forum is also where agents can communicate, coordinate work, and eventually receive bitcoin-backed rewards or tips for useful contributions.

This issue adds the bridge from Artanis/Nexus/Pylon events to Forum updates while preserving authority boundaries.

## Current code and docs to inspect

- Forum routes and tests in `workers/api/src`
- `docs/forum/README.md`
- `docs/live/AGENTS.md`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- public Forum topic/reply API behavior

## Required work

- Define which Artanis/Nexus/Pylon events should create Forum topics or replies.
- Add a service boundary for Artanis Forum publication.
- Support at least:
  - assignment created;
  - Pylon selected;
  - assignment progress;
  - incident/blocker;
  - payout intent created;
  - settlement complete;
  - release gate passed or failed.
- Ensure Forum posts are public-safe.
- Ensure publication is idempotent.
- Add tests that prove duplicate events do not create duplicate posts.
- Update AGENTS.md and Forum docs if a new public capability becomes live.

## Acceptance criteria

- Artanis can publish a simulated assignment update to the Forum.
- Replaying the same event does not duplicate the Forum post.
- Public posts do not expose wallet secrets, raw invoices, preimages, private payout targets, private customer data, or operator-only notes.
- The bridge can be disabled or paused.
- The docs accurately describe what is live.

## Out of scope

- Do not add bitcoin tipping implementation in this issue unless already available through existing payment routes.
- Do not allow arbitrary unauthenticated actors to post as Artanis.
- Do not use the unlisted void forum as the primary production coordination surface.
```

### 12. `[OPENAGENTS-NEXUS] Add two-wallet MDK bitcoin movement smoke with OpenAgents product surface receipts`

```markdown
## Context

The OpenAgents product surface/Nexus rebuild is not ready for Pylon v0.2 until it proves a real small-bitcoin payment through the OpenAgents product surface payment authority path. The user funded a local MDK test wallet for this purpose. The test must move a small amount from an OpenAgents treasury test wallet to a separate Pylon edge test wallet and record public-safe OpenAgents product surface receipts.

This issue should be done only after the payout authority ledger, service, simulation adapter, MDK adapter, policy gates, Pylon APIs, and settlement bridge are ready.

## Current code and docs to inspect

- `workers/api/src/hosted-mdk-client.ts`
- `workers/api/src/mdk-core-checkout-contract.ts`
- `workers/api/src/site-mdk-reconciliation.ts`
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- `docs/nexus/2026-06-07-nexus-google-cloud-friction-and-cloudflare-effect-rebuild-audit.md`
- the MDK adapter runbook from the earlier issue

## Required work

- Create or identify two isolated MDK wallet homes:
  - one OpenAgents treasury test wallet;
  - one Pylon edge test wallet.
- Confirm both wallets are isolated from production secrets.
- Confirm the treasury test wallet has enough bitcoin for a tiny movement proof.
- Create a payout target approval for the Pylon edge test wallet.
- Create a tiny accepted-work fixture or test assignment.
- Create an OpenAgents product surface payout intent.
- Dispatch through the MDK adapter.
- Reconcile the payment.
- Record dispatch, confirmation, verification, and settlement events.
- Publish or expose a public-safe receipt page.
- Add a docs entry with the exact proof shape and redaction policy.

## Acceptance criteria

- The test moves real bitcoin between two isolated MDK wallets.
- OpenAgents product surface records the full receipt chain.
- The public-safe receipt proves real movement without exposing raw invoice, preimage, mnemonic, wallet config, private payout target, or wallet secret.
- The same idempotency key cannot spend twice.
- The test can be repeated later with a fresh idempotency key and bounded amount.
- The runbook explains how to perform the test without printing or committing secrets.

## Out of scope

- Do not use a production customer wallet.
- Do not create a Pylon v0.2 release in this issue.
- Do not run this before simulation and policy tests are green.
```

### 13. `[OPENAGENTS-NEXUS] Add Pylon v0.2 OpenAgents product surface release gate runbook and automated evidence checklist`

```markdown
## Context

Pylon v0.2 should not be announced or released until the OpenAgents product surface/Nexus path is proven. The release gate needs to be explicit, automated where possible, and understandable by future agents.

This issue turns the roadmap into a concrete gate checklist and runbook.

## Current docs and code to inspect

- `docs/nexus/2026-06-07-nexus-google-cloud-friction-and-cloudflare-effect-rebuild-audit.md`
- `docs/live/AGENTS.md`
- `docs/forum/README.md`
- Artanis docs under `docs/artanis/`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/src/pylon-marketplace-jobs.ts`
- `workers/api/src/pylon-settlement-bridge.ts`
- payment authority service and ledger from earlier issues

## Required work

- Add a Pylon v0.2 release gate runbook in OpenAgents product surface docs.
- Include required evidence for:
  - OpenAgents product surface payout ledger migration deployed;
  - payment authority service deployed;
  - simulation adapter conformance tests green;
  - MDK adapter mocked tests green;
  - real two-wallet MDK movement proof green;
  - Pylon registration and heartbeat green;
  - Pylon wallet readiness green;
  - assignment acceptance and status green;
  - artifact/proof upload green;
  - settlement receipts green;
  - Artanis simulated assignment green;
  - Artanis real small-bitcoin assignment green;
  - public-safe receipt page green;
  - operator dashboard green;
  - Forum update bridge green;
  - AGENTS.md and OpenAPI current.
- Add an automated evidence checklist where practical, or a typed receipt format if automation needs another issue.
- Make clear that old Google Cloud Nexus health is not a release gate except for explicitly imported transition evidence.

## Acceptance criteria

- A future agent can follow the runbook and know whether Pylon v0.2 is releasable.
- The checklist distinguishes required gates from optional transition evidence.
- The checklist does not require SSH into the old Google Cloud VM for normal release classification.
- The runbook says not to create a Pylon v0.2 release until all required OpenAgents product surface/Nexus gates pass.
- The runbook references the public-safe receipt pages and Forum release/update flow.

## Out of scope

- Do not create the Pylon v0.2 release in this issue.
- Do not move bitcoin in this issue unless the two-wallet smoke issue is already complete and this issue is only recording its evidence.
- Do not revive the old Google Cloud release plan.
```

## Recommendation

Terminate the old Google Cloud plan as the primary roadmap. Keep it only as
legacy context and possible migration evidence. The implementation path should
now be Option B with a sharper mandate: rebuild Nexus control, proof, Artanis
coordination, payment authority policy, and public/operator visibility inside
OpenAgents product surface's Effect/Cloudflare stack while preserving native Pylon execution.

The new MDK decision is explicit:

- Pylons use MDK edge agent wallets.
- OpenAgents product surface/Nexus wraps MDK with a treasury payout authority service.
- MDK is not just a checkout/liquidity reference.
- Live bitcoin movement is allowed only through payout intents, spend caps,
  payout target approvals, idempotency, adapter gates, and public-safe
  receipts.

The principle should be:

> Shells and VMs may execute native work, but OpenAgents product surface should own the state,
> authority, payout policy, receipts, and public proof.

If that principle is followed, Artanis becomes a real automated supervisor
instead of a coding agent babysitting a Google Cloud terminal.
