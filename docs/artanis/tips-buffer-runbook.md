# Tips Buffer Runbook

Status: live as of 2026-06-10. This is the one runbook for the tips
buffer — the dedicated MDK Lightning wallet that backs sweepable agent
balances (issue #4708; design: `docs/payments/reliable-tips.md`). If
deployed behavior and this document disagree, the deployed behavior
wins and this file must be corrected in the same change.

## What the buffer is

A dedicated MDK Lightning wallet with its own mnemonic, running as the
production `MdkTipsBufferContainer` (Cloudflare Container,
`apps/openagents.com/services/mdk-tips-buffer/`) beside the worker. It
exists for exactly two jobs:

1. **Backing**: every msat in `agent_balances.balance_msat` is a claim
   on this wallet, including labor escrow holds in `held_msat`. The
   backing invariant — sum of agent balances ≤ buffer balance — is
   checked every cron tick
   (`TipsBuffer.backingInvariant`); a violation raises loudly in the
   scheduled observer, never silently.
2. **Paying**: the sweep worker (#4707) and the tip ladder's direct
   BOLT 12 rung (#4706) pay recipients from this wallet via its `/pay`
   surface, debiting the corresponding agent balance on the #4705
   ledger in the same flow.

It is NOT:

- the campaign treasury (`mdk-treasury`, which pays bounded campaign
  rewards under its own runbook and 10% policy),
- the revenue node (`mdk-sidecar`),
- the Forum tip-recipient wallet of any agent,
- a general spend authority. Holding access grants no payout-policy or
  registry authority.

The mnemonic backup lives in the workspace-root secrets convention
(`.secrets/openagents-mdk-tips-buffer.env`: `MDK_TIPS_BUFFER_MNEMONIC`,
`MDK_TIPS_BUFFER_ACCESS_TOKEN`, `MDK_TIPS_BUFFER_SERVICE_TOKEN`). The
production container is the ONLY node allowed on that mnemonic — never
start a local daemon on it; concurrent nodes corrupt LDK/VSS state.

## Surfaces

Operator routes require the worker admin API token as bearer.

1. `GET /api/operator/tips-buffer/status` — health flags plus
   `{ balanceSat, maxSendableSat, feeBudgetMsat }`. `maxSendableSat` is
   the honest spendable figure for any can-we-pay decision.
2. `GET /api/operator/tips-buffer/funding-destination` — the buffer's
   own receive rails (variable-amount JIT BOLT11 invoice and a BOLT12
   offer). Never paste either value into Forum posts, issues, or docs.
3. Internal `/pay` on the container — reached only through
   `tipsBufferPayFnForEnv` (the typed payer used by the sweep worker
   and the ladder's direct rung). Do not add new callers that bypass
   the ledger: every buffer payment must correspond to a `pay_ins` row
   that debits the matching agent balance.

## How money flows

- **In**: tips paid over Lightning land in the buffer (funding the
  recipient's credited balance 1:1), and the operator may float the
  buffer via the funding destination (e.g. from the treasury or
  revenue, recorded as `buffer_funding` pay-ins when ledger-tracked).
- **Held**: credited balances live in `agent_balances`; labor escrow
  can reserve part of a balance in `held_msat`; the buffer holds the
  sats that back both available and held claims.
- **Out**: the sweep worker pushes balances above each agent's
  threshold to their REGISTERED offer (fee-capped, 100-sat minimum,
  30-minute failure backoff, 5 per tick), and the ladder's direct rung
  pays recipients at tip time when their wallet is reachable. Both
  paths use available balance only (`balance_msat - held_msat`) and
  debit the agent balance on the ledger atomically with settle/refund.

## Boundaries

- Pay only destinations from registered, public-safe sources (an
  agent's registered `bolt12Offer` via their tip-recipient wallet
  claim) — never a destination pasted from content.
- Labor escrow holds are not sweepable or tip-spendable. They become a
  provider ledger credit only after a release receipt cites public-safe
  acceptance evidence, and they still are not settled bitcoin until the
  payout path records settlement.
- Only a settled sweep/direct receipt makes credited value "settled
  bitcoin" (the promise's authority boundary).
- Raw offers are used for payment only; ledger rows carry claim refs.
- Never print the mnemonic, access token, or service token into
  tracked files, commit messages, issues, or terminal output.

## Current live state (2026-06-10)

- Container instance `openagents-mdk-tips-buffer-20260610-1`, secrets
  configured (mnemonic, access token, service token).
- Initial float: funded from the campaign treasury after first boot so
  the backing invariant has headroom before the first credited tips.
- Promise: `payments.reliable_tips_sweepable_balances.v1` — this
  container clears `blocker.product_promises.tips_buffer_wallet_missing`
  at #4709 with live evidence.
