# 66. The compliance rule

> **Status: planned.** This chapter is an outline: it lists what it will
> say and build. Its draft is not written, and paths marked planned do not
> exist.

## Premise

A schema that no code uses protects nothing. This chapter states the rule that
ties each schema to the bytes the code sends, and the check that enforces it.

## Outline

- **Four conditions.** A shape is compliant when it is registered, has one
  codec, is pinned by a fixture with exact bytes, and has a schema.
- **Three kinds of codec.** Generated binary. Generated JSON, once Coax can
  make it. Hand-written and pinned by a profile.
- **When hand-written is right.** Hot netcode. Bytes that are signed, hashed
  or stored. JSON, until the mappers exist. The Everglade session is the
  model: one shared codec file and a profile that lists every frame.
- **Other people's formats.** A coding agent's stream, MCP, a NIP, LNURL. We
  register each with a fixture and write no schema for it.
- **Golden bytes.** A record needs a fixture before any codec change when a
  signature, a hash or a stored file depends on its bytes.
- **The real risk is two encoders.** Several shapes are written by hand in two
  to six places. The first step is one codec for each, and it needs no new
  tooling.
- **A proto nobody reads is not compliance.** A schema with no consumer and no
  check is marked as such.
- **The check.** One command, `coax/web/tool.sh wire-check`, fails on an
  unregistered shape, a second encoder, or a route file that no row accounts
  for. Its allowlist only shrinks.
- **Where things stand.** The registry has a row for each shape. Eight comply
  today: agent grants, the served decision, the Everglade session and its
  latency record, the Orbit League frames, the Haunted House save, the voice
  frame and the iOS home configuration.
- **The rule for new work.** A new message between apps, the API, rooms or
  devices gets a schema in `proto/` first. A page or a route never invents a
  JSON shape ([docs/reuse.md](../../docs/reuse.md)).
- **Graduating.** A package leaves `v1alpha1` only when a real consumer reads
  and writes it through generated code and the conformance list passes
  ([spec 35](../../docs/protocol/openagents-protocol-spec.md#35-conformance-and-verification)).
- **What this asks of the book.** Every chapter that sends, signs, stores or
  publishes a record names its schema, its carriage and its status.

## What we build

Nothing of its own. This chapter states the rule, and the
[compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md) builds the tools.

Built ([#13236](https://github.com/OpenAgentsInc/openagents/issues/13236)):

- `proto/registry.json`: one row for each shape, with its schema, its one
  codec file and its check.
- `coax/web/tool.sh wire-check`: the check, and `proto/wire-allowlist.txt`,
  the list of shapes still to move.

Planned:

- Generated JSON writers and readers in `#import_proto`
  ([#13237](https://github.com/OpenAgentsInc/openagents/issues/13237)).
- The steps that move Coder's and Everglade's shapes, one at a time
  ([#13250](https://github.com/OpenAgentsInc/openagents/issues/13250)).

It builds on what exists today:

- [`docs/protocol/2026-11-11-compliance-plan.md`](../../docs/protocol/2026-11-11-compliance-plan.md)
- [`docs/protocol/2026-11-11-protocol-review.md`](../../docs/protocol/2026-11-11-protocol-review.md)
- [`proto/registry.json`](../../proto/registry.json)
- [`coax/web/wire-check.coax`](../../coax/web/wire-check.coax)
- [`proto/modules.json`](../../proto/modules.json)
- [`proto/verse/everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json)
- [`everglade/shared/everglade-wire.coax`](../../everglade/shared/everglade-wire.coax)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/proto.md`](../../coax/proto.md)

Planned paths: none. The rule is checked by `wire-check` and
[`proto/registry.json`](../../proto/registry.json).

## Sources

- **Episodes:**
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md): mark each claim as live, gated or withdrawn
- **NIPs:** none
- **Also:**
  - [The compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md) and [the protocol review](../../docs/protocol/2026-11-11-protocol-review.md)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 35 and 36
  - [What to reuse](../../docs/reuse.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12

## Open questions for the owner

1. Does a chapter reach "implemented" only when its records are compliant?
2. Do we publish the fixtures as a kit that a second implementation can run?
