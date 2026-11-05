# Current room capacity qualification (#12843)

## Instrumentation and harness qualification

This increment adds source-labeled fleet and network qualification, exact
client input-to-final-whole-D ACK timing, simultaneous replicated population
peaks and actual pending-input age. The fresh room suite passes **662/662**.
The native HTTP projection compiles against its actual emitted header.
No gameplay cap, clock-credit, cast or overload policy is changed.

`capacity-profile.coax` has **31/31** killed mutants; `capacity-client.coax`
has **72/72** killed mutants, zero survivors and no equivalent exclusions.
The raid consumer passes **230/230**, proof driver **22/22**, summary **7/7**;
tools-check reports **1796 tools / 0 missing reasons**. New checks first
failed for the fleet shape, exact sequence timing, input-age reader, distinct
identities, shaped-link routing, skew selector and summary parser. The first
timing mutation run exposed untested ring/clock/histogram endpoints; adding
those checks killed the remaining mutants. Syntax errors are not counted as
behavioral red tests. No budget is raised.

### Scope of these physical smoke runs

The native image is the already-qualified **9b1abcd354** room component plus
the unchanged source-keyed 0.48 world component, SHA-256
`19456fe570906a245016d4a71d83f346b1005b34c9acf9ccfff1d7527b65ff21`.
It is deliberately labeled **component harness smoke**, not the newer
integrated game or its supported capacity envelope. The newer main changes
work-budget sharing and world behavior, so this image is not relabeled.

Apple M5 Max, 18 logical CPUs, 128 GiB, macOS 26.4; native `-O2` strict math,
Apple Clang 21. Tools/consumers use checked Coax 0.50 image
`b5872254e02de4b2fb84f039e81f602090f5526301c036c077004294a7e580eb`, with
its genuine original compiler receipt scoped to the captured pre-199402193d
compiler inputs. Everything runs nice 15 on a shared loaded machine. No GPU
or browser renderer participates in these native-socket smoke measurements.
Seed **1284301**, local development stand-in, default raid dials, mixed
druids/warlocks, actual moving/fighting consumers and town destruction.
Persistence throughput and production placement are not measured here.

Raw reports are in [room-capacity-2026-11-05/](room-capacity-2026-11-05/).

| Physical case | Outcome |
| --- | --- |
| Two towns, 16 seats each, initial 60 s smoke | 30 D frames/s, no malformed entries; failed because one town earned no XP in this short interval, and the sampler asked for a window before publication |
| Two towns, 16 seats each, 160 s, 33 Hz seat-zero input, actual 400 ms SIGSTOP/SIGCONT | **Pass**, 164.983 s, 31 complete retained windows; both towns 29 D frames/s; all 32 seats earn XP; 16/16 and 15/16 reach level two |
| One town, 16 seats, 160 s, 100 ms RTT / 30 ms jitter / 20 permille ordered read loss | 30 D frames/s, all 16 earn XP; optional all-level-two assertion **fails at 15/16**; 33 complete retained windows |

The short-run sampler now starts after the first 10 s report is available.
The final fleet driver also starts the raid only after all requested players
appear in a retained report. The physical stall smoke precedes this final
START barrier adjustment; its stall itself already used the all-player
barrier at **31.028 s**, with both signal effects successful.

The stall/skew towns offered **480 extra inputs each** from seat zero.
Input-to-final-ACK p50/p95/p99/max were **35/133/168/400 ms** and
**14/113/148/445 ms**. Both have zero missing/backward ACK samples, zero
malformed entries and zero missed XP cues. Each observed **31 simultaneously
live raiders**, up to **8 replicated zones**, and **142 / 139 town breaks**.
No player summons were instructed in these cases. Cast attempts/refusals and
each seat's XP remain in the raw JSON, including deaths and other refusals;
the consumer's pass is not an assertion that every cast succeeded.

The shaped case measured input-to-final-ACK **124/258/326 ms** at
p50/p95/p99, with zero missing/backward samples. Three existing six-socket
proxies used seeds **1284301 / 1284302 / 1284303**. Their actual loss/read
receipts are retained. These samples exercise the new harness; they do not
establish the current build's network or phone envelope.

## Rerun commands

Run each from an owned worktree with a completed, source-identified native
image. Set `RAID_BUILD`, `RAID_SEED=1284301`, `RAID_VITALS_LOG` and
`RAID_PROOF_LOG` to distinct output files. Use nice 15 and bounded build jobs.

```
coax/web/tool.sh coax/web/room/room-proof.coax fleet BIN 2 160 61261
```

Add `RAID_SKEW=1 RAID_STALL_MS=400` for the recorded stall/skew injection.
For the encounter, set `RAID_CLASS=mix RAID_MODE=fight` and the explicit
network profile; `RAID_LEVEL2=1` requests the extra combat-outcome assertion:

```
RAID_NET=100/30/20 coax/web/tool.sh coax/web/room/room-proof.coax raid BIN 16 160 61271
coax/web/tool.sh coax/web/room/capacity-summary.coax VITALS.log
```

All native ports and adjacent proxy ports above are permitted. Network
randomness is fixed for a given ordered read sequence, not operating-system
TCP boundaries. Summaries show extrema of sampled window values, not
whole-run percentiles. Missing metrics are unknown, including the new input
age metrics on the older smoke image. Retain failed outcomes alongside passes.

## Remaining integrated acceptance

Fresh all-0.50 room/world native and normal auto shared-renderer page builds
are in progress on captured main plus this instrumentation. The Mac world
emission was stopped before 15 minutes; current emission continues on the
owned coderos-4080 worktree with genuine source-bound caches and no changed
compiler gates. This record does not claim those unfinished artifacts passed.

Next: current E8/E16 and 1/2/4/8/16-town fighting curves; controlled
raider/pet/effect/area/destruction axes; current local/100/250 ms, clock skew
and host stalls; actual client presentation/frame timing under explicit phone
CPU constraints; routed multi-process placement/recovery; supported envelope
and early refusal/proof of the separately proposed 32-player encounter.
The old fae22 raid record remains historical. The prerequisite fault and
presentation-drift issues retain their own completed, source-scoped evidence.
