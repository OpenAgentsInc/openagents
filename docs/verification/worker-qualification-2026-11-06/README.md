# Worker qualification follow-up

Issues #12980 and #12981 keep the existing 1,060,000-byte worker budget and
600-microsecond optimization qualification limit. Neither limit changes.

## Correct the paired spell fixture

The failed 250 ms observation holds person 38
[offline](mac/original-facts-250-offline.json) and person 40
[online](mac/original-facts-250-room.json), 25 ms apart.
Both have condition bits 256. The original fixture independently asks each
room to hold its nearest living person. Small position differences can
select different people, so comparing those outcomes does not establish a
condition-timing regression.

The check now starts the original online hold, observes its unique present
paralyzed person, and names that person in the offline development host.
The new `hold-person CODE SEAT STAMP PERSON` control command requires a
development or local room. It retains the existing living-person, 30 m
reach, six-second NPC effect and three-second player hold contracts.
Public rooms refuse it. Invalid commands do not consume a stamp.

A new required verdict observes the same actual person on both pages.
Every existing person-condition comparison and drift tolerance remains.
The full local/100/250 ms replay, including all three raids and three
background players, passes all gameplay verdicts. This also tests the
actual generated worker and native online room together.

Tests precede the implementation. The isolated authority policies kill
24/24 observed-target mutants, 41/41 exact-effect mutants, and 33/33
room-context control mutants, with zero survivors and no equivalents.
The room-context run retains every room check. It also exposed the unused
quick-match wrapper tracked in [#12982](quick-wrapper.md).

The native-offline branch also passes its 100 ms paired replay with three
background players and a real Linux room binary. This separate check runs
at nice 15 without an optimization timing gate or raids. Its
[receipt](native/qualification.json) records unchanged inputs and a passing
gameplay result; the [log](native/replay.log) retains the exact observed
target. It does not supply a 600-microsecond result.

## Measure at normal priority

The build gate lowers process priority. A timed qualifier now refuses a
nonzero nice value before launching browsers. Build its image through the
gate with an intentionally invalid argument such as `--tick-us 1000001`,
then run the prebuilt image directly at nice 0:

```sh
coax/web/build/tools/everglade-drift-COMPILER_SHA \
  --profiles local,100,250 --bots 3 --port 9360 --tick-us 600 \
  --out /tmp/oa-worker-final-drift
```

The receipt records actual priority and hashes the native room, page,
worker, browser bridges, observer source and executable before and after
the run. Changing an input fails the run.

The retained [full replay](mac/full-replay.log) runs at nice 0 with unchanged
input hashes. Its all-sample p95 is 550/785/895 microseconds across
1,929/1,989/2,050 ticks. Local passes; 100 and 250 ms fail. No passing
600-microsecond claim follows from the gameplay result or the separate
11 ms lifecycle budget.

Five unrelated CrashReportClient processes each consume approximately one
CPU core during this attempt. None belongs to this task and none is
stopped. This is not the documented uncontended target setup. The original
passing 486/530/491-microsecond record remains in its original location;
the follow-up retains its failed observations as well.

An instrumented current worker adds the existing seven phase clocks. Its
100 ms diagnostic replay passes at 505 microseconds across 2,009 samples;
river and boat setup accounts for most of the previously unassigned time.
The diagnostic artifact is not substituted for either release artifact.
The large difference between attempts does not establish the cause of the
release timing failures. The exact worker hashes and failed observations
remain the qualification evidence.

The actual Linux release worker also runs on the target Mac at nice 0,
with three background players and every raid. All gameplay verdicts pass,
including the observed named target. Its full-sample p95 is 630/555/700
microseconds across 1,928/2,053/2,045 ticks. Local and 250 ms fail the
unchanged 600-microsecond limit. See the
[cross-build replay](linux-on-mac/full-replay.log) and its unchanged-input
[receipt](linux-on-mac/qualification.json). Both issues remain open for
timing qualification; fitting bytes and correct gameplay do not close
their done-when requirements.

## Build identities

See [build profiles](build-profiles.md) for the original Linux oversize
reproduction and platform-specific Binaryen selection. The admitted
original 0.52 compilers and their original provenance receipts remain
unchanged.

The current ordinary Mac worker is 1,054,815 bytes, SHA-256
`8a3ab66ef5b0533788cbe17763b6455e2aecf77033090231e754f5b51e314031`.
The current ordinary Linux worker is 1,043,301 bytes, SHA-256
`860c92a3f4c1d0ce0255f1b56bbce8d1819210267219f94d12e2d348c49f2a66`.
Both fit the unchanged canonical byte budget. Their timing qualification
remains separate and unresolved.

Compiling the wrapper at C `-O2` produces a rejected 1,102,285-byte
candidate. Compiling only strict math at C `-O3` produces exactly the
existing worker hash, so that change provides no optimization and is
discarded. Neither experiment changes the supported builder.

## Retained execution artifacts

Large binaries, the generated C/header banks and manifests, original
compiler/helper/receipt files, raw facts and screenshots are retained under
`~/.openagents/aethyrforge/worker-qualification-12980-12981/2026-11-06/`.
Mac artifacts live in `mac/` on the target Mac. Linux artifacts and the
original oversized reproduction live in `linux/` on coderos-4080. The
original source revision is `96b81d12ac`; the follow-up implementation is
`3a1782efa9`, `f15d481971` and `a5ff8ecc14` on the rebased main branch.
