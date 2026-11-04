# Rooms

The Everglade rooms' vitals, for each room server (shard) that reported in the last 10 minutes. Each number shows its latest value and a sparkline of the last 10 minutes. The page refreshes every 10 seconds.

- **Tick:** ticks a second, and the tick's time at p50, p95 and p99, in all and by workload: players, raiders, effects, folk, writing (serialization) and sending. Also the share of the tick spent measuring.
- **Players:** players, the longest send queue in bytes and in age, relevant entities a seat, input to acknowledgement at p50 and p95, joins and their initial-state burst in bytes and milliseconds, rejoins tried and seated, and joins refused by reason.
- **Bytes:** bytes and messages a second, out and in, by message type.
- **Seats:** each seat's room, queue, relevant entities, input to acknowledgement and round trip.

Admins only, with the same login as [Analytics](/admin/analytics). Anyone else sees nothing. What to watch during a raid, and the alarm thresholds, are in [the launch runbook](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/2026-11-04-everglade-launch-runbook.md).
