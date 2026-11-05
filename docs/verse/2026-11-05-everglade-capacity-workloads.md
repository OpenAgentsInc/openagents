# Current Everglade capacity workloads (#12843)

An encounter and a fleet have different limits. Sixteen seats in each of
sixteen independent towns is 256 players, not one 256-player fight. The
historical #12717 result (`6a846432d2`, movement in 16 rooms) and the
2026-11-03 raid numbers remain historical evidence for their own sources
and workloads. Neither qualifies today's integrated workload.

## First encounter baseline: raid-capacity-v1

| Axis | Baseline |
| --- | --- |
| Processes / live towns | One native process / one town |
| Seats | E8: 8; E16: 16, in the same encounter |
| Players | Alternating level-1 druids and warlocks, `RAID_CLASS=mix` |
| Actions | `RAID_MODE=fight`: move through the South Gate toward raiders, use the existing level-1 kit, keep sending movement and HEARD acknowledgments |
| Raid | The harness's default `event 1 0 3 12 6 1 3 3 31 1 2 1 100 15`, scaled by the existing siege rules; START NOW at checker +(3 + seats/2) s |
| Duration / seed | 160 s of checker gameplay, `RAID_SEED=1284301` |
| Pets / sustained spells | No instructed player summons or sustained spells. Incidental enemy/environment effects are possible; their peak counts are not yet measured |
| Area spells / destruction | The existing kit can use Ice Knife. Town damage is enabled by the dials, but these cases do not prescribe concurrent area-spell or destruction counts |
| Network / consumer | Loopback native sockets; no injected latency, jitter or loss; wire 4 using the current production interest selector; no browser renderer |
| Persistence | Local dev world/control/key; no API persistence or production routes |

Use the Coax `room-proof.coax raid` harness. `RAID_BUILD` names the source
of the exact native artifact, `RAID_SEED` fixes its room seed, and
`RAID_VITALS_LOG` retains complete raw server windows with monotonic sample
times. Sampling every 5 s may repeat the latest 10 s report. Maxima of
window percentiles must be described as such, not as percentiles of all
ticks in the whole run. The room output and the checker JSON are retained
beside these samples. The harness refuses more than 16 requested seats
rather than letting the checker silently clamp them to 16.

The checker verdict requires a raid, heard raider casts and player hits,
earned XP, valid messages and more than 20 D frames/s at the slowest seat.
That verdict alone is not a latency or supported-capacity acceptance gate.
Server input-to-ACK starts when the host hears input; it does not measure
the action's network transit or its presentation in a browser.

Local results and rerun commands belong in
[`docs/verification/2026-11-05-everglade-raid-capacity.md`](../verification/2026-11-05-everglade-raid-capacity.md).

## Independent fleet and gameplay curves still to qualify

The next fleet curve is **1, 2, 4, 8, 16 towns per native process**, each
with 16 fighting players using the same encounter recipe. A separate
multi-process curve must include actual directory placement, cold joins,
independent room overload and recovery. Record the configured shard/room
caps and the observed placements for each run; do not infer them from a
historical deployment or change production to collect this evidence.

Cross those workloads with controlled raider, player-pet, persistent-effect,
simultaneous area-spell and destruction counts, with actual observed peaks.
Movement-only `load.coax` remains useful but is not a substitute for these
fighting workloads. Each command stays below 15 minutes, and already
passing source-identical evidence is reused.

Network variants are local and 100/250 ms injected RTT, each with its stated
jitter/loss and socket versus actual-browser consumer. Qualify current
movement credit/four-extra-input behavior, heading/cast fairness, skew and
host stalls. Phone CPU and browser frame/presentation cost need real client
measurements. The vitals, overload, checkpoint and fault prerequisites keep
their own uncompleted gates; fast ticks cannot stand in for them.

## Proposed larger encounter, not yet admitted

Use **32 players in one town** as the next explicit density experiment:
16 druids and 16 warlocks, up to 60 live raiders, one player pet each,
16 persistent effects, eight simultaneous area spells and active town
destruction, for separate 160 s network slices. Count actual peaks and
record refused/deferred work and action/presentation ages.

This is a proposed workload, not supported capacity. The current 16-seat
room and harness reject it; no admission cap is raised here. Prove this
case early in a dedicated prototype before choosing partitioning or
changing gameplay. Current E8/E16 results cannot establish it.
