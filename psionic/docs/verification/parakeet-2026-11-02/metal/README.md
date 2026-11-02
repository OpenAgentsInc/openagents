# Parakeet Metal encoder component — 2026-11-02

The [record](record.json) qualifies the Coax FastConformer GPU arithmetic,
its bounded memory plan and the Metal effects adapter on an Apple M5 Max
(macOS 26.4, 128 GiB). It keeps the actual compiler and compiled source
identities. No executable or checkpoint is committed.

- Four new device checks pass: channel/group/padding convolution, feature
  layout, GLU halves and relative-attention positions. Inherited unchanged
  Whisper checks were not repeated.
- The final planner authority suite passes 116 checks; stock root-only
  mutation kills all 70 sites, with zero survivors and no equivalent waivers.
  Earlier raw reports retain the two genuine missing/needless distinctions
  and a failed nonfinite test fixture. The corrected boundary covers head
  width 129, and the planner no longer constructs an otherwise unused batch
  plan. Runtime nonfinite fixtures use the existing `cf_nan` helper.
- Six tiny native comparisons cover one, seventeen and thirty-three feature
  frames, subsampling and one full Conformer layer. Maximum absolute error
  against the compensated CPU reference is below 3.9e-7.
- The final ASan/UBSan host run repeats that target profile with all host,
  generated Coax reference and Objective-C effects objects instrumented.
  Wrong weight extents and nonfinite inputs refuse without publishing output;
  a successful call preserves the caller's unused output tail. Fatal sanitizer
  options are enabled; platform leak detection is disabled. Raw stdout and
  stderr are retained.
- Original checkpoint arithmetic is compared on the shortest committed
  LibriSpeech clip, through subsampling and the first two encoder layers.
  Maximum errors are 0.008789, 0.0002594 and 0.0002289; GPU prefix times are
  12.14, 9.41 and 12.91 ms, excluding load/compilation. The fixed harness
  tolerance is 1e-3 times one plus the CPU slice's maximum magnitude.

Those original slices use the chapter's earlier same-input CPU features.
They establish GPU arithmetic agreement, **not** NeMo-produced tensor parity
or the corrected valid-length frontend. Current NeMo parity, compact-feature
integration, complete transcripts and end-to-end latency belong to the sibling
reference and root integration records. They are not inferred from these slices.
The upstream physical-versus-compact probe separately establishes that compact
valid feature rows are equivalent to NeMo's masked encoder for valid output
frames; the GPU API therefore keeps its existing frame-count contract.

The GPU schedule and arithmetic are Coax. The C adapter allocates shared
buffers, binds fixed kernel arguments, serializes calls, waits and copies the
successful result. Weights/work stay resident until close. A requested GPU
failure is explicit, with no CPU fallback. Fast math is disabled. This f32
matrix/online-softmax profile uses tolerances, rather than bitwise equivalence
to the compensated CPU profile.

To reproduce the native tiny comparison after building the resident engine:

```shell
psionic/platform/parakeet-build.sh /tmp/parakeet-device-check
coax/bin/coax c psionic/tests/parakeet-device-reference.coax --library --release --header /tmp/parakeet-device-check/parakeet-ref.h > /tmp/parakeet-device-check/parakeet-ref.c
cc -std=c11 -O2 -ffp-contract=off -fno-fast-math -I/tmp/parakeet-device-check -Ipsionic/platform /tmp/parakeet-device-check/parakeet-ref.c psionic/tests/parakeet-device.c /tmp/parakeet-device-check/parakeet-metal.o /tmp/parakeet-device-check/metal-host.o -framework Metal -framework Foundation -lpthread -lm -o /tmp/parakeet-device-check/check
/tmp/parakeet-device-check/check
coax/bin/coax test psionic/lib/parakeet/plan.coax --library
coax/bin/coax mutate psionic/lib/parakeet/plan.coax --library --root-only --jobs 4
```

Compile the reference as **one Coax translation unit**, since it already loads
the production scheduler. Linking another generated runtime/fast object would
duplicate the shared public tensor exports. The compressed generated C, headers
and MSL preserve the exact programs used for the recorded sanitizer target.

The optional `MODEL RANGES PCM` arguments run only the recorded original-weight
layer slices, using ranges emitted by `parakeet_oracle.py ranges`. They do not
run the whole CPU model. Unchanged passing observations should be reused rather
than rerun merely to create another receipt.
