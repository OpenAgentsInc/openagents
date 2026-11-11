# 30. A claim's key

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

A report says what happened in one run. This chapter says exactly what the
report is about, by giving every claim a key with seven parts, so that two
results can be compared or told apart.

## Draft

Nothing is a capability in general. A claim says that admitting this subject,
to this baseline agent, on this kind of task, changed these outcomes. Leave
any part out and the claim can't be checked.

The essay writes the key as seven parts:

- **A**, the subject, at the strongest identity available: its bytes, a
  version, or an endpoint.
- **B**, the baseline agent, all of it: the model and version, the
  instructions, the router, the defaults, the sampling settings, the runtime
  and the provider.
- **D**, the kind of task the claim is about.
- **S**, the sample of that kind: the test set.
- **E**, the environment.
- **G**, the grant.
- **M**, the measurement.

Evidence records attach to a key. Each is one run's estimate of the effect,
with its uncertainty. A policy reads the records and gives a verdict.

Two results with the same key can agree or disagree. Two results with
different keys are about different things, and a reader can see which part
differs.

One part is hard to pin. A hosted model can change behind an endpoint while
every hash you hold still matches. The claim has to name what it could not
pin, because that silent change is what breaks a reproduction.

The key is also how records stay honest across time. When any part of it
changes, the old evidence is about something that no longer exists, and
chapter 35 handles that.

## What we build

A plan for what this chapter ships once the draft is approved:

- A key type with seven parts and a digest, so equal keys compare equal.
- A report field for everything that could not be pinned.
- A reader that groups published results by key and shows where two keys
  differ.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/crypto`](../../coax/lib/crypto)
- [`coax/lib/nostr/jcs.coax`](../../coax/lib/nostr/jcs.coax)

Planned paths: `openagents/eval/key.coax`.

## Sources

- **Episodes:**
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): keeping configuration and task set with each result
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md): claims that say exactly what is live
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Test-time capability" and "Capability claims as the unit of account"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P15
