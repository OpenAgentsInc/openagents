# 267. A looped MoE transformer

A standard decoder runs each layer once per token. Inkling 2 Mini runs its
middle layers **three times per token**, reusing the same weights across passes
so the model spends more serial computation per generated token without growing
its parameter footprint.

Building on [chapter 61](ch061-a-few-experts-then-a-router.md) (MoE routing) and
[chapter 86](ch086-sparse-attention-and-shared-layers.md) (cross-layer state
sharing), [`psionic/lib/inkling-loop.coax`](../lib/inkling-loop.coax) implements
the three-stage looped MoE schedule, per-pass KV-cache indexing, and variable
loop count (`K = 3` for Inkling 2 Mini, configurable `K >= 1` for
`TML-d24-varloop-research`), exercised by
[`psionic/examples/looped-moe-transformer.coax`](../examples/looped-moe-transformer.coax).

```shell
$ coax run psionic/examples/looped-moe-transformer.coax
Inkling looped schedule (K=3): 5 logical steps, 20 KV slots
prefill == step-by-step decode verified across 3 passes
```

## Looped schedule and per-pass KV slots

Every `(stage, layer, pass, pos)` tuple maps to a distinct KV-cache slot so
pass 2 and pass 3 never overwrite pass 1's attention state. Invalid loop counts
(`K = 0`), out-of-bounds pass indices, and out-of-bounds positions are rejected
before execution.

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
```

## Verified prefill and decode equivalence

`inkling_loop_prefill` and token-by-token `inkling_loop_decode_token` execute
the same prelude, `K`-pass recurrent MoE block (re-evaluating top-k routing on
every pass), and postlude layers, producing identical hidden states across
`K = 1`, `K = 3`, and `K = 5`.

<!-- source: psionic/examples/looped-moe-transformer.coax example -->
```jai
main :: () {
    cfg := InklingLoopConfig.{prelude_layers=1,loop_layers=1,postlude_layers=1,loop_count=3,dim=4,experts=4,top_k=2,max_seq=4};
    plan := inkling_loop_plan(cfg);
    #assert plan.status == InklingLoopStatus.ok;
    print("Inkling looped schedule (K=%): % logical steps, % KV slots\n", cfg.loop_count, plan.total_steps, plan.kv_slots);

    router_w := [16]f32.[
        0.5f32, -0.2f32, 0.1f32, 0.3f32,
        -0.4f32, 0.6f32, 0.2f32, -0.1f32,
        0.2f32, 0.1f32, -0.5f32, 0.4f32,
        -0.1f32, -0.3f32, 0.7f32, 0.2f32
    ];
    expert_scale := [4]f32.[0.25f32, -0.20f32, 0.30f32, -0.15f32];
    inputs := [8]f32.[1.0f32, 0.5f32, -0.25f32, 0.75f32, -0.5f32, 1.25f32, 0.5f32, -0.25f32];
    kv_pre: [128]f32; kv_dec: [128]f32; out_pre: [8]f32; out_dec: [8]f32;

    #assert inkling_loop_prefill(cfg, 2, inputs[..], router_w[..], expert_scale[..], kv_pre[..], out_pre[..]) == InklingLoopStatus.ok;
    #assert inkling_loop_decode_token(cfg, 0, inputs[0..<4], router_w[..], expert_scale[..], kv_dec[..], out_dec[0..<4]) == InklingLoopStatus.ok;
    #assert inkling_loop_decode_token(cfg, 1, inputs[4..<8], router_w[..], expert_scale[..], kv_dec[..], out_dec[4..<8]) == InklingLoopStatus.ok;

    for i: 0..<8 {
        d := out_pre[i] - out_dec[i];
        if d < 0.0f32 { d = -d; }
        #assert d < 0.000001f32;
    }
    print("prefill == step-by-step decode verified across 3 passes\n");
}
```
