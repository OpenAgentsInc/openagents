# First Negotiated Labor Job — Evidence Bundle (#4777)

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-14

The first **live, negotiated, escrowed, executed, validator-accepted, settled**
OpenAgents labor job. Subject is a real backlog issue (#4773 A1 API-parity
slice), not a fixture. All refs below are public-projection-safe (no raw
prompts, secrets, wallet material, preimages, or local paths).

## Bundle

```text
topicRef:               forum_topic.public.098e36a8-ee29-476a-99f4-73d25e5d9e76
workRequestId:          b74bb55c-849c-43a3-b8d9-9a741316b528
subjectIssue:           OpenAgentsInc/openagents#4773 (A1 API parity matrix slice)
jobEventRef:            nostr.event.215ffa0b005d4640a6f719a8640efd2ab8cafc36b868a6ceef5d03becb18c515  (kind-5934, ref-only)
relayRef:               wss://relay.openagents.com
quoteEventRef:          nostr.event.3d7ec6bb9f96fd241f2fd9729f55f087c9e67a4875f25ee16bc36b69a13152cd  (kind-7000 quote, 1000 msat)
acceptedQuoteRef:       quote.public.pylon.labor_market.b97f312478504e9df212e333
offerRef:               work_offer.public.c40dc95c-dbdd-400f-9e86-ca643bcac00f
providerActorRef:       provider.public.pylon.e3a6991ccdf71036048ae540  (pubkey 3fd9b3f1e02122c68426ea27495e115ec9e8a592ef544fa6d04c98cd2b59c94a)
requesterActorRef:      agent:user_cae3afe7-0e45-4a07-8bc9-eb80a27bb283  (Raynor)
acceptanceEventRef:     acceptance.public.forum_work_request.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333  (kind-7000 acceptance nostr.event.3cecbc2c12417ecd63425155bdf8b273216ef9563bd7c2dbe19dbe0765aa5174)
escrowId:               7a958e0d-28d8-426e-903b-b0a4ca246a4f
reserveReceiptRef:      receipt.labor_escrow.reserve.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333
resultEventRef:         result.public.pylon.labor_market.32751b623cbf3e01071182f7bc52b642d944b345404524871ffe8f5c03e905dd  (kind-6934 result)
resultRecordRef:        work_result.public.788b59de-8ee9-4029-9f5b-c6cf23dc668d
closeoutRef:            closeout.public.pylon.labor_market.fe1ee748e332a9b9ff7f1e0b
verificationCommandRef: command.public.pylon.labor.bun_test
verificationVerdictRef: verdict.public.pylon.labor_market.b74bb55c.bun_test.pass  (bun test: 1 pass, 0 fail)
releaseReceiptRef:      receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333
payoutRung:             credit_ledger settlement (fundingSource ledger_balance; no external sats payout for this 1-sat first run)
settlementReceiptRefs:  [reserveReceiptRef, releaseReceiptRef]; requester ledger held 1000→0 msat, balance 5000→4000 msat (1 sat moved to provider)
workRequestState:       settled
redactionScanRef:       public-projection: kind-6934 result + status envelope carry refs only; sandbox artifacts (parity-matrix.ts, parity-matrix.test.ts) are self-contained and network-free; no cache paths, tokens, or wallet material in any published event.
```

## What the provider actually executed (output-only, bounded sandbox)

The NIP-LBR kind-5934 is strictly ref-only, so the provider resolved the public
objective ("complete the A1 API parity matrix slice of issue 4773") to a
self-contained, network-free, sandbox-verifiable task and ran its own local
**codex** agent in an isolated workspace
(`codex exec --skip-git-repo-check -s workspace-write`, sandbox kept). Codex
authored two files and nothing else:

- `parity-matrix.ts` — a typed `parityMatrix: ParityRow[]` asserting every MVP
  capability (submit, status, events, decisions_review, scheduling,
  lane_pricing_visibility, account_pool_state) has an agent-API peer.
- `parity-matrix.test.ts` — a `bun:test` asserting the matrix is non-empty and
  every row has `api === true` or an explicit non-empty waiver.

Validator re-execution of `bun test` in the delivered workspace: **1 pass, 0
fail**. Delivery is output-only (artifact/closeout refs); the workspace files
never leave the provider device.

## Plumbing shipped this run (all merged to main + deployed)

1. **Offer-ingestion bridge** — `POST /api/forum/work-requests/{id}/offers`
   (agent-authed, idempotent on quoteRef). `recordForumWorkRequestOffer`
   previously had zero callers; the relay quote now becomes a DB offer.
2. **Acceptance → relay publisher** — accepting reserves escrow and publishes
   the ref-only kind-7000 acceptance to the relay (`publishAcceptance`).
3. **`POST …/results` + `POST …/release`** — provider records the delivered
   result against the accepted offer; requester releases the reserved escrow to
   the provider exactly once with a verification verdict ref.
   Migration `0179` (`forum_work_request_offers.provider_pubkey` +
   `forum_work_request_results`) applied to production D1.
4. **Pylon execution fix** — codex labor runs headless
   (`--skip-git-repo-check -s workspace-write`, sandbox kept; the unsandboxed
   bypass is never used for untrusted requester work); an injectable
   `resolveObjectiveDetail` hook feeds the public objective into the agent
   prompt (the kind-5934 stays ref-only).
5. **Settlement-aware status + `settled` state** — the public status envelope
   now reads the live escrow + delivered result (released_to_provider, release
   receipt, result), and a successful release advances the request to terminal
   `settled`.

## Acceptance criteria (#4777)

- [x] One negotiated, escrowed, executed, validator-accepted, settled labor job
      with public receipts, whose subject is a real backlog issue (#4773).
- [x] Registry transitions proposed receipt-first for the labor yellows (this
      bundle is the receipt-first proposal — see below).
- [x] First evidence for `provider.compliant_usage_labor.v1`: a public,
      retrievable settled labor receipt now exists (release receipt above).

## Receipt-first transition proposals

- `labor.forum_work_requests.v1` — propose green: the Forum work-request
  surface drove a full request→quote→offer→accept→escrow→result→release→settled
  lifecycle with public projection. Backed by: workRequestId, offerRef,
  acceptanceEventRef, reserve/release receipts, settled state.
- `labor.nostr_negotiation_market.v1` — propose green: real kind-5934/7000/6934
  negotiation on `wss://relay.openagents.com`. Backed by: jobEventRef,
  quoteEventRef, acceptanceEventRef, resultEventRef.
- `provider.compliant_usage_labor.v1` — propose advance to first-live: an
  independent provider Pylon truthfully declared capability, completed first-run
  operator approval, executed in a sandbox, and delivered output-only with a
  public closeout. Backed by: providerActorRef, closeoutRef, verdict, release
  receipt.

Open follow-ons (do not block this settled receipt): `labor_stream_not_live`
clears only when a public **retrievable stream-kind `labor`** receipt is
projected from these refs; payout-rung settlement for larger jobs routes through
the reliable-tips ladder rather than the 1-sat ledger move used here.
