# 51. Sync across devices

> **Status: outline.** This chapter lists what it will say and build. Its
> draft is not written, and paths marked planned do not exist.

## Premise

Chapter 2 promises one conversation on every surface. This chapter builds what
moves it: your settings, characters, saves and chats follow your account from
one device to the next.

## Outline

- **The account syncs.** Log in on a phone, a Mac or a browser and you see the
  same things. A change reaches your other open devices in a second or two,
  and a device that was offline catches up.
- **What syncs.** Settings, profile, characters and lists as rows. Saves as
  documents. Chats as logs. Thumbnails and full tool output as blobs.
- **What never syncs.** Session tokens, device keys, agent keys and wallet
  seeds. Graphics settings that belong to the hardware. Caches.
- **Four models.** A row keeps current state, and the newest write to each
  field wins. A document is saved only against the revision it was read at. A
  log is append-only. A blob is fetched when it is needed.
- **The server is the authority.** We tried a shared-editing document type
  before, and its history stalled the server three times in a week. Every
  write already went through one server, so it bought nothing.
- **How a device syncs.** One socket for each device. The device says where it
  left off and gets only what changed. A push notification can wake an app,
  and it never carries content.
- **The frames are binary.** They are protobuf messages in
  `openagents.sync.v1alpha1`: the third carriage of chapter 49. Text encoding
  would cost a third more on a slow link.
- **Guests.** A guest's data lives on one device and merges into the account
  at login.
- **Agents.** An agent is an account, so its settings and memory can sync
  across its hosts. That is a later step.
- **Privacy, said plainly.** The server can read synced data at first, and
  Settings says so. End-to-end encryption for chosen collections comes later.
  A message that looks like it holds a password or a key is not synced.
- **Limits are shown,** and export and delete cover every synced thing.
- **Where Nostr fits.** A public record is still a signed event on relays.
  Sync is for your private account state.

## What we build

A plan for what this chapter ships once the outline is approved, in the
[sync spec's](../../docs/sync/README.md) order:

- Rows first: settings and profile, pulled after a cursor and pushed in
  batches.
- The socket, then guest merge, documents, blobs and logs.
- The device list in settings, and notifications that only wake.

Two things exist today: saves that are kept only against the current revision,
and a guest's data moving to an account at login. The rest is a plan.

It builds on what exists today:

- [`docs/sync/README.md`](../../docs/sync/README.md)
- [`coax/web/api/progress-policy.coax`](../../coax/web/api/progress-policy.coax)
- [`coax/web/api/player.c`](../../coax/web/api/player.c)
- [`docs/auth/sessions-and-tokens.md`](../../docs/auth/sessions-and-tokens.md)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Sync frames: `hello`, `push`, `state`, `changes`, `ack`, `probe` | `openagents.sync.v1alpha1`, planned at `proto/openagents/sync/v1alpha1/` | Binary frames on one WebSocket | Planned |
| A chat that syncs | A log of thin entries, with blobs for large output | Inside sync frames | Planned. The entry format follows the session log ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C1) |
| A save | A document with a revision | HTTP today | Built for realm progress, with hand-written bodies |

Planned paths: `proto/openagents/sync/v1alpha1/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): the same thing on every surface
- **NIPs:** none. Sync is not a Nostr carriage.
- **Also:**
  - [Sync across devices](../../docs/sync/README.md)
  - [OpenAgents authentication](../../docs/auth/README.md)
  - [The protocol review](../../docs/protocol/2026-11-11-protocol-review.md), step 3
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Open questions for the owner

1. Confirm the sync spec's recommendations S1 to S6, including the 1 GB limit
   for each account.
2. Which collections get end-to-end encryption first?
