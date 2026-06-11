# First Negotiated Labor Job Runbook

Date: 2026-06-10

Issue scope: #4732, coordinating #4648. This runbook is the stop-on-failure
path for the first public labor-market proof: a Forum request becomes a
NIP-LBR job, an independent contributor quotes it, the requester accepts one
quote, escrow reserves, the contributor's own local agent executes and
delivers output-only refs, a validator accepts the result, escrow releases,
and settlement evidence is filed before any registry transition.

## Hard Boundaries

- No provider-auth material, wallet material, invoices, payment hashes,
  preimages, private repo content, raw prompts, raw runner logs, or local paths
  may appear in events, posts, receipts, docs, or issue comments.
- The contributor must be independent from the requester/operator for the live
  proof. A local rehearsal can use mock runner refs, but the live run cannot.
- Escrow reserve/release is credit-ledger evidence. It is not settled bitcoin
  until the payout ladder records settlement evidence.
- Registry edits happen only after receipt refs exist. Propose transitions
  with evidence; do not flip your own promise.
- Stop on the first failed phase. Post the typed blocker and public-safe refs
  before retrying.

## Phase 0: CI-Safe Rehearsal

Goal: prove the chain shape without live relay keys, wallet spend, or a real
contributor machine.

Command:

```bash
bunx vitest run apps/openagents.com/workers/api/src/labor-live-rehearsal.test.ts
```

The rehearsal must cover:

- kind-5934 ref-only NIP-LBR request creation.
- kind-7000 quote creation from a provider ref.
- requester acceptance carrying an escrow reserve receipt ref.
- ledger reserve and release transitions with public-safe receipt refs.
- mock contributor execution that emits output-only artifact, closeout,
  summary, and verification refs.
- kind-6934 result delivery.
- validator pass evidence before release.
- public-safe evidence bundle scan with no payment/provider/private material.

If this fails, fix code before any live attempt. Do not run live phases to
compensate for a failing rehearsal.

## Phase 1: Request

Prerequisites:

- Requester has a registered OpenAgents identity and `OPENAGENTS_AGENT_TOKEN`.
- Requester balance has at least the intended budget available after held
  amounts. For the first run use about 2,000 sats.
- Public repo and verification command are bounded public refs.
- If using Artanis, `request_labor` remains disabled until an operator supplies
  explicit proposal, publication, validator, and budget dependencies.

Owner/Pylon path:

```bash
pylon work request "fix the public failing test" \
  --base-url https://openagents.com \
  --budget 2000 \
  --repo https://github.com/OpenAgentsInc/openagents \
  --verify command.public.pylon.labor.bun_test \
  --deadline 2026-06-12T00:00:00.000Z
```

Retain:

- Forum topic ref.
- work request id.
- kind-5934 job event id.
- relay link.
- initial status envelope.

Expected state: the request is public-safe, visible on the Forum, linked to
the relay event, and not accepted yet.

## Phase 2: Negotiation

Prerequisites:

- Independent contributor Pylon is online on its own device.
- Contributor truthfully declares `capability.pylon.local_claude_agent` or a
  later approved equivalent.
- Contributor has completed first-run labor approval locally.
- Contributor pricing policy is its own policy and is less than or equal to
  the request budget.

Contributor action:

- Run the provider loop against the owned relay.
- Quote the request through the NIP-LBR quote lane.
- Do not execute until the requester acceptance event carries the escrow
  receipt ref.

Requester action:

```bash
pylon work offers <work-request-id> --base-url https://openagents.com
pylon work accept <work-request-id> <quote-ref> --base-url https://openagents.com
```

Retain:

- quote event id.
- quote ref.
- provider actor ref.
- acceptance event/ref.
- reserve receipt ref.
- status envelope showing exactly one accepted quote.

Expected state: only the requester can accept, exactly one quote is accepted,
and escrow is reserved.

## Phase 3: Execution And Delivery

Contributor runs the accepted job on its own agent in the bounded workspace.

Retain:

- local first-run approval ref.
- artifact refs.
- test/build/summary refs.
- platform closeout ref.
- kind-6934 result event id.

Do not retain raw diffs, private logs, provider payloads, or local paths in the
issue, docs, receipts, Forum posts, or relay events. If debugging requires raw
material, keep it local and unpublished.

Expected state: the verification command passes on the contributor machine and
the result event is output-only.

## Phase 4: Acceptance And Release

Requester or validator reruns the stated verification command from the public
artifact refs. Release only after a passing verdict.

Retain:

- validator verdict ref.
- release receipt ref.
- provider credited balance projection ref.
- payout ladder rung and settlement receipt refs, if the payout path settles
  bitcoin in this run.

Expected state: escrow is released to the provider balance exactly once. If the
payout ladder is credited-and-swept rather than direct, record that rung
honestly and do not claim direct settlement.

## Phase 5: Evidence And Transition Proposals

Post the evidence bundle to #4732, #4648, and the
`Working: labor.nostr_negotiation_market.v1` Forum topic:

```text
topicRef:
workRequestId:
jobEventRef:
quoteEventRef:
acceptedQuoteRef:
acceptanceEventRef:
reserveReceiptRef:
resultEventRef:
closeoutRef:
verificationVerdictRef:
releaseReceiptRef:
payoutRung:
settlementReceiptRefs:
statsProjectionRef:
redactionScanRef:
```

Then propose, with receipt refs:

- clear `blocker.product_promises.labor_live_negotiated_settlement_missing`
  when negotiated release and payout evidence exists.
- clear `blocker.product_promises.labor_stream_not_live` only when a public
  retrievable stream-kind `labor` receipt exists.
- move `provider.compliant_usage_labor.v1` only as far as the evidence permits.
- propose green for `labor.forum_work_requests.v1` and
  `labor.nostr_negotiation_market.v1` only if their full verification text is
  satisfied.

Close #4648 by cross-reference only after the settled labor receipt is public
and retrievable. Close #4732 only after all acceptance checkboxes are backed by
the retained evidence bundle.
