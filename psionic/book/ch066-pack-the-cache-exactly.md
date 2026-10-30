# 66. Pack the cache exactly

The V4.1 reference rounds window KV, compressed KV and index keys onto small
floating-point grids, then stores the dequantized values in BF16 tensors.
We can keep the codes and scales instead. Reading them must reconstruct the
same BF16 bits, including the rotary tail. This chapter adds that codec and
budgets its temporary work separately from persistent cache storage.

The profile is `deepseek-cache-bf16-grid-v1`. It implements the pinned
[model](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/model.py)
and [kernel](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/kernel.py)
equations with explicit F32 and BF16 rounding boundaries. An independent,
owned oracle supplies exact bits; upstream code is read, never executed.
This is a CPU codec qualification. Device captures and complete attention
execution remain separate gates in chapters 60, 67 and 68.

## Three grids

| Role | Values | Scale | Original row payload |
| --- | --- | --- | --- |
| Window KV, width 512 | FP8 E4M3 | One UE8M0 per 32 values | 512 codes + 16 scales = 528 bytes |
| Compressed KV, width 512 | FP4 E2M1 | One FP8 E4M3 per 16 values | 256 packed bytes + 32 scales = 288 bytes |
| Index key, width 128 | FP4 E2M1 | One UE8M0 per 32 values | 64 packed bytes + 4 scales = 68 bytes |

The same index grid applies to queries. The original configuration has 32
query heads of width 128. Flattening those heads keeps every 32-value group
inside its original head. Configuration identities, retained reference text
and the reviewed rules are in [cache-source.json](../fixtures/deepseek/cache-source.json).

Window scales use the F32 maximum magnitude with a floor of F32 `1e-4`.
Multiply by the F32 reciprocal of 448, round that product to F32, then use
its exponent and mantissa to round up to a power of two. Index scales use
the same exponent rule with F32 reciprocal 6 and floor `6 * 2^-126`.

Compressed scales are different: floor the maximum at `6 * 2^-9`, divide
by 6 in F32 and cast to nearest-even, saturating FP8 E4M3. The scale is a
positive finite FP8 value, including subnormals, rather than an exponent byte.
Using the expert decoder's row/32 scale layout here would change the cache.

For each value, divide by its scale in F32, clamp to the finite grid range
and round to nearest with ties to even. Packed FP4 puts even positions in
the low nibble. Window data uses one byte per value. Separate scale bytes
follow the same row-major group order as the codes.

Decoding multiplies the exact grid coefficient and scale, rounds the product
to F32 and then to BF16. The implementation preserves signed zero and BF16
subnormals. A non-finite input, NaN code/scale, zero or negative compressed
scale, or a product that would become infinite refuses. It never substitutes
a different finite value for overflow.

## Publish a whole row batch

[cache.coax](../lib/deepseek/cache.coax) exposes `dk_plan`, `dk_pack` and
`dk_unpack`. Rows range from one to 1,048,576; widths are group-aligned up
to 8,192. BF16 values cross the API as `u16` bit patterns. Codes and scales
cross as two byte views in `DkPacked`; profile and dimensions accompany them.

Packing stages all code/scale bytes before copying either public view.
Unpacking stages every BF16 word before publishing any output. A late bad
scale, code or overflow preserves the whole public result and spare tails.
Private work may change. Row groups do not depend on other rows, so packing
rows separately or in chunks produces identical codes and scales.

The caller owns valid typed allocations and extents, matching format/model/
history identity, immutable borrowed inputs and exclusive disjoint writes.
The C adapter observes actual span addresses; Coax decides admission,
rounding and publication. The codec allocates nothing and does not implement
the ring, compression, rotary operation or cache sharing itself.

## Budget work independently

For one codec batch, let `N` be the number of values and `P` its code-plus-scale
bytes. The explicit live codec inventory is `6*N + 2*P`: BF16 input, decoded
output and decode scratch, plus packed output and packing scratch. This
inventory grows with chunk size. Persistent cache capacity is independent;
a 512-row prefill chunk is allowed with a 128-row window ring.

`dk_budget` checks both limits. `dk_flash_cache_budget` specializes the pinned
configuration. Forty backbone layers and up to three draft layers each own a
128-row window. Only layers 2, 8, 14 and 20 own compressed KV **and index keys**:
three ratio-2 sources and one ratio-1 source. The other four indexer source
layers compute queries while reusing those keys. Thus the source-row count is
`3*floor(context/2) + context`.

<!-- source: psionic/lib/deepseek/cache.coax flash-budget -->
```jai
dk_flash_cache_budget :: (context:int,chunk:int,drafts:int,cache_limit:int,step_limit:int)->DkBudget #export_c {
    if context<1 || context>1048576 || chunk<1 || chunk>context || drafts<0 || drafts>3 {return DkBudget.{status=DkStatus.shape};}
    // Four producers own both compressed KV and index keys: three ratio-2,
    // one ratio-1. The other four indexers own queries, not additional keys.
    source_rows:=3*(context/2)+context;
    cache:=(40+drafts)*128*528+source_rows*356;
    // The widest codec row is the 32 x 128 index query, larger than KV rows.
    // Six BF16 bytes/value + two packed code/scale spans, all explicitly live.
    step:=chunk*28928;
    if cache>cache_limit || step>step_limit {return DkBudget.{status=DkStatus.budget};}
    return DkBudget.{cache=cache,step=step,total=cache+step};
}
```

At context 1,024 with three draft windows, the persistent payload is 3,817,472
bytes. A 64-token codec chunk requires another 1,851,392 bytes under this
inventory. The larger index-query row determines that reusable work lane.
One byte below either limit refuses, with no automatic chunk reduction.

These are logical codec payload/work counts. They exclude allocator alignment,
page tables, retained prefixes, compressor state, attention scores/candidates,
model activations, weights, GPU commands and scheduling overlap. They are not
a whole-model memory forecast. Those allocations must enter their own budgets.

## Run and check it

The [example](../examples/deepseek-cache.coax) packs and decodes both KV grids
and index keys, then admits the pinned cache/work inventory.

```shell
coax/bin/coax run psionic/examples/deepseek-cache.coax
coax/bin/coax run psionic/examples/deepseek-cache.coax --release
psionic/tests/check_deepseek_cache.sh
psionic/check.sh --only deepseek/cache --jobs 8
```

<!-- source: psionic/examples/deepseek-cache.coax example -->
```jai
main :: () {
    input:[1024]u16;for i:0..<1024 {input[i]=dk_bf16(f32(i%17-8)/4).bits;}
    for kind:0..<3 {
        width:=ifx kind==2 then 128 else 512;p:=dk_plan(cast(u8)kind,2,width);
        data:[1024]u8;scales:[64]u8;stage:[1088]u8;output:[1024]u16;work:[1024]u16;
        packed:=DkPacked.{data=data[0..<p.data],scales=scales[0..<p.scales]};
        r:=dk_pack(cast(u8)kind,2,width,input[0..<p.values],packed,stage[..],false);#assert r.status==DkStatus.ok;
        r=dk_unpack(cast(u8)kind,2,width,packed,output[..],work[..],false);#assert r.status==DkStatus.ok && r.written==p.values;
        print("cache kind %: % values, % code bytes, % scale bytes\n",kind,p.values,p.data,p.scales);
    }
    // A prefill chunk can be larger than the persistent 128-row window ring.
    #assert dk_budget(0,512,128,512,67584,2113536).status==DkStatus.ok;
    budget:=dk_flash_cache_budget(1024,64,3,3817472,1851392);#assert budget.status==DkStatus.ok;
    print("Flash codec inventory: % persistent bytes; % chunk work bytes\n",budget.cache,budget.step);
}
```

The independent oracle checks every one of the 65,536 BF16 input patterns in
uniform, sparse and fixed-anchor groups for all three profiles. Anchors expose
rounding boundaries while another lane sets the scale. It also checks all
73,728 encoded coefficient/scale pairs and every neighboring finite BF16
midpoint with its adjacent F32 values. Original-width mixed rows distinguish
scale axes, nibble order, zero signs and chunk boundaries.

Native debug/release, plain and ASan/UBSan checks cover actual aliases, dirty
work, spare tails and late failures. The pinned memory planner is compared
with an independent per-layer configuration inventory. Exact evidence,
commands, source hashes and scope are in the
[verification record](../../docs/verification/psionic-ch66-2026-10-29.md).
