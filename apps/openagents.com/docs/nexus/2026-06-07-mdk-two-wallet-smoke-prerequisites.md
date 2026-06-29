# MDK Two-Wallet Smoke Prerequisites

Date: 2026-06-07
Related issues: #431, #436

Status: prerequisites satisfied and #431 smoke executed. The evidence record
lives in `docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md`.

## Purpose

#431 is the first real bitcoin movement smoke for the OpenAgents product surface/Nexus payout path.
It must move a tiny amount from an isolated OpenAgents treasury test wallet to
an isolated Pylon edge test wallet, then record public-safe OpenAgents product surface receipts.

This prerequisite checklist defines what must exist before that smoke can run.
Issue #431 has now run against those prerequisites. This document remains the
setup source of truth; the evidence doc records the completed movement proof.

## Required Non-Secret State

The smoke is ready only when all of these public-safe facts are true:

| Gate | Required state | Public-safe record |
| --- | --- | --- |
| Treasury wallet existence | An isolated OpenAgents treasury test wallet exists under operator control. | `wallet.test.openagents_treasury` or another stable redacted wallet ref. |
| Pylon wallet existence | A separate isolated Pylon edge test wallet exists under operator or Pylon-local control. | `wallet.test.pylon_edge` or another stable redacted wallet ref. |
| Funding readiness | The treasury test wallet has enough spendable bitcoin for the bounded test amount plus fee buffer. | `balance.mdk_agent_wallet.minimum_satisfied`; never the exact balance. |
| Receive readiness | The Pylon edge test wallet can produce a fresh Lightning invoice or other supported destination for the smoke. | A redacted invoice/destination ref such as `invoice.redacted.mdk_agent_wallet.<digest>`. |
| Payout target approval | OpenAgents product surface has an active approval for the redacted Pylon payout target. | `payout_target_approval.<stable-ref>` plus `payout_target.public.<digest>`. |
| Accepted work evidence | The test has a tiny accepted-work fixture or operator-test assignment. | `accepted_work.test.<stable-ref>` or `assignment.test.<stable-ref>`. |
| Adapter readiness | The MDK agent-wallet adapter is available and can run JSON CLI commands through an operator-controlled executor. | `executor.mdk_agent_wallet.<stable-ref>` and bounded command status refs. |
| Settlement receipt readiness | OpenAgents product surface can persist payout intent, attempt, reconciliation, and payment authority receipts. | `receipt.nexus.<stable-ref>` refs only. |

All refs must be stable, public-safe identifiers. They may describe readiness
buckets and digests, but they must not contain raw local paths, hostnames,
mnemonics, wallet config, invoices, payment hashes, preimages, exact balances,
private payout targets, access tokens, or provider credentials.

## Wallet Isolation Rules

Use two separate wallet homes. Do not reuse a personal wallet, production
customer wallet, production Pylon wallet, or default wallet if its provenance is
unclear.

The local wallet homes may live in an ignored operator-owned directory such as
`.secrets/`, or outside the repository. OpenAgents product surface now ignores `.secrets/` and
`.mdk-wallet/`, but the safer practice is to keep all wallet homes outside the
tracked repo or under an ignored path.

Do not commit:

- `config.json`;
- mnemonic or seed phrase;
- daemon state;
- payment history;
- raw invoice;
- payment hash;
- preimage;
- exact balance;
- raw payout destination;
- local wallet home path when it reveals a user or host; or
- provider access tokens or webhook secrets.

## Funding Requirement

The treasury test wallet must receive real inbound bitcoin before #431 runs.
Repo code cannot manufacture spendable balance. The MoneyDevKit preview
fake-payment path is not valid evidence for #431.

The readiness check should compare the wallet balance against the planned smoke
amount plus a fee buffer, but any public or durable record should store only a
bucketed readiness ref:

- `balance.mdk_agent_wallet.minimum_satisfied`; or
- `balance.mdk_agent_wallet.minimum_not_satisfied`.

Do not record the exact wallet balance in Pylon API events, payout receipts,
Forum posts, docs, issue comments, or public pages.

## Operator Flow For #431

When #431 runs, the operator or agent should perform these steps without
printing private material:

1. Confirm both isolated wallet homes exist.
2. Confirm the treasury wallet readiness check returns a minimum-satisfied ref.
3. Create a fresh Pylon edge receive destination in private execution context.
4. Store only the redacted invoice/destination digest in OpenAgents product surface.
5. Create or select the payout target approval for that redacted target.
6. Create a tiny accepted-work or operator-test fixture.
7. Create an OpenAgents product surface payout intent with a bounded spend cap and stable
   idempotency key.
8. Dispatch through the MDK agent-wallet adapter.
9. Reconcile with MDK payment history or provider status.
10. Persist payout attempt, reconciliation, verification, settlement, and
   payment authority receipt records.
11. Publish only public-safe receipt projections.

If any step would require pasting an invoice, preimage, mnemonic, exact
balance, wallet path, or raw destination into a tracked file, issue comment,
Forum post, or browser-visible response, stop and move that material into the
private executor boundary.

## Public Receipt Shape

The #431 public receipt can say:

- a real MDK agent-wallet adapter attempted a bounded bitcoin payout;
- the source was the isolated OpenAgents treasury test wallet ref;
- the destination was an approved redacted Pylon edge payout target ref;
- the payout amount bucket or configured cap;
- the adapter attempt ref;
- the redacted payment ref digest;
- the reconciliation result ref;
- the settlement receipt ref; and
- the idempotency/ref replay status.

It must not say:

- raw invoice;
- raw payment hash;
- preimage;
- mnemonic;
- wallet config;
- wallet home path;
- exact wallet balance;
- private payout target;
- provider access token;
- webhook secret; or
- private customer or operator data.

## Current Code Alignment

The MDK agent-wallet adapter now reports wallet readiness as bucketed refs
instead of exact balances. Pylon wallet-readiness API payloads reject exact
`balance.mdk_agent_wallet.<number>` refs and `balance_sats`-style material.

The existing adapter boundary is still command-executor based. The executor is
where private raw destinations may be resolved locally. Durable OpenAgents product surface records
should receive only redacted destination and payment refs.

## #431 Completion State

#431 executed the live movement run:

- identified the two isolated wallet homes;
- confirmed bucketed treasury funding readiness;
- approved the redacted Pylon payout target;
- created the tiny operator-test fixture;
- dispatched and reconciled the payout through OpenAgents product surface authority;
- proved idempotency prevented double spend; and
- wrote the public-safe receipt chain.

See `docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md` for the redacted
receipt refs and public URL.
