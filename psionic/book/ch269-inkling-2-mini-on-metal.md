# 269. Inkling 2 Mini on Metal

Building on [chapter 47](ch047-elephant-on-metal.md) (Metal shaders from
Coax) and [chapter 72](ch072-gpt-oss-on-metal.md) (gpt-oss on Metal), this
chapter implements Apple Silicon ≥32 GB unified-memory residency planning and
fused register-dequantizing NVFP4/MXFP8 matvec kernels in
[`psionic/lib/inkling-metal.coax`](../lib/inkling-metal.coax), demonstrated by
[`psionic/examples/inkling-metal.coax`](../examples/inkling-metal.coax).

Inkling 2 Mini's ~20 GB weight footprint leaves roughly 10–12 GB on a 32 GB Mac
for macOS, the OpenAgents Desktop shell, KV cache across 3 loop passes, and
activation scratchpads. Explicit memory budgeting is therefore part of the
kernel contract, not an afterthought.

```shell
$ coax run psionic/examples/inkling-metal.coax
32GB Mac @ 4k ctx: 768 MB KV, 26368 MB total, 6400 MB headroom (max ctx 38229)
16GB Mac refused: OOM before allocation
fused NVFP4 matvec matches CPU reference: y[0] = -0.5
```

## Unified-memory residency planner

`inkling_metal_plan` computes exact per-pass KV-cache bytes, total working set,
and maximum admitted context length before any buffer allocation, refusing
16 GB devices with `oom` and oversized contexts with `context_limit`.

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
InklingMetalPlan :: struct {
    status:InklingMetalStatus;
    kv_cache_mb:int;
    total_mb:int;
    headroom_mb:int;
    max_context_len:int;
}
```

## Fused NVFP4 and MXFP8 register matvec

`inkling_metal_nvfp4_matvec` and `inkling_metal_mxfp8_matvec` dequantize
16-element NVFP4 and 32-element MXFP8 micro-blocks on the fly in registers
during GEMV without materializing FP16/FP32 weight buffers, matching the CPU
reference decoders in [chapter 266](ch266-original-nvfp4-and-mxfp8-weights.md)
and [chapter 268](ch268-execute-inkling-2-mini-on-cpu.md).

<!-- source: psionic/examples/inkling-metal.coax example -->
```jai
main :: () {
    s4k := InklingMetalBudgetSpec.{
        device_ram_mb=32768, os_reserve_mb=4096, weights_mb=20480,
        kv_bytes_per_token_pass=65536, loop_count=3, context_len=4096, scratchpad_mb=1024
    };
    p4k := inkling_metal_plan(s4k);
    #assert p4k.status == InklingMetalStatus.ok;
    print("32GB Mac @ 4k ctx: % MB KV, % MB total, % MB headroom (max ctx %)\n",
        p4k.kv_cache_mb, p4k.total_mb, p4k.headroom_mb, p4k.max_context_len);

    s16g := s4k; s16g.device_ram_mb = 16384;
    #assert inkling_metal_plan(s16g).status == InklingMetalStatus.oom;
    print("16GB Mac refused: OOM before allocation\n");

    packed := [8]u8.[0x20, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    scales := [1]u8.[0x38];
    x := [16]f32.[
        1.0f32, 2.0f32, 0.5f32, -1.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32,
        0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32
    ];
    y: [1]f32;
    #assert inkling_metal_nvfp4_matvec(packed[..], scales[..], 0.5f32, x[..], 1, 16, y[..]) == InklingMetalStatus.ok;
    print("fused NVFP4 matvec matches CPU reference: y[0] = %\n", y[0]);
}
```
