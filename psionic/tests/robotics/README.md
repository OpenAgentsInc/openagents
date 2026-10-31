# Robotics verification

The default `robotics` / `all` group runs portable Coax debug/release checks,
optional root-only mutation, and the available CPU native debug/release and
ASan/UBSan groups. It requires neither private weights, Torch nor Metal.
Original-header fact comparisons use Python's standard library.
Device/library names such as `groot-device-kernels` and `groot-text-kernels`
select portable VM checks; their actual GPU/Torch gates are explicit opt-ins.

```shell
psionic/tests/robotics/check.sh --list robotics
psionic/check.sh --only robotics/groot-vision-metal-plan robotics/groot-text-metal-plan
psionic/check.sh --only robotics/groot-action-metal-plan robotics/groot-action-transcript
```

CPU groups cover image/request/embodiment processing, text/vision layout,
linear/flow/DiT components, state-encoder helpers, full vision/text/action
metadata graphs, action kernels/wide norms and canonical action transcripts.
Shared native groups run once when several selected units use the same oracle.
`--list` prints the selection without executing any gate.

| Explicit opt-in unit under `robotics/` | Requirement / runner |
| --- | --- |
| `groot-device-gpu` | Compatible Apple Metal GPU and existing Torch reference environment; `check-groot-device.sh` |
| `groot-text-kernels-gpu` | Compatible Apple Metal GPU and existing Torch reference environment; `check-groot-text-kernels.sh` |
| `groot-linear-original` | Authenticated category2 BF16 parts and existing NumPy; CPU component slice only |
| `groot-metal-original` | Same selected parts, NumPy and compatible Metal; original state-encoder GPU slice |
| `groot-vision-metal-original` | Authenticated full shards, original reference capsule, NumPy and Metal |
| `groot-text-metal-original` | Full shards/reference capsule/NumPy/Metal; optional actual Coax vision output directory |
| `groot-action-metal-original` | Full shards/reference capsule/NumPy/Metal; optional actual Coax backbone file |
| `groot-policy-metal-original` | Full shards/reference capsule and existing Torch/NumPy/Metal; complete composed policy gate |

These opt-ins are excluded from `robotics` and `all`, and fail when required
devices or inputs are absent. They never count a skipped GPU as a pass. Set
`PSIONIC_GROOT_STATE_WEIGHTS` for the four selected state parts;
`PSIONIC_GROOT_SHARDS` and `PSIONIC_GROOT_CAPSULE` for original-model gates.
`PSIONIC_GROOT_VISION_OUTPUTS` / `PSIONIC_GROOT_BACKBONE` optionally select
actual upstream Coax features for the separate text/action gates.
`REFERENCE_PYTHON` selects existing NumPy for state/vision checks;
`GROOT_REFERENCE_PYTHON` selects the existing framework reference environment.
No package installation or model download occurs in these dispatchers.
The dispatcher uses temporary outputs; invoke the named direct runner with
an explicit new output directory to retain its qualification artifacts.

The action-plan native wrapper now has a portable default:

```shell
psionic/tests/robotics/check-groot-action-plan-native.sh NEW_OUTPUT_DIRECTORY
psionic/tests/robotics/check-groot-action-plan-native.sh NEW_OUTPUT_DIRECTORY EXECUTED_GPU_BUILD_DIRECTORY
```

Both modes run the same 3,570 native graph/ABI cases in three build modes.
The portable mode compares complete descriptor transcripts between debug,
release and sanitized builds. The explicit prior-build mode additionally
compares every transcript with the previously executed GPU graph; an optional
third argument supplies that earlier transcript directly. Portable agreement
is metadata evidence and makes no claim that a GPU ran.

Dispatcher integration reuses the existing passing component/native records;
only shell syntax and selection routing were checked for this thin-wrapper
change. No unchanged numerical, mutation or original-model gate was repeated.

## Tiny owned gantry: independent native checks

This slice qualifies the chapter 167 simulator and imitation learner. It uses
owned synthetic episodes, a declared two-axis Cartesian state in metres,
velocity commands in metres/second, an exact 0.1-second sample and a separate
zero/expert baseline. It does not drive hardware or claim compatibility with
NVIDIA's Panda embodiment.

The C oracle independently calculates the clipped expert, affine actions and
Euler integration; enumerates state/policy/action limits and non-finite inputs;
checks refusal preserves the original state; and compares learned parameters
to the closed-form slope 0.5, intercept 0 solution under frozen error/0.25m and
velocity/0.5m/s scales. Its independently written
six-episode evaluation computes teacher-forced velocity MSE and separate
closed-loop final-position success after 96 ticks. Whole evaluation episodes
are disjoint from the four training episodes.

It checks all 32 demonstrated rows against its own expert integration and pins
the recipe to 128 full-batch epochs of 32 rows, rate 1, no decay and shuffle seed
112. Separate half-gain policies distinguish errors in the two action axes.

```shell
COAX=coax/bin/coax psionic/tests/robotics/check-native.sh /tmp/robotics-native
```

The thin wrapper emits the actual Coax library, then runs the independent
oracle in native debug, release and ASan/UBSan modes without fast math or FMA.
Authority checks and focused root mutation are separate commands; unchanged
chapter 103 training/numerical dependencies need no repeated mutation gate.

Qualification on 2026-10-31: **3,882 cases passed in each of native debug,
release and ASan/UBSan**. No whole-model inference or external simulator ran.
The final run also independently checks worst-episode/axis diagnostics and the
inclusive success boundary at adjacent floating-point values, signed zero and
non-finite inputs. This supersedes the earlier 3,870-case native run preceding
those added observable diagnostics; it does not repeat an unchanged gate.

| Policy | Success after 96 ticks | Teacher-forced velocity MSE | Worst final position error |
| --- | --- | --- | --- |
| Zero | 0/6 | 0.057325929036138298 | 2 m |
| Learned | 6/6 | 2.9478047861576636e-34 | 0.00047750247539857238 m |
| Expert | 6/6 | 0 | 0.00047750247539846136 m |
| Half X gain | 2/6 | 0.0024315831520581098 | 0.020276547153583624 m |
| Half Y gain | 2/6 | 0.0026106195930875745 | 0.020276547153583624 m |

Success means the maximum absolute axis error is at most 0.01 m. The MSE uses
both velocity axes at all 576 expert-trajectory states; closed-loop success
uses a separate state rolled forward by each candidate's own actions. These
are distinct measurements. The simulation has exact observations and dynamics,
no collisions, delays, contact or image input. These results qualify this tiny
owned task; they do not establish robot transfer or GR00T support.

The separate [SingleArm fixture](../../fixtures/robotics/singlearm-panda-open-drawer/README.md)
retains an original 28KB Parquet episode and five inert selected rows. Its
Coax reader validates the exact 25-state/8-action metadata profile and restores
170 F32 words. The independent native oracle compares all those words with C
tables derived directly from the original file: 181 cases each in debug, release
and ASan/UBSan. The original-data Python oracle is optional development tooling,
requiring an existing PyArrow environment; ordinary gates need no decoder.

```shell
psionic/check.sh --only robotics --jobs 4
psionic/check.sh --only robotics/singlearm --jobs 4
```

These commands select focused authority mutation and native checks. The
[chapter 167 record](../../../docs/verification/psionic-ch112-2026-10-31.md)
describes the frozen experiment, original hashes, results and limits.
