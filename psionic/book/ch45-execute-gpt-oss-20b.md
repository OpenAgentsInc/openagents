# 45. Execute gpt-oss-20b

[Previous: Harmony](ch44-the-harmony-format.md) · [Contents](SUMMARY.md) ·
[Next: gpt-oss-120b](ch46-execute-gpt-oss-120b.md)

A decoder, a router and an attention fixture are useful pieces. They become a
model when the original embedding, every block and the complete vocabulary
head execute together. This chapter builds that path in Coax. It also supplies
the sampling prerequisites from proposed chapter 37; that chapter's broader
grammars and streaming service remain separate work.

The runnable profile is `gpt-oss-cpu-wide-v1`. It consumes original BF16 dense
parameters and original MXFP4 expert blocks directly. It uses no Torch,
converted checkpoint, inference library, Python runtime, BLAS or GPU kernel.
Python tools download data and author independent references. C maps files,
allocates storage, observes addresses and runs the native qualification harness.

## Pin a complete model before borrowing a row

The [target inventory](../fixtures/gptoss/original/pins.json) pins the
`original/` files at revision `6cee5e81ee83917806bbde320786a8fb61efebee`:
`config.json`, `dtypes.json` and `model.safetensors`. The weight file is
13,761,300,984 bytes. Its SHA-256 is
`3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed`.
The small sidecars have their own hashes. The converted files at the HF root
are a different inventory.

`g_bind_original` authenticates every complete file, including sidecars, then
binds a fixed, versioned descriptor table. The retained source header derives
363 exact tensor names, dtypes, ranks, shapes and absolute byte intervals.
Admission checks complete payload coverage, duplicate names, overlapping
intervals and every required model slot before publishing borrowed views.
`g_bind_trusted` is a metadata primitive whose explicit precondition is prior
successful authentication; it is not the public original-model entry point.

Both the bytes and the caller-owned block array must remain immutable and
alive while the model is used. Native address observation detects overlaps;
it cannot establish allocation validity, lifetime, exclusive ownership or a
caller's truthful extents.

## Compose one complete transition

Start by writing checks for the complete two-layer owned fixture. It has the
same 15 tensor slots per block, BF16 biases, two MXFP4 projections, learned
sinks, alternate masks and an untied head. Its independent reference traverses
the entire prefix one layer at a time, without a ring cache. Compare every
post-attention and post-MLP residual, selected expert ID, mixture weight,
logit and newly committed cache entry.

The original model has 24 layers, width/intermediate 2,880, 64 query heads,
8 KV heads and 32 experts, selecting four. One token follows this path:

1. Gather its original BF16 embedding row.
2. Apply learned-scale RMS, biased QKV projection and split-half YaRN.
3. Evaluate causal sink attention, with a 128-slot ring on even layers.
4. Project the context, add its bias and residual, then apply MLP RMS.
5. Project the biased router and select/normalize its top four experts.
6. Multiply original selected MXFP4 rows, apply interleaved clamped SwiGLU,
   multiply the second projection, include its bias, mix and add the residual.
7. Apply final RMS and project all 201,088 vocabulary logits.

`g_projection` bounds a product to 2^30 MACs. The full head exceeds the
teaching matvec's row limit, so BF16 products run in row tiles of 32,768.
MXFP4 products decode each selected block inside the compensated dot product;
they never expand the whole expert bank into floats. All original parameters
outside the packed expert weights are BF16, including norm scales and biases.

<!-- source: psionic/lib/gptoss/math.coax project -->
```jai
g_project :: (m:GMatrix,x:[]f32,bias:[]u8,out:[]f32,stage:[]f32,scratch:[]f32,overlap:bool)->GResult #export_c {
    p:=g_projection(cast(u8)m.encoding,m.rows,m.columns);if p.status!=GStatus.ok {return GResult.{status=p.status};}
    if m.data.count!=p.bytes || m.scales.count!=p.scales || x.count!=m.columns || (bias.count!=0 && bias.count!=2*m.rows) || out.count<m.rows || stage.count<m.rows || scratch.count<m.rows {return GResult.{status=GStatus.capacity};}
    if overlap {return GResult.{status=GStatus.alias};}
    if go_data(x)!=GoStatus.ok {return GResult.{status=GStatus.number};}if g_bytes_finite(bias)!=GStatus.ok {return GResult.{status=GStatus.number};}
    if m.encoding==GEncoding.bf16 {
        first:=0;
        while first<m.rows {
            count:=gs_window(m.rows-first,G_ROW_TILE);
            r:=qm_project(QmDtype.bf16,count,m.columns,m.data[2*first*m.columns..<2*(first+count)*m.columns],x,stage[first..<first+count],scratch[first..<first+count],false);
            if r.status!=QmStatus.ok {return GResult.{status=ifx r.status==QmStatus.number then GStatus.number else GStatus.arithmetic,cause=cast(int)r.status};}first+=count;
        }
    }else {
        for scale:m.scales {if scale>252 {return GResult.{status=GStatus.number};}}
        for row:0..<m.rows {
            acc:PsionicAccumulator;
            for group:0..<m.columns/32 {
                base:=row*m.columns/32+group;power:=f64_from_bits((cast(u64)m.scales[base]+896)<<52);
                for i:0..<16 {
                    byte:=m.data[base*16+i];lo:=byte&15;hi:=byte>>4;
                    a:=MX_E2M1[cast(int)(lo&7)]*power;if (lo&8)!=0 {a= -a;}
                    b:=MX_E2M1[cast(int)(hi&7)]*power;if (hi&8)!=0 {b= -b;}
                    acc=psionic_ops_accumulate(acc,a*f64(x[group*32+2*i]));acc=psionic_ops_accumulate(acc,b*f64(x[group*32+2*i+1]));
                }
            }
            value:=psionic_ops_total(acc);if !psionic_ops_fits_f32(value) {return GResult.{status=GStatus.arithmetic};}stage[row]=f32(value);
        }
    }
    for row:0..<m.rows {
        value:=f64(stage[row]);if bias.count!=0 {value+=f64(g_bf(bias,row));}
        if !psionic_ops_fits_f32(value) {return GResult.{status=GStatus.arithmetic};}stage[row]=f32(value);
    }
    for row:0..<m.rows {out[row]=stage[row];}return GResult.{written=m.rows};
}
```

The numerical profile matters. Dots use decoded coefficients and compensated
binary64 reductions. A projection narrows to binary32 before a bias is added;
the biased result narrows again. Norms use the learned scale directly, epsilon
1e-5 inside the root, with binary32 outputs. Activations, rotary coefficients,
cache entries, mixture weights and residuals are stored in binary32. Attention
scores, exponentials and weighted sums use binary64. Lowest expert IDs win
router ties. This is separate from upstream Torch's BF16 activation profile.
No bitwise Torch parity is claimed.

`g_forward` stages every layer's new KV slot and all public diagnostics. Only
a successful vocabulary readout commits them. A bad late head value therefore
preserves the complete live cache, logits, routes, weights and traces. Scratch
may change. Unused public tails remain untouched. The caller supplies the exact
model, sequential history and absolute position; an array cannot authenticate
its own history. Full-attention capacity may reach 131,072; even-layer storage
stays bounded by the window. Capacity planning is not an OS memory reservation.

## Choose and stop in Coax

`gs_distribution` provides temperature 0–100, top-k and top-p. Zero temperature
chooses the lowest-ID maximum. Nonzero temperatures use a stable merge sort:
descending logit, then ascending ID; keep top-k first, then the shortest nucleus
whose cumulative mass reaches top-p, and renormalize. A byte mask excludes
candidate IDs before any choice. Every supplied logit must be finite, including
masked lanes. Empty masks refuse.

The counter-based RNG is SplitMix64: seed plus the golden increment times
(counter+1), with explicitly wrapping integer arithmetic; the high 53 bits give
a uniform in [0,1). One successful emission consumes one counter, including
greedy mode. Exhaustion refuses. Cancellation is observed between steps and
consumes no counter. `<|end|>` continues the stream; return and call stop it.
Token exhaustion has its own stop reason. A cancelled or limited stream is not
a successfully parsed complete answer.

<!-- source: psionic/lib/gptoss/generation.coax emit -->
```jai
gg_emit :: (logits:[]f32,mask:[]u8,options:GSampling,state:GGeneration,cancel:bool,probabilities:[]f64,work:GSampleWork,overlap:bool)->GEmission #export_c {
    if state.stop!=GStop.running || state.remaining<1 || state.remaining>131072 {return GEmission.{status=GStatus.state};}
    if cancel {next:=state;next.stop=GStop.cancelled;return GEmission.{state=next};}
    draw:=gs_draw(state.random);if draw.status!=GStatus.ok {return GEmission.{status=draw.status};}
    d:=gs_distribution(logits,mask,options.temperature,options.top_k,options.top_p,probabilities,work.stage,work.order,work.temporary,overlap);if d.status!=GStatus.ok {return GEmission.{status=d.status};}
    pick:=gs_pick(probabilities[0..<logits.count],draw.value);#assert pick.status==GStatus.ok;
    next:=gg_advance(state,pick.token,draw.next);
    return GEmission.{token=pick.token,state=next};
}
```

Model steps and emissions are separate streaming transactions. A successful
model step may already have advanced its cache when emission refuses; retain
that step's logits and retry emission, rather than evaluating the token twice.
A whole turn is not one atomic publication. No server, concurrent scheduler or
asynchronous cancellation inside a matrix product is implemented here.

## Run the owned model

[The example](../examples/execute-gptoss.coax) needs no downloaded model:

```shell
coax/bin/coax run psionic/examples/execute-gptoss.coax
COAX=coax/bin/coax psionic/tests/check_gptoss.sh
```

<!-- source: psionic/examples/execute-gptoss.coax example -->
```jai
main :: () {
    data:=GT_FILE;blocks:[2]GBlock;model:=gt_model(data[..],blocks[..]);plan:=g_plan(model.shape);
    cache:[1280]f32;logits:[16]f32;trace:[128]f32;routes:[4]int;weights:[4]f32;
    floats:[1792]f32;scores:[8]f64;sums:[32]PsionicAccumulator;work_routes:[4]int;
    out:=GOutput.{logits=logits[..],trace=trace[..],routes=routes[..],weights=weights[..]};
    work:=GWork.{floats=floats[..],scores=scores[..],sums=sums[..],routes=work_routes[..]};
    step:=g_forward(model,1,0,g_macs(model.shape,0),cache[..],out,work,false);
    #assert step.status==GStatus.ok && plan.cache==1280;
    probabilities:[16]f64;stage:[16]f64;order:[16]int;temporary:[16]int;
    sample:=gg_emit(logits[..],data[0..<0],GSampling.{temperature=0},GGeneration.{remaining=1},false,probabilities[..],GSampleWork.{stage=stage[..],order=order[..],temporary=temporary[..]},false);
    #assert sample.status==GStatus.ok;
    print("complete tiny model: % layers; % experts; % selected\n",model.shape.layers,model.shape.experts,model.shape.selected);
    print("step: % logits; % MACs; next position %\n",step.written,step.macs,step.next);
    print("routes: % % / % %\n",routes[0],routes[1],routes[2],routes[3]);
    print("greedy next token: %; counter: %; stopped at limit\n",sample.token,sample.state.random.counter);
}
```

Expected output:

```jai
complete tiny model: 2 layers; 4 experts; 2 selected
step: 16 logits; 29696 MACs; next position 1
routes: 3 0 / 1 3
greedy next token: 5; counter: 1; stopped at limit
```

## Qualify the original checkpoint

The opt-in [original gate](../tests/check_gptoss_original.sh) downloads only
pinned data to `~/.cache/psionic/gptoss-original/`. It authenticates complete
files in Coax, executes the original model natively, and compares retained
outputs with an owned NumPy reference. That reference decodes original bytes,
uses independent binary64 BLAS reductions and traverses the whole sequence
layer by layer. It never imports or runs upstream code. NumPy is a reference
tooling dependency only. Default checks remain offline.

The fixed threshold, before candidate results are read, is
`abs(candidate-reference)/(1+abs(reference)) < 5e-4`, with exact router IDs.
For the two retained original prefixes, all layer residuals, router IDs,
mixture weights and all vocabulary logits matched exactly after binary32
storage. Independent original-rank tokenizer vectors come from chapter 44.
Fresh full-prefix replay and retained cached evaluation must agree byte for
byte under the Coax profile; replay shares kernels, while the dense Python
reference supplies the independent traversal.

On the 128 GiB Apple host, a recorded native run authenticated in 59.588 s,
then took 3.544 and 3.532 s for the two token steps. Peak RSS was 13.793 GB.
The separately recorded seeded Harmony run used temperature 0.7, top-k 40,
top-p 0.9 and seed 45001. It emitted 52 IDs through return, including framing
and intermediate channels, and its final message was `Hi! 👋`. These are
single-run observations, not throughput or tail-latency benchmarks.

The [verification record](../../docs/verification/psionic-ch45-2026-10-29.md)
binds commands, limits and retained evidence. General-purpose service admission,
GPU execution, long-context quality and BF16-runtime parity remain separate
gates. Next we change the original model identity and dimensions, keeping this
engine.
