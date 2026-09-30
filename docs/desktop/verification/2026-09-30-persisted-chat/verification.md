# Persisted desktop conversations

Issue #10006 replaces sample sidebar rows in the live desktop with shared chat
records. A fresh, successfully opened store creates a new conversation and
mounts an editable composer automatically. Stable IDs, titles, messages, archive,
and restore use `openagents-chat`; the desktop adds no storage implementation.
Selecting another conversation keeps its local draft separate.

The conversation record commits its turns and summary in one encrypted atomic
write. The index remains an auxiliary list: a restart reconciles committed
record metadata if an interrupted index write omitted a new chat or title.
Older phone records without embedded summaries remain readable. The list keeps
at most 200 conversations, and records retain whole newest turns within the
existing bounded cache. Retention counts JSON escape expansion and metadata,
rather than only the original text bytes. An oversized individual message or a
failed write reports an error; it is not silently truncated.

Interrupted writes remain pending and can recover without appending or resending
a message. A durable unanswered user message offers explicit retry after restart.
The composer clears a submitted draft only when its exact send ID appears in a
snapshot without a storage error. Corrupt or missing saved records report an
error and cannot be replaced with an empty conversation. A corrupt index blocks
writes. Clicking away from the composer cancels marked text without taking focus
back during the next presentation update.

## Authority and host boundary

The app sends the shared typed commands over the host's existing same-user
control channel. The host owns the encrypted cache and the chat relay identity;
no signing key crosses into window state. Unix socket ownership and private
permissions, or the Windows named-pipe admission, apply to these operations just
as they apply to local pairing. Cache work runs in the host's blocking worker,
and the desktop control worker keeps it off the rendering thread.

These commands manage local hosted-chat data and grant no computer execution
rights. Coder dispatch uses the phone's NIP-HOST operations and grant checks in
#10014 and #10015. The shared hosted transport is bound here; live streaming,
stop outcomes, and failure handling are verified separately in #10007.

The sidebar and composer reimplement Zeron's presentation in Rust Native. No
Zeron code, storage, engine, or agent harness is copied.

## Checks

On pinned Rust 1.97.1:

- Shared chat: 25 tests pass; the public relay smoke remains opt-in. Fixtures
  cover encrypted restart, exact send IDs, interrupted writes, metadata recovery,
  corrupt storage preservation, archive and restore, and escaped-text retention.
- Private host control: all eight tests pass. A scratch host creates a chat,
  restarts on the same temporary store, reads completed synthetic messages and
  their restored title, then archives and restores the conversation through its
  real private socket. Existing pairing, revocation, and project tests pass.
- Desktop: 106 tests pass, with three opt-in timing tests ignored. A fresh shared
  snapshot opens an editable composer without terminal or key setup.
- `openagents-connect` tests pass, including its framing and loopback connections.
- Targeted formatting and Clippy with warnings denied pass.
- The separate phone workspace passes `cargo check --offline --locked`.

All scratch identities, roots, chats, tasks, and sockets stay under temporary
directories. Tests do not use the owner's home, keychain, host, or chat lists.
No resident host deployment or store release is part of this source slice; an
installed app needs the rebuilt bundled host to accept the new commands.
