# CND-052: SHC Codex Terminal-Bench 16-Task Preserved Run

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: measured preserved multi-task smoke
Last updated: 2026-06-01
Owner: OpenAgents

This report records a larger selected Terminal-Bench 2.0 run on
`oa-shc-katy-01` using SHC Docker, Harbor, and Harbor's built-in Codex agent.
It expands the one-task and 8-task smokes in:

- `docs/bootstrap/CND-050-shc-codex-terminal-bench-smoke.md`
- `docs/bootstrap/CND-051-shc-codex-terminal-bench-8task.md`

This is still an internal substrate and agent-runtime smoke. It is not a full
Terminal-Bench score, not a public leaderboard claim, and not a provider
economics claim.

## Run Shape

| Field | Value |
| --- | --- |
| Host | `oa-shc-katy-01` |
| IP | `23.182.128.195` |
| Dataset | `terminal-bench@2.0` |
| Dataset size reported by Harbor | 89 tasks |
| Tasks requested | 16 |
| Agent | Harbor `codex` |
| Codex package inside each task | `@openai/codex@0.135.0` |
| Model | `gpt-5.5` |
| Execution mode | Sequential Harbor jobs, one attempt per task |
| Auth mode | Temporary ChatGPT Codex `auth.json` injected through `CODEX_AUTH_JSON_PATH` |
| Artifact root | `/home/ubuntu/oa-bench-runs/shc-codex-tb2-16task-gpt55-20260601/` |
| Aggregate JSON | `/home/ubuntu/oa-bench-runs/shc-codex-tb2-16task-gpt55-20260601/aggregate.json` |
| Artifact checksums | `/home/ubuntu/oa-bench-runs/shc-codex-tb2-16task-gpt55-20260601/artifact-sha256s.txt` |

The temporary host-side Codex auth file was removed after the batch completed.
Do not copy raw Codex auth material, session files, benchmark flags/secrets, or
task-local secret payloads into docs, issue comments, or public artifacts.

Command shape used per task:

```bash
uvx --from harbor harbor run \
  --dataset terminal-bench@2.0 \
  --task terminal-bench/<task> \
  --agent codex \
  --model gpt-5.5 \
  --agent-kwarg version=0.135.0 \
  --agent-env CODEX_AUTH_JSON_PATH=<session-auth-json> \
  --n-concurrent 1 \
  --n-attempts 1 \
  --jobs-dir /home/ubuntu/oa-bench-runs/shc-codex-tb2-16task-gpt55-20260601 \
  --job-name <task> \
  --yes \
  --debug
```

## Preservation

Raw Harbor result directories, agent traces, verifier outputs, console logs,
aggregate JSON, and per-file checksums remain on the SHC host.

The preserved tarballs are:

| Run | Tarball | SHA256 |
| --- | --- | --- |
| 16-task run | `/home/ubuntu/oa-bench-artifacts/shc-codex-tb2-16task-gpt55-20260601.tar.gz` | `e2ed556c34f1d26640b95ee90cebfb67c3042b3c198dcc676064a6e1b2b76148` |
| Prior 8-task run | `/home/ubuntu/oa-bench-artifacts/shc-codex-tb2-8task-gpt55-20260601.tar.gz` | `da824df860e96a11dde875875a66d02d703eb63743c51b5faaf0a6b045cd5771` |

The raw bundles are intentionally not committed because Terminal-Bench traces
can contain benchmark-local secret values, generated artifacts, and account
session details. The committed record is the score, task checksums, artifact
paths, and artifact digests.

## Aggregate Score

| Metric | Value |
| --- | --- |
| Terminal-Bench tasks run | 16 |
| Reward-1 tasks | 11 |
| Failed / errored tasks | 5 |
| Mean reward | `0.6875` |
| Total task runtime sum | `4172.571406` seconds |
| Total input tokens | 11,158,384 |
| Total cached input tokens | 10,191,360 |
| Total output tokens | 112,318 |
| Reported model cost | `$13.300340` |

`query-optimize` did not expose a final job runtime in Harbor's top-level
`result.json`, so its runtime is excluded from the summed runtime.

## Per-Task Results

| Task | Reward | Verifier | Errors | Runtime | Cost |
| --- | ---: | --- | ---: | ---: | ---: |
| `cancel-async-tasks` | `1.0` | 6 passed / 0 failed | 0 | 86.5s | `$0.099429` |
| `configure-git-webserver` | `0.0` | 0 passed / 1 failed | 0 | 209.9s | `$0.892526` |
| `count-dataset-tokens` | `1.0` | 1 passed / 0 failed | 0 | 223.5s | `$0.683767` |
| `db-wal-recovery` | `0.0` | 5 passed / 2 failed | 0 | 332.5s | `$1.082362` |
| `financial-document-processor` | `1.0` | 7 passed / 0 failed | 0 | 225.6s | `$1.132441` |
| `gcode-to-text` | `0.0` | 1 passed / 1 failed | 0 | 266.1s | `$1.009637` |
| `headless-terminal` | `1.0` | 7 passed / 0 failed | 0 | 218.9s | `$0.515026` |
| `large-scale-text-editing` | `1.0` | 5 passed / 0 failed | 0 | 426.6s | `$1.296931` |
| `multi-source-data-merger` | `1.0` | 3 passed / 0 failed | 0 | 123.2s | `$0.333803` |
| `polyglot-c-py` | `1.0` | 1 passed / 0 failed | 0 | 216.3s | `$0.498589` |
| `query-optimize` | `0.0` | 5 passed / 1 failed | 0 | n/a | `$0.867797` |
| `regex-chess` | `1.0` | 4 passed / 0 failed | 0 | 923.6s | `$1.980675` |
| `sanitize-git-repo` | `1.0` | 3 passed / 0 failed | 0 | 220.3s | `$1.383696` |
| `pypi-server` | `0.0` | 0 passed / 1 failed | 0 | 120.5s | `$0.458625` |
| `portfolio-optimization` | `1.0` | 4 passed / 0 failed | 0 | 372.3s | `$0.653984` |
| `write-compressor` | `1.0` | 3 passed / 0 failed | 0 | 206.6s | `$0.411052` |

Task checksums:

| Task | Checksum |
| --- | --- |
| `cancel-async-tasks` | `44b08781bbffcbff0555fafbad034389009edeb85c2f83cd10620f91aa24974f` |
| `configure-git-webserver` | `84d7c2fd653dad4307c7d2e9dd4f937ecbcd7ad4d1acf5d2b74c35166634178e` |
| `count-dataset-tokens` | `0ab655881a4827d9ec8f9930d4c4c8827729b20c978c20ff9cc6317f90660b5c` |
| `db-wal-recovery` | `c18abdc4fdc3a01bf374c55a9700708fe6a9662077d29db81abb692f0a3c5f6f` |
| `financial-document-processor` | `312889565426c3a4ed49c11531586aa8b7ba947b1507a846347280f44185cdba` |
| `gcode-to-text` | `0ce9e2f5430d6cad157437ae0fc23f581c3f6ce3ee3eb9233484a554011557ed` |
| `headless-terminal` | `d1324ce5d4734d79b4569b569d2aa826dbfce6eadc6ac406f137d21446ac8d1a` |
| `large-scale-text-editing` | `e2851ab29f9dc799ae4ba2ad8f7495ccd1625476a3954dde8cec09771e41208a` |
| `multi-source-data-merger` | `33fa3b988ff60ec62b6ce40ee455208cb083ac1ac46ddc5247e954c88b9d5e8e` |
| `polyglot-c-py` | `d1e52e6139c57528762bbb163dc3f89edb967511d897ac5e7859398d3f44eb3c` |
| `query-optimize` | `593405b6a2f6970f0f4c2ca2aff455a3b32bce3eecf8b506cfeeb0fe93b14838` |
| `regex-chess` | `64993ab339d437cea9397a033e825663a77d5527a42ef3c2fb2ad25133dcbd4e` |
| `sanitize-git-repo` | `b2e4391be7422fb3cf8686964979a57704aae85a36bccd2df40e22c049f58e13` |
| `pypi-server` | `9eb21236249d5b42ee64c7229bb1e5a3b3c0be7ed5ee9801bdd0d16ca575941a` |
| `portfolio-optimization` | `43497eefff71e8fe8730c3e3e97d29cd9720123be046b9722a7c538bf8a67255` |
| `write-compressor` | `17ff45a3beaa04e5f08eb4f3c6136f4b56602bdcca52ade0896cfd341f374e99` |

## Operational Notes

The original wrapper stalled after `query-optimize` had written its trial
result and verifier output. The missing five tasks were resumed into the same
artifact root with `resume-console.log`:

```text
regex-chess
sanitize-git-repo
pypi-server
portfolio-optimization
write-compressor
```

`regex-chess` is a useful stress case: Codex generated a large regex table,
passed the provided checker, ran additional randomized validation, then passed
the Harbor verifier. It cost `$1.980675`, the most expensive task in this set.

`portfolio-optimization` is a useful native-code task: Codex built and
benchmarked a C extension, passed the benchmark harness for 5000- and
8000-asset cases, and the verifier passed 4/4 checks.

The failed tasks were benchmark misses, not SHC substrate failures:

- `configure-git-webserver`
- `db-wal-recovery`
- `gcode-to-text`
- `query-optimize`
- `pypi-server`

## Interpretation

This run proves the SHC node can run a larger selected Terminal-Bench 2.0
batch through Harbor's Codex agent while preserving per-task traces, verifier
outputs, task checksums, aggregate scores, and tarred artifact bundles.

The measured internal score for this selected 16-task set is:

```text
11 / 16 tasks passed
mean reward = 0.6875
reported model cost = $13.300340
```

It does not establish full-dataset performance across all 89 Terminal-Bench
2.0 tasks. A full sweep needs a queue controller, budget cap, task pinning,
artifact upload, retry policy, and public-claim redaction before it should be
run or reported externally.

## Next Steps

- Move this manual SSH flow behind the Benchmark Cloud control API and Worker/Khala Sync
  workroom launch path.
- Add automatic tar/checksum generation and artifact upload after every run.
- Store Harbor `result.json`, trial `result.json`, verifier `ctrf.json`, and
  agent trace paths as normalized `BenchmarkResult` and proof-bundle records.
- Add task timeout and resume semantics so a stalled or partial job does not
  leave later tasks blocked.
- Add cost limits before running any full 89-task sweep.
