# 40. Installing grants nothing

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The sandbox can hold a plugin. This chapter separates the decisions around it:
finding, installing, enabling, granting and admitting are five steps, and each
is a record.

## Draft

In most plugin systems, installing a plugin gives it the run of the place.
Here it gives it nothing.

There are five decisions. You discover a plugin: you learn it exists. You
install it: its exact bytes are fetched and locked by their hash. You enable
it: it may register its operations. You grant it access to something. And the
host admits it to a run. Each decision is recorded, and each can be undone
without touching the others.

The lock matters most. A result is only meaningful about the exact bytes it
measured. So a run records the lock of everything admitted to it, and a run
that has started keeps its lock even if the defaults change underneath.

There are two admissions, and keeping them apart removes a circle. You want to
admit only safe plugins, but you can't know a plugin is safe until it has run
somewhere. So the first admission is to an evaluation sandbox with just enough
authority to measure the plugin. The second is the decision, made on that
evidence, to let it run with the authority real use needs.

Presence is its own fact. A plugin can be described, bound to a host and
granted, and still be absent because the service behind it is down. The agent
probes and reports what it found. It does not assume.

[Episode 250](../../docs/transcripts/250.md) taught this the hard way: an
agent will claim a capability it doesn't have.

## What we build

A plan for what this chapter ships once the draft is approved:

- A lock record: the hash of every admitted component, written into the run
  journal.
- Separate records for install, enable, grant and admit, with the plugin's
  state visible on every surface.
- The two admissions: an evaluation sandbox and an operating grant.

It builds on what exists today:

- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/cap`](../../coax/lib/nostr/cap)
- [`coax/lib/nostr/run`](../../coax/lib/nostr/run)
- [`coder/tui/gate.coax`](../../coder/tui/gate.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Lock | `openagents.common.v1` locks ([spec 9.2](../../docs/protocol/openagents-protocol-spec.md#92-locks)) | In the run journal (chapter 18) | Spec sketch; no `.proto` file |
| Install, grant and admit | `openagents.extension.v1` `Release` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)), `openagents.capability.v1` `Grant` ([spec 19.3](../../docs/protocol/openagents-protocol-spec.md#193-openagentscapabilityv1)), `openagents.run.v1` `Admitted` ([spec 22.1](../../docs/protocol/openagents-protocol-spec.md#221-openagentsrunv1)) | Nostr: the release is public; grants and run records are private | Spec sketch; no `.proto` file |
| Presence | `openagents.capability.v1` `Presence` ([spec 19.3](../../docs/protocol/openagents-protocol-spec.md#193-openagentscapabilityv1)) | Nostr: a private record in a kind 3188 envelope | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** the lock record in `openagents/plugins/lock.coax` and the
  separate install, enable, grant and admit records.
- **Trace:** install a plugin from a fixture release, then print its state
  after each step: `installed`, `enabled`, `granted`, `admitted`. A run
  journal starts and records the lock: the hash of each admitted component.
  Change the defaults mid-run; the running journal keeps its first lock.
- **Refusal:** fetched bytes whose hash differs from the release are refused
  and nothing is installed. A plugin that is granted but whose service the
  fake reports as down is shown as absent, and the agent does not claim it.
- **Independent check:** `shasum -a 256` over the fixture's component files
  gives the hashes in the lock.

Planned paths: `openagents/plugins/lock.coax`.

## Sources

- **Episodes:**
  - [Episode 250, *Ready the Fleet*](../../docs/transcripts/250.md): claimed capabilities that did not exist
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): progressive tool loading
  - [Episode 54, *Uploading a Plugin*](../../docs/transcripts/054.md)
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `capability`, `coder-access`, `discovery`
- **NIPs:** [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Capability admission"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P15
