# Clef 27B: independent same-Q4 runtime validation

This study compares the native runtime with the same public quantized weights.
It does not establish accuracy against the original BF16 backbone. That model
artifact-quality work is separate.

The unchanged artifact is `Clef-Q4_K_M.gguf`, 19,232,219,200 bytes, SHA-256
`9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`, from
`ggml-org/Clef-GGUF` revision
`d2aa75cb425ae922b5c3c15ef3157d417b7f7641`.

## Result and fixed acceptance

The full owned-process guard ended with status `failed`. The gate was fixed
before this run: all 40 text requests and 104 questions must be covered, all
104 top options must agree, and the largest probability difference must be at
most 0.02 in each native head mode. Raw logit differences are diagnostics for
this comparison. The separate 0.001 within-runtime optimization checks remain
unchanged.

```json
{
  "requests": 40,
  "questions": 104,
  "probability_limit": 0.02,
  "all_tops_required": true,
  "modes": {
    "staged": {
      "questions": 104,
      "top_matches": 103,
      "max_abs_probability": 0.009059166985287637,
      "passed": false
    },
    "fused-fields": {
      "questions": 104,
      "top_matches": 103,
      "max_abs_probability": 0.00905918188618815,
      "passed": false
    }
  },
  "passed": false
}
```

Both native modes matched 103 of 104 top options. The one disagreement was
`text-029`, question `日本1`: the reference gave `true` probability 0.493031923,
while the native modes gave about 0.50209110. The maximum probability difference
was 0.009059182. The probability bound passed; the required top-option agreement
failed, so this evidence does not close the runtime milestone.

The reference independently decodes the GGUF weights into float32, reverses the
recorded head permutations, and runs the pinned official Transformers operators
one decoder layer at a time on MPS. Requests run separately with batch size one
and no state cache. The official external head weights are loaded into float32
and run on the CPU. Original-BF16 trunk weights and reference head answers are
not inputs to this full comparison.

The native comparison uses the saved Stage6 binary
`77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`,
chunk size 512, fused attention, weight-shape GEMMs, Q4 FFN conversion on demand,
sequential DeltaNet, and no prefix or response cache. Both staged and fused-field
head outputs are checked. Earlier case-014 comparisons against older native
captures retain their original baseline; they are not relabeled as Stage6.

## Inputs and independent checks

The exact production parser, encoder, media admission, and tokenizer source
identities match the archived Stage6 identities. A small CPU-only encoder export
matched all 20,894 token IDs and every question span and option against the clean
official input records. The reference freshly encoded all 40 requests again.
Saved request bodies are used directly, including numeric spelling and newlines;
they are not rewritten before parsing.

Historical HTTP responses did not expose token IDs. The export establishes what
the matching source version computes from the same bytes; it does not turn that
into direct observation of the old HTTP process. Saved responses also pass type,
option, score, confidence, probability, token-count, cache, and artifact checks.
The HTTP consistency check reproduces native float32 maximum subtraction before
float64 exponentiation and summation.

The MPS acceleration pilot ran all 64 layers for case 014 and compared every
layer with the existing independent CPU float32 same-GGUF reference. Its final
fixed screen required relative hidden-row L2 at most 0.0001, raw logit difference
at most 0.001, probability difference at most 0.0001, and the same top option.
Those checks passed. Parameter copies and CPU/device input bytes were checked;
GPU allocations returned to a small baseline after each layer.

The first pilot report failed after the trunk completed because a new HTTP
checker compared integer question-type IDs with string labels. Its receipt and
executed source are preserved. A CPU-only recovery verified the saved final
layer's raw hash, applied final normalization once, matched the already-saved
normalized output byte for byte, and reran the same head and fixed gates. The
numerical functions remained unchanged. The recovery also checked actual
freshly encoded official integer question types across all 40 requests.

## Resource limits

This is correctness evidence on a shared machine, with a concurrent Cargo build;
it is not a latency benchmark. The full run used GPU, memory, and disk leases,
a 12 GiB sampled owned-process limit, 24 GiB available-memory start threshold,
12 GiB available-memory floor, 8 GiB MPS allocator limit, two CPU threads,
30-minute deadline, and 25 GiB disk floor. The guard only supervised its own
child process group. No quiet lease, application suspension, service change, or
system permission change was used. Guard records contain measured memory peaks,
cleanup results and command and source identities. The numerical failure occurred
before the guard's post-source block, so `preparation/full-source-recheck.json`
separately verifies all 152 source/input hashes and the unchanged GGUF file state
after cleanup. Execution completed in 558.57 seconds with a 10,437,940,744-byte
peak and no resource or cleanup errors.

Failed encoder build attempts remain in `encoder/`. The successful isolated
build compiled byte-identical production sources and used the matching Rust
LLVM linker; it did not rebuild or edit product code.

## Files and reconstruction

`full-reference/index.json` reconstructs the complete original receipt from
per-layer, per-case, and per-input files. `pilot-original-failure/` does the same
for the first pilot. Each split is lossless: reconstruction reproduced both the
sanitized JSON bytes and, with the local path parameters restored, the original
receipt bytes and hash. No evidence rows were dropped to fit the publisher's
one-megabyte file limit.

`manifest.json` records original and published hashes, byte lengths, split
indexes, and path-token meanings. Absolute account paths become named tokens;
model weights, raw tensor arrays, executables, and raw process-memory samples
remain outside this bundle. `source/` contains the reference and guard recipes,
including the old pilot source. Replace the documented path tokens with local
paths to reuse the recipes. Pinned artifacts, interpreter packages, repository
resource tooling, and original tensor files are required for execution.

The all-text runtime result does not add independent image-backbone coverage.
The separately recorded 14-image acceptance remains a distinct check.
