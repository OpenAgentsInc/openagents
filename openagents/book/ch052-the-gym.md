# 52. The Gym

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapters 43 to 51 build the method as records and tools. This chapter gives it
a place: the Gym, where people bring candidates, run them, check each other
and see the results.

## Draft

[Episode 289](../../docs/transcripts/289.md) opens with a joke: "I never see
your agents at the Gym. Do your agents even lift?" The Gym is where people
help agents get better.

It has four parts. A hosted runner does two-arm runs and reruns for people
without the hardware. A board lists published results, with their checks and
validations beside them. A card for each trainer shows the work they have done
and the credit it earned. And news says what was adopted, disputed or pulled
this week. "What's new in the Gym?" is one of Episode 289's routes.

The chat is the front door. You ask the agent what to test, or you build a
plugin by interview, and the agent offers to run it, publish the result and
add it to the Gym. Each offer is a card you can accept or ignore.

This chapter ends the part with a run. It takes the repository map plugin as
ported in chapter 41, its original test set and the later independent one, and
repeats the whole path in Coax: a two-arm run, checks under other keys, a
validation, a marginal run and an adoption record. The verdict should match
the one from 2026-09-29. If it doesn't, the difference is the first finding of
the new system, and we write it up.

What the old Gym did not show is in the essay: independent trainers at scale.
One operator turned the wheel by hand. The Gym is built so the next turn can
be someone else's.

## What we build

A plan for what this chapter ships once the draft is approved:

- A hosted runner for runs and reruns, as a Coax binary on our build hosts.
- A results board, trainer cards and Gym news, on the web and in the OpenAgents Metaverse.
- The capstone: the repository map's adoption repeated end to end in Coax.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/xp_ledger.coax`](../../coax/lib/nostr/xp_ledger.coax)
- [`coax/lib/nostr/nipmv.coax`](../../coax/lib/nostr/nipmv.coax)
- [`coax/remote`](../../coax/remote)
- [`apps/openagents-web`](../../apps/openagents-web)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Results board | `openagents.gym.v1` `ResultsPublication`, `Leaderboard` ([spec 21.2](../../docs/protocol/openagents-protocol-spec.md#212-openagentsgymv1)) | Nostr kind 3195, public | Spec sketch; no `.proto` file |
| A run on the hosted runner | `openagents.gym.v1` `GymRequest`, `GymReply` ([spec 21.2](../../docs/protocol/openagents-protocol-spec.md#212-openagentsgymv1)) | Nostr (NIP-EVAL's Gym profile) | Spec sketch; no `.proto` file |

Planned paths: `openagents/eval/gym/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:00 and 1:00, the Gym
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): leaderboards and classes
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): a benchmark viewer
- **Old code at `coder-v1.0.0`, crates:** `gym`, `gym-bridge`, `gym-leaderboard`, `verse-gym`, `coderbench`, `eval-runner`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-XP](../../nostr/nips/openagents/NIP-XP.md), [NIP-MV](../../nostr/nips/openagents/NIP-MV.md), [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Our collective: Coder, the Gym, and the Metaverse"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P6, P15

## Open questions for the owner

1. Who pays for a run on the hosted runner: the trainer in sats, us up to a
   free amount, or both?
