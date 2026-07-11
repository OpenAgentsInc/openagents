# D1-B: authoritative conversation through Desktop Runtime Gateway

- Issue: #8669
- Parent track: #8574
- Status: implemented; close after the main receipt is posted
- Depends on: closed #8667 and #8668

## Landed boundary

Desktop Runtime Gateway protocol v2 adds two closed queries:

- `conversation.catalog` — confirmed thread refs/metadata/entity versions plus
  personal-scope phase/cursor/pending count;
- `conversation.thread` — opens the exact thread scope and returns confirmed
  message refs/bodies/timestamps/entity versions plus thread phase/cursor/
  pending count.

It adds two bounded commands:

- `conversation.create(threadRef, title)`;
- `conversation.append(threadRef, messageRef, body)`.

Both enqueue the canonical shared mutator and return `pending_reconcile` plus
the durable mutation id. Enqueue never means server acceptance or completion.
The signed renderer still has only the generic schema-decoded Runtime Gateway
call; no new preload method or IPC channel was added.

## Safety and honesty

The protocol cannot represent owner identity, access/refresh/provider tokens,
store/session/overlay/transport objects, raw events, process/filesystem
authority, `MessagePort`, or arbitrary commands. Not-live and read failures are
typed and body-free. Entity refs/title/body are schema-bounded.

## Explicit residual

The current Effect Native shell still consumes its local-only chat channels.
Switching the visible catalog/transcript/composer to protocol v2 is the next
bounded leaf. Provider-neutral runtime streaming, assistant replies, rich
composer context, and live GUI/account acceptance remain separate D1 work.
