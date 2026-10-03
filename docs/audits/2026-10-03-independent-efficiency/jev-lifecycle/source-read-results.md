# Batched source-read results

Batching immutable Git blob reads reduced local warm source-assembly time on all
three retained development inputs. The complete context and both packed outputs
were unchanged, except for elapsed-time fields. This is a deterministic
implementation improvement; it makes no new Jev or native coding performance
claim.

| Task | Original median | Batched median | Reduction | Batched range, five observations |
| --- | ---: | ---: | ---: | ---: |
| Alpha | 2.052 s | 0.784 s | 61.8% | 0.760–0.823 s |
| Beta | 1.335 s | 0.437 s | 67.2% | 0.406–0.468 s |
| Gamma | 2.123 s | 0.363 s | 82.9% | 0.353–0.369 s |

[Raw timing observations](source-read-timing.json) retain every observation from
the final serial measurement, source and test hashes, warmups, and output hashes.
The [prospective amendment](source-read-amendment.md) defines the unchanged
selection policy, equality gate, and timing procedure. The original source is
preserved in [context-at-round-one.py](../../../../bench/jev-lifecycle/context-at-round-one.py).
Its SHA-256 remains
`4e4bc6acf510bb852af38bc47e1bc20f66e3d3a2817e00ff1dcb4762a2d20acc`.

## What changed and what was checked

The admitted-file loop now uses two Git processes for the complete batch: first
check immutable blob identities and sizes, then read the bounded blobs. Previously
it used two processes per source file. Package-manifest discovery and targeted
source reads use their original code paths. No ranking rule, source span, catalog
suggestion, candidate limit, or byte budget changed.

Fifteen focused tests passed in 7.123 seconds under Python 3.13. New tests compare
batch and single reads for empty files, Unicode, and duplicate blob contents;
refuse oversized content before fetching it; and reject malformed identity,
framing, size, UTF-8, or trailing data. Existing admission, provenance, partial
source, and packing tests also passed.

The [benchmark driver](../../../../bench/jev-lifecycle/benchmark_context.py)
compared all non-timing context fields and both deterministic and recorded-Jev
packs on every observation. This includes candidate IDs, order, text, catalog,
omissions, source hashes, and selected payload hashes. The outputs also match the
original live-preparation candidate and pack hashes. Recorded Scores were reused;
no model was called.

## Measurement boundary and retained setup failures

The final measurement ran serially on the local Mac with Python 3.13. It performed
one warmup per implementation and then five pairs per task, alternating which
implementation ran first. Assembly time includes Git reads, validation, selection,
and canonical index hashing. It excludes Python startup, input JSON loading,
packing, index construction, and output writes. The operating-system cache was
not reset. Subsecond assembly does not imply a subsecond complete preparation
pipeline.

An initial test command used the system Python 3.9 and failed to import `tomllib`;
the successful command used the already-installed Python 3.13. Two initial timing
launches may have overlapped after the first process handle was not retained.
The [surviving provisional artifact](source-read-timing-provisional.json) is kept
and excluded from the result above; the earlier artifact was not independently
retained. The final run started only after confirming no benchmark process
remained. No original preparation or model-call record was replaced.

## Resource tradeoff

Batching buffers the complete response and then copies individual blobs. At the
existing worst case of 48 files at 2 MiB each, the raw response plus copies can
occupy roughly 192 MiB, before decoding and subprocess overhead. Previously the
raw-read stage held one file at a time. These three actual batches contain only
298, 231, and 259 kB of source bytes. Peak memory was not measured.

This bounded tradeoff is acceptable for this small experiment, but a streaming
batch reader would be preferable if inputs regularly approach the file limit.
That would require its own equivalence and resource checks. The result does not
justify changing the frozen selection policy or claiming an executor benefit.
