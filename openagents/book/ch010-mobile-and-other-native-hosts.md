# 10. Mobile and other native hosts

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. Chapters 1–3 already share `apps/shared/chat/screen.coax`
> between desktop (`apps/openagents-desktop`, primary) and iOS
> (`apps/openagents-ios`); this chapter finishes full host parity across iOS,
> Linux desktop, and Android.

## Premise

Chapters 1 to 9 built the chat page, history, chips and settings grounded
in the primary desktop app (`apps/openagents-desktop`) on shared Coax logic
that iOS already consumes. This chapter completes full native host parity
across iOS, Linux desktop, and Android. It adds no feature. It qualifies
every native host against the same acceptance case, and it states the rule
that keeps them one app.

## Draft

[Episode 289](../../docs/transcripts/289.md) says it plainly: go to
openagents.com, or open the terminal, desktop or mobile app, and you are
talking to the same thing. That is promise P1. It is a rule about code as
much as about product.

We have broken it before. The archive holds a web app, a phone app, a
desktop app and a terminal, built at different times in different stacks,
and each reset left the others behind ([Episodes
183](../../docs/transcripts/183.md), [196](../../docs/transcripts/196.md) and
[275](../../docs/transcripts/275.md)). A feature on one surface was missing
on the next. This book builds one target at a time instead, and each new
target reuses what the first one built.

"One app" means three separate things, and the book never mixes them up:

- **Shared application code.** The state, actions, routes and components of
  chapters 1 to 9 are Coax, compiled once to C for every native host. A host
  draws them and runs the effects they ask for. No host has its own product
  logic.
- **Shared account and conversation data.** Your account and your
  conversations belong to you, not to a device. Chapter 35 moves a
  conversation between devices.
- **Agent execution.** A task runs in one place at a time: on a computer you
  enrolled or on our hosted service. That place is the task's execution
  owner. Every other client presents the task and never repeats its tool
  calls, model requests or payments. Chapter 37 builds the rule.

The native hosts already exist and draw the same display-list kit: AppKit
with Metal on the Mac (the primary desktop host from chapter 1), UIKit with
Metal on iOS, Wayland with Vulkan on Linux, and the Vulkan host on Android
([GPU components, stages G6 and G7](../../components/gpu-components.md#stages)).
Each supplies the same three things: the focused field's editor
(`NSTextInputClient` on the Mac, `UITextField` on iOS, the Wayland
text-input protocol on Linux, `InputConnection` on Android), an
accessibility bridge built from the committed rows (`NSAccessibilityElement`,
`UIAccessibilityElement`, AT-SPI, virtual nodes), and system sheets. A wide
window shows the sidebar beside the chat page; the layout reads the window's
width, not the platform's name.

The desktop app already owns the primary chat workflow and configures the
computer it runs on as an environment (chapter 8): a resident environment
service, runnable without the window, owns the agent's turns, so closing
the app never stops them. The
[desktop and environment plan](../../docs/protocol/environments-claude-and-world-editing.md#31-one-desktop-product-two-independent-lifetimes)
owns that service.

This chapter qualifies the remaining native hosts—iOS, Linux desktop, and
Android—against the same shared modules and chapter 6's acceptance case.
Each host claims nothing until chapter 6's case passes there with a real
input method and native accessibility (VoiceOver, AT-SPI, or TalkBack).

Each host qualifies on its own evidence. A host runs chapter 6's case
through the same `coax drive` operations and must give the same trace. A
host that lacks an operation refuses it. The web is a different
presentation and has its own chapter (chapter 11).

## What we build

A plan for what this chapter ships once the draft is approved:

- Full native host parity for the chat page, history, chips and settings on
  iOS (`apps/openagents-ios`), Linux desktop, and Android, consuming the
  same shared Coax modules as the primary macOS desktop app.
- Chapter 6's case and its device checks on each native host, with the
  targets recorded as they pass.

It builds on what exists today:

- [`apps/openagents-desktop`](../../apps/openagents-desktop/README.md)
- [`apps/shared/host/apple`](../../apps/shared/host/apple)
- [`apps/shared/host/linux`](../../apps/shared/host/linux)
- [`apps/shared/host/android`](../../apps/shared/host/android)
- [`components/gpu-components.md`](../../components/gpu-components.md)
- [`coax/native.md`](../../coax/native.md#layers-and-execution-ownership)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply, between a surface and the hosted runtime | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | Nostr kinds 25900 and 26900, encrypted; HTTP chat routes | Spec sketch; no `.proto` file ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| One conversation on every device | `openagents.sync.v1alpha1`, a log per conversation ([chapter 35](ch035-sync-across-devices.md)) | Binary frames on one WebSocket | Planned |

### The chapter's check

- **Source:** the lean app's shared module, built for each host, and
  chapter 6's `openagents/examples/cases/chat-page.ops`.
- **Trace:** chapter 6's trace, byte for byte after each host's name line,
  on `macos-arm64`, then on `android-emulator`.
- **Refusal:** a host that lacks an operation the case uses refuses it by
  name and is not named as a target.
- **Independent check:** each platform's own UI test reads the chat page
  through its accessibility tree with no `coax drive`.

Planned paths: none. The desktop app is
[`apps/openagents-desktop`](../../apps/openagents-desktop/README.md); hosts
extend [`apps/shared/host`](../../apps/shared/host).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, one agent behind every surface
  - [Episode 172, *Sync Engine*](../../docs/transcripts/172.md): state that syncs across browsers
  - [Episode 187, *Mobile Sync*](../../docs/transcripts/187.md)
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): against a terminal-only product
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): one suite across computers, cloud and mobile
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`, `openagents-chat-app`, `openagents-ui`, `coder-sync`, `openagents-connect`
- **NIPs:** [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Also:**
  - [The apps roadmap](../../docs/apps/2026-11-10-apps-roadmap.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16

## Open questions for the owner

1. The [sync spec](../../docs/sync/README.md) recommends syncing a
   conversation through our service, readable by the server at first
   (chapter 35). Confirm that, and say which collections get end-to-end
   encryption first.
