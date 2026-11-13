# 1. One app, every surface

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

This is the first chapter, so there is nothing before it. You set up the one
app this book builds: openagents.com and the OpenAgents iPhone app, written
once in Coax on [Coax Native](../../coax/native.md). The chapter says what
"one app" shares and what it doesn't, and it gives the rule every later
chapter keeps.

## Draft

Begin from the [Coax Native starter](../../coax/native/README.md), with
Updates enabled or omitted. Its first iOS profile runs Coax-generated Wasm
inside WebKit using shared DOM components. Native account services are
installed ahead of the login and chat chapters so compatible application
code can arrive over the air. The native-control renderer and this chapter's
complete two-host acceptance case remain planned.

[Episode 289](../../docs/transcripts/289.md) says it plainly: go to
openagents.com, or open the terminal, desktop or mobile app, and you are
talking to the same thing. That is promise P1. It is a rule about code as
much as about product.

We have broken this rule before. The archive holds a web app, a phone app, a
desktop app and a terminal, built at different times in different stacks,
and each reset left the others behind ([Episodes
183](../../docs/transcripts/183.md), [196](../../docs/transcripts/196.md) and
[275](../../docs/transcripts/275.md)). A feature that existed on one surface
was missing on the next.

This book builds the app first and the agent second. Chapters 1 to 6 build a
small app on Coax Native and run one acceptance case on the web and on the
iPhone. Chapters 7 to 11 turn it into OpenAgents: the public website, login,
the logged-in shell and a conversation screen that answers from fixtures.
Only then, in chapter 12, does a model answer.

"One app" means three separate things, and the book never mixes them up:

- **Shared application code.** The state, actions, routes and components are
  Coax, compiled once to C for the iPhone and to Wasm for the browser. A host
  draws them and runs the effects they ask for. No host has its own product
  logic.
- **Shared account and conversation data.** Your account and your
  conversations belong to you, not to a device. Chapter 8 logs you in on both
  apps, and chapter 35 moves a conversation between them.
- **Agent execution.** A task runs in one place at a time: on the device, on
  a paired computer or on our hosted service. That place is the task's
  execution owner ([Coax Native](../../coax/native.md#layers-and-execution-ownership)).
  Every other client presents the task and never repeats its tool calls,
  model requests or payments. Chapter 37 builds the rule.

Four owners divide the work. Coax is the language and compiler. Coax Native
owns app composition and the platform boundary. OpenAgents, this book, owns
the product and the agent. The Metaverse Engine owns worlds. The book uses
the framework; it does not write a second component toolkit or a second
router. Where a piece already exists, such as the components in
[`components/`](../../components), the login flow in
[`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax) or
the session record in
[`coax/lib/agent/session/`](../../coax/lib/agent/session), a chapter uses it.

Every chapter that teaches code ends the same way. It names its Coax source,
the exact trace a run prints, a case the code must refuse, and a check that
does not share the code under test. [CONTRACT.md](CONTRACT.md) says what each
status means and what evidence moves a chapter from one to the next.

## What we build

A plan for what this chapter ships once the draft is approved:

- The book's app skeleton: one Coax app module that the web build and the
  iPhone build both link.
- The scope of version 1 for this book: openagents.com and the iPhone app.
  Android follows on the same acceptance cases (chapter 75). Desktop,
  terminal and Windows are additional platforms with their own evidence.
- A table of targets and what each needs as evidence before a chapter may
  name it ([CONTRACT.md](CONTRACT.md#targets)).

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/web.md`](../../coax/web.md)
- [`components/spec.md`](../../components/spec.md)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`apps/shared`](../../apps/shared)
- [`coax/lib/agent`](../../coax/lib/agent)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply, between a surface and the hosted runtime | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | Nostr kinds 25900 and 26900, encrypted; HTTP chat routes | Spec sketch; no `.proto` file. Today's `/chat` routes write their own JSON ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| One conversation on every device | `openagents.sync.v1alpha1`, a log per conversation ([chapter 35](ch035-sync-across-devices.md)) | Binary frames on one WebSocket | Planned |

Planned paths: none. The app lives in
[`apps/openagents-web`](../../apps/openagents-web) and
[`apps/openagents-ios`](../../apps/openagents-ios); shared app code goes in
[`apps/shared`](../../apps/shared), and generic framework pieces go to Coax
Native.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, one agent behind every surface
  - [Episode 172, *Sync Engine*](../../docs/transcripts/172.md): state that syncs across browsers
  - [Episode 187, *Mobile Sync*](../../docs/transcripts/187.md)
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across terminal, Android and iOS
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): against a terminal-only product
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): one suite across computers, cloud and mobile
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`, `openagents-chat-app`, `openagents-ui`, `coder-sync`, `openagents-connect`
- **NIPs:** [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Also:** [Coax Native](../../coax/native.md), [the apps roadmap](../../docs/apps/2026-11-10-apps-roadmap.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Open questions for the owner

1. The [sync spec](../../docs/sync/README.md) recommends syncing a
   conversation through our service, readable by the server at first
   (chapter 35). Confirm that, and say which collections get end-to-end
   encryption first.
