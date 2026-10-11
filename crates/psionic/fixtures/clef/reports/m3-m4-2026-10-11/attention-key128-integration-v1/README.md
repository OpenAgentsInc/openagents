# Rejected attention key128 integration

The candidate fails the unchanged full-model raw-logit limit. Do not integrate
it or use the synthetic speed result as a product improvement claim.

The release build and 24 backend CPU tests passed. All four focused attention
GPU tests also passed. The Flash candidate answered all 40 corpus requests
and both long requests before cleanup. Against the frozen Stage6 corpus,
all 104 top answers and token counts match, but 35 questions across 24 requests
exceed the absolute 0.001 raw-logit limit. The largest difference is
0.004385113716125488, on `text-010`, question `日本0` (423 tokens).

The runner stopped before loading Stage6 for the long comparisons. The
candidate's 4k and 16k responses have 3,886 and 15,480 actual tokens, but they
have no fresh long baseline and establish no long-history parity. No 27B
text or image check ran. No latency measurement ran. The candidate stayed
opt-in throughout; the three product files were restored after this failure
was frozen.

## Checks and scope

- `flash-text-check/summary.json` contains all 104 raw-logit comparisons.
- `flash-text-check/candidate/rows.jsonl` contains the candidate's exported
  raw logits and coordinates. The pooled hidden tensors stay in the local
  archive; their hashes and sizes are in `publication-manifest.json`.
- All 42 requests and HTTP responses, the server log, memory samples, and
  owned-server cleanup receipt are included.
- `candidate.patch`, the three exact candidate source files, source manifests,
  binary hashes, build commands, CPU results, and four GPU test results are
  included. No executable or model weight is included.
- `failure-diagnosis.json` compares saved HTTP probabilities with the existing
  same-GGUF CPU goldens. This is diagnostic only: those goldens do not contain
  raw logits. Both variants match all 104 CPU top answers. Key128 has slightly
  higher maximum and mean probability errors; 50 question maxima improve and
  54 worsen. This does not replace or relax the failed raw-logit gate.

The candidate changes the softmax update interval from 32 to 128 keys. Its
f16 exponentials therefore use different running maxima, and its f32 sum/PV
reduction grouping changes. These are concrete numerical changes in the
source, not an established attribution of every observed output difference.
The next proposed experiment retains the exact 32-key softmax/PV updates
while calculating four QK score blocks together. It has not run.

## Resources and reproduction

The build used Rust 1.97.1, two jobs, the existing target directory, a 24 GiB
owned-process cap, a 36 GiB startup requirement, and a 12 GiB host floor.
Peak sampled memory was 25,372,033,024 bytes (23.63 GiB). The four GPU tests
used a 4 GiB cap and peaked at 138,068,640 bytes. The Flash server used a
36 GiB cap and peaked at 21,368,356,864 bytes. All owned groups stopped,
and no resource limit, sampler, or cleanup failure occurred.

These were shared-machine correctness runs. No quiet lease, desktop control,
service control, system setting change, or unrelated process control was used.
The server and reference model did not overlap. Source and build identities
are in the receipts; exported HTTP durations are not serving latency data.

The Python files and configs are the original infrastructure snapshots with
local path roots replaced by `<WORKTREE>`, `<SCRATCH>`, `<FLASH_ARTIFACTS>`,
`<TARGET>`, and `<OWNER_HOME>`. Restore those roots to a prepared scratch
layout before reproducing. The baseline captures, exact artifacts, and frozen
executables identified by the manifest must be staged separately. Follow
`VERIFICATION_PLAN.md`; do not silently raise its bounds or change its gate.
`publication-manifest.json` maps each original file hash to the scrubbed copy.
