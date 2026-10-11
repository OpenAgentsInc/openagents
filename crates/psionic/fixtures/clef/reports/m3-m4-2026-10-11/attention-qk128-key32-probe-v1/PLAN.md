# QK batching with unchanged 32-key updates

This scratch-only candidate calculates 128 QK scores together, then performs
four original 32-key softmax and PV updates. It is not built or run yet.
The earlier full key128 candidate failed the fixed full-model raw-logit gate;
its failure remains separate and does not change this experiment's criteria.

## Mechanism and expected contribution

The current shader performs four QK and four PV TensorOps calls per 128 keys.
The candidate performs one QK and four PV calls: eight calls become five,
with the same total products. It also removes three QK-to-softmax barriers
per 128 keys. The query tile remains 16 and dispatch remains 128 threads.
Threadgroup storage grows from 19,456 to 25,600 bytes, below the measured
32,768-byte device limit. Admission checks the actual compiled pipeline too.

This eliminates 37.5% of these calls, not 37.5% of model latency. The measured
full key128 prototype had two calls per 128 keys but changed rounding and
failed model parity. A purely call-overhead-limited estimate here is at most
1.6 times attention throughput; actual improvement can be smaller or absent.
There is no measured full-model benefit or target-completion claim.

`source_check.py` verifies the inner update expressions match the original
shader after only the score-address substitution. The lane grouping and sum
order, running maximum, f16 exponentials, f32 denominator, alpha scaling,
running-output writes, and PV descriptor remain unchanged. The QK output
width can still change its dot-product reduction, so source equivalence is
not enough to establish output equivalence.

A larger query tile with a cooperative-tensor running accumulator is a second
possible design, but it is not part of this candidate. It needs a separate
mapping of each register element to a query row for alpha scaling, and its
register pressure and scheduling effect are unknown. This candidate changes
only score batching and has a direct comparison with the original algorithm.

## Fixed checks

Before the normal attention checks, three diagnostic cases copy the actual
QK intermediates from diagnostic-only copies of both full shaders. They cover
random query tails, rising score bands, and a 15,361-key history. Every
computed score must be finite and bitwise identical, uncomputed cells must
remain poisoned, and fixed independent f64 dot samples must meet the existing
absolute 0.001 limit. Each diagnostic kernel must leave its normal output
bitwise unchanged. Diagnostic kernels never supply timing samples.

The normal suite retains 12 small cases, repeated/nonaligned chunks, and six
representative final chunks at 1k, 4k, and 16k for Flash and 27B head geometry.
All cross-variant attention outputs now require bitwise equality, in addition
to the existing independent f64 and chunk-invariance limits. Every source
buffer and redzone is checked. Each timed output must match its immutable,
previously finite-checked expected bytes. The fixed timing sequence contains
five alternating pairs for each of the six large cases: 60 timing rows.

The score diagnostics add 12 commands and 12 checks. Totals are 80 commands
and 156 comparisons in check mode; 140 commands and 216 comparisons in bench
mode. A failure stops the run and remains in its logs. No larger bounds or
alternative layouts are tried automatically.

## Resources and scope

The copied guard keeps a 2 GiB process cap, 14 GiB startup availability,
12 GiB host floor, 50-second internal limit, and 60-second external limit.
Explicit Metal buffers stay below 1 GiB and the observed driver allocation
limit stays 1.5 GiB. The original largest case is about 233 MiB; diagnostic
cases run in separate autorelease scopes and do not increase that peak plan.
Compilation uses the build lease, two Swift compiler threads, and a 4 GiB
process cap. GPU work requires its own GPU and memory leases. Quiet leases
are refused. No host, desktop, service, or unrelated process is controlled.

The baseline is the restored product shader, identified in `manifest.json`.
Inputs are deterministic synthetic half values; no model weights are loaded.
A passing microprobe would still require separate source review, product
build, and the unchanged full-model 0.001 raw-logit and 100% top-answer gates.
