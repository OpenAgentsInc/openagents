# 46. Execute gpt-oss-120b

[Previous: gpt-oss-20b](ch45-execute-gpt-oss-20b.md) · [Contents](SUMMARY.md) ·
[Next: speculative decoding](ch47-let-the-small-model-guess.md)

A larger model should exercise the same implementation. gpt-oss-120b changes
24 layers to 36 and 32 experts to 128. It still selects four experts and shares
the 2,880 width/intermediate, 64/8 heads, 64-wide head, vocabulary and tokenizer.
The `gpt-oss-cpu-wide-v1` engine from chapter 45 executes it without a second
forward implementation.

## Identity changes before dimensions

Revision `b5c939de8f754692c1647ca79fbf85e8c1e70f8a` has seven original
Safetensors shards, alongside config, dtype metadata and a shard index.
[The manifest](../fixtures/gptoss/original/pins.json) records every byte length
and SHA-256. The complete profile binds 543 tensors. An identically shaped
checkpoint, converted layout or missing shard does not inherit this evidence.
Every required tensor view is authenticated and mapped before publication.

The fixed target table comes from the retained original headers, which are
checked for drift by [the descriptor tool](../tests/gptoss_pins.py). The runtime
checks all extents and the exact slot shapes before borrowing the payload.
Sidecars and the index participate in identity admission even though the
versioned table determines the fixed runtime offsets.

## Separate parameters, history and scratch

Mapped weight bytes remain original BF16/U8. Only selected experts are
numerically read during a forward pass. This does not make unselected banks
free: whole-file authentication reads every shard, and their mappings remain
part of the process's memory profile.

Each layer needs `2 * kv_heads * 64` floats per cached position. Even layers
reserve 128 positions, odd layers reserve the chosen full-attention capacity.
For capacity C, total cache floats are:

```jai
20b:  12 * 1024 * (128 + C)
120b: 18 * 1024 * (128 + C)
```

Scratch contains a current residual, projections, attention temporaries, the
staged KV slots for all layers, traces/routes/weights and the complete head.
It is caller-owned and reusable across token steps. The model's explicit MAC
budget counts attention against the visible prefix. A short budget refuses
before a cache write. It does not impose a deadline or reserve physical RAM.

<!-- source: psionic/lib/gptoss/layout.coax plan -->
```jai
g_plan :: (s:GShape)->GPlan #export_c {
    if !g_shape(s) {return GPlan.{status=GStatus.shape};}if !g_mac_fits(s.vocab,s.width) {return GPlan.{status=GStatus.budget};}
    d:=s.width;q:=s.heads*64;k:=s.kv_heads*64;h:=s.hidden;l:=s.layers;r:=s.vocab+2*h+q+2*k+d+s.experts;
    lengths:=[17]int.[d,d,q+2*k,q,d,s.experts,2*h,h,d,d,3*q+k,2*k*l,2*d*l,l*s.selected,s.vocab,r,r];
    p:=GPlan.{cache=g_cache_base(s,l),scores=s.capacity,sums=d,routes=l*s.selected};
    for i:0..<17 {p.begin[i+1]=p.begin[i]+lengths[i];}p.floats=p.begin[17];return p;
}
```

At capacity 256, cache storage is 18 MiB for 20b and 27 MiB for 120b. A full
131,072-token cache is much larger even with window-bounded alternate layers;
choose a capacity before allocating. Full-context admission does not establish
that every machine can allocate or execute it. Snapshotting for speculation
adds another complete copy of the chosen state.

[The residency example](../examples/gptoss-residency.coax) only plans storage:

```shell
coax/bin/coax run psionic/examples/gptoss-residency.coax
```

<!-- source: psionic/examples/gptoss-residency.coax example -->
```jai
main :: () {
    a:=g_original_shape(0,256);b:=g_original_shape(1,256);small:=g_plan(a);large:=g_plan(b);
    #assert small.status==GStatus.ok && large.status==GStatus.ok;
    print("20b: % layers, % experts, % cache bytes at capacity 256\n",a.layers,a.experts,small.cache*4);
    print("120b: % layers, % experts, % cache bytes at capacity 256\n",b.layers,b.experts,large.cache*4);
    print("original parameter tensors: % / %\n",G20_SPECS.count,G120_SPECS.count);
    print("same engine: BF16 dense rows and selected original MXFP4 experts\n");
}
```

Expected output:

```jai
20b: 24 layers, 32 experts, 18874368 cache bytes at capacity 256
120b: 36 layers, 128 experts, 28311552 cache bytes at capacity 256
original parameter tensors: 363 / 543
same engine: BF16 dense rows and selected original MXFP4 experts
```

## Measure the machine that actually runs

This chapter ran the original seven shards on the 128 GiB Apple host. The
weight payload is about 65.249 GB. It fits on this machine, so this profile
keeps all mappings resident; it does not claim an expert offload scheduler.
A smaller host needs a separately measured residency policy before execution.
The chapter 45 runner uses the same authentication, allocation and native
forward functions for both identities.

The retained original run authenticated in 284.846 s, executed two token steps
in 6.451 and 6.309 s, and reached peak RSS 65.293 GB. Its independent dense
reference matched all retained layer residuals, routes, coefficients and
201,088 logits exactly after binary32 storage. The threshold and arithmetic
profile were the same as chapter 45. This is independent traversal/byte-decoding
evidence, not evidence from running upstream Torch.

A separate seeded Harmony turn used seed 45001, temperature 0.7, top-k 40 and
top-p 0.9. It emitted 58 IDs through return and its final message was
`Hi there! 👋 How can I help you today?`. Prefill for its 24-token request took
127.710 s; prefill plus continuation took 446.093 s. Peak RSS was 65.302 GB.
These are individual CPU measurements; file warmth and simultaneous machine
activity differ between runs. No speed comparison with another runtime is
implied.

Run the default offline gates or opt in to the full original qualification:

```shell
psionic/tests/check_gptoss.sh
psionic/tests/check_gptoss_original.sh 120b /path/to/numpy/python3
```

The [verification record](../../docs/verification/psionic-ch46-2026-10-29.md)
retains the checkpoint identity, raw measurement logs and qualification summary.
Next we use the shared tokenizer and both resident models to check draft tokens.
