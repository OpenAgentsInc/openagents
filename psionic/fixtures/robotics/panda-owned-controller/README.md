# Paired controller at held observations

This is a NEW static-state compatibility proof: the unchanged Coax controller
consumes the fifteen original SDK PcSamples and the corresponding already
qualified owned samples. It does not run the sample generator again, step the
plant or run a policy. The owned input bundle is an exact data alias of the
qualified debug output, with its source/record identity frozen before execution.

`experiment.json` freezes fifteen paired contexts, three7-value actions,
twenty-five held controller calls per pair, source default home, all state
updates and comparison budgets. Each action starts from a fresh `pc_reset` on
both sides. The controller sample stays constant. `new_goal` is true only on call0;
subsequent calls retain that goal. The unchanged controller still advances its
internal gripper target once per call. These are call counts, not simulated
elapsed time, jaw trajectories or a500Hz/real-time qualification.

The actions are zero, a dense vector, and a vector containing inclusive±1
pose components plus two saturated components. The source default-home vector
is independently stated in the profile and checked byte-for-byte against
`pc_default_home`. Status, readiness, torque-clipping counts and pose-action
clipping counts must agree exactly. Frozen absolute bounds are torque2e-8,
gripper2e-12 and every21F64 state field1e-10, with relative tolerance zero.

Five refused siblings start from fresh reset: reset with negative mass,
step with negative mass, a NaN action component, a short action span and a
not-ready state. Their exact expected statuses are mass,mass,number,shape,state.
Every failed call preserves a byte snapshot of its initialized state; returned
torque/gripper/count fields are canonical zero. Normal pair comparisons
serialize logical fields and do not assume struct padding is stable on value
copies. Actual singular/refusal outcomes remain in status histograms; success
is never inferred merely from matching two failures.

`qualification.json` records the first passing debug/release/fail-fast ASan+
UBSan execution of this NEW harness, reusing byte-identical qualified Coax
C/header inputs. Each mode has2,490 canonical serialized events and86,679
assertions. There are45 successful reset pairs,1,125 successful held-step
pairs, and75 refused sibling pairs (150 actual refused calls), all atomic.
SDK-versus-owned maximum torque difference is6.53699e-13; gripper values agree
exactly; the maximum state-field difference is1.99840e-15. Status/readiness/
clipping decisions agree exactly. Complete transcripts match byte for byte
between all three named Darwin-arm64 scalar-libm profiles.

The ledger covers7torque+2gripper+21state values after every reset and held
call, plus all refused siblings. Ordinary sample/expected values and decoded comparison fields are checked
finite, with exact transcript extents, before comparison. The intentional NaN
action is a declared refused fault input.
Decoder self-checks reject a NaN or either infinity in the last state field
and both truncated/extended transcripts. The complete event inventory checks
sample, action, call, side and kind before field maxima. No paired event can
be silently skipped by a truncated zip.

Run only this new harness with the recorded qualified sample artifact directory
(containing its exact record, debug/release generated Coax C/header and debug
sample output), using a fresh result directory:

```shell
python3 psionic/tests/robotics/panda_owned_controller_reference.py --qualified-samples /tmp/panda-owned-sample-native-first --output /tmp/new-paired-controller-proof
```

The SDK samples are the public original matched records from
`../panda-inverse-dynamics/`; their original capture is reused. The owned sample
bundle contains2,160F64 words,17,280bytes, SHA256
`1443b1519e20474a00463a29b423e335e82312a567b3f7913eab284de0746ba7`.
Input/reference/source hashes and the prior generator proof are explicit in
the profile. The transcript schema has10I64 metadata fields and30F64 logical
values per event; it is test tooling, not a published robot trace protocol.

The fifteen contexts contain repeated q/v inputs and are not fifteen independent
physical states. Three actions do not turn them into45 new states or episodes.
This demonstrates controller-output compatibility for the frozen held-state
corpus only. It establishes no fresh observation timing, integration/contact,
closed-loop stability/task success, physics replay, hardware safety, generic
frame provenance, compiler typed clocks or `#bounded` result. No SDK, neural
model, physics step or old controller gate runs in this qualification.

The combined portable gate is
`psionic/check.sh --only robotics/panda-owned-sample`. It runs the sample
constructor's native gate once, then verifies the newly generated C/header and
output bytes against this frozen profile. A metadata-only alias cache uses the
exact committed historical receipt and owned bytes for the paired harness; it
requires no original private directory. Differing C/header/output bytes fail
explicitly and require a new profile. [Alias guard evidence](../panda-owned-sample/cache-alias-qualification.json)
checks poisoned C/header, receipt, sample, mode and extent inputs without
rerunning the generator or paired harness during publication.
