# 99. Watch the whole action path

> **Status: partly implemented.** The room server measures its own action
> path and reports it every 10 s (#12791, commit b35943120f): tick p50, p95
> and p99 by workload, each socket's send queue bytes and age, bytes and
> messages a second by frame and entry, relevant entities a seat, the join
> burst, rejoins, input to acknowledgement per seat, and admission refusals
> by reason (`room-stats.coax`, `room-stats.h`). `/admin/rooms` shows each
> with a 10-minute sparkline. Measuring costs 0.35 to 1.1% of the tick in a
> 16-player local raid.

A player who says "my spell took a second to land" is describing the whole
action path, not one machine. This chapter adds measurement along that
path, so the slow part can be named.

## What this chapter teaches

- The stages of the action path and what to measure at each.
- Percentiles by workload, not host averages.
- Queue depth and queue age.
- Where the numbers are shown and which ones to watch during a raid.

## The design

The research doc (section 15) lists the path:

```text
input sampling → network → server queue → simulation → dependency work →
replication queue → network back → presentation
```

| Stage | Current observation |
| --- | --- |
| Input to acknowledgement | `rm_st_ack` reports when the newest acknowledged input arrived; the host records its delay. |
| Simulation | Tick p50, p95 and p99 by workload, with timing by part. |
| Replication queue | Per-socket queued bytes and oldest age; slow socket closures. |
| Network out | Bytes and messages by frame and row; deferred rows and byte allowances. |
| Interest | Relevant entities and budget omissions per seat. |
| Join | Bytes until the first complete state, and admission refusals by reason. |
| Reconnect | Rejoins and whether the player recovered their held seat. |

A host average hides one saturated room. Report percentiles per room and per
workload, and the worst room.

## Run it

Ask a running server for its health line:

```shell
coax/web/room/bin/room --port 8987 &
curl -s localhost:8987/health
```

An idle server, two seconds after it starts, answers:

```text
ok 0 players, fanout p50 -1 us p95 -1 us over 0 reads, 0 rate-limited, 0 slow closed, shard 1, everglade tick p50 100 us at the 95th 100 us over 61 ticks
```

The fan-out is −1 because no socket has been read yet. The tick figures come
from 0.1 ms buckets.

## What is left

- Qualify each workload against its current tick, bandwidth and queue-age
  bounds. The counters above exist; their presence alone does not prove a
  deployment can sustain a larger room or more shards.

## Acceptance checks

- During a 60-raider raid with 16 seats, the admin view shows tick p95,
  bytes per seat per second and the slowest socket's queue age, and each is
  within its stated bound.
- A bot throttled to 64 kbit/s shows as the oldest queue, by name.

## Dependencies

Chapters 86 (the server) and 92 (the budget it measures).
