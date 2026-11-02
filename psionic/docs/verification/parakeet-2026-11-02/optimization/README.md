# Exact row scheduling for the resident decoder

V2 keeps `psionic-tdt-v1` arithmetic: compensated binary64 sums, the original
bias and product order within each output, and one f32 store. It changes which
CPU computes each complete dense output row. Coax owns row ranges, validation,
dot products and recurrent updates. C owns the persistent pthread effects.
There are at most seven helper threads plus the calling thread. Every gate
finishes before the LSTM changes its state. A partial pthread startup uses the
actual successfully created count; close joins every helper. Dispatch, startup
and shutdown are serialized. The public info struct is unchanged; the additive
`psionic_parakeet_decoder_threads` query reports the actual count.

The complete V1 evidence and source archive remain in `../runtime/` at main
`e13a571981`. No original reference inference was repeated. This folder records
one changed-product run through the same 29 frozen clips, then a separate
decoder-only run on the original NeMo encoder outputs.

| Recognition timer | V1 | V2 |
| --- | ---: | ---: |
| Requests 2–29, mixed-length cohort median | 1,554.992 ms | 840.241 ms |
| Requests 2–29, nearest-rank 95th percentile | 2,962.937 ms | 1,453.952 ms |
| All 29 clips, inference seconds / audio seconds | 0.162095 | 0.091140 |
| 5.025-second English clip | 1,360.674 ms | 559.329 ms |

The observed median ratio is 1.85. This was one ordered pass on the same Apple
M5 Max, with global CPU load and filesystem caches uncontrolled. The first clip
includes lazy GPU weight upload; the warm rows omit that clip. The public
recognition timer starts after acquiring the API lock and ends before
last-success snapshot/output publication. It includes frontend, encoder,
decoder and their checks. The corpus harness's separate total timer includes
inspection and file output. Neither is a microphone or key-release latency
measurement. Model-open times were 20.272 and 15.862 seconds; those observations
do not establish a cold-load improvement. The 300 ms five-second recognition
target is still unmet.

Qualification started with independent serial-vs-parallel checks. Four tiny VM
seeds compare every token, frame ID and workspace f32 bit. Native checks use
nontrivial 65/64/128 widths, actual row dispatch, six seeds and six thread
configurations. Three seeds force 21 zero-duration emissions, exercising every
recurrent update and the per-frame symbol cap. O0, O2, fatal ASan/UBSan,
production floating-point flags, partial pthread startup, repeated close and
concurrent callers pass. The pure partition/count/thread policy kills all 92
root mutants. No mutation waivers or legacy whole-model mutation reruns were
used. Failed early mutation reports remain in `native/`; final report is v5.

An early concurrent *test client* exceeded Apple's 512 KiB default pthread
stack with its 0.8 MB local fixture. The oracle now explicitly allocates a
4 MiB client stack; product row workers contain no weight arrays. Failed
receipts remain. Whole current API CPU-only linking/refusals and the additive
query pass O2 and fatal ASan/UBSan. Leak detection is disabled on macOS; these
records make no leak-check claim.

The independent comparison passes all 29 full GPU transcriptions and all 29
decoder-only transcriptions against frozen NeMo tokens, frame IDs and text.
All feature/final-encoder tensors remain inside the original tolerance and are
also byte-identical to V1. Unicode WER/CER counts are unchanged. See
`reference/manifest.json`, `reference/full-gpu-comparison.json`,
`reference/resident-decoder-comparison.json` and
`reference/timing-comparison.md`. Raw product tensors/tokens are in
`gpu-corpus/`; decoder-only tokens are in `resident-decoder/`.

`record.json` pins the built products and build invocation. `source-pins.json`
and `source-v2.tar.gz` preserve the complete Coax closure and affected native
sources. `producer/` contains the actual generated C, headers and Metal.
`native/record.json` describes focused native profiles and their exact scope.

Further speed work should measure the frontend/encoder/decoder separately,
then consider Coax-generated device decoder rows and gates, batching independent
decoder projections, and reducing CPU/GPU synchronization. A different
reduction or storage precision needs a separately qualified numerical profile.
Authenticated model reuse can improve fresh-process startup without skipping
pin validation. Incremental audio/cache/endpointing and actual microphone
key-release measurement belong to the streaming and Coder integration work;
this change retains bounded whole-utterance semantics.
