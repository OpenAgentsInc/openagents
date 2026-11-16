# 4. A reply from the service

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 3 draws your messages, and nothing answers. This chapter sends a
message to our service and draws the reply as it streams in. It adds
effects: typed requests the app asks the host to run, answers that come
back as actions, Stop, a fake service for tests and a clock the tests
control.

## Draft

An effect is a typed request with an operation ID, a declared capability
and a bounded payload
([Coax Native, "Services and native modules"](../../coax/native.md#services-and-native-modules)).
`composer.send` now commits two things together: your message in the state
and a `chat.send` request in the effect outbox. The host takes the request
from the outbox after the commit and runs it with its native HTTP service.
The answer comes back later as an ordinary action. A component never calls
the host; only the outbox does.

Taking a request from the outbox means the host accepted or refused it, not
that it succeeded. State says what an effect is doing: pending, done,
failed or unknown. Unknown is real: a send can reach the service and its
answer can be lost. An unknown outcome never retries on its own. The send
carries its operation ID, and the service answers a repeated ID with the
first result, so a deliberate retry stores the message once.

The reply streams. The service answers with parts, and each part arrives as
a `chat.part` action that appends to the reply in the session record and
marks the transcript dirty. While a reply comes, Send becomes Stop.

Stop is a cancel, and cancellation has one rule on every host. Pressing
Stop ends the pending turn at once. A part that arrives after it is
dropped, because the turn it belongs to is closed, and the transcript ends
with `Stopped`. A late answer can never change the screen of a turn you
stopped, a screen you left or a session you logged out of.

The requests the app makes follow two rules, the minimum of the protocol
that chapters 62 to 67 complete:

- **One schema.** A record that crosses the wire or is stored has one
  definition in [`proto/`](../../proto/README.md), a row in
  [`proto/registry.json`](../../proto/registry.json) and one codec file.
  `coax/web/tool.sh wire-check` fails a shape with neither
  ([how to register one](../../proto/README.md#register-a-wire-shape)).
- **The decision is Coax.** A route's rule on the service is an
  `#authority` file, `*-policy.coax`, checked with `coax mutate` and zero
  survivors. The C route moves facts in and the answer out
  ([COAX.md](../../coax/web/api/COAX.md)).

The API is versioned. An installed iPhone app can be weeks older than the
service, because every change ships in a store build. A route keeps
answering the app versions still installed, and a change to a record is a
new version, never an edit in place.

Tests never reach the real service. A fake service answers with recorded
parts, in a declared order, each held until the test releases it, so a run
is the same every time. The clock is controlled too: `coax drive clock
+MS` advances the app's time and `clock data release NAME` lets one held
answer through. The pinned test environment is seed 13096,
2026-01-01T00:00:00Z, en-US and UTC, the values the engine's agent pin uses
([`metaverse-engine/agent/pin.coax`](../../metaverse-engine/agent/pin.coax)).
The clock never fast-forwards a real network timeout into a success. A
check against the live service is a separate record that names its real
nondeterminism.

The reply in this chapter comes from whatever answers the chat route today.
Chapter 12 puts the first loop of our own behind it, and this chapter's case
still passes.

## What we build

A plan for what this chapter ships once the draft is approved:

- `chat.send` as an effect with pending, done, failed and unknown states, and
  its operation ID.
- The host's HTTP service on iOS, running requests from the outbox and
  delivering streamed parts as actions.
- Stop, with the late-part fence.
- A fake chat service with recorded parts and held answers, and the
  controlled clock.
- The chat record registered in `proto/`, with its codec, before the app
  sends it.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/EFFECTS.md`](../../coax/EFFECTS.md)
- [`coax/lib/agent/session/stream.coax`](../../coax/lib/agent/session/stream.coax)
- [`coax/lib/gateway/sse.coax`](../../coax/lib/gateway/sse.coax)
- [`coax/web/api/COAX.md`](../../coax/web/api/COAX.md)
- [`proto/README.md`](../../proto/README.md)
- [`docs/metaverse/agent-clock.md`](../../docs/metaverse/agent-clock.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | HTTP stream | Spec sketch; no `.proto` file. Today's `/chat` routes write their own JSON ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| The session record the screen draws | Coder's transcript log, id `openagents.session.v1` on disk; planned package `openagents.session.log.v1alpha1` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C1) | Not sent | Planned. Written by hand in `coax/lib/agent/session/log.coax` |

### The chapter's check

- **Source:** the chat effect and the fake service in the lean app, with
  their checks, and `openagents/examples/cases/reply.ops`.
- **Trace:** `type composer.input hello`, `tap composer.send`, `state turn`
  prints `pending`; three `clock data release chat.part` and one `clock data
  release chat.done`; `state transcript` prints `you: hello` and the
  recorded reply.
- **Refusal:** a second send, `tap composer.stop` after one part, then a
  release of the held parts: each answers `late part dropped`, the
  transcript ends with `notice: Stopped`, and `logs` lists the drops. A lost
  answer followed by a retry with the same operation ID stores one message.
- **Independent check:** the session record's and the stream parser's own
  checks, written before this chapter, and the service's suite for the chat
  route on a build host against the same request bodies.

Planned paths: `openagents/examples/cases/reply.ops`. Service routes go in
[`coax/web/api/`](../../coax/web/api) and their schemas in
[`proto/`](../../proto).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, chat with the same thing everywhere
- **Also:**
  - [Coax Native, "Time and lifecycle"](../../coax/native.md#time-and-lifecycle)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 8 and 10
  - [The protocol review](../../docs/protocol/2026-11-11-protocol-review.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P12, P13

## Open questions for the owner

1. Which service answers in this chapter, before chapter 12: the existing
   `/chat` route with its live models, or only the fake service until our
   own loop exists?
