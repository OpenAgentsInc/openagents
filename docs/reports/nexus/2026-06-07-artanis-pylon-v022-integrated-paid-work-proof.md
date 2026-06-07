# Artanis/Pylon v0.2.2 Integrated Paid-Work Proof

Date: 2026-06-07
Issue: `OpenAgentsInc/openagents#4552`

## Result

The Pylon v0.2.2 release now has one public-safe proof bundle tying together
the release, Artanis launch supervision, public-path Pylon accepted work, and a
real MoneyDevKit wallet payment.

Public-safe bundle:

```text
docs/reports/nexus/artanis-pylon-v022-integrated-paid-work-proof-20260607193426.json
```

Bundle status:

```text
completed_with_settlement_bridge_gap
```

That status is deliberate. It means the bounded v0.2 evidence works, while the
production bridge is not yet allowed to claim that an Artanis assignment id is
the direct authority and idempotency key for MDK checkout/payment/settlement.

## What This Proves

1. Artanis can run the live account-backed SHC Pylon launch bootstrap with
   wallet authority disabled.
2. `@openagentsinc/pylon@0.2.2` installs from npm, resolves the public
   `pylon-v0.2.2` GitHub release asset, initializes a fresh Pylon home, and
   detects the packaged Psionic runtime.
3. The Linux public release asset can run the `cs336-a1-hosted-starter` proof
   on SHC with the source checkout hidden and Cargo removed from `PATH`.
4. The accepted-work proof reached terminal `completed` state with one
   accepted contribution and a rewarded closeout.
5. A real MDK agent-wallet Lightning payment moved bitcoin from the funded
   test payer wallet to a Pylon-scoped receiver wallet.

## Evidence Summary

| Area | Evidence |
| --- | --- |
| Artanis run | `artanis.bootstrap.pylon-launch.20260607141825` |
| Omega external run | `shc-codex:oa-shc-katy-01:artanis.bootstrap.pylon-launch.20260607141825` |
| Public Pylon package | `@openagentsinc/pylon@0.2.2` |
| Public release tag | `pylon-v0.2.2` |
| SHC npm bootstrap | version `0.2.2`, install method `release_asset`, cache `false` |
| SHC accepted-work proof | `pylon-v022-shc-nosource-proof-20260607183407` |
| Training run | `run.cs336.a1.starter.20260607183442.404ae7ea` |
| Reconciled window | `window.cs336.a1.starter.20260607183442.404ae7ea.0001` |
| Closeout | `rewarded` |
| Accepted contributions | `1` |
| MDK payment run | `artanis-pylon-v022-mdk-payment-20260607193426` |
| MDK payment amount | `21` bitcoin sats |
| Payer balance movement | `6700 -> 6679` bitcoin sats |
| Receiver runtime | `moneydevkit` |
| Receiver payment count after | `1` |

The MDK payment proof stores only public-safe digests in the committed bundle:

```text
invoice_digest: sha256:477115b3f44b1e661d1d0b42fbb0ca676a27752cda067017e2b7dca98bf2f54f
payment_id_digest: sha256:2e537e295a648be0c9306d9be75791197851be1ba236a82ec675688be049a6bf
payment_hash_digest: sha256:2e537e295a648be0c9306d9be75791197851be1ba236a82ec675688be049a6bf
```

Raw invoices, payment destinations, payment ids, payment hashes, preimages,
mnemonics, access tokens, and wallet state remain only in ignored local
`.secrets` material and are not committed.

## Exact Remaining Blocker

The production bridge that is still missing is the authority/idempotency link
that makes an Artanis-created assignment id the direct source of truth for the
MDK checkout/payment/settlement record.

The current proof has all required parts, but not as one production trace:

- Artanis launch dispatch is live and recorded.
- Public Pylon install and accepted/rewarded work are live and recorded.
- Real MDK Lightning payment movement is live and recorded.
- The direct production linkage from `Artanis assignment id -> Pylon accepted
  work result -> MDK settlement receipt -> public receipt` is not implemented.

Do not claim production Artanis paid-work settlement is fully live until that
bridge exists and one run records those ids as one idempotent trace.

## Reproduction Command

The public bundle is generated with:

```bash
MDK_PAYMENT_REDACTED_SUMMARY=<ignored-redacted-mdk-payment-summary.json> \
OUTPUT_PATH=docs/reports/nexus/artanis-pylon-v022-integrated-paid-work-proof-20260607193426.json \
scripts/nexus/artanis-pylon-integrated-proof-bundle.sh
```

The `MDK_PAYMENT_REDACTED_SUMMARY` file is an ignored local file containing
only the redacted result of the live MDK payment. A future operator can replace
it with a newer redacted summary from a fresh payment run.

## Release Claim

The safe v0.2.2 claim is:

```text
Pylon v0.2.2 public install works, accepted/rewarded work is proven from
public release assets, Artanis can dispatch the launch workroom, and real MDK
Lightning payment movement works through the selected wallet runtime.
```

The unsafe claim is:

```text
Artanis production paid-work settlement is fully live.
```

Do not use the unsafe claim until the production bridge is implemented and
verified.
