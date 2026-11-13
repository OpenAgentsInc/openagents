# 34. State that survives

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Everything so far lives as long as the app is open. This chapter keeps what
should outlast it: the draft you were typing, where you were, and the record
of every conversation, and it reconnects to the service without losing or
repeating anything.

## Draft

Three kinds of state outlast a launch, and each has one home.

- **This device's preferences,** such as which folds of the side are shut,
  stay on the device. On the web they live in the browser's storage; on the
  iPhone, in the app's own files.
- **Your unsent work,** such as a draft or the route you were on, also stays
  on the device, and the app restores it at the next launch.
- **Your account's data,** such as conversations and settings, lives on our
  service, and the device keeps a copy it can show while offline.

Nothing secret is written to the first two. Tokens and keys stay in the
Keychain or the browser's protected store, and seeds never leave the
wallet's own storage.

A restore is an action like any other. The app reads what it saved, checks
its version, and either applies it or starts clean and says so. A saved
state from an older app version that the current one can't read is
discarded, not guessed at.

A reconnect is a resume. The app asks the service for what changed since the
last record it saw, numbered by the server, and applies it in order. It
resends only what the service never acknowledged, with the same operation
IDs, so a message is stored once however often the network drops (chapter
9). A turn that was running when the connection dropped keeps running where
its execution owner is; the app shows its record and does not start it again
(chapter 37).

## What we build

A plan for what this chapter ships once the draft is approved:

- Saved preferences and drafts, restored at launch, on the web and the
  iPhone.
- A local copy of the conversation list and records, readable offline.
- Reconnect: catch up from the last server number, resend unacknowledged
  sends with their IDs.

It builds on what exists today:

- [`docs/sync/README.md`](../../docs/sync/README.md)
- [`coax/web/state-keep.js`](../../coax/web/state-keep.js)
- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`coax/lib/agent/session/log.coax`](../../coax/lib/agent/session/log.coax)
- [`coax/checked-state.md`](../../coax/checked-state.md)

### The chapter's check

- **Source:** the persistence and reconnect actions in the shared app.
- **Trace:** type a draft, end the app, launch it again and print the
  restored route and draft; then drop the connection during a send, reconnect
  and print the server numbers applied.
- **Refusal:** a saved state with an unknown version is discarded with a
  logged reason, and the app starts at home.
- **Independent check:** the service's store holds the message once after
  three forced reconnects with the same operation ID.

Planned paths: none new. The actions go in [`apps/shared`](../../apps/shared).

## Sources

- **Episodes:**
  - [Episode 172, *Sync Engine*](../../docs/transcripts/172.md)
  - [Episode 187, *Mobile Sync*](../../docs/transcripts/187.md)
- **Also:**
  - [Sync across devices](../../docs/sync/README.md), "How a device syncs"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
