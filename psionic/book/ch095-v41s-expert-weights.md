# 95. V4.1's expert weights

V4.1's routed experts keep their original FP4 bytes. A separate scale tensor
assigns one UE8M0 scale to each row and 32 elements along K. The shared expert
uses FP8 E4M3 weights with scales per 32 × 32 block. This chapter reads both
formats, rounds activations onto the original FP8 grid, and runs clamped
SwiGLU with the route weight applied before the down projection.

The implementation is `deepseek-expert-cpu-wide-v1`: original encoded
coefficients, F32 input/projection/activation storage, and compensated binary64
products and reductions. Exact decoded coefficients and activation grids use
binary64 so the entire finite format range remains representable. This is a
CPU equation reference; original device reductions and BF16 storage need their
own full-model qualification.

The pinned original [model](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/model.py),
[kernels](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/kernel.py)
and [conversion reference](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/convert.py)
were read, never executed. Their identities and format rules are retained in
[expert-contract.json](../fixtures/deepseek/expert-contract.json). The reviewed
original [shard header](../fixtures/deepseek/expert-original.json) independently
confirms the stored types and shapes.

## Two layouts, separate scales

| Matrix | Stored bytes | Scale tensor |
| --- | --- | --- |
| Routed gate/up | I8 `[2304,2560]`, logically FP4 `[2304,5120]` | UE8M0 `[2304,160]` |
| Routed down | I8 `[5120,1152]`, logically FP4 `[5120,2304]` | UE8M0 `[5120,72]` |
| Shared gate/up | FP8 `[2304,5120]` | UE8M0 `[72,160]` |
| Shared down | FP8 `[5120,2304]` | UE8M0 `[160,72]` |

Each packed FP4 byte puts even K in the low nibble and odd K in the high
nibble. At scale byte 127, `0x21` therefore decodes to 0.5 followed by 1.
The sign bit and the seven other positive magnitudes give
`0, 0.5, 1, 1.5, 2, 3, 4, 6`, including signed zero.

UE8M0 bytes 0–254 encode `2^(byte-127)`; byte 255 is NaN and refuses even
when paired with a zero coefficient. All FP4 codes are finite. FP8 E4M3
reserves codes 127 and 255 for NaN, while its largest finite magnitude is 448.
The [decoder](../lib/deepseek/expert.coax) preserves exact binary64 bits for
every finite code/scale pair. Some finite pairs exceed F32's range, so
`de_decode` returns wide values. Projection checks its final F32 narrowing.

`DeMatrix` keeps kind, logical rows/columns, original data and separate scale
views together. FP4 scale indexing is `[row, k/32]`; FP8 indexing is
`[row/32, k/32]`. Rows are 1–8,192 and K is a multiple of 32 up to 8,192.
Every byte and scale extent must match exactly. Direct encoded projection
avoids materializing a full matrix of decoded weights.

## Route weighting belongs before the next grid

Every expert projection input rounds onto an FP8 E4M3 activation grid in 32-value groups.
The F32 maximum magnitude has a floor of 1e-4. Multiply it by the F32 inverse
of 448, round the scale up to a power of two using the reference's exponent/
mantissa rule, and round normalized values to nearest with ties to even.
Wide dequantized grid values preserve the exact code/scale product, including
the rare finite grid value above F32's range. Original coefficients remain
encoded until each product reads them.

Gate and up projections publish F32 values. With the original limit 10, clamp
up to `[-10,10]` and gate only from above at 10. Evaluate ordinary SiLU times
up, round that product to F32, multiply by the F32 route weight, and round
again. The weighted activation then enters the down projection's FP8 grid.
The shared expert follows the same path with route weight one.

Putting the weight after the down projection changes the input grid. The
independent owned fixture changes by 4.94994 under that wrong ordering. Its
clamp cases also distinguish an upper-only gate clamp from a symmetric one.

<!-- source: psionic/lib/deepseek/expert.coax expert -->
```jai
de_expert :: (e:DeExpert,x:[]f32,route:f32,limit:f32,out:[]f32,work:DeWork,overlap:bool)->DeResult #export_c {
    p:=de_plan(e);if p.status!=DsStatus.ok {return DeResult.{status=p.status};}
    if x.count!=p.width || out.count<p.width || work.floats.count<p.floats || work.wide.count<p.wide {return DeResult.{status=DsStatus.capacity};}if overlap {return DeResult.{status=DsStatus.alias};}
    v:=de_views(work.floats,p);r:=de_project(e.gate,x,v.gate,work.wide,v.stage,false);if r.status!=DsStatus.ok {return r;}
    r=de_project(e.up,x,v.up,work.wide,v.stage,false);if r.status!=DsStatus.ok {return r;}
    activated:=de_activate(v.gate,v.up,route,limit,v.activation,v.stage,false);if activated.status!=DsStatus.ok {return activated;}
    r=de_project(e.down,v.activation,v.pending,work.wide,v.stage,false);if r.status!=DsStatus.ok {return r;}
    for j:0..<p.width {out[j]=v.pending[j];}return DeResult.{written=p.width,gate_clamped=activated.gate_clamped,up_clamped=activated.up_clamped};
}
```

`de_mixture` validates all expert metadata, evaluates only the selected routed
experts, accumulates their weighted outputs and adds one shared FP8 expert.
It supports 1–384 routed experts and 1–8 unique selected IDs; the original
model uses six of 384. Route weights arrive as finite nonnegative F32 values.
[Chapter 91](ch091-routers-that-balance-themselves.md) owns their selection and
normalization. The example explicitly converts its returned wide weights at
this boundary.

## Run an owned mixture

The [example](../examples/deepseek-expert.coax) runs a real owned quantized
FFN mixture with unequal width/hidden dimensions, FP4 routed matrices and a
shared FP8 branch. Its active clamps are counted separately.

```shell
coax/bin/coax run psionic/examples/deepseek-expert.coax
coax/bin/coax run psionic/examples/deepseek-expert.coax --release
psionic/tests/check_deepseek_expert.sh
psionic/check.sh --only deepseek/expert --jobs 8
```

<!-- source: psionic/examples/deepseek-expert.coax example -->
```jai
main :: () {
    b0:[8192]u8;b1:[8192]u8;b2:[8192]u8;bs:[8192]u8;
    experts:=[3]DeExpert.[de_fixture_0(b0[..]),de_fixture_1(b1[..]),de_fixture_2(b2[..])];shared:=de_fixture_3(bs[..]);
    logits:=[3]f32.[0,-2,3];bias:[3]f32;raw:[3]f64;ranked:[3]f64;
    route:=dr_route(DrConfig.{profile=DrProfile.v41,k=2,scale=1.5},logits[..],bias[..],raw[..],ranked[..],false);#assert route.status==DrStatus.ok;
    ids:[2]int;weights:[2]f32;for i:0..<2 {ids[i]=route.ids[i];weights[i]=f32(route.weights[i]);}
    x:=DEG_INPUT;out:[64]f32;floats:[352]f32;wide:[64]f64;acc:[64]PsionicAccumulator;
    r:=de_mixture(experts[..],shared,x[..],ids[..],weights[..],10,out[..],DeWork.{floats=floats[..],wide=wide[..]},acc[..],false);#assert r.status==DsStatus.ok && r.written==64;
    print("selected FP4 experts %, %; route weights enter before down projection\n",ids[0],ids[1]);
    print("one shared FP8 expert is included after the routed sum\n");
    print("clamped gate lanes %; clamped up lanes %\n",r.gate_clamped,r.up_clamped);
    print("FP4: low nibble first, row/32 scales; FP8: 32 x 32 scales\n");
}
```

The plan exposes exact private intervals for gate, up, activation, projection
stage and pending output, plus a wide activation-grid workspace. Mixture work
adds a single-expert result, a combined result and compensated accumulators.
The caller allocates these before execution; the library allocates nothing.

All public outputs publish after complete success. A failure in the last down
coefficient or in the shared expert preserves the previous complete output
and spare tail. Private work may change. Valid typed extents/allocations,
matching tensor identity, immutable borrowed bytes, consistent expert shapes
and exclusive disjoint writable spans remain caller duties. C observes native
addresses; all admission, format and numerical decisions stay in Coax.

## Original component qualification

The complete 7,389,759,032-byte original shard 3 was SHA-256-admitted in Coax.
Its reviewed header covers the whole payload and contains all 384 experts of
layer 0, the shared expert and the original BF16 gate/F32 correction bias.
Two qualifications retain separate executed builds: individual routed/shared
FFNs and a complete original 384/6 MoE on the same owned 5,120-value input.

The complete MoE selects IDs `83,197,339,258,156,290`; the next excluded ID is
350, with a biased cutoff margin about 0.276785. All gate logits, F32 route
weights and 5,120 output values match the independent NumPy/Decimal reference
bit for bit. Individual original FFNs also match every gate, up, weighted
activation and output value, plus the exact down-input grid. These are
component equations on one owned input, with original-device/BF16 reductions
and complete V4.1 generation left to chapter 100.

Routine checks reconstruct the [retained evidence](../fixtures/deepseek/expert-cpu-original/manifest.json)
without original weights or NumPy. They verify actual numbers, route order,
margins and shared combination; altered outputs/IDs fail even if archive hashes
are recomputed. All FP4 and FP8 code/scale pairs, 1,012 rounding cases and 426
scale-transition cases also run against independent vectors.

An explicit new original qualification uses an already downloaded shard and a
new output directory. NumPy belongs only to the tooling Python's path. The
default runs the full original MoE; `--pair` runs individual original FFNs.

```shell
psionic/tests/qualify_deepseek_expert.sh /path/to/model-00003-of-00048.safetensors /tmp/expert-qualification
```

The [verification record](../../docs/verification/psionic-ch63-2026-10-29.md)
contains frozen gates, actual build identities, checks and limitations.
