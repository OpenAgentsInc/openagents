# Research log: 2026-08-31, Phase 0 in progress

This is a dated research note, not a plan change and not an admission. It
records what actually happened the day this folder went from "proposed" to
"started", and what should happen next. Precedence stays where it was: the
reading order above is still the plan of record; nothing here overrides it.

## What happened on 2026-08-31

A Coder session on the RTX 4080 Linux box (i7-14700K, 28 threads, RTX 4080
16 GB, 125 GB RAM, driver 595.58.03, CUDA 13.2, Ollama 0.32.15 on
`127.0.0.1:11434`) did the first Phase 0 work this folder describes:

1. **Linux build and integration, verified.** `~/openagents` (a separate
   checkout from this one) was fast-forwarded to `7c8c7c57` ("Add the
   Streamable HTTP MCP client to the CLI"). `cargo build --release -p
   openagents-cli` succeeded in ~1m04s on rustc 1.88.0, and the resulting
   binary reports `OpenAgents v0.0.0-dev`. `--help` and subcommand help work;
   the Ollama lane was verified end to end against the live Ollama server.
   This is success criterion (1) from [LINUX-CLI.md](./LINUX-CLI.md) —
   install/build/run on plain Linux — for one box.

2. **A systematic model benchmark was planned and started.** Deliverables in
   the `~/openagents` checkout (uncommitted at the time of this note):
   `docs/2026-08-31-ollama-coding-bench-plan.md` (8.9 KB) and its companion
   runner `scripts/bench-ollama-coding.sh` (8.7 KB), plus results accumulating
   in `bench-results/ollama-coding-2026-08-31.jsonl` (9 rows when the session
   paused).

The benchmark plan is a real Phase 0 artifact and its method is sound. What it
measures, why, and the controls worth knowing before reading any verdict:

- **Metrics from the server, not a stopwatch.** TTFT (first streamed token),
  prefill throughput (`prompt_eval_count / prompt_eval_duration`), and
  generate throughput (`eval_count / eval_duration`) come from Ollama's own
  `/api/generate` accounting. The agent pattern is "long context in, few
  tokens out", so prefill throughput is the number that dominates real
  coding-agent latency; generate t/s matters for streaming feel; TTFT covers
  short-turn responsiveness.
- **Four workload shapes, identical across models:** short factual; compile-
  checkable code generation; long-context prefill over a real 700-line Rust
  file from the repo (the TTFT killer, and the differentiator for agentic
  use); and tool-style JSON emission. Compile and JSON checks are gates, not
  scores — a model that fails them cannot win on speed.
- **Fairness controls:** `num_ctx` pinned at 8192 for every model (serving
  behavior on this GPU, not context-ceiling marketing); temperature 0;
  `num_predict` 512; one warm-up call plus two measured runs, medians
  reported; `ollama stop` between models so no weights linger in VRAM; VRAM
  state and `/api/ps` offload info (`size_vram` vs `size`) logged per model,
  so partial CPU offload shows up honestly as a result rather than hiding in
  the numbers.
- **Scorecard:** `0.45*norm(PP) + 0.35*norm(GEN) + 0.20*norm(TTFT^-1)`,
  gated by the compile/JSON checks. A "premium" pick and a "snappy" pick are
  separated if they differ.
- **Candidates:** 16 installed tags (qwen3.5 family 0.8b–9b, qwen3:8b, qwen3.8
  27B dense in Q4_K_M and Q8_0+MTP, qwen35-27b dense, qwen3:30b MoE,
  qwen3-coder:30b MoE, gemma4-e4b, nemotron-3-nano MoE, gpt-oss 20b/120b;
  the medpsy specialist and the nomic embedding model excluded), plus fresh
  pulls `granite4.2:8b` (pulled, 5.3 GB),
  `nemotron-3.5-lightning:30b-a3b` (~18 GB), and the stretch bet
  `qwen3.8-flash-next:125b-a6b-nvfp4` (~63 GB). Two frontier tags
  (`glm-5.3-flash`, `deepseek-v4-flash`) are cloud-only and noted as not
  locally testable.

Two methodology bugs were already found and fixed in the script while proving
it on `qwen3.5:0.8b`, both the kind that silently fake a result:

- **The KV prefix cache was scoring warm-cache runs.** A unique nonce appended
  after the long-context file still left the whole file prefix cached, so
  "longctx" run 2 reported ~157k t/s prefill. Moving the nonce *before* the
  prompt body defeats the prefix and every measured run now prefills from
  zero — longctx runs read ~33.5k t/s, consistent across both runs.
- **gpt-oss:latest fails hard on this box** with `tensor
  "blk.0.ffn_down_exps.weight" size overflow` — an Ollama-side load failure
  for that tag on this GPU/driver combination, recorded as a result, not
  worked around.

## How this maps onto the plan of record

- [LINUX-CLI.md](./LINUX-CLI.md) workstream **C** (GPU probes) and **D**
  (in-process inference) are untouched — the benchmark drives Ollama, which
  the plan calls a temporary compatibility lane, not the product. What moved
  is the *evidence base*: for the first time there is a measurement framework
  for "which local model is the right default on a real Linux GPU box", which
  is the same question `openagents inference doctor` will eventually answer
  about admitted GGUFs. The probe-and-admission pattern is shared; only the
  serving layer differs.
- [OPEN-QUESTIONS.md](./OPEN-QUESTIONS.md) **Q15** (first Linux GPU backend)
  gains its first hard data point from this machine: CUDA 13.2, driver 595.58,
  and honest offload accounting for models above ~13 GB of weights.
- **Q16** (first Linux model artifact) is being answered in its Ollama-lane
  form: the benchmark is exactly a "which quant/family fits a 16 GB card and
  codes well" study, pre-admission.

## What should happen next, once the benchmark run succeeds

The steps below assume the full candidate matrix completes and produces a
clean scorecard. They are sequenced so that each one only starts when the
previous one has produced its artifact.

### 1. Land the artifacts first

Commit and push `docs/2026-08-31-ollama-coding-bench-plan.md`,
`scripts/bench-ollama-coding.sh`, the aggregated results (the JSONL plus a
small aggregate script/table — per the plan's §3, results live under
`bench-results/` and the verdict goes into the plan doc's §5), and the
completed §5 verdict. The bench plan itself says results are filled in after
the run; a plan doc with an empty §5 and no committed results is not a
finished artifact. Public-safe only: no absolute home paths in committed
results beyond what the repo already records, and no secrets (there are none
in this lane).

### 2. Turn the scorecard into a real-use verification

Run the top two models through the actual product path, not just the raw API:
`openagents coder --headless --plain --model ollama:<tag>` on a real multi-
round tool turn (the plan's §3 step 5). The API benchmark proves throughput;
only the product path proves the integration behaves — tool-call loops,
`--prompt` flushing, session store writes, interruption. A model that wins
the scorecard but fumbles tool calls loses the job. Record both verdicts
(winner on the bench, winner in-product) even if they differ; if they differ,
say why.

### 3. Publish an honest recommendation with an expiry

Write the verdict as a *dated recommendation with an explicit scope*, in the
plan doc's §5: best default for day-to-day coding through
`openagents coder --model ollama:<tag>` on RTX 4080 16 GB + i7-14700K, as of
2026-08-31, with the runner-up, the gates each model passed or failed, and
the offload boundary where the recommendation flips. A recommendation without
its scope will be quoted somewhere the scope does not hold (this folder's
whole subject is how unscoped claims drift); one with scope can be re-run and
replaced when the hardware or model set changes.

### 4. Make the benchmark repeatable, not one-shot

The script already parameterizes candidates; two small additions make the
result durable instead of a single-day snapshot:

- a `--from-results` aggregation mode (or a small companion script) so the
  scorecard can be regenerated from the JSONL without re-running; and
- a dated re-bench trigger written into the verdict: re-run when Ollama
  ships a new version, when a new candidate family lands (the plan already
  tracked ollama.com/library for this), or when the GPU/driver changes.

Repeatability is what turns "we measured once" into Phase 0 evidence that
`inference doctor` can later cite.

### 5. Feed the winners into the Psionic Linux backends (the actual product lane)

This is the bridge from the Ollama compatibility lane to the plan of record.
Once the bench names the best-fitting families and quants for this class of
hardware, the next program is the one [CODEROS.md](./CODEROS.md) and
[LINUX-CLI.md](./LINUX-CLI.md) actually ask for: **in-process Psionic
inference on Linux** — CPU backend first on `linux-x86_64` (gnu and musl),
then the CUDA packet on this box. The benchmark's output becomes the admission
input: which family (qwen3.5/qwen3.8), which quant (Q4_K_M vs Q8_0), and what
"fits a 16 GB card" means in practice, validated by the same metrics the CLI
will report. Success criteria (2)–(3) in [CODEROS.md](./CODEROS.md) —
`inference run` through generate without Ollama, then `openagents coder
--local` completing a multi-round tool turn against the in-process engine —
are the next gates, and this benchmark supplies the model selection and the
baseline numbers to compare against.

### 6. Extend the evidence, box by box

The plan's Phase 0 logic is per-machine evidence, not a support matrix. The
natural next boxes, in order of information value:

- a second NVIDIA box (different VRAM class) to test where the offload
  boundary actually sits;
- an AMD/ROCm or Vulkan-only box for the non-NVIDIA path;
- a NixOS host running the **musl** artifact (already the installer's NixOS
  case), to confirm the benchmark and doctor run there unchanged.

Each re-run should reuse the same script and prompts, appending a dated
results file — the script was built to make this free.

### 7. Only then, the distro questions

The benchmark plus (5) give Phase 0 the "Coder + local models are excellent
on Linux" evidence that [CODEROS.md](./CODEROS.md)'s sequencing law requires
before any substrate or profile work. When both the Ollama lane verdict and
the first in-process Linux generate exist, [SUBSTRATE.md](./SUBSTRATE.md)'s
study order becomes actionable — NixOS module first prototype, CUDA
enablement as the first hard test — and [OPEN-QUESTIONS.md](./OPEN-
QUESTIONS.md) Q15/Q16 close with evidence instead of hypotheses. Until then,
no ISO, no profile, no substrate decision.

## What not to do yet (unchanged from the plan of record)

- Do not read the scorecard as a Psionic admission decision. It is an
  Ollama-lane measurement on one box; the acceptance/promotion authority for
  in-process models remains the Psionic program's gates.
- Do not enable CUDA in the macOS artifact because a Linux box has CUDA
  working — [docs/psionic/PLAN.md](../psionic/PLAN.md) keeps that boundary.
- Do not promote `openagents inference doctor` claims from this benchmark;
  doctor needs named probes (libc, GPU, display, docker, coredump) that this
  script does not implement.
- Do not start a substrate choice, a CoderOS profile, or an ISO while the
  benchmark is still uncommitted and the in-process lane has not produced its
  first Linux generate.
