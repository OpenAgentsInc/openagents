# Browser byte storage

`host.js` exposes optional IndexedDB effects to a Coax app through its C/Wasm
face. The host stores opaque bytes. Coax chooses namespaces, schema versions,
keys, content, retention, migrations and whether a record should be trusted.
This is the byte pipe for the later Nostr database; it does not implement that
database or persist any signing key automatically.

## Imports and bounds

All parameters and results are wasm32 `i32`. Pointers address the imported
`env.memory`. Functions return **0 when accepted**, otherwise the status below
immediately, with no asynchronous reply. Slots are caller-selected 0–7; a
nonzero 32-bit `token` identifies the request. Only one request may occupy a
slot. Signed C representations of nonzero `u32` tokens retain their bits.

```c
int coaxial_storage_put(int slot, int token,
    int namespace_ptr, int namespace_len, int version,
    int key_ptr, int key_len, int bytes_ptr, int bytes_len);
int coaxial_storage_get(int slot, int token,
    int namespace_ptr, int namespace_len, int version,
    int key_ptr, int key_len);
int coaxial_storage_delete(int slot, int token,
    int namespace_ptr, int namespace_len, int version,
    int key_ptr, int key_len);
int coaxial_storage_cancel(int slot, int token);
```

- Namespace: 1–64 UTF-8 bytes. Key: 1–128 UTF-8 bytes. Malformed UTF-8 is
  refused; strings are not normalized. Versions are integers 1–2147483647.
- Records: 0–1048576 bytes. Empty bytes are a present record, distinct from a
  missing record. The entire input span must fit the current Wasm memory.
- Put copies its input before returning. Reusing the input or growing Wasm
  memory cannot change an accepted request's bytes.
- The app must have the inbox/message exports and at least 16 inbox bytes when
  requesting storage. It should retain this capacity until replies complete.
- The app calls these imports after the engine has loaded. A fresh engine
  installation cancels all pending storage effects, even with the same layout.

Each origin uses IndexedDB database `coaxial-bytes`, database version 1, object
store `bytes`. The object-store key is `[namespace, version, key]`, and its value
is an `ArrayBuffer`. **A namespace separates records, not principals:** scripts
with access to the same origin share this database. Each app should choose a
stable, distinct namespace. The host does not fall back to localStorage.
The older one-text-per-app convenience/action 4 is unchanged.

## Inbox replies

An accepted request replies through
`coaxial_message(state, 768 + slot, total_bytes)`. The inbox contains this
**16-byte little-endian header**, followed by uninterpreted payload bytes:

| Offset | `u32` field | Meaning |
| --- | --- | --- |
| 0 | token | Original request token |
| 4 | status | Table below |
| 8 | length | Payload bytes actually present |
| 12 | needed | Full record payload length; zero for put/delete/missing/error |

Get success includes the exact record bytes. Put/delete success has no payload;
delete is idempotent. A get that exceeds the inbox has `CAPACITY`, no payload,
and its full required payload length in `needed`. No record is truncated.
A malformed stored value reports `CORRUPT`; a stored record beyond the record
limit reports `LIMIT` and its required size. Consumers must check status and
length before decoding bytes, including proto bytes.

| Code | Name | Outcome |
| --- | --- | --- |
| 0 | OK | Accepted immediately, or transaction completed successfully |
| 1 | MISSING | Get found no record; cancel found no matching pending token |
| 2 | UNAVAILABLE | Browser denied storage, blocked an open, changed its version, or lost the store |
| 3 | QUOTA | Browser reported quota exhaustion |
| 4 | LIMIT | Record exceeds the 1 MiB limit |
| 5 | INVALID | Invalid slot/token/version/text/span |
| 6 | BUSY | Slot already has a pending request |
| 7 | ERROR | Other IndexedDB failure/transaction abort |
| 8 | TIMEOUT | Operation/open did not complete within 10 seconds |
| 9 | CAPACITY | Missing receiver/header room, or result cannot fit the inbox |
| 10 | CORRUPT | Stored value is not an `ArrayBuffer` |

Put/delete success is reported only after `transaction.oncomplete`, not when
an individual request succeeds. Browser storage remains subject to eviction,
user deletion, private-mode limits and platform durability guarantees. A quota
or generic failure does not imply a record was written.

## Cancellation and replacement

`cancel(slot, token)` returns OK for the matching pending request and MISSING
for an absent/mismatched one. It frees the slot, suppresses all its later
callbacks and attempts to abort its transaction. It produces no inbox reply.
Engine replacement does the same for every slot. Slot identity and engine
generation both guard completion, so an old callback cannot complete a new
request with the same slot and token. A failed/blocked open does not poison
later opens; a late database success is closed.

**Cancellation and timeout do not promise rollback of an already committed
write.** The result can be unknown if commit won the race; read to reconcile it.
Queued cancelled writes are aborted before they can overwrite/delete later
transactions. Deleting a record does not erase copies from memory, browser
backups or storage media. This host does not provide encryption or secure
erasure. Secret persistence requires a separately reviewed key-custody policy.

## Focused verification

```sh
bun test coax/web/storage.test.ts
```

The tests compile a small C face to Wasm and run the **shipped complete host**
in headless Chrome. Actual IndexedDB is exercised across page reloads, including
binary/empty records, namespace/version separation, delete, input reuse and
memory growth, inbox exhaustion, malformed spans/text, and queued cancellation
and engine replacement. Separate, explicitly injected effects cover unavailable
and private-mode-style denial, quota, blocked/late opens, hung transactions,
commit/abort races and stale callbacks after slot/token reuse. They do not claim
to reproduce every browser's private-mode or power-loss behavior. No network
service, secret, live signer, compiler bootstrap or iOS build is required.
