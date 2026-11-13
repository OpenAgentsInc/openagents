# Capstone: a guard's decision is paid for

> **Status: design draft.** This page names a route across five books and
> says which steps have a chapter today. It runs nothing. Steps marked
> *open* have no chapter or no code yet.

A player walks up to Bram, a guard in Everglade. Bram may stay quiet, greet
the player or remark on the weather. A Pylon, someone's gaming computer,
makes that choice. The Pylon's answer is checked, and its owner is paid a
few satoshis for it.

Each book covers one part of this trace. No book follows it end to end.
This capstone joins the parts, so a reader can follow one decision and one
payment through the whole stack.

## Why this route

- It crosses both groups of books: worlds and models (Everglade, the OpenAgents Metaverse Engine,
  Psionic) and money and identity (Nostr, Bitcoin).
- It is small. One enum, one job, one payment.
- Play never depends on it. If the Pylon is slow or absent, the guard
  follows his deterministic rule. That is already the design in
  [Pylon decisions in Everglade](../../metaverse/pylon-guard-decisions.md).
- It exercises every step of [the argument](../README.md#the-argument):
  bounded, deterministic, checkable, payable and ownable.

## The trace

| # | Step | Owner | Today |
| --- | --- | --- | --- |
| 1 | The room decides that Bram is calm and a player is near, and asks for a decision. Only one request is pending in the realm. | [Everglade](../../../everglade/book/README.md) Part VI; [the Metaverse Engine 77](../../../metaverse-engine/book/ch077-people-who-talk-back.md) | The guard's ambient rule is deterministic. The request path is opt-in and shadows the rule. |
| 2 | The game holds a ticket for the request. A late, stale or replaced answer is dropped. | [Psionic 28](../../../psionic/book/ch028-a-decision-arrives-while-play-continues.md) | Chapter written, with a shared consumer in `psionic/lib/game/decision.coax`. |
| 3 | The request becomes a typed decision: three options, the guard's context, one model identity. | [Psionic 12–14](../../../psionic/book/ch012-decisions-have-a-different-mask.md), [20](../../../psionic/book/ch020-serve-systemone.md) | Chapters written. |
| 4 | The buyer finds a Pylon from its signed beacon and sends the job encrypted to the Pylon's key. | [Psionic 24–25](../../../psionic/book/ch024-a-pylon-answers-decisions.md); [Nostr 82](../../../nostr/book/ch082-pylons-environments-and-deploys.md) and [100](../../../nostr/book/ch100-pylons-and-the-mesh.md) | Chapters written. Beacons and service receipts are read and verified. |
| 5 | The Pylon runs Elephant and returns the choice, its probability and the model's identity. | [Psionic 14](../../../psionic/book/ch014-execute-elephant.md), [29](../../../psionic/book/ch029-a-guard-asks-a-pylon.md) | Partly implemented: the shadow path runs. Since 2026-11-13 the guard asks Gemini Decision 1 first, and this step is the Elephant door behind it ([the door order](../../research/2026-11-13-gemini-decision-1.md#everglades-guards)). |
| 6 | The buyer checks the answer. It samples some jobs, runs them again and compares. | [Psionic 195](../../../psionic/book/ch195-check-a-peers-work.md), [219](../../../psionic/book/ch219-how-far-apart-may-two-honest-answers-be.md) | Chapters written. |
| 7 | The buyer's ledger decides what is owed: release after agreement, withhold on disagreement, one payment intent per receipt. | [Psionic 238](../../../psionic/book/ch238-pay-for-verified-work.md) | Partly implemented, in process. |
| 8 | The payment intent becomes a Lightning payment to the Pylon owner's address. | [Nostr 86](../../../nostr/book/ch086-lightning-payments-for-requests.md); [Bitcoin 113](../../../bitcoin/book/ch113-find-a-route-and-pay.md) and [114](../../../bitcoin/book/ch114-invoices.md); [the wallet plan](../../lightning/README.md) | **Open.** No chapter connects step 7's intent to a real payment. |
| 9 | A paid receipt is published, signed by the buyer, and credited to the Pylon's beacon. | [Nostr 82](../../../nostr/book/ch082-pylons-environments-and-deploys.md) | **Open.** Nostr 82 lists paid receipts as not built. |
| 10 | The room applies the choice if the guard is still calm. Bram speaks one authored line. | [Psionic 30](../../../psionic/book/ch030-measure-a-guard-decision.md); Everglade Part VI | **Open.** The ambient behavior that acts on an answer is pending. |
| 11 | The Pylon owner sees the payment in a wallet that uses the same seed as their Nostr key. | [Bitcoin 121](../../../bitcoin/book/ch121-one-seed-for-nostr-and-bitcoin.md) | Chapter written. The product wallet is the Spark SDK. |

## What must be decided before this is written

These are design questions. Each needs an owner decision, not more code.

1. **Who pays.** The realm's maker, OpenAgents, or the player. The first
   version should probably have OpenAgents pay from a capped budget, since
   the decision is ambient and the player did not ask for it.
2. **The price.** A fixed price per decision is the simplest. Psionic 180
   (observe, meter and price) is the place to derive it.
3. **Payment size.** One decision is worth less than a routable Lightning
   payment. The route needs a rule for batching: for example, pay when a
   Pylon's verified balance passes a threshold.
4. **Which wallet sends.** The books build Lightning from scratch. The
   product wallet is the Spark SDK. Say which one this route uses, and what
   the from-scratch code checks.
5. **What a failed check costs.** Psionic 238 withholds payment. Decide
   whether a Pylon that fails repeatedly also loses admission.

## What "done" means

The capstone follows the same four rules as a chapter:

1. **A trace worked by hand.** One decision, with the request bytes, the
   answer, the check, the ledger entry and the payment amount written out.
2. **A runnable route.** One command from a clean checkout that runs the
   trace against a local room, a local Pylon and a regtest or test wallet,
   with an exact transcript. It is added to the
   [route catalog](../routes.json).
3. **Broken cases.** A late answer changes nothing in the game. A wrong
   answer is not paid. A repeated receipt is paid once.
4. **An independent check.** The payment is confirmed by a second wallet,
   and the receipt is verified by a reader that did not write it.

Live payments on mainnet are a production step and need the owner's
go-ahead. The route above stops at a test wallet.

## Chapters this would add

| Book | Chapter | Content |
| --- | --- | --- |
| Psionic | A new chapter after 238, or an extension of it | A payment intent becomes an invoice request and a settled payment, exactly once. |
| Nostr | Extend 82 | Paid receipts. |
| Bitcoin | Part XIX | Paying for checked work: the buyer's and the Pylon owner's side. |
| Everglade | Part VI, chapter 20 or a new chapter | The guard acts on an answer, and what the player sees when there is none. |
