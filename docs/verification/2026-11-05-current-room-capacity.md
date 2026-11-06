# Current room capacity qualification (#12843)

The source-scoped native envelope is four independent16-seat fighting
towns (64 players) on the recorded hardware. Sixteen towns/256 players pass
as a functional stress point with explicit input drops and reduced tick
margin. The16-seat encounter and renderer/network limits are qualified
separately below; no production or physical-phone capacity is inferred.

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
| F4, four towns / 64 seats | 166.091 s | 10.44 / 12.98 / 14.55 | 30 | Town maxima 38 / 43 / 44 / 54 |
| F8, eight towns / 128 seats | 167.151 s | 20.67 / 26.06 / 32.60 | 29 | Town maxima 42 / 53 / 56 / 75 |
| F16, sixteen towns / 256 seats | 167.308 s | 23.13 / 29.85 / 33.81 | 28 | p95 town max 60; max 74 |

All listed cases pass the existing gameplay consumer. E8 and E16 have zero missing
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

F4/F8/F16 all pass their real gameplay consumers. F8 earns XP at 123/128
seats and F16 at 245/256; each town earns XP, and the remaining seats are
retained in the raw outcomes. These verdicts do not assert equal combat
outcomes for all players. No missing ACK samples or lost outcomes are
observed. F4/F8/F16 process output peaks at **2,050,541 / 4,100,687 /
8,203,685 B/s**, relevant whole picks at **106 / 108 / 107** per seat,
latest deferred rows at **938 / 2,115 / 4,796**, and row wait at **16 / 17 /
17 ticks**. Writing p95 peaks at **3.48 / 7.26 / 9.22 ms**. The oldest
pending-input gauge reaches **11 / 36 / 19 ms**. Input credit drops are
**0 / 1,024 / 8,518**; expendable cue drops reach **39.9 / 56.0 / 129.6
per second**. Output queues are empty at the sampled instants; accepted
transport frame refusal remains zero.

These larger reports explicitly omit some bounded per-seat detail rows:
F4 up to54, F8 up to246, F16 up to622 lines. Aggregate metrics remain present,
and all town consumer JSON is retained separately. This is not described as
a complete per-seat server trace or silently interpreted as missing zeroes.

The first F16 attempt is retained as `f16-process-limit-preflight*`: the
orchestrator tried to own a server plus16 child processes in the SDK's16
process slots. Only15 towns/240 players joined, the all-player START barrier
correctly never fired, and the run failed. The repaired driver owns two
eight-client groups; the SDK limit stays unchanged. Its23 checks pass, and
the corrected physical run has **256 actual seated connections /16 live
worlds** before START, all16 town consumers pass.

### Clock skew, host stall and real API recovery

Current F2 with33Hz seat-zero input in each town and one actual400ms
SIGSTOP/SIGCONT passes in **165.009s**, with31 retained windows. Each offers
480 extra inputs. The raw injection records both successful effects after
the all-player barrier. The first Linux injection preflight correctly fails:
NixOS has no `/bin/kill`; the driver now resolves `kill` through PATH, as
the platform does, without shell interpolation or signaling unrelated PIDs.
That failed effect is not counted as an executed stall.

The corrected stall towns receive29DHz, with ACK p50/p95/p99/max
**31/131/165/398ms** and **15/115/149/381ms**, no missing/backward samples.
All16 /15 seats earnXP; each town earnsXP. Largest window tick p50/p95/p99
is3.12/5.09/6.31ms. Pending input age reaches146ms, per-seat pending5,
credit drops875; output queue bytes/age are zero at sampled instants.
Those drops are visible enforcement under skew/stall, not extra movement
credit being silently granted. Existing source-identical movement-credit,
four-extra-input, fine-heading and cast-fairness checks remain the
correctness oracle; workload ACK measurements alone do not prove fairness.

The normal scratch API check passes with the **current Mac native image**,
including five actual travel assertions: source sends on only after API
acceptance; destination seats and saves the arriving HP/XP after source
crash; destination crash then replacement rejoin/save keeps that state under
the next owner. The physical travel slice takes **43.870s**. Prerequisite
checkpoint/fence/outage and drift evidence retains its source scope and is
reused where unchanged. This is a real two-process recovery integration,
not a claim of production placement or Postgres fleet throughput.

### Separate32-seat same-town proposal

`background-check.coax full BIN 40` tests the existing ceiling in25.044s.
Fifteen parent consumers plus one owned peer are **16 actual seated
connections, zero grace holds**. Distinct candidates17 through32 are all
refused; the native report counts16 full refusals, keeps16 seated, and the
original consumers advance by more than30 frames. The peer exits0. The
early32-seat proposal therefore **fails current admission**; no prototype
capacity, partitioning decision, gameplay change or higher cap is claimed.

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
coax/web/tool.sh coax/web/room/capacity-browser.coax 'http://127.0.0.1:61252/realms/everglade/?relay=ws://localhost:61251&room=pair-raid-1284301&roomtrace=1' observation.json frames.json phone
coax/web/tool.sh apps/aethyrforge-desktop/frame-profile.coax frames.json
coax/web/tool.sh coax/web/room/latency-summary.coax observation.json
```

### Actual shaped phone CPU proxy

The corrected driver records actual WebSocket origins and checks the requested
relay path. Local relays accept the documented `ws://localhost:PORT` spelling;
earlier numeric-address requests fell back to the page proxy and are excluded
from shaped-link acceptance. The older local baseline above remains a valid
page-proxy→native local observation. Another failed capture exposed an
automation error: Enter during a delayed connection opened chat and prevented
movement. The driver now allows one picker key only after5s still idle,
never while connecting, and bounds collection at45s for50 completed traces.

Fifteen bots use `RAID_RUN_FULL=1` to stay for the entire300s fixture even
after raid victory/regroup. During both corrected captures the actual native
minute log records16 players; bot and browser receipts are retained. All
Chrome processes close before the next native GPU measurement.

Both links use30ms jitter/20-permille ordered-read loss, CPU throttle4,
390×844/DPR3, actual hardware Metal, fixed proxy seeds1284301/1284302:

| Browser link | Submitted / retained frames | Submitted fps | Wasm / encode / CPU p95 ms | Completed input→GPU p50/p95/max ms | Completed / expired / missing traces |
| --- | --- | --- | --- | --- | --- |
|100ms RTT |576/600|56.7|5.825 /2.820 /8.665|158.145 /357.704 /558.636|50 /13 /0|
|250ms RTT |537/600|48.5|7.040 /3.085 /10.130|345.615 /772.716 /874.480|50 /0 /0|

Both pass actual movement,50 matched observations, requested socket route
and no page errors.49/50 clock intervals are known in each. The13 expired
100ms traces stay **unknown**: completed-sample quantiles do not bound the
full offered set. No60fps support is claimed for these shaped CPU-proxy
combinations. Bounded GPU admission skips24 /63 frame submissions rather
than growing a queue; maximum pending is2 and retained staging zero.
Proxy final receipts prove traffic:100ms has1,215 reads up/1,263 down/39
lost, maximum hold265ms;250ms has633 up/704 down/12 lost, maximum hold636ms.

Current native-socket100ms/30/20 E16 also passes160s:30DHz, all16 earnXP,
ACK p50/p95/p99/max **133/256/323/520ms**, no missing/backward samples,
33 complete retained windows. Raw cast/death refusals and the15/16 level-two
outcome remain visible. Three six-socket proxies, not a larger SDK socket
table, carry the actual clients.

Current native-socket250ms/30/20 E16 passes the full160s as well:30DHz,
15/16 seats earnXP,11/16 reach level two; no malformed messages, missedXP
cues or missing/backward ACK samples. ACK p50/p95/p99/max is
**332/760/874/1249ms**. It sees31 live raiders,8 zones and184 destruction
event frames.32 complete retained windows have largest tick p50/p95/p99
**3.18/4.02/4.55ms**, writing p950.90ms and sending p950.38ms. Pending
input age peaks113ms, per-seat queued16, credit drops0; sampled output
queue bytes/age are zero. Output peaks510,303B/s, deferred rows614,
row wait34ticks and expendable cue drops16.3/s; lost outcomes0.
The native100ms profile's corresponding pending-input age is40ms,
queued7, credit drops0. Initial/final windows can have fewer players;
the consumer JSON records the requested16-seat encounter throughout its
full gameplay interval. Summary values are window extrema, never pooled.

## Supported envelope for these artifacts

The conservative native envelope is **four independent towns /64 fighting
players in one process**,16 seats each,160s, the defined mixed-class kit,
default raid and local stand-in persistence on the recorded i7-14700K.
The F4 largest window tick p99 is14.55ms against the actual Everglade
**30Hz /33.33ms** simulation budget, receivedD rate30Hz, input-credit
drops0 and sampled lost outcomes0. Per-seat reporting is bounded and
omits up to54 detail lines; this is not a complete per-seat trace.

F8 and F16 are retained successful functional stress points, **not an
unqualified higher service guarantee**: F8 has1024 input-credit drops;
F16 has8518,28DHz and window tick p9933.81ms, slightly over the simulation
budget. Defer/row-wait/cue-drop growth and bounded report omissions are
explicit above. Sixteen full towns are256 players across separate worlds,
not a256-player same-location encounter.

E16 native sockets are qualified separately for local,100/30/20 and
250/30/20 profiles with their measured ACK tails. Desktop and phone CPU
proxy local captures submit60fps; the shaped phone CPU proxy drops to
56.7/48.5fps, so **60fps is not supported for those combinations**. The
unknown expired100ms observations remain excluded from completed-sample
quantiles. Do not multiply independently tested fleet, summons, network
and renderer maxima into a combined capacity claim.

Overload remains bounded: the recorded full-town probe explicitly refuses
new connections; input-credit enforcement drops excess work; relevance
defers rows; expendable cues can drop; GPU admission skips submissions
instead of queueing indefinitely. Cast/death/cooldown/busy refusals remain
in the raw outcomes. Durable acceptance, checkpoints, ownership and
outages retain the completed prerequisite fault records, including the
current real two-process travel recovery above. Local persistence means
these figures **do not qualify Postgres throughput, production placement,
multi-process fleet throughput, physical phones or longer soaks**.

The32-seat same-town target remains unsupported at its actual admission
boundary. Historical fae22/9b1 results keep their original identities.
Future source, compiler, hardware or workload changes require a new
qualification; these receipts do not relabel later main commits.

## Controlled effect points and final checks

All controlled points use the same current Linux native artifact,75s,
ten actual connected gameplay sockets/two towns, loopback and fixed
fixture identities. Eight potential mixed casters plus a slow reader
share the busy town; the tenth player moves in an independent town.
Active casters are2/4/8 and the raid-size dial6/12/24. Actual mirror
population, destruction-event frames and raider-cast frames are measured
over the run. The table's tick/serialization/bytes are the **one final
retained report window**, not maxima over unretained windows.

| Active casters / raid dial | Live pets / zones / raiders observed peak | Casts offered / busy refusals | Break / raider-cast frames | Tick p50/p95/p99 ms | Writing / sending p95 ms | Output B/s |
| --- | --- | --- | --- | --- | --- | --- |
|2 /6|4 /0 /19|308 /59|73 /25|1.57 /2.03 /2.23|0.42 /0.15|188,909|
|4 /12|8 /0 /34|552 /59|70 /16|1.74 /2.71 /3.31|0.48 /0.17|247,899|
|8 /24|8 /4 /37|1040 /58|74 /25|1.92 /3.40 /3.69|0.49 /0.18|279,916|

All three cases pass:150/150 health probes, pending work peaks4/6 then drains
to0, publication completes, all final consumers stay connected, and the
slow reader closes once then rejoins. Its tick advances450→1650. The
independent moving room receives2250 /2251 /2251 frames with maximum gaps
105 /141 /111ms. Final pending input ages are19 /13 /0ms, credit drops0,
socket queue age/bytes0 and lost outcomes0. The final report's relevant
entity maxima are43 /74 /68. Expendable cue drops remain visible (the
eight-caster final window has15.3/s). These do not claim end-to-end presentation for
each effect or count accepted spells from offered commands.

The final tool checks include **238/238** background checks and **231/231**
raid checks, the23/23 fleet driver checks and tools-check **1799/0**.
Normal tool emission also executes its dependencies' checks. The new
profile-specific peak assertion first fails against the unconditional
zone requirement, then passes:2/4 casters do not request Moonbeam, while
8 casters do. The failed two-caster physical receipt is retained as
`preflights/effects2-zone-expectation.log`; actual recovery/isolation passed
there, but the old gate incorrectly required a zone. No game policy or
product invariant changes. Source-identical662 room checks and previously
killed31/72/44 policy mutants are reused from580e/f9ba instead of repeated.

To rerun the qualified curve, set the exact native artifact, compiler and
source label above, `RAID_RUN_FULL=1`, `RAID_CLASS=mix RAID_MODE=fight`,
and distinct raw output paths. Run native fleet sizes1/2/4/8/16 separately,
then the100/250 network variants and stall/skew example above. Run each
caster point and the independent admission probe separately:

```
coax/web/tool.sh coax/web/room/background-check.coax stress BIN 75 2
coax/web/tool.sh coax/web/room/background-check.coax stress BIN 75 4
coax/web/tool.sh coax/web/room/background-check.coax stress BIN 75 8
coax/web/tool.sh coax/web/room/background-check.coax full BIN 40
```

The scratch recovery rerun uses `coax/web/api/test.sh` with
`API_TESTS_ONLY=room_checks.coax ROOM_FAULT_SLICE=travel`,
`ROOM_FAULT_SEED=1284301`, `ROOM_FAULT_BUILD` naming the measured image,
and `ROOM_BIN` pointing to it. No production credentials or deployment
are needed. Every recorded command stays below15minutes.
