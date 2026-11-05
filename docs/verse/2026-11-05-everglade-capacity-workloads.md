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

## Current qualification harness

`room-proof.coax fleet BIN TOWNS SECONDS PORT [SEATS_PER_TOWN]` runs the same
mixed-class fighting consumer in **1, 2, 4, 8 or 16 distinct towns** within
one actual native process. It requires `RAID_BUILD`, `RAID_SEED` and
`RAID_VITALS_LOG`; `RAID_PROOF_LOG` keeps the native process log. Every town
has distinct character identities and its own client process, with at most
sixteen SDK sockets each. START NOW waits for admission. The default fleet
duration is 160 s; each command ends before 15 minutes. This is local
stand-in persistence, not a measured Postgres or production-shard capacity.

The raid consumer now measures exact input creation through a received final
whole-D ACK, retaining missing ring generations and backward clock samples as
unknown. Its p50/p95/p99 and maximum are millisecond histogram values; the
last bucket means **2000 ms or longer**. It also counts simultaneous live
raiders, live player pets and zones in each seat's actual replication mirror.
Those are observed replication peaks, not global populations or inferred
simultaneous counts from cumulative entity sightings.

For a sixteen-seat networked encounter, `room-proof.coax raid` accepts
`RAID_NET=100/30/20` or `250/30/20`. It distributes the actual sockets across
three existing six-socket proxies rather than increasing their SDK limit.
All proxy ports are adjacent to the explicitly supplied room port, and each
proxy receives a recorded seed. The proxy models ordered TCP read holds and
retransmission; fixed seeds reproduce choices for the same read sequence,
not operating-system packet boundaries.

`RAID_SKEW=1` offers seat zero one extra input every ten 30 Hz client ticks
(33 Hz); other seats continue at 30 Hz. `RAID_STALL_MS=400` in fleet mode
pauses only the process started by this harness after all requested players
are present in a retained vitals report and 30 s have elapsed, then resumes
it. The injection reports its actual barrier, elapsed time and signal results.
The server's existing clock-credit, four-extra-input, fine-heading and cast
fairness checks remain the correctness oracle; workload measurements report
their actual ACK tails, drops, refusals and recovery.

Native vitals add pending input counts and oldest pending monotonic age at
report time. These are latest gauges, not maxima between report windows.
Unknown backward ages remain `-1`. `capacity-summary.coax VITALS.log` reports
minima/maxima of sampled window values without converting absent or unknown
metrics into zero or treating window percentiles as whole-run percentiles.

The optional `RAID_LEVEL2=1` assertion is a stricter combat outcome test:
everyone must reach level two. Keep that verdict alongside performance
measurements. Earning XP and preserving progression do not imply identical
levels for every participant at a fixed time under randomized combat.

The integrated prerequisite issues are now closed, including the
[fault matrix](../verification/2026-11-05-mmo-fault-matrix.md). Current capacity
still requires the exact current artifact and consumer profiles below; older
fae22 and 9b1 component smoke runs retain their own identities.

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
