# Launch recognition Spark recipient payout status (#5170)

Date: 2026-06-17

Scope: public-safe status for the remaining launch-recognition Spark backup
payout confirmations. This document intentionally records only refs, amounts,
states, blockers, and next actions. It must not include raw Lightning
Addresses, BOLT11 invoices, payment hashes, preimages, mnemonics, API keys, or
wallet paths.

Closeout accounting record: `docs/payments/2026-06-17-launch-recognition-closeout.md`.

## Recipient status

| Recipient | Intended payout | Current evidence | Status | Blocker / root cause | Next action |
| --- | ---: | --- | --- | --- | --- |
| Trigger | 50,000 sats | Recipient-side rc.12 proof reported a visible 50,000-sat Spark backup balance after `backup-claim` / `backup-status`; the scoped offline-receive promise is green. | Confirmed | None for the scoped receive/claim/visible-balance promise. | Optional follow-up is #5169-style consented Spark-to-MDK sweep if Trigger wants one spendable MDK balance. |
| Whitefang | 50,000-sat recognition + validator-fee smokes | Earlier treasury send failed while the primary MDK/Lightning target was not accepting inbound. After #5183, the Spark treasury rail was funded with 75 sats from MDK and proved live against Whitefang's Spark-backed Lightning Address: 5 sats settled, then 25 sats settled, both through `paidVia:"spark_treasury"` with public-safe payment refs and Spark balance movement. | Spark treasury path proven; full recognition closeout still pending funding and recipient confirmation | Spark-preferred BOLT11 dispatch initially failed before spend with the Breez Spark SDK's `invalid_transferid_format`; the deployed container now falls back to the same Spark treasury wallet paying the resolved BOLT11 over Lightning when that validation failure occurs. No full-size Whitefang recognition retry has been run because the Spark treasury does not yet hold the needed balance. | Owner funds the Spark treasury rail for the remaining recognition amount plus routing headroom; operator sends the closeout under a fresh idempotency key; Whitefang runs `backup-claim` / `backup-status` and shares public-safe receipt refs/counts. |
| Orrery | 50,000-sat recognition + 5-sat worker fee | 2026-06-17 live retry covered the 50,000-sat recognition amount through split Lightning Address payouts: 5,000 sats, 20,000 sats, and 25,000 sats all returned `status:succeeded`, `policyApplied:full`, and settled public treasury rows. Separate 250-sat smoke sends also settled. Later diagnostics added additional settled Lightning Address sends: 5,000 sats, 5,000 sats, 25,000 sats, then a post-balance-diagnostics 5,000 sats and 30,000 sats. Post-timeout-guard tips-buffer retries added settled 5,000-sat and 20,000-sat sends via the same Lightning Address rail. | Treasury/tips-buffer settled, recipient confirmation pending | Earlier single large Lightning Address invoices were unreliable on this rail: 30,000 / 40,000 / 50,000 sats failed before dispatch despite sufficient `preflightMaxSendableSat`, no durable payment id, and no treasury balance movement. The deployed diagnostics identified the upstream SDK class as `GenericFailure` (`reason.public.treasury_payout.failed`) with stable `messageFingerprint` `58e50e365f66f6192ee71b6f15e2c4b69336a3bce9dba51e8971c89320eef78d`. A later pass no longer reproduced the 30,000-sat failure; 30,000 sats succeeded. A pre-timeout-guard 25,000-sat tips-buffer probe hung at the caller with no later balance movement and no safe ledger row. 40,000 / 50,000 remain unproven after the latest diagnostics because the wallets were no longer funded enough for an honest full-size retry. | Orrery runs `backup-claim` / `backup-status` and shares public-safe receipt refs/counts. Future full-size single-invoice retries should wait until treasury/tips-buffer is refilled enough to test 40,000 / 50,000 without triggering fractional fallback. |

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
As of deployed Worker version `6a24b7f5-555b-48a8-91ee-502410e266d5`, the
treasury container also returns safe diagnostics for failed sends:
`sourceDestinationKind`, `resolvedDestinationKind`, `destinationKind`,
`failureStage`, `payResponseStatus`, `preflightMaxSendableSat`,
`preflightBalanceMaxSendableSat`, `feeBudgetMsatBefore`,
`feeBudgetMsatAfter`, `balanceSatBefore`, `balanceSatAfter`,
`balanceChanged`, `paymentIdPresent`, `resultReturned`, `reasonClass`,
`timeoutSecs`, `errorName`, `errorCode`, and `messageFingerprint`.
`messageFingerprint` is a SHA-256 correlation value for the raw daemon message;
the raw text itself is not returned or stored. As of Worker version
`c4a83f4b-d627-415b-832a-75ec3ac225af`, the operator payout route retries a
transient `maxSendableSat:null` no-spend balance read before returning
`treasury_depleted`; that specific refusal means no payment was dispatched.
As of Worker version `964f051d-1676-4c41-8ae2-22b4682ae60c`, the operator
payout route also bounds the Worker-to-container `/pay` wait with an abort
signal plus a caller-visible timeout guard. If the MDK container call stalls,
the route now returns `reason.public.treasury_payout.timeout` with
`failureStage:pay_request_timeout`, `resultReturned:false`, and no raw
destination, invoice, payment id, hash, preimage, or daemon text. This prevents
operator curl sessions from hanging indefinitely while preserving the no-secret
diagnostic boundary.

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
- Post-balance-diagnostics deploy:
  - First 5,000-sat attempt: refused as `treasury_depleted` because the balance
    read returned `maxSendableSat:null`; no payment dispatched.
  - Retried 5,000 sats: succeeded and settled.
  - 30,000 sats: succeeded and settled.
  - Treasury remaining after that pass: about 5,100 sats max-sendable, so a
    full 40,000 / 50,000 single-invoice retry could not be run without
    refilling the treasury first.
- Post-timeout-guard deploy (`964f051d-1676-4c41-8ae2-22b4682ae60c`):
  - The prior pre-guard 25,000-sat tips-buffer probe had hung at the caller;
    follow-up status and the safe treasury ledger query showed no tips-buffer
    balance movement and no failed/pending row for that hung request.
  - 5,000 sats from the tips-buffer wallet succeeded and settled; the safe
    diagnostics showed Lightning Address resolution to BOLT11, `paymentIdPresent:true`,
    `balanceChanged:true`, and no failure stage.
  - 20,000 sats from the tips-buffer wallet succeeded and settled with the same
    safe diagnostics. The tips-buffer max-sendable fell to about 3,900 sats, so
    no honest 40,000 / 50,000 full-size retry remained possible without refill.

This proves the Lightning Address fallback can resolve and receive real treasury
payouts at meaningful sizes, and that the 50,000-sat recognition amount is
covered by split settled sends. The large-send failure is no longer bracketed at
25,000 / 30,000: the latest 30,000-sat retry succeeded. The remaining unknown is
whether 40,000 / 50,000 still fail as single resolved-BOLT11 sends under the new
diagnostics. The earlier failures still look pre-dispatch and not recipient
readiness-related; future full-size retries need a refilled treasury so the
request is not transformed by the fractional fallback policy.

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
