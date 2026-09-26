# Signed-event JSONL import

`nostr-relay import-jsonl` imports an ordered export of signed Nostr events into
the relay's Postgres store. It preserves each event's `id`, `pubkey`, `sig`,
`created_at`, kind, tags, and content. The command never signs, rewrites, or
decrypts an event.

## Source contract

Prepare UTF-8 JSONL with one complete NIP-01 event object per line, in the
source relay's durable admission order. For a source that records an
`ingest_seq`, sort ascending by that value. Lines must contain exactly the
seven event members, may not be blank, and may not exceed 131,072 bytes.
Unknown or duplicate JSON members fail the import.

Export signed event objects, not database projections or unsigned event
payloads. Keep the source export and a SHA-256 digest until cutover is
accepted.

## Run

Stop every relay process using the destination database before importing. This keeps ephemeral notifications and the
durable sequence boundary out of live client traffic. The command applies and
verifies the relay's embedded migrations, so use the same database-owner
credential as a normal first start.

```sh
sha256sum source-events.jsonl
DATABASE_URL='postgres://nostr_relay:<YOUR_DB_PASSWORD>@127.0.0.1:5432/nostr-relay' \
  ./nostr-relay import-jsonl < source-events.jsonl
```

Success prints one JSON report:

```json
{"input_lines":7,"stored":4,"duplicate":1,"already_removed":0,"ephemeral":1,"expired":1}
```

`stored` counts committed durable admissions. `duplicate` counts an event ID
already present. `already_removed` counts a valid event that an earlier or
existing deletion/replacement has made non-current. `ephemeral` and `expired`
events are validated but not stored.

## Failure and replay

The import commits one valid durable event at a time. A malformed event,
invalid signature, current relay-policy refusal, database error, or unexpected
coordination outcome stops at that line and returns a non-zero status. The
successful prefix remains committed.

Fix or remove only a source-export defect after reconciling it with the source
relay, then rerun the complete file. Duplicate and already-removed outcomes
make full-file durable replay idempotent: the replay does not allocate new
durable sequence numbers for those events. Ephemeral events can notify a live
gateway again, which is another reason to run the command while the relay is
stopped.

The importer applies NIP-01 shape, cryptographic, replacement, deletion, and
the configured M2 policy pipeline. It uses the historical-admission lane for
extension rules a source relay could not have enforced. This does not bypass
author ownership, signatures, event identity, limits, or relay policy.

After the report matches the source reconciliation, start the relay and verify
health, NIP-11, representative event IDs, replacement heads, and deleted-event
absence before moving the hostname. Use
[`runbook-debian-vps.md`](runbook-debian-vps.md) for the cutover and rollback
sequence.
