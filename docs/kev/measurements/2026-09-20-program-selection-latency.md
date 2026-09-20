# Program selection latency, per Kev checkpoint

The latency behind
[`docs/decision-models/2026-09-20-program-selection-local-doors.md`](../../decision-models/2026-09-20-program-selection-local-doors.md):
three Kev checkpoints answering the `program-selection-v1` question on the
44 open items of the suite of the same name, eight passes each, one door at
a time. That record reads the numbers; this one keeps the per-block tables
and the conditions they were taken under.

## What ran

```text
CARGO_TARGET_DIR=$HOME/target-oa cargo build -p kev --features serve --release --bin kev-serve
CARGO_TARGET_DIR=$HOME/target-oa cargo build -p gym --release --bin gym

kev-serve --adapter-dir ~/repos/kev-artifacts/kev-0.5b \
    --base-dir ~/repos/kev-artifacts/qwen2.5-0.5b --default kev-0.5b --port 8101
kev-serve --adapter-dir ~/repos/kev-artifacts/kev-0.6b \
    --base-dir ~/repos/kev-artifacts/qwen3-0.6b --default kev-0.6b --port 8102
kev-serve --adapter-dir ~/repos/kev-artifacts/kev-4b \
    --base-dir ~/repos/kev-artifacts/qwen3-4b --default kev-4b --port 8104

gym latency --suite crates/gym/suites/program-selection-v1.json \
    --door kev-0.5b=http://127.0.0.1:8101 --timeout 300 --blocks 8
gym latency --suite crates/gym/suites/program-selection-v1.json \
    --door kev-0.6b=http://127.0.0.1:8102 --timeout 300 --blocks 8
gym latency --suite crates/gym/suites/program-selection-v1.json \
    --door kev-4b=http://127.0.0.1:8104 --timeout 300 --blocks 8
```

Suite `program-selection-v1`, digest `7b66bc4ce8336de2`; question set
`program-selection-v1`, digest `88388df7d2b93d14`. Every door ran on CPU at
fp32, the `kev-serve` default. `kev-0.5b` and `kev-0.6b` are the checkpoints
`scripts/fetch-kev-artifacts.sh` verifies against the manifests; `kev-4b` is
the pinned Hub revision `0d68f2491a04`, fetched by hand because the Hub's
`main` had moved to a checkpoint whose digests the manifest does not match.

The machine is an eight-core `INTEL(R) XEON(R) PLATINUM 8559C` with 31 GiB
of memory on Linux 5.15.200, with nothing of ours running other than the
three `kev-serve` processes. Sweeps ran one at a time, so two of the three
servers were idle during each. No call was refused or lost in any block.

## `kev-0.5b`

| Block | Calls | Refused | Lost | p50 | p95 | Mean |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 44 | 0 | 0 | 480.8 ms | 726.6 ms | 607.8 ms |
| 1 | 44 | 0 | 0 | 574.8 ms | 1179.0 ms | 694.2 ms |
| 2 | 44 | 0 | 0 | 537.3 ms | 1838.4 ms | 852.2 ms |
| 3 | 44 | 0 | 0 | 538.9 ms | 797.6 ms | 648.5 ms |
| 4 | 44 | 0 | 0 | 499.3 ms | 763.9 ms | 654.3 ms |
| 5 | 44 | 0 | 0 | 480.4 ms | 754.3 ms | 620.0 ms |
| 6 | 44 | 0 | 0 | 503.1 ms | 729.0 ms | 643.0 ms |
| 7 | 44 | 0 | 0 | 543.1 ms | 1480.7 ms | 709.4 ms |

| Statistic | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- |
| p50 | 519.7 ms | 33.9 ms | 6.5% | 480.4 to 574.8 ms |
| p95 | 1033.7 ms | 424.6 ms | 41.1% | 726.6 to 1838.4 ms |

## `kev-0.6b`

| Block | Calls | Refused | Lost | p50 | p95 | Mean |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 44 | 0 | 0 | 652.6 ms | 2054.8 ms | 889.2 ms |
| 1 | 44 | 0 | 0 | 637.5 ms | 1018.5 ms | 897.7 ms |
| 2 | 44 | 0 | 0 | 658.3 ms | 1093.7 ms | 855.5 ms |
| 3 | 44 | 0 | 0 | 718.2 ms | 1465.2 ms | 1137.5 ms |
| 4 | 44 | 0 | 0 | 728.1 ms | 1197.6 ms | 937.0 ms |
| 5 | 44 | 0 | 0 | 721.2 ms | 1325.4 ms | 905.0 ms |
| 6 | 44 | 0 | 0 | 525.5 ms | 835.2 ms | 668.7 ms |
| 7 | 44 | 0 | 0 | 523.4 ms | 824.0 ms | 683.7 ms |

| Statistic | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- |
| p50 | 645.6 ms | 82.3 ms | 12.8% | 523.4 to 728.1 ms |
| p95 | 1226.8 ms | 401.7 ms | 32.7% | 824.0 to 2054.8 ms |

## `kev-4b`

| Block | Calls | Refused | Lost | p50 | p95 | Mean |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 44 | 0 | 0 | 3161.0 ms | 6217.5 ms | 3715.2 ms |
| 1 | 44 | 0 | 0 | 3378.0 ms | 5793.3 ms | 4229.5 ms |
| 2 | 44 | 0 | 0 | 3680.1 ms | 12738.9 ms | 5371.4 ms |
| 3 | 44 | 0 | 0 | 3244.8 ms | 5667.3 ms | 4052.4 ms |
| 4 | 44 | 0 | 0 | 3473.4 ms | 7203.5 ms | 4608.4 ms |
| 5 | 44 | 0 | 0 | 3343.7 ms | 6904.5 ms | 4224.4 ms |
| 6 | 44 | 0 | 0 | 2882.1 ms | 9981.2 ms | 4072.8 ms |
| 7 | 44 | 0 | 0 | 2473.0 ms | 4375.2 ms | 3113.4 ms |

| Statistic | Mean over blocks | Standard deviation | Relative | Range |
| --- | --- | --- | --- | --- |
| p50 | 3204.5 ms | 376.2 ms | 11.7% | 2473.0 to 3680.1 ms |
| p95 | 7360.2 ms | 2714.2 ms | 36.9% | 4375.2 to 12738.9 ms |

## Reading

- The p50 is stable across blocks on every door: relative variation of
  6.5%, 12.8%, and 11.7%. The p95 is not, at 41.1%, 32.7%, and 36.9%, and
  on every door the worst p95 comes from one block with a long tail rather
  than a slow block throughout. Quote the p50 and give the p95 as a range.
- `kev-4b` at 3.2 s median is about six times `kev-0.5b` and about
  seventeen times the hosted Jev median of 192 ms recorded in
  [`docs/decision-models/2026-09-19-program-selection.md`](../../decision-models/2026-09-19-program-selection.md).
  Its worst block p95 is 12.7 s.
- These are CPU fp32 numbers on a server-class Xeon. They say nothing about
  a GPU or an Apple device, and nothing about `kev-8b`, which was not
  fetched.
