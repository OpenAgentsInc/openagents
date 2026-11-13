# 75. Cross-host conformance

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Every client in this book was built a chapter at a time, and each part's
cases ran on the web and the iPhone as they were written. This chapter
collects those cases into one conformance run and runs it on every host the
book names, so a host qualifies on evidence and not on a claim.

## Draft

Chapter 1 made a rule: no surface has its own agent logic. Chapter 6 made it
testable: one acceptance case, written as `coax drive` operations, gives the
same trace on two hosts. Since then every chapter that teaches code has added
cases. This chapter is not a new client. It is the run that keeps all the
clients honest.

A conformance run plays every case against one host and compares its trace
with the reference trace. A host qualifies for a case when its trace matches
and its declared operations cover the case. A host that can't run an
operation refuses that case and does not claim it. The result is a table:
each case, each host, pass, fail or refused, with the build each ran on.

The hosts are the targets in [CONTRACT.md](CONTRACT.md#targets): the web,
the iPhone simulator and device, the Android emulator and device, and the
desktop hosts. Android comes next after the web and the iPhone. It reuses the
existing Android adapters in
[`apps/shared/host/android`](../../apps/shared/host/android) and the same
cases; it does not claim a finished OpenAgents Android app until those cases
pass there. The terminal is a host with declared omissions: it shows text and
actions, and refuses cases that need a picture, a camera or a world.

Two kinds of checks stay outside the shared run because they can't be
shared. Keyboard input methods, accessibility with VoiceOver and TalkBack,
backgrounding and each platform's SDKs need the real host, and the device
cases record who ran them and on what. A simulator pass is evidence for the
simulator only.

Two product rules hold on every host. There is one conversation, with no
switch between a chat mode and a work mode. And the screen shows only things
that work: a control that does nothing is not drawn. A new member added to
the registry appears on every host with no change to any of them.

## What we build

A plan for what this chapter ships once the draft is approved:

- One conformance run over every case in the book, per host, with its result
  table.
- Android qualified against the first milestone's cases (chapter 11) on the
  existing Android adapters.
- Device cases for keyboard, accessibility, lifecycle and SDK checks, with
  their evidence recorded per target.

It builds on what exists today:

- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-cli`](../../apps/openagents-cli)
- [`apps/openagents-desktop`](../../apps/openagents-desktop)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`apps/everglade-mobile`](../../apps/everglade-mobile)
- [`apps/shared`](../../apps/shared)
- [`coax/lib/tui`](../../coax/lib/tui)
- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/web`](../../coax/web)

### The chapter's check

- **Source:** the case files under `openagents/examples/cases/`.
- **Trace:** the result table: one line per case and host.
- **Refusal:** a host that lacks an operation reports the case as refused,
  not passed.
- **Independent check:** each platform's own UI tests for the same flows,
  until the portable cases cover them.

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The stream a surface draws | `openagents.chat.v1` results and, for hosted runs, planned `openagents.agent.managed.v1alpha1` events ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1), [spec 18.2](../../docs/protocol/openagents-protocol-spec.md#182-openagentsagentmanagedv1); [compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C5) | HTTP stream; Nostr kinds 26900 and 27000 | Spec sketch; no `.proto` file. The web chat polls hand-written lines today |
| State that follows you between surfaces | `openagents.sync.v1alpha1` ([chapter 35](ch035-sync-across-devices.md)) | Binary frames on one WebSocket | Planned |

Planned paths: none new. The clients are the apps in [`apps/`](../../apps);
the portable cases live with each app's tests.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30 and 3:00, every surface
  - [Episode 139, *Going Mobile*](../../docs/transcripts/139.md)
  - [Episode 187, *Mobile Sync*](../../docs/transcripts/187.md)
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): against the terminal alone
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): back to a small terminal
  - [Episode 277, *Coder Terminal*](../../docs/transcripts/277.md): the Coder terminal
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): Coder on phones
- **Old code at `coder-v1.0.0`, crates:** `openagents-desktop`, `openagents-mobile`, `openagents-terminal`, `openagents-cli`, `openagents-web`, `openagents-ui`, `coder-mobile`, `terminal-app`
- **NIPs:** [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P13, P16

## Open questions for the owner

1. Does Android get the full OpenAgents app for 1.0, or does the Android game
   app come first and the agent follow?
