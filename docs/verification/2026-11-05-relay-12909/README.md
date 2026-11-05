# Rust relay: one unreadable stored event no longer stops the process (#12909)

relay.openagents.com runs the Rust relay image
`nostr-relay:relay-58d642e708-20261026175552`, built from commit `58d642e708`.
The relay crates (`crates/nostr-relay`, `crates/nostr`) left `main` in
`0b401561a0`, so the fix is built on `58d642e708` and kept here as a patch,
and as the pushed tag `nostr-relay-12909` (commit `55bc4be823`).

## The failure

The production table holds one kind 31990 handler event that the current
read check refuses. Any REQ whose answer includes it made
`decode_event_row` return `StoreError::CorruptRow`; the database worker
treated that as fatal and the process exited (`Container called exit(1)`).

## The fix

[0001-skip-unreadable-stored-events.patch](0001-skip-unreadable-stored-events.patch):

- Event reads (`query_filter*`, `event_by_id`, `event_by_ingest`,
  `events_after`, the handler list) leave out a row that fails the read check
  and write one warning line with its id and reason:
  `{"level":"warn","message":"a read skipped a stored event that fails the read check","id":"…","reason":"…"}`.
  The row stays in the table. The relay never deletes or rewrites it.
- `CorruptRow` is no longer fatal to a database worker. The read-state
  snapshot, which promises a complete cut, still refuses that one request.
- A history read that skipped a row ends with NIP-67 "more", not "finish",
  because the relay holds a matching event it did not send.

## Test

`crates/nostr-relay/tests/gateway_postgres.rs`,
`stored_row_failing_read_check_contract`: it stores a readable kind 1 note and
a kind 31990 whose `alt` tag fails the platform-link check (the production
reason text), bypassing admission as the old relay did, then sends the same
REQ twice on fresh connections.

| Run | Result |
| --- | --- |
| Without the source change | the worker logs `a database worker failed … stored event is corrupt: invalid event: a platform URL contains the bech32 placeholder and an optional NIP-19 type`, the REQ never gets EOSE, the test fails |
| With the fix | both REQs return the note and EOSE, the 31990 is left out, the warning line names its id; `store_postgres` 1 passed, `gateway_postgres` 3 passed, `--lib` 57 passed |

Rerun on a disposable Postgres 16 cluster (Unix socket only):

```sh
git worktree add ../relay-12909 nostr-relay-12909
cd ../relay-12909
scripts/test-postgres.sh   # or the store and gateway suites alone
```

## Deploy

Deploying is a production change and is listed in the workspace
`NEEDS_OWNER.md`. From the tag, `deploy/nostr-relay.cloudbuild.yaml` builds
the image (`_TAG=relay-12909`), then the Cloud Run service
`openagents-nostr-relay` takes the new image. Nothing in the database
changes.
