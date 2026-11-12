# Service levels and alerts

This page sets three service levels for the Verse rooms, explains how each one
is measured, and lists every alert with its runbook line (#13297). The policy
lives in `coax/web/api/service-policy.coax`. When a target here changes, change
it there in the same commit.

## The three service levels

| Level | Target | Objective | Measured from |
| --- | --- | --- | --- |
| Tick time | The room tick's p99 is 10 ms or less | 99.9% of shard-minutes that had players | Each room's `tick p99 ms=` line in its 10 s report |
| Join time | Play to seated in 5 s or less | 95% of joins | The `room-join` analytics event's time, minus the newest navigation, `create-done` or `portal-shown` in the hour before it; team sessions are excluded |
| Checkpoint age | The oldest unsaved checkpoint is 15 s old or less | 99.9% of shard-minutes | Each room's `checkpoint unsaved age ms most=` line (`room-observe.coax rm_st_unsaved_ms`) |

Each level is counted over 30 days, with a count for each day. The
`checkpoint capture age ms most=` line can't serve as the checkpoint level,
because a slot whose player left keeps its last capture.

How the numbers are kept:

- Rooms post `/api/playtest/room-stats` every 10 s. That table keeps one day.
- The trigger `verse.room_minutes_add` (migration 0168) folds each report into
  `verse.room_minutes`. That table keeps one row per shard per minute, holding
  the worst tick p99 and the worst unsaved age.
- `verse.service_levels` counts the good and total minutes and joins.

Read the levels at `GET /api/playtest/service-health` (JSON) or on
[/status](https://openagents.com/status) under "Service, last 30 days".

## Alerts

`POST /api/jobs/service-watch` runs every minute on Cloud Scheduler, on staging
and on production. It does four things:

1. It reads each shard's facts (`verse.service_shard_facts`) and the
   database's connection count.
2. It runs them through the policy.
3. It opens an alert for each condition that holds and resolves each alert
   whose condition stopped (`verse.service_alerts`).
4. It mails every alert it opens or resolves to `OA_ALERT_TO`, the owner's
   address.

Separately, the green-main watcher (`coax/remote/after.sh --watch`,
`pipeline-lib.sh pl_service_watch`) reads the service health every minute. It
passes each alert it sees open or resolve to the Foreman inbox and to
`OA_PIPELINE_NOTIFY`.

| Alert | Fires when | Runbook |
| --- | --- | --- |
| `silent:N` | Shard N named its address, isn't draining, was heard within the last day, and has sent no report for 20 s | `coax/web/room/vm-deploy.sh status`; restart the unit, or `vm-deploy.sh switch run` |
| `tick:N` | Tick p95 is over 10 ms on 2 or more reports within 2 minutes | `/admin/rooms`: find which workload grew; roll back the room (`vm-deploy.sh rollback`) if a release did it |
| `checkpoint-failures:N` | A checkpoint post failed in the last 5 minutes | Check the API's log for `room-checkpoint`, the database, and oa-api's health |
| `fenced:N` | 3 or more checkpoints were fenced (stale owner) in 10 minutes | A character may be playing on two shards: check the directory's placement and the seat tickets' owners |
| `unsaved:N` | A checkpoint has stayed unsaved for 30 s, so its player's encounter is held | Check the checkpoint store: oa-api's 5xx and the database; then the room's checkpoint retries |
| `db-connections` | 80% or more of the database's connections are in use | Cloud SQL `openagents-production-pg`: find who holds the connections (`pg_stat_activity`); check oa-api's instances |
| API 5xx (Cloud Monitoring) | oa-api answers 5xx more than 3 times a minute for 5 minutes | Read oa-api's log for the failing route; check the database and the last API deploy; roll back with `coax/web/api/deploy.sh` if a release did it |

A silent shard raises its alert within about 80 s: 20 s of silence, plus at most
one minute until the next scheduled run.

## Resources and cost

| Resource | Name | Cost |
| --- | --- | --- |
| Cloud Scheduler job | `oa-api-service-watch` (every minute) | $0.10 a month |
| Cloud Scheduler job | `oa-api-staging-service-watch` (every minute) | $0.10 a month |
| Cloud Monitoring alert policy | `oa-api: 5xx responses (#13297)` (`alertPolicies/5351676641037482764`), on the free Cloud Run `request_count` metric | about $0.10 a month (one condition) |
| Notification channel | the existing `One Production On-call` email channel | free |
| Email | Resend, through the API's existing `email_send` | within the current plan |

To add the jobs again, run `coax/web/api/schedules.sh service-watch`. Run it with
`--staging` to add only the staging job.
