# Autopilot Coder Current Status And Gap Audit

Date: 2026-06-09

Status: current repo audit after the OA-AUTO P0 issue flow, the hosted Gemini
closeout bridge commit, OA-AUTO-019 production Pylon placement wiring, and
OA-AUTO-020 durable Pylon assignment lease creation, and OA-AUTO-021 normalized
coding assignment payload contract, and OA-AUTO-022 real no-spend requester
Pylon worker closeout loop, and OA-AUTO-023 Autopilot delivery ingestion from
Pylon worker closeouts, and OA-AUTO-024 customer review/revision API.
It also includes OA-AUTO-025, the documented public no-spend Autopilot Coder
smoke command, OA-AUTO-026, the signed/verifier-gated L402 retry path, and
OA-AUTO-027, the CI-safe paid Autopilot Coder route smoke, the #4619
epic closeout for the real no-spend Pylon execution path, and the #4620
buyer-payment-ledger verifier wiring for paid Autopilot work.
This document is intentionally stricter than
the implementation log: it distinguishes route-harness proof from a real paid
agent doing real coding work.

## Scope Read

This audit is based on the current `docs/autopilot-coder/` folder:

- `README.md`
- `implementation-log.md`
- `2026-06-09-probe-autopilot-sites-agent-api-audit.md`

It also cross-checks the current implementation surfaces those docs describe:

- `apps/openagents.com/workers/api/src/autopilot-work-request.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-routes.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-quote.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-assignment-planner.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-placement-selector.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-pylon-assignment-synthesizer.ts`
- `apps/openagents.com/workers/api/src/autopilot-work-fallback-lease-adapter.ts`
- `apps/openagents.com/workers/api/src/autopilot-coding-assignment.ts`
- `apps/openagents.com/workers/api/src/l402-payment-headers.ts`
- `apps/openagents.com/workers/api/src/pylon-api.ts`
- `apps/openagents.com/workers/api/src/pylon-api-routes.ts`
- `apps/openagents.com/workers/api/src/index.ts`
- `apps/pylon/src/assignment.ts`
- `apps/openagents.com/workers/api/src/openagents-openapi.ts`
- `apps/openagents.com/workers/api/src/openagents-capability-manifest.ts`
- D1 migrations `0140` through `0147` for Autopilot work orders and Pylon
  coding-assignment payload storage.
- production Pylon API store wiring in `apps/openagents.com/workers/api/src/index.ts`.

The GitHub issue flow for `OA-AUTO-001` through `OA-AUTO-018` is closed, and
the follow-on P0 issues `OA-AUTO-019` through `OA-AUTO-027` are also closed as
of this audit. Epic #4619 is also complete for the no-spend Pylon route
contract. The #4620 repo-contract work now wires paid Autopilot work to the
buyer-payment ledger verifier. Those issues built the first Autopilot
work-order spine plus the
initial production Pylon placement, Pylon assignment lease, and normalized
assignment-payload pieces plus a bounded no-spend requester-Pylon closeout
loop, Autopilot delivery ingestion from that closeout, and the owner-granted
review/revision API plus documented no-spend and paid-route smokes and signed
L402 retry verification. They did not, by themselves, build the full paid
coding-agent product.

## Executive Finding

The current implementation proves an Autopilot work-order state machine, not a
full commercial coding-agent loop.

The repo can now do this in tests and route harnesses:

```text
registered-agent request
-> typed Autopilot work order
-> deterministic quote
-> 402 payment-required response for payable L402 work
-> signed L402 proof-header retry accepted by a verifier hook
-> buyer funding projection
-> placement decision
-> durable Pylon assignment lease or fallback lease intent
-> requester Pylon no-spend or buyer-funded assignment acceptance/progress/artifact/worker-closeout refs
-> Autopilot work order delivered projection from worker closeout refs
-> owner/customer review can accept, reject, or request changes
-> optional injected hosted execution closeout
-> delivered projection and delivered events
```

The repo cannot yet honestly claim this:

```text
user tells any agent "do this on Autopilot"
-> agent discovers the live API
-> agent pays a real L402 or MDK checkout
-> OpenAgents verifies real payment
-> OpenAgents launches a real coding worker
-> worker edits/builds/tests
-> OpenAgents ingests real artifacts
-> customer accepts or requests changes
-> OpenAgents settles eligible workers/providers
-> Forum/public surfaces report only safe status
```

That second loop is still the target product.

## What The Issue Flow Actually Built

The closed P0 issue flow built the minimum typed orchestration spine:

| Area | Current status | What it proves |
| --- | --- | --- |
| Work request schema | Built in `autopilot-work-request.ts` | Public-safe typed work requests can be decoded and unsafe prompt/private/secret-shaped values are rejected. |
| Invocation routes | Built in `autopilot-work-routes.ts` | `POST /api/autopilot/work`, detail reads, idempotency, registered-agent auth, and D1 persistence exist. |
| Event route | Built | Callers can poll or stream public-safe queued, payment, running, delivered, access, and blocked state signals. |
| Agent docs | Built | The route is represented in agent-facing docs/OpenAPI/capability surfaces. |
| Access requirements | Built | Missing repo/customer/operator/privacy/Pylon/secret-broker needs are projected as typed requirements instead of implicit text. |
| Repository authority projection | Built | Public read can proceed; write/branch/PR authority is explicit and not silently granted. |
| Deterministic quote | Built | Persisted request inputs produce stable quote refs and amounts. |
| Buyer proof intake | Partially built | L402 retries are now signed, quote-bound, expiry-checked, persisted as buyer-payment challenges, and verifier-gated against buyer-payment ledger redemption/receipt/entitlement/reconciliation state before an order can move to funded. MDK checkout retry remains fail-closed until checkout reconciliation is wired. |
| Funding vs payout | Built | Buyer payment proof and worker payout eligibility remain separate. |
| Typed task records | Built | A work order can contain independently projected tasks. |
| Assignment planner | Built | Tasks become assignment intents with ready/blocked/payment/access planner state. |
| Queue inventory | Built | Existing queue surfaces can be dry-run inventoried for foldover. |
| Placement policy | Built | Privacy/runner preferences are persisted and projected. |
| Pylon presence input | Built with production Pylon API store wiring | Placement can prefer a compatible requester Pylon from D1-backed Pylon registrations, heartbeats, version, capability, and wallet-readiness records. |
| Local coding capability refs | Built | Placement can distinguish local coding-agent readiness without exposing local secrets. |
| Pylon assignment synthesis, lease creation, no-spend worker closeout, and delivery ingestion | Built | Ready requester-Pylon work creates a durable no-spend `pylon_api_assignments` lease, idempotent retries do not duplicate it, the Pylon API can poll and accept it, the Pylon runtime can submit progress/artifact/proof refs and a worker closeout, and that closeout can move the Autopilot work order to delivered. |
| Paid Pylon route smoke | Built as CI-safe route smoke | Payable work can receive a signed L402 challenge, retry through a verifier-approved proof ref, create a buyer-funded requester-Pylon assignment payload, accept/closeout through the Pylon API, deliver the work order, and accept review while settlement/payout stay blocked. |
| Fallback lease adapter | Built as intent projection | Ready work can become controlled SHC/cloud/hosted Gemini fallback lease intents. It is not yet real runner dispatch. |
| Placement refusal/retry | Built | The API can return actionable retry/needs-input guidance instead of silent stalls. |
| Hosted execution closeout bridge | Built in route harness | An injected hosted executor can return public-safe refs and move an order to delivered. It is not the production hosted executor. |

The important phrase is "intent projection" for several of these. A projection
is a controlled plan or offer shape. It is not a worker having accepted a
lease, run tools, modified a repo, created a Site, or produced verified
artifacts.

## The L402 Reality Check

The current Autopilot route no longer funds work from a proof ref alone.

What is built:

- `paymentRequiredResponse` returns HTTP `402` for payable L402 work.
- The response includes a public-safe `WWW-Authenticate: L402` header.
- When the MDK route signing boundary is configured, the response also returns
  a private `x-openagents-l402-credential` retry header for the paying agent.
- `parseOpenAgentsPaymentHeaders` can parse:
  - `Authorization: L402 <credential>:<public-safe-proof-ref>`;
  - `Authorization: LSAT ...`;
  - `X-OpenAgents-L402: <credential>:<public-safe-proof-ref>`.
- Autopilot L402 retry verification checks the signed credential against:
  - deterministic quote amount and currency;
  - challenge ref;
  - endpoint and product refs;
  - request-body digest derived from the stored work request;
  - owner/agent/work-order scope refs;
  - challenge expiry;
  - the payment proof ref supplied by the agent.
- The route then calls an explicit payment verifier dependency. Production now
  wires that verifier to the buyer-payment ledger. If the ledger redemption,
  receipt, entitlement, or matched reconciliation is missing or mismatched, the
  order does not move to funded.
- Route coverage now exercises unpaid, malformed, unverified, expired,
  mismatched, verified, and idempotent replay cases.
- The paid Autopilot Coder smoke now exercises the route-level payable path:
  `402`, signed retry, verifier-approved proof ref, funded work projection,
  Pylon assignment, worker closeout, delivered projection, owner review, and
  blocked settlement/payout authority. The verifier in that smoke is the
  buyer-payment ledger verifier, not a proof-ref allowlist.
- MDK checkout proof retry is fail-closed; a header ref no longer funds a work
  order without a real checkout verifier.

What is not built or not proven:

- No real Lightning invoice is generated for Autopilot work.
- No real MDK checkout session is created for Autopilot work in this route.
- No agent wallet pays a real challenge in the Autopilot route test.
- The paid smoke records a matched ledger redemption bundle in test; it does
  not prove external payment movement.
- The production Autopilot route verifies ledger state, but the deployed
  MDK/L402 reconciler still has to create those ledger rows from actual
  external payment movement.
- No live production registered-agent request has completed the paid flow.

Therefore the current exact claim is:

```text
Autopilot work verifies signed, quote-bound L402 retry credentials and requires
buyer-payment ledger redemption/receipt/entitlement/reconciliation proof before
moving payable work to funded.
```

The current exact non-claim is:

```text
Autopilot has a completed live MDK/L402 paid coding-agent flow with an agent
wallet payment and external payment verification.
```

## Hosted Gemini Reality Check

The hosted Gemini lane is also only partially proven.

What is built:

- `hosted_gemini` is a typed runner kind.
- Quotes can include hosted Gemini placement cost.
- Placement can select `hosted_gemini` as fallback.
- Fallback lease intents can target `fallback_lane.openagents.hosted_gemini`.
- A route dependency hook, `executeReadyWork`, can be injected in tests.
- If the injected executor returns public-safe assignment, closeout, proof, and
  result refs for the selected hosted Gemini assignment, the order moves to
  `delivered`.
- Delivered orders project:
  - `state: "delivered"`;
  - `nextAction.state: "delivered"`;
  - public-safe execution closeout refs;
  - queued and delivered events;
  - no worker payout authority;
  - no accepted-work authority;
  - no deploy/spend authority;
  - no Forum autopublish authority.

What is not built or not proven:

- No production hosted Gemini executor binding is installed.
- No model/provider request is made by the Autopilot worker route.
- No real coding agent session is started from this lane.
- No repo checkout, patch, test run, Site build, or artifact upload happens
  through this lane.
- No usage meter, budget ledger, or provider policy enforcement exists for this
  Autopilot lane.
- No live production smoke proves a real hosted Gemini worker did real work.

The test executor proves the state-machine seam where a real executor should
plug in. It does not prove the executor exists.

## Current End-To-End Product Status

The desired user-facing end state is:

```text
"Do this on Autopilot"
-> OpenAgents API receives a typed work request
-> missing access or payment is requested with minimal friction
-> work runs on the best allowed runner
-> real artifacts are returned
-> review/acceptance gates run
-> payment and settlement gates run
-> safe status is visible to the user and, where allowed, the Forum
```

Current status by phase:

| Phase | Current status | Gap to desired product |
| --- | --- | --- |
| Agent discovery | Mostly built | Needs refreshed docs/examples that say exactly which parts are live, and a real agent smoke that follows them. |
| Auth and owner grant | Built for registered-agent customer-order scopes | Needs unauthenticated or anonymous-paid entry path, plus smoother owner grant prompts. |
| Request intake | Built for typed public-safe work requests | Needs production wiring and better examples for real agents, plus private/secret-broker modes later. |
| Payment request | L402 challenge issuance built for configured signing boundary; MDK checkout intent still planned | Needs hosted checkout creation for checkout mode and production agent docs for the private credential header. |
| Payment verification | L402 signed retry verification, buyer-payment ledger verifier wiring, and CI-safe paid smoke built | Needs deployed MDK/L402 reconciliation, agent-wallet or checkout smoke, and external payment movement. |
| Placement | Selector, production Pylon registration store wiring, Pylon lease creation, and intent projections built | Needs real worker execution and closeout recovery after assignment acceptance. |
| Pylon local path | Lease creation, Pylon polling, Pylon acceptance, progress submission, artifact/proof submission, worker closeout, and Autopilot delivered projection built for no-spend and CI-safe buyer-funded public refs | Needs richer real repo edit/build/test artifacts and live paid-mode policy. |
| Hosted fallback path | Lease intent plus test executor hook built | Needs production executor binding and provider/runtime policy. |
| SHC/cloud fallback path | Lease intent built | Needs production runner adapter and lease execution. |
| Probe coding loop | Normalized coding assignment contract exists, and the Pylon no-spend loop consumes current assignment projections and returns public-safe refs | Needs richer real coding execution, repository checkout/patch/test adapters, and non-Pylon runner consumers. |
| Result ingestion | Pylon worker closeout refs are captured on assignment state and can deliver the Autopilot work order with public-safe artifact/blocker/build/preview/proof/result/summary/test refs | Needs richer diff/log/blob storage and operator-only evidence retention beyond public refs. |
| Acceptance | Owner-granted review API built for delivered Autopilot work | Needs operator review, accepted-work-to-payout eligibility bridge, and richer follow-up work creation. |
| GitHub writeback | Not built in Autopilot work path | Needs branch/commit/PR lane after repo grants and proof/test gates. |
| Sites adapter | Existing Sites control plane exists | Needs Autopilot task adapter for `site_generation` and `site_adjustment`. |
| Forum reporting | Not built for Autopilot work orders | Needs redacted lifecycle renderer and idempotent posting bridge. |
| Settlement | Explicitly blocked | Needs accepted-work payout eligibility and settlement bridge. |
| Production smoke | CI-safe no-spend and paid-route smokes built | Needs staging/live no-spend and paid smokes against deployed credentials and real payment verification. |
| No-spend smoke | Built as CI-safe command | Needs staging/live production run with real deployed credentials when that environment is ready. |
| Paid smoke | Built as CI-safe command | Needs staging/live run with external payment movement, live verifier, and real worker execution. |

## What Changed Since The Original Audit

The older audit in this folder correctly identified the target system, but it
now contains stale rows that say there is no `POST /api/autopilot/work`
contract, no event stream, no quote service, no placement model, and no
fallback lease shape. Those are now built.

The implementation log is more current: `OA-AUTO-001` through `OA-AUTO-021`
are implemented, and the hosted execution closeout bridge was added after the
first issue sequence.

The difference between "we did the issue flow" and "the product works" is:

- The issue flow built contracts, projections, state transitions, and tests.
- It did not build live payment movement.
- It did not build live paid worker dispatch.
- It did not build real coding execution.
- It did not build settlement.
- It did not build Forum reporting.
- It did not run the two required staging/live product smokes.

That is why the system is substantially closer, but still not the thing the
user asked for.

## Exact Remaining Build Plan

### Step 1: Replace proof-ref-only payment with real Autopilot payment issuance

Status: partially implemented by OA-AUTO-026 and #4620 ledger-verifier wiring.

Built:

- Signed Autopilot L402 credential issuance for configured signing boundaries.
- Buyer-payment challenge persistence on L402 `402` responses when the ledger
  is configured.
- Deterministic quote, work-order, owner/caller, request-digest, amount,
  expiry, endpoint, product, and scope binding.
- Paid retry verification against the signed credential and stored work order.
- Explicit payment verifier hook that fails closed when missing or rejected.
- Production verifier wiring to the buyer-payment ledger. A paid retry must
  have a redeemed challenge, issued receipt, active entitlement covering the
  L402 scopes, matching amount/product/challenge refs, and a matched
  reconciliation event.
- Route coverage for unpaid, malformed, unverified, expired, mismatched,
  verified, and idempotent replay cases.
- MDK checkout proof retries remain payment-required until a real checkout
  verifier exists.

Still required:

- Deployed reconciler proof that writes ledger rows from actual payment
  movement, not from test setup.
- MDK checkout intent creation/reconciliation for checkout mode.
- Sandbox or staging agent-wallet smoke against the deployed endpoint.
- CI-safe route-level paid smoke is built by OA-AUTO-027 and now uses the
  buyer-payment ledger verifier, but it still does not move external funds.

Acceptance:

- A test agent can submit payable public Autopilot work, receive a real payment
  challenge, pay it with an agent wallet or MDK checkout flow, retry, and have
  the Worker verify the paid challenge before moving to funded.

### Step 2: Wire production Pylon placement input into Autopilot work routes

Status: implemented by OA-AUTO-019.

Build:

- Replace the test-only `pylonRegistrations` dependency with production reads
  from the Pylon registration/heartbeat store.
- Select requester Pylon when:
  - owner linkage matches;
  - heartbeat is fresh;
  - client version is compatible;
  - assignment-ready capability exists;
  - local coding-agent capability exists;
  - wallet readiness is sufficient for the selected payment mode.
- Return `needs_input` when the user should install/restart/connect Pylon.

Acceptance:

- A real registered agent tied to an owner with an online compatible Pylon gets
  `selectedRunnerKind: "requester_pylon"` without manual dependency injection.

Remaining gap after this step:

- Production placement now reads the Pylon API store, but selected Pylon
  placement still produces assignment intents until the scheduler creates
  durable leases in the next step.

### Step 3: Convert Pylon assignment intents into real Pylon assignment leases

Status: implemented by OA-AUTO-020.

Build:

- Autopilot scheduler transition from `pylonAssignmentIntents` to durable
  Pylon assignment records.
- Lease expiry, duplicate prevention, cancellation, and stale-run recovery.
- Pylon poll/accept/progress/artifact/closeout flow linked back to the
  Autopilot work order.

Acceptance:

- One no-spend public Autopilot task creates a Pylon assignment, a Pylon polls
  it, accepts it, reports progress, submits artifact/proof refs, and closes it.

Current acceptance state:

- Built: durable assignment creation, idempotency, poll, and accept.
- Built by OA-AUTO-022 after this step: real no-spend worker progress,
  artifact/proof submission, and worker closeout.
- Still open: Autopilot work-order delivery projection from worker closeout,
  customer review/acceptance, richer repo edit/build/test artifacts, and paid
  mode.

### Step 4: Normalize the real coding assignment contract

Status: implemented by OA-AUTO-021.

Build:

- A single assignment payload shared by requester Pylon, SHC, cloud sandbox,
  hosted Gemini, and later privacy lanes.
- Fields for:
  - objective;
  - repo refs;
  - branch/write/PR authority refs;
  - allowed tools;
  - auth refs;
  - acceptance criteria;
  - budget and timeout;
  - public/private trace policy;
  - closeout schema.
- Redaction checks for prompts, local paths, provider payloads, invoices,
  wallet material, private repos, and secrets.

Acceptance:

- The same Autopilot work order can be leased to Pylon or fallback lanes using
  one normalized payload.

Current acceptance state:

- Built: shared requester-Pylon/fallback assignment schema, work-order mapping,
  public-safe ref-only objective/repository/authority/budget/trace/closeout
  fields, and unsafe fixture rejection.
- Still open: real workers must consume this payload and submit closeout
  artifacts in OA-AUTO-022 and later closeout-ingestion work.

### Step 5: Implement the actual worker loop for one lane

Status: implemented for the bounded requester-Pylon no-spend lane by
OA-AUTO-022, with CI-safe buyer-funded route coverage added by OA-AUTO-027.

Pick one first lane. The lowest-friction product path should be:

1. requester Pylon/local coding agent for no-spend public work;
2. hosted Gemini fallback for paid public work;
3. SHC/cloud fallback after that.

Build:

- Worker checkout/setup.
- Tool execution boundary.
- Patch/Site artifact production.
- Test/build command execution.
- Public-safe closeout generation.
- Operator-only evidence retention.

Acceptance:

- A real worker, not an injected test function, produces a closeout for one
  public docs/test task.

Current acceptance state:

- Built: the Pylon runtime can poll a current OpenAgents assignment projection,
  normalize the embedded coding assignment, accept the assignment, submit
  progress, submit artifact/proof refs, and submit a public-safe worker
  closeout.
- Built: Worker-side Pylon assignment state records the worker closeout as
  `closeout_submitted` and keeps it separate from operator accepted-work
  closeout.
- Built by OA-AUTO-023 after this step: the closeout can automatically move the
  Autopilot work order into delivered state with public-safe artifact, blocker,
  build, preview, proof, result, summary, and test refs.
- Still open: it does not yet perform a real repo checkout/patch/test/PR
  workflow, and the buyer-funded smoke still uses deterministic payment
  verification.

### Step 6: Add closeout ingestion for real artifacts

Status: implemented for Pylon worker public-safe refs by OA-AUTO-023.

Build:

- Ingest:
  - diff refs;
  - test refs;
  - preview refs;
  - build refs;
  - blocker refs;
  - artifact refs;
  - safe summary refs.
- Store private evidence separately from public projection.
- Preserve no self-acceptance: worker closeout is not accepted work.

Acceptance:

- Delivered Autopilot work shows useful customer-safe result refs, and private
  material does not appear in API projections, events, docs, issue comments, or
  Forum posts.

Current acceptance state:

- Built: Pylon worker closeout ingestion moves matching Autopilot work orders
  to `delivered`.
- Built: delivered projections include public-safe artifact, blocker, build,
  preview, proof, result, summary, and test refs plus no accepted-work,
  settlement, payout, deploy, spend, or Forum autopublish authority.
- Built: unsafe local-path-shaped closeout refs are rejected before persistence.
- Still open: private operator evidence storage, full diff/blob storage, and
  richer build/test log redaction are not implemented.

### Step 7: Add customer review and revision API

Status: implemented by OA-AUTO-024.

Build:

- Owner or owner-granted agent can:
  - accept delivered work;
  - reject delivered work;
  - request changes;
  - ask for a follow-up task.
- Keep accepted-work authority separate from worker completion.

Acceptance:

- A delivered task can move to accepted or revision-required without DB edits.

Current acceptance state:

- Built: owner-granted registered agents can accept, reject, or request changes
  on delivered Autopilot work.
- Built: review decisions are idempotent and public-safe.
- Built: accepted/rejected/revision-required states update next actions, task
  lifecycle projections, and event streams.
- Built: worker closeout, buyer payment, and review remain separate from
  payout, settlement, deploy, spend, and Forum publication authority.
- Still open: operator review and accepted-work payout eligibility are not
  implemented.

### Step 8: Add GitHub and Sites delivery adapters

Build:

- GitHub:
  - branch creation;
  - commit refs;
  - PR refs;
  - tests/proofs attached;
  - only after write/branch/PR grants exist.
- Sites:
  - map `site_generation` and `site_adjustment` tasks to existing Site project,
    builder session, version, preview, save, and deploy-review records.

Acceptance:

- Repo-change work can deliver a PR.
- Site work can deliver a preview or saved version.
- Production deploy remains owner/operator gated.

### Step 9: Add Forum reporting bridge

Build:

- Forum reporting policy per order/task:
  - private;
  - public-safe summary;
  - campaign topic;
  - operator-approved only.
- Redacted summary renderer.
- Idempotent topic/reply posting.
- Queue foldover report from existing work.

Acceptance:

- One public-safe Autopilot work order gets a Forum lifecycle topic/update
  without exposing private source, raw prompts, provider logs, payment secrets,
  invoices, or local paths.

### Step 10: Add settlement and payout eligibility

Build:

- Accepted-work to payout candidate bridge.
- Worker/provider/referrer payout policy.
- Duplicate prevention.
- Spend caps.
- Settlement receipts.
- Reconciliation states.

Acceptance:

- Buyer payment alone cannot pay anyone.
- Delivered work alone cannot pay anyone.
- Accepted work with eligible payment mode can create payout eligibility.
- Settlement requires actual settlement receipt refs.

### Step 11: Run the two product smokes

Required smokes:

1. No-spend public task:
   - registered agent submits task;
   - requester Pylon selected;
   - real Pylon accepts and runs;
   - real closeout ingested;
   - customer-safe delivered projection visible.

Status: implemented as a CI-safe command by OA-AUTO-025:

```sh
bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:no-spend
```

The smoke drives the route-level no-spend path and scans retained projections
for private paths, wallet/payment material, provider payloads, raw prompts,
raw logs, raw source archives, secret material, and forbidden
hosted-infrastructure wording. It does not pass the test-only
`pylonRegistrations` placement dependency and does not use an injected hosted
executor; placement reads the Pylon API store and the closeout travels through
the Pylon assignment API before Autopilot ingests delivered refs.

The companion Pylon runtime regression is:

```sh
bun test --cwd apps/pylon tests/assignment.test.ts tests/live-worker-loop-smoke.test.ts
```

That pair closes epic #4619 for the CI-safe no-spend Pylon execution path.
The remaining no-spend gap is staging/live execution against deployed
credentials.

2. Paid public task:
   - registered or anonymous-paid agent submits task;
   - real MDK or L402 payment is issued and paid;
   - payment is verified;
   - real worker executes;
   - closeout is ingested;
   - customer reviews;
   - settlement remains blocked or proceeds according to policy.

Status: implemented as a CI-safe route command by OA-AUTO-027 and upgraded by
#4620 ledger-verifier wiring:

```sh
bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:paid
```

The smoke drives the payable route path through signed L402 retry,
buyer-payment-ledger-verified funding, requester-Pylon assignment, Pylon
closeout, delivered projection, owner review, and retained-projection
redaction checks. It does not prove live external payment movement, a deployed
MDK/L402 reconciler writing ledger rows, or a production coding worker doing
repository edits.

Only after these smokes pass should public copy say "pay and Autopilot does the
work."

## Updated Priority List

### P0: Make the claim real

1. Deployed MDK/L402 reconciliation into the Autopilot buyer-payment ledger.
2. MDK checkout intent creation and reconciliation for checkout-mode work.
3. Staging/live paid smoke with external payment movement and real worker execution.
4. Staging/live no-spend smoke against deployed credentials.
5. Richer diff/blob/build-log ingestion and operator-only evidence storage.
6. Accepted-work payout eligibility and settlement bridge.

### P1: Make it useful across product surfaces

1. GitHub writeback lane.
2. Autopilot Sites task adapter.
3. Hosted Gemini production executor binding.
4. SHC/cloud runner adapter.
5. Forum reporting bridge.
6. Stale assignment/retry/recovery policy.
7. Operator dashboard slices.

### P2: Make it a marketplace

1. Accepted-work payout eligibility.
2. Pylon paid-mode settlement ladder generalized to Autopilot work.
3. Provider/runtime cost accounting.
4. Privacy-tier compiler.
5. Secret-brokered task mode.
6. TEE and Maple AI lane adapters.
7. Referrer/signature/data contribution payout bridges.

## Current Truth In One Sentence

OpenAgents now has a serious Autopilot Coder control-plane skeleton, including
typed requests, quotes, signed/verifier-gated L402 retry, placement, assignment
intents, production Pylon placement input, durable no-spend Pylon assignment
lease creation, a normalized coding assignment payload, a bounded
requester-Pylon no-spend worker closeout loop, delivered closeout projection in
a route harness, and Autopilot delivered projection from real Pylon worker
closeout refs, owner-granted customer review states, and a documented no-spend
smoke command; it still does not have a live paid path where a real agent pays,
a live MDK/L402 verifier confirms payment movement, a real worker produces
accepted repo/Site changes, artifacts are paid-settlement eligible, and
eligible workers/providers are settled.
