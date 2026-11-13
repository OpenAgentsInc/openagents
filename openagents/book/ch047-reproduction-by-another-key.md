# 47. Reproduction by another key

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

One person's result is a report. This chapter has a second person, with a
different signing key, run the same test set on the same subject and publish
whether they got the same verdict.

## Draft

An author who measures their own plugin has every reason to see a good result.
The cheapest defense is someone else running it again.

A check is a rerun to protocol: the same test set, the same subject at the
same lock, the same gate, and a different signer. If the verdict matches, the
check confirms. If it doesn't, the check disputes. Both are published, and
both stay visible.

A dispute is as valuable as a confirmation. It is the method working. So a
check earns credit whichever way it comes out, and a disputed result counts
toward nothing until it is resolved.

Anyone can check. You don't need the author's permission, and you don't need
ours. You need the published test set, the published subject and a computer. A
hosted runner can do the rerun for people who lack the hardware.

How many confirmations make a claim a candidate for adoption is policy, not
protocol. The old tree's policy asked for three distinct signers. Chapter 49
writes the policy down, and chapter 60 takes up what "distinct" has to mean
when keys are free.

The archive's hardest lesson is behind this chapter. A success announcement
was recorded, then cut from the episode, because analysis showed the result
was in-sample and did not reproduce. A worker's own green checks are not
enough.

## What we build

A plan for what this chapter ships once the draft is approved:

- A `check` action: rerun a published result under your key and publish a
  confirmation or a dispute.
- A reader that decides, from two signed events alone, whether one confirms
  the other.
- Hosted reruns for people without the hardware.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/event_sign.coax`](../../coax/lib/nostr/event_sign.coax)
- [`coax/lib/nostr/keys.coax`](../../coax/lib/nostr/keys.coax)
- [`coax/remote`](../../coax/remote)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Confirmation or dispute | `openagents.eval.v1` `Publication` that cites the first ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | Nostr kind 3189, signed by the checker | Spec sketch; no `.proto` file. A reader verifies a signature over the bytes it received, never over bytes it wrote again ([spec 35](../../docs/protocol/openagents-protocol-spec.md#35-conformance-and-verification)) |

### The chapter's check

- **Source:** the `check` action in `openagents/eval/check.coax` and the
  reader that decides from two signed events whether one confirms the other.
- **Trace:** a fixture report signed by key A; key B reruns the same test
  set, subject lock and gate on recorded answers, and publishes to a fake
  relay. The reader prints `confirms`. A second rerun with a different
  verdict prints `disputes`, and both stay listed.
- **Refusal:** a check signed by the same key as the report is refused as a
  reproduction. A check whose subject lock differs from the report's is
  refused, with the lock named.
- **Independent check:** the fixture events use keys and signatures checked
  against the BIP-340 test vectors, and NIP-01 event ids from another Nostr
  implementation match the ids the reader computes.

Planned paths: `openagents/eval/check.coax`.

## Sources

- **Episodes:**
  - [Episode 265, *Vulnerability Scanner*](../../docs/transcripts/265.md): reproducibility for security findings
  - [Episode 252, *Observer, Automated QA*](../../docs/transcripts/252.md): independent verification
  - [Episode 259, *Verifiable Software*](../../docs/transcripts/259.md): independent review and receipts
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): results that must keep their limits
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`, `gym-bridge`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-XP](../../nostr/nips/openagents/NIP-XP.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Reproduced capability claim"
  - The transcript index's note on the removed success clip: [docs/transcripts/README.md](../../docs/transcripts/README.md#the-latest-measurement-arc-revises-its-own-success-story)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P15
