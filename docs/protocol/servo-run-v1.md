# Portable illustrative servo run v1

`psionic.robotics.servo-run.v1` is a new inert companion to SLV1 and immutable
RXE1. Its sole codec is `psionic/lib/robotics/servo-run-wire.coax`. It never
executes code, performs inference, downloads an unverified model or moves
physical hardware. Model/assembly/topology digests bind the source RBA1 data.

Profile `servo-native-wasm-f64-v1` fixes `scalar-servo-play-f64-v1`, binary64
arithmetic with round-to-nearest/ties-even, no contraction or fast-math, and
the SLV1 2 ms clock/relative-radian/one-radian-effort conventions. Results are
recomputed through the shared actuator. Other execution modes are refused.
Only actual native/Wasm observations qualify the declared exact trace; they
do not qualify arbitrary GPU arithmetic or a physical robot.

A run contains an initial and final complete actuator snapshot, including
all 16 delay cells, cursor and absolute step counter. Parameters come from
each accompanying SLV1 packet. There are at most 32 ordered held-target
commands and 4096 additional steps. Commands apply at their recorded absolute
tick before the next step; equal-tick commands retain their stored order.
The final boundary may contain a target command without a further step.
Parameter changes create a new run. End reasons: paused=0, completed=1,
cancelled=2, limit=3. Opening replays only a bounded analytical computation.

The initial observation contains the snapshot's actual position/rate/output,
commanded target and available effort. Applied target and all force fields
are zero because no step has run in this branch. The initial graph has one
row. Camera, selection, explosion, chart viewport and playback cursor are
presentation state and absent from the artifact. Seeking derives a separate
session; it cannot rewrite the source artifact. Branch creates a new initial
snapshot with a nonzero parent artifact identity; the snapshot's absolute
tick records the branch point. No random seed is claimed for deterministic
arithmetic.

SLR1 uses little-endian integers and exact finite IEEE binary64 words,
preserving signed zero. Its complete length is `1088 + 12 * commands`, at
most 1472 bytes. No padding or trailing bytes.

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 4 | ASCII `SLR1` |
| 4 | 4 | exact complete length |
| 8 | 4 | target command count |
| 12 | 4 | end reason |
| 16 | 32 | SHA-256 of UTF-8 `servo-native-wasm-f64-v1` |
| 48 | 32 | parent artifact identity; all zero for root |
| 80 | 32 | computed trace identity |
| 112 | 312 | initial SLV1 packet |
| 424 | 160 | initial snapshot |
| 584 | 312 | final SLV1 packet |
| 896 | 160 | final snapshot |
| 1056 | 12 × n | u32 absolute tick, binary64 target |
| 1056 + 12 × n | 32 | artifact identity |

A snapshot is 19 binary64 words (position, rate, observed output, all 16
queue cells), then u32 cursor and step count. Its parameters are exactly its
SLV1 configuration's parameters. All actuator/state/source-limit admission
precedes publication. Initial/final config, packets, complete queues and
trace must equal replay's actual results, including exact float words.

The trace starts with tagged SHA-256 of the initial SLV1 packet, tag
`psionic.robotics.servo-trace.v1`; after each actual step it becomes ordinary
SHA-256 of `previous_digest || SLV1_current_packet`. The artifact identity is
tagged SHA-256 of all bytes preceding its final digest, tag
`psionic.robotics.servo-run.v1`. Identities bind bytes, not authorship, rights,
account authority or hardware safety. Readers require exact framing, profile,
artifact digest and current model/assembly/topology identities before replay;
bad records preserve the caller's previous run, pose, cursor and graph.

Native Save/Open-latest uses the existing byte-storage contract under the
shared OpenAgents world-state directory. Browser Save/Open-latest uses that
same opaque byte contract in IndexedDB; it does not imply browser shell/model
offline availability. File Open/Share use existing platform consent/document
effects. The full verified world pack remains in the shared asset cache.
