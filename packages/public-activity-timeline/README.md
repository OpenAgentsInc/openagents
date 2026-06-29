# @openagentsinc/public-activity-timeline

Shared contract for `openagents.public_activity_timeline.v1`, the public-safe
Pylon visibility timeline that agents, CLI commands, Worker routes, replay
builders, web, and desktop surfaces should all consume.

This package defines the schema only. It does not read D1, validate work,
authorize settlement, move sats, post to the Forum, dispatch assignments, or
promote product claims.

Agent-facing route docs, curl recipes, stale/error states, and the public
evidence endpoint index live in
`docs/launch/2026-06-18-agent-activity-endpoint-guide.md`.

## Envelope

Timeline responses use this shape:

```json
{
  "schemaVersion": "openagents.public_activity_timeline.v1",
  "generatedAt": "2026-06-18T18:00:00.000Z",
  "staleness": {
    "contractVersion": "projection_staleness.v1",
    "composition": "live_at_read",
    "maxStalenessSeconds": 0,
    "rebuildsOn": ["public_activity_timeline_read"]
  },
  "nextCursor": "2026-06-18T18:00:02.000Z:settlement_receipt:event.public.settlement.1",
  "sourceLag": [],
  "events": []
}
```

`sourceLag` is part of the contract. A fresh envelope may still contain stale or
unavailable source legs, and agents must display that rather than treating a new
`generatedAt` as proof that every source is current.

## Cursor

The stable cursor key is:

```text
{event.ts}:{event.sourceKind}:{event.eventRef}
```

Events sort by `ts`, then `sourceKind`, then `eventRef`. `ts` is an ISO-8601 UTC
timestamp ending in `Z`; `sourceKind` is one of the finite source-kind values;
`eventRef` must be a public-safe opaque ref, not a URL with query secrets.

## Event Kinds

The finite event-kind enum covers:

- `pylon_registered`
- `pylon_heartbeat`
- `wallet_ready`
- `assignment_ready`
- `window_opened`
- `window_closed`
- `work_claimed`
- `trace_submitted`
- `verification_queued`
- `verification_verified`
- `verification_rejected`
- `settlement_recorded`
- `real_bitcoin_moved`
- `forum_topic_created`
- `forum_posted`
- `artanis_tick`
- `capacity_snapshot`
- `projection_gap`

Every event must carry `sourceRefs` or `blockerRefs`. If a source table cannot
produce enough public-safe detail, emit `projection_gap` with blocker refs
instead of fabricating state.

## Real Bitcoin Rule

`realBitcoinMoved: true` is allowed only when the event cites a receipt source.
Simulation rows remain visible as `settlement_recorded` with
`realBitcoinMoved: false` plus caveat refs, and they must not emit
`real_bitcoin_moved`.

## Fixtures

The `./fixtures` export includes:

- `emptyTimelineFixture`
- `activeTimelineFixture`
- `staleTimelineFixture`
- `replayRangeTimelineFixture`
- `simulationOnlyTimelineFixture`
- `realBitcoinTimelineFixture`

They are intended for route, CLI, replay, and UI contract tests. All fixtures are
public-safe refs only.
