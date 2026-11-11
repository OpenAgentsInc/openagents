# Sync across devices

Status: spec, 2026-11-11. Nothing here is built yet, except where a line
says it exists today.

Your account follows you. Log in on a phone, a Mac or a browser, and you see
the same character, settings, worlds, saves and chats. A change on one device
reaches your other open devices within a second or two, and a device that was
offline catches up when it reconnects.

This spec builds on the auth system in [docs/auth/](../auth/README.md): an
account is the thing that syncs, and every device that syncs holds a session
for it.

## What we learned before the Nuke

Before 2026-10-27 (`0b401561a0`) the OpenAgents apps synced chats and the
sidebar across devices. Read the old docs with
`git show 0b401561a0^:PATH`:

- `apps/openagents/docs/registry-sync.md`
- `apps/openagents/docs/chat2-sync.md`
- `apps/openagents/docs/sync-resource-resilience.md`
- `apps/openagents/docs/sync-capacity-calibration.md`
- `docs/mobile/coder-sync.md` and `docs/deployment/coder-sync-vm.md`
- the client in `apps/openagents/crates/sync/`

Five lessons carry over:

1. **CRDT history wedged the server.** The sidebar was a Loro CRDT document
   synced through a Durable Object. Three incidents in one week came from the
   same cause: the document kept its whole history, and the server had to
   replay it under a CPU limit. Every write already went through one server,
   so the CRDT bought nothing. The fix was the **registry**: rows holding
   current state only, a per-field last-writer-wins rule with hybrid logical
   clocks (about 40 lines), and a cursor so a device asks only for what
   changed. We start there.
2. **The server should not parse what it relays.** chat2 made the chat
   server a dumb, authenticated log of opaque blobs. Nothing on the server
   could wedge on a large document.
3. **Keep synced data thin.** Chats grew to 1 MB a day because full tool
   output went into the synced document; 85% of one chat was tool output.
   Sync the summary and keep the large part in a blob fetched on demand.
4. **Bound the client, not just the server.** A device with thousands of
   old chats opened thousands of connections. One connection per device, a
   cap on parallel work, and paging by ID fixed it.
5. **One place to run it.** coder-sync ran as its own service with its own
   Postgres and bucket. Today our API and its Postgres already exist, and
   the database is protected and backed up (#13194). Sync lives there, not in
   a second service.

What exists today that we keep:

- **Saves with revisions.** `GET/POST /api/playtest/progress/<world>/<name>`
  (`coax/web/api/player.c`, `progress-policy.coax`, migration 0119) keeps a
  save only when the device sends the revision that is there now, and
  answers 409 with the current one otherwise. It keeps the previous save too.
  This is the right rule for whole documents.
- **Guest keys move to an account.** A guest player key holds worlds, saves
  and sessions; logging in moves them to the account (`/api/playtest/signin`).
  Account sync keeps this: play first, log in later, lose nothing.
- **Position checkpoints** for the room servers stay as they are. They are
  game state the room owns, not account sync.
- **Browser byte storage** (`coax/web/STORAGE.md`): IndexedDB records of up
  to 1 MiB under a namespace and key. This is the web client's local store.

## What syncs

| Kind | Examples | Model |
| --- | --- | --- |
| Settings | Gyro mode and speed, inverted look, HUD layout, sound, language | Rows |
| Profile | Display name, chosen character, Lightning Address choice | Rows |
| Characters | Your Everglade characters from Character Create | Rows, with the mesh and textures as blobs |
| Lists | Favourite and recent worlds, friends, pinned chats, bookmarks | Rows |
| Saves | A world's progress, a WorldBuilder world | Documents |
| Chats | OpenAgents chats, agent conversations you choose to sync | Logs |
| Large things | Thumbnails, full tool output, screenshots, uploads | Blobs |

What never syncs:

- **Secrets.** Session tokens, device keys, agent keys and wallet seeds stay
  on the device that made them. The wallet's encrypted backup has its own
  path ([wallets-and-custody.md](../auth/wallets-and-custody.md)).
- **Per-device tuning.** The graphics tier, render scale and frame-rate
  governor state belong to the hardware. A setting can say which it is:
  account-wide, or this device only.
- **Caches.** Downloaded assets, compiled shaders and world packs are
  fetched again.

## The four models

Each synced thing uses one of four models. A new feature picks one; it never
invents a fifth.

### Rows: current state, per-field newest wins

For settings, profile, lists and small records. This is the pre-Nuke
registry, rebuilt in Coax.

- A row is `{collection, id, fields, clocks, deleted, seq}`. Each field has
  its own clock, so changing gyro speed on the phone and HUD layout on the
  Mac at the same time keeps both.
- A clock is a hybrid logical clock, `"{ms:013}-{counter:06}-{device}"`.
  Comparing the strings orders the writes; the device ID breaks ties.
- A write to a field applies only when its clock is newer than the stored
  one. `null` clears a field and is still a clocked write. Applying the same
  write twice changes nothing, so a device can resend safely.
- `update` never creates a row; `upsert` creates one, and revives a deleted
  row only when newer than the delete.
- Deletes leave a tombstone for 30 days. A device whose cursor is older than
  that gets a full resync.
- The server stores current state only. No history, so nothing grows
  without limit.

### Documents: whole bytes, guarded by revision

For saves and WorldBuilder worlds: one blob of bytes the server stores
without reading. This is the progress rule that exists today, made general.

- A write sends the revision it was based on. The server keeps it only if
  that is still the current revision, then bumps the revision.
- Otherwise the server answers 409 with the current bytes. The client merges
  if the feature knows how (WorldBuilder can, object by object), or keeps
  both and asks you which to keep.
- The server keeps the previous revision, so one bad write can be undone.

### Logs: append-only, numbered by the server

For chats and anything else that is a sequence of messages.

- A device appends an entry with an ID it made itself. The server gives it
  the next number in that log. Resending the same ID returns the same
  number, so retries never duplicate a message.
- A device catches up by asking for entries after the last number it has.
- An entry is thin: text and a summary. Anything large is a blob the entry
  points to.
- Deleting a chat deletes the log everywhere.

### Blobs: large and immutable, by content hash

For images, meshes, tool output and uploads.

- Stored in a private Cloud Storage bucket, named by the SHA-256 of the
  bytes, and owned by the account that uploaded them.
- Fetched only when shown. A row, document or log entry holds the hash.
- A blob no longer referenced is deleted after 30 days.

## How a device syncs

1. **Local first.** Every device keeps its own copy: IndexedDB on the web
   (through the byte storage above), files on Mac, iPhone and Android. The
   app reads and writes the local copy, so it works offline and opens
   instantly.
2. **An outbox.** Writes made offline wait in a queue on the device. Reads
   show the queued writes on top of the server's state.
3. **One connection per device.** The device opens one WebSocket to the API,
   `wss://openagents.com/api/sync`, with its session. It sends `hello` with
   its cursor and gets back everything after it. Then it pushes its outbox
   and receives other devices' changes as they happen.
4. **A cursor per account.** Every change to an account's rows, documents
   and logs takes the next number in that account's sequence. A device that
   was away asks for "after N" and gets only what changed.
5. **Wake, then fetch.** When the socket is closed, as on a phone in the
   background, a push notification may say "something changed" with no
   content. The app fetches when it next opens. Content never travels in a
   push notification.
6. **Bounded.** A device opens one socket, whatever the number of chats or
   worlds. Catching up pages through changes by number, at most a fixed
   batch at a time.

The frames are defined as protobuf in `proto/openagents/sync/v1alpha1/`, with generated Coax codecs, like every other wire in `proto/`. They follow the pre-Nuke registry, as binary frames (a type byte,
a small header, raw bytes) rather than base64 JSON, because base64 costs a
third more on slow links:

| From the device | Meaning |
| --- | --- |
| `hello {cursor, device}` | First frame. `cursor` is null on a new device |
| `push {batch, ops}` | Row writes, document writes, log appends |
| `probe` | Is the socket alive |

| From the server | Meaning |
| --- | --- |
| `state {seq, full, changes, floor}` | The reply to `hello`: everything after the cursor, or everything if the cursor is too old |
| `changes {seq, changes}` | Other devices' changes, as they happen |
| `ack {batch, seq, applied, refused}` | What happened to a push, with a reason for each refusal |
| `probe-ok {seq}` | The socket is alive |

## Accounts, guests and devices

- **The account syncs.** Every sync request carries a session from the auth
  system (#13201). The server checks it once per socket and again when the
  session is revoked; a revoked session's socket closes within a second.
- **Guests sync too, to one device.** A guest key holds rows, saves and
  worlds on the server, as it does today, but has no other devices to sync
  with. Logging in merges the guest's data into the account. Rows merge by
  clock, documents by keeping both when they differ, logs by moving them.
- **Devices are listed.** Settings shows each device with its name, its last
  sync time and Remove, from the sessions list (#13201). Removing a device
  ends its session and its socket.
- **Agents are accounts too.** An agent's own settings and memory can sync
  between the hosts it runs on, under its own account and scopes (#13211).
  An agent never reads its owner's synced data unless a grant says so.

## Privacy

- **We can read synced data on the server**, as we can read a save today.
  We say so in Settings. End-to-end encryption for chosen collections (notes,
  private chats) is a later step, sealed with the same keys as the wallet
  backup. It is not in the first build.
- **Messages that look like they hold a password or key are not synced.**
  The pre-Nuke Coder sync did this, and we keep it.
- **Export and delete.** "Export my data" includes every synced row,
  document, log and blob (#13214). Deleting the account deletes them, and
  the backups age them out within 35 days.

## Limits, shown

Every limit is shown in Settings, with how much you use:

| Limit | Default |
| --- | --- |
| Storage per account, blobs included | 1 GB |
| A document | 8 MB |
| A blob | 50 MB |
| A log entry | 64 KB |
| Pushes | 20 a second per device, bursting to 100 |
| Devices | 20 per account |

When a limit is hit, the app says which one and what to do. No limit is
hidden.

## Storage and backups

- Rows, documents and log entries live in the API's Postgres, in a `sync`
  schema, written only through migrations run by the migration role. The API
  role gets row access only (#13194).
- The nightly encrypted dumps and point-in-time recovery cover them.
- Blobs live in a private bucket with object versioning, so a deleted blob
  can be restored within 30 days.

## Build order

1. **Rows.** The `sync` schema, the merge rule in Coax with shared test
   vectors, `GET/POST /api/sync` (pull after a cursor, push a batch), and
   settings and profile as the first collections. Gyro settings move from
   per-browser storage to account rows. Depends on sessions (#13201).
2. **The socket.** `wss://openagents.com/api/sync` with `hello`, `push` and
   live `changes`, reconnect with backoff, and probes, on web, Mac and
   iPhone through the shared page host.
3. **Guest merge.** Logging in merges the guest's rows and saves, with tests
   for every conflict case.
4. **Documents.** Generalise the progress rule to any save; move WorldBuilder
   worlds onto it, keyed by account instead of an unguessable world ID.
5. **Blobs.** The bucket, content-hash upload, on-demand fetch and cleanup.
   Characters sync with their meshes.
6. **Logs.** Chats sync across devices, thin entries with blobs for large
   output.
7. **Device list and wake.** Settings' device list, push notifications that
   only wake, export and delete.
8. **Later.** End-to-end encrypted collections; an agent's memory across
   its hosts.

Each step ships on its own and is checked with two headless browsers (and
the iPhone simulator from step 2) changing the same account at the same time,
including with one offline and on 150 ms of added latency.

## Decisions

| # | Decision | Recommendation |
| --- | --- | --- |
| S1 | CRDTs or server authority | Server authority, with per-field newest-wins rows. The pre-Nuke CRDT wedged the server, and every write goes through our API anyway |
| S2 | Where sync runs | In the existing API and its Postgres, not a separate service |
| S3 | Encryption | Server-readable at first, said plainly in Settings; end-to-end for chosen collections later |
| S4 | Graphics settings | Per device, never synced |
| S5 | Guests | Sync to their one device, and merge into the account at login |
| S6 | Storage limit | 1 GB per account, shown |
