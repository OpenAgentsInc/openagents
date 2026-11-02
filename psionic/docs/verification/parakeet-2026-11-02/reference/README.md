# Actual NeMo reference for Parakeet — 2026-11-02

The reference runs **NVIDIA NeMo itself**, pinned to
[`5f15875acd0885ed92eadfee874ae87b0529ffa2`](https://github.com/NVIDIA-NeMo/Speech/tree/5f15875acd0885ed92eadfee874ae87b0529ffa2),
on the owned RTX 4080. This commit includes the corrected single-utterance TDT
symbol-cap behavior ([upstream fix](https://github.com/NVIDIA-NeMo/Speech/pull/16096)).
`nemo-record/record.json` retains all 723 strict safe-tensor bindings, loaded
module hashes, source/model/dataset pins,13 actual official-decoder adversaries,
and 29 utterance stage/token/frame/duration/text observations.

The original `.nemo` archive is read only as an uncompressed tar metadata
container through bounded HTTP Range requests. No pickle is read or loaded.
The YAML is parsed with `safe_load`; four explicit official NeMo classes are
constructed. The pinned HuggingFace safetensors keys are renamed into those
classes and **strict** `load_state_dict` checks every key and shape. Failed
initial alias/typecheck setup attempts are preserved in `actual-v1.log.gz` and
`actual-v2.log`; both stopped before audio inference. The successful run is
`actual-v3.log`. The first log is stored as deterministic gzip (mtime zero),
with its original uncompressed SHA-256 and byte count retained in the manifest;
decompression reproduces the original bytes exactly. This is an opt-in independent reference, never a runtime
package or dependency of the shipped library.

The isolated environment uses Python 3.12, PyTorch 2.12.0+cu126, NeMo 3.1.0+5f15875,
and CUDA 12.6 on the RTX 4080; `environment.txt` records the actual packages.
TF32 is disabled and float32 inference uses no CUDA graphs. Evaluation mode
already disables NeMo dither; the profile also explicitly sets dither0 and
pad_to0. OneLogger reported no exporters and collected no telemetry.

## Product comparison

The final complete product pipeline passes on **all 29 utterances**:
[`full-gpu-comparison.json`](full-gpu-comparison.json). Every normalized feature
and final encoder value is inside the tolerance frozen before inference:
`0.001 + 0.001 * abs(reference)`. The largest absolute feature difference is
0.00043448805809020996; the largest final encoder difference is
0.00004367530345916748. All emitted token IDs, frame IDs and text match NeMo
exactly. This covers the complete CPU frontend → Metal encoder → CPU TDT route.

The independently isolated [resident decoder record](resident-decoder/record.json)
also passes 29/29 on recorded NeMo encoder tensors, through one actual pinned
model open. It checks refusal of a second owner and preserves named public
information fields, tokens, JSON and caller text across four invalid-audio
calls. It does not claim full CPU encoder qualification and compares no C
padding or unused private workspace.

The first complete product attempt is retained in `failed-product-v1/`:
a missing mel-bank initialization in the one-call runtime produced zero
features and empty text, although the separate frontend unit had supplied its
own correctly initialized bank. The fourth clip refused after workspace reuse; its relocated bank overlapped
the previous raw-feature span. The runtime was corrected before the successful
v2 pass; no tolerance or quality gate changed. The failure demonstrates why
separately passing components cannot substitute for the actual resident path.

[Product quality and latency](product-quality-latency.md) and its
[JSON](product-quality-latency.json) name every language slice, source/model
identity and timing. The observed machine is an Apple M5 Max with 128 GiB on
macOS 26.4. The fresh engine open took 20.272077 s, with filesystem cache
uncontrolled. One resident pass processed 298.435 s of audio in 48.374912 s
of inference; whole-process time was 69.0402945 s including initialization
and recording. These are complete request times, not separately measured
stage durations, medians, key-release latency or a comparison across computers.

## Frozen input set and scoring

[`manifest.json`](../../../../fixtures/parakeet-reference/manifest.json)
was frozen before inference: SHA256 `ffe97a41feb987b8a6457b5c44908b54855803d78bf8abfe7c00de31fc6abaf2`.
It names `google/fleurs` revision
`70bb2e84b976b7e960aa89f1c648e09c59f894dd` and chooses one 1..20.47 second
utterance per supported language by the archive's order, without seeing any
model output. SourceWAV, TSV and PCM hashes, reference and selected member are
recorded individually. FLEURS 32-bit PCM is converted to 16-bit at 16 kHz with
explicitly disabled conversion dither. The English original 4 LibriSpeech clips
retain their own existing source/PCM pins and frozen 5% WER / 3% CER gates.

The multilingual metric is an **independent reference metric**: Unicode NFKC,
casefold, keep Unicode letter/number/mark categories and ASCII apostrophe;
other characters separate words. CER counts Unicode codepoints including
normalized spaces. This is **not** a claim that the old Coax `pk_normalize`
ASCII-only lowercase routine is equivalent to Unicode casefold.

This 25 utterance set reports finite smoke coverage. It is not a representative
accuracy estimate for any language or for all 25 languages. No multilingual
quality threshold was invented after observing the result. Numerical/token
parity and reported error counts are separate gates.

## Actual reference results

All 4 original English clips have 0/46 word errors and 0/258 character errors.
The table below is the actual NeMo reference on the selected FLEURS clips;
product parity is established only by separately named comparison receipts.

| Language | Word errors / words | WER | Character errors / characters | CER |
| --- | ---: | ---: | ---: | ---: |
| bg | 5 / 34 | 14.71% | 14 / 190 | 7.37% |
| hr | 1 / 17 | 5.88% | 2 / 99 | 2.02% |
| cs | 2 / 22 | 9.09% | 3 / 130 | 2.31% |
| da | 3 / 12 | 25.00% | 4 / 67 | 5.97% |
| nl | 1 / 15 | 6.67% | 1 / 101 | 0.99% |
| en | 2 / 19 | 10.53% | 4 / 99 | 4.04% |
| et | 5 / 19 | 26.32% | 6 / 146 | 4.11% |
| fi | 2 / 22 | 9.09% | 1 / 215 | 0.47% |
| fr | 0 / 19 | 0.00% | 0 / 110 | 0.00% |
| de | 1 / 18 | 5.56% | 1 / 121 | 0.83% |
| el | 3 / 20 | 15.00% | 3 / 142 | 2.11% |
| hu | 0 / 17 | 0.00% | 0 / 104 | 0.00% |
| it | 0 / 23 | 0.00% | 0 / 158 | 0.00% |
| lv | 5 / 17 | 29.41% | 6 / 105 | 5.71% |
| lt | 1 / 8 | 12.50% | 2 / 65 | 3.08% |
| mt | 2 / 21 | 9.52% | 4 / 138 | 2.90% |
| pl | 0 / 20 | 0.00% | 0 / 160 | 0.00% |
| pt | 1 / 36 | 2.78% | 1 / 225 | 0.44% |
| ro | 4 / 15 | 26.67% | 7 / 87 | 8.05% |
| sk | 1 / 16 | 6.25% | 3 / 88 | 3.41% |
| sl | 7 / 20 | 35.00% | 7 / 105 | 6.67% |
| es | 2 / 29 | 6.90% | 2 / 169 | 1.18% |
| sv | 3 / 12 | 25.00% | 6 / 71 | 8.45% |
| ru | 0 / 21 | 0.00% | 0 / 154 | 0.00% |
| uk | 0 / 14 | 0.00% | 0 / 100 | 0.00% |

## The frontend bug this comparison found

Actual NeMo valid feature length is `floor(samples/160)`; the centered STFT
physically returns one extra frame, which is masked to zero. Per-feature
normalization uses only the valid prefix: sample variance denominator n−1,
then 1e−5 added to the standard deviation. The historical Coax profile included
the extra frame in normalization; its English transcripts still matched, so
word-error checks alone did not detect the discrepancy.

`boundary-probe/record.json` records both hop-aligned and nonaligned input
lengths and validates compact feature storage on the real weights. Official
NeMo's encoder produces **bit-identical valid subsampled and final encoder
outputs** from physicalfeatures+validlength and from compactvalidfeatures:
56,080 samples gives 351 physical / 350 valid → 44 encoded;168,960 samples gives
1,057 physical / 1,056 valid → 133 physical encoded / 132 valid, while compact produces 132.
NeMo's masked convolution sequence masks before every subsampling layer and
again at its end. Its Conformer convolution masks padding before depthwise
convolution. These masks permit the compact product representation; merely
cropping an unmasked convolution does not establish the same result.

The same probe records exact special/unknown-token strings from official
SentencePieceTokenizer, including UTF8 hex so meaningful spaces survive review.
Control-like user-defined pieces are preserved by NeMo. Unknown ID 0 emits the
configured `" ⁇ "` surface, including surrounding spaces; this is separate
from a caller choosing HuggingFace's `skip_special_tokens=True`.

## Reproduction

The tools in `psionic/tests/parakeet_reference*.py` are independent oracle
exceptions. Run model inference only in an isolated reference environment on
the owned GPU, from the pinned inspected NeMo checkout. Never execute a
repository's downloaded scripts on the owner's Mac. `parakeet_reference.py`
records actual official modules; `parakeet_reference_probe.py` records
physical/valid and tokenizer boundaries; `parakeet_reference_compare.py`
compares public product artifacts and explicitly labels decoder-only coverage.
The production engine links none of NeMo, PyTorch, cuBLAS or Python.
