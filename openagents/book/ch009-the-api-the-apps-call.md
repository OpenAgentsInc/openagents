# 9. The API the apps call

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 8 logged you in. This chapter names the small set of requests the
web and iPhone apps make after that, and the two rules every one of them
follows: one schema per record, and the decision in Coax. The whole protocol
comes later, in chapters 62 to 67.

## Draft

The apps need few requests at first: who you are, your settings, your
conversations, a message sent, a reply streamed back and a cancel. Each is a
route of the HTTP API in [`coax/web/api/`](../../coax/web/api), the same
service that already runs the login.

Two rules hold for every one of them. They are the minimum of the protocol,
and the apps can't wait for the rest.

- **One schema.** A record that crosses the wire or is stored has one
  definition in [`proto/`](../../proto/README.md), a row in
  [`proto/registry.json`](../../proto/registry.json) and one codec file.
  `coax/web/tool.sh wire-check` fails a shape with neither
  ([how to register one](../../proto/README.md#register-a-wire-shape)). The
  web and the iPhone decode the same bytes with the same generated codec.
- **The decision is Coax.** A route's rule is an `#authority` file,
  `*-policy.coax`, checked with `coax mutate` and zero survivors. The C route
  is a thin adapter that moves facts in and the answer out
  ([COAX.md](../../coax/web/api/COAX.md)).

A request carries its operation ID. Sending the same message twice with one
ID stores it once and answers twice with the same result, so a retry after a
dropped connection is safe. That matters more on a phone than anywhere else.

The API is versioned. An installed iPhone app can be weeks older than the
website, because store review and updates take time. A route keeps answering
the versions that are still installed, and a change to a record is a new
version, never an edit in place (chapter 76).

## What we build

A plan for what this chapter ships once the draft is approved:

- The apps' first routes: account, settings, conversation list, send,
  stream and cancel, each with its record registered in `proto/`.
- One `*-policy.coax` file per new rule, with its mutation run.
- A version rule: the oldest app version each route still answers.

It builds on what exists today:

- [`coax/web/api/COAX.md`](../../coax/web/api/COAX.md)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`proto/README.md`](../../proto/README.md)
- [`proto/registry.json`](../../proto/registry.json)
- [`coax/web/wire-check.coax`](../../coax/web/wire-check.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | HTTP | Spec sketch; no `.proto` file. Today's `/chat` routes write their own JSON ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| Grant and agent key | `openagents.account.v1alpha1` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, binary protobuf | In use ([chapter 8](ch008-accounts-and-login.md)) |

### The chapter's check

- **Source:** the routes' policy files in `coax/web/api/` and their
  registered records.
- **Trace:** each route's request and answer for a fixture account, decoded
  by the generated codec and printed field by field.
- **Refusal:** a send with an operation ID that was already used returns the
  first result and stores nothing new.
- **Independent check:** `wire-check`, and the API suite on a build host.

Planned paths: none. The routes go in [`coax/web/api/`](../../coax/web/api)
and their schemas in [`proto/`](../../proto).

## Sources

- **Also:**
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 8 and 10
  - [The compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md)
  - [The protocol review](../../docs/protocol/2026-11-11-protocol-review.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12, P13
