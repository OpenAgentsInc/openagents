# 268. Execute Inkling 2 Mini on CPU

Paralleling [chapter 64](ch064-execute-gpt-oss-20b.md), this chapter
assembles the NVFP4/MXFP8 weight loader from
[chapter 266](ch266-original-nvfp4-and-mxfp8-weights.md) and the 3-pass looped
MoE runner from [chapter 267](ch267-a-looped-moe-transformer.md) in
[`psionic/lib/inkling-cpu.coax`](../lib/inkling-cpu.coax), demonstrated by
[`psionic/examples/inkling-cpu.coax`](../examples/inkling-cpu.coax).

CPU execution is not the interactive target—it is the bit-auditable reference
that Metal kernels in [chapter 269](ch269-inkling-2-mini-on-metal.md) must
match.

```shell
$ coax run psionic/examples/inkling-cpu.coax
Inkling 2 Mini checkpoint admitted: 24 layers (6 prelude + 12x3 loop + 6 postlude)
chat template formatted: 109 bytes
CPU greedy next token: 1
```

## Checkpoint admission and chat template

`inkling_cpu_admit` validates layer counts, shard byte bounds, and loop count
before execution, rejecting truncated shards and invalid `config.json` loop
counts. `inkling_format_turn` and `inkling_format_assistant_prefix` format
system, user, assistant, tool, and `<think>` turns with Inkling 2 Mini's
special tokens.

<!-- source: psionic/lib/inkling-cpu.coax types -->
```jai
InklingCpuStatus :: enum u8 {ok;shape;truncated;bad_loop;capacity;unknown_role;}
InklingRole :: enum u8 {system;user;assistant;tool;}
InklingCheckpointSpec :: struct {
    total_layers:int;
    prelude_layers:int;
    loop_layers:int;
    postlude_layers:int;
    loop_count:int;
    dim:int;
    vocab_size:int;
    experts:int;
    top_k:int;
    shard_bytes:int;
    min_shard_bytes:int;
}
InklingDecodeStep :: struct {
    status:InklingCpuStatus;
    token_id:int;
    top_logit:f32;
}
```

## Deterministic CPU prefill and greedy decode

<!-- source: psionic/examples/inkling-cpu.coax example -->
```jai
main :: () {
    spec := InklingCheckpointSpec.{
        total_layers=24, prelude_layers=6, loop_layers=12, postlude_layers=6,
        loop_count=3, dim=4, vocab_size=4, experts=4, top_k=2,
        shard_bytes=20480, min_shard_bytes=20000
    };
    #assert inkling_cpu_admit(spec) == InklingCpuStatus.ok;
    print("Inkling 2 Mini checkpoint admitted: % layers (% prelude + %x% loop + % postlude)\n",
        spec.total_layers, spec.prelude_layers, spec.loop_layers, spec.loop_count, spec.postlude_layers);

    buf: [128]u8;
    n := inkling_format_turn(InklingRole.system, "You are Inkling.", buf[..], 0);
    n = inkling_format_turn(InklingRole.user, "Hello", buf[..], n);
    n = inkling_format_assistant_prefix(true, buf[..], n);
    #assert n > 0;
    print("chat template formatted: % bytes\n", n);

    cfg := InklingLoopConfig.{prelude_layers=1,loop_layers=1,postlude_layers=1,loop_count=3,dim=4,experts=4,top_k=2,max_seq=8};
    embed := [16]f32.[
        1.0f32, 0.2f32, -0.1f32, 0.4f32,
        -0.3f32, 1.1f32, 0.5f32, -0.2f32,
        0.4f32, -0.5f32, 1.2f32, 0.3f32,
        -0.2f32, 0.3f32, -0.4f32, 1.3f32
    ];
    router_w := [16]f32.[
        0.5f32, -0.2f32, 0.1f32, 0.3f32,
        -0.4f32, 0.6f32, 0.2f32, -0.1f32,
        0.2f32, 0.1f32, -0.5f32, 0.4f32,
        -0.1f32, -0.3f32, 0.7f32, 0.2f32
    ];
    expert_scale := [4]f32.[0.25f32, -0.20f32, 0.30f32, -0.15f32];
    prompt := [2]int.[0, 1];
    kv: [256]f32;
    step := inkling_cpu_prefill_and_next(cfg, 4, prompt[..], embed[..], router_w[..], expert_scale[..], kv[..]);
    #assert step.status == InklingCpuStatus.ok;
    print("CPU greedy next token: %\n", step.token_id);
}
```
