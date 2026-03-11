# Mission Control Buy Mode Payments

This note defines the desktop behavior for Mission Control `Buy Mode` payment
state and observability in the current MVP build.

## Contract

Mission Control `Buy Mode` remains a one-request-at-a-time smoke-test buyer
lane.

For a single request, the desktop now tracks and shows:

- request publish state
- provider feedback / result state
- the Spark wallet payment pointer as soon as Spark accepts the send attempt
- terminal wallet success before showing buyer success
- terminal wallet failure detail without discarding the original payment pointer

## Payment State Semantics

The inline Mission Control `PAY` cell and the buy-mode payment history pane use
the following meanings:

- `idle`: no provider invoice or wallet send attempt exists yet
- `invoice`: provider requested payment but no Spark payment pointer exists yet
- `queued`: provider invoice is present and the Spark send command is queued
- `pending`: Spark assigned a payment pointer and wallet confirmation is still pending
- `sent`: wallet reached a terminal success state
- `failed`: wallet reached a terminal failed state
- `returned`: reserved for HTLC-return detail when the wallet layer exposes it

Terminal buyer success is still wallet-authoritative. A provider result alone is
not enough.

## Log Stream

Mission Control log mirroring now includes:

- `autopilot_desktop::buyer`
- `autopilot_desktop::buy_mode`
- selected `autopilot_desktop::input` UI errors for `network.requests` and `spark.wallet`
- selected `breez_sdk_spark::sdk` Lightning-send polling lines

That keeps the Mission Control console focused on buyer-payment truth instead of
every Spark sync line.

## History Pane

The `Buy Mode Payments` pane now retains and renders, when available:

- timestamp
- request id
- wallet payment pointer
- request and result event ids
- provider pubkey
- Lightning destination pubkey
- payment hash
- wallet method
- wallet-side status detail
- compact invoice snippet

The payment pointer is preserved even for failed buyer payments so operators can
correlate wallet events and request outcomes after failure.

## Breez 0.6.6 Limitation

The pinned `breez-sdk-spark` dependency used by this repo does not persist the
full Lightning terminal reason into the public wallet payment record for Bolt11
sends.

In practice:

- Mission Control can mirror the raw Breez polling line, for example
  `Polling payment status = failed UserSwapReturned`
- the wallet payment history retains payment hash, destination pubkey, invoice,
  and a generic Lightning failure detail
- the exact Breez terminal label such as `UserSwapReturned` is currently a log-stream
  truth, not a stored wallet-history field

That is why the history pane is explicit about wallet evidence and pointers, and
the console mirrors the relevant Breez send-status lines inline.
