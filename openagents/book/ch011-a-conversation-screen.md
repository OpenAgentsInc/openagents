# 11. A conversation screen

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The shell has a place for chats and nothing in it. This chapter builds the
conversation screen, a composer under a transcript, and has it answer from
fixtures. It is the first milestone: on the website and the iPhone, the same
actions open login, log you in, send a message, receive a reply and cancel a
pending one.

## Draft

The screen is a transcript with a composer at its foot: a real text field
and Send. You send a message, it appears in the transcript, and the reply
streams in under it as Markdown. While a reply comes, Send becomes Stop.
Under a finished reply, Copy copies it. The web already has this page,
[`chat-web.coax`](../../apps/openagents-web/chat-web.coax), with live models
behind it. This chapter makes its state and actions shared, so the iPhone
gets the same screen.

The transcript is the session record in
[`coax/lib/agent/session/`](../../coax/lib/agent/session): one conversation
as its parts, the same for every engine and every surface. A part is your
message, the reply's text, thinking, or a tool call with its result. The
screen only reads the record and draws what it finds. The text, the edit
buffer and the Markdown are the shared ones from `components/core/`.

Here the reply comes from a fixture, not a model. A fake conversation service
answers each message with recorded parts, released by the test clock one at
a time. That makes every run the same, and it lets this chapter finish before
any agent exists. Chapter 12 replaces the fixture with the first real loop,
and the case still passes.

Stop is a cancel, with chapter 5's rule. Pressing Stop ends the pending turn
at once. Parts that arrive after it are dropped, because the turn they belong
to is closed. Logging out does the same for every open turn. Reconnecting
never sends a message twice: the send carries an operation ID, and the
service answers a repeated ID with the first result (chapter 9).

This screen runs no agent. It sends a message to whatever executes the turn
and draws the record that comes back. Chapter 37 gives every turn one
execution owner, so a second screen open on the same conversation shows the
reply and never runs it again.

## What we build

A plan for what this chapter ships once the draft is approved:

- The conversation screen's state and actions in the shared app: draft,
  send, stream, stop and copy.
- A fake conversation service with recorded replies.
- The first milestone's cases, on the web and the iPhone: log in, send,
  receive the fixture reply, and cancel a pending turn.

It builds on what exists today:

- [`apps/openagents-web/chat-web.coax`](../../apps/openagents-web/chat-web.coax)
- [`apps/openagents-web/chat`](../../apps/openagents-web/chat)
- [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax)
- [`coax/lib/agent/session/stream.coax`](../../coax/lib/agent/session/stream.coax)
- [`components/core/markdown.coax`](../../components/core/markdown.coax)
- [`components/core/edit.coax`](../../components/core/edit.coax)
- [`coax/lib/gateway/sse.coax`](../../coax/lib/gateway/sse.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The session record a screen draws | Coder's transcript log, id `openagents.session.v1` on disk; planned package `openagents.session.log.v1alpha1` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C1) | Not sent yet | Planned. Written by hand in `coax/lib/agent/session/log.coax` |
| A message and its reply | `openagents.chat.v1` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | HTTP stream | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** the conversation module in `apps/shared/` and
  `openagents/examples/cases/first-milestone.ops`.
- **Trace:** `tap home.login`, the login operations of chapter 8, `type
  composer.input "hello"`, `tap composer.send`, `clock data release` for each
  fixture part, then `state transcript`; the web and iPhone traces match.
- **Refusal:** send, `tap composer.stop`, then release the held parts; the
  transcript ends at the stop, and the trace prints `late part dropped`. A
  second case logs out with a turn pending and gets the same result.
- **Independent check:** the session record's own checks in
  `coax/lib/agent/session/`, and a reconnect case that resends with the same
  operation ID and gets one stored message.

Planned paths: `openagents/examples/cases/first-milestone.ops`; the screen
module goes in [`apps/shared`](../../apps/shared).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, chat with the same thing everywhere
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across devices
- **Also:**
  - [The chat zero-base audit](../../docs/audits/2026-11-11-chat-zero-base-audit.md)
  - [The apps roadmap](../../docs/apps/2026-11-10-apps-roadmap.md), step 2
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P6
