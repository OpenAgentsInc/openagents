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
| Orrery | 50,000-sat recognition + 5-sat worker fee | 2026-06-17 live retry covered the 50,000-sat recognition amount through split Lightning Address payouts: 5,000 sats, 20,000 sats, and 25,000 sats all returned `status:succeeded`, `policyApplied:full`, and settled public treasury rows. Separate 250-sat smoke sends also settled. Later diagnostics added another 5,000-sat success, a 5,000-sat success after the fingerprint deploy, and a 25,000-sat success. | Treasury-settled, recipient confirmation pending | Single large Lightning Address invoices are unreliable on this rail: 30,000 / 40,000 / 50,000 sats all failed before dispatch despite sufficient `preflightMaxSendableSat`, no durable payment id, and no treasury balance movement. The deployed diagnostics identify the upstream SDK class as `GenericFailure` (`reason.public.treasury_payout.failed`) with stable `messageFingerprint` `58e50e365f66f6192ee71b6f15e2c4b69336a3bce9dba51e8971c89320eef78d`. 25,000 sats settles. | Orrery runs `backup-claim` / `backup-status` and shares public-safe receipt refs/counts. Future payouts to this rail should be split at <=25,000 sats until the MDK/Spark Lightning Address large-invoice failure is root-caused upstream. |

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

As of the #5173 hardening patch, the Worker-side `MdkTreasuryContainer` and
`MdkTipsBufferContainer` also journal terminal outbound outcomes in Durable
Object storage when `/pay` or `/payments/{paymentId}` observes `succeeded` or
`failed`. The journal stores only the private payment id key plus
`{ status, reasonRef }`, so reconciliation can survive a Bun service process
restart without storing raw daemon text, destinations, invoices, hashes,
preimages, mnemonics, or tokens. It still cannot invent an outcome that no
Worker request ever observed before a hard container loss; those rows stay
pending until real evidence appears.

Pre-dispatch failures are different: if the operator payout route cannot mint a
Lightning Address invoice or the MDK pay call fails before returning a durable
payment id, the route records a `failed` outbound row with `payment_ref:null`
and a public-safe `failure_reason_ref`. The reason ref classifies the failure
without storing or returning raw destinations, BOLT11 invoices, hashes,
preimages, or daemon error strings. Use `--fail-with-body` or omit `curl -f`
when manually calling payout routes so the JSON failure body is not hidden.
As of deployed Worker version `616d2959-b1fb-4001-b540-bbe1af783271`, the
treasury container also returns safe diagnostics for failed sends:
`destinationKind`, `failureStage`, `preflightMaxSendableSat`, `reasonClass`,
`timeoutSecs`, `errorName`, `errorCode`, and `messageFingerprint`.
`messageFingerprint` is a SHA-256 correlation value for the raw daemon message;
the raw text itself is not returned or stored.

## Orrery Lightning Address retry notes

On 2026-06-17, the operator retried Orrery through the Spark-backed Lightning
Address fallback using the admin payout route. The small-to-large sequence was:

- 5,000 sats: succeeded and settled.
- 40,000 sats: failed before dispatch; treasury balance and max-sendable were
  unchanged afterward, proving no sats left the treasury on that attempt.
- 20,000 sats: succeeded and settled.
- 25,000 sats: succeeded and settled.
- Post-diagnostics 5,000 sats: succeeded and settled. No failure diagnostics
  were produced because the payment succeeded.
- Post-#5173/fingerprint deploy:
  - 5,000 sats: succeeded and settled.
  - 50,000 sats: failed before dispatch with `errorCode: genericfailure`,
    `failureStage: pay_throws`, `destinationKind: bolt11`,
    `preflightMaxSendableSat: 64795`, no payment id, and unchanged treasury
    balance.
  - 40,000 sats: same `genericfailure` / same fingerprint / no balance
    movement.
  - 30,000 sats: same `genericfailure` / same fingerprint / no balance
    movement.
  - 25,000 sats: succeeded and settled.

This proves the Lightning Address fallback can resolve and receive real treasury
payouts at meaningful sizes, and that the 50,000-sat recognition amount is
covered by split settled sends. The large-send failure is now bracketed more
tightly: 25,000 succeeds while 30,000, 40,000, and 50,000 fail before dispatch
with the same upstream `GenericFailure` fingerprint. That makes the current best
explanation a recipient-rail / LSP / SDK large-invoice failure on the resolved
BOLT11 path, not treasury insufficient balance and not recipient readiness. Do
not send single invoices above 25,000 sats on this rail until the upstream cause
is fixed or disproven.

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
