# Background Agent Per-Run Live Surface Thin-DO Spike

Date: 2026-07-04
Status: BA-G3 design doc + inert Worker API spike for
[#8210](https://github.com/OpenAgentsInc/openagents/issues/8210)

## Decision

Do not replace Durable Streams for background-agent run output. Durable
Streams remain the default live/resume transport for definition-backed runs.
The per-run Durable Object pattern is a future candidate only when WS-10's
one-status-spine work grows a client-facing live channel that needs
multi-client watching, steering, status decay, and run-local fanout.

The spike in
`apps/openagents.com/workers/api/src/agent-definition-live-surface-spike.ts`
therefore declares no `wrangler` binding, exports no live Durable Object
class, and registers no route. It pins the contract the live implementation
must satisfy if that adoption gate opens.

## Identity

The future object is one coordination atom per owner-scoped run:

```text
AGENT_DEFINITION_RUN_LIVE.getByName(
  "agent-definition-run-live:<ownerAgentUserId>:<runId>"
)
```

The run store remains the retained source of truth for run history. The live
object may cache and fan out run-local status/events, but `ownerAgentUserId`
and `runId` are the routing inputs and the owner-scoped run row remains the
authorization anchor.

## Thin Shell

The Durable Object must be a transport shell. Business logic stays in injected
services so it can be unit-tested without a live object:

- `auth_verifier`
- `clock`
- `event_projector`
- `run_store`
- `status_spine_reader`
- `operator_audit_sink`

The shell owns only Durable Object lifecycle work: WebSocket upgrade,
attachment persistence, SQLite migrations, alarm scheduling, replay/fanout,
and calling the injected services.

## Storage And Migrations

The object uses Durable Object SQLite storage and an explicit
`_sql_schema_migrations` table. It must not use `PRAGMA user_version`.

The spike pins two numbered migrations:

- migration 1 creates `_sql_schema_migrations`, `live_clients`,
  `live_events`, and `live_alarm_tasks`;
- migration 2 adds run/client and due-task indexes.

`live_clients.serialized_attachment_json` is the durable mirror of the
hibernatable WebSocket attachment. `live_alarm_tasks` is the single queue for
all alarm reasons.

## WebSocket Hibernation

Each socket attachment is public-safe and bounded:

- schema version
- owner agent user id
- definition id
- run id
- client id
- watch-intent ref
- last acked sequence
- connected/last-seen wall-clock milliseconds

Attachments must not contain authorization headers, signatures, raw webhook
bodies, raw prompts, raw provider payloads, email bodies, tokens, or secrets.
On hibernation resume, the object reconstructs client mapping from
`deserializeAttachment()` plus the persisted `live_clients` row and then
replays from the last acked sequence.

## Single Alarm

Cloudflare Durable Objects expose one alarm per object, so the live design
must multiplex all run-local scheduled work through `live_alarm_tasks`.
The object schedules the platform alarm to the earliest due task and the
alarm handler processes due tasks idempotently before rescheduling the next
earliest task.

Initial task kinds:

- `client_idle_timeout`
- `pending_outbox_flush`
- `run_status_decay`
- `terminal_gc`

## Adoption Gate

`decideAgentDefinitionRunLiveSurface` enforces the current gate:

- no WS-10 client-facing live channel: stay on `durable_streams`;
- operator flag alone: stay on `durable_streams`;
- WS-10 channel plus operator enablement: candidate `thin_do_live_surface`;
- live adoption still requires a real binding, route contract, migration, and
  end-to-end tests in the same future change.

## Verification

The executable spike coverage lives in
`apps/openagents.com/workers/api/src/agent-definition-live-surface-spike.test.ts`.
It asserts the default transport gate, deterministic per-run DO naming,
numbered in-DO migrations, absence of `PRAGMA user_version`, hibernation-safe
attachment shape, injected-service refs, and single-alarm task selection.

Relevant platform docs:

- Cloudflare Durable Objects:
  <https://developers.cloudflare.com/durable-objects/>
- Durable Object WebSocket hibernation:
  <https://developers.cloudflare.com/durable-objects/best-practices/websockets/>
- Durable Object alarms:
  <https://developers.cloudflare.com/durable-objects/api/alarms/>
