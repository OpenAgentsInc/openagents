# 53. Why an open market

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The evidence chapters (43 to 52) can tell a good member from a bad one. This chapter says why the market
for members has to be open, need no permission and pay its authors. It builds
nothing; it sets the test the next eight chapters must pass.

## Draft

The project starts here. [Episode 1](../../docs/transcripts/001.md) is
recorded the day after OpenAI's first DevDay in November 2023, where a store
with revenue sharing was promised. It introduces an open agent platform with
Bitcoin payments to the people who contribute. [Episode
47](../../docs/transcripts/047.md) reviews the store that shipped. Three years
later, [Episode 288](../../docs/transcripts/288.md) asks how much revenue that
platform has shared with its developers, and gives the answer as zero.

Episode 288 also reads the questions developers ask under a new marketplace
announcement. Can we charge? What is the share? Can we link to our own
checkout? Nobody answers them. And it quotes a developer on the review queue:
submitting apps has been horrible. The episode's reply is that you should not
need to ask anybody's permission.

That gives three requirements. The market is open: the records are public and
the code is open source. It needs no permission: anyone with a key can
publish, and no review queue stands between a contribution and its evaluation.
And it pays: authors receive a share of all paid use.

We built a first version early. [Episode 92](../../docs/transcripts/092.md)
launches an agent store with payouts, [Episode
93](../../docs/transcripts/093.md) pays 100,000 sats to four builders, and
[Episode 98](../../docs/transcripts/098.md) shares revenue minute by minute.
Then [Episode 100](../../docs/transcripts/100.md) names the problem: the
payment plumbing was ahead of agents worth paying for.

Two things are different now. The evidence chapters let the market tell which members help.
And one general agent in front of every member creates the demand: a plugin
that is adopted is used by everyone who talks to the agent. [Episode
269](../../docs/transcripts/269.md) calls this the advantage that outlasts any
model or interface.

## What we build

A plan for what this chapter ships once the draft is approved:

- Nothing to run. This chapter states the part's test: a stranger publishes, a
  second stranger pays, and the first is paid, with no step that needs us to
  say yes.
- A one-page comparison of terms: who may publish, who reviews, who is paid
  and how, here and on closed marketplaces.

It builds on what exists today:

- [`nostr/nips/openagents/README.md`](../../nostr/nips/openagents/README.md)

### The chapter's check

- **Source:** this chapter has no code. Its one mechanism is the part's
  test, and it rests on chapters 54 and 56: a release anyone can sign, and a
  paid call.
- **Trace:** chapter 54's check, which publishes a stranger's release to a
  fake relay, then chapter 56's, where a second key pays a priced call on a
  fake Lightning wallet and the first key's author is paid.
- **Refusal:** chapter 54's refusal, a release signed by a key that is not
  the package root.
- **Independent check:** in the combined trace, no step waits on an
  approval from us: the trace has no review step between publish and pay.

Planned paths: none. This chapter states the case for the market; chapters 54
to 61 build it.

## Sources

- **Episodes:**
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): the whole episode
  - [Episode 1, *Intro*](../../docs/transcripts/001.md): an open platform with Bitcoin payments
  - [Episode 47, *Reviewing the GPT "Store"*](../../docs/transcripts/047.md)
  - [Episode 92, *Introducing the Agent Store*](../../docs/transcripts/092.md): the agent store
  - [Episode 93, *The Sats Must Flow*](../../docs/transcripts/093.md): the first payout to builders
  - [Episode 98, *Agent Revenue Sharing*](../../docs/transcripts/098.md): revenue sharing by the minute
  - [Episode 100, *Looking Ahead*](../../docs/transcripts/100.md): plumbing ahead of useful agents
  - [Episode 125, *The Master Plan*](../../docs/transcripts/125.md)
  - [Episode 269, *Last Mover Advantage*](../../docs/transcripts/269.md)
  - [Episode 282, *Not Asking Permission*](../../docs/transcripts/282.md)
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-MKT](../../nostr/nips/openagents/NIP-MKT.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), Part III: extensible three ways
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10, P11, P12
