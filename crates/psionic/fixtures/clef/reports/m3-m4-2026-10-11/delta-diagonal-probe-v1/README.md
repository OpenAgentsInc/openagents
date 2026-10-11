# DeltaNet block32: correctness passes, speed fails

Do not integrate this candidate or rebuild the full server for it. The
production sequential scan beat both unchanged block32 and the prepared-diagonal
candidate at every tested shape by the median paired GPU ratio. Product source
was not changed; no model was loaded.

| Shape | Sequential median (ms) | Block32 median (ms) | Diagonal median (ms) | Block32 / sequential paired median | Diagonal / sequential paired median |
| --- | ---: | ---: | ---: | ---: | ---: |
| Flash, 1024 tokens | 0.996 | 1.093 | 1.394 | 1.376x | 1.370x |
| Flash, 2048 tokens | 1.517 | 2.132 | 2.139 | 1.407x | 1.409x |
| 27B, 1024 tokens | 1.055 | 1.961 | 1.983 | 1.838x | 1.861x |
| 27B, 2048 tokens | 2.118 | 3.936 | 3.987 | 1.857x | 1.882x |

Ratios are paired within each round, not ratios of the absolute medians. All six
rounds, including outliers and their order, remain in
[stdout.jsonl](run-1/stdout.jsonl). Each implementation occupies each order
position twice. Timing includes each block preparation plus scan, and excludes
the common normalization/dot preparation from every implementation. This is a
short shared-machine synthetic experiment, not sustained full-model latency.
No clock, thermal, bandwidth, or scheduling cause was established.

All 26 correctness checks passed before their corresponding timing group:
22 independent dense-f64/edge fixtures and four complete actual-shape comparisons.
The existing bound remains `2e-5 + 2e-4*abs(reference)`. Both grouped head orders
match sequential bitwise on 768 production-prepared one-token outputs with zero
initial state. All candidate final states match original block32 bitwise.
The largest dense-f64 output differences were sequential `5.998e-8`, block32
`3.137e-7`, and diagonal `3.100e-7`; both block final states reached `9.260e-7`.
This does not establish that the previously failed weighted `1e-3` gate is fixed.
The speed failure is enough to stop further integration work on this candidate.

The [summary](run-1/summary.json) retains every check and timing pair. Its
`status: passed` describes completion/correctness; the performance decision here
is **rejected**. The [guard receipt](run-1/receipt.json) records 33.803 seconds,
peak accounted child memory 1,047,397,744 bytes, and no sampling, memory, or
cleanup failure. The 2 GiB cap, 14 GiB start, 12 GiB available-memory floor and
60-second deadline were unchanged. GPU and memory leases were released. No
quiet lease, desktop-app action, or system-service action was used.

All three Metal4.0 sources and the Swift probe compiled in the
[bounded build](build-1/receipt.json). Executable SHA-256:
`9f0d90073f4e7dae0072d639e4e86162114d8788652289bcfb3a36c8ec1eaf5b`.
One unused-mutation warning is preserved in the build log. The executable and
AIR objects are omitted; exact sources, source hashes and compile commands are
included. No weight files, model activations, or per-sample memory timeline are
included.

[PREPARED_PLAN.md](PREPARED_PLAN.md) preserves the plan as it stood before the
build/run, including its then-current unexecuted status. The proposed
[candidate.patch](candidate.patch) was never applied. Complete before/after
source snapshots and their original hashes are retained. The candidate reuses
production `K·Q`, removes the TensorOps same-token diagonal, and adds `U*(K·Q)`
after the strictly lower product without changing recurrent-state arithmetic.

[sanitization.json](sanitization.json) maps every selected original file/hash to
its published copy/hash. Private absolute paths are replaced by artifact labels.
The public `guard.py` requires `CLEF_REPO` to name a local checkout containing the
shared guard modules. That explicit path-configuration change is also recorded;
it is not presented as the original executed guard. Reproduction still needs
coordinated build/memory and GPU/memory leases. It must not overlap another
measurement, and the resource bounds must not be increased to make a test pass.
