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

## Current integrated artifacts

The normal all-0.50 native room build **passes**, including its physical
startup-default check. Product source is frozen at **85cfc9d1b185**, plus
580e2b853f's read-only `room-observe.coax` / `room-stats.h` instrumentation.
No gameplay policy changes during measurement. The room suite's **662/662**
result above applies to this product source and is reused unchanged.

Linux binary SHA-256 **158b40f4e1039284a4670ada82c2aaffca9d541443457fd4fea92db9b3394f63**;
genuine checked Coax 0.50 **5cc82481bf6db529e919a404134b6d18b02c984f443809af7426767af869af9c**.
Intel i7-14700K, 28 logical CPUs, 125 GiB reported total RAM, NixOS
Linux 6.18.49 on coderos-4080. Native compilation uses the standard room
build's `-O2`, strict floating point, shared Coax math and ordered export
ownership. All runs are nice 15 on a shared machine, with one Coax worker.

The same six checked portable C/H units link on the Mac with the standard
room host/world/catalog/math/WS sources and exact build flags. Mac binary
SHA-256 **90dc184cb1d3baee9cefb416043bda886b2793134bc15aaf24e4e2724be7d10a**;
startup defaults exact, no forbidden C-library simulation math references.
This reuses completed producer emissions; it neither skips their checks nor
claims a new compiler bootstrap. Original compiler receipts retain their
captured compiler-input scope, preceding later compiler source edits.

The normal current **auto shared-renderer** page also passes, including its
strict export audit and 1320 carrier checks. Release module **2,012,780 B**,
SHA-256 **beb074630e69ea6c13481a59abc4a42594bc4177ebfff4fc64cef5b59d6bb744**;
518.654 s total. Its genuine checked tool compiler is the Mac b5872254…
image identified above. The actual emitted carrier's 231-source join to
85cfc9d1b185 and native/Wasm buffer parity are recorded independently in
the #12868 current-producer receipt. Later SDK-only files outside that
captured emission are not retroactively claimed as its inputs.

## Current encounter and fleet measurements

Raw current receipts are in [current/](room-capacity-2026-11-05/current/).
Each uses the same mixed-class fighting recipe, seed 1284301 (next town
adds one), 160 s of client gameplay, local stand-in persistence and no
injected network fault. Every requested player is visible in a retained
report before START NOW. Sampled windows may repeat: these are **maxima of
window percentiles**, never pooled whole-run quantiles.

| Case | Duration incl. orchestration | Largest window tick p50/p95/p99 ms | Received D Hz | Input→final-ACK p50/p95/p99/max ms |
| --- | --- | --- | --- | --- |
| E8, one town / 8 seats | 162.606 s | 1.57 / 2.14 / 2.49 | 30 | 14 / 31 / 31 / 33 |
| E16, one town / 16 seats | 165.077 s | 2.94 / 4.79 / 7.26 | 30 | 15 / 31 / 32 / 34 |
| F2, two towns / 32 seats | 165.079 s | 5.34 / 8.41 / 12.78 | 30 | See per-town JSON |

All three pass the existing gameplay consumer. E8 and E16 have zero missing
or backward ACK samples, malformed entries, missed XP cues, transport frame
refusals or sampled lost outcomes; all 8 / 16 seats earn XP. Level-two
counts are 8 / 14; the optional all-level-two assertion was not requested.
Both see 31 simultaneous live raiders; replicated zone peaks are 4 / 8;
no player summons are instructed. They hear 143 destruction events each.
Actual process output peaks at 247,372 / 507,802 B/s. Relevant whole picks
peak at 91 / 101. Latest deferred rows peak at 233 / 471 and row wait at
8 / 13 ticks. Expendable cue drops peak at 0.8 / 9.6 per second; these
remain visible limits, not hidden by the gameplay verdict.

Latest pending-input and socket-output age gauges are zero at these 31
sample points. That is not a claim of zero queueing between reports: host
input→queued-ACK p95 reaches 32 ms. Writing p95 peaks at 0.45 / 1.04 ms,
writing p99 at 0.53 / 2.78 ms, sending p95 at 0.16 / 0.53 ms.

## Actual current browser observations

Apple M5 Max / macOS 26.4, Chrome 156.0.8078.80, actual WebGPU Metal adapter, fallback
false; headless, muted and fresh owned profiles. Fifteen real mixed-class
fighting bot sockets share the native Mac town with the browser. Actual
reports retain 16 seats. The initial desktop run precedes raid START and is
retained separately as the renderer/session baseline. The phone run and
`desktop-fight` run occur after START; the final bot receipt passes, sees
31 live raiders and 8 zones, and all fifteen bots earn XP.

`capacity-browser.coax` retains raw frame rows separately from all 50 actual
latency observations, checks movement and actual ACK/whole state, then
closes its own Chrome. The shared loopback target guard is authority-checked:
**44/44 mutants killed**, no equivalents. Browser dependencies pass 136
checks. Phone means **390×844, DPR 3, CDP CPU throttle 4** on this Mac; it is
an explicit CPU/viewport proxy, not measured physical phone hardware.

| Case | Submitted frames / retained rows | Submitted fps | Wasm / encode / CPU p95 ms | Input→GPU completion p50/p95/max ms |
| --- | --- | --- | --- | --- |
| Desktop baseline, 960×397 actual canvas | 600 / 600 | 60 | 0.765 / 0.475 / 1.315 | 29.586 / 32.559 / 34.719 |
| Phone CPU proxy during raid, 390×844 | 600 / 600 | 60 | 3.285 / 2.430 / 5.480 | 58.175 / 69.836 / 486.165 |
| Desktop during raid, 960×397 | 600 / 600 | 60 | 2.025 / 0.920 / 3.040 | 50.810 / 57.440 / 66.295 |

No skipped submitted frames, page errors or observer missing/refused/expired
rows in these captures; maximum two GPU submissions pending, zero retained
staging bytes. The phone maximum includes an initial high-latency sample and
is retained, not discarded as warmup. Known clock intervals are 50 / 50 / 49
of 50; uncertainty and the unknown row stay in the raw records. GPU
completion measures neither physical scanout nor interpolation: the actual
interpolation delay is separately 100 ms. No 120 Hz claim follows from this
headless 60 Hz compositor.

Rerun with owned local servers and the matching release page / native image:

```
coax/web/tool.sh coax/web/room/capacity-browser.coax 'http://127.0.0.1:61252/realms/everglade/?relay=ws://127.0.0.1:61251&room=pair-raid-1284301&roomtrace=1' observation.json frames.json phone
coax/web/tool.sh apps/aethyrforge-desktop/frame-profile.coax frames.json
coax/web/tool.sh coax/web/room/latency-summary.coax observation.json
```

## Remaining integrated acceptance

Continuing F4/F8/F16, controlled caster/raider/pet/effect/destruction points,
current 100/250-ms and stall/skew, routed placement/recovery and the
supported envelope / separately proposed 32-player target refusal. The old
fae22 raid record remains historical. The prerequisite fault and drift
issues retain their own completed, source-scoped evidence.
