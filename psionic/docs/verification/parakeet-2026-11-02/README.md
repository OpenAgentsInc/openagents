# Parakeet chapter 153 qualification

The bounded resident original-weight Parakeet v3 recognizer works through
Psionic's own Coax frontend, Metal FastConformer and CPU TDT decoder. It has
a pinned loader, static native API and explicit `openagents transcribe`
backend. [Chapter 153](../../../book/ch153-transcribe-with-parakeet.md) explains
the implementation; [the guide](../../parakeet.md) gives installation and use.

This record qualifies that profile, not Coder's microphone UI or a streaming
recognizer. Inputs are mono 16 kHz f32, 320..327520 samples. GPU failure is an
explicit refusal. The legacy compensated CPU teaching results remain historical.

## What actually ran

| Evidence | Observation |
| --- | --- |
| [Independent NeMo reference](reference/manifest.json) | Pinned official modules, all 723 strict safetensors bindings; no pickle execution. Four original English recordings plus one frozen FLEURS utterance for each of 25 languages. |
| [Complete product comparison](reference/full-gpu-comparison.json) | 29/29 feature and final encoder tensors within elementwise `0.001 + 0.001*abs(reference)`; all token IDs, frame IDs and raw text exact. Largest absolute feature/encoder gaps 0.00043449 / 0.00004368. |
| [Quality and timing](reference/product-quality-latency.md) | English 0/46 word errors and 0/258 character errors. Every multilingual smoke score is reported without a population-accuracy claim. One ordered pass: 298.435 s audio / 48.375 s inference; fresh engine open 20.272 s, filesystem cache uncontrolled. |
| [CPU decoder](reference/resident-decoder/record.json) | 29 recorded NeMo encoder tensors decoded exactly through one resident engine, plus public refusal checks. This does not rerun the full CPU encoder. |
| [Reference boundary probes](reference/boundary-probe/record.json) | Actual physical/valid feature lengths, compact versus masked encoder equivalence, and SentencePiece unknown/control surfaces. Thirteen separately recorded forced-logit TDT cases check cap/blank/duration behavior. |
| [Resident API](runtime/README.md) | O0/O2 and fatal ASan/UBSan boundary checks; one-model ownership, pinned mapping, atomic publication and cleanup. Unsupported macOS LeakSanitizer startup failure and same-binary continuation retained; no leak-sanitizer claim. |
| [Late failure snapshot](snapshot/record.json) | Clip A succeeds; different clip B computes but refuses a one-byte text buffer. Caller text and all previous A info/token/frame/JSON/feature/encoder bytes survive. Short inspection buffers refuse atomically; a later B success replaces the snapshot. |
| [Metal encoder](metal/record.json) | Small native and sanitized GPU probes plus original-weight prefix slices; Coax kernels and scheduling. New planner kills 70/70 emitted root mutants. |
| [Frontend/text checks](frontend/) | Valid/physical length policy 21/21, frontend 63/63 and text 43/43 emitted root mutants killed. Earlier surviving runs remain alongside final checks. |
| [CLI](cli/) | 13 supported/refused transport cases, a 17/17 mutation policy observation, and actual complete recognizer execution from a foreign working directory with flags before the domain and a WAV path containing spaces. |

[record.json](record.json) indexes component evidence and source identities.
Each native/reference component retains its own actual producer hashes and
snapshots. Tests used existing Coax compilers; no compiler implementation or
analysis budget changed. The CLI's automatic build dependency list was expanded
after its real inference witness; the dispatch and inference bodies are identical.
[The rebase audit](publication/rebase-source-audit.json) checks that all retained
component source pins remain identical on current main. Both agents' CLI flag
additions are preserved; the combined CLI is rebuilt and its transport checks rerun.

## Bugs found on the real integration

The generic centred STFT stores one extra column. NeMo excludes that column
from per-feature normalization. The new model-specific frontend uses the valid
prefix and compacts it for the encoder. The generic spectral and legacy teaching
profiles are unchanged. Four error-free transcripts alone had missed this gap.

The first full GPU attempt then found an uninitialized mel bank in the resident
composition: fresh storage produced zero features, and reused storage could
contain old values. The regression fails before bank initialization and passes
afterward with nonconstant audio and poisoned/reused workspace. The failed run
is preserved under [failed-product-v1](reference/failed-product-v1/).

SentencePiece's unknown ID renders as `" ⁇ "`, including spaces; controls remain
literal. Independent recorded tokenizer cases now bind that product renderer.
New model admission checks also replaced an arbitrary invalid-number sentinel
with an explicit parse result; initial surviving mutation runs remain recorded.

## Remaining work

Keep the model resident when integrating Coder. This baseline does not meet the
proposal's 300 ms release goal: the original four English API calls take roughly
743–1541 ms, without microphone/key timing. Measure stages before making a
latency claim. Candidate improvements include independent decoder rows in a
persistent worker pool, decoder projection reuse, cached FFT twiddles, fused
encoder launches and smaller uploads; each must retain arithmetic/reference checks.

Coder still needs opt-in capture, timestamped pre-roll/tail, silence admission,
draft insertion, cancellation, long-utterance segmentation, repository vocabulary
rescoring and real release-latency evaluation. Chapter 154's seeded cache-aware
stream is separate from this full-attention checkpoint. No microphone was opened
and no owner action is required for this offline recognizer qualification.

The large unchanged TDT/Parakeet mutation gates and whole-model CPU transcription
were not repeated. New and changed code was tested in focused slices. Raw failed
attempts are observations, never relabeled as passes or waived as equivalent.
