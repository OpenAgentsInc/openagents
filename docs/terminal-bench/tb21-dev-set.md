# Terminal-Bench 2.1 development set

Terminal-Bench 2.1 (TB2.1) gives Microcoder 89 more tasks to tune on. Its
tasks are disjoint from Terminal-Bench 4 (TB4), so tuning on them doesn't
touch the held-out tasks of the
[out-of-sample study](2026-09-26-out-of-sample-study.md)
([#9683](https://github.com/OpenAgentsInc/openagents/issues/9683)).

On coderos-4080 on September 26, 2026, **83 of the 89 tasks grade 1** with
their reference solutions under `microcoder --check-grading`. The other six
fail because of upstream drift in packages, downloads, or live data, not
because of Microcoder. The one Microcoder fault found, the host's `docker
exec` umask, is fixed for the reference solution.

## The rule: TB2.1 is fair game

You can tune Microcoder's loop, prompts, Jev questions, knowledge entries,
and configuration on any TB2.1 task, and read its transcripts, verifier
output, and task files.

- No TB2.1 task shares a name with a TB4 task. The installed TB4 `tasks/`
  and TB2.1 `tasks/` folder names have no overlap.
- The study already names TB2.1 as an allowed knowledge source ("different
  tasks") in its [rules between rounds](2026-09-26-out-of-sample-study.md#rules-between-rounds).
  Its loop-change rule names only the 14 excluded Microcoder tasks, so treat
  TB2.1 as an extension of that excluded set, and keep an entry's
  `written_from` pointing at its TB2.1 task.
- Everything else in the study's rules still holds: never open a TB4
  held-out task's files, transcripts, or verifier output to tune.

Earlier harnesses already used some TB2.1 tasks. The `terminal-bench`
checkout's `archive/` tasks and `bench/terminal-bench/profiles/task-pool.json`
are TB2.1 tasks, and the archive-check confirmation ran 12 of them. That is
development history, not a held-out conflict.

## Install

The tasks come from Harbor Hub through Harbor's own `download` command, with
the Harbor virtual environment in `bench/terminal-bench`:

```sh
mkdir -p ~/.openagents/terminal-bench/upstream/terminal-bench-2.1/stage
cd ~/.openagents/terminal-bench/upstream/terminal-bench-2.1/stage
~/openagents/bench/terminal-bench/.venv/bin/harbor download \
  terminal-bench/terminal-bench-2-1 --export -o .
mv terminal-bench-2-1 ../tasks && cd .. && rmdir stage
```

`terminal-bench/terminal-bench-2-1` is the dataset reference of the public
leaderboard in `bench/terminal-bench/reference/tb21-leaderboard.json`. The
download resolved `@latest` to 89 tasks, 946 files, and 60 MB. To check that
a copy matches the one measured here, compare its tree digest:

```sh
cd ~/.openagents/terminal-bench/upstream/terminal-bench-2.1/tasks
find . -type f | LC_ALL=C sort | xargs sha256sum | sha256sum
# c13961acb68d296b5b12ce158b9e9b89fbe7d561b06b1d748f5cda59a48d0ec2
```

Every task has TB4's layout (`task.toml`, `instruction.md`, `environment/`,
`solution/solve.sh`, `tests/test.sh`), a prebuilt `alexgshaw/<task>` image,
internet access, no Compose file, no GPU, and a shared verifier. Microcoder
reads them without code changes.

## Run a task

Point `MICROCODER_TASKS` at the folder:

```sh
export MICROCODER_TASKS=~/.openagents/terminal-bench/upstream/terminal-bench-2.1/tasks
microcoder fix-git --check-grading   # reference solution, no model cost
microcoder fix-git                   # the loop
```

Microcoder prints no Fable reference line for a TB2.1 task, because its
Fable 5.1 low reference covers TB4 only. Use the table below instead.

Keep the load low when a study batch is running: the check-grading sweep
here ran at most three tasks at a time.

## Check-grading results

The sweep ran each task once at three at a time; each failure ran again.
Most tasks finish in under two minutes; `compile-compcert` took 16 minutes
and `caffe-cifar-10` 31.

**The umask fault.** On coderos-4080 `docker exec` inherits dockerd's umask
of 0000. `git-multibranch`'s reference solution creates `/run/sshd`
world-writable, and sshd refuses to start. The leaderboard runs, which pass
this task, had the usual umask 022, so Microcoder now runs the reference
solution with `umask 022`. The
loop's commands and the tests still inherit 0000. A task that checks
permissions can fail in the loop for that reason; fix it outside a
pre-registered study round, since it changes the loop's environment.

| Task | Result | Why |
| --- | --- | --- |
| `git-multibranch` | pass | The umask fault above; passes with the fix. `solve.sh` still exits nonzero at its last line because the image has no `tmux`, which doesn't change the grade. |
| `largest-eigenval` | pass | Failed once on its two `test_speedup` timing tests while the host was loaded; passed on a rerun. Timing-sensitive. |
| `build-cython-ext` | fail | Upstream drift: `pip install -e .` now resolves `planarity` 1.0.0, which breaks pyknotid's `test_reconstructed_space_curve` (`KeyError: 'pos'`). |
| `mcmc-sampling-stan` | fail | Upstream drift: CRAN's `RcppParallel` 6.2.1 needs `cmake`, which the image lacks, so `StanHeaders` and `rstan` don't install. Failed twice. |
| `build-pov-ray` | fail | Upstream drift: www.povray.org returns 403 for the POV-Ray 2.2 download. Failed twice. |
| `qemu-startup` | fail | The verifier's `apt-get install curl` gets 404 from Debian's bullseye-security pool: the index lists `curl` 7.74.0-1.3+deb11u16 and the file is gone. The tests can't install `uv`. Failed twice. |
| `qemu-alpine-ssh` | fail | The same bullseye-security 404 in the verifier. Failed twice. |
| `protein-assembly` | fail | The reference solution queries live RCSB, PubChem, and FPbase data; dnachisel raised `NoSolutionError` both times. Likely live-data drift. |

Leave the six failing tasks out of tuning comparisons: a Microcoder failure
there doesn't say anything about the loop. The top agents passed them
before the drift (see the table).

## Reference stats per task

From `bench/terminal-bench/reference/tb21-leaderboard.json`, fetched
September 23, 2026 from Harbor Hub's public leaderboard. Fable 5.1 has no
TB2.1 row. The table uses:

- **Fable 5**: Claude Code, Fable 5, xhigh (rank 6, 83.8%). Passes out of
  five trials, dollars per trial, and mean agent seconds.
- **Luna**: Codex, GPT-5.6 Luna, max (rank 14, 75.7%). The closest public
  row to Microcoder's model.
- **Astra**: Codex, GPT-6 Astra, high (rank 1, 87.4%). No per-task cost is
  published.
- **All rows**: the pass rate over all 22 leaderboard rows' 110 trials.

Dollars per trial are the source job's per-task cost divided by its trials.
Fable 5's source job prices 366 of its row's 553 dollars, so its per-task
dollars are a lower bound. `bench/terminal-bench/reference/tb21-dev-set.json`
holds the same data, plus errored-trial counts, for tools.

| Task | Category | Difficulty | Check-grading | Fable 5 | Fable 5 $/trial | Fable 5 s | Luna | Luna $/trial | Astra | All rows |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `adaptive-rejection-sampler` | scientific-computing | medium | pass | 5/5 | $1.61 | 584 | 5/5 | $0.71 | 5/5 | 79% |
| `bn-fit-modify` | scientific-computing | hard | pass | 5/5 | $0.31 | 155 | 5/5 | $0.10 | 5/5 | 99% |
| `break-filter-js-from-html` | security | medium | pass | 5/5 | $0.41 | 148 | 4/5 | $0.52 | 1/5 | 77% |
| `build-cython-ext` | debugging | medium | fail | 4/5 | $1.11 | 555 | 5/5 | $0.52 | 5/5 | 88% |
| `build-pmars` | software-engineering | medium | pass | 5/5 | $0.28 | 148 | 5/5 | $0.29 | 5/5 | 99% |
| `build-pov-ray` | software-engineering | medium | fail | 4/5 | $0.82 | 400 | 5/5 | $0.25 | 4/5 | 70% |
| `caffe-cifar-10` | machine-learning | medium | pass | 5/5 | $1.18 | 1460 | 5/5 | $1.97 | 5/5 | 85% |
| `cancel-async-tasks` | software-engineering | hard | pass | 5/5 | $0.27 | 138 | 3/5 | $0.06 | 5/5 | 68% |
| `chess-best-move` | games | medium | pass | 4/5 | $0.17 | 139 | 2/5 | $0.12 | 5/5 | 72% |
| `circuit-fibsqrt` | software-engineering | hard | pass | 5/5 | $1.04 | 438 | 5/5 | $0.40 | 5/5 | 86% |
| `cobol-modernization` | software-engineering | easy | pass | 2/5 | $0.49 | 888 | 5/5 | $0.26 | 5/5 | 96% |
| `code-from-image` | software-engineering | medium | pass | 5/5 | $0.04 | 18 | 5/5 | $0.03 | 5/5 | 95% |
| `compile-compcert` | system-administration | medium | pass | 2/5 | $0.46 | 2332 | 5/5 | $2.62 | 5/5 | 67% |
| `configure-git-webserver` | system-administration | hard | pass | 0/5 | $0.19 | 107 | 4/5 | $0.31 | 0/5 | 36% |
| `constraints-scheduling` | personal-assistant | medium | pass | 5/5 | $0.17 | 57 | 5/5 | $0.04 | 5/5 | 98% |
| `count-dataset-tokens` | model-training | medium | pass | 5/5 | $0.14 | 79 | 1/5 | $0.11 | 5/5 | 93% |
| `crack-7z-hash` | security | medium | pass | 5/5 | $0.42 | 415 | 5/5 | $0.86 | 5/5 | 95% |
| `custom-memory-heap-crash` | debugging | medium | pass | 5/5 | $0.69 | 262 | 4/5 | $0.27 | 3/5 | 72% |
| `db-wal-recovery` | file-operations | medium | pass | 2/5 | $0.72 | 568 | 2/5 | $0.62 | 5/5 | 51% |
| `distribution-search` | machine-learning | medium | pass | 5/5 | $0.39 | 154 | 5/5 | $0.06 | 5/5 | 96% |
| `dna-assembly` | scientific-computing | hard | pass | 4/5 | $3.50 | 1100 | 4/5 | $0.59 | 5/5 | 63% |
| `dna-insert` | scientific-computing | medium | pass | 0/5 | $0.84 | 285 | 0/5 | $0.19 | 5/5 | 30% |
| `extract-elf` | file-operations | medium | pass | 1/5 | $1.09 | 428 | 0/5 | $0.27 | 0/5 | 40% |
| `extract-moves-from-video` | file-operations | hard | pass | 1/5 | $3.93 | 1792 | 2/5 | $2.79 | 5/5 | 33% |
| `feal-differential-cryptanalysis` | mathematics | hard | pass | 5/5 | $0.67 | 243 | 5/5 | $0.15 | 5/5 | 100% |
| `feal-linear-cryptanalysis` | mathematics | hard | pass | 5/5 | $1.49 | 487 | 5/5 | $0.57 | 5/5 | 86% |
| `filter-js-from-html` | security | medium | pass | 0/5 | $2.54 | 852 | 0/5 | $0.53 | 0/5 | 3% |
| `financial-document-processor` | data-processing | medium | pass | 5/5 | $0.58 | 138 | 5/5 | $0.28 | 5/5 | 87% |
| `fix-code-vulnerability` | security | hard | pass | 5/5 | $0.16 | 75 | 5/5 | $0.10 | 5/5 | 100% |
| `fix-git` | software-engineering | easy | pass | 5/5 | $0.14 | 64 | 5/5 | $0.09 | 5/5 | 100% |
| `fix-ocaml-gc` | software-engineering | hard | pass | 5/5 | $1.31 | 878 | 5/5 | $2.33 | 5/5 | 98% |
| `gcode-to-text` | file-operations | medium | pass | 5/5 | $0.35 | 142 | 0/5 | $0.99 | 5/5 | 49% |
| `git-leak-recovery` | software-engineering | medium | pass | 5/5 | $0.16 | 61 | 5/5 | $0.05 | 5/5 | 98% |
| `git-multibranch` | system-administration | medium | pass | 5/5 | $0.35 | 162 | 5/5 | $0.19 | 5/5 | 97% |
| `gpt2-codegolf` | software-engineering | hard | pass | 5/5 | $1.15 | 434 | 2/5 | $1.53 | 5/5 | 55% |
| `headless-terminal` | software-engineering | medium | pass | 5/5 | $0.44 | 217 | 5/5 | $0.34 | 3/5 | 75% |
| `hf-model-inference` | data-science | medium | pass | 5/5 | $0.15 | 88 | 4/5 | $0.10 | 5/5 | 85% |
| `install-windows-3.11` | system-administration | hard | pass | 4/5 | $3.33 | 1719 | 4/5 | $1.67 | 2/5 | 40% |
| `kv-store-grpc` | software-engineering | medium | pass | 5/5 | $0.13 | 64 | 1/5 | $0.07 | 4/5 | 75% |
| `large-scale-text-editing` | file-operations | medium | pass | 5/5 | $0.21 | 126 | 5/5 | $0.12 | 5/5 | 96% |
| `largest-eigenval` | mathematics | medium | pass | 5/5 | $0.81 | 351 | 5/5 | $0.78 | 5/5 | 91% |
| `llm-inference-batching-scheduler` | machine-learning | hard | pass | 5/5 | $1.43 | 519 | 5/5 | $0.37 | 5/5 | 94% |
| `log-summary-date-ranges` | data-processing | medium | pass | 5/5 | $0.10 | 43 | 5/5 | $0.05 | 4/5 | 94% |
| `mailman` | system-administration | medium | pass | 5/5 | $1.08 | 468 | 5/5 | $0.67 | 5/5 | 84% |
| `make-doom-for-mips` | software-engineering | hard | pass | 0/5 | $0.00 | 900 | 0/5 | $2.95 | 0/5 | 3% |
| `make-mips-interpreter` | software-engineering | hard | pass | 5/5 | $2.95 | 1027 | 1/5 | $1.47 | 5/5 | 67% |
| `mcmc-sampling-stan` | data-science | hard | fail | 5/5 | $0.35 | 515 | 3/5 | $0.86 | 5/5 | 92% |
| `merge-diff-arc-agi-task` | debugging | medium | pass | 5/5 | $0.26 | 125 | 5/5 | $0.14 | 5/5 | 95% |
| `model-extraction-relu-logits` | mathematics | hard | pass | 3/5 | $0.88 | 325 | 3/5 | $0.27 | 2/5 | 41% |
| `modernize-scientific-stack` | scientific-computing | medium | pass | 5/5 | $0.12 | 48 | 5/5 | $0.04 | 5/5 | 100% |
| `mteb-leaderboard` | data-science | medium | pass | 5/5 | $0.62 | 710 | 5/5 | $0.61 | 5/5 | 84% |
| `mteb-retrieve` | data-science | medium | pass | 4/5 | $0.19 | 129 | 5/5 | $0.09 | 1/5 | 55% |
| `multi-source-data-merger` | data-processing | medium | pass | 5/5 | $0.17 | 64 | 5/5 | $0.07 | 5/5 | 99% |
| `nginx-request-logging` | system-administration | medium | pass | 5/5 | $0.15 | 90 | 5/5 | $0.06 | 5/5 | 100% |
| `openssl-selfsigned-cert` | security | medium | pass | 5/5 | $0.16 | 68 | 5/5 | $0.06 | 5/5 | 95% |
| `overfull-hbox` | debugging | easy | pass | 4/5 | $0.34 | 304 | 3/5 | $0.32 | 5/5 | 80% |
| `password-recovery` | security | hard | pass | 5/5 | $0.69 | 222 | 5/5 | $0.11 | 5/5 | 95% |
| `path-tracing` | software-engineering | hard | pass | 4/5 | $1.38 | 1614 | 5/5 | $1.43 | 5/5 | 79% |
| `path-tracing-reverse` | software-engineering | hard | pass | 5/5 | $3.75 | 1317 | 5/5 | $1.67 | 5/5 | 72% |
| `polyglot-c-py` | software-engineering | medium | pass | 5/5 | $0.24 | 118 | 5/5 | $0.12 | 5/5 | 99% |
| `polyglot-rust-c` | software-engineering | hard | pass | 5/5 | $0.45 | 187 | 5/5 | $0.16 | 5/5 | 88% |
| `portfolio-optimization` | optimization | medium | pass | 5/5 | $0.47 | 223 | 4/5 | $0.10 | 5/5 | 96% |
| `protein-assembly` | scientific-computing | hard | fail | 0/5 | $1.94 | 665 | 3/5 | $0.81 | 5/5 | 45% |
| `prove-plus-comm` | software-engineering | easy | pass | 5/5 | $0.07 | 40 | 5/5 | $0.04 | 5/5 | 99% |
| `pypi-server` | software-engineering | medium | pass | 5/5 | $0.15 | 85 | 0/5 | $0.09 | 4/5 | 75% |
| `pytorch-model-cli` | model-training | medium | pass | 4/5 | $0.34 | 162 | 5/5 | $0.21 | 5/5 | 85% |
| `pytorch-model-recovery` | model-training | medium | pass | 4/5 | $0.19 | 357 | 5/5 | $0.17 | 5/5 | 92% |
| `qemu-alpine-ssh` | system-administration | medium | fail | 5/5 | $0.66 | 550 | 0/5 | $0.45 | 5/5 | 73% |
| `qemu-startup` | system-administration | medium | fail | 4/5 | $0.96 | 422 | 5/5 | $0.49 | 5/5 | 84% |
| `query-optimize` | data-science | medium | pass | 2/5 | $0.16 | 641 | 3/5 | $0.27 | 5/5 | 66% |
| `raman-fitting` | scientific-computing | medium | pass | 3/5 | $0.34 | 153 | 0/5 | $0.80 | 4/5 | 31% |
| `regex-chess` | software-engineering | hard | pass | 5/5 | $2.92 | 2032 | 4/5 | $1.86 | 5/5 | 66% |
| `regex-log` | data-processing | medium | pass | 5/5 | $0.34 | 138 | 5/5 | $0.10 | 5/5 | 89% |
| `reshard-c4-data` | data-science | medium | pass | 5/5 | $0.72 | 379 | 5/5 | $0.34 | 5/5 | 99% |
| `rstan-to-pystan` | data-science | medium | pass | 5/5 | $1.42 | 532 | 5/5 | $0.41 | 5/5 | 95% |
| `sam-cell-seg` | data-science | hard | pass | 5/5 | $3.10 | 1648 | 5/5 | $0.37 | 4/5 | 87% |
| `sanitize-git-repo` | security | medium | pass | 3/5 | $0.62 | 186 | 3/5 | $0.44 | 0/5 | 45% |
| `schemelike-metacircular-eval` | software-engineering | medium | pass | 5/5 | $2.31 | 1393 | 4/5 | $0.78 | 5/5 | 80% |
| `sparql-university` | data-querying | hard | pass | 5/5 | $0.38 | 140 | 5/5 | $0.08 | 5/5 | 97% |
| `sqlite-db-truncate` | debugging | medium | pass | 5/5 | $0.18 | 74 | 5/5 | $0.06 | 5/5 | 99% |
| `sqlite-with-gcov` | system-administration | medium | pass | 5/5 | $0.41 | 168 | 4/5 | $0.17 | 5/5 | 95% |
| `torch-pipeline-parallelism` | software-engineering | hard | pass | 5/5 | $0.72 | 364 | 5/5 | $0.36 | 5/5 | 65% |
| `torch-tensor-parallelism` | software-engineering | hard | pass | 5/5 | $0.38 | 187 | 0/5 | $0.13 | 5/5 | 85% |
| `train-fasttext` | model-training | hard | pass | 2/5 | $0.66 | 2365 | 0/5 | $1.93 | 5/5 | 32% |
| `tune-mjcf` | scientific-computing | medium | pass | 5/5 | $0.56 | 357 | 5/5 | $0.20 | 5/5 | 93% |
| `video-processing` | video-processing | hard | pass | 4/5 | $1.27 | 479 | 1/5 | $0.81 | 2/5 | 33% |
| `vulnerable-secret` | security | medium | pass | 5/5 | $0.30 | 97 | 5/5 | $0.05 | 1/5 | 77% |
| `winning-avg-corewars` | software-engineering | medium | pass | 0/5 | $0.00 | 3600 | 3/5 | $1.13 | 5/5 | 75% |
| `write-compressor` | software-engineering | hard | pass | 5/5 | $2.12 | 751 | 5/5 | $0.29 | 5/5 | 82% |
