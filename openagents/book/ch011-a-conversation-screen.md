# 11. A conversation screen

> **Status: implemented — the first milestone on the web and the iPhone
> simulator, with recorded fakes.** Login, one message, a fixture reply and
> Stop run from one shared module, and the same five cases give byte-equal
> traces on the build host, in headless Chrome and in the iPhone simulator
> ([#13516](https://github.com/OpenAgentsInc/openagents/issues/13516)). A
> physical iPhone, Android and the live chat page are not covered yet: see
> "What remains".

## Premise

The shell has a place for chats and nothing in it. This chapter builds the
conversation screen, a composer under a transcript, and has it answer from
fixtures. It is the first milestone: on the website and the iPhone, the same
actions open login, log you in, send a message, receive a reply and cancel a
pending one.

## The screen

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

## What we built

- The shared app,
  [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax):
  routes (home, login, chats, chat), the login of
  [`app-login-flow.coax`](../../apps/shared/app-login-flow.coax), and the
  conversation screen's state and actions (draft, send, stream, stop, copy)
  over the session record in
  [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax).
  It also holds the fake services. Each request is held until the test clock
  releases it, and it is answered from a recording: the code request, the
  code check (code `123456`), the account, and a reply in three parts. The
  module answers the shared `coax drive` operations (`tree`, `tap`, `type`,
  `state`, `clock`, `logs`) itself, so every host gives the same answers.
- The website's page,
  [`apps/openagents-web/milestone.coax`](../../apps/openagents-web/milestone.coax),
  served at `/milestone`. It draws the shared view with the kit's components
  on the mono skin, through the browser
  ([`coax/web/dom-host.js`](../../coax/web/dom-host.js)).
- The iPhone's screen,
  [`SharedAppViewController.swift`](../../apps/openagents-ios/OpenAgents/Shared/SharedAppViewController.swift),
  which draws the same view with UIKit controls. Launched with `-OADrive
  SOCKET`, the app answers `coax drive` on that socket through the native
  hosts' driver,
  [`apps/shared/host/drive.c`](../../apps/shared/host/drive.c), using
  [`apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c).
- The cases, in
  [`openagents/examples/cases/`](../examples/cases): `login`,
  `login-cancel`, `first-milestone`, `logout-pending` and `reconnect`, each
  an `.ops` file and its `.trace`. The runner,
  [`apps/shared/cases.coax`](../../apps/shared/cases.coax), plays them on
  one host and compares the output with the trace byte for byte.

On both hosts, `tap` and `type` go through the platform's own input. In
Chrome they are a real click and real typing. On the iPhone they are the
button's own action and the text field's own text input. The driver then
asks the app what reached it (`input`). Both hosts refuse a driver tap on a
shown control that no real press delivered. A matching trace therefore
shows that the browser's and UIKit's controls reached the shared app.

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
| A message and its reply | `openagents.chat.v1` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | HTTP stream | Spec sketch; no `.proto` file. The fixture answers in the app, with no carriage |

### The chapter's check

Run it from the repository's root:

```shell
coax/bin/coax test apps/shared/openagents-app.coax
coax/web/tool.sh apps/shared/cases.coax cpu
coax/web/tool.sh apps/shared/cases.coax web
coax/web/tool.sh apps/shared/cases.coax ios-simulator DEVICE
```

`web` serves `apps/openagents-web` as built: build the page with
`coax/web/wasm.sh apps/openagents-web/milestone.coax
apps/openagents-web/milestone-face.c apps/openagents-web/build/milestone.wasm`.
`ios-simulator` takes a booted simulator's name or UDID that has the app
from `apps/openagents-ios/build.sh` installed.

- **Source:** the shared app,
  [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax),
  and
  [`openagents/examples/cases/first-milestone.ops`](../examples/cases/first-milestone.ops).
  The case logs in, sends `hello`, releases the reply's three parts,
  copies the reply, then sends again and stops.
- **Trace:**
  [`first-milestone.trace`](../examples/cases/first-milestone.trace), 33
  operations. The build host, headless Chrome and the iPhone simulator
  print the same bytes after the host's name line. That line is `drive:
  ... ready` on the web and on the iPhone, and the cpu host has none.
  `state transcript` after the reply prints `you: hello` and `reply:
  Hello! This is a fixture reply.`
- **Refusal:** in the same case, `tap composer.stop` with two parts and the
  end still held. Each later `clock data release` answers `late part
  dropped`, the transcript ends at the stop with `notice: Stopped`, and
  `logs` lists the three drops. Before that, `tap composer.send` with an
  empty draft answers `composer.send can't be pressed now`.
  [`logout-pending.ops`](../examples/cases/logout-pending.ops) logs out
  with a turn pending: the late part is dropped the same way, and the
  transcript is empty after logout.
- **Independent check:** the session record's own checks
  (`coax/bin/coax test coax/lib/agent/session/session.coax`, 6 checks), which
  were written before this chapter, and the expected values written by hand
  in the shared app's 149 checks. The
  [`reconnect`](../examples/cases/reconnect.ops) case loses the send's
  answer, sends again with the same operation ID (`op-1`), and the service
  stores one message.

Recorded on 2026-11-13: all five cases passed on cpu, on web (headless
Chrome on macOS) and on ios-simulator (iPhone 17, iOS 26.5).

## What remains

- **A physical iPhone and Android.** The cases have run only in the
  simulator. A device run also checks the keyboard's input methods,
  VoiceOver and backgrounding. Those are device targets (CONTRACT.md) and
  stay unnamed until they run
  ([#13517](https://github.com/OpenAgentsInc/openagents/issues/13517)). The Android app has no host for the shared
  app yet.
- **The live chat page.** `/chat`
  ([`chat-web.coax`](../../apps/openagents-web/chat-web.coax)) still keeps
  its own state. Moving it onto the shared app's conversation state is
  chapter 12's work, when a real loop replaces the fixture.
- **Markdown.** The fixture reply is plain text. The shared view gives the
  reply a role, and each host draws it as text.

Planned paths: none new. The screen module is
[`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax),
and its cases are in [`openagents/examples/cases`](../examples/cases).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, chat with the same thing everywhere
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across devices
- **Also:**
  - [The chat zero-base audit](../../docs/audits/2026-11-11-chat-zero-base-audit.md)
  - [The apps roadmap](../../docs/apps/2026-11-10-apps-roadmap.md), step 2
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P6
