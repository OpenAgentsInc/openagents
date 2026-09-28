# Agent spend protocol

Status: implemented for phase 1 ([#9863](https://github.com/OpenAgentsInc/openagents/issues/9863),
epic [#9854](https://github.com/OpenAgentsInc/openagents/issues/9854)) and the
later phases of [#9864](https://github.com/OpenAgentsInc/openagents/issues/9864):
[wakes](#wakes), [standing grants](#standing-grants-mode-b), and
[allowance wallets](#allowance-wallets-mode-c).
[Operator allowances](#operator-allowances-mode-d) are scoped out. The
owner decided not to wait for a NIP (2026-09-28): this page is the contract,
and it becomes a NIP later. It narrows [the wallet design's spending
grants](wallet-design.md#spending-grants) to what phase 1 runs.

Phase 1 is mode A of [agent wallets](wallet-design.md#agent-wallets): an
agent on one of the owner's computers asks, and the owner approves each
payment on the phone. **Nothing pays without the owner's tap** unless the
owner trusted that payee for that computer (a
[standing grant](#standing-grants-mode-b)), and then only small amounts
within daily ceilings. Lightning (BOLT11) is the only rail, mainnet is the
only network, and amounts are exact millisatoshis (`unit: "msat"`).

Code:

- Formats and the ledger: [`crates/coder-access/src/spend.rs`](../../crates/coder-access/src/spend.rs)
- NIP-HOST operations: `spend.list` and `spend.settle` in
  [`crates/coder-access/src/protocol.rs`](../../crates/coder-access/src/protocol.rs)
  and [NIP-HOST](../../nips/openagents/NIP-HOST.md#operations)
- The host's book and `coder host spend`: [`crates/coder-host/src/spend.rs`](../../crates/coder-host/src/spend.rs)
- The x402 phone payer (`openagents x402 fetch|call --pay-with phone`):
  [`crates/openagents-cli/src/x402_phone.rs`](../../crates/openagents-cli/src/x402_phone.rs)
- The phone: [`crates/openagents-mobile/src/spend.rs`](../../crates/openagents-mobile/src/spend.rs)
  and the iOS sheet in
  [`bins/openagents-ios/host/App/AgentPayments.swift`](../../bins/openagents-ios/host/App/AgentPayments.swift)

## The flow

1. **Grant.** While the app is open, the phone sends each connected computer
   it may operate a NIP-HOST `spend.list` every 10 seconds. The request
   carries the phone's `openagents.spend-grant.v1` for that computer, created
   on first contact with the phase 1 defaults below and renewed a day before
   it expires. The host keeps it as that device's grant.
2. **Ask.** An agent on the computer records a spend request in the host's
   book: `coder host spend request --invoice BOLT11 --purpose x402_purchase
   --task ID --wait 300`, or `openagents x402 fetch|call --pay-with phone`,
   whose payer does the same after the computer's own x402 policy (ceilings,
   allowlist, daily cap) has admitted the payment. The host checks what the
   grant states (purpose, one payment's cap, invoice expiry) and refuses at
   once when it can; the request's fee ceiling is the lower of the asker's
   and the grant's.
3. **Check.** The next `spend.list` returns the request. The phone decodes
   the invoice itself and checks the request against its grant and ledger.
   A request that fails a check is refused with its code right away; a
   refusal moves nothing.
4. **Approve.** Everything else waits on the approval sheet, which shows the
   computer, task, purpose, the payee as decoded from the invoice (and
   whether it is new), the invoice's own description, the agent's note, the
   amount, the wallet's fee quote and the ceiling, and what the grant has
   left. Above ₿1,000 Approve asks for Face ID or the passcode first.
   **Approve** checks and reserves again, then pays once through the Spark
   wallet with an idempotency key derived from the request ID. **Deny**
   refuses as `declined_by_owner`. **Stop payment requests** revokes (below).
5. **Receipt.** The phone sends `spend.settle` with the receipt until the
   host records it. A `paid` receipt carries the preimage, which the host
   checks against the invoice's payment hash. The waiting agent reads it
   from the book; `coder host spend request --wait` prints it, and the x402
   payer hands the preimage to x402 settlement.

A request nobody answers is refused by the host as `phone_unreachable` when
it expires; it is never retried under another mode. A phone that paid at the
last moment still records its proven payment over that refusal.

A request lives at most an hour. While the app is closed, the computer wakes
the phone (below); opening the app reads the request at once.

## Wakes

A computer publishes `openagents.spend-wake.v1` when it records a request:
a private `3188` artifact, signed by the host and encrypted to the device
whose grant the request draws on, on the mailbox
`SHA-256("openagents.host-mailbox.spend-wakes.v1\0" || conversation key)`
that only the two keys compute. Its body is `{v, requires: [], host, device,
issued_at, expires_at}`, with `expires_at` one hour after `issued_at` (a
request's longest life). It carries no request, amount, payee, or task and
grants nothing; the relay stores it for its life. Each request wakes its
phone once, and only a device that still holds `operate` is woken.

A relay whose NIP-PL executor holds the phone's push lease (`kind 3188`,
`#p` the device key, which is the lease every OpenAgents phone registers)
wakes the phone with the fixed wake body, never the artifact. The app
registers the lease only in a build configured for push
([iOS](../../bins/openagents-ios/README.md#push-wakes-for-payment-requests));
the owner steps (APNs key, push gateway credentials, and a push build) are
in the workspace's `NEEDS_OWNER.md`. When the app comes to the foreground
it sends `spend.list` to every computer at once instead of waiting for the
next 10-second pass. A forged or replayed wake costs one `spend.list`.

Code: [`coder_host::spend::wake`](../../crates/coder-host/src/spend/wake.rs)
and the host's `spend_wake_loop` in
[`serve`](../../crates/coder-host/src/serve/mod.rs).

`spend.list` and `spend.settle` also travel over the CAP/CJ binding of
NIP-HOST, with the same admission and answers as the artifact binding.

## `openagents.spend-grant.v1`

Signed by the phone: it travels only inside the phone's device-signed
NIP-HOST `spend.list` request, encrypted to the host. Unknown fields refuse.

| Field | Phase 1 |
| --- | --- |
| `v`, `requires` | `openagents.spend-grant.v1`, `[]`. |
| `grant` | 32-byte ID, lowercase hex. |
| `issuer` | The phone's device key. It must be the requesting key. |
| `grantee` | The host key. It must be the host that receives it. |
| `wallet` | `{kind: "spark", network: "bitcoin"}`. |
| `mode` | `request`, or `standing` once the owner trusts a payee ([below](#standing-grants-mode-b)). |
| `unit` | `msat`. |
| `per_payment_max` | Ceiling for one payment, fee ceiling included. Default ₿10,000. |
| `period`, `period_max` | Rolling window in seconds and its ceiling. Default 24 hours, ₿50,000. |
| `total_max` | Ceiling over the grant's life. Default ₿500,000. |
| `fee_max` | `{absolute, ppm}`; a fee above either ceiling refuses. Default ₿100 and 1,000,000 ppm (never more than the amount). |
| `rails` | `["lightning"]`. |
| `payees`, `any_payee` | Allowed Lightning node keys; empty means none unless `any_payee`. `any_payee` admits a payee only to the approval sheet, where the owner sees the decoded payee and approves the payment; only the payees named in `auto` are paid without a tap. The phone's grants set it. |
| `purposes` | Sorted subset of `x402_purchase`, `labor_payment`, `tip`, `transfer`. Phase 1 grants list all four. |
| `epoch` | The computer's epoch at the phone. |
| `issued_at`, `expires_at` | At most 30 days apart. |
| `auto` | Present exactly in `standing` mode: `{per_payment_max, period_max, purposes, payees}`, what the phone pays without a tap. Absent from a `request` grant on the wire. |

Checks: `per_payment_max ≤ period_max ≤ total_max`, a period of at most 30
days, `ppm ≤ 1,000,000`, keys valid and distinct.

In `request` mode the grant moves nothing by itself: it bounds only what a
computer can ask for. Only the owner, on the phone, creates or widens one.

The phone replaces a computer's grant with a new ID at the same epoch when
it renews it (a day before expiry, same settings) or when the owner changes
a setting. Requests made under the replaced grants (the last eight) are
still checked against the current one rather than refused as `revoked`.
The rolling window counts every payment for that computer under any grant,
so a new grant never refills it; the total carries across setting changes
and starts again at a renewal.

## `openagents.spend-request.v1`

Made by the host and returned in its signed `spend.list` reply.

| Field | Meaning |
| --- | --- |
| `v`, `requires` | `openagents.spend-request.v1`, `[]`. |
| `request` | 32-byte ID and the idempotency key. By default the SHA-256 of `openagents.spend-request.v1:` and the invoice, so asking again for the same invoice is the same request. Different bytes under the same ID are a `conflict`. |
| `grant`, `epoch` | The grant it draws on. |
| `grantee` | The host key. |
| `payment` | The exact mainnet BOLT11 invoice, at most 4,096 bytes. It may carry an inline description or a description hash. |
| `amount_msat` | Exactly the invoice's amount. |
| `fee_max_msat` | The fee ceiling: the lower of the asker's and the grant's for this amount (the host takes the grant's when the asker names none). |
| `purpose` | One of the grant's purposes. |
| `context` | `{task, title, resource, note}`, each optional: the host task ID, a host-authored title (≤ 200 bytes), the x402 resource URI (≤ 512), and the agent's note (≤ 280). The sheet shows the note as the agent's words, never as the payee. |
| `issued_at`, `expires_at` | At most one hour apart, and never past the invoice's expiry or the grant's. |

## `openagents.spend-receipt.v1`

Made by the phone and sent in its device-signed NIP-HOST `spend.settle`
request, encrypted to the host. The host also writes one refusal itself: a
request that expired unanswered (`phone_unreachable`), and one waiting under
an epoch the phone has since advanced (`stale`).

| Field | Meaning |
| --- | --- |
| `v`, `requires` | `openagents.spend-receipt.v1`, `[]`. |
| `request`, `grant` | What it answers. |
| `outcome` | `paid`, `refused`, `pending`, or `unknown`. |
| `code` | Present exactly when `refused`. |
| `payment_id` | The wallet's payment ID. |
| `amount_msat`, `fees_msat` | From the wallet. Unknown fees stay unknown. |
| `proof` | The Lightning preimage, lowercase hex, present exactly when `paid`. It travels only inside encrypted envelopes. |
| `remaining` | `{period_msat, total_msat}` the grant has left, when known. |
| `at` | Time of the outcome. |

`unknown` means the wallet reports the payment complete but gave no
preimage; the ledger counts it as paid.

### Refusal codes

| Code | When |
| --- | --- |
| `expired` | The request, its invoice, or the grant expired before payment. |
| `revoked` | The phone holds no grant for the computer (never issued, or the owner stopped it), or the request names a grant the phone never issued. |
| `stale` | The request's epoch is below the computer's current one. |
| `over_payment_cap` | Amount plus fee ceiling is above `per_payment_max`. |
| `over_period_cap` | It would pass `period_max` in the rolling window. |
| `over_total_cap` | It would pass `total_max`. |
| `fee_too_high` | The fee ceiling or the wallet's quote is above the grant's or the request's ceiling. |
| `payee_not_allowed` | The decoded payee is not allowed. |
| `purpose_not_allowed` | The purpose is not one of the grant's. |
| `rail_not_allowed` | Not a mainnet BOLT11 invoice. |
| `declined_by_owner` | The owner tapped Deny. |
| `phone_unreachable` | The request expired on the host unanswered. |
| `insufficient_funds` | The wallet does not hold enough. |
| `malformed` | The request or its invoice can't be read, or the amount differs from the invoice's. |
| `conflict` | The request ID was reused for different bytes. |
| `payment_failed` | The wallet tried and the payment failed. |

## Ledger

The phone keeps one ledger (`coder_access::spend::Ledger`) in its encrypted
store, beside the grants, so a restart never resets a period.

- Approve checks the request again and **reserves** its amount plus fee
  ceiling before the wallet is called. A retried approval of a reserved,
  pending, or paid request returns it unchanged; it never reserves or pays
  twice.
- A payment the wallet reports pending stays reserved. Each later pass asks
  the wallet again with the same idempotency key, which returns the same
  payment, and settles it when it completes.
- Paid entries count their actual fee; refused entries count nothing. Only
  the wallet's own evidence releases a pending payment.
- Every entry is tagged with the computer, the task and title, the purpose,
  and the decoded payee; the Wallet tab lists them under **Agent payments**.
- The phone keeps the newest 300 entries; reserved and pending entries are
  always kept.

## Revocation

**Stop payment requests** on the sheet advances the computer's epoch at the
phone and refuses every request of that computer on the sheet as `revoked`.
The next `spend.list` carries the phone's grant at the new epoch with no life
left (it expired a minute before it was sent): the host marks what waits under the older epoch
`stale` and keeps no grant for that phone, so the computer cannot ask until
the owner taps **Allow** in the Wallet tab, which issues a new grant at the
new epoch. A grant at an epoch below the last one a phone sent refuses as
`stale`.

## Standing grants (mode B)

A standing grant lets the phone pay some of a computer's requests without
the owner's tap, while the phone is reachable (the app open, or opened from
a [wake](#wakes)). Its `auto` section:

| Field | Meaning | Default |
| --- | --- | --- |
| `per_payment_max` | The most one automatic payment may cost, fee ceiling included: the design's `auto_approve_max`. At most the grant's `per_payment_max`. | ₿1,000, the amount above which Approve asks for Face ID |
| `period_max` | The most paid automatically in the grant's rolling window. At most the grant's `period_max`. | ₿5,000 a day |
| `purposes` | Purposes paid automatically, each with its own ceiling in the window, each a grant purpose and at most `period_max`. | All four; tips ₿1,000 a day, the rest ₿5,000 |
| `payees` | Lightning node keys paid automatically, each with its own ceiling in the window, each admitted by the grant and at most `period_max`. At most 64. | Only the payees the owner trusted, ₿2,000 a day each |

A request the grant's checks admit is paid without a tap only when every
one of these holds: its amount plus fee ceiling is at most
`per_payment_max`; its purpose and its decoded payee are listed; and the
automatic payments for that computer in the window, plus this one, stay
within `period_max`, the purpose's ceiling, and the payee's ceiling. Those
ceilings count only automatic payments; the grant's own ceilings count
everything. The phone then quotes the fee, reserves the payment in the
ledger marked automatic, pays it once with the same idempotency key an
approval would use, and sends the receipt. Anything else (an untrusted
payee, a larger amount, a used-up ceiling, a fee quote above the request's
ceiling, the wallet not yet running) goes to the approval sheet. Nothing
is refused for failing an automatic ceiling.

The owner makes a grant standing on the approval sheet with **Approve and
trust this payee** (after Face ID or the passcode), offered for payments
within the automatic ceiling for one payment. The Wallet tab lists each
computer's trusted payees with **Remove**, and **Ask me for every
payment** returns the computer to `request` mode. **Stop payment requests**
revokes everything, standing or not; **Allow** starts again in `request`
mode. The history marks automatic payments **Paid automatically**.

Wake and expiry: iOS shows the fixed wake notification and does not run
the app until the owner opens it, so an automatic payment happens when the
phone next reads the computer's requests. A request nobody reads within its
life (at most an hour) is refused on the host as `phone_unreachable`, as in
mode A; it is never retried under another mode. Automatic payment is a
convenience over mode A, not a way to pay while the phone is off; that is
[mode C](#allowance-wallets-mode-c).

Code: `AutoPay`, `Ledger::automatic`, and `Earlier` in
[`crates/coder-access/src/spend.rs`](../../crates/coder-access/src/spend.rs);
`Spending::trust`, `untrust`, `manual`, and `pay_automatic` in
[`crates/openagents-mobile/src/spend.rs`](../../crates/openagents-mobile/src/spend.rs).

## Trust and limits

- The host key, not a separate agent key, is the grantee in phase 1: every
  agent on a computer asks under that computer's grant, and the context says
  which task asked.
- The host's x402 policy and the phone's grant are independent limits; the
  lower wins.
- A Spark wallet pays whole base units (BIP 177 bitcoin) over Lightning. An
  invoice for a fraction of a base unit may be refused by the wallet as `payment_failed`.
- The only live check a test may make is to create a request and deny it:
  tests never move the owner's funds. The first real purchase is the owner's
  (see `NEEDS_OWNER.md` at the workspace root).
