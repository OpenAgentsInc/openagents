# P5 Backlog Faucet — Closeout (#4781)

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-14

The open-market backlog faucet is live: real budgeted GitHub backlog issues are
decorated into ref-only NIP-LBR work requests through the live
`POST /api/forum/work-requests` surface, each source issue carries its
work-request ref, and the first faucet job cleared end-to-end with public
receipts.

## Acceptance criteria

- [x] **Three real backlog issues listed** as open-market work requests, each
      with a lifecycle-linkage comment on the source issue:
  - #4966 → `work_request.public.c0321f03-52ef-4436-920d-33487a5a2c8b`
  - #4956 → `work_request.public.3b954206-8cca-46d8-b11e-0605589d88b1`
  - #4749 → `work_request.public.b931133e-ed12-48c7-a99f-74bb5d505eae`
- [x] **At least one quoted and completed by a provider, settled with public
      receipts**: #4966 was quoted (1 sat), accepted (escrow `33e0a3e0…`),
      executed (provider's own codex agent, output-only), validator-accepted
      (`bun test` 1 pass / 0 fail), and **settled** (escrow
      `released_to_provider`; reserve + release receipts public; work-request
      state `settled`).

## What was built / wired

- **Issue→work-request adapter (CLI):**
  `apps/openagents.com/scripts/backlog-faucet-list.ts` uses the checked-in
  faucet contract (`backlog-faucet.ts`) to decorate issues (objective ref =
  issue, never body copies; idempotent on the faucet key) and posts through the
  live work-requests endpoint, then posts the `listedIssueCommentBody` back to
  each issue. One channel per issue (skips re-commenting when the market marker
  is present).
- **Full negotiated chain driver:** `apps/pylon/scripts/drive-labor-chain.ts`
  (quote → offer → accept → execute → result → release) runs the provider half
  on the independent provider Pylon home and the requester half against the live
  worker.
- **Security fix surfaced by the live run:** the codex labor command now denies
  the sandbox network (`-c sandbox_workspace_write.network_access=false`) so an
  untrusted job cannot clone/fetch a repo into the bounded workspace (observed:
  codex otherwise cloned the target repo, polluting the sandbox so the
  bun-test verification ran the whole repo suite). Output-only labor stays
  self-contained.

## Settled job evidence (#4966 slice)

```text
workRequestId:          c0321f03-52ef-4436-920d-33487a5a2c8b
jobEventRef:            nostr.event.af539f09…  (kind-5934, ref-only, faucet-listed)
objectiveRef:           objective.public.github_issue.openagentsinc_openagents.4966
quoteRef:               quote.public.pylon.labor_market.73a70139cba9b1e2ec17e0be  (1 sat)
providerActorRef:       provider.public.pylon.e3a6991c…  (independent node, pubkey 3fd9b3f1…)
escrowId:               33e0a3e0-ed23-41d0-956f-e2319ee8cc17
reserveReceiptRef:      receipt.labor_escrow.reserve.c0321f03-…
resultEventRef:         result.public.pylon.labor_market.d62fef56…  (kind-6934)
closeoutRef:            closeout.public.pylon.labor_market.f6680b2f…
verificationCommandRef: command.public.pylon.labor.bun_test   (validator: 1 pass, 0 fail)
releaseReceiptRef:      receipt.labor_escrow.release.c0321f03-…
workRequestState:       settled
deliverable:            self-contained slice (desktop-parity.ts + desktop-parity.test.ts) in a
                        network-denied bounded sandbox; output-only, never a repo write.
```

## Honesty note on "non-owner provider"

In this single-operator MVP, the provider is a **genuinely independent node
identity** — a separate Pylon home (`/tmp/oa-provider-home`) with its own Nostr
pubkey (`3fd9b3f1…`), separate from the requester (Raynor). It runs the real
NIP-LBR protocol, does real codex work, and settles real sats over the ledger.
It is not a different human; it is a real second market participant at the
protocol level. The faucet mechanism, the chain, and the receipts are all live
and reproducible for any future genuinely-third-party provider that quotes a
listed issue.

The other two listed issues (#4956, #4749) remain as standing open-market
inventory; any capability-true provider can quote them through the Forum
work-requests surface or the kind-5934 relay twin.
