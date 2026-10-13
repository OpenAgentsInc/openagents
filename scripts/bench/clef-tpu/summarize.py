"""Summarize campaign JSON (synced from gs://openagentsgemini-decision-throughput/tpu/<date>/) into
markdown tables. usage: python3 summarize.py DATA_DIR [ops_dir]"""
import json, glob, os, sys, collections
D = sys.argv[1]
ref = {"H100 spot (Psionic Q4)": 0.11, "A100 spot (Psionic Q4)": 0.072, "Jev price": 0.042}
LAYERS = {"flash": {"gdn": 24, "attn": 8}, "27b": {"gdn": 48, "attn": 16}}
PROJ = {"gdn": ["gdn_qkv", "gdn_z", "gdn_out", "mlp_gate_up", "mlp_down"],
        "attn": ["attn_q_gate", "attn_kv", "attn_o", "mlp_gate_up", "mlp_down"]}

def load(pat):
    out = []
    for f in sorted(glob.glob(os.path.join(D, "**", pat), recursive=True)):
        try: d = json.load(open(f)); d["_file"] = os.path.relpath(f, D); out.append(d)
        except Exception: pass
    return out

def per_m(x):  # $/M tokens at tok/s and $/h
    return None

print("## Matmul (one chip), best TFLOPS by dtype at 8,192 tokens, and the projection-time model\n")
mm = load("matmul-*.json")
for d in mm:
    e = d["env"]; rows = [r for r in d["rows"] if "tflops" in r]
    by = collections.defaultdict(dict)
    for r in rows: by[(r["shape"], r["dtype"])][r["M"]] = r
    dtypes = sorted({r["dtype"] for r in rows})
    print(f"**{e['chip']}** ({e['device_kind']}, jax {e['jax']}, {e['zone']}, `{d['_file']}`)\n")
    print("| shape (K x N) | " + " | ".join(dtypes) + " |"); print("| --- |" + " ---: |" * len(dtypes))
    for s in sorted({k[0] for k in by}):
        cells = []
        for t in dtypes:
            r = by.get((s, t), {}).get(8192); cells.append(f"{r['tflops']:.0f}" if r else "—")
        K, N = next(iter(by[(s, dtypes[0])].values()))["K"], next(iter(by[(s, dtypes[0])].values()))["N"]
        print(f"| {s} ({K} x {N}) | " + " | ".join(cells) + " |")
    print()
    # projection time per token for the whole backbone, by M and dtype
    print("Backbone projection time per token (sum of the measured matmuls over all layers; no attention/GDN core, norms, or overheads):\n")
    print("| model | dtype | " + " | ".join(f"{m}" for m in [512, 2048, 8192, 32768]) + " | tok/s at 8k |")
    print("| --- | --- |" + " ---: |" * 5)
    for model in ("flash", "27b"):
        for t in dtypes:
            cells = []; tps8 = None
            for M in [512, 2048, 8192, 32768]:
                tot = 0; ok = True
                for kind, n in LAYERS[model].items():
                    for p in PROJ[kind]:
                        r = by.get((f"{model}.{p}", t), {}).get(M)
                        if not r: ok = False; break
                        tot += n * r["seconds"]
                if ok:
                    cells.append(f"{tot / M * 1e6:.1f} µs")
                    if M == 8192: tps8 = M / tot
                else: cells.append("—")
            print(f"| {model} | {t} | " + " | ".join(cells) + f" | {tps8:,.0f} |" if tps8 else f"| {model} | {t} | " + " | ".join(cells) + " | — |")
    print()

print("## Gated DeltaNet (gdn/v3 fused Pallas kernel), one layer, one chip\n")
for d in load("gdn-*.json"):
    if "state" in d["_file"]: continue
    e = d["env"]
    print(f"**{e['chip']}** (`{d['_file']}`)\n")
    print("| config | batch x seq | state | fused tok/s | µs/token over all GDN layers | pure-JAX chunked tok/s | err vs token-step ref (mean rel / max abs) |")
    print("| --- | --- | --- | ---: | ---: | ---: | --- |")
    for r in d["rows"]:
        n = LAYERS[r["config"]]["gdn"]
        f = f"{r['fused_tok_s']:,.0f}" if "fused_tok_s" in r else r.get("fused_error", "")[:60]
        us = f"{1e6 / r['fused_tok_s'] * n:.1f}" if "fused_tok_s" in r else "—"
        c = f"{r['chunked_jax_tok_s']:,.0f}" if "chunked_jax_tok_s" in r else "—"
        err = f"{r['err_rel']:.2e} / {r['err_max']:.1e}" if "err_rel" in r else "—"
        print(f"| {r['config']} ({r['n_kq']}/{r['n_v']}) | {r['batch']} x {r['seq']} | {r['state']} | {f} | {us} | {c} | {err} |")
    print()
for d in load("gdn-state-*.json"):
    print(f"State-pool dtype over chunked prefill ({d['env']['chip']}, `{d['_file']}`):\n")
    print("| config | T | chunk | pool | vs single call (mean rel) | vs ref (mean rel) | last chunk vs ref |"); print("| --- | ---: | ---: | --- | ---: | ---: | ---: |")
    for r in d["rows"]:
        print(f"| {r['config']} | {r['T']} | {r['chunk']} | {r['state_pool']} | {r['vs_single_call_mean_rel']:.2e} | {r.get('vs_ref_mean_rel', float('nan')):.2e} | {r.get('last_chunk_vs_ref_mean_rel', float('nan')):.2e} |")
    print()

print("## Causal attention, head_dim 256, one chip (best implementation per length)\n")
for d in load("attn-*.json"):
    e = d["env"]; best = {}
    for r in d["rows"]:
        if "tflops" in r:
            k = (r["config"], r["seq"])
            if k not in best or r["tflops"] > best[k]["tflops"]: best[k] = r
    print(f"**{e['chip']}** (`{d['_file']}`)\n")
    print("| config | seq | best impl | TFLOPS | µs/token per layer |"); print("| --- | ---: | --- | ---: | ---: |")
    for k in sorted(best): r = best[k]; print(f"| {r['config']} {r['nq']}/{r['nkv']} | {r['seq']} | {r['impl']} | {r['tflops']:.0f} | {r['us_per_token']:.2f} |")
    print()

print("## Full-model prefill (SGL-JAX Engine)\n")
for d in load("prefill-*.json"):
    e = d["env"]; chips = e.get("tp") or 1
    spot, od = e.get("price_spot_chip_h"), e.get("price_ondemand_chip_h")
    print(f"**{e.get('model')}** on {e['chip']} TP {chips}, {e.get('quant')}, load {e.get('load')}, chunk {e.get('chunk')} — engine start {e.get('engine_start_s', 0):.0f} s (`{d['_file']}`)\n")
    print("| input len | batch | steady s | tok/s | tok/s per chip | spot $/M | on-demand $/M | first (compile) s |")
    print("| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for r in d["rows"]:
        if not r.get("tok_s"): print(f"| {r['input_len']} | {r['batch']} | error: {str(r.get('error'))[:80]} | | | | | |"); continue
        pc = r["tok_s"] / chips
        sm = spot / 3600 / pc * 1e6 if spot else None; om = od / 3600 / pc * 1e6 if od else None
        print(f"| {r['input_len']} | {r['batch']} | {r['steady_s']:.3f} | {r['tok_s']:,.0f} | {pc:,.0f} | {sm:.4f} | {om:.4f} | {r.get('first_s', 0):.2f} |")
    print()
