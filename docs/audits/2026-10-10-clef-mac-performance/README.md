# Clef on the M5 Max: experiments and preparation incident

This round completed a paired Metal/Ollama comparison after recovering from an
agent-caused desktop freeze. It **does not establish an idle-Mac speed result**.
The desktop was using 43–77% of the GPU before inference, despite low CPU load.
The one-submission Metal candidate passed numerical checks but did not show a
convincing overall latency improvement. Its patch is retained as an experiment;
the default runtime is unchanged.

The agent froze desktop apps with `SIGSTOP`; the owner held the power button to
recover. The model benchmark had made zero requests before that restart. See
[the incident investigation](incident.md) for the evidence and corrective changes.

## Paired comparison

Local HTTP medians in seconds, 15 measured samples per cell. Every request has
a fresh nonce; each baseline/candidate/comparator pair receives the same state.
All three variants report matching token counts for paired requests. Warmups
and process startup are excluded from the table.

| Variant | About 1k | About 4k | About 16k |
| --- | ---: | ---: | ---: |
| Baseline Metal | 1.170 | 3.638 | 15.792 |
| One-submission candidate | 1.086 | 3.554 | 15.906 |
| Ollama 0.40.2, MLX/mxfp8 | 1.134 | 3.613 | 14.943 |

The actual input lengths vary with the nonce: 1,078–1,083, 3,914–3,919, and
15,508–15,513 tokens. The Psionic variants use the identical Q4_K_M GGUF;
Ollama uses its installed mxfp8 artifact, so that comparison includes differences
in weight format and serving implementation. It is not a kernel-only comparison.

### Drift matters more than the aggregate

| Round | Variant | 1k | 4k | 16k |
| --- | --- | ---: | ---: | ---: |
| 1 | Baseline | 0.537 | 1.878 | 8.646 |
| 1 | Candidate | 0.746 | 2.585 | 15.289 |
| 1 | Ollama | 1.053 | 3.325 | 14.911 |
| 2 | Baseline | 1.206 | 3.855 | 16.106 |
| 2 | Candidate | 1.120 | 3.554 | 18.200 |
| 2 | Ollama | 1.160 | 3.846 | 15.528 |
| 3 | Baseline | 1.206 | 3.638 | 15.792 |
| 3 | Candidate | 1.093 | 3.646 | 15.906 |
| 3 | Ollama | 1.122 | 3.651 | 14.926 |

The unchanged baseline moves from 0.537 to 1.206 seconds at 1k and from 8.646
to about 16 seconds at 16k. The candidate loses the first round, improves some
short-input medians in later rounds, and does not consistently improve 16k.
An overall median alone would conceal that drift. Three rounds alternate
forward/reverse/forward order; they are not evenly counterbalanced.

The 0.35-second 1k and 7.5-second 16k targets in
[#11196](https://github.com/OpenAgentsInc/openagents/issues/11196) remain open.
None of these desktop-contended measurements can pass its idle-machine gate.

## Machine, isolation, and timing

- M5 Max, 40 GPU cores, 18 CPU cores, 128 GiB unified memory; macOS 26.4
  (25E246), Xcode 26.6, SDK 26.5; AC power, automatic power mode.
- Baseline source: `1d16475293b1645796a873fc6877ed6d842fd07c`. Candidate:
  that source plus [`candidate.patch`](candidate.patch). Binary and model
  hashes are recorded in [`environment.json`](environment.json).
- Three rounds, three workloads, one excluded warmup plus five measured calls
  per workload and variant: 162 successful requests, 135 measured.
- The quiet lease was held throughout. Servers ran serially. New cooperating
  builds waited; desktop applications stayed running and display settings were
  left alone after recovery. No kernel profiling or skip flag was enabled.
- During the comparison, sampled one-minute CPU load ranged from 2.23 to 5.00.
  GPU use before the run was still high. In a separate ten-second sample,
  WindowServer dominated the GPU client time counters. This identifies the
  desktop as a major competing workload, not a specific offending application.
- The desktop experiment explicitly allowed GPU contention and used a
  60-second CPU settling period. The summary marks it `diagnostic_only`.
  It did not silently weaken an idle-machine gate.
- All 108 Psionic requests, including warmups, report an execution-lock wait
  that rounds to 0 ms in the server logs; none reports an answer-cache hit.
  Psionic also reports zero cached input tokens. Ollama does not return that
  cache counter here; unique nonce-prefixed states prevent repeating a request.
- The table measures whole local HTTP requests. The server logs separately
  record rounded wait and processing time, which is not GPU-only time.
  There are too few samples for a defensible p99 claim.
- The recorded power-management check reports no thermal warning; this is not
  continuous thermal telemetry. GPU clock, power, and temperature were
  unavailable. This does not rule out thermal or power management and does not
  establish either as the cause of the latency drift.

## Configuration screening

The exploratory sweep stopped when one-minute CPU load reached 8.32, above
its limit of 8. The harness rejected that sample, stopped its server, and
released the quiet lease. No setting from this partial sweep is recommended.

| Candidate setting | 1k median | 4k median | 16k median |
| --- | ---: | ---: | ---: |
| Chunk 1,024 | 0.619 s | 3.958 s | 17.899 s |
| Chunk 4,096 | 1.198 s | 4.189 s | 19.414 s |
| One Rayon thread, chunk 2,048 | 1.440 s | Rejected | Not run |

Each filled cell has only three measured requests, in one fixed variant order,
after one warmup. The 4- and 8-thread settings and the final default control did
not run. All rows use the one-submission candidate. The incomplete control,
changing host load, and desktop GPU contention prevent a causal comparison.
See [all sweep records](sweep/records.jsonl), [summary](sweep/summary.json),
[analysis](sweep/analysis.json), and [lease receipt](sweep-lease.json).

## Sustained GEMM experiment

The probe compiled, but **did not run**. After the sweep stopped, the final
quiet lease waited for an existing build. Its subsequent CPU admission did not
complete within the 300-second deadline. The monitoring wrapper exited before
starting either the probe or the HTTP correctness corpus. The lease receipt
confirms release with exit code 1. See [the refusal](final-admission/summary.json),
[host samples](final-admission/host.jsonl), and [lease](final-admission/lease.json).

The retained probe measures the existing `clef_gemm` kernel at 1, 4, 16, and 48
GEMMs per command buffer. It alternates repeated and distinct weight buffers and
reverses order across five rounds. Each matrix product has 2,048 token rows,
12,288 outputs, and 4,096 inputs. It checks two exact output values and records
both GPU timestamps and wall time. This is a staged experiment, with no measured
TF/s result from this round. The cause of the previously reported sustained
throughput drop remains unresolved.

## Correctness and checks

The candidate makes each chunk’s layer stack and head-input projections share
one GPU submission. It retains the diagnostic path’s per-layer submissions.
It changes submission boundaries, not the model weights or intended arithmetic.

- All 45 measured baseline/candidate pairs produce identical answer objects.
- The weighted Metal test passes on 155- and 7,274-token records: repeated
  execution, whole-input and 2,048/512/64-token chunks, and the observer path
  have identical f32 bit patterns. Against the CPU reference on the short
  record, maximum logit difference is 2.79e-4 and probability difference is
  1.35e-5. The test took 126.36 seconds.
- The unit executable reports 20 passing test functions; five opt-in functions
  return without model work in that run. The weighted test above is separate.
- The additional 40-request HTTP correctness corpus did not run because the
  final admission timed out. No result is claimed for it. The completed weighted
  test and 45 paired answer checks above are the numerical evidence from this
  round.
- The benchmark harness self-test passes: paired nonces, reversed rounds,
  warmup exclusion, CPU/GPU admission, wait deadline, and cancellation cleanup.
- An independent read-only review recomputes all comparison and sweep medians,
  checks paired tokens and answers, verifies the patch and sanitized-config
  hashes, and finds no broken local links or obvious private-path leaks.
- Both server builds and the Swift diagnostic probe compile. Formatting checks
  find pre-existing differences, including in the untouched baseline files.
  The runtime patch is archived rather than applied, and this result commit
  changes no production Rust implementation.

These numerical checks use synthetic requests. They do not establish better
router or prepared-answer accuracy.

## What ships and what remains

This change ships the bounded benchmark harness, synthetic GEMM probe, the exact
candidate patch, raw evidence, incident report, and reproduction instructions.
The harness requires a quiet lease, checks both CPU and GPU admission by default,
has a wait deadline, and cleans up its own servers on cancellation. It never
suspends or terminates existing desktop applications.

The runtime candidate stays unpromoted. The ordered follow-up is in
[next steps](next-steps.md): obtain an idle GPU baseline, resolve sustained GEMM
behavior, retest submission boundaries, fuse attention, tune real matrix shapes,
and then revisit parallel DeltaNet and serving throughput.

See [reproduction instructions](reproduce.md),
[comparison analysis](comparison/analysis.json),
[all comparison requests](comparison/records.jsonl),
[host samples](comparison/host.jsonl), and
[separate server timing logs](comparison/server-timings.json).
Paths in retained text are replaced with `$SCRATCH`, `$REPO`, and `$TARGET`.
The patch and raw test logs retain their original whitespace for exact
reproduction; source and documentation whitespace checks exclude these raw
artifacts. Weights, executables, and full private system logs are not committed.
In metadata,
`config_sha256` covers the original configuration before replacing paths;
`sanitized_config_sha256` covers the retained configuration object. Raw run
summaries predate the final harness's additional GPU-admission disclaimer fields;
all retained comparison and sweep groups already have `usable_comparison: false`.
