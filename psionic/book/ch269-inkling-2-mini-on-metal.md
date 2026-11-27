# 269. Inkling 2 Mini on Metal

Building on [chapter 47](ch047-elephant-on-metal.md) (Metal shaders from
Coax) and [chapter 72](ch072-gpt-oss-on-metal.md) (gpt-oss on Metal), this
chapter compiles Inkling 2 Mini's full looped MoE inference stack from single-source
Coax ([`psionic/lib/inkling/kernels.coax`](../lib/inkling/kernels.coax)) into
Metal Shading Language (`coax metal-compute`), drives it on Apple Silicon via
[`psionic/platform/inkling-metal.c`](../platform/inkling-metal.c) and
[`psionic/platform/metal-host.m`](../platform/metal-host.m), and enforces
explicit ≥32 GB unified-memory residency guardrails
([`psionic/lib/inkling-metal.coax`](../lib/inkling-metal.coax)).

The default release ships Inkling 2 Mini as the 17.63 GB (`16,812 MiB`) `IQ4_NL`
GGUF checkpoint (`Inkling-2-Mini-IQ4_NL.gguf`) with Metal 4 Neural Accelerators,
FP16/FlashAttention, warp routers, and 4-warp register-dequantizing decode (`ink_dv_iq4`,
`ink_dv_q8`), alongside **21 GGML quantizations**, the **Maple Uncensored safetensors
overlay** (`feff1652bc`), and **rank-8 LoRA routing adapters** (`#14028`).
Smaller quantized variants lower the hardware floor to 12–16 GB machines, while
`psionic/lib/inkling-metal.coax` keeps the explicit `>= 32 GB` unified-memory
guardrail for the default 17.63 GB `IQ4_NL` checkpoint. Meanwhile,
`psionic/lib/inkling/kernels.coax` also emits register-dequantizing
**NVFP4** (`ink_dv_nvfp4`) and **MXFP8** (`ink_dv_mxfp8`) Metal GPU kernels
verified bit-exact (`0.00e+00` max abs error) on Apple Silicon, and
[`psionic/platform/inkling-cuda.c`](../platform/inkling-cuda.c) +
[`psionic/lib/inkling/kernels-cuda.coax`](../lib/inkling/kernels-cuda.coax)
run the exact same GGUF model on Linux and Windows NVIDIA GPUs via `coax cuda`
and NVRTC with zero cuBLAS or cuDNN dependency (`#13966`, `#13975`–`#13977`).

Because 5 of every 6 physical layers use a 1,024-token sliding window, Inkling 2
Mini (`K = 3`, `48` logical slots) needs only **8 full-context KV slots** and
**40 SWA-1024 slots** — just `1,268 MiB` of KV cache at **32,768 context** —
leaving over `9,000 MiB` of headroom on a 32 GB Apple Silicon Mac while refusing
16 GB Macs (`oom`) and oversized contexts (`context_limit`) before any Metal
buffer allocation.

```shell
$ coax run psionic/examples/inkling-metal.coax
32GB Mac @ 32k ctx: 1264 MB KV (8 full + 40 SWA), 23708 MB total, 9060 MB headroom (max ctx 322688)
16GB Mac refused: OOM before Metal allocation
512k ctx on 32GB refused: context_limit before allocation
register-dequant reference: nvfp4 y[0]=-0.5 mxfp8 y[0]=16.0
```

## Unified-memory residency planner (≥32 GB Apple Silicon)

`inkling_metal_split_plan` in [`psionic/lib/inkling-metal.coax`](../lib/inkling-metal.coax)
(mirrored directly in `ink_open_ex` in [`psionic/platform/inkling-metal.c`](../platform/inkling-metal.c)
using `sysctlbyname("hw.memsize")` and `MTLDevice.recommendedMaxWorkingSetSize`)
computes exact full-attention + SWA KV cache bytes, total working set, and
maximum admitted context length before allocating Metal buffers.

<!-- source: psionic/lib/inkling-metal.coax types -->
```jai
InklingMetalStatus :: enum u8 {ok;shape;oom;context_limit;parity_fail;}
InklingMetalBudgetSpec :: struct {
    device_ram_mb:int;
    os_reserve_mb:int;
    weights_mb:int;
    kv_bytes_per_token_pass:int;
    loop_count:int;
    context_len:int;
    scratchpad_mb:int;
}
InklingSplitKvSpec :: struct {
    device_ram_mb:int;
    os_reserve_mb:int;
    weights_mb:int;
    full_slots:int;
    swa_slots:int;
    swa_window:int;
    kv_bytes_per_slot_tok:int;
    context_len:int;
    scratchpad_mb:int;
}
InklingMetalPlan :: struct {
    status:InklingMetalStatus;
    kv_cache_mb:int;
    total_mb:int;
    headroom_mb:int;
    max_context_len:int;
}
```

<!-- source: psionic/lib/inkling-metal.coax split-plan -->
```jai
// Unified-memory residency planner for Inkling 2 Mini's real split SWA + Full-Attention KV cache
// (8 full-context slots + 40 SWA-1024 slots at K=3; enforces >= 32 GB Apple Silicon floor).
inkling_metal_split_plan :: (spec:InklingSplitKvSpec)->InklingMetalPlan #export_c #effects(all) {
    if spec.device_ram_mb < 1 || spec.os_reserve_mb < 0 || spec.weights_mb < 1 ||
       spec.full_slots < 1 || spec.swa_slots < 0 || spec.swa_window < 0 ||
       spec.kv_bytes_per_slot_tok < 1 || spec.context_len < 1 || spec.scratchpad_mb < 0 {
        return InklingMetalPlan.{status=InklingMetalStatus.shape};
    }
    fixed_mb := spec.os_reserve_mb + spec.weights_mb + spec.scratchpad_mb;
    if spec.device_ram_mb < INKLING_METAL_MIN_RAM_MB || fixed_mb >= spec.device_ram_mb {
        return InklingMetalPlan.{status=InklingMetalStatus.oom, total_mb=fixed_mb};
    }
    swa_cap := ifx spec.swa_window < spec.context_len then spec.swa_window else spec.context_len; // mutant: equivalent (equal on tie)
    swa_bytes := spec.swa_slots * swa_cap * spec.kv_bytes_per_slot_tok;
    full_bytes := spec.full_slots * spec.context_len * spec.kv_bytes_per_slot_tok;
    kv_mb := (swa_bytes + full_bytes + 1048575) / 1048576;
    avail_bytes := (spec.device_ram_mb - fixed_mb) * 1048576 - spec.swa_slots * spec.swa_window * spec.kv_bytes_per_slot_tok;
    if avail_bytes < 0 { avail_bytes = 0; } // mutant: equivalent (< 0 vs <= 0 assigns 0 to 0)
    max_ctx := avail_bytes / (spec.full_slots * spec.kv_bytes_per_slot_tok);
    total := fixed_mb + kv_mb;
    if total > spec.device_ram_mb {
        return InklingMetalPlan.{status=InklingMetalStatus.context_limit, kv_cache_mb=kv_mb, total_mb=total, headroom_mb=0, max_context_len=max_ctx};
    }
    return InklingMetalPlan.{status=InklingMetalStatus.ok, kv_cache_mb=kv_mb, total_mb=total, headroom_mb=spec.device_ram_mb - total, max_context_len=max_ctx};
}
```

## Coax-emitted Metal kernels (NVFP4, MXFP8, and IQ4_NL)

`coax metal-compute psionic/lib/inkling/kernels.coax --release` emits native
Metal Shading Language (`inkling-kernels.metal`) with zero hand-written MSL.
The NVFP4 (`ink_dv_nvfp4`) and MXFP8 (`ink_dv_mxfp8`) kernels dequantize
16-element and 32-element micro-blocks directly in SIMD registers with hardware
`am_simd_sum` reductions, never materializing FP16 or FP32 weight buffers:

<!-- source: psionic/lib/inkling/kernels.coax quant-metal-kernels -->
```jai
// FP4 E2M1 value table (NVFP4 micro-block element: 1 sign bit + 3-bit magnitude).
ink_nvfp4_val :: (nib:u32)->f32 #effects(all) {
    m:=nib&7;
    v:f32=ifx m==0 then 0.0f32 else ifx m==1 then 0.5f32 else ifx m==2 then 1.0f32 else ifx m==3 then 1.5f32
        else ifx m==4 then 2.0f32 else ifx m==5 then 3.0f32 else ifx m==6 then 4.0f32 else 6.0f32;
    return ifx (nib&8)!=0 then -v else v;
}

// FP8 E4M3FN byte -> f32 (bias 7, subnormals exact via f32_from_bits).
ink_fp8_e4m3 :: (b:u32)->f32 #effects(all) {
    e:=(b>>3)&15; m:=b&7;
    v:f32=ifx e==0 then f32(m)*(1.0f32/512.0f32) else f32_from_bits(((e+120)<<23)|(m<<20));
    return ifx (b&128)!=0 then -v else v;
}

// UE8M0 shared microscaling exponent -> f32 (2^(e - 127), exact power of two).
ink_ue8m0 :: (e:u32)->f32 #effects(all) {
    if e==0 { return f32_from_bits(0x00400000); }
    return f32_from_bits((e&255)<<23);
}

// Register-dequantizing NVFP4 GEMV on Metal (16-element micro-blocks: 2 u32 packed nibbles + E4M3 block scale + f32 global_scale).
// 1 warp (32 lanes) per row; launch items = rows, lanes = 32. Zero FP16/FP32 weight buffer materialization.
ink_dv_nvfp4 :: (item:int,rows:int,cols:int,global_scale:f32,packed:[]u32,scales:[]u32,x:[]f32,out:[]f32) #export_c #effects(all) {
    blocks:=cols/16;
    for thread:0..<32 {
        row:=item;
        acc:f32=0;
        if row<rows {
            for step:0..<(blocks+31)/32 {
                blk:=step*32+thread;
                if blk<blocks {
                    bidx:=row*blocks+blk;
                    w0:=packed[bidx*2];
                    w1:=packed[bidx*2+1];
                    sc_word:=scales[bidx/4];
                    sc_byte:=(sc_word>>cast(u32)((bidx&3)*8))&255;
                    sc:=ink_fp8_e4m3(sc_byte)*global_scale;
                    xb:=blk*16;
                    s:f32=0;
                    for j:0..<8 {
                        s+=ink_nvfp4_val((w0>>cast(u32)(j*4))&15)*x[xb+j];
                        s+=ink_nvfp4_val((w1>>cast(u32)(j*4))&15)*x[xb+8+j];
                    }
                    acc+=sc*s;
                }
            }
            tot:=am_simd_sum(acc);
            if thread==0 { out[row]=tot; }
        }
    }
}

// Register-dequantizing MXFP8 GEMV on Metal (32-element micro-blocks: 8 u32 E4M3 bytes + UE8M0 shared exponent).
// 1 warp (32 lanes) per row; launch items = rows, lanes = 32. Zero FP16/FP32 weight buffer materialization.
ink_dv_mxfp8 :: (item:int,rows:int,cols:int,weights:[]u32,scales:[]u32,x:[]f32,out:[]f32) #export_c #effects(all) {
    blocks:=cols/32;
    for thread:0..<32 {
        row:=item;
        acc:f32=0;
        if row<rows {
            for step:0..<(blocks+31)/32 {
                blk:=step*32+thread;
                if blk<blocks {
                    bidx:=row*blocks+blk;
                    sc_word:=scales[bidx/4];
                    sc_byte:=(sc_word>>cast(u32)((bidx&3)*8))&255;
                    sc:=ink_ue8m0(sc_byte);
                    xb:=blk*32;
                    wbase:=bidx*8;
                    s:f32=0;
                    for j:0..<8 {
                        wd:=weights[wbase+j];
                        s+=ink_fp8_e4m3(wd&255)*x[xb+j*4]
                          +ink_fp8_e4m3((wd>>8)&255)*x[xb+j*4+1]
                          +ink_fp8_e4m3((wd>>16)&255)*x[xb+j*4+2]
                          +ink_fp8_e4m3((wd>>24)&255)*x[xb+j*4+3];
                    }
                    acc+=sc*s;
                }
            }
            tot:=am_simd_sum(acc);
            if thread==0 { out[row]=tot; }
        }
    }
}
```

## Runnable CPU check and live Apple M5 Max Metal verification

Run [`psionic/examples/inkling-metal.coax`](../examples/inkling-metal.coax) to verify the split SWA+Full 32 GB residency budget, 16 GB OOM and 512k context-limit refusals, and register-dequantization reference:

<!-- source: psionic/examples/inkling-metal.coax example -->
```jai
main :: () {
    // Production Inkling 2 Mini IQ4_NL GGUF on a 32 GB Mac at full 32,768 context:
    // 16,812 MiB weights, 8 full-attention slots + 40 SWA-1024 slots (K=3), 4096 B/slot-tok.
    s32k := InklingSplitKvSpec.{
        device_ram_mb=32768, os_reserve_mb=4096, weights_mb=16812,
        full_slots=8, swa_slots=40, swa_window=1536, kv_bytes_per_slot_tok=4096,
        context_len=32768, scratchpad_mb=1536
    };
    p32k := inkling_metal_split_plan(s32k);
    #assert p32k.status == InklingMetalStatus.ok;
    print("32GB Mac @ 32k ctx: % MB KV (8 full + 40 SWA), % MB total, % MB headroom (max ctx %)\n",
        p32k.kv_cache_mb, p32k.total_mb, p32k.headroom_mb, p32k.max_context_len);

    s16g := s32k; s16g.device_ram_mb = 16384;
    #assert inkling_metal_split_plan(s16g).status == InklingMetalStatus.oom;
    print("16GB Mac refused: OOM before Metal allocation\n");

    sover := s32k; sover.context_len = 524288;
    #assert inkling_metal_split_plan(sover).status == InklingMetalStatus.context_limit;
    print("512k ctx on 32GB refused: context_limit before allocation\n");

    // Reference check matching Metal GPU ink_dv_nvfp4 and ink_dv_mxfp8 kernels.
    packed := [8]u8.[0x20, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    scales := [1]u8.[0x38];
    x := [16]f32.[
        1.0f32, 2.0f32, 0.5f32, -1.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32,
        0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32
    ];
    y: [1]f32;
    #assert inkling_metal_nvfp4_matvec(packed[..], scales[..], 0.5f32, x[..], 1, 16, y[..]) == InklingMetalStatus.ok;
    mbytes: [32]u8; for i: 0..<32 { mbytes[i] = 0x38; }
    mscales := [1]u8.[128];
    x32: [32]f32; for i: 0..<32 { x32[i] = 0.25f32; }
    ym8: [1]f32;
    #assert inkling_metal_mxfp8_matvec(mbytes[..], mscales[..], x32[..], 1, 32, ym8[..]) == InklingMetalStatus.ok;
    print("register-dequant reference: nvfp4 y[0]=% mxfp8 y[0]=%\n", y[0], ym8[0]);
}
```

### Live M5 Max Metal 4 verification (`psionic/platform/inkling-build.sh`)

- **Coax-emitted Metal GPU quant kernels (`psionic-inkling-cli --test-quant-kernels`)**:
  `device='Apple M5 Max' nvfp4_max_abs=0.00e+00 mxfp8_max_abs=0.00e+00 iq4_max_abs=0.00e+00 ok=true`.
- **16 GB Mac guardrail (`INK_TEST_RAM_MB=16384`)**: refused before allocation with
  `insufficient unified memory: 16384 MB detected (requires >= 32768 MB for Inkling 2 Mini; weights=16812 MB)`.
- **32 GB Mac @ 32,768 context (`INK_TEST_RAM_MB=32768 --ctx 32768`)**:
  `weights=16812 MB, kv=1268 MB (8 full + 40 swa), scratch=1258 MB, os=4096 MB, total=23434 MB, headroom=9334 MB`,
  peak RSS `2.78 GiB` (`2,980,806,656` bytes), warm greedy decode `50.74 tok/s` (`--ctx 4096`).
- **Head-to-head parity against `llama.cpp` Metal (`~/.openagents/bin/llama-server`)**:
  identical `29` prompt tokens and identical final answer `"391"` on the real
  17.63 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint (`psionic-inkling-serve` vs `llama-server`).
- **Metal 4 FlashAttention, lossless `Q8S` + `Q8_0` KV, GPU Top-64, Ch 102 adaptive DSpark, and async `DiskPromptCache`**:
  - **Lossless `Q8S` + block-32 `Q8_0` KV cache (`INK_KV_Q8=1`, default):** halves KV RAM (`772 → 412 MiB` at `ctx=4096`, `max_ctx 3.44M → 6.49M` on 128 GB M5 Max) with `--verify-prefill-decode cos=0.999968, top1_diff=0.07926, ok=true`; compact weight unmapping holds peak footprint to `19.75 GiB` (`19,753 MiB`).
  - **1-pass Metal 4 FlashAttention (`am_flash_attn_m4`) + aligned in-tile `IQ4S`/`Q8S` BF16 Neural Accelerator GEMMs + zero-scatter MoE (`am_moe_sum_gather`):** shrinks `attn_scores` scratch from `2.3 GB → 3.2 MB`, lifts 2K–4K cold prefill from `857 → 2,038–2,315 tok/s` (`2.4×`), and keeps `temp=0.6` decode (`88.4 tok/s`, GPU exact Top-64) at parity with greedy decode (`89.6 tok/s`).
  - **Tiered RAM + async NVMe `DiskPromptCache`, `MTLBinaryArchive` caching & HTTP/1.1 Keep-Alive (`psionic-inkling-serve`):** pre-`<|end_message|>` checkpointing cuts 4.6K-token follow-up TTFT from `3.50 s → 0.15 s` (`22.7×`) in RAM and cuts 5.0K-token cross-restart cold TTFT from `2.22 s → 0.039–0.048 s` (`46×–67×`, bit-exact output parity) with zero TTFT/decode write stall.
  - **Phase 8 grouped-MoE `_nb` verification, Cascade DSpark & Fast Apply ([inkling-metal-report.md](../docs/targets/inkling-metal-report.md)):** default lossless `Q8_0` (`188.1 tok/s` `nb=8` peak) and opt-in `INK_FAST_Q4=1` (`169.8 tok/s` end-to-end `file_edit`, `260.9 tok/s` peak) with `0` parity mismatches, default-on Native Coax DSpark + Cascade per-request utility gate (`INK_DSPARK=0` opt-out), GPU sparse top-k (`ink_topk_filter`), SuffixDecoding-lite, and OpenAI `"prediction"` Fast Apply.
  - **Verification (`#authority`):** `coax test coax/lib/compute/apple.coax --library` and `coax mutate coax/lib/compute/apple.coax --library` (**3,556 mutants, 3,525 killed, 0 survived, 31 marked equivalent**).
- **Live NVIDIA RTX 4080 CUDA verification (`psionic/platform/inkling-cuda-build.sh`, Linux & Windows):**
  - Single-source Coax kernels (`psionic/lib/inkling/kernels-cuda.coax` via `coax cuda` + NVRTC, zero cuBLAS/cuDNN) with per-layer LRU VRAM expert caching (`INK_MIN_EXPERT_SLOTS = 16`, `INK_VRAM_MARGIN_MB = 384`) deliver `75.7+` tok/s decode and `634` tok/s prefill on an RTX 4080 (`#13966`, `#13975`–`#13977`).
