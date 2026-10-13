"""Full-model prefill through the SGL-JAX offline Engine (real scheduler, chunked prefill).

For each input length and batch size: submit B prompts of L random token ids with
max_new_tokens=1, time the generate() call (so prefill + one sampled token per prompt).
Each shape runs REPEATS times; the first includes JIT compile, the best of the rest is
the steady state. Also records engine start (model load + precompile) time and HBM.

env: MODEL TP QUANT(yaml name or "") LOAD(dummy|auto) LENS BATCHES CHUNK MAXPREFILL MEMFRAC
"""
import os, sys, time, json, random
T_START = time.time()
from common import env, write

MODEL = os.environ.get("MODEL", "Qwen/Qwen3.5-9B")
TP = int(os.environ.get("TP", "1")); QUANT = os.environ.get("QUANT", ""); LOAD = os.environ.get("LOAD", "dummy")
LENS = [int(x) for x in os.environ.get("LENS", "1024 4096 16384 32768").split()]
BATCHES = [int(x) for x in os.environ.get("BATCHES", "1 2 4 8 16 32").split()]
CHUNK = int(os.environ.get("CHUNK", "8192")); MAXPREFILL = int(os.environ.get("MAXPREFILL", "32768"))
REPEATS = int(os.environ.get("REPEATS", "3")); MAX_TOKENS = int(os.environ.get("MAX_BATCH_TOKENS", "262144"))
TAG = os.environ.get("TAG") or f"{MODEL.replace('/', '_')}-tp{TP}-{QUANT or 'bf16'}-{LOAD}"

def main():
    from sgl_jax.srt.entrypoints.engine import Engine
    kw = dict(model_path=MODEL, trust_remote_code=True, load_format=LOAD, tp_size=TP, device="tpu",
              dtype="bfloat16", mem_fraction_static=float(os.environ.get("MEMFRAC", "0.85")),
              disable_radix_cache=True, chunked_prefill_size=CHUNK, max_prefill_tokens=MAXPREFILL,
              max_running_requests=int(os.environ.get("MAXRUN", "64")), download_dir=os.path.expanduser("~/hf"),
              random_seed=3, skip_server_warmup=True, log_level="error",
              page_size=int(os.environ.get("PAGE", "64")), context_length=max(LENS) + 256,
              precompile_token_paddings=[int(x) for x in os.environ.get("TOKPAD", "1024 2048 4096 8192").split()],
              precompile_bs_paddings=[int(x) for x in os.environ.get("BSPAD", "8 64").split()],
              precompile_num_threads=int(os.environ.get("PCTHREADS", "8")))
    if QUANT:
        local = os.path.expanduser(f"~/{QUANT}.yaml")
        kw["quantization_config_path"] = local if os.path.exists(local) else os.path.expanduser(
            f"~/sglang-jax/python/sgl_jax/srt/utils/quantization/configs/{QUANT}.yaml")
    t0 = time.time()
    eng = Engine(**kw)
    t_engine = time.time() - t0
    e = env(use_jax=False); e.update(model=MODEL, tp=TP, quant=QUANT or "bf16", load=LOAD, chunk=CHUNK, max_prefill=MAXPREFILL,
                        engine_start_s=t_engine, process_start_to_engine_s=time.time() - T_START)
    try:
        si = eng.get_server_info()
        e["server_info"] = {k: v for k, v in si.items() if k in ("max_total_num_tokens", "internal_states", "version", "max_running_requests")}
    except Exception as ex:
        e["server_info_error"] = str(ex)[:200]
    print("engine up", t_engine, flush=True)
    rows = []
    rng = random.Random(0)
    first_answer = None
    for L in LENS:
        for B in BATCHES:
            if B * L > MAX_TOKENS: continue
            times = []
            err = None
            for rep in range(REPEATS):
                ids = [[rng.randrange(1000, 150000) for _ in range(L)] for _ in range(B)]
                t1 = time.time()
                try:
                    out = eng.generate(input_ids=ids, sampling_params={"max_new_tokens": 1, "temperature": 0})
                except Exception as ex:
                    err = str(ex)[:300]; break
                times.append(time.time() - t1)
                if first_answer is None: first_answer = time.time() - T_START
            r = {"input_len": L, "batch": B, "tokens": B * L, "times_s": times, "error": err}
            if len(times) > 1:
                best = min(times[1:]); r.update(steady_s=best, tok_s=B * L / best, first_s=times[0])
            elif times:
                r.update(steady_s=times[0], tok_s=B * L / times[0], first_s=times[0])
            rows.append(r); print(json.dumps(r), flush=True)
            write(f"prefill-{TAG}-{e['host']}.json", {"env": e, "first_answer_since_process_start_s": first_answer, "rows": rows})
            if err: break
    eng.shutdown()

if __name__ == "__main__":
    main()
