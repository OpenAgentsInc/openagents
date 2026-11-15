# Robotics RB-00 / RB-01a — decoded experiment query

Design/source base: `d0b9aac147`. This first coding slice implements the
[frozen decoded contract](../../psionic/docs/robotics/experiment-contract-rb01.md)
and advances [chapter 257](../../psionic/book/ch257-a-robot-description-becomes-an-experiment.md)
to partly implemented. RB-01's persistent record/codec is a separate next slice.

## Implemented boundary

`psionic/lib/robotics/experiment-policy.coax` owns `rex_check`, `rex_admit`
and `rex_map`. They admit bounded periods, collection metadata, angle/rate
observations, joint/motor permutations and caller-declared identities.
They bind every active joint mapping field against the expected embodiment.
The map preserves radian quantities, reports requested and limited targets
with saturation, and stages all results before publication. The API has no
device, simulator, file or network effect.

The two-joint fixture is owned illustrative data. Its exact independent
decimal/rational table is in the freeze, with a 1e−15 absolute tolerance
for the non-dyadic first joint and exact checks for the second joint.
The example includes a wrong-sign refusal under unchanged advertised IDs.

## Observed checks

The focused run used the existing robotics dispatcher on a build host:

```shell
coax/remote/remote.sh run -- bash psionic/check.sh --only robotics/experiment-policy
```

- Coax debug and release: **33 checks passed in each mode**, including their
  quantified cases and retained dependency checks.
- First root mutation qualification: **199 mutants, 198 killed, zero
  survivors, one marked equivalent**. The equivalent is the redundant
  physics-period cap's one-unit increase: physics ≤ control ≤ 1,000,000,000
  still excludes it. No other surviving mutant was waived.
- Actual native public API in debug `-O0` and release `-O2`, each plain and
  under **ASan/UBSan**: all four variants passed. These runs compare six
  independent joint-action rows, signed-zero inputs, complete target
  preservation, a complete late-refusal descriptor, and 35 nonfinite
  field/word combinations, including signaling/quiet NaNs and infinities.
- The exact example transcript passed in debug and release through
  `psionic/tests/robotics/check-experiment-native.coax example` after a
  correction to the example's void `main` declaration. Earlier component
  and native results were retained; they were not repeated for that fix.
- The extracted native-effects helper also passed the existing construction
  native qualifier's four debug/release and plain/sanitized variants.

The initial generic `coax/remote/check.sh only` attempt selected no Psionic
suites. It supplies no evidence here. The slice was registered with the
actual `psionic/tests/robotics/check.sh` dispatcher before the focused run.
`psionic/check.sh` is tracked nonexecutable, hence the explicit `bash` above.

During development, tests exposed a Boolean-versus-all-bits equality-mask
error, then missing boundary observations. The final checks cover minimum
and maximum cardinalities, direct shape refusals, matching-but-unsupported
signal/unit pairs, empty limits, adjacent period caps and requested negative
zero at both target boundaries. Compile-time checks use raw IEEE words for
the preflight predicate; actual nonfinite values are exercised natively.

## Limits and handoff

The identities are matched against an expectation supplied by the caller.
This does not authenticate a robot asset, derive a calibration or verify
source rights. The fixture tokens are not real model hashes. Inactive array
slots are copied by decoded admission and are outside active map matching.

No wire/stored shape is introduced. Do not serialize the Coax struct layout.
RB-01b must register the inert versioned record and independent golden bytes,
then implement its one codec and canonical handling of inactive storage.
The full RB-01 package remains open until that work passes.

This is CPU component evidence only. No Microduck dynamics, 61-channel
observation profile, policy training, hardware control, GPU target or
complete robotics workflow is qualified. No product adoption is recorded.
