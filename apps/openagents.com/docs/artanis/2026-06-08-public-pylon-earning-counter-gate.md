# Public Pylon Earning Counter Gate

OpenAgents product surface now exposes a deterministic public earning gate in
`/api/public/pylon-stats` as `earningLaunchGate`.

The gate exists because public Pylon counters can be live while broad earning
copy is still unsafe. A Pylon can be seen in the last 24 hours without being
online now. A wallet-ready Pylon is not necessarily assignment-ready. An
assignment-ready Pylon is not accepted-work, payout, or settlement evidence.

## Gate Contract

`earningLaunchGate.publicEarningCopyAllowed` is `true` only when all current
public counters are nonzero:

- `pylonsOnlineNow`
- `pylonsWalletReadyNow`
- `pylonsAssignmentReadyNow`

When any required counter is zero, the gate returns `state: "blocked"` plus
machine-readable blocker refs:

- `blocker.public.pylon.online_now_zero`
- `blocker.public.pylon.wallet_ready_now_zero`
- `blocker.public.pylon.assignment_ready_now_zero`

If the stats store is unavailable, the gate also returns:

- `blocker.public.pylon.stats_unavailable`

The blocked claim refs are the claims public dashboards and launch copy must
not make while the gate is red:

- `blocked_claim.public.pylon.automatic_bitcoin_earning`
- `blocked_claim.public.pylon.self_serve_paid_work`
- `blocked_claim.public.pylon.assignment_ready_payouts`

## Readiness Boundaries

Wallet-ready is receive/readiness evidence only. It is not send authority,
outbound liquidity, accepted work, payout dispatch, or settlement evidence.

Assignment-ready requires fresh heartbeat, wallet readiness, and public compute
readiness refs. It is still not evidence that a Pylon accepted work or was paid.

The Artanis public report carries the same gate under
`pylonSummary.earningLaunchGate` so dashboards do not recompute policy from raw
counters.

## Verification

The regression suite covers:

- zero-counter blockers;
- stale heartbeat expiration;
- wallet-ready without assignment readiness;
- a two-Pylon fresh heartbeat ready smoke;
- Artanis report propagation; and
- browser dashboard rendering of the gate state.

Run the focused checks with:

```sh
bun run --cwd workers/api test -- src/public-pylon-stats.test.ts src/artanis-public-report.test.ts
bun run --cwd apps/web test -- src/page/loggedOut/page/login.scene.test.ts src/docs-blog-route.test.ts
```
