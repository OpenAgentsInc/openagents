# Attention key128 verification

Only the opt-in `fused-tensorops` key tile changes from 32 to 128. The staged
attention default stays unchanged. The prototype passed its independent f64,
full-output, tail, guard, causal-position, and repeat checks before integration.
This plan tests the compiled product before making a full-model claim.

## Frozen inputs and binaries

`source-manifest.json` and `candidate.patch` freeze the candidate. The build
checks the manifest before and after compilation and copies both executables
only after the release server build, backend test build, and nonignored Clef
CPU tests pass. `binaries.json` records their SHA-256 hashes. Rust 1.97.1,
two Cargo jobs, and the existing agent1 target are fixed.

Stage6's reference server is `../stage6/psionic-openai-server`, SHA-256
`77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`.
`baseline-receipts.json` freezes the existing 40-request / 104-answer fused-field
captures, every request and response, raw export index and rows, source
manifests, and the 4k/16k latency-request input files. No old timing is reused.

The source audit verifies all 157 serving files match Stage6. Other changed
product files are the explicitly disabled block-Delta implementation/tests and
generated CUDA stubs, which do not execute on Metal. Both new server runs set
Delta sequential, fused-fields, fused-tensorops, weight-shapes, prefix-cache bytes 0,
batch requests 1, and two CPU threads. Flash uses `resident-f16` and chunk size 2048;
27B uses the same Q4_K_M artifact, external official BF16 head, `q4k-on-demand`,
and chunk size 512. The gate does not compare the Q4 model with a BF16 trunk.

## Checks, in order

1. Run `gpu_checks.py`: exactly the four ignored fused-attention backend tests.
   These cover dense/staged agreement, 127/128/129 boundaries, causal masks,
   changing softmax maxima, repeated calls, and nonaligned chunks. Keep the
   process under 4 GiB, require 16 GiB available before startup, retain 12 GiB of
   host headroom, and stop at 180 seconds. No unrelated kernel tests run.
2. Run `compare_attention.py flash-text-config.json`, then the 27B config after
   cleanup. Each executes the candidate's 40 fixed corpus requests plus the
   existing 4k/16k requests (42 requests/110 answers), followed by only the two
   long requests on fresh Stage6 (2 requests/6 answers). That is 44 requests per
   model and 88 in total. The saved corpus is reused only after all receipt,
   request, source, binary, artifact, configuration, and export checks pass.
3. Run `check_images.py`: candidate 27B, six images/14 answers and 10 typed HTTP
   refusals using the existing image acceptance runner. Keep the existing
   image/BF16-reference probability gate unchanged. Verify the exact same
   GGUF, head, and vision manifest, and record the candidate binary hash.

The text checks require finite raw logits with maximum absolute difference
at most 0.001, 100% raw-logit and HTTP top agreement, the exact question/option
coordinates, identical actual token counts, no missing/duplicate answers,
no truncation, and zero prefix/full-answer cache hits. Pooled hidden-row
changes are diagnostic, because attention rounding can change those rows.
Native token IDs are not exported. Their equality is inferred from identical
request bytes and unchanged encoder/serving source, not proved by counts.
The source numbers in fixture JSON retain their exact spelling during model
substitution. Long requests use the existing fixed bodies; only the model
name changes for 27B. Their actual counts must fall in 3,500–5,000 and 15,000–17,000.

The text runner stops after a corpus-gate failure and preserves the failed
results. It does not load Stage6 again in that case. Any long-history failure
also fails the candidate. No tolerance, cap, deadline, or mode changes are
automatic, and no failing result establishes acceptance.

## Resources and scheduling

| Check | Process cap | Startup available | Host floor | Startup deadline | Request deadline | Server deadline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Flash text | 36 GiB | 48 GiB | 12 GiB | 180 s | 300 s | 1800 s |
| 27B text | 52 GiB | 64 GiB | 12 GiB | 600 s | 300 s | 1800 s |
| 27B images | 52 GiB | 64 GiB | 12 GiB | 600 s | 300 s | 1800 s |

Each server owns a process group and a fresh loopback port. The shared guard
samples the sum of each member's maximum of RSS and physical footprint;
this is a sampled refusal/cleanup bound, not an allocation reservation.
Memory availability uses free + inactive + speculative pages from `vm_stat`.
The text runner retains the existing shared-machine load thresholds 150/180.
It fails closed rather than changing thresholds when the host is busy.

All GPU checks require the GPU and memory leases, forbid quiet leases, and
wait for the current reference model to release its group and leases. No
model runs overlap each other. The current 24 GiB build must finish and clean
up before these checks. No desktop, application, service, or system settings
are changed. Root review and scheduling handoff are required before launch.

These are correctness checks with row export. A later latency comparison must
use fresh paired runs; none of these durations are product latency evidence.

## Preparation checks already complete

The comparison self-tests reject logit, top, token, coordinate, nonfinite,
model-identity, duplicate, missing, and malformed-export failures. A separate
case proves pooled-row inequality alone does not fail the attention gate.
Both baseline metadata audits pass. The kernel peer reviewed the comparison
harness without finding a blocker. The product source and build are still
frozen; full-model and GPU results are pending.
