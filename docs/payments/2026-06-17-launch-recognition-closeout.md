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

Current production treasury balance after the #5183 deploy and live Spark rail
smokes:

| Rail | Balance | Spendable | State |
| --- | ---: | ---: | --- |
| MDK treasury | 15 sats | 5 sats | ok |
| Spark treasury | 41 sats | 41 sats | ok |
| **Aggregate** | **56 sats** | **46 sats** | ok |

The Spark rail is configured, reachable, and live-spend tested. The operator
moved 75 sats from MDK treasury into Spark treasury, then sent two real
Spark-treasury outbound smokes to Whitefang's Spark-backed Lightning Address:
5 sats settled with a 7-sat Spark balance delta, and 25 sats settled with a
27-sat Spark balance delta. Earlier Spark-preferred BOLT11 attempts failed
before dispatch because the Breez Spark SDK returned
`invalid_transferid_format`; the deployed container now retries that exact
validation failure with `preferSpark:false`, so the treasury still spends from
the Spark wallet while avoiding the broken Spark-preferred BOLT11 metadata
path.

## Recipient table

| Recipient | Owed | Settled sent | Pending sent | Failed attempts | Recipient-confirmed | Closeout state |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Trigger | 50,000 | 50,000 | 0 | 0 | 50,000 | Closed. Recipient-side rc.12 Spark backup status reported 50,000 sats visible. |
| Whitefang | 50,000 | 1,030 | 0 | 0 | 1,000 | Not closed. The 1,000-sat canary is confirmed, and two Spark-treasury smokes totaling 30 sats settled after #5183. Remaining recognition closeout needs a funded Spark treasury balance. |
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
- Whitefang still needs the remaining recognition closeout. The clean path is
  now proven as #5183 Spark treasury -> Whitefang Spark-backed Lightning
  Address; fund the Spark treasury rail first, then send the remaining
  recognition amount plus expected routing fee under a fresh operator
  idempotency key.
- The three pending rows remain unresolved pending intent until the wallet
  surface returns a terminal status.
