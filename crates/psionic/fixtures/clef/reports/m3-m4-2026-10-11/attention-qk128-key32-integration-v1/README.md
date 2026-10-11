# QK128 scoring with unchanged key32 updates

The compiled candidate passes all required correctness checks. Flash and 27B
both match Stage6 with exactly equal raw logits and pooled rows for all
104 corpus answers and six long-history answers per model. All 220 top
answers match. The image suite and typed refusal checks also pass. These
export runs establish correctness on the retained cases, not serving speed.

Only three backend attention files change. The opt-in fused kernel scores
128 keys together, then keeps each original 32-key softmax and value-product
update, including half rounding and reduction order. Threadgroup storage is
25,600 bytes. The staged default stays unchanged. The earlier full key128
candidate remains rejected and is documented separately; it was not retried
or relabeled as this candidate.

## Checks

- Release server and backend test builds passed with Rust 1.97.1 and two jobs.
  All 25 nonignored Clef CPU tests passed. The four focused attention GPU
  tests passed separately; the remaining ignored GPU tests were not run.
- Each model completed 42 candidate requests and two fresh Stage6 long
  requests. The 40-request corpus comparison has 104 answers; the 4k/16k
  comparison has six. Both comparisons have maximum raw-logit difference 0,
  under the unchanged 0.001 limit, and all pooled-row hashes match.
- Long requests contain 3,886 and 15,480 actual tokens. Every compared token
  count matches, cached token counts are zero, and no state is truncated.
  Native token IDs are not exported: their identity is inferred from exact
  request bytes and 157 unchanged serving/encoder source files.
- Six image requests produce 14 answers with matching expected and reference
  tops and token counts. Maximum probability difference from the existing
  BF16 image reference is 0.0017794304477648515. All ten HTTP refusal cases
  pass under the existing image gate; this is separate from the raw-logit
  attention comparison and the unresolved model quantization-quality work.

All attempts in this integration succeeded on their first run. The prior
failed key128 experiment is preserved in its separate published bundle.
Full request bodies, responses, raw logits, index coordinates, source audits,
resource samples, cleanup receipts, and the exact patch are included here.

## Build and resources

The server build took 701.24 seconds, and the backend test build took 15.95
seconds. The guarded build finished in 718.88 seconds with 23.674 GiB peak
sampled owned memory, below its fixed 24 GiB cap. At least 81.953 GiB remained
available. The four GPU tests took 1.14 seconds; their guard finished in
1.435 seconds with 134,234,832 bytes peak accounted memory.

Flash used a 36 GiB cap, 48 GiB startup requirement, and 12 GiB host floor;
its two owned servers peaked at 21,405,450,240 bytes. 27B text and images used
52 GiB, 64 GiB, and 12 GiB respectively. The maximum text peak was
44,655,869,952 bytes; images peaked at 45,371,916,288 bytes. All owned groups
stopped and all leases released, with no sampling, resource, or cleanup
errors. Some macOS signal-0 probes returned EPERM after leader exit; the
receipts separately verified zero group members and successful cleanup.
No quiet lease, application, service, or host setting was changed.

These are shared-machine correctness runs. Their timings include row export
and are not latency measurements. Fresh paired latency work requires its
own scheduled window and evidence.

## Identity and reproduction

The candidate server SHA-256 is
`f4def388b4b6622fef9023c2616639c215829d7a519359eb64d1dbca9fdfa31f`.
The test binary SHA-256 is
`5606e487c058db9680210ec857d5ea871cdb360fb8b2a493b028f6adac0dfcdc`.
Stage6's server SHA-256 is
`77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`.
The source freeze remains the exact 2,307-file manifest prepared before
compilation; later documentation-only commits do not relabel it.

See `VERIFICATION_PLAN.md` and the frozen scripts/configs for the order and
bounds. Local roots are replaced by `<INTEGRATION>`, `<SCRATCH>`,
`<WORKTREE>`, and `<OWNER_HOME>`; restore them before reproducing. Original
and scrubbed hashes are mapped in `publication-manifest.json`. The large
build memory log is split at complete lines. Binaries, model weights, and
pooled hidden tensors are omitted; their sizes/hashes remain recorded.
