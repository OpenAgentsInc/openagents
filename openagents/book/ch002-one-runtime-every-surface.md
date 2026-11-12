# 2. One runtime, every surface

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 1's loop runs in a terminal. This chapter makes it one library that
the web page, the terminal, the desktop app and the phone all link, so there
is one agent and not four.

## Draft

[Episode 289](../../docs/transcripts/289.md) says it plainly: go to
openagents.com, or open the terminal, desktop or mobile app, and you are
chatting with the same thing. That is promise P1, and it is a rule about code
as much as about product. No surface has its own agent logic.

We have broken this rule before. The archive holds a web app, a phone app, a
desktop app and a terminal, built at different times in different stacks, and
each reset left the others behind ([Episodes
183](../../docs/transcripts/183.md), [196](../../docs/transcripts/196.md) and
[275](../../docs/transcripts/275.md)). A feature that existed on one surface
was missing on the next.

Coax makes one runtime practical because it compiles to C and to Wasm. The
same library links into the browser, the command line, a native desktop app
and a phone app. The runtime takes a small host interface: a way to call a
model, a place to store the log, a clock and a network connection. Each
surface supplies those and draws the result.

There is one conversation, not a chat mode and a work mode. You can start a
request on your phone and read the result in your terminal, because the
conversation belongs to your account and not to the device. Chapter 51 builds
the sync that moves it. A hosted copy of the runtime serves surfaces that
can't run the work themselves.

## What we build

A plan for what this chapter ships once the draft is approved:

- The agent runtime as one Coax library with a host interface. It builds for
  native and for Wasm.
- A hosted service that runs the same library for surfaces that need it.
- A conformance check: the same input log produces the same requests on every
  host.

It builds on what exists today:

- [`coax/lib/gateway`](../../coax/lib/gateway)
- [`coax/lib/nostr`](../../coax/lib/nostr)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-cli`](../../apps/openagents-cli)
- [`apps/openagents-desktop`](../../apps/openagents-desktop)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`coax/web`](../../coax/web)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply, between a surface and the hosted runtime | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | Nostr kinds 25900 and 26900, encrypted; HTTP chat routes | Spec sketch; no `.proto` file. Today's `/chat` routes write their own JSON ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| One conversation on every device | `openagents.sync.v1alpha1`, a log per conversation ([chapter 51](ch051-sync-across-devices.md)) | Binary frames on one WebSocket | Planned |

Planned paths: `openagents/agent/`.

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
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Open questions for the owner

1. Which surfaces run the runtime on the device, and which always call the
   hosted service?
2. The [sync spec](../../docs/sync/README.md) recommends syncing a
   conversation through our service, readable by the server at first
   (chapter 51). Confirm that, and say which collections get end-to-end
   encryption first.
