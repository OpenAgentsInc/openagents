# Launch recognition Spark recipient payout status (#5170)

Date: 2026-06-17

Scope: public-safe status for the remaining launch-recognition Spark backup
payout confirmations. This document intentionally records only refs, amounts,
states, blockers, and next actions. It must not include raw Lightning
Addresses, BOLT11 invoices, payment hashes, preimages, mnemonics, API keys, or
wallet paths.

## Recipient status

| Recipient | Intended payout | Current evidence | Status | Blocker / root cause | Next action |
| --- | ---: | --- | --- | --- | --- |
| Trigger | 50,000 sats | Recipient-side rc.12 proof reported a visible 50,000-sat Spark backup balance after `backup-claim` / `backup-status`; the scoped offline-receive promise is green. | Confirmed | None for the scoped receive/claim/visible-balance promise. | Optional follow-up is #5169-style consented Spark-to-MDK sweep if Trigger wants one spendable MDK balance. |
| Whitefang | 50,000-sat recognition + 5-sat validator fee | Earlier treasury send failed while the primary MDK/Lightning target was not accepting inbound. No public-safe rc.12+ Spark-backed Lightning Address publish, treasury retry receipt, or recipient `backup-claim` / `backup-status` proof is captured in this repo yet. | Blocked, not invisible | Missing recipient-published Spark fallback target and recipient-side claim/status proof. This is not classified as "sent but invisible"; there is no completed Spark fallback payout receipt to reconcile yet. | Whitefang installs rc.12+, runs `pylon wallet backup-receive --kind lightning-address`, reports readiness, receives the operator-approved treasury retry, then runs `pylon wallet backup-claim` and `pylon wallet backup-status` and shares public-safe receipt refs/counts. |
| Orrery | 50,000-sat recognition + 5-sat worker fee | 2026-06-17 live retry covered the 50,000-sat recognition amount through split Lightning Address payouts: 5,000 sats, 20,000 sats, and 25,000 sats all returned `status:succeeded`, `policyApplied:full`, and settled public treasury rows. Separate 250-sat smoke sends also settled. | Treasury-settled, recipient confirmation pending | A single full-sized Lightning Address retry is unreliable: one 50,000-sat attempt and one 40,000-sat attempt both failed before dispatch, with no treasury balance movement and no durable payment id. The daemon surfaced only the generic public-safe reason ref, so the precise upstream route/liquidity reason is still not proven. | Orrery runs `backup-claim` / `backup-status` and shares public-safe receipt refs/counts. Future retries for this rail should split intentionally below the live route threshold instead of repeatedly attempting one 50,000-sat invoice. |

## Treasury pending semantics

`pending` in `treasury_transactions` is a local D1 projection state, not a
settled Lightning receipt. For outbound rows it means the treasury or
tips-buffer MDK container returned an internal payment id, but OpenAgents has
not recorded a terminal `succeeded` event/preimage or a terminal `failed`
event for that id. It does not prove that the recipient received sats, and it
does not by itself prove that the sats are permanently gone from the treasury;
the live wallet balance / max-sendable value remains the spendable-wallet
truth.

The operator API now exposes a public-safe reconciliation action:
`POST /api/operator/treasury/transactions/reconcile` with
`{ "transactionId": "..." }`. The route is admin-only, reads the stored
internal payment id, asks the correct MDK container (`treasury` or
`tips_buffer`) for `/payments/{paymentId}`, and moves the D1 row from
`pending` to `settled` or `failed` only when the container reports a terminal
outcome. If the container still reports `pending` (including after an event map
restart), the D1 row stays `pending`. The API response does not expose payment
ids, hashes, preimages, invoices, destinations, mnemonics, or tokens.

The Worker cron also reconciles a bounded batch of pending outbound rows every
tick using the same logic, so future terminal MDK outcomes are persisted without
requiring a manual operator click. Legacy rows whose stored payment refs are
redacted or whose container no longer has the terminal event remain documented
as pending/blocked rather than guessed.

Pre-dispatch failures are different: if the operator payout route cannot mint a
Lightning Address invoice or the MDK pay call fails before returning a durable
payment id, the route records a `failed` outbound row with `payment_ref:null`
and a public-safe `failure_reason_ref`. The reason ref classifies the failure
without storing or returning raw destinations, BOLT11 invoices, hashes,
preimages, or daemon error strings. Use `--fail-with-body` or omit `curl -f`
when manually calling payout routes so the JSON failure body is not hidden.

## Orrery Lightning Address retry notes

On 2026-06-17, the operator retried Orrery through the Spark-backed Lightning
Address fallback using the admin payout route. The small-to-large sequence was:

- 5,000 sats: succeeded and settled.
- 40,000 sats: failed before dispatch; treasury balance and max-sendable were
  unchanged afterward, proving no sats left the treasury on that attempt.
- 20,000 sats: succeeded and settled.
- 25,000 sats: succeeded and settled.

This proves the Lightning Address fallback can resolve and receive real treasury
payouts at meaningful sizes, and that the 50,000-sat recognition amount is
covered by split settled sends. It does not prove the exact upstream cause of
the single-invoice 40,000/50,000 failure. The best current explanation is
single-payment route/liquidity fragility or a generic MDK send failure for that
invoice size, not recipient readiness and not treasury insufficient balance.
The treasury ended this sequence nearly depleted, so another full 50,000-sat
retry requires funding first.

## Operator rule

Do not re-send recognition payouts from memory. Before any retry, reconcile the
existing treasury attempt state and use the approved treasury path with an
idempotency key / public-safe receipt. A recipient is complete only when one of
these is true:

- recipient-confirmed Spark backup credited balance exists after
  `backup-claim` / `backup-status`; or
- the blocker above is still current and documented with the next required
  recipient/operator action.

For #5170, Whitefang and Orrery satisfy the issue's fallback acceptance path as
documented blockers, not as completed recipient confirmations.
