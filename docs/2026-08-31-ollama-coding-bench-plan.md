# Benchmark plan: picking the day-to-day local coding model on the RTX 4080 box

Date: 2026-08-31
Machine: i7-14700K (28 threads), RTX 4080 16 GB VRAM (~1.9 GB used at idle by the
desktop), 125 GB RAM, 173 GB free disk. Ollama serves on `127.0.0.1:11434`.
Driver 595.58.03, CUDA 13.2.

Question this answers: **which locally-served Ollama model is the best default
for day-to-day coding through `openagents coder --model ollama:<tag>` on this
machine**, backed by measurements rather than vibes.

Related: `docs/2026-08-28-qwen38-and-ollama-audit.md` (how the local lane works
in the Rust CLI), `docs/coder/2026-08-26-psionic-local-inference-integration-analysis.md`.

---

## 1. Method

### 1.1 What we measure

Every candidate model runs the same script (`scripts/bench-ollama-coding.sh`,
committed beside this doc), which drives Ollama's HTTP API directly so the
numbers are comparable across models and independent of any CLI layer:

| Metric | Symbol | How | Why it matters for coding |
|---|---|---|---|
| Time to first token | **TTFT** (s) | wall time from request start to first streamed token | Tool-calling agents sit through this on every step; a coding session runs dozens of steps. Includes prefill of the whole prompt. |
| Prefill throughput | **PP t/s** | `prompt_eval_count / prompt_eval_duration` | The agent pattern is "long context in, few tokens out": file contents, tool results, diffs. This dominates real latency. |
| Generate throughput | **GEN t/s** | `eval_count / eval_duration` | How fast code streams out once the model starts talking. |
| Prompt size | tokens | `prompt_eval_count` | Sanity check that every model saw the same input. |
| Output length | tokens | `eval_count` | Detects a model rambling or truncating. |
| Total wall time | (s) | measured | What the user actually waits. |

Ollama reports `prompt_eval_*` / `eval_*` / `load_duration` in every
`/api/generate` response (streamed final chunk or non-streaming body), so the
metrics come from the server's own accounting, not a stopwatch on stdout.

### 1.2 The workload: four prompts, same for every model

The point is a day-to-day *coding* answer, not a vibe chat. Four prompt shapes
cover the axes that actually distinguish models:

1. **Short factual** — "What does the `?` operator do in Rust? Answer in two
   sentences." (warm-up shape, ~30 output tokens)
2. **Code generation** — write a specific, checkable function (a `Result`-returning
   parser with a exact signature). Verdict: does it compile?
3. **Long-context prefill** — a real 700-line Rust source file from this repo as
   context plus a question about it. This is the TTFT killer and the
   differentiator for agentic use: cheap models fall over here.
4. **Tool-style JSON** — "Return only a JSON object with keys `file`, `line`,
   `severity` for this diff." Agentic coding is mostly structured output; a
   model that can't emit clean JSON is a bad agent even at high tok/s.

Each prompt runs `2` times per model after 1 warm-up call (model load + shader
compile happens once, cached by the server); reported numbers are the median
of the 2 measured runs. `num_ctx` is pinned to 8192 for every model so context
window differences don't distort prefill (8192 fits the file-context prompt
with headroom on every candidate). `temperature 0`, `num_predict 512` cap.

Load behavior is recorded: `load_duration` from the API plus `/api/ps` before
and after each run, so we know whether a model spilled to CPU.

### 1.3 Fairness controls

- Models run sequentially, one at a time; `ollama stop <tag>` between models so
  no weights linger in VRAM and steal context room.
- VRAM state (`nvidia-smi --query-gpu=memory.used`) logged before each model.
- Desktop itself holds ~2 GB VRAM; a 16 GB card realistically fits ~13 GB of
  weights plus KV cache. A model whose weights exceed that will partially
  offload to CPU — that is a *result* (it will behave that way in real use),
  not a bench failure, and it shows up honestly in GEN t/s.
- Ollama reports per-run `offload` info via `/api/ps` (`size_vram` vs `size`);
  we log the VRAM-resident percentage per model.

### 1.4 Scorecard

For each model:

```
agent_score = 0.45*norm(PP t/s)      # long-context ingestion dominates agent latency
            + 0.35*norm(GEN t/s)     # snappy streaming
            + 0.20*norm(TTFT^-1)     # responsiveness on short turns
```

normalized within the candidate set. Compile/JSON checks are gates: a model
that fails them cannot win regardless of speed. Final recommendation = fastest
model that passes the gates, with a "premium" pick (best quality) and a "snappy"
pick (best latency) if they differ.

---

## 2. Candidates

### 2.1 Already installed (17 local tags; embed model excluded)

| Tag | Params | Quant | Family | Est. size on disk |
|---|---|---|---|---|
| qwen3.5:0.8b | 0.9B | Q8_0 | qwen35 | 1.0 GB |
| qwen3.5:2b | 2.3B | Q8_0 | qwen35 | 2.7 GB |
| qwen3.5:4b | 4.7B | Q4_K_M | qwen35 | 3.4 GB |
| qwen3.5:latest (9b) | 9.7B | Q4_K_M | qwen35 | 6.6 GB |
| qwen3:latest (8b) | 8.2B | Q4_K_M | qwen3 | 5.2 GB |
| qwen3:30b | 30.5B MoE (A3B) | Q4_K_M | qwen3moe | 18 GB |
| qwen3-coder:latest (30b) | 30.5B MoE (A3B) | Q4_K_M | qwen3moe | 18 GB |
| qwen35-27b-local:latest | 26.9B dense | Q4_K_M | qwen35 | 17 GB |
| qwen3.8:latest | 27.3B dense | Q4_K_M | qwen35 | 17 GB |
| qwen3.8:27b-mtp-q8_0 | 27.3B dense +MTP | Q8_0 | qwen35 | 29 GB |
| gemma4-e4b-gguf:latest | 8B (e4b) | Q4_K_M | gemma4 | 9.6 GB |
| nemotron-3-nano:latest | 31.6B MoE | Q4_K_M | nemotron_h_moe | 24 GB |
| gpt-oss:latest (20b) | 21B MoE (A3.6B) | MXFP4 | gptoss | 13 GB |
| gpt-oss:120b | 117B MoE (A5.1B) | MXFP4 | gptoss | 65 GB |
| medpsy-17b-q4-local | 2B (specialist) | Q4_K_M | qwen3 | 1.3 GB — **excluded** (not a coding generalist) |
| nomic-embed-text | 137M embed | F16 | nomic-bert | 274 MB — **excluded** (embedding model) |

### 2.2 Fresh pulls worth testing (one-time cost, disk permitting)

Chosen from ollama.com/library search (scraped 2026-08-31); tags verified to
exist on the registry. Rule of thumb: on 16 GB VRAM a dense Q4 model larger
than ~13 GB partially offloads; MoE models are the interesting case because
only the active experts cost bandwidth.

| Tag | Why pulled | Size |
|---|---|---|
| `qwen3.8-flash-next:125b-a6b-nvfp4` | Newest frontier MoE, 6B active params — the "big brain, fast tokens" bet | ~63 GB |
| `nemotron-3.5-lightning:30b-a3b` | MoE 30B/A3B, NVIDIA's agent-tuned line | ~18 GB |
| `granite4.2:8b` | IBM small dense, strong tool-calling reputation | ~5 GB |
| `glm-5.3-flash` | cloud-only tag — **not testable locally**, noted for completeness | — |
| `deepseek-v4-flash` | cloud-only — **not testable locally** | — |

Total fresh pull budget: ~86 GB. With 173 GB free this fits. (If disk pressure
appears mid-run, `qwen3.8-flash-next` is dropped first — it is a stretch goal.)

### 2.3 The complete candidate list for the run

Dense small: `qwen3.5:0.8b`, `qwen3.5:2b`, `qwen3.5:4b`, `qwen3.5:9b`,
`qwen3:8b`, `granite4.2:8b`, `gemma4-e4b`, `qwen3.8:latest`
Dense large: `qwen35-27b-local`, `qwen3.8:27b-mtp-q8_0`
MoE: `qwen3:30b`, `qwen3-coder:30b`, `nemotron-3-nano`, `gpt-oss:20b`,
`gpt-oss:120b`, `nemotron-3.5-lightning:30b-a3b`, `qwen3.8-flash-next:125b-a6b-nvfp4`

gpt-oss:120b (65 GB) and qwen3.8-flash-next (63 GB) will be CPU-offload-heavy;
they are included as reference points for "does a huge MoE still beat a small
dense model once it streams from RAM".

---

## 3. Execution order

1. `ollama pull` the four fresh tags (log pull time).
2. Run the bench script over all 17+4 candidates; it appends one JSON line per
   (model, prompt, run) to `bench-results/ollama-coding-2026-08-31.jsonl`.
3. Aggregate with a small Python script into the scorecard table in §5.
4. Verify the gates: compile check + JSON check per model.
5. Sanity-run the top two through the real path:
   `openagents coder --headless --plain --model ollama:<tag> "write a function ..."`
   and confirm the integration behaves.
6. Write the verdict into §5, commit, push.

## 4. Known pitfalls this plan accounts for

- **First call after model switch includes load.** Warm-up call absorbs it;
  `load_duration` still logged.
- **Desktop holding VRAM** (~1.9 GB) reduces effective headroom; logged, not
  tuned around.
- **MoE active-param marketing lies.** A "A3B" model may still ship huge shared
  weights; `/api/ps` `size_vram/size` is the truth.
- **num_ctx differences.** Pinned at 8192 for every model, even the ones
  advertising 262k — we are comparing serving behavior on this GPU, not
  ceiling claims.
- **Thinking models burn tokens on `<think>`.** Output token counts expose
  this; where a model thinks, its GEN t/s and wall time include the thinking
  unless we cap it. We report both raw and note which families think.

## 5. Results

Run completed 2026-08-31 on the machine described above. Raw data:
`bench-results/ollama-coding-2026-08-31.jsonl` (one JSON line per model /
prompt / run, client-side TTFT + server-reported token accounting). Aggregator:
`scripts/aggregate-ollama-bench.py`, saved output
`bench-results/scorecard-2026-08-31.txt`. Compile gate: `scripts/compile-gate-ollama.sh`
(rustc, edition 2021).

### 5.1 Incidents found by the run itself

- **gpt-oss:latest failed to load at first** — every request answered
  `HTTP 500: tensor "blk.0.ffn_down_exps.weight" size overflow`. A `ollama rm`
  + fresh `ollama pull` fixed it: the local blob was corrupt. If a model 500s
  on this machine, re-pull before diagnosing anything else. After re-pull it
  runs 100% VRAM-resident.
- **gemma4-e4b cannot serve the coder lane at all**: `openagents coder` sends
  tools on `/api/chat` and Ollama refuses gemma4 with `400 ... does not
  support tools`. It benchmarks fine standalone but is **not usable as a
  day-to-day coder model in this CLI**. Any candidate must pass the tools
  check, not just the speed check.
- **gpt-oss ignores `think:false`** (accepted, but still streams a separate
  `thinking` channel, burning output budget — visible as 248–512 output tokens
  on prompts other models answer in 25–70). Its measured tok/s are honest, but
  its wall-times overstate real answer latency.
- **qwen3:30b (non-coder) fails the JSON gate**: with `think:false` it emits
  `<think>` into the response body and spends all 512 tokens reasoning, never
  answering. The same architecture as `qwen3-coder:latest` which behaves —
  treat qwen3:30b as misconfigured/deprecated on this box.
- **qwen3.8-flash-next:125b-a6b-nvfp4 and :125b-mlx are 404 on the registry**
  despite listing on ollama.com — could not be tested. `qwen3.6:27b` and
  `qwen3.6:27b-coding` were pulled instead.

### 5.2 VRAM residency (weights in VRAM at 8192 ctx, measured via /api/ps)

| Model | VRAM % | Consequence |
|---|---|---|
| qwen3.5:0.8b / 2b / 4b / 9b | 100 | fully GPU |
| qwen3:latest (8B), gemma4-e4b, granite4.2:8b | 100 | fully GPU |
| gpt-oss:latest (20B MoE) | 100 | fully GPU (13 GB weights) |
| qwen35-27b-local | 81 | slight CPU spill |
| qwen3:30b / qwen3-coder:30b | 76 | partial offload |
| qwen3.8:latest / qwen3.6:27b | 72–73 | partial offload |
| nemotron-3-nano | 56 | major offload |
| nemotron-3.5-lightning:30b-a3b | 52 | major offload |
| qwen3.8:27b-mtp-q8_0 | 43 | mostly CPU |
| gpt-oss:120b | — | fails to load at 8192 ctx (500) |

The 16 GB card's usable budget after the desktop (~2 GB) and KV cache is
~13 GB of weights: exactly where the cliff sits. Every dense model at 17 GB+
spills, and spill costs 5–20× in tokens/s.

### 5.3 Scorecard (median over 4 prompt classes × 2 cold-prefill runs)

Median t/s across short / codegen / longctx / json; `lc-pp` = prefill t/s on
the 748-line real-file prompt — the number that governs agentic latency.

| Model | score | pp t/s | gen t/s | ttft s | lc-pp | compile | JSON | tools |
|---|---|---|---|---|---|---|---|---|
| qwen3.5:0.8b | 1.000 | 13979 | 312 | 0.02 | 33625 | ✗ | ✓ | ✓ |
| qwen3.5:2b | 0.693 | 9816 | 196 | 0.02 | 22215 | ✓ | ✓ | ✓ |
| gemma4-e4b | 0.394 | 5059 | 142 | 0.12 | 9730 | ✓ | ✓ | **✗** |
| qwen3.5:4b | 0.368 | 4384 | 149 | 0.12 | 8746 | ✓ | ✓ | ✓ |
| qwen3:latest (8B) | 0.347 | 5370 | 111 | 0.12 | 6709 | ✓ | ✓ | ✓ |
| granite4.2:8b | 0.288 | 4466 | 100 | 0.22 | 5667 | ✓ | ✓ | ✓ |
| gpt-oss:latest | 0.278 | 3716 | 144 | 2.72† | 7443 | ✓ | ✓ | ✓ |
| qwen3.5:9b | 0.252 | 3160 | 98 | 0.12 | 5807 | ✓ | ✓ | ✓ |
| qwen3:30b | 0.134 | 780 | 92 | 0.32 | 1692 | ✗ | ✗ | ✓ |
| qwen3-coder:30b | 0.126 | 681 | 89 | 0.32 | 1404 | ✓ | ✓ | ✓ |
| nemotron-3.5-lightning:30b-a3b | 0.081 | 277 | 69 | 0.82 | 630 | ✓ | ✓ | ✓ |
| nemotron-3-nano:31.6B | 0.078 | 287 | 66 | 0.82 | 657 | ✓ | ✓ | ✓ |
| qwen3.6:27b / 27b-coding | 0.030 | 343 | 22 | 0.72 | 820 | ✓ | ✓ | ✓ |
| qwen3.8:latest (27B) | 0.030 | 330 | 23 | 0.72 | 778 | ✓ | ✓ | ✓ |
| qwen35-27b-local | 0.027 | 476 | 13 | 0.62 | 1091 | ✓ | ✓ | ✓ |
| qwen3.8:27b-mtp-q8_0 | 0.000 | 133 | 6 | 1.72 | 329 | ✓ | ✓ | ✓ |

† gpt-oss TTFT is inflated by the thinking channel it streams even with
`think:false`; its true first-visible-answer is faster than measured.

### 5.4 Reading the table

- **The VRAM cliff is the whole story.** Below ~13 GB of weights, prefill runs
  4 000–34 000 t/s and generation 100–312 t/s. Above it, prefill collapses to
  300–1 700 t/s and generation to 6–92 t/s. No amount of "27B is smarter"
  survives a 15–35× speed loss on the workload that matters (long tool
  results). On this machine the day-to-day model must fit fully in VRAM.
- **MoE does not rescue size here.** qwen3-coder (30B-A3B) and the nemotron
  MoEs hold most weights in VRAM but still score far below the small dense
  models, because at 76–52% residency the offloaded slice stalls every token.
  gpt-oss:20b is the one MoE that fits 100% and it lands mid-pack (its
  thinking channel also eats wall time).
- **Thinking models are a bad default for agent steps.** qwen3.5 family thinks
  briefly; qwen3:30b thinks forever; gpt-oss thinks in a side channel. For
  tool-loop latency you want the answer, not the essay.

### 5.5 Verdict

Gates applied: must compile the parse_kv task, must emit parseable JSON, must
accept tools through `openagents coder` (gemma4 fails), must be VRAM-resident.

- **Default day-to-day model: `qwen3.5:2b`.** 196 t/s generation, 22 000 t/s
  prefill, 0.02 s TTFT, compiles, clean JSON, tool-capable, 2.7 GB — leaves
  VRAM for the desktop, a second model, or embeddings. The 0.8b is faster
  (312 t/s) but fails the compile gate; the 4b costs 25% speed for +1 size
  class of quality; 2b is the knee of the curve.
- **Quality pick when VRAM is free: `gpt-oss:latest` (20B MoE).** The largest
  model that fits 100% in VRAM on this card: gpt-oss-class reasoning at
  144 t/s generation, 7 400 t/s prefill. Cost: 13 GB VRAM (nothing else big
  can be resident), and it thinks in a side channel so cap `num_predict`.
  Use it for hard single-shot questions, not long agent loops.
- **Middle path: `qwen3.5:4b` / `qwen3:latest`.** ~110–150 t/s, still fully
  resident, more headroom than 2b on hard prompts.
- **Not recommended on this machine despite reputation:** qwen3.8 and
  qwen3.6:27b (27B dense, 15–28 t/s after offload), qwen3-coder:30b (89 t/s,
  JSON-fine but 5× slower than 2b on prefill), anything ≥30B dense, and
  gpt-oss:120b (does not load at 8192 ctx).
- **Run command:**
  `openagents coder --model ollama:qwen3.5:2b` (interactive) or
  `... --headless --plain --model ollama:qwen3.5:2b "<prompt>"` (scripts).
  Verified live: `qwen3.5:9b` and `qwen3.5:2b` both answer through the CLI
  headless path; `gemma4` refused with the tools error.

### 5.6 Re-running this benchmark

```bash
scripts/bench-ollama-coding.sh                 # all candidates, ~40 min
scripts/bench-ollama-coding.sh <tag> ...       # subset
scripts/compile-gate-ollama.sh <tag> ...       # rustc gate
python3 scripts/aggregate-ollama-bench.py      # scorecard from newest JSONL
```

Caveats: numbers are specific to this box (14700K + 4080 16 GB, desktop
holding ~2 GB VRAM, Ollama 0.32.15); a driver or Ollama upgrade can move the
VRAM cliff; `num_ctx` 8192 is pinned — larger windows shift the offload
threshold downward; results include only models pulled at run time.
