# 70. Pylons

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Our own hardware is small. This chapter adds everyone else's: a Pylon is a
program you run on your computer that serves models to the network and pays
you in Bitcoin.

## Draft

The first AI product from this team was a network for selling spare compute.
[Episode 290](../../docs/transcripts/290.md) tells the story: you ran a
program on a gaming computer, it served an open model, and you were paid in
Bitcoin. At its height about 300 people ran it at once. It worked, and it
failed for one reason. Nobody was buying. The inference was not the best, the
cheapest, the fastest or the most private.

[Episode 174](../../docs/transcripts/174.md) names that failure and proposes
agents as the buyers. [Episodes 203](../../docs/transcripts/203.md),
[214](../../docs/transcripts/214.md) and [221](../../docs/transcripts/221.md)
rebuild the network under the name Pylon. Episode 290 relaunches it and says
who it is for: people with strong computers at home that sit idle most of the
day, who would be both sellers and buyers. Gamers.

Demand is different this time because the buyer is in this book—and because
the **OpenAgents Executor Network** (`#13912`–`#13928`) turns consumer Macs
and NVIDIA GPUs into a verifiable execution, validation, and decentralized
training mesh rather than an unverified token relay:

- **Three `NIP-PYLON` lanes:** `cj-executor` (runs digest-pinned `exec-vm-v1` /
  `wasm-exec-v1` programs after passing a local self-test), `cj-validate`
  (a second distinct physical device independently replays traces via
  [Psionic ch234](../../psionic/book/ch234-exact-replay-as-a-verification-class.md)
  and publishes signed NIP-32 kind `1985` verdicts), and `cj-training`
  (runs bounded Local-SGD / DiLoCo routing-adapter windows whose updates pass
  a 6-tier Byzantine verifier and the Quarantine Optimizer in
  [Psionic ch272–274](../../psionic/book/ch272-bounded-training-windows-and-diloco.md)).
- **Live Bitcoin mainnet two-leg settlement:** Verified across an Apple M5 Max
  worker and an NVIDIA RTX 4080 Linux validator over a live Coax Nostr relay,
  automatically paying both legs on Bitcoin mainnet via Breez Spark (`200 sats`
  worker + `50 sats` validator, `0` sat fee, zero double-spend on restart),
  gated by `#authority` rolling 24h UTC caps and reconciled into signed kind
  `30201` pool aggregates ([W4.3 mainnet settlement record](../../docs/verification/executor-w43-mainnet-settlement.md),
  [Bitcoin ch123](../../bitcoin/book/ch123-paying-for-requests.md),
  [Psionic ch238](../../psionic/book/ch238-pay-for-verified-work.md)).
- **Ambient decision demand:** The front makes decisions on every turn, coding
  agents invoke exact weight modules, and every guard and shopkeeper in the
  OpenAgents Metaverse makes decisions too (chapter 74).

Episode 290 calls this having ambient compute as a primitive. The Executor
Network capstone ([executor-network-flywheel.md](../../docs/books/capstones/executor-network-flywheel.md))
proves the end-to-end loop on our own hardware; scaling to independent outside
operators is what 1.0 tracks.

## What we build

A plan for what this chapter ships once the draft is approved:

- A Pylon you install with one command (`openagents pylon`), which passes a
  digest self-test and advertises decision, `cj-executor`, `cj-validate`, and
  `cj-training` lanes.
- Routing through the gateway and `NIP-PYLON` relays with distinct-device
  exact replay verification and Byzantine-safe training quarantine.
- Receipt-first two-leg Breez Spark / Lightning payouts under rolling daily
  caps, with a reconciled public kind `30201` settled feed (`paid_msat.bitcoin`).

It builds on what exists today:

- [`psionic/lib/pylon`](../../psionic/lib/pylon)
- [`psionic/lib/mesh`](../../psionic/lib/mesh)
- [`psionic/lib/peer/pay.coax`](../../psionic/lib/peer/pay.coax), [`psionic/lib/peer/settled-feed.coax`](../../psionic/lib/peer/settled-feed.coax), and [`pay/executor-settlement.coax`](../../pay/executor-settlement.coax)
- [`psionic/lib/peer/training-window.coax`](../../psionic/lib/peer/training-window.coax), [`psionic/lib/peer/train-verify.coax`](../../psionic/lib/peer/train-verify.coax), and [`psionic/lib/peer/quarantine-opt.coax`](../../psionic/lib/peer/quarantine-opt.coax)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)
- [`apps/openagents-cli/pylon.coax`](../../apps/openagents-cli/pylon.coax) and [`apps/openagents-cli/pylon-settle.coax`](../../apps/openagents-cli/pylon-settle.coax)
- [`docs/connect-a-pylon.md`](../../docs/connect-a-pylon.md)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A Pylon's announcement | `openagents.pylon.v1` `Beacon` ([spec 27.1](../../docs/protocol/openagents-protocol-spec.md#271-openagentspylonv1)) | Nostr kind 30200, public (NIP-PYLON) | Spec sketch; no `.proto` file |
| Service receipt and pool totals | `openagents.pylon.v1alpha1` `ServiceReceipt`, `PoolAggregate` ([`mesh.proto`](../../proto/openagents/pylon/v1alpha1/mesh.proto)) | Nostr kinds 3201 and 30201 | Schema file: a projection of NIP-PYLON's JSON. The signed JSON stays the authority |

### The chapter's check

- **Source:** a Pylon from the one-command install, a fake relay that
  records events, the gateway, a fake Lightning payer and the controlled
  clock.
- **Trace:** the Pylon's kind 30200 beacon; a decision routed to it and
  answered; a recompute of a sampled result on a second machine that
  matches; a kind 3201 service receipt; and the payout record from chapter
  58.
- **Refusal:** a Pylon that returns a wrong answer on a sampled result gets
  no receipt for it and is not paid. The gateway prints the job ID and `not
  reproduced`, and stops routing to that Pylon.
- **Independent check:** event IDs and signatures are recomputed by
  [`event_reference.cjs`](../../coax/tests/nostr/event_reference.cjs), and
  the receipt's bytes are decoded by Python's protobuf library against
  [`mesh.proto`](../../proto/openagents/pylon/v1alpha1/mesh.proto).

Planned paths: none. Pylons are [`psionic/lib/pylon`](../../psionic/lib/pylon).

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 2:30 to 5:30 and 24:00 to 26:00, Pylons for gamers
  - [Episode 1, *Intro*](../../docs/transcripts/001.md): an open platform tied to a compute network
  - [Episode 174, *GPUtopia 2.0*](../../docs/transcripts/174.md): sellers without buyers
  - [Episode 178, *Swarm Inference*](../../docs/transcripts/178.md): paid inference between two machines
  - [Episode 201, *Fracking Apple Silicon*](../../docs/transcripts/201.md): idle Apple hardware
  - [Episode 203, *Pylon and Nexus*](../../docs/transcripts/203.md)
  - [Episode 214, *Compute Market*](../../docs/transcripts/214.md): a compute market beta
  - [Episode 221, *Pylon Launch*](../../docs/transcripts/221.md): the Pylon launch
  - [Episode 223, *Pay the People*](../../docs/transcripts/223.md): payout bottlenecks
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): construct additional Pylons
- **Old code at `coder-v1.0.0`, crates:** `pylon`, `psionic`, `inference`, `compute-workbench`
- **NIPs:** [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md)
- **Also:**
  - Psionic's chapters on the Pylon service and Nostr From Scratch's on its events, listed in [the books' shared topics](../../docs/books/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11, P12

## Open questions for the owner

1. As public operators join beyond our own machines, should per-job payouts
   stay fixed (starting at `200 sats` worker + `50 sats` validator for exact
   replay) or move to open market bidding under daily caps?
2. What trust/reputation threshold graduates a new public Pylon from Tier-2
   redundant replication to sampled validation?
3. Beyond our own coding agents and the Metaverse, who is the first external
   buyer of verified execution or DiLoCo training capacity?
