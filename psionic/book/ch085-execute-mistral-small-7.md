# 85. Execute Mistral Small 7

> **Status: partly implemented.** The checkpoint inventory (from its headers
> alone), the mixture-of-experts router and layer, the latent-attention layer
> on FP8 weights, a small model of both layers against an oracle, and the
> memory plan on this host run. The original weights, the whole 36-layer
> model, its tokens and logits, references from the published model, and a
> measured resident run do not yet; see
> [What this does not do yet](#what-this-does-not-do-yet).

The plan names this chapter "Mistral Small 7"; its pinned target is
Mistral-Small-4-119B-2603 (Apache-2.0). It joins
[chapter 84](ch084-attention-through-a-latent.md)'s latent attention with a
mixture of experts (MoE): each of its 36 decoder layers holds 128 routed
experts and one shared expert, and a router sends each token to four of the
routed experts. The checkpoint holds 119.0 billion text values, stored
mostly in FP8; a token multiplies about 6.1 billion of them. This chapter
reads the checkpoint's inventory without downloading it, runs both halves of
a decoder layer on FP8 weights against an independent oracle, and plans
where the model fits on this machine.

```shell
coax/bin/coax run psionic/examples/mistral-small.coax
coax/bin/coax test psionic/lib/mistral-small.coax
python3 -I psionic/tests/mistral_small_manifest.py --fetch DIR
python3 -I psionic/tests/mistral_small_manifest.py DIR
```

The [library](../lib/mistral-small.coax), its
[check oracle](../tests/mistral-small-oracle.coax), the
[example](../examples/mistral-small.coax), the
[refusal example](../examples/mistral-small-refuse.coax) and the
[manifest tool](../tests/mistral_small_manifest.py) are the implementation.
It loads chapter 84's latent-attention unit (and through it chapter 90's
`ml_attention`), chapter 82's FP8 decode and chapter 33's residency plan.
Evidence from [chapter 81](ch081-execute-ministral-4.md)'s Ministral is not
counted for this model.

## The pinned checkpoint, read by its headers

The weights come from Hugging Face at revision
`a11f36bebf709121056b1dbcc943d1c6afbe494d` of
`mistralai/Mistral-Small-4-119B-2603`. The repository holds the same weights
twice: in Mistral's own layout (`consolidated-*.safetensors`, 7 shards) and
in the Hugging Face layout (`model-*.safetensors`, 3 shards of 49.1, 49.1
and 22.7 GB). This chapter pins the Hugging Face layout, 120,922,973,936
bytes. This host's disk had 38.7 GB free, so nothing but headers was
downloaded.

`--fetch` downloads `config.json`, `model.safetensors.index.json` and, with
HTTP range requests, the 8-byte length and the JSON header of each shard:
about 4 MB. The second form of the tool checks that the config, the index
and the headers hash to their pins; that in each shard every tensor's bytes
are exactly its shape times its dtype size, tiling the data with no gap and
ending at the shard's pinned size; and that the index and the headers agree
on every tensor's shard. A header read by range cannot be checked against
the shard's SHA-256, which needs the whole shard; the shard hashes are
recorded in the tool for a later full download.

There are 1,485 tensors. When layers 0 to 35 each hold a tensor of the same
dtype and shape under one name, the tool writes it as one line named
`layers.0..35`. The text model is then 38 lines; the vision tower and its
projector, 222 tensors that this chapter does not run, are kept apart.

<!-- source: psionic/lib/mistral-small.coax shape -->
```jai
// The text model's shape: hidden width, layers, attention heads, the query
// and key-value latent ranks, each head's non-rotary, rotary and value
// widths, the routed experts, how many a token uses, an expert's width, the
// shared expert's width and the vocabulary.
M4Shape :: struct { hidden:int; layers:int; heads:int; q_rank:int; kv_rank:int; nope:int; rope:int; value:int;
    experts:int; top_k:int; expert_mlp:int; shared_mlp:int; vocab:int; }
M4Status :: enum u8 {ok;shape;capacity;number;arithmetic;checkpoint;}
// The pinned text_config: hidden_size 4096, num_hidden_layers 36,
// num_attention_heads 32, q_lora_rank 1024, kv_lora_rank 256,
// qk_nope_head_dim 64, qk_rope_head_dim 64, v_head_dim 128,
// n_routed_experts 128, num_experts_per_tok 4, moe_intermediate_size 2048
// (times n_shared_experts 1 for the shared expert), vocab_size 131072.
m4_small4 :: ()->M4Shape {
    return M4Shape.{hidden=4096,layers=36,heads=32,q_rank=1024,kv_rank=256,nope=64,rope=64,value=128,
        experts=128,top_k=4,expert_mlp=2048,shared_mlp=2048,vocab=131072};
}
// Bounds every function checks before it reads a weight.
M4_TOP_MAX :: 8;M4_EXPERTS_MAX :: 256;
m4_valid :: (s:M4Shape)->bool {
    if s.hidden<1 || s.layers<1 || s.heads<1 || s.q_rank<1 || s.kv_rank<1 || s.nope<1 || s.value<1 || s.vocab<1 {return false;}
    if s.rope<2 || s.rope%2!=0 || s.rope/2>64 || s.expert_mlp<1 || s.shared_mlp<1 {return false;}
    return s.experts<=M4_EXPERTS_MAX && s.top_k>=1 && s.top_k<=M4_TOP_MAX && s.top_k<s.experts;
}
```

The expected lines come from the shape alone. A decoder layer has 35
tensors: two block norms, the router's gate (BF16), every expert's fused
`gate_up_proj` and `down_proj` (FP8, one scale an expert), the shared
expert's three matrices, and the five attention projections with their two
norms. Each FP8 matrix carries a BF16 `weight_scale_inv` (the value every
code is multiplied by) and a BF16 `activation_scale`.

<!-- source: psionic/lib/mistral-small.coax inventory -->
```jai
m4_put :: (out:[]u8,n:int,s:string)->int {for i:0..<s.count {out[n+i]=s[i];}return n+s.count;}
m4_put_int :: (out:[]u8,n:int,v:int)->int {
    digits:[20]u8;k:=0;x:=v;
    while true {digits[k]=cast(u8)('0'+x%10);k+=1;x/=10;if x==0 {break;}}
    m:=n;while k>0 {k-=1;out[m]=digits[k];m+=1;}
    return m;
}
// One manifest line: the name, the dtype, the dimensions joined by commas
// ("-" for a scalar).
m4_line :: (s:M4Shape,i:int,out:[]u8)->int {
    t:=m4_expected(s,i);n:=m4_put(out,0,"language_model.");
    if t.layered {n=m4_put(out,n,"model.layers.0..");n=m4_put_int(out,n,s.layers-1);n=m4_put(out,n,".");n=m4_put(out,n,m4_group(i-2));}
    n=m4_put(out,n,t.name);n=m4_put(out,n,ifx t.fp8 then " F8_E4M3 " else " BF16 ");
    if t.rank==0 {return m4_put(out,n,"-");}
    for k:0..<t.rank {if k>0 {n=m4_put(out,n,",");}n=m4_put_int(out,n,t.dims[k]);}
    return n;
}
// The values and bytes a line stands for: per-layer lines count every
// layer; an FP8 value is one byte, a BF16 value two.
m4_values :: (s:M4Shape,i:int)->int {
    t:=m4_expected(s,i);v:=1;
    for k:0..<t.rank {v*=t.dims[k];}
    return ifx t.layered then v*s.layers else v;
}
m4_bytes :: (s:M4Shape,i:int)->int {return ifx m4_expected(s,i).fp8 then m4_values(s,i) else 2*m4_values(s,i);}
// A manifest matches when it holds every expected line exactly once and
// nothing else (the expected lines are distinct; a check holds them so).
// bad names the first expected line missing or repeated, or
// is -1 when they are all there once but there are more lines.
M4Inventory :: struct { status:M4Status; lines:int; parameters:int; bytes:int; bad:int; }
M4_LINES_MAX :: 64;
m4_inventory :: (s:M4Shape,manifest:string)->M4Inventory {
    if !m4_valid(s) {return M4Inventory.{status=M4Status.shape,bad=-1};}
    ends:[65]int;lines:=0;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==M4_LINES_MAX {return M4Inventory.{status=M4Status.checkpoint,lines=lines+1,bad=-1};}
        lines+=1;ends[lines]=i;
    }}
    r:M4Inventory;r.lines=lines;r.bad=-1;buf:[160]u8;
    for e:0..<M4_EXPECTED {
        n:=m4_line(s,e,buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {return M4Inventory.{status=M4Status.checkpoint,lines=lines,bad=e};}
        r.parameters+=m4_values(s,e);r.bytes+=m4_bytes(s,e);
    }
    if lines!=M4_EXPECTED {return M4Inventory.{status=M4Status.checkpoint,lines=lines,bad=-1};}
    return r;
}
```

The checks hold the pinned headers to the shape and the config to the
fields the shape is built from:

<!-- source: psionic/lib/mistral-small.coax checks-checkpoint -->
```jai
#test "the pinned headers hold exactly the tensors the config names" {
    s:=m4_small4();r:=m4_inventory(s,MS_PINNED_MANIFEST);
    #assert MS_PINNED_LINES==38 && r.status==M4Status.ok && r.lines==38 && r.bad==-1;
    // The text model: 118,972,845,632 values in 120,065,825,920 bytes; with
    // the vision tower, the projector and the 3 headers, the 3 shard sizes.
    #assert r.parameters==118972845632 && r.bytes==120065825920;
    #assert r.bytes+MS_PINNED_OTHER_BYTES+MS_PINNED_HEADER_BYTES==MS_PINNED_BYTES;
    // 1,485 tensors: 35 lines stand for 36 layers each, 222 are vision.
    #assert 3+35*36+MS_PINNED_OTHER_TENSORS==MS_PINNED_TENSORS;
    buf:[2048]u8;n:=m4_config_text(s,buf[..]);
    #assert cast(string)buf[0..<n]==MS_PINNED_CONFIG;
}
```

The count works by hand. The two BF16 tables (embedding and readout) are
2 × 131,072 × 4,096 × 2 = 2,147,483,648 bytes. One routed expert is
3 × 2,048 × 4,096 = 25,165,824 FP8 bytes, and 36 layers of 128 experts are
115,964,116,992 bytes, 96.6% of the text model. Attention adds
1,024 × 4,096 + 4,096 × 1,024 + 320 × 4,096 + 6,144 × 256 + 4,096 × 4,096
= 27,000,832 bytes a layer, the shared expert 25,165,824, the router
2 × 128 × 4,096 = 1,048,576. With 728,192 bytes of norms and scales, the
text model is 120,065,825,920 bytes; with the vision tower's 856,944,640
bytes and 203,376 bytes of headers, that is the pinned shard total.

## FP8 products

Every matrix but the router, the embedding and the readout is E4M3 FP8: a
sign, four exponent bits and three fraction bits, as in
[chapter 82](ch082-original-fp8-weights.md). The scale is one BF16 value for
the whole matrix (one an expert for the routed experts), not chapter 82's
128 × 128 blocks. Because the scale is shared by a whole row, the product
sums the codes' exact values times the input and multiplies the row's sum by
the scale once.

<!-- source: psionic/lib/mistral-small.coax steps -->
```jai
// An FP8 matrix: row-major E4M3 codes and the scale every code is
// multiplied by (the checkpoint's weight_scale_inv, a BF16 value).
M4Matrix :: struct { codes:[]u8; scale:f32; }
m4_matrix_ok :: (m:M4Matrix,rows:int,cols:int)->bool {
    return m.codes.count==rows*cols && fp8_f32_scale_ok(f32_bits(m.scale));
}
// out[r] = scale times the sum over k of code[r, k] x[k], one compensated
// binary64 sum a row, narrowed to f32 once. A NaN code refuses.
m4_linear :: (m:M4Matrix,x:[]f32,out:[]f32)->bool {
    cols:=x.count;
    for r:0..<out.count {
        acc:PsionicAccumulator;
        for k:0..<cols {
            code:=m.codes[r*cols+k];
            if fp8_nan(code) {return false;}
            acc=psionic_ops_accumulate(acc,fp8_unit(code)*f64(x[k]));
        }
        v:=psionic_ops_total(acc)*f64(m.scale);
        if !psionic_ops_fits_f32(v) {return false;}
        out[r]=f32(v);
    }
    return true;
}
// RMS norm with epsilon 1e-6, then the weight.
M4_EPSILON :: 0.000001;
m4_norm :: (x:[]f32,weight:[]f32,out:[]f32)->bool {
    acc:PsionicAccumulator;
    for v:x {acc=psionic_ops_accumulate(acc,f64(v)*f64(v));}
    scale:=1/sqrt(psionic_ops_total(acc)/f64(x.count)+M4_EPSILON);
    for i:0..<x.count {
        v:=f64(x[i])*scale*f64(weight[i]);
        if !psionic_ops_fits_f32(v) {return false;}
        out[i]=f32(v);
    }
    return true;
}
// hidden += update, narrowed to f32.
m4_residual :: (hidden:[]f32,update:[]f32)->bool {
    for i:0..<hidden.count {
        v:=f64(hidden[i])+f64(update[i]);
        if !psionic_ops_fits_f32(v) {return false;}
        hidden[i]=f32(v);
    }
    return true;
}
```

The checkpoint's config sets `activation_scheme` to `static`: a reference
that follows it rounds each activation to FP8 with the stored
`activation_scale` before the product. This profile keeps activations in
f32 and records the activation scales without applying them; an
activation-quantized profile is a separate, later step.

## The router

Mistral Small 4's router is not chapter 164's. The transformers definition
(`Mistral4TopkRouter`, read and not run) takes the softmax of all 128
logits, chooses the four largest, and divides their scores by their sum
(`norm_topk_prob` is true); `routed_scaling_factor` is 1, there is no
correction bias, and with one expert group the group choice changes
nothing. Work one by hand. One input value 1 and four experts with gate rows
1, 3, 2 and 0 give logits 1, 3, 2, 0. The softmax is e, e³, e², 1 over their
sum 31.19: scores 0.0871, 0.6439, 0.2369 and 0.0321. Choosing two takes
experts 1 and 2. Their weights are 0.6439 / 0.8808 = 0.7311 and 0.2689. A
renormalized softmax over the chosen experts is a softmax over their logits
alone, so the two weights are the sigmoid of the logit gap: σ(1) and σ(−1).
The margin, the last chosen score minus the best unchosen one, is
0.2369 − 0.0871 = 0.1497.

<!-- source: psionic/lib/mistral-small.coax route -->
```jai
// The router for one normed input x, with one binary64 score an expert in
// fwork: each expert's logit is its gate row
// times x; the scores are the softmax of all the logits. The top_k largest
// scores are chosen, the lower index first on a tie. Their weights are their
// scores divided by the chosen scores' sum, times
// routed_scaling_factor 1. margin is the last chosen score minus the
// largest unchosen one: how far the selection is from changing.
M4_SCALE :: 1.0;
M4Route :: struct { status:M4Status; count:int; ids:[8]int; weights:[8]f32; margin:f64; }
m4_route :: (gate:[]f32,x:[]f32,k:int,fwork:[]f64)->M4Route {
    E:=fwork.count;h:=x.count;
    if E<2 || E>M4_EXPERTS_MAX || k<1 || k>M4_TOP_MAX || k>=E || h<1 || gate.count!=E*h {return M4Route.{status=M4Status.shape};}
    largest:=0.0;
    for e:0..<E {
        acc:PsionicAccumulator;
        for i:0..<h {acc=psionic_ops_accumulate(acc,f64(gate[e*h+i])*f64(x[i]));}
        // A gate value that is not finite makes the compensated sum NaN.
        fwork[e]=psionic_ops_total(acc);
        if !(fwork[e]==fwork[e]) {return M4Route.{status=M4Status.number};}
        largest=ifx e==0 then fwork[e] else m4_larger(largest,fwork[e]);
    }
    // Every exponent is at most 0, so no exp overflows; the largest is 1.
    acc:PsionicAccumulator;
    for e:0..<E {fwork[e]=exp(fwork[e]-largest);acc=psionic_ops_accumulate(acc,fwork[e]);}
    total:=psionic_ops_total(acc);
    for e:0..<E {fwork[e]/=total;}
    r:M4Route;r.count=k;used:[256]bool;sum:=0.0;
    for slot:0..<k {r.ids[slot]=m4_best(fwork,used[..]);used[r.ids[slot]]=true;sum+=fwork[r.ids[slot]];}
    r.margin=fwork[r.ids[k-1]]-fwork[m4_best(fwork,used[..])];
    // transformers adds 1e-20 to the sum; the top k of a softmax over at
    // most 256 experts sum to at least k / 256, where 1e-20 is below half a
    // unit in the last place, so the sum is unchanged and the term omitted.
    for slot:0..<k {r.weights[slot]=f32(fwork[r.ids[slot]]/sum*M4_SCALE);}
    return r;
}
// The larger of two finite values, without a comparison: (a + b + |a - b|) / 2.
m4_larger :: (a:f64,b:f64)->f64 {return (a+b+abs(a-b))/2;}
// The unused expert with the largest score, the lowest index on a tie.
m4_best :: (score:[]f64,used:[]bool)->int {
    best:=-1;
    for e:0..<score.count {if !used[e] && (best<0 || score[e]>score[best]) {best=e;}}
    return best;
}
// A route whose margin is below M4_MARGIN is a near tie: a reference that
// rounds differently may choose another expert.
M4_MARGIN :: 0.00001;
```

<!-- source: psionic/lib/mistral-small.coax checks-route -->
```jai
#test "the router, worked by hand" {
    // One input value 1, four experts with gate 1, 3, 2, 0: the softmax of
    // the logits is e, e^3, e^2, 1 over their sum 31.19, so experts 1 and 2
    // are chosen with scores 0.6439 and 0.2369. Renormalized over the two,
    // their weights are e^3 / (e^3 + e^2) = 0.7311 and 0.2689: the sigmoid
    // of the logit gap, +1 and -1. The margin is 0.2369 - 0.0871 = 0.1497.
    gate:=[4]f32.[1,3,2,0];x:=[1]f32.[1];fwork:[4]f64;
    r:=m4_route(gate[..],x[..],2,fwork[..]);
    sum:=exp(1.0)+exp(3.0)+exp(2.0)+1;
    #assert r.status==M4Status.ok && r.count==2 && r.ids[0]==1 && r.ids[1]==2;
    #assert abs(r.weights[0]-0.7310586)<1e-6 && abs(r.weights[1]-0.2689414)<1e-6 && abs(r.weights[0]+r.weights[1]-1)<1e-6;
    #assert abs(r.margin-(exp(2.0)-exp(1.0))/sum)<1e-12 && abs(r.margin-0.149738)<1e-6;
    #assert abs(fwork[3]-1/sum)<1e-15;
    // One expert takes weight 1, a tie goes to the lower index with margin 0.
    r=m4_route(gate[..],x[..],1,fwork[..]);#assert r.ids[0]==1 && r.weights[0]==1;
    tie:=[4]f32.[2,3,3,0];r=m4_route(tie[..],x[..],2,fwork[..]);
    #assert r.ids[0]==1 && r.ids[1]==2 && r.weights[0]==0.5 && r.weights[1]==0.5 && r.margin>0;
    r=m4_route(tie[..],x[..],1,fwork[..]);#assert r.ids[0]==1 && r.margin==0 && r.margin<M4_MARGIN;
    // Logits of 1000 and -1000 overflow nothing: the softmax subtracts the largest.
    far:=[4]f32.[1000,-1000,999,0];r=m4_route(far[..],x[..],2,fwork[..]);
    #assert r.status==M4Status.ok && r.ids[0]==0 && r.ids[1]==2 && abs(f64(r.weights[0])-1/(1+exp(-1.0)))<1e-6;
    // Logits all near -1000: shifted by the largest, not underflowed. A
    // workspace left holding large values does not move the shift.
    low:=[4]f32.[-1001,-1000,-1003,-1002];big4:=[4]f64.[1e300,1e300,1e300,1e300];
    r=m4_route(low[..],x[..],2,big4[..]);
    #assert r.status==M4Status.ok && r.ids[0]==1 && r.ids[1]==0 && abs(f64(r.weights[0])-1/(1+exp(-1.0)))<1e-6;
    hi4:=[4]f32.[1000,-1000,0,0];big4=[4]f64.[1e300,1e300,1e300,1e300];
    r=m4_route(hi4[..],x[..],2,big4[..]);#assert r.status==M4Status.ok && r.ids[0]==0 && r.ids[1]==1 && r.weights[0]==1 && r.weights[1]==0;
    // Refusals: shapes and a gate that is not finite.
    #assert m4_route(gate[..],x[..],0,fwork[..]).status==M4Status.shape && m4_route(gate[..],x[..],4,fwork[..]).status==M4Status.shape;
    #assert m4_route(gate[..],x[..],3,fwork[..]).status==M4Status.ok && m4_route(gate[0..<3],x[..],2,fwork[..]).status==M4Status.shape;
    #assert m4_route(gate[..],x[0..<0],2,fwork[0..<0]).status==M4Status.shape && m4_route(gate[0..<1],x[..],1,fwork[0..<1]).status==M4Status.shape;
    #assert m4_route(gate[0..<2],x[..],1,fwork[0..<2]).status==M4Status.ok;
    ten:[10]f32;big:[257]f32;one:[1]f32;w10:[10]f64;w257:[257]f64;
    #assert m4_route(ten[..],one[..],8,w10[..]).status==M4Status.ok && m4_route(ten[..],one[..],9,w10[..]).status==M4Status.shape;
    #assert m4_route(big[0..<256],one[..],8,w257[0..<256]).status==M4Status.ok && m4_route(big[..],one[..],8,w257[..]).status==M4Status.shape;
    huge:f32=3e38;inf:=huge*huge;bad:=gate;bad[2]=inf;#assert m4_route(bad[..],x[..],2,fwork[..]).status==M4Status.number;
    bad[2]=-inf;#assert m4_route(bad[..],x[..],2,fwork[..]).status==M4Status.number;
    bad[2]=3e38;#assert m4_route(bad[..],x[..],2,fwork[..]).status==M4Status.ok;
}
#test "the router matches the oracle's ranking by logits" {
    for seed:0..<40 {
        gate:[40]f32;m4o_values(seed,gate[..],0);xs:[8]f32;m4o_values(seed+3,xs[..],0.1);x:[8]f64;for i:0..<8 {x[i]=f64(xs[i]);}
        for k:1..<5 {fwork:[5]f64;r:=m4_route(gate[..],xs[..],k,fwork[..]);#assert r.status==M4Status.ok && r.count==k && m4o_agrees(r,m4o_route(gate[..],x[..],k));}
    }
}
```

The oracle ranks the experts by their logits rather than by the softmax
(the softmax keeps their order) and computes each chosen weight as a softmax
over the chosen logits, so it shares no arithmetic with the library.

## The mixture layer

Each expert is a SwiGLU: down(silu(gate x) × up x). In the checkpoint the
gate and up rows of all 128 experts are one tensor of 128 × 4,096 × 4,096
codes, gate rows first. The layer norms the residual row, routes it, sums
the chosen experts' outputs times their weights and adds the shared
expert's output unweighted, in binary64, then adds the result to the
residual.

<!-- source: psionic/lib/mistral-small.coax moe -->
```jai
// down(silu(gate x) * up x): gate and up are width rows each, down maps
// width back to hidden. inner holds 2 * width values.
m4_swiglu :: (gate:M4Matrix,up:M4Matrix,down:M4Matrix,x:[]f32,inner:[]f32,out:[]f32)->bool {
    w:=inner.count/2;g:=inner[0..<w];u:=inner[w..<2*w];
    if !m4_linear(gate,x,g) || !m4_linear(up,x,u) {return false;}
    for i:0..<w {
        a:=f64(g[i]);v:=a/(1+exp(-a))*f64(u[i]);
        if !psionic_ops_fits_f32(v) {return false;}
        g[i]=f32(v);
    }
    return m4_linear(down,g,out);
}
// One mixture layer's weights: the block norm (post_attention_layernorm),
// the router's gate rows (BF16, decoded), every expert's gate_up (gate rows
// then up rows) and down codes one expert after another with one scale an
// expert, and the shared expert's three matrices.
M4Moe :: struct { norm:[]f32; gate:[]f32; up:[]u8; up_scale:[]f32; down:[]u8; down_scale:[]f32; shared_gate:M4Matrix; shared_up:M4Matrix; shared_down:M4Matrix; }
m4_moe_work :: (s:M4Shape)->int {return 2*s.hidden+2*rp_max(s.expert_mlp,s.shared_mlp);}
m4_moe_fwork :: (s:M4Shape)->int {return s.hidden+s.experts;}
m4_moe_shape :: (s:M4Shape,w:M4Moe)->bool {
    h:=s.hidden;E:=s.experts;I:=s.expert_mlp;S:=s.shared_mlp;
    if !m4_valid(s) || w.norm.count!=h || w.gate.count!=E*h || w.up.count!=E*2*I*h || w.down.count!=E*h*I || w.up_scale.count!=E || w.down_scale.count!=E {return false;}
    for e:0..<E {if !fp8_f32_scale_ok(f32_bits(w.up_scale[e])) || !fp8_f32_scale_ok(f32_bits(w.down_scale[e])) {return false;}}
    return m4_matrix_ok(w.shared_gate,S,h) && m4_matrix_ok(w.shared_up,S,h) && m4_matrix_ok(w.shared_down,h,S);
}
// tokens rows of hidden through the layer: the block norm, the route, each
// chosen expert's output times its weight, plus the shared expert's, summed
// in binary64 and narrowed once; then the residual. routes[t] records token
// t's route. On a refusal, written counts the tokens already done.
M4Result :: struct { status:M4Status; written:int; }
m4_moe :: (s:M4Shape,w:M4Moe,tokens:int,hidden:[]f32,routes:[]M4Route,work:[]f32,fwork:[]f64)->M4Result {
    if !m4_moe_shape(s,w) {return M4Result.{status=M4Status.shape};}
    h:=s.hidden;I:=s.expert_mlp;E:=s.experts;
    if tokens<1 || hidden.count!=tokens*h || routes.count<tokens || work.count<m4_moe_work(s) || fwork.count<m4_moe_fwork(s) {return M4Result.{status=M4Status.capacity};}
    normed:=work[0..<h];out:=work[h..<2*h];inner:=work[2*h..<work.count];sums:=fwork[0..<h];
    for t:0..<tokens {
        for i:0..<h {sums[i]=0;}
        if !m4_norm(hidden[t*h..<(t+1)*h],w.norm,normed) {return M4Result.{status=M4Status.arithmetic,written=t};}
        r:=m4_route(w.gate,normed,s.top_k,fwork[h..<h+E]);routes[t]=r;
        if r.status!=M4Status.ok {return M4Result.{status=r.status,written=t};}
        for slot:0..<r.count {
            e:=r.ids[slot];up:=w.up[e*2*I*h..<(e+1)*2*I*h];
            gate:=M4Matrix.{codes=up[0..<I*h],scale=w.up_scale[e]};upm:=M4Matrix.{codes=up[I*h..<2*I*h],scale=w.up_scale[e]};
            down:=M4Matrix.{codes=w.down[e*h*I..<(e+1)*h*I],scale=w.down_scale[e]};
            if !m4_swiglu(gate,upm,down,normed,inner[0..<2*I],out) {return M4Result.{status=M4Status.arithmetic,written=t};}
            for i:0..<h {sums[i]+=f64(r.weights[slot])*f64(out[i]);}
        }
        if !m4_swiglu(w.shared_gate,w.shared_up,w.shared_down,normed,inner[0..<2*s.shared_mlp],out) {return M4Result.{status=M4Status.arithmetic,written=t};}
        for i:0..<h {
            v:=sums[i]+f64(out[i]);
            if !psionic_ops_fits_f32(v) {return M4Result.{status=M4Status.arithmetic,written=t};}
            out[i]=f32(v);
        }
        if !m4_residual(hidden[t*h..<(t+1)*h],out) {return M4Result.{status=M4Status.arithmetic,written=t};}
    }
    return M4Result.{written=tokens};
}
```

A one-value layer works by hand. Hidden 2 normalizes to 2 / √(4 + 10⁻⁶),
just under 1. Gate rows 1, 2 and −1 choose expert 1 alone, with weight 1.
Its gate code 0x38 is 1, so silu(1) = 0.7311; its up code 0x40 is 2, giving
1.4621; its down code 0x44 is 3, giving 4.3864. The shared expert's codes
are all 1: silu(1) = 0.7311. The residual becomes 2 + 4.3864 + 0.7311 =
7.1174 (7.1174088, since the normed value is a little under 1).

<!-- source: psionic/lib/mistral-small.coax checks-hand -->
```jai
#test "a mixture of one hidden value, worked by hand" {
    // Hidden 2 normalizes to 2 / sqrt(4 + 1e-6), just under 1. Gate rows 1,
    // 2 and -1 choose expert 1 alone, weight 1. Its gate row 1 (code 0x38)
    // gives silu(1) = 0.7311, its up row 2 (0x40) makes 1.4621, its down 3
    // (0x44) 4.3864. The shared expert's 1, 1, 1 give silu(1) = 0.7311. The
    // residual adds 5.1174 to 2: 7.1174, a little under
    // 7.11741 since the normed value is a little under 1.
    s:=M4Shape.{hidden=1,layers=1,heads=1,q_rank=1,kv_rank=1,nope=1,rope=2,value=1,experts=3,top_k=1,expert_mlp=1,shared_mlp=1,vocab=1};
    norm:=[1]f32.[1];gate:=[3]f32.[1,2,-1];up:=[6]u8.[0x30,0x30,0x38,0x40,0x30,0x30];down:=[3]u8.[0x30,0x44,0x30];ones:=[3]f32.[1,1,1];
    sg:=[1]u8.[0x38];
    w:=M4Moe.{norm=norm[..],gate=gate[..],up=up[..],up_scale=ones[..],down=down[..],down_scale=ones[..],
        shared_gate=M4Matrix.{codes=sg[..],scale=1},shared_up=M4Matrix.{codes=sg[..],scale=1},shared_down=M4Matrix.{codes=sg[..],scale=1}};
    h:=[1]f32.[2];routes:[1]M4Route;work:[4]f32;fwork:[4]f64;
    r:=m4_moe(s,w,1,h[..],routes[..],work[..],fwork[..]);
    #assert r.status==M4Status.ok && r.written==1 && routes[0].ids[0]==1 && routes[0].weights[0]==1 && abs(h[0]-7.1174088)<0.000001;
}
```

<!-- source: psionic/lib/mistral-small.coax checks-moe -->
```jai
m4_moe_view :: (f:[]f32,c:[]u8)->M4Moe {
    return M4Moe.{norm=f[0..<8],gate=f[8..<48],up=c[0..<240],up_scale=f[48..<53],down=c[240..<360],down_scale=f[53..<58],
        shared_gate=M4Matrix.{codes=c[360..<384],scale=f[58]},shared_up=M4Matrix.{codes=c[384..<408],scale=f[59]},shared_down=M4Matrix.{codes=c[408..<432],scale=f[60]}};
}
#test "the mixture layer matches the binary64 oracle, token by token and route by route" {
    s:=m4o_shape();
    for seed:0..<8 {
        a:=m4o_moe_weights(seed);hidden:[56]f32;m4o_values(seed+7,hidden[..],0);h:=hidden;
        routes:[7]M4Route;work:[22]f32;fwork:[13]f64;
        r:=m4_moe(s,m4_moe_view(a.f[..],a.c[..]),7,h[..],routes[..],work[..],fwork[..]);
        #assert r.status==M4Status.ok && r.written==7;
        for t:0..<7 {
            ref:[8]f64;m4o_moe(a.f[..],a.c[..],hidden[t*8..<t*8+8],ref[..]);g:=m4o_gap(h[t*8..<t*8+8],ref[..]);#assert g.gap<=g.limit;
            x:[8]f64;n:[8]f64;for i:0..<8 {x[i]=f64(hidden[t*8+i]);}m4o_rms(x[..],a.f[0..<8],n[..]);
            #assert routes[t].count==2 && m4o_agrees(routes[t],m4o_route(a.f[8..<48],n[..],2));
        }
    }
}
#test "a mixture of one token at a time equals the prompt at once" {
    s:=m4o_shape();a:=m4o_moe_weights(3);hidden:[56]f32;m4o_values(1,hidden[..],0);w:=m4_moe_view(a.f[..],a.c[..]);
    all:=hidden;routes:[7]M4Route;work:[22]f32;fwork:[13]f64;
    #assert m4_moe(s,w,7,all[..],routes[..],work[..],fwork[..]).written==7;
    each:=hidden;one:[1]M4Route;
    for t:0..<7 {
        #assert m4_moe(s,w,1,each[t*8..<t*8+8],one[..],work[..],fwork[..]).written==1;
        #assert one[0].ids[0]==routes[t].ids[0] && one[0].ids[1]==routes[t].ids[1] && one[0].margin==routes[t].margin;
    }
    for i:0..<56 {#assert all[i]==each[i];}
}
```

## Rotary positions and the softmax scale

Each head's query and the shared key have 64 non-rotary and 64 rotary
values. The rotary part uses YaRN with factor 128 over an original context
of 8,192. For a rotary width of 64, the correction dimensions are
64 ln(8192 / (32 × 2π)) / (2 ln 10000) = 12.88 and
64 ln(8192 / 2π) / (2 ln 10000) = 24.92, so pairs 0 to 11 keep the frequency
10000^(−2j/64), pairs 25 to 31 turn 128 times slower, and the pairs between
are mixed along a ramp. The weights store the rotary values interleaved, as
pairs (x₀, x₁), (x₂, x₃), …; the rotation writes each pair's halves split,
first halves then second halves, the layout transformers produces. The query
and the key share it, so their products are unchanged.

The softmax scale is 128^(−1/2) times mscale², with mscale =
0.1 ln 128 + 1 = 1.4852: 0.19497. Llama 4's position factor multiplies the
query by 1 + 0.1 ln(1 + ⌊p / 8192⌋): 1 below position 8,192 and 1.0693 from
8,192 to 16,383.

<!-- source: psionic/lib/mistral-small.coax rope -->
```jai
// YaRN with the pinned rope_parameters: theta 10000, factor 128, original
// context 8192, beta_fast 32, beta_slow 1. Pair j of a rotary width d turns
// by position times inv_freq(j). The correction dimensions bound a ramp from
// the original frequency (j below lo) to the original over 128 (j at hi or
// above); mscale and mscale_all_dim are equal, so cos and sin are unscaled.
M4_PI :: 3.141592653589793;
m4_correction :: (d:int,rotations:f64)->f64 {return f64(d)*log(8192/(rotations*2*M4_PI))/(2*log(10000.0));}
m4_inv_freq :: (d:int,j:int)->f64 {
    // For every even width 2 to 128, 0 <= lo < hi <= d - 1 (lo is about
    // d / 5, hi about 2d / 5, rounded up), so transformers' clamps to 0 and
    // d - 1 and its lo == hi guard never apply; a check covers every width.
    lo:=floor(m4_correction(d,32));hi:=-floor(-m4_correction(d,1));
    ramp:=(f64(j)-lo)/(hi-lo);ramp=(abs(ramp)-abs(ramp-1)+1)/2;
    base:=exp(-log(10000.0)*f64(2*j)/f64(d));
    return base/128*ramp+base*(1-ramp);
}
// Rotates x's interleaved pairs (x[2j], x[2j+1]) and writes them split: the
// first halves to out[j], the second to out[d/2 + j], the layout
// transformers' interleaved rotary produces. q and k share it, so their
// products are those of the pairs rotated in place.
m4_rotate :: (x:[]f32,position:int,out:[]f32)->bool {
    d:=x.count;
    for j:0..<d/2 {
        a:=f64(position)*m4_inv_freq(d,j);c:=cos(a);sn:=sin(a);u:=f64(x[2*j]);v:=f64(x[2*j+1]);
        p:=u*c-v*sn;q:=v*c+u*sn;
        if !psionic_ops_fits_f32(p) || !psionic_ops_fits_f32(q) {return false;}
        out[j]=f32(p);out[d/2+j]=f32(q);
    }
    return true;
}
// The softmax scale: qk_head_dim^-1/2 times mscale^2, mscale = 0.1 ln 128 + 1
// (yarn_get_mscale with mscale_all_dim 1); and Llama 4's position factor on
// the query, 1 + 0.1 ln(1 + floor(position / 8192)).
m4_softmax_scale :: (s:M4Shape)->f64 {m:=0.1*log(128.0)+1;return m*m/sqrt(f64(s.nope+s.rope));}
m4_position_scale :: (position:int)->f64 {return 1+0.1*log(f64(1+position/8192));}
```

<!-- source: psionic/lib/mistral-small.coax checks-rope -->
```jai
#test "YaRN frequencies, scales and the rotation, by hand" {
    // Width 64: the correction dimensions are 64 ln(8192 / (32 * 2 pi)) /
    // (2 ln 10000) = 12.88, so lo = 12, and 64 ln(8192 / 2 pi) / (2 ln
    // 10000) = 24.92, so hi = 25. Pairs below 12 keep theta^(-2j/64); pairs
    // from 25 on turn 128 times slower; pair 18 is mixed 6/13 of the way.
    #assert abs(m4_correction(64,32)-12.8805)<0.0001 && abs(m4_correction(64,1)-24.9217)<0.0001;
    #assert m4_inv_freq(64,0)==1 && m4_inv_freq(64,11)==m4o_base(11) && m4_inv_freq(64,12)==m4o_base(12);
    #assert m4_inv_freq(64,25)==m4o_base(25)/128 && m4_inv_freq(64,31)==m4o_base(31)/128;
    #assert abs(m4_inv_freq(64,18)-(m4o_base(18)/128*6.0/13+m4o_base(18)*7.0/13))<1e-18;
    for d:1..<65 {
        lo:=floor(m4_correction(2*d,32));hi:=-floor(-m4_correction(2*d,1));
        #assert lo>=0 && lo<hi && hi<=f64(2*d-1);
        for j:0..<d {#assert abs(m4_inv_freq(2*d,j)-m4o_inv_freq(2*d,j))<=1e-15*m4o_inv_freq(2*d,j);}
    }
    // The softmax scale (0.1 ln 128 + 1)^2 / sqrt(128) = 0.194970; Llama 4's
    // factor is 1 below position 8192, then 1 + 0.1 ln 2 = 1.069315.
    #assert abs(m4_softmax_scale(m4_small4())-0.194970)<0.000001;
    #assert m4_position_scale(0)==1 && m4_position_scale(8191)==1 && m4_position_scale(8192)==1+0.1*log(2.0);
    #assert m4_position_scale(16383)==1+0.1*log(2.0) && m4_position_scale(16384)==1+0.1*log(3.0) && abs(m4_position_scale(8192)-1.069315)<0.000001;
    // One pair at position 1 with frequency 1 turns by one radian.
    x:=[2]f32.[1,0];out:[2]f32;#assert m4_rotate(x[..],1,out[..]) && out[0]==f32(cos(1.0)) && out[1]==f32(sin(1.0));
    y:=[4]f32.[1,2,3,4];o4:[4]f32;#assert m4_rotate(y[..],0,o4[..]) && o4[0]==1 && o4[1]==3 && o4[2]==2 && o4[3]==4;
    big:=[2]f32.[3e38,3e38];#assert !m4_rotate(big[..],1,out[..]);
    neg:=[2]f32.[3e38,-3e38];#assert !m4_rotate(neg[..],1,out[..]);
    // A rotated query against a rotated key depends only on their distance.
    q:=[4]f32.[0.75,-0.5,0.25,1];k:=[4]f32.[0.5,0.25,-1,0.5];a:[4]f32;b:[4]f32;
    #assert abs(m4o_dot(7,3,q[..],k[..],a[..],b[..])-m4o_dot(1007,1003,q[..],k[..],a[..],b[..]))<0.000001;
    #assert abs(m4o_dot(7,3,q[..],k[..],a[..],b[..])-m4o_dot(7,4,q[..],k[..],a[..],b[..]))>0.01;
}
```

## The latent-attention layer

The query goes down to rank 1,024, is normed and goes up to 32 heads of
128. The key-value projection goes down to a latent of 256 plus a rotary key
of 64 shared by every head. The cache keeps only that: the normed latent and
the rotated key, 320 values a position. The core is chapter 84's absorbed
form: each head's non-rotary query is moved into latent space through its
rows of `kv_b_proj` (W_UK), scored against every cached latent, and the
weighted sum of latents goes through the head's value rows (W_UV) once.

<!-- source: psionic/lib/mistral-small.coax attention -->
```jai
// One attention layer's weights: the block norm (input_layernorm), the
// query's down projection, its norm and up projection, the key-value down
// projection (latent then rotary key), the latent's norm, the latent's up
// projection to each head's key and value (kv_b), and the output projection.
M4Attn :: struct { norm:[]f32; q_a:M4Matrix; q_norm:[]f32; q_b:M4Matrix; kv_a:M4Matrix; kv_norm:[]f32; kv_b:M4Matrix; o:M4Matrix; }
// A cache row a position: the normed latent then the rotated rotary key.
m4_row :: (s:M4Shape)->int {return s.kv_rank+s.rope;}
// Workspace, in order: kv, normed, qa, q, latent, attended, out; each
// region is in use while its neighbour is written. at[i] is region i's
// first value; at[7] the whole.
m4_layout :: (s:M4Shape)->[8]int {
    at:[8]int;
    at[1]=m4_row(s);at[2]=at[1]+s.hidden;at[3]=at[2]+s.q_rank;at[4]=at[3]+s.heads*(s.nope+s.rope);
    at[5]=at[4]+s.kv_rank;at[6]=at[5]+s.heads*s.value;at[7]=at[6]+s.hidden;
    return at;
}
m4_attention_work :: (s:M4Shape)->int {return m4_layout(s)[7];}
m4_attn_shape :: (s:M4Shape,w:M4Attn)->bool {
    h:=s.hidden;
    if !m4_valid(s) || w.norm.count!=h || w.q_norm.count!=s.q_rank || w.kv_norm.count!=s.kv_rank {return false;}
    if !m4_matrix_ok(w.q_a,s.q_rank,h) || !m4_matrix_ok(w.q_b,s.heads*(s.nope+s.rope),s.q_rank) || !m4_matrix_ok(w.kv_a,m4_row(s),h) {return false;}
    return m4_matrix_ok(w.kv_b,s.heads*(s.nope+s.value),s.kv_rank) && m4_matrix_ok(w.o,h,s.heads*s.value);
}
// The latent core for one position, absorbed: each head's non-rotary query
// moved into latent space through its W_UK rows, scores against every cached
// row 0 .. position (latent part through that query, rotary part directly)
// times scale, a softmax, the weighted sum of latents, and the head's W_UV
// rows applied after the sum. latent (kv_rank values) holds the moved query,
// then the sum. out holds heads x value.
m4_latent :: (s:M4Shape,kv_b:M4Matrix,q:[]f32,cache:[]f32,position:int,scale:f64,latent:[]f32,out:[]f32,scores:[]f64)->bool {
    L:=latent.count;row:=m4_row(s);qw:=s.nope+s.rope;
    for head:0..<s.heads {
        for j:0..<L {
            acc:PsionicAccumulator;
            for i:0..<s.nope {acc=psionic_ops_accumulate(acc,f64(q[head*qw+i])*fp8_unit(kv_b.codes[(head*(s.nope+s.value)+i)*L+j]));}
            v:=psionic_ops_total(acc)*f64(kv_b.scale);
            if !psionic_ops_fits_f32(v) {return false;}
            latent[j]=f32(v);
        }
        largest:=0.0;
        for t:0..<position+1 {
            acc:PsionicAccumulator;
            for j:0..<L {acc=psionic_ops_accumulate(acc,f64(latent[j])*f64(cache[t*row+j]));}
            for j:0..<s.rope {acc=psionic_ops_accumulate(acc,f64(q[head*qw+s.nope+j])*f64(cache[t*row+L+j]));}
            scores[t]=psionic_ops_total(acc)*scale;
            largest=ifx t==0 then scores[t] else m4_larger(largest,scores[t]);
        }
        den:PsionicAccumulator;
        for t:0..<position+1 {scores[t]=exp(scores[t]-largest);den=psionic_ops_accumulate(den,scores[t]);}
        total:=psionic_ops_total(den);
        for j:0..<L {
            acc:PsionicAccumulator;
            for t:0..<position+1 {acc=psionic_ops_accumulate(acc,scores[t]*f64(cache[t*row+j]));}
            latent[j]=f32(psionic_ops_total(acc)/total);   // a mean of f32 values
        }
        for v:0..<s.value {
            acc:PsionicAccumulator;
            for j:0..<L {acc=psionic_ops_accumulate(acc,fp8_unit(kv_b.codes[(head*(s.nope+s.value)+s.nope+v)*L+j])*f64(latent[j]));}
            x:=psionic_ops_total(acc)*f64(kv_b.scale);
            if !psionic_ops_fits_f32(x) {return false;}
            out[head*s.value+v]=f32(x);
        }
    }
    return true;
}
// tokens rows of hidden at positions start .. start + tokens - 1 through the
// layer. cache holds a row for every earlier position; the layer writes each
// token's row before it attends, so a token sees itself and everything
// before. Every count, code and cached row is checked before the first
// write; on an arithmetic refusal written counts the tokens done.
m4_attention :: (s:M4Shape,w:M4Attn,tokens:int,start:int,hidden:[]f32,cache:[]f32,work:[]f32,fwork:[]f64)->M4Result {
    if !m4_attn_shape(s,w) {return M4Result.{status=M4Status.shape};}
    h:=s.hidden;row:=m4_row(s);
    if tokens<1 || start<0 || hidden.count!=tokens*h || start>cache.count/row-tokens || work.count<m4_attention_work(s) || start>fwork.count-tokens {return M4Result.{status=M4Status.capacity};}
    for m:[5]M4Matrix.[w.q_a,w.q_b,w.kv_a,w.kv_b,w.o] {for c:m.codes {if fp8_nan(c) {return M4Result.{status=M4Status.number};}}}
    for i:0..<start*row {if (f32_bits(cache[i])&0x7fffffff)>=0x7f800000 {return M4Result.{status=M4Status.number};}}
    at:=m4_layout(s);
    kv:=work[0..<at[1]];normed:=work[at[1]..<at[2]];qa:=work[at[2]..<at[3]];q:=work[at[3]..<at[4]];
    latent:=work[at[4]..<at[5]];attended:=work[at[5]..<at[6]];out:=work[at[6]..<at[7]];
    for t:0..<tokens {
        p:=start+t;x:=hidden[t*h..<(t+1)*h];own:=cache[p*row..<(p+1)*row];
        ok:=m4_norm(x,w.norm,normed) && m4_linear(w.kv_a,normed,kv);
        ok=ok && m4_norm(kv[0..<s.kv_rank],w.kv_norm,own[0..<s.kv_rank]) && m4_rotate(kv[s.kv_rank..<row],p,own[s.kv_rank..<row]);
        ok=ok && m4_linear(w.q_a,normed,qa) && m4_norm(qa,w.q_norm,qa) && m4_linear(w.q_b,qa,q);
        for head:0..<s.heads {
            r:=q[head*(s.nope+s.rope)+s.nope..<(head+1)*(s.nope+s.rope)];
            ok=ok && m4_rotate(r,p,kv[0..<s.rope]);
            for j:0..<s.rope {r[j]=kv[j];}
        }
        ok=ok && m4_latent(s,w.kv_b,q,cache,p,m4_softmax_scale(s)*m4_position_scale(p),latent,attended,fwork);
        ok=ok && m4_linear(w.o,attended,out) && m4_residual(x,out);
        if !ok {return M4Result.{status=M4Status.arithmetic,written=t};}
    }
    return M4Result.{written=tokens};
}
```

Two independent checks hold the layer. The oracle computes it in the
expanded form: every cached latent is projected to each head's key and
value, and the rotary pairs are rotated in place in their interleaved
layout. Chapter 90's `ml_attention` checks the core: E4M3 codes times 0.5
are exact BF16 values, so it reads the same W_UK and W_UV, and the two agree
within chapter 84's tolerance, also when every score is ±1,500.

<!-- source: psionic/lib/mistral-small.coax checks-attention -->
```jai
m4_attn_view :: (f:[]f32,c:[]u8)->M4Attn {
    return M4Attn.{norm=f[0..<8],q_a=M4Matrix.{codes=c[0..<32],scale=f[15]},q_norm=f[8..<12],q_b=M4Matrix.{codes=c[32..<80],scale=f[16]},
        kv_a=M4Matrix.{codes=c[80..<136],scale=f[17]},kv_norm=f[12..<15],kv_b=M4Matrix.{codes=c[136..<166],scale=f[18]},o=M4Matrix.{codes=c[166..<214],scale=f[19]}};
}
#test "latent attention matches the expanded binary64 oracle" {
    s:=m4o_shape();
    at:=m4_layout(s);#assert at[0]==0 && at[1]==7 && at[2]==15 && at[3]==19 && at[4]==31 && at[5]==34 && at[6]==40 && at[7]==48;
    #assert m4_attention_work(s)==7+2*8+4+12+3+6;
    for seed:0..<6 {
        a:=m4o_attn_weights(seed);hidden:[48]f32;m4o_values(seed+11,hidden[..],0);h:=hidden;
        cache:[6*7]f32;work:[62]f32;fwork:[6]f64;
        r:=m4_attention(s,m4_attn_view(a.f[..],a.c[..]),6,0,h[..],cache[..],work[..],fwork[..]);
        ref:[48]f64;none:[1]f32;m4o_attention(a.f[..],a.c[..],6,0,hidden[..],none[0..<0],ref[..]);g:=m4o_gap(h[..],ref[..]);
        #assert r.status==M4Status.ok && r.written==6 && g.gap<=g.limit;
    }
}
#test "positions across 8192: the cached past, Llama 4's factor and far rotations" #steps 2000000000 {
    s:=m4o_shape();a:=m4o_attn_weights(2);hidden:[32]f32;m4o_values(5,hidden[..],0);
    cache:[8194*7]f32;m4o_past(1,cache[0..<8190*7]);past:=cache;
    h:=hidden;work:[62]f32;fwork:[8194]f64;
    r:=m4_attention(s,m4_attn_view(a.f[..],a.c[..]),4,8190,h[..],cache[..],work[..],fwork[..]);
    ref:[32]f64;m4o_attention(a.f[..],a.c[..],4,8190,hidden[..],past[0..<8190*7],ref[..]);g:=m4o_gap(h[..],ref[..]);
    #assert r.status==M4Status.ok && r.written==4 && g.gap<=g.limit;
    for i:0..<8190*7 {#assert cache[i]==past[i];}
}
#test "one token at a time equals the prompt at once" {
    s:=m4o_shape();a:=m4o_attn_weights(4);hidden:[48]f32;m4o_values(3,hidden[..],0);w:=m4_attn_view(a.f[..],a.c[..]);
    all:=hidden;c1:[42]f32;work:[62]f32;fwork:[6]f64;
    #assert m4_attention(s,w,6,0,all[..],c1[..],work[..],fwork[..]).written==6;
    each:=hidden;c2:[42]f32;
    for t:0..<6 {#assert m4_attention(s,w,1,t,each[t*8..<t*8+8],c2[0..<(t+1)*7],work[..],fwork[..]).written==1;}
    for i:0..<48 {#assert all[i]==each[i];}for i:0..<42 {#assert c1[i]==c2[i];}
}
#test "the absorbed core equals chapter 90's ml_attention when kv_b is exact in BF16" {
    // E4M3 codes times 0.5 are exact BF16 values, so ml_attention reads the
    // same W_UK and W_UV; both forms agree within chapter 84's tolerance.
    s:=m4o_shape();codes:[30]u8;m4o_codes(5,codes[..]);kv_b:=M4Matrix.{codes=codes[..],scale=0.5};
    w:[60]u8;for i:0..<30 {b:=f32_bits(f32(fp8_unit(codes[i])*0.5))>>16;w[2*i]=cast(u8)(b&255);w[2*i+1]=cast(u8)(b>>8);}
    ms:=MlShape.{heads=2,nope=2,rope=4,value=3,latent=3,capacity=8};
    for p:0..<8 {
        q:[12]f32;m4o_values(p,q[..],0);cache:[56]f32;m4o_past(p+2,cache[..]);
        latent:[3]f32;mine:[6]f32;scores:[8]f64;
        #assert m4_latent(s,kv_b,q[..],cache[..],p,0.375,latent[..],mine[..],scores[..]);
        theirs:[6]f32;row:[7]f32;mw:[64]f32;msc:[16]f64;
        r:=ml_attention(ms,MlMode.compressed,w[..],q[..],cache[p*7..<p*7+3],cache[p*7+3..<p*7+7],cache[0..<p*7],p,0.375,theirs[..],row[..],mw[..],msc[..],false);
        #assert r.status==DsStatus.ok && la_gap(mine[..],theirs[..]).worst<=LA_TOLERANCE;
        // Scores of +-1500 a position: the softmax shifts by the largest,
        // so neither exp overflows nor every term underflows.
        for sign:0..<2 {
            big:=q;for h:0..<2 {for i:0..<2 {big[h*6+i]=0;}for i:0..<4 {big[h*6+2+i]=ifx sign==0 then 1000 else -1000;}}
            same:=cache;for t:0..<p+1 {for i:0..<4 {same[t*7+3+i]=1;}}
            #assert m4_latent(s,kv_b,big[..],same[..],p,0.375,latent[..],mine[..],scores[..]);
            r=ml_attention(ms,MlMode.compressed,w[..],big[..],same[p*7..<p*7+3],same[p*7+3..<p*7+7],same[0..<p*7],p,0.375,theirs[..],row[..],mw[..],msc[..],false);
            #assert r.status==DsStatus.ok && la_gap(mine[..],theirs[..]).worst<=LA_TOLERANCE;
        }
    }
    // W_UK rows of 0 and W_UV rows of 448 at scale 1e38: the moved query
    // is 0, the value product overflows.
    big:[30]u8;for h:0..<2 {for v:0..<3 {for j:0..<3 {big[(h*5+2+v)*3+j]=0x7e;}}}
    q:[12]f32;m4o_values(1,q[..],0);cache:[7]f32;m4o_past(2,cache[..]);for j:0..<3 {cache[j]=1;}latent:[3]f32;mine:[6]f32;scores:[1]f64;
    #assert !m4_latent(s,M4Matrix.{codes=big[..],scale=1e38},q[..],cache[..],0,0.375,latent[..],mine[..],scores[..]);
    #assert m4_latent(s,M4Matrix.{codes=big[..],scale=1e30},q[..],cache[..],0,0.375,latent[..],mine[..],scores[..]) && abs(f64(mine[0])-1.344e33)<1e27;
}
```

## The memory plan on this machine

The cache is small. A position costs 36 layers × 320 values × 4 bytes =
46,080 bytes in f32, against 36 × 32 × 256 × 4 = 1,179,648 for full keys and
values: 25.6 times less. At 4,096 positions that is 188,743,680 bytes. The
weights are not small: 120,065,825,920 bytes, and the plan must hold every
expert, not only the four a token uses. Chapter 33's `rp_admit` decides:

- In RAM (134,828,441,600 bytes by `free -b`), the weights, the 4,096-token
  cache and the scratch need 120,277,720,192 bytes. Resident, with
  14,550,721,408 bytes to spare. Against the 127,495,778,304 bytes the
  system reported available when measured, 7,218,058,112 to spare.
- On the RTX 4080 (17,171,480,576 bytes), resident is refused, short by
  103,106,239,616 bytes. With streaming, 14,812,102,656 bytes stay on the
  card and the rest pass through two slots of 1,073,741,824 bytes (the
  readout).
- At the full context of 1,048,576 tokens, the cache alone is 48.3 GB, and
  RAM refuses the model.
- The shards need 120.9 GB of disk; this disk had 38.7 GB free. The plan
  holds in RAM, but the files cannot be stored here as the disk stands.

<!-- source: psionic/lib/mistral-small.coax plan -->
```jai
// Chapter 33's accounting from the shape, bytes as stored: FP8 matrices, the
// BF16 router, embedding, readout, norms and scales. vectors: every norm and
// scale. slot: the largest matrix that may stream, the readout. expert: one
// routed expert's three matrices.
m4_weights :: (s:M4Shape)->RpWeights {
    h:=s.hidden;E:=s.experts;I:=s.expert_mlp;S:=s.shared_mlp;
    attention:=s.q_rank*h+s.heads*(s.nope+s.rope)*s.q_rank+m4_row(s)*h+s.heads*(s.nope+s.value)*s.kv_rank+h*s.heads*s.value;
    expert:=3*I*h;
    // FP8 matrices one byte a value; the router two.
    layer:=attention+E*expert+3*S*h+2*E*h;
    // Norms (two block norms, the query and latent norms), four BF16 scale
    // values an expert, two for each of the eight other FP8 matrices.
    vectors:=s.layers*2*(2*h+s.q_rank+s.kv_rank+4*E+16)+2*h;
    table:=2*s.vocab*h;
    return RpWeights.{total=s.layers*layer+vectors+2*table,vectors=vectors,slot=table,embedding=table,expert=expert};
}
// f32 state for capacity tokens: each layer's latent cache rows.
m4_state :: (s:M4Shape,capacity:int)->int {return 4*s.layers*capacity*m4_row(s);}
// f32 scratch for chunk tokens: the residual and the widest step (the
// attention work or the mixture's), one score row, and the logits.
m4_scratch :: (s:M4Shape,chunk:int,capacity:int)->int {
    return 4*(chunk*(s.hidden+rp_max(m4_attention_work(s),m4_moe_work(s)+s.experts))+capacity+s.vocab);
}
m4_plan :: (s:M4Shape,r:RpRequest,budget:int)->RpPlan {
    if !m4_valid(s) {return RpPlan.{status=RpStatus.model};}
    if r.capacity<1 || r.capacity>M4_TOKENS || r.chunk<1 || r.chunk>r.capacity {return RpPlan.{status=RpStatus.request};}
    return rp_admit(m4_weights(s),m4_state(s,r.capacity),m4_scratch(s,r.chunk,r.capacity),r.stream,budget);
}
// Values a decode token multiplies: every matrix but the embedding table
// (one row of it is read), with top_k of the experts in each layer.
m4_active :: (s:M4Shape)->int {
    w:=m4_weights(s);unused:=s.layers*(s.experts-s.top_k)*w.expert;
    // The router and the readout are BF16: count their values once.
    return w.total-w.vectors-w.embedding-unused-s.layers*s.experts*s.hidden-s.vocab*s.hidden;
}
// This host, measured on 2026-11-02: nvidia-smi's memory.total for the RTX
// 4080 (16,376 MiB), free -b's total and available memory, and df's free
// bytes on the disk that would hold the checkpoint.
M4_HOST_GPU :: 17171480576;
M4_HOST_RAM :: 134828441600;
M4_HOST_AVAILABLE :: 127495778304;
M4_HOST_DISK :: 38661877760;
```

<!-- source: psionic/lib/mistral-small.coax checks-plan -->
```jai
// The weight bytes equal the header survey's text bytes, 728,192 of them
// norms and scales.
#test "the plan counts every expert, holds the model in RAM, and streams on the GPU" {
    s:=m4_small4();w:=m4_weights(s);
    #assert w.total==120065825920 && w.vectors==728192 && w.slot==1073741824 && w.embedding==1073741824 && w.expert==25165824;
    #assert 36*128*w.expert==115964116992;
    // Active a token: 6,095,372,288 values of 118,972,845,632.
    #assert m4_active(s)==6095372288;
    // 46,080 bytes of latent cache a token: 36 layers x (256 + 64) x 4.
    #assert m4_state(s,1)==46080 && m4_state(s,4096)==188743680;
    c:=la_cache(MlShape.{heads=32,nope=64,rope=64,value=128,latent=256,capacity=163840},36,4096,4);
    #assert c.status==DsStatus.ok && c.latent_bytes==m4_state(s,4096) && c.full_bytes==36*4096*4*32*256;
    #assert m4_scratch(s,256,4096)==4*(256*(4096+rp_max(m4_attention_work(s),m4_moe_work(s)+128))+4096+131072);
    #assert m4_attention_work(s)==320+2*4096+1024+4096+256+4096 && m4_moe_work(s)+128==2*4096+4096+128;
    big:=s;big.expert_mlp=8192;#assert m4_scratch(big,1,1)==4*(4096+2*4096+2*8192+128+1+131072);
    r:=RpRequest.{capacity=4096,chunk=256,stream=false};live:=m4_state(s,4096)+m4_scratch(s,256,4096);
    ram:=m4_plan(s,r,M4_HOST_RAM);#assert ram.status==RpStatus.ok && ram.mode==RpMode.resident && ram.resident==w.total && ram.need==w.total+live;
    avail:=m4_plan(s,r,M4_HOST_AVAILABLE);#assert avail.mode==RpMode.resident && M4_HOST_AVAILABLE-avail.need==7218058112;
    gpu:=m4_plan(s,r,M4_HOST_GPU);#assert gpu.status==RpStatus.weights && gpu.mode==RpMode.refused && gpu.need-M4_HOST_GPU==103106239616;
    r.stream=true;gpu=m4_plan(s,r,M4_HOST_GPU);
    #assert gpu.status==RpStatus.ok && gpu.mode==RpMode.streamed && gpu.resident==M4_HOST_GPU-live-2*1073741824 && gpu.streamed==w.total-gpu.resident;
    // The whole context, 1,048,576 tokens: 48.3 GB of latent cache; RAM
    // cannot hold it beside the weights.
    r=RpRequest.{capacity=M4_TOKENS,chunk=256};full:=m4_plan(s,r,M4_HOST_RAM);
    #assert full.status==RpStatus.weights && m4_state(s,M4_TOKENS)==48318382080;
    // The checkpoint's three shards need 120.9 GB of disk; this disk had 38.7 GB free.
    #assert MS_PINNED_BYTES-M4_HOST_DISK==82261096176;
}
```

## The run

The example prints the inventory, the hand route, the rotary constants, a
small model of one attention layer and one mixture layer against the
oracle, and the plan.

<!-- source: psionic/examples/mistral-small.coax example -->
```jai
main :: () {
    print("profile mistral-small-cpu-f64-v1, tolerance % x (1 + largest |reference value|), near-tie margin %\n",M4_TOLERANCE,M4_MARGIN);
    s:=m4_small4();
    print("pinned revision %, % shard bytes, header only\n",MS_PINNED_REVISION,MS_PINNED_BYTES);
    r:=m4_inventory(s,MS_PINNED_MANIFEST);
    print("text inventory: % lines, % expected, status %, % values in % bytes\n",r.lines,M4_EXPECTED,r.status,r.parameters,r.bytes);
    print("  plus % vision and projector tensors in % bytes and % header bytes: % tensors, % bytes\n",
        MS_PINNED_OTHER_TENSORS,MS_PINNED_OTHER_BYTES,MS_PINNED_HEADER_BYTES,MS_PINNED_TENSORS,r.bytes+MS_PINNED_OTHER_BYTES+MS_PINNED_HEADER_BYTES);
    buf:[2048]u8;n:=m4_config_text(s,buf[..]);
    print("config fields match the pinned config.json: %\n",cast(string)buf[0..<n]==MS_PINNED_CONFIG);

    // The hand trace: four experts, logits 1, 3, 2, 0, two chosen.
    gate:=[4]f32.[1,3,2,0];x:=[1]f32.[1];fwork:[4]f64;
    route:=m4_route(gate[..],x[..],2,fwork[..]);
    print("hand route: experts % and %, weights % and %, margin %\n",route.ids[0],route.ids[1],route.weights[0],route.weights[1],route.margin);
    print("rope: lo % hi %, softmax scale %, position factor at 8192 %\n",
        floor(m4_correction(64,32)),-floor(-m4_correction(64,1)),m4_softmax_scale(s),m4_position_scale(8192));

    // The small model: one attention layer and one mixture layer, six tokens,
    // against the binary64 oracles.
    t:=m4o_shape();
    print("small model: hidden 8, 2 heads, query rank 4, latent 3, rotary 4; 5 experts of width 3, 2 chosen, 1 shared\n");
    for seed:0..<3 {
        a:=m4o_attn_weights(seed);m:=m4o_moe_weights(seed);hidden:[48]f32;m4o_values(seed+11,hidden[..],0);
        h:=hidden;cache:[42]f32;work:[62]f32;fw:[13]f64;routes:[6]M4Route;
        r0:=m4_attention(t,m4_attn_view(a.f[..],a.c[..]),6,0,h[..],cache[..],work[..],fw[0..<6]);
        r1:=m4_moe(t,m4_moe_view(m.f[..],m.c[..]),6,h[..],routes[..],work[0..<22],fw[..]);
        mid:[48]f64;none:[1]f32;m4o_attention(a.f[..],a.c[..],6,0,hidden[..],none[0..<0],mid[..]);
        o:[48]f32;for i:0..<48 {o[i]=f32(mid[i]);}
        last:[48]f64;for k:0..<6 {m4o_moe(m.f[..],m.c[..],o[k*8..<k*8+8],last[k*8..<k*8+8]);}
        g:=m4o_gap(h[..],last[..]);smallest:=1.0;ids:[6]int;
        for k:0..<6 {if routes[k].margin<smallest {smallest=routes[k].margin;}ids[k]=routes[k].ids[0];}
        print("  seed %: status % %, largest difference % (limit %), first experts %, smallest router margin %\n",
            seed,r0.status,r1.status,g.gap,g.limit,ids,smallest);
    }

    // The memory plan on this host, for 4,096 tokens in chunks of 256.
    w:=m4_weights(s);
    print("weights % bytes (% vectors); one expert %; % values active a token\n",w.total,w.vectors,w.expert,m4_active(s));
    print("latent cache % bytes a token (% with full keys and values in f32)\n",m4_state(s,1),4*s.layers*s.heads*(s.nope+s.rope+s.value));
    q:=RpRequest.{capacity=4096,chunk=256,stream=false};
    ram:=m4_plan(s,q,M4_HOST_RAM);
    print("RAM % bytes: % %, % bytes to spare\n",M4_HOST_RAM,ram.status,rp_mode_name(ram.mode),M4_HOST_RAM-ram.need);
    av:=m4_plan(s,q,M4_HOST_AVAILABLE);
    print("RAM available when measured % bytes: % %, % bytes to spare\n",M4_HOST_AVAILABLE,av.status,rp_mode_name(av.mode),M4_HOST_AVAILABLE-av.need);
    gpu:=m4_plan(s,q,M4_HOST_GPU);
    print("GPU % bytes, resident: %, need %, short by %\n",M4_HOST_GPU,rp_status_name(gpu.status),gpu.need,gpu.need-M4_HOST_GPU);
    q.stream=true;gpu=m4_plan(s,q,M4_HOST_GPU);
    print("GPU streaming: % %, % bytes resident, % streamed through two slots of %\n",gpu.status,rp_mode_name(gpu.mode),gpu.resident,gpu.streamed,gpu.slot);
    q=RpRequest.{capacity=M4_TOKENS,chunk=256};full:=m4_plan(s,q,M4_HOST_RAM);
    print("RAM at % tokens: %, need %\n",M4_TOKENS,rp_status_name(full.status),full.need);
    print("disk: % bytes free, the shards need %, short by %\n",M4_HOST_DISK,MS_PINNED_BYTES,MS_PINNED_BYTES-M4_HOST_DISK);
}
```

```text
profile mistral-small-cpu-f64-v1, tolerance 0.00001 x (1 + largest |reference value|), near-tie margin 0.00001
pinned revision a11f36bebf709121056b1dbcc943d1c6afbe494d, 120922973936 shard bytes, header only
text inventory: 38 lines, 38 expected, status ok, 118972845632 values in 120065825920 bytes
  plus 222 vision and projector tensors in 856944640 bytes and 203376 header bytes: 1485 tensors, 120922973936 bytes
config fields match the pinned config.json: true
hand route: experts 1 and 2, weights 0.7310586 and 0.26894143, margin 0.14973849934787756
rope: lo 12.0 hi 25.0, softmax scale 0.19496949473302966, position factor at 8192 1.0693147180559945
small model: hidden 8, 2 heads, query rank 4, latent 3, rotary 4; 5 experts of width 3, 2 chosen, 1 shared
  seed 0: status ok ok, largest difference 3.368912642187638e-07 (limit 0.00004743635504603966), first experts [4, 1, 2, 3, 4, 4], smallest router margin 0.018497420130014736
  seed 1: status ok ok, largest difference 1.355105534450729e-07 (limit 0.0000316393179425114), first experts [4, 0, 4, 3, 2, 3], smallest router margin 0.027626292853483045
  seed 2: status ok ok, largest difference 1.841326854878922e-07 (limit 0.00004683470737162018), first experts [0, 1, 4, 1, 4, 1], smallest router margin 0.04865610695839964
weights 120065825920 bytes (728192 vectors); one expert 25165824; 6095372288 values active a token
latent cache 46080 bytes a token (1179648 with full keys and values in f32)
RAM 134828441600 bytes: ok resident, 14550721408 bytes to spare
RAM available when measured 127495778304 bytes: ok resident, 7218058112 bytes to spare
GPU 17171480576 bytes, resident: weights: the weights, state and scratch exceed the budget and streaming is off, need 120277720192, short by 103106239616
GPU streaming: ok streamed, 14812102656 bytes resident, 105253723264 streamed through two slots of 1073741824
RAM at 1048576 tokens: weights: the weights, state and scratch exceed the budget and streaming is off, need 168411536512
disk: 38661877760 bytes free, the shards need 120922973936, short by 82261096176
```

The small model has hidden width 8, 2 heads, a query rank of 4, a latent of
3 and a rotary width of 4; its mixture has 5 experts of width 3, two chosen,
and one shared expert. Every FP8 code and scale is generated, not taken from
the checkpoint. The largest difference from the oracle is about 3 × 10⁻⁷,
well inside the frozen tolerance of 10⁻⁵ × (1 + the largest reference
value).

## Refusals

A shape that says 64 routed experts no longer matches the pinned headers:
the inventory names the first expected line the headers do not hold. An FP8
code with the NaN pattern (0x7f or 0xff) in any attention matrix is refused
before the layer writes anything.

<!-- source: psionic/examples/mistral-small-refuse.coax example -->
```jai
// A shape that says 64 routed experts instead of 128: the first expert line
// no longer matches the pinned headers. The refusal names the first
// expected tensor the headers do not hold. Then an FP8 code with the NaN
// pattern in the small model's output projection: the attention layer
// refuses before it writes anything.
main :: () {
    s:=m4_small4();s.experts=64;
    r:=m4_inventory(s,MS_PINNED_MANIFEST);
    buf:[160]u8;n:=m4_line(s,r.bad,buf[..]);
    print("refused: status %, tensor % not in the pinned headers: %\n",r.status,r.bad,cast(string)buf[0..<n]);
    t:=m4o_shape();a:=m4o_attn_weights(0);a.c[200]=0x7f;
    hidden:[8]f32;m4o_values(1,hidden[..],0);h:=hidden;cache:[7]f32;work:[62]f32;fw:[1]f64;
    q:=m4_attention(t,m4_attn_view(a.f[..],a.c[..]),1,0,h[..],cache[..],work[..],fw[..]);
    print("refused: status %, % tokens written, hidden unchanged %\n",q.status,q.written,h[0]==hidden[0] && h[7]==hidden[7]);
}
```

```text
refused: status checkpoint, tensor 3 not in the pinned headers: language_model.model.layers.0..35.mlp.experts.down_proj F8_E4M3 64,4096,2048
refused: status number, 0 tokens written, hidden unchanged true
```

The library also refuses wrong weight counts, scales that are zero,
negative or not finite, a gate that is not finite, a cache row that is not
finite, too little room, and arithmetic that leaves f32; the checks cover
each.

## What this does not do yet

- **Original weights.** No shard of 5 GB or less holds a complete unit.
  The Hugging Face layout's shards are 49.1, 49.1 and 22.7 GB; the Mistral
  layout's smallest shard (1.07 GB) holds only `output.weight`, the readout.
  No layer, router or expert has run on the published values.
- **References.** Router choices, intermediates and logits have not been
  compared with a reference run of the published model for any layer or
  token.
- **The whole model.** The 36 layers, the embedding, the final norm, the
  readout, the Tekken tokenizer and the chat format have not been joined;
  no token has been generated.
- **Measured residency.** The plan is computed from the pinned shapes and
  this host's measured memory; no resident run has been measured, and the
  checkpoint does not fit on this host's free disk.
- **Static FP8 activations.** The checkpoint's `activation_scale` values
  are recorded, not applied.
- **The vision tower** and its projector.

