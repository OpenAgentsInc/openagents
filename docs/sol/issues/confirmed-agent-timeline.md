# D1-D: confirmed provider-neutral agent timeline

- Issue: #8672
- Parents: #8574, #8597, #8566
- Status: closed; historical checked-in issue source
- Depends on: closed #8670 and #8671

## Landed boundary

`packages/khala-sync-client` now owns one reader for the already-landed
`agent_run` and `agent_run_event` entity contracts. A consumer opens exactly
`scope.agent_run.<runRef>` and receives:

- explicit scope phase, cursor, and pending count;
- confirmed public run ref, route ref, lifecycle timestamps/status, and entity
  version; and
- at most the newest 500 confirmed events, deduplicated by event ref and sorted
  by sequence/ref, with generic type/summary/status/artifact refs/timestamp and
  entity version.

The projection deliberately omits owner, goal/objective, repository,
runtime/backend, event source, raw payload JSON, external callback refs, and
all store/session/transport objects. Cached rows are returned only while the
exact run scope is live; catching-up, must-refetch, denied, and idle snapshots
carry status but no content.

## Evidence

The real shared SQLite store fixture applies out-of-order event entries, replays
the same changelog batch at least once, closes/reopens the database, and proves
ordered reconstruction without duplicates. It separately proves the 500-event
bound and fail-closed non-live behavior.

Contract: `khala_sync.client.confirmed_agent_timeline.v1`.

## Explicit residual

This leaf does not expose the timeline through Desktop Runtime Gateway or
mobile, launch a provider runtime, bind a thread to a run, stream a live owner
account, or add interrupt/resume. Those consume this shared reader in later D1
leaves; no competing event schema was introduced.
