# Terminal-Bench 2.0: replicating REAP's 69.1%, Khala's measured baseline, and the path to beat it (#6253)

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Status: 2026-06-26. Honesty bar (from #6253): published numbers come only from
the owner-armed real seam over the official TB-2.0 task set; pilot/attempted-only
denominators are never presented as the full score; the GLM model is attributed
to **Z.ai (REAP-pruned)**, not a serving vendor.
This is a dated evidence snapshot and methodology note; it does not assert the
current backing lane for public `openagents/khala`.

This document has three parts, matching the issue's three goals:

- **A.** How to replicate REAP's claimed **69.1%** honestly.
- **B.** Khala's **measured** TB-2.0 baseline and an inference-method comparison.
- **C.** The concrete **path to beat** it with the Khala orchestrator.

---

## A. Replicating the 69.1% (methodology)

### The claim

`0xSero/GLM-5.2-504B` — Z.ai GLM-5.2, REAP-pruned keep-168, NVFP4, MIT — is
reported at **69.1% on Terminal-Bench 2.0**, claimed as the highest TB-2.0 score
for a model that fits on 4× RTX PRO 6000 (source: the 0xSero model card
`REPORT.md` + the X post in #6253). Owner-armed Hydralisk replication lanes
serve this checkpoint for evidence runs; the public Khala route remains a
black-box router unless fresh owner-armed route evidence says otherwise.

### The official denominator

Terminal-Bench 2.0 is **89 tasks**. The harbor dataset
`terminal-bench/terminal-bench-2` resolves to exactly 89 task ids (independently
confirmed here: the runner's dataset resolution reported "There are 89 tasks
available in this dataset", and the cached task set enumerates 89 names — see
`tasks.bounded-subset.txt` header and the full list below). A "69.1%" over 89 is
**~61.5 solved** (0.691 × 89 ≈ 61.5; the issue's pilot saw ~60/87 ≈ 69% but on
an **attempted-only** denominator, which is *not* decision-grade).

### Reproduction harness

Run Terminal-Bench 2.0 through **Harbor**, the official runner, with the
`terminus-2` agent driving the model:

```sh
harbor run \
  --dataset terminal-bench/terminal-bench-2 \   # 89 tasks
  --agent terminus-2 \
  --model openai/glm-5.2-reap-504b-g4 \          # raw GLM-REAP lane (not Khala)
  --n-concurrent <C> \
  --yes
```

- **Verifier isolation:** use Harbor's `[verifier] environment_mode = "separate"`
  so the per-task `tests/test.sh` reward float in `/logs/verifier/reward.txt`
  is computed in a distinct container from the agent (the distinct-device
  verifier requirement in #6253). Map `reward.txt → accepted-outcome`.
- **Serving guardrails** (the documented REAP profile): `enable_thinking=false`;
  `repetition_penalty` 1.05 (or 1.10); **either** `min_p=0.05` **or** the
  **MTP-2 speed profile with `min_p` omitted** (vLLM rejects `min_p` under
  speculative decoding). The live owner-armed baseline uses **TP4 + MTP-2 +
  `min_p` omitted + rep-penalty 1.05**.
- **Denominator discipline:** score over all 89, counting agent-timeout /
  infra-error trials as **non-pass** (they are not solves). Do not quietly drop
  errored tasks to inflate the rate — report errors separately *and* keep them
  in the denominator.

### Replication status (2026-06-26 owner-armed snapshot)

The owner-armed full-89 Harbor run on Hydralisk is the only decision-grade
source. From `GET /api/public/gym/run-progress` at **2026-06-26 12:53 UTC**:

| run | profile | completed | passed | pass-rate / completed | errored | phase |
|---|---|---:|---:|---:|---:|---|
| `glm52-reap-baseline` | GLM-5.2 REAP 504B **TP4, MTP-2**, rep 1.05 | 19/89 | 10 | **52.6%** | 6 | running |
| `khala-live` | Khala heuristic public route | 88/89 | 22 | **25%** | 57 | completed |

**Honest read:** the raw GLM-REAP run is **partial** (19/89) and trending in the
~50–70% region, consistent with the 69.1% claim but **not yet confirmed** — it
must finish all 89 before we publish a replication number or an honest gap. The
public Khala run finished but at 25%, dragged down by 57 errored trials (a
serving/tool-calling failure mode, not model quality — see §C).

> When the GLM-REAP run completes, fill in the final line here:
> `decisionGrade: true, GLM-REAP TB-2.0 = <passed>/89 = <pct>%; gap vs 69.1% = <Δ>`.

---

## B. Khala's measured baseline + inference-method comparison

### B.1 Black-box probe of the public route (this subtree)

The 2026-06-26 public route snapshot **routed by request shape** — confirmed by
reading the `openagents.{served_model,supply_lane}` envelope on real responses.
This records the dated measurement and does not assert the current backing lane:

| request shape | served_model | supply_lane |
|---|---|---|
| plain chat completion | `openagents/glm-5.2-reap-504b` | hydralisk |
| **tool-bearing** completion | `accounts/fireworks/models/deepseek-v4-flash` | fireworks |

So the public Khala route's **Terminal-Bench path in that snapshot** was a
serving-routing result, not a pure GLM-REAP result. Do not carry this forward as
a current serving claim without a fresh owner-armed route probe.

The isolated bounded probe (`run-khala-tb2.sh`, 89-task dataset confirmed,
named-task subset, separate jobs dir) measures the end-to-end path through the
real Harbor verifier without colliding with an owner-armed decision-grade run.
Per-task results land in `last-run-summary.json` (public-safe). See
`MEASURED-RUN.md` for the recorded numbers from this environment.

### B.2 Inference-method comparison axes (issue goal B)

The owner-armed Hydralisk/Codex runs established the serving-method deltas to
score TB-2.0 against (throughput numbers from
`docs/inference/2026-06-25-glm-5.2-reap-504b-serving-audit.md` and the Hydralisk
evidence cited in #6253):

| serving method | interactive decode tok/s | TB-2.0 accuracy effect |
|---|---|---|
| 4× TP | ~35 → ~47 tok/s | baseline serving profile |
| 8× TP | **not faster** for interactive decode | no accuracy win; wastes a host |
| two independent 4× replicas | ~67 aggregate tok/s | throughput, not per-task accuracy |
| non-MTP | ~35 tok/s | baseline |
| **MTP-2 speculative + drop `min_p`** | **~47 tok/s** | speed win; **accuracy must be re-measured** because vLLM disables `min_p` under speculative decoding (sampling change → possible solve-rate change) |
| context 250K stable vs 65K fast-lane | — | longer context helps multi-file terminal tasks; measure |
| quant NVFP4 (REAP cut) vs heavier serve | — | the open question: does the pruned/NVFP4 cut cost solve-rate vs full GLM? (#6323 NVFP4-753B pilot is the comparison) |

The decision-grade comparison table — **solve-rate × cost-per-accepted-outcome ×
tok/s × TTFT per serving method** — is populated from owner-armed full-89 runs.
The harness here (and the Gym/Harbor seam) is what produces those rows; the
accuracy cells are pending the per-profile owner-armed runs. The key
**accuracy** hypothesis to test: *MTP-2 speculative decoding (which forces
dropping `min_p`) changes the sampling distribution — does it move TB-2.0
solve-rate, or only tok/s?* That is the one inference-method question that can
only be answered by scoring, not by throughput benchmarking.

---

## C. The path to beat it with Khala

The competitive goal is for the **Khala orchestrator** to exceed the raw
single-model GLM-REAP score on the same TB-2.0 tasks, on **solve-rate and/or
cost-per-accepted-outcome**. Based on the measured findings above, the path has
two stages — and the first is *not* orchestration, it's **un-breaking serving**.

### Stage 1 — Close the serving gap (Phase 0/1; why the dated Khala snapshot lagged)

In the 2026-06-26 snapshot the public Khala route scored **25%** vs the raw GLM
lane's partial **~53%+**, with errors/fallback dominating the gap:

1. **Fix GLM tool-calling (#6310).** Tool requests to the primary GLM lane
   `provider_error` ~100%, so every TB-2.0 task (all tool-driven) either errors
   or silently falls to a different model. Either (a) route tool-bearing
   requests to a healthy tool-caller cleanly, or (b) fix the GLM-5.2-REAP vLLM
   `--tool-call-parser` ↔ `--reasoning-parser` ↔ chat-template interaction.
   **#6323's NVFP4 full-753B pilot is a candidate fix:** if the full model
   tool-calls clean where the pruned REAP checkpoint errors, route the coding
   lane to it and keep REAP-504B on the 4× hosts.
2. **Repair the fallback chain (#6319).** GPT-OSS-120B returns 404, GPT-OSS-20B
   returns empty; treat empty content as failure so a 200 is never an
   empty/no-tool response. This alone removes most of the 57 errored Khala
   trials.

**Expected effect:** with tool-calling healthy, the Khala route's *baseline*
(single best model per task) should converge to the raw GLM-REAP score instead of
being dragged to 25% by infra errors. This is the prerequisite for any honest
"beat it" claim — you cannot out-orchestrate a broken transport.

### Stage 2 — Beat the single model with orchestration (the Gym thesis)

Once serving is reliable, apply the Gym policy axes and score on **solve-rate AND
cost-per-accepted-outcome** (a costlier ensemble only wins if accepted-outcome-
per-dollar justifies it):

1. **Verifier-pick / best-of-N.** For each TB-2.0 task, run N candidates
   (GLM-REAP + DeepSeek-V4-Flash + GPT-OSS-120B + Gemini Flash + a Fireworks
   lane) and **keep the one that passes the executed verifier**. Because TB-2.0
   has a real `test.sh` reward, the pick is objective — best-of-N strictly
   raises solve-rate up to the union of what any single model can solve, at the
   cost of N× tokens (so it must be gated on cost-per-accepted-outcome).
2. **Per-task coordinator routing** (heuristic / TRINITY / Conductor): cheap
   single-model on easy tasks, escalate to fan-out only on hard/long tasks.
   This is how you beat raw GLM on **cost-per-accepted-outcome** even when
   solve-rate is similar.
3. **Tool-set / module composition** per task family (git tasks vs sqlite tasks
   vs crypto tasks have different optimal tool/plugin sets).

**The beat condition (explicit):** Khala beats GLM-REAP iff, over the same
official 89 tasks, **either** `khala_solve_rate > glm_reap_solve_rate` **or**
`khala_accepted_outcomes / khala_cost > glm_reap_accepted_outcomes /
glm_reap_cost`, measured from owner-armed runs with executed verifiers. If
neither holds, document why orchestration didn't help (e.g. the union of models
adds no tasks GLM-REAP couldn't already solve, and the ensemble cost dominates).

### Why best-of-N should win on solve-rate (the mechanism)

GLM-REAP at ~69% leaves ~28 of 89 unsolved. If even one of {DeepSeek-V4-Flash,
GPT-OSS-120B, Gemini Flash} solves a subset of those 28 that GLM misses, the
verifier-pick union exceeds 69.1% with zero regression on the tasks GLM already
solves. The risk is purely **cost** (N× tokens per task) and **latency**, which
is exactly why the issue requires cost-per-accepted-outcome, not just solve-rate.

---

## Official 89-task denominator (for reproducibility)

The full set this lane targets (sorted), independently enumerated from the
resolved `terminal-bench/terminal-bench-2` dataset:

```
adaptive-rejection-sampler, bn-fit-modify, break-filter-js-from-html,
build-cython-ext, build-pmars, build-pov-ray, caffe-cifar-10,
cancel-async-tasks, chess-best-move, circuit-fibsqrt, cobol-modernization,
code-from-image, compile-compcert, configure-git-webserver,
constraints-scheduling, count-dataset-tokens, crack-7z-hash,
custom-memory-heap-crash, db-wal-recovery, distribution-search, dna-assembly,
dna-insert, extract-elf, extract-moves-from-video, feal-differential-cryptanalysis,
feal-linear-cryptanalysis, filter-js-from-html, financial-document-processor,
fix-code-vulnerability, fix-git, fix-ocaml-gc, gcode-to-text, git-leak-recovery,
git-multibranch, gpt2-codegolf, headless-terminal, hf-model-inference,
install-windows-3.11, kv-store-grpc, large-scale-text-editing, largest-eigenval,
llm-inference-batching-scheduler, log-summary-date-ranges, mailman,
make-doom-for-mips, make-mips-interpreter, mcmc-sampling-stan,
merge-diff-arc-agi-task, model-extraction-relu-logits, modernize-scientific-stack,
mteb-leaderboard, mteb-retrieve, multi-source-data-merger, nginx-request-logging,
openssl-selfsigned-cert, overfull-hbox, password-recovery, path-tracing,
path-tracing-reverse, polyglot-c-py, polyglot-rust-c, portfolio-optimization,
protein-assembly, prove-plus-comm, pypi-server, pytorch-model-cli,
pytorch-model-recovery, qemu-alpine-ssh, qemu-startup, query-optimize,
raman-fitting, regex-chess, regex-log, reshard-c4-data, rstan-to-pystan,
sam-cell-seg, sanitize-git-repo, schemelike-metacircular-eval, sparql-university,
sqlite-db-truncate, sqlite-with-gcov, torch-pipeline-parallelism,
torch-tensor-parallelism, train-fasttext, tune-mjcf, video-processing,
vulnerable-secret, winning-avg-corewars, write-compressor
```

(= 89 tasks.)
