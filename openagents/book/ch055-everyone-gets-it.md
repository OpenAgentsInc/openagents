# 55. Everyone gets it

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A release exists in the registry. This chapter gets it onto every agent:
installs are pinned by hash, and an adopted release reaches every OpenAgents
through a signed defaults update.

## Draft

[Episode 289](../../docs/transcripts/289.md): "If it's good, it gets added to
the registry, and then all OpenAgents will have your plugin." This is promise
P10, and it is two steps.

The first step is an install. You pick a release, and your agent fetches its
components and checks each against the hash in the signed release. What is
installed is exactly what was published. An install is pinned. It does not
move to a newer release unless you move it.

The second step is the defaults. The defaults are a package of their own: a
release that lists the members every agent starts with. When we adopt a member
(chapter 49), we publish a new defaults release that depends on the member's
release and cites the adoption record. Every agent that follows our defaults
picks it up on its next sync. That is how one person's plugin reaches
everyone.

The chain can be audited from either end. Start from a member in your agent
and you can walk back to the defaults release, the adoption record, the
validations and the original report. Start from an author's release and you
can see whether it was adopted and why.

Updates are safe to take. A run in progress keeps its lock. A defaults release
can be rolled back by publishing the previous list again. And a person can
follow different defaults, or none.

In the old tree this path carried one plugin to every Coder. The plan is for
it to carry every kind of member to every surface.

## What we build

A plan for what this chapter ships once the draft is approved:

- Install by hash: fetch, verify against the signed release, and pin.
- A defaults package and a sync that every surface runs.
- A provenance view: from any member back to its release, its adoption and its
  evidence.

It builds on what exists today:

- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/crypto`](../../coax/lib/crypto)
- [`coder/tui/plugins.coax`](../../coder/tui/plugins.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A pinned install | `openagents.common.v1` locks ([spec 9.2](../../docs/protocol/openagents-protocol-spec.md#92-locks)) | A file on the device | Spec sketch; no `.proto` file |
| The defaults update | `openagents.extension.v1` `ComponentSet`, `Registry` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Nostr: a signed release | Spec sketch; no `.proto` file |

Planned paths: `openagents/registry/install.coax`, `openagents/registry/defaults/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, all OpenAgents will have your plugin
  - [Episode 211, *Autopilot Online*](../../docs/transcripts/211.md): automatic upgrades
  - [Episode 256, *Release Candidate*](../../docs/transcripts/256.md): release candidates
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `ext-eval`, `discovery`
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-DEPLOY](../../nostr/nips/openagents/NIP-DEPLOY.md)
- **Also:**
  - `packages/coder-defaults` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10

## Open questions for the owner

1. Does a defaults update install on its own, or does each person confirm it?
