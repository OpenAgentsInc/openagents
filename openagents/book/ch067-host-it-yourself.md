# 67. Host it yourself

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The API is ours to run. This chapter hands over the last piece: the same
runtime, relay and registry reader as a bundle you run on your own machines,
still joined to the public market.

## Draft

Episode 289 names the self-hosted version of the API alongside ours, and says
of the whole system that it is all open source. The repository carries one
license for everything in it.

Self-hosting matters for three kinds of people. Some can't send their data to
anyone. Some want to change the agent. And some want to be sure the thing they
depend on can't be taken away. [Episodes 267](../../docs/transcripts/267.md)
and [271](../../docs/transcripts/271.md) to
[274](../../docs/transcripts/274.md) are about that last group: services that
survive a host being replaced.

The bundle has the agent runtime, the API in front of it, a relay, a registry
reader and the defaults sync. A runner for evaluations is optional. You bring
model access: your own provider keys through the gateway, a model you serve,
or Pylons.

A self-hosted instance is not cut off. It reads the public registry, so it can
install any release. It can follow our defaults, or publish its own adoptions
from the same public evidence. Its users' paid calls produce the same
receipts. Its trainers' checks count like anyone's.

You can also run less. A relay and a registry reader alone make a mirror. The
runtime with only the answer member makes a support desk.

One thing is not settled by code. A self-hosted instance could call paid
members and not pay. Payment in the request (chapter 56) stops that for
members served by their authors. For members that run locally, it is a
question of license and norms, and it is listed below.

## What we build

A plan for what this chapter ships once the draft is approved:

- A self-host bundle: runtime, API, relay, registry reader and defaults sync,
  with one configuration file.
- A guide for three sizes: a mirror, a support desk and a full instance.
- A check that a self-hosted instance installs a public release and produces
  valid receipts.

It builds on what exists today:

- [`nostr/relay/relay_server.coax`](../../nostr/relay/relay_server.coax)
- [`coax/lib/gateway`](../../coax/lib/gateway)
- [`os`](../../os)
- [`coax/web`](../../coax/web)
- [`LICENSE`](../../LICENSE)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| What an instance offers | `openagents.discovery.v1` `Manifest` ([spec 30.3](../../docs/protocol/openagents-protocol-spec.md#303-openagentsdiscoveryv1)) | HTTP | Spec sketch; no `.proto` file |
| Everything else it sends | The same schemas as ours ([chapter 65](ch065-one-schema-three-carriages.md)) | All three carriages | A self-hosted instance passes the same fixtures ([chapter 66](ch066-the-compliance-rule.md)) |

### The chapter's check

- **Source:** the bundle in `openagents/selfhost/` and its one configuration
  file, started on loopback with a fake model provider and a recorded copy
  of the public registry, at two sizes: the mirror and the full instance.
- **Trace:** the mirror serves the release listing from its own relay. The
  full instance installs one public release by hash, makes one paid call
  against a fake Lightning payer, and prints its receipt.
- **Refusal:** a release whose bytes do not match its listed hash is refused
  at install. A configuration that names no model access stops the instance
  at start, and it prints the missing setting.
- **Independent check:** the receipt passes chapter 57's meter and its
  signature checks under
  [`event_reference.cjs`](../../coax/tests/nostr/event_reference.cjs), and
  the instance's records pass the same fixtures as ours (chapter 66).

Planned paths: `openagents/selfhost/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 3:00, a self-hosted version of the API
  - [Episode 267, *Immortal Infrastructure*](../../docs/transcripts/267.md)
  - [Episode 271, *Sarah on the BEAM*](../../docs/transcripts/271.md): continuity after lost work
  - [Episode 273, *Open Source*](../../docs/transcripts/273.md)
  - [Episode 274, *The First Repo*](../../docs/transcripts/274.md): hosting our own repository
  - [Episode 150, *Neutrality Wins*](../../docs/transcripts/150.md)
- **Old code at `coder-v1.0.0`, crates:** `nostr-relay`, `coder-service`, `coder-cloud`, `tenancy`
- **NIPs:** [NIP-DEPLOY](../../nostr/nips/openagents/NIP-DEPLOY.md), [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md), [NIP-REG](../../nostr/nips/openagents/NIP-REG.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12, P13

## Open questions for the owner

1. How are authors paid when a self-hosted instance runs a paid plugin
   locally: by license terms, by plugins that call home for payment, or not at
   all?
