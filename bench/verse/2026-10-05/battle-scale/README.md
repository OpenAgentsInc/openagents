# Verse battle capacity evidence

Issue: [V18 / #10637](https://github.com/OpenAgentsInc/openagents/issues/10637).
The final integrated campaign passes all seven stages, including three
consecutive delayed durable combined short runs and a ten-minute combined soak.
Earlier failed campaigns remain retained below. The accepted profile is one
native renderer and nineteen headless workers on one machine.

## Final accepted campaign

`execution-manifest-final-main-repeat.json` pins baseline
`c0611aab1f880ff9184a091eb6c62ccb36658a62`, source patch SHA-256
`9732f2d1042d4b72ec3b832479d6e3e3b89f689859c300173f0859f12c254126`,
battle executable `6136dae2b32b6941b0093327eb6222313bcd6d8f43dfcc0028108020247f8a92`,
and renderer executable `eb809733aff3c32df67d142edfb382a623f60d23f08a5693c366ceff92e7d548`.
All seven receipt hashes and the current source patch verify.

The three sixty-second repeats record 1,793, 1,792, and 1,792 workload ticks,
ordinary correction p95 below 0.214 m, and maximum below 0.641 m. The isolated
renderer, six-hundred-second simulated authority run, and sixty-second delayed
durable network run pass. The ten-minute combined soak records 17,910 workload
ticks, simulation p99 upper bound 28.667 ms, steady CPU frame p95 5.248 ms,
GPU p95 5.772 ms, and snapshot age p95 223.896 ms. Native ordinary correction
maxima are 0.6401 m and 0.4801 m; every late window passes.

The soak's 114 post-thirty-second RSS samples grow from 368,783,360 bytes to a
413,212,672-byte peak: 44,429,312 bytes against the unchanged 64 MiB limit.
Recovery verifies twenty inventories, forty live hostiles, sixty actors, 127
active receipts, ledger revision 787, 512 retained events, and a 446,777-byte
checkpoint. Two actual respawns are accepted. Queue peaks are twenty requests,
two writes, and 911,226 held reply bytes. The final result establishes this
declared profile; the route, seeded progression, device, and operating limits
below remain applicable.

## Workload and declared budgets

`battle_scale` exercises the production authenticated TLS worker, durable host,
and shared native session. Network and combined modes run for wall-clock time:
one native session and nineteen headless workers enroll twenty characters in
one instance, with thirty-nine cultists and one boss. Cultists have 20,000 hit
points to keep the measured crowd active. Each client disconnects and reconnects
halfway through the run. Movement, targeting, casts, area effects, hostile
pursuit, equipment, quest claims, recovery items, and defeated-player respawns
use existing authority paths. Inventory and a completed quest objective are
seeded; this workload does not establish loot drops or quest-giver interaction.

Authority mode uses authenticated in-process dispatch and simulated time.
Its accepted battle movement counters must cover all twenty players. It has
no transport or persistent-storage measurement. Network mode projects the
primary native session without GPU drawing. Combined mode draws that session's
actual scene, mounts, lighting, and HUD offscreen at 1280 × 720 with the low
quality profile. Required actor roots and equipment mounts cannot be culled to
pass the renderer check.

Budgets are encoded in `common/battle_acceptance.rs` before each measured run:

- Authority simulation p99 upper bound: 33.333 ms.
- Combined CPU projection and drawing p95: 16.667 ms; GPU scene p95: 16.667 ms.
- Primary ordinary prediction correction p95: 0.25 m; maximum: 1 m. Explicit
  life, epoch, and teleport discontinuities are recorded separately.
- Applied snapshot age p95: 400 ms, including late lifetime windows.
- Authority throughput: at least 98% of 30 Hz over the declared wall duration.
- Every client completes both segments, observes all twenty players and forty
  live hostiles, binds at least ten movement intervals, confirms at least 240
  interval physics steps, and observes interval movement in at least 80% of
  battle snapshots.
- Aggregate accepted equipment, quest claim, item use, and respawn counts must
  be nonzero. Accepted Fireball, Web, and Thunderwave casts must be nonzero.
- Active reward receipts: at most 128; retained events: at most 512; recovery
  checkpoint: at most 1 MiB. All twenty recovered inventories must match.
- A soak of at least 600 seconds requires at least sixty resident-memory
  observations after thirty seconds and peak growth no greater than 64 MiB
  above the first steady observation.

Missing measurements, empty workloads, omitted route error details,
unscheduled transport errors, failed participants, and authority/storage
failure cannot pass. Read/write queues, profiles, correction diagnostics, and
expiry samples remain bounded and retain omission counters.

## Route and measurement limits

The current scratch proxy uses a bounded pipeline of eight chunks per stream
direction. Each chunk receives 40 ms delay plus 0–20 ms jitter. FIFO delivery
preserves bytes. The proxy's original default serial profile remains available;
its throughput limit differs from the pipelined profile. The two profiles are
reported explicitly and their results are not interchangeable. The route tests
exercise a real scratch TCP connection, multi-megabyte ordered bytes, queue
bounds, and two-way delay.

All participants, the authority, storage, and the renderer share this machine
and process. Request turnaround includes queues and durable server work.
Applied freshness starts at local snapshot verification. Neither measurement
is isolated network RTT. Offscreen GPU timestamps exclude display presentation,
capture, and physical input-to-display latency. The final pending GPU query
slots and bounded profile omissions are reported. Thirty-second windows cover
late soak behavior beyond the first bounded startup/steady sample buffers.

New receipts identify the base revision, executable SHA-256, compiled harness,
drivers, acceptance code, network loop, worker, native session, lockfile,
content, and executed proxy script. Early exploratory pilots lack a complete
revision/executable binding and cannot establish final acceptance. A build or
test running concurrently can affect CPU and disk timing; no result below
relabels that interference as a passing workload.

## Retained exploratory results

Historical three-contact, four-contact, strict-navigation, and native battle
failures remain in their original directories referenced by the audit.
The isolated renderer baseline here passes its own synthetic CPU/GPU budgets;
it does not establish combined battle acceptance.

| Receipt | Outcome and limitation |
| --- | --- |
| `authority-pilot.json` | Earlier authority timing pilot passes its original gates; it predates the accepted movement population gate and complete source binding. |
| `network-pilot.json` | Simultaneous initial authentication exceeds the pending per-IP budget; twelve players fail connection. |
| `network-native-pilot.json` | Twenty local native predictors fail interval workload coverage. |
| `network-handoff-pilot.json` | Serial route and earlier batching produce 791 clock expiries; its original correction gate fails. Later continuity gates are stricter. |
| `network-tick-batch-pilot.json` | Experimental four-step batches on the serial route produce 686 expiries and fail freshness and correction gates. This batching experiment does not ship. |
| `network-pipelined-pilot.json` | Twenty predictors with the pipelined route still produce 395 expiries and fail continuity and correction gates. |
| `network-hybrid-pilot.json` | Current-main six-step batches with one native session and nineteen headless workers recover all twenty inventories and forty hostiles, but 499 expiries and other workload failures prevent acceptance. |
| `network-ordinary-operation-pilot.json` | Keeping ordinary mutations in order improves the primary's interval coverage, but 426 expiries, prediction outliers, and 3.43 seconds of storage pauses fail the gates. |
| `network-queue-backpressure-pilot.json` | The request queue peaks at twenty and storage refusals fall to zero. All clients reconnect and inventories recover, but 418 expiries, prediction outliers, and authority throughput still fail the gates. |
| `network-authority-credit-pilot.json` | Exact response credit alone cannot overcome deferred-read admission delays. Throughput meets its gate, but 605 expiries and interval workload gaps fail acceptance. |
| `network-read-admission-pilot.json` | Capturing reads at admission reduces storage pauses to 0.012 seconds and held replies peak at 1,374,429 bytes. Throughput and ordinary corrections pass, but 496 expiries and every segment's interval coverage prevent acceptance. |
| `network-event-cadence-pilot.json` | The 200 ms event-page cadence passes all network gates: 1,774 authority ticks, all twenty inventory recoveries, and 49 clock expiries. This revision precedes the next main integration and does not establish combined or soak acceptance. |
| `network-final.json` | After integrating faster snapshot scheduling and storage synchronization from main, throughput reaches 1,800 ticks and storage has no pauses, but request pressure produces 342 expiries and interval coverage failures. The execution manifest and original source patch retain this failed revision. |

The recorded three-capsule motor regression, unrelated-player containment
regression, and per-tick catch-up observation regression cover production
failure containment. The mutation regression withholds earlier command
acknowledgments and requires a recovery item to be consumed successfully.
The final `service-backpressure-tests.log` records 204 passing service tests
and two ignored subprocess fixtures. `mixed-operation-fixture-failure.log`
retains an earlier test-fixture failure: the fixture spent mana from the wrong
player, so its recovery item was correctly refused. The corrected regression
passes in the service suite.
The stalled-writer regression requires ordered commands and snapshots to wait
for disk completion while the existing request queue and two write slots bound
backpressure.

The full `authority-credit-world-tests.log` records 509 passing world tests and
two ignored subprocess fixtures for the credit contract. The subsequent
`read-admission-tests.log` records nineteen passing transport tests after reads
capture their immutable admission prefix. Replies remain behind the durable
fence. Pending encoded replies and each fence have a 16 MiB byte budget, with
at most 32 MiB held across the admission queue and two write slots. Read
projection timing covers reads captured while authority mutations await commit;
already committed reads can bypass that timing stage.

`event-cadence-tests.log` records seventeen passing worker tests after event
pages use a 200 ms minimum cadence. Pose replication retains its requested
cadence. Per-request-kind turnaround and pending-window occupancy are included
in subsequent pilots. The network pilot passes; repeated combined and soak acceptance remain required.

## Reproduce the workload

Build the standalone example from the recorded base plus the retained source
patch, or from the closing commit that includes that patch:

```sh
CARGO_TARGET_DIR=~/work/openagents-target-agent1 \
CARGO_BUILD_JOBS=2 \
VERSE_BENCH_SOURCE_REVISION="$(git rev-parse HEAD)" \
cargo build -p verse --no-default-features --features remote-chamber \
  --example battle_scale
```

Run each mode with a scratch home and no desktop surface. Replace the output
path and duration for each receipt. The example creates and removes its own
content, authority state, TLS certificates, and delayed route. Its process exits
with failure when any declared acceptance gate fails.

```sh
scratch_battle_home=$(mktemp -d /tmp/verse-battle-home.XXXXXX)
HOME="$scratch_battle_home" VERSE_QUALITY=low VERSE_GPU_TIMING=1 \
  env -u DISPLAY -u WAYLAND_DISPLAY \
  ~/work/openagents-target-agent1/debug/examples/battle_scale \
  combined /tmp/verse-battle-result.json 60
battle_result=$?
rmdir "$scratch_battle_home"
exit "$battle_result"
```

Use `authority` for simulated in-process timing, `network` for TLS and durable
storage without GPU drawing, and `combined` for the actual native scene and HUD.
Durations from 10 to 3,600 seconds are accepted. A 600-second combined run
activates the steady memory-growth gate. Run builds and tests separately from
capacity measurements; retain a failed receipt before changing the fixture or
implementation.

The snapshot-yield revision retains separate source and execution manifests.
Its `network-yield-final.json` still fails continuity and correction budgets,
with 207 expiries; no accepted respawn occurs in that pilot. Subsequent fixture
parameters place player nineteen at `[-6, 0, -8]` near hostile pursuit to exercise
actual defeat and the normal respawn request. The original failed placement
remains in its receipt and source patch.

Durable replies renew only their numeric world-clock credit from their completed
fence. Their admission identity, sequence, tick, and projected scene retain the
ordered prefix. This credit confirms no character travel and changes no future
or lag limit. The byte budget reserves room for u64 clock growth. Fast snapshots
retain their send-based deadline; slow responses yield one cadence before the
next periodic projection.

`renderer-low-timed.json` passes with GPU timestamps enabled: CPU drawing p95
is 6.09 ms and GPU p95 is 1.54 ms. The `*-retry-*` receipts used the integrated
renderer default (high quality with timestamps disabled); their CPU results do
not establish GPU acceptance. The `combined-low-timed-01.json`,
`combined-low-timed-02.json`, and `combined-low-timed-03.json` receipts use the
explicit low profile and valid timestamps. All three fail ordinary movement
correction gates; two also fail workload continuity or accepted respawn coverage.
Their manifests preserve the exact environment and implementation. No soak has
passed at this revision.

The applied-confirmation revision records actual completed interval movement in
at most sixteen runtime records per actor. Completed durable fences can attach
a record whose life, epoch, and applied sequence fit the response's original
admission prefix. Header confirmations do not rebuild or relabel the scene body.
The native session counts resulting corrections, retires confirmed history,
and preserves newer confirmed travel across older body projections. The
`applied-confirmation-world-tests.log` records 514 passing tests and two existing
ignored subprocess fixtures; subsequent focused checks cover the final delivery
and client validation paths. Authority throughput now counts ticks only inside
the measured workload window. Late correction windows retain the same budgets.

`applied-confirmation-native-tests.log` retains a failed teleport regression:
selecting the prior confirmed pose before checking the teleport stamp could
reactivate prediction after a reset. The correction excludes retained pose
fallbacks across teleport, death, or control changes. The native regression also
delivers actual travel confirmation before an older scene body and requires that
the body neither regresses the confirmation nor suppresses correction evidence.

Final focused confirmation checks pass: six native session tests, twenty-one
transport tests, one client validation regression with incompatible-proof cases,
and three capacity-verdict tests. The capacity-verdict regression also rejects
late correction outliers and insufficient workload-scoped ticks despite a larger
whole-process tick count. The new execution manifest preserves each subsequent
measurement and its executable and source hashes.

`network-applied-confirmation.json` completes all participants and durable
recovery but fails native continuity in its second segment, with 84 expiries.
The first native segment passes ordinary correction limits (p95 0.18 m; maximum
0.49 m); the second fails at p95 0.29 m and maximum 1.22 m. Its manifest and
source patch retain that revision. Reconnect traces show fresh interval clocks
starting behind verified world time. The subsequent bootstrap change aligns
only the initial grounded, neutral interval timeline to verified authority time;
it retains the character pose and encodes the earlier gap as neutral input.
Ordinary credit updates still change permission without advancing the local
clock, and later movement cannot be applied retroactively through that gap.

`network-interval-bootstrap.json` passes all network timing, continuity,
correction, and recovery gates. It fails only accepted respawn coverage:
player nineteen's minimum health is twelve, and its equipment, healing, and
protective casts prevent actual defeat. The next declared frontline recipe
retains its initial position and normal movement and offensive casts, but
postpones its inventory mutations, healing, Shield, and Misty Step until the
first actual respawn. It then rejoins the ordinary mixed recipe. The other
nineteen players retain that recipe throughout. Actor health, NPC behavior,
authority combat, respawn admission, and all acceptance budgets are unchanged.
The fourteen bootstrap prediction tests and six native session tests pass.

`network-frontline-defeat.json` accepts a real respawn and preserves all twenty
inventories and forty live hostiles, but fails two continuity segments and one
ordinary native correction maximum (1.13 m). The next worker change rejects
tracked input from an already retired generation or epoch without requesting a
fresh scene for every queued stale input. Normal binding still rejects each
stale proposal, preserves its token outcome, and grants no new execution. A
bounded-backlog regression queues sixteen stale intervals against a delayed
peer and requires their rejection within 500 ms with at most one initial scene
request. The headless producer also stops proposing old-context intervals after
a newer verified control arrives; scene bodies retain their original prefix.

`retired-input-worker-tests.log` retains the initial backlog test's shutdown
failure: all sixteen stale proposals were rejected, but the scratch peer had
already closed its stop receiver. Cleanup now tolerates that expected closure;
peer writes also stop cleanly when the worker closes its scratch socket.

`network-retired-input.json` passes every gate with eleven clock expiries and
both native segments above 95% interval coverage. Its three combined receipts
pass timing and workload coverage but fail ordinary correction maxima (1.04–1.38
m). The next revision bounds capsule prediction to twelve substeps beyond
confirmed travel or the verified neutral bootstrap interval. The complete input
clock and transmitted intervals continue; confirmation without elapsed time
replays the previous rendered interval without releasing a catch-up burst.
Corrections remain measured from actual unsmoothed poses. Reports also expose
the input-to-prediction clock delay and limited-step counts. This bound can add
local prediction delay on slow routes; it does not establish physical
action-to-display latency or replace the retained device verification work.


`network-bounded-prediction.json` passes ordinary correction budgets but fails
primary interval coverage in its second segment (109 of 138 battle snapshots)
and workload throughput (1,761 ticks; 1,764 required). Actual prediction-clock
delay reaches 358 ms and remains explicit in the receipt. The next handoff
change reuses a verified control no older than 50 ms for interval entry when
its life and epoch match exactly. Entry still drains earlier IO, and server
admission remains authoritative. Missing, old, or mismatched control requires
the normal refresh; respawn retains its separate lifecycle refresh.


`network-handoff-control.json` and the first two combined repeats pass all
gates. The third combined repeat retains a 1.61 m ordinary correction, while
timing, coverage, and recovery pass. Its trace shows a new confirmed capsule
replayed into older crowd geometry; overlap recovery chooses another exit and
moves the estimate away from authority. The next predictor change holds its
pose when a blocking overlap requires recovery. Authority retains every
contact and chooses the exit. Inputs, clocks, and packet proposals continue;
query failures and truncation remain errors. Deferral counts and the strongest
contact diagnostic are reported. No correction is smoothed, capped, or omitted.


Combined repeats two, three, and four of the embedding-deferral revision pass
consecutively on one executable. Its 600-second soak fails at 4,146 authority
ticks, about 138 seconds, with `Hostile projectile position disagrees with its
flight`. The process retains its partial profiles through the declared deadline;
missing recovery, interrupted reconnects, and late freshness failures remain
failures. A late-time, large-coordinate projectile regression reproduces the
same invariant failure before the fix. Canonical swept endpoints now use the
same launch-to-arrival equation as checkpoint validation, preserving collision
ordering and the existing 2 mm validation tolerance. This change needs new
acceptance receipts; the passing short runs do not establish a passing soak.


The first canonical-flight world suite records 518 passing tests, two ignored
fixtures, and two prediction failures: a flat jitter route retains 10.7 cm of
lag, and the delayed TLS fixture retains 54 cm. The global motor horizon
introduced permanent lag even in static scenes. The correction scopes the
100 ms horizon to nearby projected actor capsules. Static terrain retains the
ordinary bounded input history. Catch-up adds at most one extra motor step per
elapsed step and never occurs on a zero-time acknowledgment; entering a crowd
cannot rewind the rendered clock. Existing prediction budgets remain intact.


The crowd-scoped horizon passes all thirty prediction tests but still fails
the delayed TLS fixture around a stationary player (54 cm p95). That horizon
and its catch-up policy do not ship. The final predictor retains the original
bounded input replay and only defers speculative overlap recovery, with every
query error preserved. Input time still comes from local elapsed time; a
zero-time confirmation replays existing history rather than granting time.


`canonical-flight-prediction-replay.log` records thirty passing prediction tests;
`canonical-flight-worker-replay.log` records nineteen passing worker tests,
including the actual delayed TLS fixture. The new completed-confirmation test
requires that a zero-time acknowledgment replays existing time without granting
an interval. The earlier failed fixtures and full-suite failure remain retained.


`authority-canonical-flight.json` passes six hundred simulated seconds, and
its isolated renderer passes. `network-canonical-flight.json` fails only the
ordinary correction maximum (1.35 m). Its trace shows geometry-only refreshes
replaying past blocked input despite an unchanged confirmed motor state. The
next change applies those collider refreshes to future integration. Changed
authority travel still reconciles, and query validation remains mandatory.


The timed-history revision passes isolated rendering, six hundred simulated
authority seconds, the delayed network profile, and all three consecutive
combined sixty-second repeats on one executable. The ordinary correction p95
is at most 0.1601 m and maximum at most 0.6401 m across those repeats. Its
600-second soak fails: all workers stop near authority time 200 seconds with
`Owned HUD life or clock mismatch`, before the scheduled midpoint reconnect.
Authority continues for 17,971 workload ticks and restores twenty characters,
forty live hostiles, sixty actors, sixty-six active receipts, and 512 events.
The primary's first segment retains ordinary correction maximum 0.801 m; the
missing second segment and late freshness windows remain failures.

The cinematic director clamps frame time at the authored scene duration, while
the unlocked authority and owned HUD continue advancing. A new post-cinematic
wire regression covers the exact endpoint and one and five hundred seconds
after it. The correction belongs to live game framing; cinematic playback
retains its bounded duration. The timed-history manifest preserves this failed
soak and its passing short runs; they cannot establish final acceptance.


The final predictor records each processed motor step within the existing
256-step input history. Deferred embedding steps remain held during later
reconciliation. A grounded horizontal confirmation compares authority travel
with the estimate at that exact physics step and applies the real difference
to the already processed pending path. No correction is capped or smoothed.
Changed fixed geometry, movement policy, forces, support, or vertical motion
requires full replay. A wall-overlap check also refuses the translation path.
The motor history has at most 257 states, including the confirmed baseline;
lifecycle changes clear it. The guard regression exercises a wall, an external
force, and a slowing policy independently.

`post-cinematic-clock-regression-before-service.log` reproduces the actual HUD
clock failure. `post-cinematic-main-world-tests.log` records 537 passing tests
and two existing ignored fixtures after the live clock correction. Native
reports now retain the worker's underlying error separately from the enclosing
observation failure. `campaign-post-cinematic.py` records a fresh source patch
and manifest, checks isolated GPU timing, then runs authority, network, three
combined repeats, and a 600-second combined soak. It retains failing receipts
and refuses aggregate acceptance if any stage fails.


The post-cinematic campaign passes authority, network, isolated rendering, and
combined repeats one and three. Repeat two fails only the ordinary correction
p95: 0.3224 m against the unchanged 0.25 m budget; its maximum is 0.6401 m.
The trace shows deferred crowd movement corrected in six- and twelve-step
chunks. The campaign skips the soak when that repeat fails. The native send
regression reproduces a completed four-step history waiting for another wake-up
under the six-step threshold. The subsequent threshold sends at 30 Hz
(four 120 Hz physics steps) while keeping twelve-step catch-up packets, complete
input segments, queue limits, lag limits, and server work limits intact.
The revised native fixture retains direction changes and delayed catch-up.
Fresh acceptance remains required.


The four-step revision integrates main's sequenced teleport send path while
preserving its acknowledgment barrier and the audit's ordered ordinary
mutations. Its merged checks pass 537 world tests (two existing ignored fixtures)
and six native session tests. The first benchmark link fails because the shared
filesystem is full; the retained failure log records `No space left on device`.
The unchanged retry passes after three obsolete, inactive executables are
removed, each identified by a retained execution-manifest hash. Build caches
and other checkouts remain intact.

`execution-manifest-thirty-hz-send.json` binds the current campaign to main base
`94555e0d65e06618137bed77e71f51cb12f5f695` and its retained source patch. Isolated
rendering, six hundred simulated authority seconds, delayed durable networking,
and all three combined sixty-second repeats pass. The required combined
600-second soak remains in progress. No earlier failed campaign is relabeled.


The four-step 600-second soak completes every client segment and exact recovery,
with 17,855 workload ticks, forty live hostiles, twenty inventories, 118 active
receipts, 906 ledger revisions, 512 retained events, and a 440,301-byte restored
checkpoint. It fails the ordinary correction maximum (4.0481 m) in a late native
window and steady whole-process RSS growth (96,997,376 bytes; 64 MiB allowed).
The first-32 correction details fill before the outlier, and 38 later details
are explicitly omitted. A passing percentile cannot conceal that maximum.

The next diagnostic revision keeps the strongest ordinary correction as one
additional bounded record. Completed segment reports are written to the scratch
state directory, checked by SHA-256 and exact length, and loaded after workload
memory sampling. Each record has a 16 MiB limit and at most forty records exist.
This releases completed measurement trees during the second half; it removes
no measurements and still gates whole-process RSS. An encoding, storage, or
integrity failure produces a failed participant. `campaign-peak-diagnostic.py`
runs a 600-second diagnostic soak, explicitly outside aggregate acceptance.
Production prediction, collisions, workload recipes, and acceptance budgets
remain unchanged for that diagnostic.

The peak diagnostic soak finishes all forty client segments and exact recovery,
but rendering refuses animation marker catch-up. It also fails prediction
maximums (3.8572 m and 2.3884 m), a late p95 (0.3200 m), actual respawn coverage,
and whole-process steady RSS growth (82,300,928 bytes). Authority remains live;
recovery retains forty live hostiles, twenty characters, 62 active receipts,
850 ledger revisions, 512 events, and a 425,738-byte checkpoint. This diagnostic
is failed and does not count as acceptance.

The strongest correction places the client at a fixed wall, then replays its
processed path against a later hostile capsule at the confirmed baseline.
`wall-constrained-regression-before.log` reproduces the collapse from 5.6500 m
to 0.7000 m. Fixed-scene reconciliation now collision-constrains each retained
motor state before applying it; changed geometry, policy, support, or forces
still require replay. Future steps retain current actor collisions and query
errors. `wall-constrained-prediction-tests.log` records 35 passing prediction
tests, including the failure and existing refusal guards.

`animation-clock-handoff-before.log` reproduces marker overflow when local
locomotion time hands off to authority time with the same life and animation.
Render values now carry the prediction control epoch as phase ownership;
authority and authored playback use no local epoch. Changing that owner starts
marker delivery from the new source's current time. The original per-advance
marker budget and atomic refusal remain intact. Renderer errors now retain the
actor, selection, phase owner, and both clocks.

Producer histories keep compact typed rows until receipt serialization,
including the same first/recent 128 rows and eight transitions with their
128-row preceding histories. This avoids retaining expanded JSON maps for each
transition. Whole-process RSS and all other acceptance budgets still apply.
Fresh aggregate acceptance remains required for these changes.

The frontline diagnostic's minimum health is 88 in the first half and 72 in the
second half; its mixed spell cycle also slows and displaces pursuers. The next
recipe approaches the nearest hostile and casts only Fire Bolt until its first
real respawn, then rejoins the mixed workload. It changes no health, damage,
collision, authority, or respawn rule. Other participants retain the full mixed
recipe. `campaign-wall-phase.py` checks three consecutive combined repeats
before isolated rendering, authority, delayed networking, and the combined soak;
a failed gate stops the campaign without lowering a budget.

Before the next capacity campaign, the branch rebases onto main
`fae8c18a22fc6cbf007d7dcf0182423137f896ef`, including elapsed movement-prefix
consumption within the existing tick budget. Completed-travel observations now
accept a newer physics step within the same partially consumed packet sequence;
they retain the unconsumed inputs and do not mint elapsed time. The merged
regression exercises partial and final confirmation through that observation
path. Pre-rebase world checks pass 538 tests (two existing ignored fixtures) and
engine checks pass 125 tests. The pre-rebase native/build chain is canceled for
integration and is not reported as passing.

Merged world verification records 540 passing tests and two existing ignored
fixtures, with one migration test refusing to reopen its scratch writer lock.
`wall-phase-main-migration-retry.log` records that exact test passing in isolation
without a code change. The full-suite failure is retained and is not relabeled
as a passing suite. New partial-confirmation and wall regressions pass in the
merged run.

`wall-phase-main-native-tests.log` selects zero tests because the invocation
omits `remote-chamber` and uses the source filename as its module filter. It is
not native-session verification. The corrected invocation enables the remote
chamber and desktop features and runs the crate's library tests, including the
six tests in `imported::chamber_session::tests`.

Merged renderer verification passes 78 tests with six existing ignored fixtures.
The host build initially fails while creating its world archive because the
shared filesystem is full. The retry preserves that log and removes only
obsolete, inactive ELF executables identified by retained hashes or test paths.
`obsolete-executable-cleanup-wall-phase.json` and
`obsolete-test-executable-cleanup-wall-phase.json` record every removed path,
size, inode, and SHA-256. Build caches and other checkouts remain intact.

The corrected native feature set passes 51 library tests, including all six
session tests, with four existing ignored fixtures. Host verification passes two
library tests and the actual TLS two-world integration test after the disk
recovery. A benchmark command incorrectly asks the `verse` package for
`service-net`, which belongs to `verse-world`; it performs no build. The retained
corrected invocation uses the example's required `remote-chamber` feature.

The first corrected-feature build still enables the desktop defaults and fails
because ALSA development metadata is unavailable. The offscreen acceptance
profile uses the documented `--no-default-features --features remote-chamber`
command instead, with `VERSE_BENCH_SOURCE_REVISION` set to the rebased commit.
The failed desktop build is retained; it establishes no runtime result.

The first pinned wall/phase combined repeat completes all clients and exact
recovery, including two actual frontline respawns. It fails only ordinary
prediction p95 (0.2532 m against 0.25 m) and maximum (1.0668 m against 1 m), so
the campaign stops without a soak. It records 1,765 workload ticks, twenty
recovered characters, forty live hostiles, 82 active receipts, 102 ledger
revisions, 512 events, and a 302,387-byte checkpoint. The strongest remaining
correction follows a projected actor-230 overlap with penetration 0.1453 m;
forward input points out of that contact, but prediction holds twenty steps
before confirmation. The next regression exercises straight outward motion,
inward input, a fixed wall, loss of floor support, external force, a jump,
and a second opposing capsule. No earlier failure is relabeled.

`separating-crowd-regression-before.log` reproduces outward input freezing at
zero travel. The revised prediction permits only grounded horizontal straight
motion that separates every initial convex capsule contact. It sweeps all other
colliders, verifies non-increasing original penetration at the endpoint, and
requires the same unchanged supporting floor. It adds no depenetration push.
Inward movement, opposing contacts, fixed-wall obstruction, floor loss, jumps,
external motion, and query failures retain their prior handling. Deferred past
steps stay deferred. A bounded counter records actual separating steps.
`separating-crowd-prediction-tests-supported.log` records 37 passing prediction
tests, including all seven contact/support cases. An earlier compile failure
attempts to read the private support pose; a read-only motor accessor fixes the
boundary without changing motor behavior. Fresh acceptance remains required.

The separating-motion revision passes the full world suite: 542 tests, with two
existing ignored fixtures, including the earlier migration writer-lock case.
Physics verification passes 131 tests with one existing ignored fixture.
Native session verification and a fresh offscreen build precede the next pinned
`campaign-separating-crowd.py` run. Acceptance budgets stay unchanged.

The separating-motion campaign passes three consecutive combined sixty-second
repeats, isolated rendering, six hundred simulated authority seconds, and the
delayed network profile. Its combined six-hundred-second soak completes every
client and recovers twenty characters, forty live hostiles, and sixty actors.
It records 124 active receipts, ledger revision 912, 512 retained events, and
an equal 442,358-byte checkpoint size before and after recovery. Steady RSS
growth is 50,442,240 bytes, within 64 MiB. The campaign still fails the ordinary
correction maximum: 1.6714 m at 170.82 seconds, including its late-window gate.
All other declared gates pass. The exact failed receipt and pinned manifests
remain retained. The strongest trace has unchanged life and epoch, a previously
processed 74-step interval, and nonzero external motion in its confirmed motor.
A new current-boundary input marks that whole history for replay against a
later crowd pose. A regression now checks that future input cannot rewrite
completed local travel; changed authority motor state still requires correction.

`fresh-input-regression-before.log` reproduces a 3.2017 m collapse when a fresh
interval input replays historical steps against a later capsule pose. Queueing
frame-profile input at the current clock boundary now preserves the processed
motor and changes future integration only. Existing dirty state from changed
authority motor, fixed geometry, or retired past input remains effective. The
legacy arrival profile retains its replay behavior. The first corrected test
run reaches the preserved-history assertion but fails a test expectation that
uses positive Z for forward input at yaw zero; the expected direction is fixed
to negative Z before full verification. Neither failed test log is relabeled.
`campaign-fresh-input.py` repeats the same seven-stage unchanged-budget campaign
on fresh pinned source and executables after targeted checks.

Fresh-input verification passes 543 world tests (two existing ignored fixtures),
all six native session tests, formatting, and the offscreen build. The first
combined repeat passes every declared gate except throughput: 1,761 workload
ticks against the required 1,764. Journal sync reaches 405.319 ms, storage pause
totals 0.7966 seconds, and schedule dropped time totals 0.4781 seconds. Native
ordinary correction maxima are 0.4068 m and 0.5334 m. The campaign stops and
retains the failure. `campaign-fresh-input-repeat.py` reruns the complete
seven-stage campaign on the identical source and executable hashes to check
repeatability; it changes no storage rule, workload, or acceptance budget.

The identical-source repeat campaign passes all three combined sixty-second
runs (1,798, 1,792, and 1,793 workload ticks), isolated rendering, six hundred
simulated authority seconds, and delayed durable networking. Native ordinary
correction p95 stays below 0.138 m and maximum stays below 0.641 m across the
three repeats. `execution-manifest-fresh-input-repeat.json` pins source patch
SHA-256 `8999a419864d78e4d8aa066ff5139598a123f465570882bd738ed95808d9a852`,
battle executable `b853005398a46a569cf9fff35685e534ae2672c63797bf22fb2785e3ff4b70b0`,
and renderer executable `c7e50e348bfd888da7c7098769c7c258bb61beabe60ee38ab9c0e289d0ae8b27`.
The combined ten-minute soak passes every declared gate. It records 17,892
workload ticks, simulation p99 upper bound 27.039 ms, steady CPU frame p95
5.735 ms, GPU p95 5.465 ms, and snapshot age p95 224.777 ms. Native ordinary
correction maxima are 0.6401 m and 0.4268 m, with p95 zero in both steady
segments; all twenty late windows pass. Its 114 post-thirty-second RSS samples
grow 48,709,632 bytes from 373,096,448 to a peak 421,806,080 bytes, within
64 MiB. Recovery verifies twenty character inventories, forty live hostiles,
sixty actors, 124 active receipts, ledger revision 912, 512 events, and a
439,216-byte checkpoint. The frontline player has one accepted actual respawn.
All seven receipt hashes and the source-patch hash verify.

Before the closing commit, main advances to
`c0611aab1f880ff9184a091eb6c62ccb36658a62`, adding bounded native recovery of
verified authority time, movement frames during a teleport reply wait, and
batched renderer instance-index uploads. Those changes affect this workload
and merge without conflicts. The accepted earlier revision remains pinned;
`campaign-final-main.py` measures the integrated revision after targeted
world, native, and renderer checks. Its acceptance is pending.

Integrated verification passes 546 world tests (two existing ignored fixtures),
six native session tests, and 78 renderer tests (six existing ignored fixtures).
The first integrated benchmark build fails with `No space left on device`.
`obsolete-executable-cleanup-final-main.json` records six obsolete paths,
2,196,600,344 bytes across unique executable inodes, their hashes, and retained
proofs. Each inode is checked against running `/proc` executables before removal.
The accepted earlier battle and renderer hashes remain in their receipts.
Only inactive earlier-source executables are removed; build caches and other
checkouts remain intact. The retry retains a separate build log.

The integrated build retry passes. Its first combined repeat passes all gates
except throughput: 1,739 workload ticks against 1,764 required. It records
732.481 ms maximum journal sync, 1.5262 seconds of storage pause, and 0.4536
dropped seconds. Simulation p99 is 10.396 ms; native ordinary correction p95
stays below 0.167 m and maximum below 0.630 m. Recovery verifies twenty
characters and forty live hostiles. The failed campaign stops and remains
retained. `campaign-final-main-repeat.py` runs the complete campaign on the
same source and executables, with unchanged workload and budgets.

The integrated repeat campaign passes all seven stages. The final summary at
the top of this document identifies the accepted receipts and quantitative
results. Earlier pending and failed entries above describe their recorded
source stages. The closing checkout also integrates subsequent main changes
to zone camera helpers, tower content, and terminal presentation; those changes
do not alter the measured chamber authority, worker, prediction, native session,
or renderer implementation. The campaign's pinned revision remains explicit.

Post-rebase checks verify the receipt's measured source-file hashes against the
closing checkout. Source/prose whitespace checks pass. Raw retained command logs
and implementation patches preserve their original whitespace, including blank
log endings and patch context lines; they are excluded from prose formatting.
