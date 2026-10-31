# Tiny owned gantry: independent native checks

This slice qualifies the chapter 112 simulator and imitation learner. It uses
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
chapter 70 training/numerical dependencies need no repeated mutation gate.

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
[chapter 112 record](../../../docs/verification/psionic-ch112-2026-10-31.md)
describes the frozen experiment, original hashes, results and limits.
