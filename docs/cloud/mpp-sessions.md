# Metered Lightning sessions

Status: profile `openagents.mpp.lightning-session.v1`, implemented in
[`pay_ledger::session`](../../crates/pay-ledger/src/session.rs) and tested on
a mock rail only ([#10721](https://github.com/OpenAgentsInc/openagents/issues/10721)).
No route offers a session yet, and no funded session has run. A funded
session qualification needs the owner's separate authorization
(`NEEDS_OWNER.md`).

A metered session funds streamed or composed work whose cost is not known
up front. It is a separate rail from the [prepaid compute
balance](compute-balance.md) and from x402 `exact`, which is one fixed
charge per request and provides no session.

## The profile

| Term | Rule |
| --- | --- |
| Deposit | One Lightning payment, observed paid before the session opens. A session opens once per deposit payment hash, so a reconnect cannot fund it twice. |
| Charge basis | A frozen rate in millisatoshis per unit (`second`, `token`, or `node`), set at opening. |
| Ceiling | At most the deposit. A debit that would pass it is refused. |
| Admission | The admission digest every debited record must carry. A session cannot widen effects. |
| Recipients | The fixed list a debit may name. A session cannot add a recipient. |
| Return destination | An explicit BOLT12 offer or Lightning address the payer gave at opening. It is never inferred. |
| Expiry | No debit at or after it; expiry closes the session. |

## Lifecycle

1. **Open** on a paid deposit.
2. **Debit** one admitted resource record (a task or a graph node) at a
   time. Each debit posts once, as the settlement `mpp:<session>:<record>`,
   in the same transaction. A retry of the same record with the same terms
   changes nothing; other terms conflict.
3. **Close** or **expire.** The remainder becomes one planned refund to the
   return destination. Closing again changes nothing.
4. **Refund.** `planned` → `sending` (the wallet reference is recorded
   first) → `sent`, `failed`, or `unknown`. `unknown` stays owed until a
   wallet lookup says `sent` or `failed`, and is never retried blindly;
   `failed` stays owed and may be planned again. A sent refund is final.

Conservation holds at every step: deposit = debited + refunded + owed +
open remainder. Refunds that are planned, sending, unknown, or failed are
liabilities.

## Separation from other rails

- A session's money never enters the prepaid compute balance, and a
  balance hold never funds a session.
- A session refund is the return of an unspent deposit under this profile.
  It is not the release of a balance hold, and it is not a refund of a
  settled charge.

## Compatibility and evidence

The profile reuses the central settlement table and the existing
`openagents` share, so payout, reconciliation, and replay work as for every
other settlement. The mock-rail tests in
[`crates/pay-ledger/tests/sessions.rs`](../../crates/pay-ledger/tests/sessions.rs)
cover deposit, incremental debit, disconnect and reconnect, ceiling, expiry,
closure, remainder return, unknown refunds, restart, and refused widening.
What remains unverified: a real deposit invoice, a real refund payment to a
BOLT12 offer, and wire compatibility with a third-party MPP client.
