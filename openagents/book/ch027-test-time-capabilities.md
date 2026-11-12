# 27. Test-time capabilities

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Parts III and IV fill the agent with members. This chapter asks whether each
one helps, and gives the vocabulary for the answer: a candidate, a claim,
adoption and credit.

## Draft

Test-time compute means spending more computation when a model answers: longer
reasoning, more samples, a search. It helps, and the published work says so.

Test-time capabilities go one step further. An agent can gain or lose
abilities while it runs, with no change to its weights, when a tool, a plugin,
a skill, a knowledge entry or another agent is admitted into the run. Episode
289 names this as the second of two ideas that make a general agent possible.
Decision models are the first.

"Lose" is the important word. A member can make an agent worse as easily as
better. Skills that an agent wrote for itself have been measured doing worse
than no skills at all. So a plugin that is installed, described or shown in a
demo proves nothing.

The essay sets out four statements, and this part builds each one:

- A component is a candidate capability.
- Evidence makes a claim. A claim says what admitting one identified component
  to one baseline agent changed, on one kind of task, under one grant, by one
  measurement.
- Adoption is policy. Turning evidence into "everyone gets this" is a separate
  decision with written rules.
- Credit goes to verification. The people who check a claim earn credit, and
  so do authors whose work is adopted.

Most of the mechanisms are prior art. What the essay proposes is the chain:
candidate, admission, a controlled comparison, reproduction, outside
validation, adoption, credit, and checking again when anything changes. The
essay is the specification, and the next nine chapters make it run.

## What we build

A plan for what this chapter ships once the draft is approved:

- The record types for a candidate, a claim, an evidence record and a policy.
- A glossary page for this part's terms, linked from every chapter.
- No runner yet. This chapter fixes the words so the later ones can be checked
  against them.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`docs/books/glossary.md`](../../docs/books/glossary.md)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A claim and its evidence | `openagents.eval.v1` `Publication` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)); evidence types are in `openagents.common.v1` ([spec 9.4](../../docs/protocol/openagents-protocol-spec.md#94-evidence-observation-and-context-manifests)) | Nostr kind 3189, public (NIP-EVAL) | Spec sketch; no `.proto` file. `coax/lib/nostr/eval` handles the NIP's JSON |
| Policy | `openagents.policy.v1` ([spec 19.2](../../docs/protocol/openagents-protocol-spec.md#192-openagentspolicyv1)) | Nostr (NIP-POL) | Spec sketch; no `.proto` file |

Planned paths: `openagents/eval/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, test-time capabilities
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): reusable components need evidence
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): measured components
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): benchmark discipline
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`, `gym`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-KB](../../nostr/nips/openagents/NIP-KB.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), Part I: the concept and the lifecycle figure
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P15
