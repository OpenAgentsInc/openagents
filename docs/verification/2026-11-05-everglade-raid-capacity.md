# Fighting encounter baseline, first part of #12843

Two native socket runs of
[`raid-capacity-v1`](../verse/2026-11-05-everglade-capacity-workloads.md):
8 and 16 mixed druids/warlocks fighting in one town for 160 seconds each,
same seed **1284301**, default siege dials, local dev world, town damage on.
No network faults, browser renderer or API persistence. Runs are sequential.

## Source and environment

- Room artifact: **fae22e7101**, the successful standard native build from
  the interest-vitals work. SHA-256:
  `04339e37ddc7147c093b2aa64c34a56c89fd5fb3fbe6b434f912c6b9388d6140`.
- Harness: **09c18ba3df plus this room-proof patch**. Room/simulation source
  was unchanged between fae22e7101 and the start of these measurements.
- Coax **coax0 0.48.0**, compiler executable SHA-256
  `cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`;
  Apple Clang **21.0.0**, native room **-O2**, strict room-math flags from
  `coax/web/room/build.sh`. No fresh compiler-release qualification is implied.
- Apple **M5 Max**, **18 logical CPUs**, **128 GiB RAM**, macOS **26.4**
  (**25E246**, arm64). All our tools/processes run at nice **15**, with
  **one** Coax build/check worker. The machine is shared and loaded: load
  averages immediately before E8 **12.32 / 19.08 / 27.84**, before E16
  **21.86 / 20.48 / 26.83**. These are not isolated CPU-envelope measurements.
- Native loopback port **8955**, wire **4** with the production interest
  selector. No injected latency/jitter/loss. Both runs exit **0**, `raids`.

**Freshness limit:** while these runs were in progress, **990b014e4a** landed
new room travel state and a hello hook. These measurements do not qualify
that newer native room or its changed structure/layout. Keep their fae22
identity. No newer-source binary is substituted or labeled with these numbers.

## Results

The harness now retains raw `/api/room/vitals` reports with monotonic sample
times: **31 samples per run**. They are latest **10 s windows**, sampled
every **5 s**, so windows may repeat. The table reports **maxima of window
values/percentiles**, not whole-run percentiles. `complete=true` means the
retained reports fit the log and were written; it does not assert continuous
coverage of every instant or completeness of client presentation.

| Observation | E8 | E16 |
| --- | ---: | ---: |
| Players in every retained server window | 8 | 16 |
| Server tick Hz range | 29.9–30.1 | 30.0–30.1 |
| Tick p50, largest window (ms) | 0.52 | 0.96 |
| Tick p95, largest window (ms) | 1.50 | 1.36 |
| Tick p99, largest window (ms) | 2.06 | 1.58 |
| Serialization p95, largest window (ms) | 0.41 | 0.34 |
| Serialization p99, largest window (ms) | 0.42 | 0.37 |
| Sending p95, largest window (ms) | 0.10 | 0.09 |
| Host input-to-ACK p95, largest window (ms) | 30 | 26 |
| Sampled queued bytes / output age (ms), largest | 0 / 0 | 0 / 0 |
| Whole picks at the largest seat, most sampled | 95 | 105 |
| Health-only / blocked / hidden picks, most sampled | 0 / 0 / 0 | 0 / 0 / 0 |
| Rows left for later, most sampled | 47 | 52 |
| Changed-row wait, most sampled (ticks) | 8 | 17 |
| Expendable cues dropped/s, largest window | 4.8 | 11.2 |
| Outcomes lost/s, largest sampled window | 0 | 0 |
| Server output bytes/s, largest window | 250,504 | 520,294 |
| Measurement share of tick, largest window (%) | 0.53 | 0.58 |
| Slowest seat's received D Hz, whole checker run | 30 | 30 |
| Received frame-text bytes/s/player, checker mean | 25,601 | 28,148 |
| Player casts sent by checker | 1,065 | 1,971 |
| Refusals heard: dead / other / busy / slot | 618 / 81 / 0 / 0 | 791 / 128 / 0 / 0 |
| Seats earning XP / reaching level 2 | 8 / 8 | 16 / 16 |
| Town breaks heard, largest seat count | 151 | 147 |
| Malformed entries / missed XP cues | 0 / 0 | 0 / 0 |

The minute reports show zero rate-limit frames, full queues, slow sockets
closed and events with no room. Their maximum bytes waiting are **2,750**
and **3,179** respectively; that lifetime/minute measure differs from the
10 s queue samples above. The separate `/health` polling reports a worst
tick p95 of **1,400 us** for both runs; do not replace the vitals-window
maxima with that different measurement.

These are real fights, with raider casts, player hits, rewards and town
breakage heard by the consumers. The checker has unclassified `other`
refusals; they are not assumed to be overload-free. Its cumulative
`least_raiders_seen` fields are **253 / 250**, not simultaneous population
counts. The workload does not measure peak live raiders, pets, sustained
effects or concurrent area spells. Lower observed p95 at E16 does not imply
negative scaling: these are one run each on a shared host, with different
fight histories and contention.

Raw evidence:

- [E8 checker and room summary](data/everglade-raid-2026-11-05/eight-proof.log)
  and [vitals windows](data/everglade-raid-2026-11-05/eight-vitals.txt).
- [E16 checker and room summary](data/everglade-raid-2026-11-05/sixteen-proof.log)
  and [vitals windows](data/everglade-raid-2026-11-05/sixteen-vitals.txt).

## Rerun and checks

Build and label the native room being measured. Each command runs one
160 s case; leave the other case stopped. Choose an unused allowed port.

```
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 RAID_CLASS=mix RAID_MODE=fight RAID_EXPECT=any RAID_SEED=1284301 RAID_BUILD=SOURCE RAID_VITALS_LOG=/tmp/eight-vitals.txt RAID_PROOF_LOG=/tmp/eight-room.log coax/web/tool.sh coax/web/room/room-proof.coax raid /absolute/path/to/room 8 160 8955
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 RAID_CLASS=mix RAID_MODE=fight RAID_EXPECT=any RAID_SEED=1284301 RAID_BUILD=SOURCE RAID_VITALS_LOG=/tmp/sixteen-vitals.txt RAID_PROOF_LOG=/tmp/sixteen-room.log coax/web/tool.sh coax/web/room/room-proof.coax raid /absolute/path/to/room 16 160 8955
```

Harness suite: **21/21**. New checks were written first and caught the
initial seed parser's inherited nine-digit limit. The final parser accepts
the stated positive signed-32-bit range and rejects empty/invalid explicit
seeds. Report checks cover incomplete/non-200 data, sample stamps and
whole-record refusal when the bounded log is full.

The requested full room suite passes **545/545** on the measured source
plus this harness change, before 990b014e4a landed. The simulation/worker
is untouched by this patch; reuse the previously qualified fae22 worker
size **1,004,062 / 1,060,000 bytes**, without claiming the newer travel
worker's size. No API product source changes, no deployment/release script,
no production database or visible browser. `tools-check` still reports
the seven unrelated missing headers named in the fault-slices record.

## Remaining acceptance

#12843 stays open. These two cases establish a source-pinned local encounter
baseline, not a supported fleet or phone/browser envelope. The newer travel
build, integrated checkpoint/overload/fault prerequisites, multi-town and
routed multi-process curves, controlled pet/effect/area/destruction axes,
clock skew/stalls and movement/cast fairness, 100/250 ms network cases,
oldest input/output ages, action-to-presentation and recovery still need
qualification. Server ACK starts after input reaches the host; socket
receipt is not browser presentation. The workload document specifies a
separate proposed 32-player same-town experiment without raising admission.
