# P1 (#4777) live run: provider quoting works; offer-ingestion bridge is the gap

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-14

## What now works (live, verified)

- An **independent provider Pylon** is online on `wss://relay.openagents.com`
  (separate Pylon home `/tmp/oa-provider-home`, nostr pubkey `3fd9b3f1…`,
  npub1uxy…), declaring `capability.pylon.local_claude_agent`,
  `capability.pylon.local_codex`, and `capability.public.pylon.labor.local_agent.v0.3`.
  Brought online with `pylon provider go-online`.
- It **quoted a live work request**: requester Raynor posted work request
  `b74bb55c-849c-43a3-b8d9-9a741316b528` (kind-5934 event
  `215ffa0b…`, target issue #4773 A1 parity slice, budget 100 sats,
  verification `command.public.pylon.labor.bun_test`). The provider published a
  real **kind-7000 quote** `3d7ec6bb9f96fd241f2fd9729f55f087c9e67a4875f25ee16bc36b69a13152cd`
  for **1 sat** (1000 msats).
- The fix that unblocked quoting: **`PYLON_LABOR_MARKET_AUTO_QUOTE=true`**
  (labor quoting is opt-in; default false → `refusal.labor_market.auto_quote_disabled`).
  Also set `PYLON_NIP90_RELAYS=wss://relay.openagents.com`,
  `PYLON_NIP90_PRICE_MSATS=1000`.

So the genuinely-missing piece (a live, independent, quoting provider) is done,
and the escrow funding need is now **1 sat** (trivial).

## The real blocker (a production gap, not setup)

The openagents.com Worker has **no relay→DB offer-ingestion bridge**:
- `recordForumWorkRequestOffer` (forum-work-request-negotiation.ts) writes an
  offer to `forum_work_request_offers`, but it has **zero production callers**.
- Nothing in the Worker subscribes to the relay for kind-7000 quotes.
- So `GET /api/forum/work-requests/{id}/offers` returns `offers: 0` even though
  the quote is live on the relay, and `POST …/acceptances` (which reserves
  escrow) has no quote to reference.

The negotiation chain breaks exactly here: relay quote → (no bridge) → API offer.

## To close #4777

1. Add `POST /api/forum/work-requests/{id}/offers` (agent-authed) that calls
   `recordForumWorkRequestOffer({workRequestId, offerId, quoteRef,
   providerActorRef, amountSats, capabilityRefs, relayEventRef})`.
2. Wire the provider to submit its quote to that endpoint after publishing the
   relay kind-7000 event (or add a relay-listener worker that ingests quotes).
3. Deploy the Worker.
4. Requester `pylon work accept b74bb55c <quoteRef>` → escrow reserves 1 sat
   (fund Raynor 1+ sat first) → provider executes via codex → output-only
   kind-6934 result → validator reruns bun test → escrow releases → settlement.
5. Post the evidence bundle (runbook Phase 5) and close #4777 (unblocks
   #4781/#4782/#4783).

## Update (later 2026-06-14): bridge shipped, escrow reserved — execution is the next gap

Steps 1–3 above are **done and deployed**:
- `POST /api/forum/work-requests/{id}/offers` (agent-authed, idempotent on
  quoteRef), `POST …/results`, and `POST …/release` shipped in
  `forum-routes.ts` + `forum-work-request-negotiation.ts`; migration `0179`
  (`forum_work_request_offers.provider_pubkey` + `forum_work_request_results`)
  applied to production D1; Worker deployed.
- The provider's live kind-7000 quote was ingested as a DB offer
  (`offerId c40dc95c…`, `state offered`).
- Requester accepted: work request is `quote_accepted`, **escrow reserved**
  (escrowId `7a958e0d…`, reserveReceiptRef present), Raynor balance
  `available 4000 / held 1000` (1 sat genuinely held). The acceptance kind-7000
  was published to the relay (`3cecbc2c…`, `status processing`, `#p` = provider).
- Provider first-run labor approval granted on the provider home
  (`approve-labor --job-type code_task`).

**New blocker — the execution/objective-resolution gap.** Driving the accepted
job through the real provider code path (`handleLaborMarketEventOnce`, the exact
function the loop runs) returns
`action: "refused", reasonRef: "refusal.labor_market.execution_refused"`
because `runtime.runLabor` throws: the local agent (`codex exec`) is handed a
prompt built by `laborPrompt()` that contains **only opaque refs** —
`objective.public.pylon_work.7b5a38b0…`, `repo.public.github…`, and the
verification-command ref — with **no readable task detail**
(`request.request.content` is empty; the relay kind-5934 is ref-only by design,
per the runbook's "no raw prompts on the relay" boundary).

Two coupled problems make the current target unverifiable-in-sandbox:
1. **No objective resolution.** The provider never resolves the public
   objective/title (which *does* live in the Forum work-request DB, e.g.
   "Complete the A1 API parity matrix slice of issue 4773…") into the agent
   prompt. `laborPrompt()` just lists refs. So no agent can act on it.
2. **Sandbox vs. repo objective.** Execution runs in an isolated empty
   workspace (`cache/labor-market/<ref>`), but the A1 parity slice is a
   repo-dependent objective — even with the title, a self-contained `bun test`
   in an empty dir can't meaningfully verify it. The runbook's own fixture
   shape (the `sum.ts`/`sum.test.ts` `fixingRunner`) is **self-contained** for
   exactly this reason.

**Honest close path (does not fabricate completion):** the first live job must
be a genuinely **self-contained bounded task** whose verification is meaningful
in an isolated workspace, AND the provider must receive readable task detail.
Minimal plumbing:
- Carry a public-safe, self-contained `objectiveDetail` (or have the provider
  resolve the Forum work-request title via the public
  `GET /api/forum/work-requests/{id}`) into `laborPrompt()`.
- Point the first job at a self-contained slice (e.g. "create `parity.ts`
  exporting a typed MVP-surface parity matrix + `parity.test.ts` asserting every
  row has an api peer; `bun test` passes") that still references #4773 honestly
  as the subject in the title.
- Then: quote → accept (escrow) → codex executes the self-contained task →
  `bun test` genuinely passes → kind-6934 result → `POST /results` → validator
  reruns `bun test` → `POST /release` → settlement → evidence bundle → close
  #4777.

Until that plumbing lands, **do not** publish a result for the empty sandbox —
that would be agent-prose-claims-completion, which INVARIANTS forbid. The 1-sat
escrow stays honestly reserved (or is refunded) in the meantime.

## Provider run command (reproducible)

```
PYLON_HOME=/tmp/oa-provider-home PYLON_OPENAGENTS_BASE_URL=https://openagents.com \
CODEX_HOME=$HOME/.codex PYLON_NIP90_RELAYS=wss://relay.openagents.com \
PYLON_NIP90_PRICE_MSATS=1000 PYLON_LABOR_MARKET_AUTO_QUOTE=true \
  bun apps/pylon/scripts/nip90-provider-serve.ts
```
(After `bun apps/pylon/src/index.ts provider go-online` in the same home.)
