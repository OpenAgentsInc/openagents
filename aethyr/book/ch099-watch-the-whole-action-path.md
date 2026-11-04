# 99. Watch the whole action path

> **Status: partly implemented.** oa-room's `/health` reports the players,
> the time to handle one socket's read (p50 and p95), frames the rate limits
> dropped, slow sockets closed, the shard, and the Everglade tick's p50 and
> p95 in 0.1 ms buckets (`room-host.c`). Its minute log line splits the
> tick's time by part (move, abilities, town, spells, people, end, send) and
> counts events a tick dropped. Bytes by message type, queue age, relevant
> entities per seat, join bursts and input-to-acknowledgement time on the
> server are #12791.

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

| Stage | Measure | Today |
| --- | --- | --- |
| Input to acknowledgement | ms from an `I` being heard to its `ACK` going out; the socket's round trip from `TCP_INFO` | client side only, in `load.coax` |
| Server queue | inputs queued per seat; owed steps | not reported |
| Simulation | tick time p50 and p95 by workload (players, raiders, effects) | tick p50 and p95; time by part per minute |
| Serialization | time in the send loop, apart from the rules | inside `end` |
| Replication queue | bytes and age of each socket's queue | peak bytes waiting, slow closed |
| Network out | bytes and messages by message type; `s` rows by id range | not reported |
| Interest | relevant entities per seat | not reported |
| Join | bytes from a join to its first whole state | not reported |
| Reconnect | rejoins and how many came back to their seat | not reported |

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

- Every "not reported" row above, on the admin room view, with the launch
  runbook naming the numbers to watch during a raid (#12791).

## Acceptance checks

- During a 60-raider raid with 16 seats, the admin view shows tick p95,
  bytes per seat per second and the slowest socket's queue age, and each is
  within its stated bound.
- A bot throttled to 64 kbit/s shows as the oldest queue, by name.

## Dependencies

Chapters 86 (the server) and 92 (the budget it measures).
