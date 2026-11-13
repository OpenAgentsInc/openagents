# 50. Credit

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Checking other people's claims is work. This chapter pays for it in credit:
experience points that go to the people who verify and to the authors whose
work is adopted.

## Draft

If only authors are rewarded, everyone writes plugins and nobody checks them.
The method then fails, because it depends on checks. So credit goes to
verification.

Two rules carry it. When someone reruns a result to protocol, the checker, the
original evaluator and the test set's author each earn credit, whether the
check confirms or disputes. When a member is adopted, its author, the test
set's author and the evaluators whose results the adoption cites each earn
credit.

Running your own test earns nothing. Publishing earns nothing. A view or a
download earns nothing. Credit follows work that another person relied on.

The credit is experience points, and your name beside them. In the old tree a
referee signed each award, and any reader could recompute the ledger from the
public records. The first awards went out in September 2026: nine for checks,
then two for the first adoption.

Experience points are not money. They were never paid out, and this chapter
keeps that line. Money arrives in the market chapters (53 to 61), where authors are paid for paid
use. The two answer different questions. Points say who did the work that made
a member trustworthy. Sats say whose member people paid to use.

[Episode 284](../../docs/transcripts/284.md) gives the design rule for all of
it: progress should reward verified work, and never token use or failed loops.

## What we build

A plan for what this chapter ships once the draft is approved:

- Award rules for checks and adoptions, and a referee that signs awards from
  the public records.
- A ledger any reader can recompute, with revocation for awards whose evidence
  is withdrawn.
- Your credit shown on every surface, with the work behind each award.

It builds on what exists today:

- [`coax/lib/nostr/xp_ledger.coax`](../../coax/lib/nostr/xp_ledger.coax)
- [`coax/lib/nostr/xp_keys.coax`](../../coax/lib/nostr/xp_keys.coax)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Award and revocation | `openagents.xp.v1` `Award`, `Revocation`, `Quest` ([spec 28.1](../../docs/protocol/openagents-protocol-spec.md#281-openagentsxpv1)) | Nostr kinds 3193, 3194 and 30193, public (NIP-XP) | Spec sketch; no `.proto` file. `coax/lib/nostr/xp_ledger.coax` handles the NIP's JSON |

### The chapter's check

- **Source:** the award rules and referee in `openagents/eval/credit.coax`.
- **Trace:** one fixture check and one fixture adoption replay. The referee
  signs awards to the checker, the original evaluator and the test set's
  author for the check, and to the author, the test set's author and the
  cited evaluators for the adoption. The trace prints the derived ledger.
- **Refusal:** an author running their own test earns nothing, and the trace
  shows no award. A revoked award leaves the ledger.
- **Independent check:** the ledger is recomputed from the same public
  events by [`coax/lib/nostr/xp_ledger.coax`](../../coax/lib/nostr/xp_ledger.coax),
  written before this chapter, and the totals are counted by hand from the
  fixture.

Planned paths: `openagents/eval/credit.coax`.

## Sources

- **Episodes:**
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): reward verified work
  - [Episode 225, *Developer Bounties*](../../docs/transcripts/225.md)
  - [Episode 223, *Pay the People*](../../docs/transcripts/223.md)
  - [Episode 155, *Open Knowledge Graph*](../../docs/transcripts/155.md): contributor incentives for knowledge
- **Old code at `coder-v1.0.0`, crates:** `xp-ledger`, `gym-leaderboard`, `contribution-service`
- **NIPs:** [NIP-XP](../../nostr/nips/openagents/NIP-XP.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Capability credit"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14, P15

## Open questions for the owner

1. Should verification also earn sats, for example a bounty for a check or a
   validation, or does it stay points only?
