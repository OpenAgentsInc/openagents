# Independent qualification of the row-parallel decoder

The original pinned NeMo observations and frozen manifest are reused unchanged.
No NeMo/reference inference was repeated. The existing independent comparator
checks the new producer artifacts, using exactly the original elementwise
absolute/relative tolerance and exact token/frame/raw-text rule.

- `full-gpu-comparison.json`: all 29 complete feature/encoder/decoder pipelines
  pass. Actual product artifacts are in [gpu-corpus](../gpu-corpus/).
- `resident-decoder-comparison.json`: all 29 decoder-only calls on recorded
  NeMo encoder tensors pass. This does not qualify the CPU encoder. Product
  artifacts are in [resident-decoder](../resident-decoder/).
- `v1-v2-stage-identity.json`: the recorded normalized features and final
  encoder tensors are byte-identical across v1 and v2 in all 29 clips.
- `timing-comparison.json` and [the table](timing-comparison.md): independent
  timing arithmetic and Unicode NFKC/casefold rescoring. Every WER/CER count
  remains unchanged. Each FLEURS language still has only one smoke utterance.

The comparison JSON retains the actual `/tmp` producer paths and hashes.
`manifest.json` maps those identities to durable archived files and checks
that both copies have the same bytes. It also pins the comparator and timing
proof source. Producer/source/compiler/native images are qualified by the
separate [optimization receipt](../record.json), not inferred from these
output comparisons. The independent reference remains pinned in the
[baseline record](../../reference/manifest.json).

The displayed timing labels are clarified at publication: reported recognition/inference
excludes API-lock wait, snapshot publication and caller-output copies; it is not
whole-call wall time. Numeric observations and raw producer records are unchanged.

The timing proof is computed by `psionic/tests/parakeet_reference_timing.py`
from the unchanged v1 score table, actual v2 stdout and passing full comparison.
The two versions each have one ordered resident pass; caches, scheduling and
first-request Metal warm-up are uncontrolled. These observations do not
establish a steady-state distribution, stage attribution, key-release latency
or the 300 ms dictation target. Baseline records are preserved separately.
