# 32. External validation

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Reproduction shows the result is real on the author's tests. This chapter asks
the harder question: does it hold on tests the author never wrote or saw?

## Draft

A plugin and its test set often come from the same person on the same
afternoon. The tests cover what the author thought of. A reproduction confirms
the result on those tests, and says nothing about the rest of the world.

Validation is a second test set, written by someone else, released after the
subject, and aimed at the same kind of task. The same subject at the same lock
runs on it. If the effect holds, the claim is externally validated. A related
result, called transfer, runs the subject on a different kind of task to see
how far the effect reaches.

Independence is read from the records and is not taken on trust. The second
test set has a different signer, and it was created after the subject's
release. A reader checks both from the signed events.

The first full case is in the record. The repository map passed 5 of 6 cases
with the plugin against 2 of 6 without. Three signers reproduced that. A
second test set, released later under another key, validated it at 4 of 6
against 2 of 6. The validated effect was smaller than the original. That is
the finding this method exists to surface: the author's tests flattered the
plugin a little, and the outside tests showed by how much.

The essay is plain about the limit of that case. Every hand on the wheel was
one operator's. A real validation needs a person who isn't us.

## What we build

A plan for what this chapter ships once the draft is approved:

- A `validates` result: the same subject and lock on a second, independent
  test set.
- An independence check that reads signers and creation times from the events.
- A `transfer` result for a different kind of task, reported separately.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Validation and transfer results | `openagents.eval.v1` `Publication` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | Nostr kind 3189, public | Spec sketch; no `.proto` file |

Planned paths: `openagents/eval/validate.coax`.

## Sources

- **Episodes:**
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): evaluating on tasks that did not teach the component
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:00, agents at the Gym
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`, `gym`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Externally validated capability claim"
  - The first adoption record, `docs/extensions/measurements/2026-09-29-first-adoption.md` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P15

## Open questions for the owner

1. How do we get the first validation written by someone outside the company?
