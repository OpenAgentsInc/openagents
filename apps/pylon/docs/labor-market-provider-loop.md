# Labor market provider loop

Status: **retired**. The Pylon-side NIP-LBR negotiation lane this page used to
describe was deleted on 2026-07-14 by commit `21e82ce829` (`feat(vp1): retire
money sites and wallet authority`), along with `apps/pylon/src/labor-market.ts`.
No Pylon quotes, wins, or delivers market jobs today.

The protocol it spoke survives, and so does most of the discipline. This page
records what is still real, so the next reader of
[#30](https://openagents.com/OpenAgentsInc/openagents/issues/30) starts from the
tree rather than from a description of it.

Protocol: `docs/nips/LBR.md`. Related: `nip90-provider-loop.md`.

## Commands that no longer exist

| Removed command                | Status                                                                             |
| ------------------------------ | ---------------------------------------------------------------------------------- |
| `pylon provider approve-labor` | Deleted. `apps/pylon/src/cli-catalog.ts` offers only `go-online` and `go-offline`. |
| `pylon provider once`          | Deleted with the provider loop.                                                    |
| `pnpm run provider:serve`      | Deleted with `apps/pylon/scripts/nip90-provider-serve.ts`.                         |

The `laborMarket` configuration block, the `PYLON_LABOR_MARKET_AUTO_QUOTE` and
`PYLON_LABOR_MARKET_PRICE_MSATS` environment overrides, and the
`labor-market-state.json` quote store went with the module that read them.
`evaluateLbrRequestForQuote` and the `LABOR_MARKET_VERIFICATION_COMMANDS`
registry no longer exist anywhere in the tree.

## What survived

**The protocol, in `packages/nip90`.** Kept and extended through the
retirement:

- `lbr.ts` — request, quote, acceptance, and result.
- `lbr-closeout.ts` — a content-addressed, public-safe receipt binding one whole
  lifecycle, carrying `verificationCommandRef`, `testRef`,
  `platformCloseoutRef`, and a re-derivable digest.
- `lbr-bond.ts` — forfeitable provider bonds, added after the retirement in
  `3590489b6e`.

**The buyer half of negotiation.** `packages/sarah/src/lbr-request-quote/` was
recovered on 2026-07-24 as request and quote only. It does not accept, execute,
or settle.

**The admission rules, uncalled.** `apps/pylon/src/labor.ts` still holds the
parts of the lane that were about safety rather than money: the first-run
operator approval store, the bounded workspace resolver that rejects `..` and
`.git` escapes, `evaluateLaborRequestSafety` and its provider-auth
exfiltration detector, the public-safe result projection, and the sandboxed
local agent command builders for `codex`, `opencode`, and `claude_code`.
Nothing imports the module; `makeConfiguredLaborRuntime` sits in
`scripts/uncalled-production-symbol-baseline.json`.

**The wrong half of the state machine.**
`apps/pylon/src/coordinator/labor-job-state.ts` also survives, also uncalled.
Read it before reusing it. Its transition table contains
`settle: { delivered: "settled" }`, and `delivered` is a state the provider
sets, so that machine pays a provider's own claim. The doctrine the Pylon arc
earned is the opposite: a lease is not an earning claim, and only a settlement
receipt is. That rule is not expressible in this table.

## What replaced the settlement end

`openagents provider settle` (commit `f0c36de6a0`) settles against the
surviving LBR closeout receipt rather than against a provider's submission. An
earning requires a receipt that names this job and this provider, was not
issued by the provider to itself, carries a verification command _and_ the
evidence it produced, carries the platform's own closeout, is
content-addressable, landed inside the lease window, and prices the job exactly
as the lease did. Ten named refusals cover the failures, and each earns zero.

Nothing moves: a settled decision reports `payout_rail: "not_connected"` and
`custody: "none"`.

## The boundaries, which still hold

These were never the part that broke, and they constrain any revival:

- Pay for verified work only, never for presence.
- The provider never self-accepts and never sees requester funds — it sees a
  receipt ref.
- Work runs on the contributor's own agent, own credentials, own machine. No
  provider-auth material enters events, artifacts, or state.
- The relay is transport, not authority. Settlement truth comes from the
  platform's receipt systems.
- Raw session material, diffs, and logs stay on-device. Only refs travel.

## What a revival still needs

`INVARIANTS.md` holds that payments, markets, wallet custody, payout, billing
credits, and settlement are outside the accepted MVP, and that any revival
needs a fresh owner-approved design, custody model, invariant change, and proof
program. `scripts/vp1-retired-money-surface-guard.mjs` enforces that on every
push.

Beyond the owner decision, #30 still lists: presence with capacity as a
quantity, the claim and lease transport with its idle keepalive and resubscribe
(the 2026-06-12 run went dark about a minute after going online because 60
seconds of relay silence was treated as fatal), a provider identity seam, and a
buyer. Per the issue, the first buyer is OpenAgents' own operation — validator
replay jobs, delegation children, and registry service jobs. None of that demand
exists as jobs yet.
