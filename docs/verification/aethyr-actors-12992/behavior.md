# Bounded actor intent selection (#12992, partial delivery)

The change containing this record adds a typed behavior compiler and a pure
fixed-step intent selector. Base for final host qualification: `593ceb26af`.
The [source hash inventory](behavior-source-hashes.txt) pins all nine added
sources. This is not completion of #12992: no live guard, villager, mount,
perception query or movement body consumes these programs yet.

## Supported boundary

`behavior-plan.coax` admits owned programs against a compiled archetype and
the full existing state-schema identity. Bounds are eight priority sequences,
four conjunctive guards and four actions per sequence, eight schedule rows,
16 anchors, and the archetype's declared action-work budget. Guards bind only
declared typed fields, supplied sensed/path flags and the current schedule.
Unknown verbs, invalid arguments, unsupported controller combinations,
noncanonical values and malformed schedule ordering refuse before selection.

The seven verbs are HOLD, GO, CHASE, FLEE, INTERACT, SAY and CONTROLLED.
They are body commands, not execution implementations. The internal AGB1
fingerprint covers the complete schema identity and every active compiled
field, anchor, guard, action and schedule word. It is not a new public/save
format. The trusted compiler's owned outputs and the projector's admitted
snapshots are the supported inputs; arbitrary decoded structs are not.

`behavior_open` verifies plan shape/fingerprint once and creates a cursor.
`behavior_project` admits the shared `StateStore` and copies only declared
fields. `behavior_step` requires the exact next paid 1/64-second realm step;
it creates no clock. It validates cursor, context, world epoch and full body
feedback correlation key before selection. Refusal returns the old cursor
with no command/events. The caller must discard stale feedback and finish
the same paid tick with admitted input before publishing it. A locally
matching key is not evidence of current entity or owner authority.

Priority is ascending, ties use full stable IDs. One existing random stream
draw is consumed per fresh choice, including a sole candidate. Continuation
and idle consume none. Repeat suppression stays within the top priority
group. Cooldowns, action durations and interruption operate on paid ticks.
HOLD/SAY succeed at duration; other verbs time out as failure. Explicit
matching body outcomes can complete earlier. Lost/replaced targets and
changed scheduled anchors cancel obsolete commands. A time jump resolves
the current row directly instead of replaying skipped routine transitions.
Unavailable next actions fail with serial zero and do not mint a body serial.
Counters refuse at one billion without wrapping or publishing partial state.

## Checks and mutation

The final [baseline](behavior-tests.txt) passes **381/381 checks**, including
25 new behavior checks and all loaded dependency checks. The action-pair
property covers all 49 pairs of native verbs, including clearing arguments,
anchors and target generations at transition. Its 200-million-step compile
budget is deliberate: the fixed 49 cases consume 154,322,949 steps, mainly
schema hashing, and complete in seconds. The compiler's near-limit diagnostic
is retained. Other checks cover exact admission/counter bounds, active and
inactive cursor corruption, all feedback keys/outcomes, full-width IDs and
seeds, known/visible consistency, cooldown boundaries, noninterruptible
progression, simultaneous completion/preemption, schedule jumps and weighted
choice against independently generated golden choices.

| Entry-source mutation scope | Result | Evidence |
| --- | --- | --- |
| Behavior compiler | 391 killed, zero survivors | [report](behavior-plan-mutate.txt) |
| Intent selector | 765 killed, zero survivors | [merged report](behavior-mutate.txt), [shard 1](behavior-mutate-1.rec), [shard 2](behavior-mutate-2.rec) |

The compiler initially retained 92 survivors, then three. Boundary checks,
active schedule-literal fingerprint coverage and an inactive field-slot
forgery check were added. The shared value gate replaced a redundant finite
precheck. [Initial](behavior-plan-mutate-initial.txt) and
[intermediate](behavior-plan-mutate-intermediate.txt) reports are retained.

Runtime qualification first exposed an unexercised value comparator. The
next unsharded run hit its 600-second cap and was stopped; it is not passing
evidence. Two subsequent shards retained 38 survivors. Tests were sharpened;
overwritten state assignments and redundant checks were removed, the schedule
scan starts after the already validated phase-zero row, and fact/signal guards
are separate from schedule predicates. The next shards retained two survivors:
disabled scheduled GO versus fixed anchor 1, and an unavailable second action's
failure outcome. Both now have direct checks. The final two complete shard
records merge to zero. No mutant is waived or marked equivalent.

Mutation ran at the earlier base `331caba6f2`; entry sources and their tests
have identical hashes after rebasing. Main meanwhile extended imported pack
and image code. The full rebased baseline and freshly emitted native/Wasm
artifacts below qualify that changed dependency closure. The earlier phase
137/137 result at `a70321f380` and definition 316/316 result at `331caba6f2`
remain unchanged evidence; neither gate is rerun or claimed as a whole-engine
mutation pass.

## Independent execution evidence

`behavior-trial.coax` executes 256 actual paid steps for each of 30, 60 and
144 FPS and uneven 5/27/14/31 ms frames. Integer nanosecond frame intervals
feed the existing `FrameClock`; the same paid-step sequence drives the shared
state store and selector. Four authored sequences combine visible-target
priority, a scheduled GO, HOLD and weighted SAY. Two schedule rows depend on
day parity and a typed world fact. Recorded jumps at ticks 73 and 129 select
the current row. Observations and GO completion/failure feedback are scripted,
not produced by live perception/navigation. Every eleventh tick probes stale
feedback, then completes the same step normally: 23 probes per cadence.

Each row records 64 exact words: state facts/time, signals, schedule, selected
sequence/action, command, random/serial counters, outcomes, both event slots,
cooldown state, feedback/refusal probe, cursor tick and the full policy hash.
The [independent Python oracle](../../../aethyr/actors/tests/behavior_oracle.py)
constructs schema/program SHA-256 independently, uses weighted bisection
instead of subtracting tickets, and models cooldowns with absolute deadlines
instead of elapsed subtraction. It checks every word and the complete row
set. [Oracle output](behavior-oracle.txt) records both targets.

The [native observations](behavior-native.txt) contain all 1,024 rows
(234,584 bytes), SHA-256
`b4c59675eddc0225ad01b51df709c885c4a2a1d128548ab000199a002a2fbfec`.
The final C emitted for Wasm is also compiled by GCC 15.2.0 with
`-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all
-fno-omit-frame-pointer -fno-fast-math -ffp-contract=off` and exercised through
the same thin C ABI face that Chrome calls. ASan/UBSan with leak detection
reports zero diagnostics. The output is byte-identical to the earlier
independently checked native reference, and the oracle also checks the final
native artifact directly. Native stack remains the system's 8 MiB limit.

Actual headless muted Chrome instantiates the release Wasm and executes all
four profiles through that face. Each profile matches all 256 oracle rows.
The 64 row words are identical across cadences and targets; the observation
text additionally prefixes the FPS and row index, so file hashes differ:

| Browser cadence | Text SHA-256 |
| --- | --- |
| 30 FPS | `8536ec29f7f9795ab1e2101a4ca182107481cc91cf73642a0843e7bfdc7d288e` |
| 60 FPS | `296340786736a7ea8b081a677ac1e4ee683a607c629c87f4666288bb723989dd` |
| 144 FPS | `1b6e634a27aa8cfb31fd417e6ddd0de0d3ada380e7fc4344a6fc8ca496cd07fd` |
| Uneven | `513081fdeb97a473ec4a60a1adc4b965163faa4463092bae2082d65fe8da0118` |

Coax compiler: 0.52.0, SHA-256
`bd13739490a65b8969c6fb4298ca8f94264416e41ad1a146ca8170fdec8acacc`.
Final Wasm: **80,040 bytes**, SHA-256
`908a1486e3938e2fd2c9bc37321cbf3f7770ea9582101bca98ea12e40912cf9c`.
The final build reports 11.6 seconds: 7.1 s Coax emission, 0.9 s clang,
0.3 s link and 0.2 s wasm-opt, plus setup. The ordinary 1 MiB Wasm stack and
64 MiB imported memory are unchanged. The CDP driver's separate 2 GiB stack
setting is its existing tool arena convention, not the actor module's stack.
Generated C SHA-256:
`880894552d04ba7d8ded34dabc3c0a93c95ebbf90583d68a517e873da7fab432`;
header:
`8a2d87a4a5d9845076b967c66568ac8cbfde6d3dc48e52194a4a5d1bec28de4f`.
Generated C, header, Wasm, sanitizer executable, outputs and build timings are
retained under `~/.openagents/aethyrforge/actors-12992/behavior-v4/`.

## Work and allocation scope

The four-row fixture peaks at five guard evaluations, four row-predicate
evaluations, one random draw, one emitted command and two events per paid
step. Each cadence ends with 134 sequence successes, two failures and 20
interruptions; 168 commands start. Emitted command ticks are HOLD 105,
GO 41 and SAY 110. These are selection counts, not physical actor frames.

The general admitted profile caps guard evaluations at 36, row-predicate
evaluations at eight, draws at one, commands at one and events at two.
`work.sequences` counts row-predicate evaluations, not all loop visits:
eligibility, priority and weighted selection make additional fixed bounded
scans. There is no runtime heap allocation in these procedures. The fixture
retains a fixed 256×64×8 = 131,072-byte trace plus its count; this is test
storage, not an actor's resource budget. Successful execution at the default
stack/memory settings is measured; exact native/Wasm stack peaks and actual
actor pool/resource allocations are not measured or qualified here.

## Reproduction and remaining delivery

```sh
coax/bin/coax test aethyr/actors/behavior.coax --library
coax/bin/coax mutate aethyr/actors/behavior-plan.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/actors/behavior.coax --library --root-only --shard 1/2 --jobs 8 > /tmp/b1.rec
coax/bin/coax mutate aethyr/actors/behavior.coax --library --root-only --shard 2/2 --jobs 8 > /tmp/b2.rec
coax/bin/coax mutate aethyr/actors/behavior.coax --library --root-only --merge /tmp/b1.rec --merge /tmp/b2.rec
coax/bin/coax run aethyr/actors/tests/behavior-trial.coax > /tmp/behavior.txt
python3 aethyr/actors/tests/behavior_oracle.py /tmp/behavior.txt
coax/web/wasm.sh aethyr/actors/tests/behavior-trial.coax aethyr/actors/tests/behavior-face.c /tmp/behavior.wasm
coax/web/tool.sh aethyr/actors/tests/behavior-browser.coax /tmp/behavior.wasm /tmp/behavior-browser
python3 aethyr/actors/tests/behavior_oracle.py /tmp/behavior-browser-*.txt
```

Use each command only when its covered inputs change; the checked-in reports
are reusable evidence. [Tools admission](behavior-tools-check.txt) passes
1,890 tracked tool files with zero new files missing a reason. The oracle has
Firm reason 2. All scenario sequencing and input generation stay in Coax;
the C face only projects the ABI, and browser JavaScript only reads words.

The [module README](../../../aethyr/actors/README.md) retains all remaining
requirements: actor instances/lifecycle/current ownership; aggregate pools and
resource reservations/retirement; actual perception and nav/model integration;
shared human/AI rider input and mount fixture; guard/villager playable adoption
from editable compiled data; adopted native/Wasm replay and captures; exact
residual adapter inventory. No private realm scheduler is migrated in this
checkpoint. Chapter 75 stays partly implemented; no teaching registry status
is promoted. #12992 stays open.

The attempted `python3 psionic/tests/book.py --book aethyr` stops before book
validation at an inherited capability pin failure:
`coax.wgsl.current: evidence changed; review verse/everglade/gpu.bindings`.
Both that file and `coax/book/capability-claims.json` are identical to remote
base `593ceb26af`; neither is edited here. The [failure log](behavior-book.txt)
is retained. This is not a passing book check or a chapter promotion.
