# 267. A looped MoE transformer

A standard decoder runs each physical layer once per token. **Inkling 2 Mini**
(`24` physical layers, `d = 2304`, `128` experts / `top_k = 8`, `26.4B` total /
`3.4B` active / `5.4B` weight activations per token) runs its `12` middle layers
(`6..17`) **three times per token** (`K = 3`), reusing the same weights across
passes without mutating them:

- **Prelude (`0..5`)**: 6 physical layers executed once (`slots 0..5`).
- **Looped middle block (`6..17`)**: 12 physical layers executed `K` times with
  per-pass KV-cache and shortconv slots (`pass 0` in `slots 6..17`, `pass 1` in
  `slots 24..35`, `pass 2` in `slots 36..47`), plus cross-pass message mixing
  from the previous pass's residual bank back onto the saved prelude state.
- **Coda (`18..23`)**: 6 physical layers executed once (`slots 18..23`).

That yields `6 + 12 × 3 + 6 = 48` logical layer steps per token on Inkling 2
Mini (`K = 3`), and generalizes cleanly to configurable `K >= 1` for
`TML-d24-varloop-research` (`K = 1 → 24` slots, `K = 3 → 48` slots, `K = 5 → 72`
slots).

Building on [chapter 61](ch061-a-few-experts-then-a-router.md) (MoE routing) and
[chapter 86](ch086-sparse-attention-and-shared-layers.md) (cross-layer state
sharing), [`psionic/lib/inkling-loop.coax`](../lib/inkling-loop.coax) implements
the `#authority` looped schedule, slot/physical-layer bijection, and full-math
Looped MoE reference (RMSNorm, causal scaled dot-product Q/K/V attention,
per-pass KV cache, cross-pass recurrence gate, top-k softmax MoE router with
per-expert SiLU FFNs, and genuine layer-major prefill vs. step-by-step decode),
while [`psionic/platform/inkling-metal.c`](../platform/inkling-metal.c) executes
the same looped schedule on Metal 4 against the 17.6 GB `Inkling-2-Mini-IQ4_NL.gguf`
checkpoint with configurable `--loops K`.

```shell
$ coax run psionic/examples/looped-moe-transformer.coax
Inkling 2 Mini schedule: K=1 -> 24 slots, K=3 -> 48 slots, K=5 -> 72 slots
K=1 prefill==decode exact; tok2_x1e4=8403,4571,-12231,4587
K=3 prefill==decode exact; tok2_x1e4=8987,7882,-12593,5198
K=5 prefill==decode exact; tok2_x1e4=9266,9821,-12817,5414
```

## Looped schedule and per-pass KV slots

Every `(stage, layer_in_stage, pass)` maps to a unique logical slot `il` via
`inkling_logical_slot` (and back to its physical weight owner `ip` via
`inkling_physical_layer`), so pass 2 and pass 3 never overwrite pass 1's KV
cache or shortconv history. Invalid loop counts (`K < 1` or `K > 16`), invalid
layer counts, and out-of-bounds slots are refused before execution.

<!-- source: psionic/lib/inkling-loop.coax types -->
```jai
InklingLoopStatus :: enum u8 {ok;shape;limit;pass;capacity;}
InklingLoopStage :: enum u8 {prelude;loop_body;postlude;}
InklingLoopConfig :: struct {
    prelude_layers:int;
    loop_layers:int;
    postlude_layers:int;
    loop_count:int;
    dim:int;
    experts:int;
    top_k:int;
    max_seq:int;
}
InklingLoopPlan :: struct {
    status:InklingLoopStatus;
    total_steps:int;
    kv_slots:int;
}
InklingMoEWeights :: struct {
    hidden:int;
    wq:[]f32;          // [n_phys * dim * dim]
    wk:[]f32;          // [n_phys * dim * dim]
    wv:[]f32;          // [n_phys * dim * dim]
    wo:[]f32;          // [n_phys * dim * dim]
    w_router:[]f32;    // [n_phys * experts * dim]
    w_up:[]f32;        // [n_phys * experts * hidden * dim]
    w_down:[]f32;      // [n_phys * experts * dim * hidden]
    w_loop_gate:[]f32; // [loop_layers * dim]
}
```

<!-- source: psionic/lib/inkling-loop.coax plan -->
```jai
inkling_loop_plan :: (cfg:InklingLoopConfig)->InklingLoopPlan #export_c #effects(all) {
    if cfg.loop_count < 1 || cfg.loop_count > INKLING_LOOP_MAX_PASSES {
        return InklingLoopPlan.{status=InklingLoopStatus.pass};
    }
    if cfg.prelude_layers < 0 || cfg.loop_layers < 1 || cfg.postlude_layers < 0 {
        return InklingLoopPlan.{status=InklingLoopStatus.shape};
    }
    if cfg.dim < 1 || cfg.dim > INKLING_PROD_MAX_DIM || cfg.experts < 1 || cfg.experts > INKLING_PROD_MAX_EXPERTS {
        return InklingLoopPlan.{status=InklingLoopStatus.limit};
    }
    if cfg.top_k < 1 || cfg.top_k > cfg.experts || cfg.max_seq < 1 || cfg.max_seq > INKLING_PROD_MAX_SEQ {
        return InklingLoopPlan.{status=InklingLoopStatus.shape};
    }
    total_steps := cfg.prelude_layers + cfg.loop_layers * cfg.loop_count + cfg.postlude_layers;
    return InklingLoopPlan.{status=InklingLoopStatus.ok, total_steps=total_steps, kv_slots=total_steps * cfg.max_seq};
}

// Logical KV/shortconv slot index matching psionic/platform/inkling-metal.c:
//   - Pass 0 and prelude/postlude occupy slots [0 .. n_phys)
//   - Recurrent passes p >= 1 occupy slots n_phys + (p - 1) * loop_layers + layer_in_stage
inkling_logical_slot :: (cfg:InklingLoopConfig, stage:InklingLoopStage, layer_in_stage:int, pass_idx:int)->int #export_c #effects(all) {
    if layer_in_stage < 0 { return -1; }
    n_phys := cfg.prelude_layers + cfg.loop_layers + cfg.postlude_layers;
    if stage == InklingLoopStage.prelude {
        if pass_idx != 0 || layer_in_stage >= cfg.prelude_layers { return -1; }
        return layer_in_stage;
    } else if stage == InklingLoopStage.loop_body {
        if pass_idx < 0 || pass_idx >= cfg.loop_count || layer_in_stage >= cfg.loop_layers { return -1; }
        if pass_idx == 0 { return cfg.prelude_layers + layer_in_stage; }
        return n_phys + (pass_idx - 1) * cfg.loop_layers + layer_in_stage;
    } else {
        if pass_idx != 0 || layer_in_stage >= cfg.postlude_layers { return -1; }
        return cfg.prelude_layers + cfg.loop_layers + layer_in_stage;
    }
}

// Physical layer index owning a logical KV slot (inverse of inkling_logical_slot, matching inkling-metal.c:1294).
inkling_physical_layer :: (cfg:InklingLoopConfig, logical_slot:int)->int #export_c #effects(all) {
    n_phys := cfg.prelude_layers + cfg.loop_layers + cfg.postlude_layers;
    total := cfg.prelude_layers + cfg.loop_layers * cfg.loop_count + cfg.postlude_layers;
    if logical_slot < 0 || logical_slot >= total || cfg.loop_layers < 1 { return -1; }
    if logical_slot < n_phys { return logical_slot; }
    return cfg.prelude_layers + ((logical_slot - n_phys) % cfg.loop_layers);
}

inkling_kv_slot :: (cfg:InklingLoopConfig, stage:InklingLoopStage, layer_in_stage:int, pass_idx:int, pos:int)->int #export_c #effects(all) {
    if pos < 0 || pos >= cfg.max_seq || layer_in_stage < 0 { return -1; }
    step := 0;
    if stage == InklingLoopStage.prelude {
        if pass_idx != 0 || layer_in_stage >= cfg.prelude_layers { return -1; }
        step = layer_in_stage;
    } else if stage == InklingLoopStage.loop_body {
        if pass_idx < 0 || pass_idx >= cfg.loop_count || layer_in_stage >= cfg.loop_layers { return -1; }
        step = cfg.prelude_layers + pass_idx * cfg.loop_layers + layer_in_stage;
    } else {
        if pass_idx != 0 || layer_in_stage >= cfg.postlude_layers { return -1; }
        step = cfg.prelude_layers + cfg.loop_count * cfg.loop_layers + layer_in_stage;
    }
    return step * cfg.max_seq + pos;
}
```

## Production Metal 4 recurrence dispatch

In [`psionic/platform/inkling-metal.c`](../platform/inkling-metal.c), `ink_open_ex`
allocates one KV/shortconv slot per logical step (`n_slots = n_phys + (K - 1) * (loop_end - loop_start)`),
snapshots the prelude residual bank once, and on each pass `pass > 0` precomputes
all 12 looped layers' cross-pass messages concurrently before dispatching physical
layers `ip ∈ [start, end)` into their pass-specific logical slot `il`:

```text
    /* 3. Looped core passes [0 .. loop_count) over physical layers [start .. end) */
    for (int pass = 0; pass < hp->loop_count; pass++) {
        int bank_n_blocks = 0;
        if (pass > 0) {
            int src_off = prelude_n_blocks + 1;
            bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                int64_t bn = (int64_t)bank_n_blocks * d;
                CALL(s, batch, K_COPY, (bn + INK_NT - 1) / INK_NT, a_int(bn),
                     a_f32v(s->blocks, (int64_t)src_off * d, bn),
                     a_f32v(s->bank_blk, 0, bn));
            }
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->bank_out, 0, d));
            pm_barrier(batch);
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->prelude, 0, d), a_f32v(s->cur, 0, d));
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                int64_t pn = (int64_t)prelude_n_blocks * d;
                CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
                     a_f32v(s->prelude_blk, 0, pn),
                     a_f32v(s->blocks, 0, pn));
            }
            /* Sprint 32: Precompute ALL 12 looped layers' messages concurrently in this wave (48 TGs, 0 extra barriers)! */
            if ((end - start) <= s->prefill_b) {
                for (int ip = start; ip < end; ip++) {
                    const InkLayerTensors *ML = &s->gguf.layers[ip];
                    CALL(s, batch, K_MIX, 4, a_int(d), a_int(bank_n_blocks),
                         a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * d),
                         a_f32v(s->bank_out, 0, d),
                         a_tensor_f32(s, &ML->loop_msg_query),
                         a_f32v(s->messages, (int64_t)(ip - start) * 4 * d, 4 * d));
                }
            }
            pm_barrier(batch);
        }
        for (int ip = start; ip < end; ip++) {
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (pass == 0) {
                if (dispatch_layer(s, bp, il, ip, &n_blocks, 0, -1, tau) != 0) return 1;
            } else {
                if (dispatch_layer(s, bp, il, ip, &n_blocks, 1, bank_n_blocks, tau) != 0) return 1;
            }
            batch = *bp;
        }
    }
```

## Verified prefill == step-by-step decode equivalence

[`psionic/examples/looped-moe-transformer.coax`](../examples/looped-moe-transformer.coax)
runs genuine layer-major sequence prefill (`inkling_moe_prefill`) against
token-by-token autoregressive decode (`inkling_moe_decode_token`) across `K = 1`,
`K = 3`, and `K = 5` passes (matching an independent Python `math` reference to
`< 1e-5` across all tokens and dims), with zero surviving mutants on
`psionic/lib/inkling-loop.coax` (`408/408` non-equivalent mutants killed).

<!-- source: psionic/examples/looped-moe-transformer.coax example -->
```jai
main :: () {
    // 1. Production Inkling 2 Mini schedule (24 phys layers: 6 prelude + 12 loop x K + 6 coda).
    ink3 := InklingLoopConfig.{prelude_layers=6, loop_layers=12, postlude_layers=6, loop_count=3, dim=2304, experts=128, top_k=8, max_seq=32768};
    ink1 := ink3; ink1.loop_count = 1;
    ink5 := ink3; ink5.loop_count = 5;
    p1 := inkling_loop_plan(ink1);
    p3 := inkling_loop_plan(ink3);
    p5 := inkling_loop_plan(ink5);
    #assert p1.status == InklingLoopStatus.ok && p3.status == InklingLoopStatus.ok && p5.status == InklingLoopStatus.ok;
    print("Inkling 2 Mini schedule: K=1 -> % slots, K=3 -> % slots, K=5 -> % slots\n", p1.total_steps, p3.total_steps, p5.total_steps);

    // 2. Full-math Looped MoE (RMSNorm + causal Q/K/V attention + cross-pass gate + top-2 SiLU MoE).
    wq: [64]f32; wk: [64]f32; wv: [64]f32; wo: [64]f32;
    wr: [64]f32; wup: [384]f32; wdn: [384]f32; wlg: [8]f32;
    il_test_fill(wq[..], 11, 0.35f32);
    il_test_fill(wk[..], 23, 0.35f32);
    il_test_fill(wv[..], 37, 0.35f32);
    il_test_fill(wo[..], 53, 0.30f32);
    il_test_fill(wr[..], 71, 0.50f32);
    il_test_fill(wup[..], 89, 0.35f32);
    il_test_fill(wdn[..], 101, 0.30f32);
    il_test_fill(wlg[..], 131, 0.60f32);
    w := InklingMoEWeights.{hidden=6, wq=wq[..], wk=wk[..], wv=wv[..], wo=wo[..], w_router=wr[..], w_up=wup[..], w_down=wdn[..], w_loop_gate=wlg[..]};
    inputs := [12]f32.[
        1.0f32, -0.5f32, 0.25f32, 0.75f32,
        -0.4f32, 1.1f32, -0.3f32, 0.6f32,
        0.8f32, 0.2f32, -0.9f32, 0.5f32
    ];

    for ki: 0..<3 {
        k := ifx ki == 0 then 1 else ifx ki == 1 then 3 else 5;
        cfg := InklingLoopConfig.{prelude_layers=1, loop_layers=2, postlude_layers=1, loop_count=k, dim=4, experts=4, top_k=2, max_seq=4};
        kp: [512]f32; vp: [512]f32; kd: [512]f32; vd: [512]f32;
        out_pre: [12]f32; out_dec: [12]f32;
        #assert inkling_moe_prefill(cfg, w, 3, inputs[..], kp[..], vp[..], out_pre[..]) == InklingLoopStatus.ok;
        for pos: 0..<3 {
            #assert inkling_moe_decode_token(cfg, w, pos, inputs[pos*4..<(pos+1)*4], kd[..], vd[..], out_dec[pos*4..<(pos+1)*4]) == InklingLoopStatus.ok;
        }
        for i: 0..<12 {
            d := out_pre[i] - out_dec[i];
            if d < 0.0f32 { d = -d; }
            #assert d < 0.000001f32;
        }
        // Print token-2 hidden state scaled to 1e-4 integers for deterministic cross-checking against Python reference.
        h0 := cast(int)(out_pre[8] * 10000.0f32);
        h1 := cast(int)(out_pre[9] * 10000.0f32);
        h2 := cast(int)(out_pre[10] * 10000.0f32);
        h3 := cast(int)(out_pre[11] * 10000.0f32);
        print("K=% prefill==decode exact; tok2_x1e4=%,%,%,%\n", k, h0, h1, h2, h3);
    }
}
```

### Live 17.6 GB Metal 4 prefill-vs-decode verification

On Apple M5 Max against `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf`
via `psionic-inkling-cli --verify-prefill-decode` (29 prompt tokens, 200,058
unpadded logits):

| Loop count `K` | Logical slots | Top-1 prefill (`<\|message_model\|>`) | Top-1 decode | Top-1 diff | Logit cosine similarity |
| --- | --- | --- | --- | --- | --- |
| `K = 1` | `24` | `200001` (`24.0184`) | `200001` (`24.0383`) | `0.01985` | `0.999996` |
| `K = 3` (Inkling 2 Mini) | `48` | `200001` (`26.5479`) | `200001` (`26.5482`) | `0.00038` | `0.999999` |
| `K = 5` | `72` | `200001` (`25.3720`) | `200001` (`25.2193`) | `0.15276` | `0.999949` |
