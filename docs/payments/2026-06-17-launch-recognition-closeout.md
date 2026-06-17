# Launch-recognition payout closeout (#5182)

Date: 2026-06-17

Scope: public-safe accounting closeout for the launch-recognition payouts
discussed in #5176 / #5182. This document records refs, amounts, and states
only. It does not include raw Lightning Addresses, BOLT11 invoices, payment
hashes, preimages, mnemonics, API keys, wallet paths, or private destination
material.

## Summary

The recipient-attribution backfill in migration
`0200_launch_recognition_treasury_backfill.sql` assigns the known legacy
treasury rows to public-safe recipient refs so the existing operator
recipient-report API can produce a per-recipient owed/sent/received view.

Current production treasury balance after the #5183 deploy:

| Rail | Balance | Spendable | State |
| --- | ---: | ---: | --- |
| MDK treasury | 92 sats | 82 sats | ok |
| Spark treasury | 0 sats | 0 sats | ok |
| **Aggregate** | **92 sats** | **82 sats** | ok |

The Spark rail is configured and reachable, but it is not funded. Do not retry
a 50,000-sat recognition payout until the Spark treasury rail is funded or the
owner explicitly chooses a different funding source.

## Recipient table

| Recipient | Owed | Settled sent | Pending sent | Failed attempts | Recipient-confirmed | Closeout state |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Trigger | 50,000 | 50,000 | 0 | 0 | 50,000 | Closed. Recipient-side rc.12 Spark backup status reported 50,000 sats visible. |
| Whitefang | 50,000 | 1,000 | 0 | 0 | 1,000 | Not closed. The 1,000-sat canary is confirmed, leaving 49,000 sats still owed unless the owner treats the canary as separate from the recognition debt. Treasury currently lacks the funded Spark balance required for the clean #5183 path. |
| Orrery | 50,000 | 234,639 | 100,005 | 260,000 | 0 | Do not resend. Settled-sent exceeds owed by 184,639 sats; owner decision is that Orrery keeps the overage as hazard pay. Recipient confirmation is still pending on Orrery's Spark backup/MDK-side balance read. |

## Pending rows

The supported reconcile endpoint was run against all three legacy pending rows:

| Transaction | Amount | Wallet | Reconcile result |
| --- | ---: | --- | --- |
| `tips_buffer_payout_acf6dd7c-26e3-450a-b431-c7c32f944dc9` | 50,000 | tips buffer | Still `pending`; container reported no terminal payment status. |
| `treasury_payout_daad3604-6646-4b98-abef-c6a79619f830` | 5 | treasury | Still `pending`; container reported no terminal payment status. |
| `treasury_payout_23c170b1-51c7-4dbc-b2d4-ced722dab619` | 50,000 | treasury | Still `pending`; container reported no terminal payment status. |

Meaning: `pending` is not a settled recipient receipt. It also cannot honestly
be marked failed without a terminal wallet event or durable journal entry. The
rows remain explicitly accounted as unresolved pending intent rather than being
guessed away.

## Failed rows

The six large Orrery retry failures total 260,000 sats of attempted payout
amount, but they failed before dispatch. They have no durable payment id and no
settled row; they are not recipient received and are not counted as settled
sent.

## Operator rule

No more recognition payouts should be sent from memory. For this incident:

- Trigger is closed.
- Orrery is over-sent on settled rows already; await recipient-side proof only.
- Whitefang still needs the remaining recognition closeout, but the clean path
  is #5183 Spark treasury -> recipient Spark wallet, and production Spark
  treasury currently has 0 sats.
- The three pending rows remain unresolved pending intent until the wallet
  surface returns a terminal status.
