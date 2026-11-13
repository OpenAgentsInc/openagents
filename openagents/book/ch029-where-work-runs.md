# 29. Where work runs

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Members need a computer to run on. This chapter says which one: yours, one we
host, or later a Pylon. It keeps the grant and the keys with the computer's
owner.

## Draft

A message from your phone that says "fix this issue" needs a machine with your
repository on it. The front's decision includes this: does the work need a
computer? If it does, the plan names one.

The first choice is your own computer. Coder runs where the work is, with your
logins and your files, under a grant you control. Your phone reaches it by
pairing. The phone can ask, and the computer's owner decides what is allowed.
Keys stay on the computer.

The second choice is a computer we host, for when yours is off or when you
have none. [Episode 276](../../docs/transcripts/276.md) announces that
service, and [Episode 280](../../docs/transcripts/280.md) shows the operating
system built for it. A hosted computer is a sandbox that we run on our own
cloud account and that keeps to the same record and grant rules.

The third choice comes in chapter 70: another person's machine, paid in
Bitcoin.

The protocol already separates the pieces. One record describes a host, one
says how to reach it, and others carry terminals, control and environments. So
the agent does not care which kind of computer it got. It has a plan, a grant
and a place to run.

[Episode 167](../../docs/transcripts/167.md) names what this is for: an agent
that keeps working overnight.

## What we build

A plan for what this chapter ships once the draft is approved:

- A computer field in the plan, chosen from the computers you have paired or
  rented.
- Pairing: a phone asks, the owner's policy decides, and keys stay on the
  host.
- A hosted computer as a member, with its cost shown before it starts.

It builds on what exists today:

- [`coder/tui/host.coax`](../../coder/tui/host.coax)
- [`coder/tui/daemon.coax`](../../coder/tui/daemon.coax)
- [`coax/lib/nostr/host`](../../coax/lib/nostr/host)
- [`coax/lib/nostr/reach`](../../coax/lib/nostr/reach)
- [`coax/lib/nostr/term`](../../coax/lib/nostr/term)
- [`coax/lib/nostr/ctrl`](../../coax/lib/nostr/ctrl)
- [`coax/remote`](../../coax/remote)
- [`os`](../../os)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A request to a host and its reply | NIP-HOST's JSON; `openagents.host.v1` ([`host.proto`](../../proto/openagents/host/v1/host.proto), [spec 23.3](../../docs/protocol/openagents-protocol-spec.md#233-openagentshostv1)) has a different shape | Nostr kind 3188 envelope, or a REACH frame | `coax/lib/nostr/host` writes the NIP's JSON, which is the definition for `coder host` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md)). The schema file has no product consumer |
| How to reach a host | `openagents.reach.v1` ([`reach.proto`](../../proto/openagents/reach/v1/reach.proto), [spec 23.4](../../docs/protocol/openagents-protocol-spec.md#234-openagentsreachv1)) | Frames on a direct channel | Schema file; no product consumer |
| Terminals and environments | `openagents.terminal.v1`, `openagents.environment.v1` ([spec 23.5](../../docs/protocol/openagents-protocol-spec.md#235-openagentsterminalv1), [spec 23.2](../../docs/protocol/openagents-protocol-spec.md#232-openagentsenvironmentv1)) | Nostr kind 3188 envelope, or a REACH frame | Spec sketch; no `.proto` file |

Planned paths: `openagents/agent/computers.coax`.

## Sources

- **Episodes:**
  - [Episode 167, *Overnight Agent*](../../docs/transcripts/167.md): an overnight agent
  - [Episode 198, *Claude Code on the Web*](../../docs/transcripts/198.md): coding agents on the web
  - [Episode 276, *Coder Cloud*](../../docs/transcripts/276.md)
  - [Episode 280, *CoderOS*](../../docs/transcripts/280.md)
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): trusted devices
- **Old code at `coder-v1.0.0`, crates:** `coder-host`, `coder-reach`, `coder-computers`, `oa-computers`, `coder-cloud`, `boat`, `oa-boat`, `solver-box`, `coder-environment`, `coder-link`
- **NIPs:** [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md), [NIP-REACH](../../nostr/nips/openagents/NIP-REACH.md), [NIP-TERM](../../nostr/nips/openagents/NIP-TERM.md), [NIP-CTRL](../../nostr/nips/openagents/NIP-CTRL.md), [NIP-ENV](../../nostr/nips/openagents/NIP-ENV.md), [NIP-WS](../../nostr/nips/openagents/NIP-WS.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3, P13

## Open questions for the owner

1. Does 1.0 offer hosted computers to everyone, and at what price? Episode 276
   named one dollar an hour at the time.
2. How does hosted compute fit inside the monthly cloud budget as use grows?
