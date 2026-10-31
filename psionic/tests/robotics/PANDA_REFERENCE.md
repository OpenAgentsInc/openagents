# Source-framework policy baseline

This opt-in development tool runs the original learned weights through the
pinned installed Transformers backbone and the independently authored Torch
action head from `groot_vla_reference.py`. Learned arithmetic runs on MPS in
F32. The resident model keeps its weights across requests and collects no
hidden-layer capture hooks. Python is never linked into the product policy.

The reference shares Coax raw-image preparation, tokenizer, state
normalization, finite admission, action decoder, chunk supervisor, controller,
clock checks and trace codec with the native loop. Transformers computes its
own vision, positions and text features. The C case-loop body is shared through
`panda_loop_effects.h`; native callers retain their original CLI, profile,
schema and fields. Reference callers use a distinct profile and ledger schema.

Build the development bridge, optionally reusing already qualified generated C:

```shell
psionic/tests/robotics/build-panda-reference.sh /tmp/panda-reference-build QUALIFIED_BUILD
psionic/tests/robotics/check-panda-reference-native.sh ORIGINAL_PST /tmp/panda-reference-native QUALIFIED_BUILD
```

Run one frozen case per command, with an existing pinned reference environment:

```shell
psionic/tests/robotics/run-panda-loop-reference.sh \
  --library /tmp/panda-reference-build/panda-reference.dylib \
  --weights ORIGINAL_GR00T_SHARDS --cosmos ORIGINAL_COSMOS_CONFIG \
  --pst ORIGINAL_PST --noise FROZEN_NOISE_F32 --pairs ORIGINAL_EVALUATION_PAIRS \
  --output NEW_PRIVATE_DIRECTORY --case 020
```

The diagnostic subset is the three previously timed-out cases 020/030/040.
Each keeps its original paired XML/init_state, nominal controller home,
explicit fixed noise, 8-action execution horizon and 720-tick limit. This is a
posthoc comparison under the owned fixed-reset/F32 profile, not a checkpoint
training holdout, NVIDIA BF16 wrapper or published benchmark.

Before those cases, `--qualify ORIGINAL_152_CAPSULE --raw-frame RAW_FRAME_NPZ`
runs the changed resident integration against the already recorded genuine
first-frame result. It checks every Coax-prepared word and all 5280 normalized
values under the unchanged declared budget. The measured integration result
was bit-identical; an independent CPU adapter audit also matched all 112 decoded
values and 16 gripper decisions. Existing references and gates are reused.

Every request privately saves its prepared observation and normalized output
for SAME-observation diagnosis. These capsules are never public payloads.
Inference wall durations include this archive work. Reference ledger
`gpu_seconds` is null because device time is not instrumented; it must not be
interpreted as zero or replaced by wall time. Simulation pauses during
inference, so 20Hz is logical simulation cadence.

The native FFI checks poison every callback output word, exercise callback
failure/nonfinite/partial writes and verify atomic refusal. A one-tick native
ASan integration through the shared loop reproduced the original decoded 112,
trace seed and first 25 physical records exactly, with exact physics replay.

Compare all completed outcomes without replacing the original records.
Independently evolving trajectories cannot localize neural numerical drift;
an outcome difference requires comparison at the SAME actual observations
before assigning a cause. Unexpected provider/SDK/controller/clock failures
abort without a completed ledger. Existing output directories are refused.
