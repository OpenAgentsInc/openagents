# M10 live proof pass, 2026-06-14

This bundle records the live production pass run after the retained overnight
proof evidence. It proves the requester-Pylon lane through delivered work and
accepted review, and it proves the SHC lane through scheduled dispatch plus
fallback lease intent. It does not close #4768 because the SHC lane still lacks
a terminal live closeout or decision receipt.

## Lane A: OpenAgents SHC

- Work order: `autopilot_work_order.1531e063-71e4-49aa-a378-5d8d7fdbb3b3`
- Launch: scheduled for `2026-06-14T02:34:13Z`, dispatched at
  `2026-06-14T02:34:56Z`.
- Production state: `accepted_free_slice`.
- Runner projection: `openagents_shc` fallback lease intent.
- Terminal evidence: not present. `executionCloseout` is still `null`.

The live worker has Pylon closeout ingestion, but not an equivalent SHC/fallback
worker closeout route. The blocker is recorded in `blocker-record.json`.

## Lane B: Requester Pylon

- Work order: `autopilot_work_order.fa64ac58-901c-4a90-a125-03792decb300`
- Remote Pylon: `pylon.m10.archlinux.20260614`
- Launch: scheduled for `2026-06-14T02:40:01Z`, dispatched at
  `2026-06-14T02:41:00Z`.
- Production state after closeout: `delivered`.
- Production state after review: `accepted`.
- Verification command: `bun test apps/pylon/tests/proof-smoke-checklist.test.ts`
  on the Arch Pylon checkout, 6 tests passed.
- Funding: `buyerFundingState: "not_required"`, `fundedAmountCents: 0`,
  `settlementBlockedReasonRef: "settlement.no_worker_payout_mode"`.
- Wallet readiness: MDK daemon online, `balanceSats: 0`, `receiveReady: true`,
  production registration `walletReady: true`.

## Files

- `summary.json` — compact lane result and close decision.
- `lane-a-shc-live-status.json` — live SHC scheduled dispatch and fallback
  lease projection.
- `lane-b-requester-pylon-accepted-status.json` — accepted requester-Pylon work
  order projection.
- `arch-pylon-wallet-readiness.json` — zero-balance MDK readiness and production
  Pylon registration projection.
- `arch-pylon-worker-closeout.json` — worker closeout refs admitted by the live
  Pylon assignment API.
- `arch-pylon-proof-smoke-checklist-test.txt` — verification command output.
- `blocker-record.json` — exact remaining close blocker.
