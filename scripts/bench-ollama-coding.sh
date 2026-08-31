#!/usr/bin/env bash
# Benchmark every candidate Ollama model for day-to-day coding on this machine.
# Companion to docs/2026-08-31-ollama-coding-bench-plan.md
#
# Usage: scripts/bench-ollama-coding.sh [tag ...]
#   With no arguments, runs the full candidate list from the plan doc.
# Appends one JSON line per (model, prompt, run) to
#   bench-results/ollama-coding-<date>.jsonl
#
# Metrics per run: TTFT (client-measured), prompt/eval tokens + durations
# (server-reported), wall time, load_duration, VRAM residency from /api/ps.
#
# Measurement rules:
#   - `think: false` is requested so output tokens are answer tokens, not
#     reasoning tokens; models that cannot disable thinking fall back to
#     leaving it on, and the row records that.
#   - Every request carries a unique nonce suffix, so Ollama's prefix KV
#     cache never serves a repeat prefill: every run's prompt_eval is cold.
#   - Errors (model fails to load, request refused) become JSON rows, not
#     silence: a model that cannot run is a benchmark result.
set -euo pipefail

OLLAMA=${OLLAMA:-http://127.0.0.1:11434}
NUM_CTX=8192
NUM_PREDICT=512
RUNS=2
HERE="$(cd "$(dirname "$0")" && pwd)"
RESULTS="$HERE/../bench-results"
mkdir -p "$RESULTS"
OUT="$RESULTS/ollama-coding-$(date +%F).jsonl"

# The candidate list from the plan doc §2.3.
DEFAULT_CANDIDATES=(
  qwen3.5:0.8b
  qwen3.5:2b
  qwen3.5:4b
  qwen3.5:9b
  qwen3:latest
  gemma4-e4b-gguf:latest
  qwen35-27b-local:latest
  qwen3.8:latest
  qwen3.8:27b-mtp-q8_0
  qwen3:30b
  qwen3-coder:latest
  nemotron-3-nano:latest
  gpt-oss:latest
  gpt-oss:120b
  granite4.2:8b
  nemotron-3.5-lightning:30b-a3b
  qwen3.8-flash-next:125b-a6b-nvfp4
)
CANDIDATES=("${@:-${DEFAULT_CANDIDATES[@]}}")

# The long-context prompt reads a real source file from this repo.
CONTEXT_FILE="$HERE/../crates/openagents-cli/src/session_store.rs"
[ -f "$CONTEXT_FILE" ] || CONTEXT_FILE="$HERE/../crates/openagents-cli/src/swarm.rs"

vram_used_mib() {
  nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits 2>/dev/null \
    | head -1 | tr -d ' ' || echo "n/a"
}

ps_vram_pct() {  # % of the named model resident in VRAM right now
  curl -s "$OLLAMA/api/ps" | python3 -c "
import json,sys
try: d=json.load(sys.stdin)
except Exception: print('n/a'); raise SystemExit
for m in d.get('models',[]):
    if m.get('name')=='$1' or m.get('model')=='$1':
        s=m.get('size') or 0; v=m.get('size_vram') or 0
        print(f'{(100*v/s):.0f}' if s else 'n/a'); raise SystemExit
print('not-loaded')
"
}

# --- per-run worker ---------------------------------------------------------
# One python process per (model, prompt) so a crash never loses the run.
bench_pair() {  # $1=model  $2=prompt-name  $3=run-index
  local model="$1" pname="$2" run="$3"
  OLLAMA_HOST="$OLLAMA" PROMPT_NAME="$pname" MODEL="$model" RUN="$run" \
  NUM_CTX="$NUM_CTX" NUM_PREDICT="$NUM_PREDICT" OUT="$OUT" \
  CONTEXT_FILE="$CONTEXT_FILE" python3 <<'PYEOF'
import json, os, time, urllib.request, urllib.error

OLLAMA   = os.environ["OLLAMA_HOST"]
model    = os.environ["MODEL"]
pname    = os.environ["PROMPT_NAME"]
run      = os.environ["RUN"]
num_ctx  = int(os.environ["NUM_CTX"])
num_pred = int(os.environ["NUM_PREDICT"])
out_path = os.environ["OUT"]
ctx_file = os.environ.get("CONTEXT_FILE", "")

def emit(row):
    with open(out_path, "a") as f:
        f.write(json.dumps(row) + "\n")
    if "error" in row:
        print(f"  {model:44s} {pname:8s} run{run}  ERROR: {row['error'][:120]}")
    else:
        print(f"  {model:44s} {pname:8s} run{run}  "
              f"ttft={row['ttft_s']}s  gen={row['gen_tps']}t/s  "
              f"pp={row['prompt_tps']}t/s  out={row['gen_tokens']}tok  "
              f"wall={row['wall_s']}s")

prompts = {
    "short": "What does the ? operator do in Rust? Answer in two sentences.",
    "codegen": ("Write a Rust function: pub fn parse_kv(line: &str) -> "
        "Result<Vec<(String, String)>, String> that parses 'key=value' pairs "
        "separated by ';' into a vector, trimming whitespace, returning Err on "
        "any entry without '='. Return ONLY the function, no prose."),
    # longctx is built below from the context file
    "json": ("Review this diff and return ONLY a JSON object with keys file, "
        "line, severity (severity is one of low|medium|high):\n\n--- DIFF ---\n"
        "- fn total(a: i32, b: i32) -> i32 { a + b }\n"
        "+ fn total(a: i32, b: i32) -> i32 {\n"
        "+     let buf = a.to_string() + &b.to_string();\n"
        "+     buf.parse().unwrap_or(0)\n"
        "+ }\n--- END DIFF ---"),
}
if pname == "longctx":
    with open(ctx_file, "r", errors="replace") as f:
        src = f.read()
    prompts["longctx"] = (
        "Below is a Rust source file. Answer in at most 3 sentences: what is "
        "the largest struct in this file and what is it for?\n\n--- FILE START "
        "---\n" + src + "\n--- FILE END ---")

# The nonce defeats the server's prefix KV cache. It goes BEFORE the prompt
# body so the whole prefix — including the long-context file — differs on
# every run: every measured run prefills from zero, the way the first turn
# of a session does.
nonce = f"[bench {os.getpid()}-{time.time_ns()}]\n\n"
prompt_text = nonce + prompts[pname]

def request_body(think):
    body = {
        "model": model,
        "prompt": prompt_text,
        "stream": True,
        "options": {"num_ctx": num_ctx, "num_predict": num_pred,
                    "temperature": 0},
    }
    if think is not None:
        body["think"] = think
    return json.dumps(body).encode()

def stream_once(body):
    ttft, chunks, final = None, [], {}
    started = time.perf_counter()
    req = urllib.request.Request(
        OLLAMA + "/api/generate", data=body,
        headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=1800) as resp:
        for line in resp:
            doc = json.loads(line)
            if ttft is None and doc.get("response"):
                ttft = time.perf_counter() - started
            if doc.get("response"):
                chunks.append(doc["response"])
            if doc.get("done"):
                final = doc
    return ttft, "".join(chunks), final, time.perf_counter() - started

try:
    try:
        ttft, text, final, wall = stream_once(request_body(False))
        think_mode = "disabled"
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")
        # Models that cannot turn thinking off refuse `think:false`.
        if "think" in detail.lower():
            ttft, text, final, wall = stream_once(request_body(True))
            think_mode = "on (model refused disable)"
        else:
            raise
except Exception as e:
    emit({"ts": time.strftime("%FT%T", time.gmtime()),
          "model": model, "prompt": pname, "run": int(run),
          "error": f"{type(e).__name__}: {e}"})
    raise SystemExit(0)

prompt_tokens = final.get("prompt_eval_count")
prompt_dur = final.get("prompt_eval_duration")
eval_tokens = final.get("eval_count")
eval_dur = final.get("eval_duration")

emit({
    "ts": time.strftime("%FT%T", time.gmtime()),
    "model": model, "prompt": pname, "run": int(run),
    "think": think_mode,
    "ttft_s": round(ttft, 3) if ttft is not None else None,
    "wall_s": round(wall, 3),
    "load_s": round(final.get("load_duration", 0) / 1e9, 3),
    "prompt_tokens": prompt_tokens,
    "prompt_tps": round(prompt_tokens / (prompt_dur / 1e9), 1)
        if prompt_tokens and prompt_dur else None,
    "gen_tokens": eval_tokens,
    "gen_tps": round(eval_tokens / (eval_dur / 1e9), 1)
        if eval_tokens and eval_dur else None,
    "thought_in_channel": bool(final.get("thinking")),
    "output_preview": text[:400],
})
PYEOF
}

# --- main loop -------------------------------------------------------------
echo "candidates: ${CANDIDATES[*]}"
echo "results -> $OUT"
echo "context file: $CONTEXT_FILE ($(wc -l < "$CONTEXT_FILE") lines)"

for model in "${CANDIDATES[@]}"; do
  echo "=== $model ==="
  if ! curl -s "$OLLAMA/api/tags" | python3 -c "
import json,sys
sys.exit(0 if any(m['name']=='$model' for m in json.load(sys.stdin).get('models',[])) else 1)
"; then
    echo "  SKIP (not pulled)"
    echo "{\"ts\":\"$(date -u +%FT%T)\",\"model\":\"$model\",\"skipped\":\"not-pulled\"}" >> "$OUT"
    continue
  fi
  echo "  vram_used=$(vram_used_mib) MiB"
  # warm-up: one throwaway call absorbs model load and shader compile
  bench_pair "$model" short 0 >/dev/null 2>&1 || true
  for pname in short codegen longctx json; do
    for r in $(seq 1 "$RUNS"); do bench_pair "$model" "$pname" "$r" || true; done
  done
  echo "  vram_resident_pct=$(ps_vram_pct "$model")"
  ollama stop "$model" >/dev/null 2>&1 || true
done
echo "done: $OUT"
