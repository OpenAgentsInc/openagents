# Sites Builder Event Stream

Issue #194 adds the first replayable event stream contract for Autopilot Sites
builder sessions.

## Endpoint

`GET /api/sites/builder-sessions/:sessionId/events`

The endpoint returns `text/event-stream` with Server-Sent Events generated from
the durable `site_builder_events` ledger.

Clients can reconnect with either:

- `?cursor=<last-sequence>`
- `Last-Event-ID: <last-sequence>`

The response includes events with `sequence > cursor` in ascending sequence
order. Empty replays return a valid SSE comment so clients can distinguish a
healthy empty replay from a network failure.

## Authorization

The stream uses the same ownership boundary as the builder session read API.
Unauthorized users receive `404`, not a distinct forbidden response, so private
session existence is not leaked.

Non-admin owners only receive customer-visible events. Admin users receive
operator-safe event projections as well.

## Scope

This issue implements replayable SSE snapshots over the ledger. Long-lived
live push can be layered on later using the same sequence cursor contract.
The stream does not expose raw runner payloads, provider logs, source archives,
secrets, or unbounded diagnostics.
