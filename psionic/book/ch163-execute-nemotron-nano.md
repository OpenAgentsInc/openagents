# 163. Execute Nemotron Nano

> **Status: partly implemented.** The checkpoint inventory, the tokenizer on
> ASCII text, all three layer kinds and the whole model with greedy decoding
> from its cache run, and one whole-model run on the original weights is
> recorded. Text outside ASCII and device kernels do not yet; see
> [What this does not do yet](#what-this-does-not-do-yet).

NVIDIA-Nemotron-3-Nano-4B-BF16 is a hybrid language model. Of its 42 layers,
21 are Mamba-2 state-space layers, 4 are attention layers and 17 are plain
MLP layers. This chapter checks that a pinned download of the checkpoint holds
exactly the tensors its config describes, builds each of the three layer
kinds in Coax on top of [chapter 162](ch162-a-recurrence-you-can-inspect.md)'s
recurrence, and runs one layer of each kind on the original weights against
an independent oracle. It then reads the pinned tokenizer, puts the 42 layers
together with greedy decoding from a cache, and records one whole-model run
on the original weights against an independent binary64 reference.

```shell
coax/bin/coax run psionic/examples/nemotron-h.coax
coax/bin/coax test psionic/lib/nemotron-h.coax
coax/bin/coax test psionic/lib/nemotron-model.coax
coax/bin/coax test psionic/lib/nemotron-tokens.coax
psionic/tests/check_nemotron_slices.sh ~/models/nemotron-3-nano-4b-bf16-dfaf35de
psionic/tests/check_nemotron_run.sh ~/models/nemotron-3-nano-4b-bf16-dfaf35de
```

The [library](../lib/nemotron-h.coax), the [whole model](../lib/nemotron-model.coax), the [tokenizer](../lib/nemotron-tokens.coax), its [check oracle](../tests/nemotron-h-oracle.coax),
the [example](../examples/nemotron-h.coax), the
[refusal example](../examples/nemotron-h-refuse.coax) and the
[slice check](../tests/check_nemotron_slices.sh) are the implementation.
The example needs no weights: the checkpoint's header is committed as a
fixture, and the layers run on a small model with the same structure. The
slice check needs the download (7.9 GB) and takes about 80 seconds, most of it
the Python oracle.

## The pinned checkpoint

The weights come from Hugging Face at revision
`dfaf35de3e30f1867dd8dbc38a7fc9fb52d3914f` of
`nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16`, under the NVIDIA Nemotron Open Model
License. One file, `model.safetensors`, holds 7,947,142,640 bytes with SHA-256
`55d4e251…0dad42`. The repository also ships Python model code; it was read
as the architecture's description and never run.

`psionic/tests/nemotron_manifest.py` hashes `config.json` and
`model.safetensors` against their pins, reads only the safetensors header,
and writes `psionic/fixtures/nemotron-nano-manifest.coax`: one line per
tensor (name, dtype, dimensions) and a copy of the config fields this chapter
builds its shape from. No weights are committed.

The config gives the shape. `hybrid_override_pattern` names each layer's
kind, one character a layer: `M` is Mamba-2, `*` is attention and `-` is an
MLP.

<!-- source: psionic/lib/nemotron-h.coax shape -->
```jai
// The model's shape. ssm is the Mamba-2 mixer (chapter 162's shape); the
// attention layers have attn_heads query heads sharing kv_heads key and
// value heads, each attn_dim wide, and no position encoding.
NhShape :: struct { hidden:int; vocab:int; layers:int; ssm:SsShape; attn_heads:int; kv_heads:int; attn_dim:int; mlp:int; }
// arithmetic: a value that is not finite or does not fit f32 (a hidden
// value that is not finite fails the first norm). recurrence: chapter 162
// refused; ssm holds its status. token: a token id outside the vocabulary.
NhStatus :: enum u8 { ok; shape; pattern; checkpoint; capacity; arithmetic; recurrence; token; }
NhResult :: struct { status:NhStatus; written:int; ssm:SsStatus; }
// The pinned config: hidden_size 3136, mamba_num_heads 96, mamba_head_dim
// 80, ssm_state_size 128, n_groups 8, num_attention_heads 40,
// num_key_value_heads 8, head_dim 128, intermediate_size 12544.
nh_nano :: ()->NhShape {
    return NhShape.{hidden=3136,vocab=131072,layers=42,ssm=SsShape.{heads=96,head_dim=80,state_dim=128,groups=8},attn_heads=40,kv_heads=8,attn_dim=128,mlp=12544};
}
NH_NANO_PATTERN :: "M-M-M-MM-M-M*-M-M*-M-M-M*-M-M-MM*-MMM-M-M-";
// Both norms use epsilon 1e-5 (layer_norm_epsilon and rms_norm_eps).
NH_EPSILON :: 0.00001;
// Layer kinds, one character each: M Mamba-2, * attention, - MLP.
NH_MAMBA :: 0; NH_ATTENTION :: 1; NH_MLP :: 2;
nh_kind :: (pattern:string,layer:int)->int {
    c:=pattern[layer];
    if c=='M' {return NH_MAMBA;}
    if c=='*' {return NH_ATTENTION;}
    if c=='-' {return NH_MLP;}
    return -1;
}
// counts: layers of each kind; bad: the first unknown character, or -1.
NhPattern :: struct { status:NhStatus; counts:[3]int; bad:int; }
nh_pattern :: (s:NhShape,pattern:string)->NhPattern {
    r:NhPattern;r.bad=-1;
    if pattern.count!=s.layers {r.status=NhStatus.pattern;return r;}
    for i:0..<s.layers {
        k:=nh_kind(pattern,i);
        if k<0 {r.status=NhStatus.pattern;r.bad=i;return r;}
        r.counts[k]+=1;
    }
    return r;
}
```

From the shape and the pattern, the chapter writes the inventory it expects,
in the checkpoint's own names:

<!-- source: psionic/lib/nemotron-h.coax checkpoint -->
```jai
// The tensor at position i of the expected inventory: the embedding, the
// final norm and the readout, then each layer's tensors in name order.
NhTensor :: struct { layer:int; suffix:string; rank:int; dims:[3]int; }
nh_d1 :: (layer:int,suffix:string,a:int)->NhTensor {t:=NhTensor.{layer=layer,suffix=suffix,rank=1};t.dims[0]=a;return t;}
nh_d2 :: (layer:int,suffix:string,a:int,b:int)->NhTensor {t:=nh_d1(layer,suffix,a);t.rank=2;t.dims[1]=b;return t;}
NH_MAMBA_NAMES :: [9]string.[".mixer.A_log",".mixer.D",".mixer.conv1d.bias",".mixer.conv1d.weight",".mixer.dt_bias",
    ".mixer.in_proj.weight",".mixer.norm.weight",".mixer.out_proj.weight",".norm.weight"];
NH_ATTENTION_NAMES :: [5]string.[".mixer.k_proj.weight",".mixer.o_proj.weight",".mixer.q_proj.weight",".mixer.v_proj.weight",".norm.weight"];
NH_MLP_NAMES :: [3]string.[".mixer.down_proj.weight",".mixer.up_proj.weight",".norm.weight"];
NH_PER_KIND :: [3]int.[9,5,3];
nh_layer_tensor :: (s:NhShape,kind:int,layer:int,k:int)->NhTensor {
    m:=s.ssm;v:=m.heads*m.head_dim;c:=ss_channels(m);q:=s.attn_heads*s.attn_dim;kv:=s.kv_heads*s.attn_dim;h:=s.hidden;
    if kind==NH_MAMBA {
        name:=NH_MAMBA_NAMES[k];
        if k==2 {return nh_d1(layer,name,c);}
        if k==3 {t:=nh_d2(layer,name,c,1);t.rank=3;t.dims[2]=4;return t;}
        if k==5 {return nh_d2(layer,name,v+c+m.heads,h);}
        if k==6 {return nh_d1(layer,name,v);}
        if k==7 {return nh_d2(layer,name,h,v);}
        if k==8 {return nh_d1(layer,name,h);}
        return nh_d1(layer,name,m.heads);
    }
    if kind==NH_ATTENTION {
        name:=NH_ATTENTION_NAMES[k];
        if k==1 {return nh_d2(layer,name,h,q);}
        if k==2 {return nh_d2(layer,name,q,h);}
        if k==4 {return nh_d1(layer,name,h);}
        return nh_d2(layer,name,kv,h);
    }
    if k==0 {return nh_d2(layer,NH_MLP_NAMES[k],h,s.mlp);}
    if k==1 {return nh_d2(layer,NH_MLP_NAMES[k],s.mlp,h);}
    return nh_d1(layer,NH_MLP_NAMES[k],h);
}
nh_expected_count :: (s:NhShape,pattern:string)->int {
    n:=3;for i:0..<s.layers {n+=NH_PER_KIND[nh_kind(pattern,i)];}
    return n;
}
nh_expected :: (s:NhShape,pattern:string,i:int)->NhTensor {
    if i==0 {return nh_d2(-1,"backbone.embeddings.weight",s.vocab,s.hidden);}
    if i==1 {return nh_d1(-1,"backbone.norm_f.weight",s.hidden);}
    if i==2 {return nh_d2(-1,"lm_head.weight",s.vocab,s.hidden);}
    j:=i-3;layer:=0;
    while j>=NH_PER_KIND[nh_kind(pattern,layer)] {j-=NH_PER_KIND[nh_kind(pattern,layer)];layer+=1;}
    return nh_layer_tensor(s,nh_kind(pattern,layer),layer,j);
}
```

The pinned header must hold each expected line exactly once and nothing
else. A changed field, a moved layer kind, a repeated line or an extra line
is refused, and the refusal names the first expected tensor the header does
not hold:

<!-- source: psionic/lib/nemotron-h.coax inventory -->
```jai
nh_put :: (out:[]u8,n:int,s:string)->int {for i:0..<s.count {out[n+i]=s[i];}return n+s.count;}
nh_put_int :: (out:[]u8,n:int,v:int)->int {
    digits:[20]u8;k:=0;x:=v;
    while true {digits[k]=cast(u8)('0'+x%10);k+=1;x/=10;if x==0 {break;}}
    m:=n;while k>0 {k-=1;out[m]=digits[k];m+=1;}
    return m;
}
// One manifest line: the name, BF16, the dimensions joined by commas.
nh_line :: (t:NhTensor,out:[]u8)->int {
    n:=0;
    if t.layer>=0 {n=nh_put(out,n,"backbone.layers.");n=nh_put_int(out,n,t.layer);}
    n=nh_put(out,n,t.suffix);n=nh_put(out,n," BF16 ");
    for i:0..<t.rank {if i>0 {n=nh_put(out,n,",");}n=nh_put_int(out,n,t.dims[i]);}
    return n;
}
// A manifest matches when it holds every expected line exactly once and
// nothing else. bad names the first expected tensor missing or repeated,
// or is -1 when the expected lines are all there once but there are more.
// parameters counts the values. More than NH_LINES_MAX lines are refused.
NhInventory :: struct { status:NhStatus; lines:int; parameters:int; bad:int; }
NH_LINES_MAX :: 512;
nh_inventory :: (s:NhShape,pattern:string,manifest:string)->NhInventory {
    p:=nh_pattern(s,pattern);
    if p.status!=NhStatus.ok {return NhInventory.{status=p.status,bad=p.bad};}
    ends:[513]int;lines:=0;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==NH_LINES_MAX {return NhInventory.{status=NhStatus.checkpoint,lines=lines+1,bad=-1};}
        lines+=1;ends[lines]=i;
    }}
    r:NhInventory;r.lines=lines;r.bad=-1;buf:[96]u8;
    for e:0..<nh_expected_count(s,pattern) {
        t:=nh_expected(s,pattern,e);n:=nh_line(t,buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {return NhInventory.{status=NhStatus.checkpoint,lines=lines,bad=e};}
        size:=1;for k:0..<t.rank {size*=t.dims[k];}r.parameters+=size;
    }
    if lines!=nh_expected_count(s,pattern) {return NhInventory.{status=NhStatus.checkpoint,lines=lines,bad=-1};}
    return r;
}
```

The check below compares the pinned header and the pinned config fields with
what the shape predicts:

<!-- source: psionic/lib/nemotron-h.coax checks-checkpoint -->
```jai
#test "the pinned checkpoint holds exactly the tensors its config names" {
    s:=nh_nano();p:=nh_pattern(s,NH_NANO_PATTERN);
    #assert p.status==NhStatus.ok && p.counts[NH_MAMBA]==21 && p.counts[NH_ATTENTION]==4 && p.counts[NH_MLP]==17 && p.bad==-1;
    #assert nh_expected_count(s,NH_NANO_PATTERN)==263 && NH_PINNED_TENSORS==263;
    r:=nh_inventory(s,NH_NANO_PATTERN,NH_PINNED_MANIFEST);
    #assert r.status==NhStatus.ok && r.lines==263 && r.bad==-1 && r.parameters==3973556832;
    buf:[1024]u8;n:=nh_config_text(s,NH_NANO_PATTERN,buf[..]);
    #assert cast(string)buf[0..<n]==NH_PINNED_CONFIG;
}
```

All 263 tensors are BF16. Together they hold 3,973,556,832 values.

## A Mamba-2 layer, by hand

Each layer starts with an RMS norm of its input and ends by adding its
result back to that input. Between them, the Mamba-2 mixer does five things
for each token:

1. `in_proj` maps the 3,136 normed values to 17,504: the gate `z` (7,680),
   the channels to convolve (9,728: `x` 7,680, then `B` and `C`, 1,024 each)
   and one raw step size per head (96).
2. A causal convolution of width 4 and `silu` run over each of the 9,728
   channels, with the last three raw inputs kept as history.
3. Chapter 162's recurrence runs the 96 heads of 80 values each, eight heads
   to a group that shares one `B` and one `C` of width 128:
   `S = exp(-exp(log A) dt) S + dt x B`, `y = C S + D x`, then `y silu(z)`.
4. The gated output is normalized in 8 groups of 960 values, each group
   divided by the root of its mean square plus 1e-5, then scaled by a weight.
5. `out_proj` maps the 7,680 values back to 3,136.

Chapter 162 left steps 1, 4 and 5 to this chapter. Step 4 is the one that is
easy to get wrong: the normalization is per group, not over all 7,680
values, and it comes after the gate.

Work one group by hand. Take a group of two gated values (3, 4) with
weights (1, 2). The mean square is (9 + 16) / 2 = 12.5, and its root (with
1e-5 added) is 3.5355. So 3 becomes 3 / 3.5355 = 0.84853 and 4 becomes
2 × 4 / 3.5355 = 2.26274. The example prints the same pair:

```text
gate norm of (3, 4), weights (1, 2): (0.8485278, 2.2627409)
```

With one group of four, (3, 4, 0, -2) would divide by the root of 29 / 4
instead; with two groups of two, (0, -2) divides by the root of 2 and becomes
(0, -1.41421). A check asserts both.

The convolution and recurrence take their input in chapter 162's order:
convolved channels first, then `z`, then `dt`. The checkpoint's `in_proj`
rows are `z` first, so the layer writes each slice of rows to its place:

<!-- source: psionic/lib/nemotron-h.coax steps -->
```jai
// out[r] = sum over k of w[r * cols + k] x[k], one compensated binary64 sum
// a row, narrowed to f32 once.
nh_linear :: (w:[]f32,cols:int,x:[]f32,out:[]f32)->bool {
    for r:0..<out.count {
        acc:PsionicAccumulator;
        for k:0..<cols {acc=psionic_ops_accumulate(acc,f64(w[r*cols+k])*f64(x[k]));}
        v:=psionic_ops_total(acc);
        if !psionic_ops_fits_f32(v) {return false;}
        out[r]=f32(v);
    }
    return true;
}
// RMS norm in groups of width: each group divided by the root of its mean
// square plus epsilon, then scaled by weight. One group is the block norm;
// the Mamba-2 mixer's gate norm uses ssm.groups groups.
nh_norm :: (x:[]f32,width:int,weight:[]f32,out:[]f32)->bool {
    for g:0..<x.count/width {
        acc:PsionicAccumulator;
        for i:0..<width {acc=psionic_ops_accumulate(acc,f64(x[g*width+i])*f64(x[g*width+i]));}
        scale:=1/sqrt(psionic_ops_total(acc)/f64(width)+NH_EPSILON);
        for i:0..<width {
            v:=f64(x[g*width+i])*scale*f64(weight[g*width+i]);
            if !psionic_ops_fits_f32(v) {return false;}
            out[g*width+i]=f32(v);
        }
    }
    return true;
}
// hidden += update, narrowed to f32.
nh_residual :: (hidden:[]f32,update:[]f32)->bool {
    for i:0..<hidden.count {
        v:=f64(hidden[i])+f64(update[i]);
        if !psionic_ops_fits_f32(v) {return false;}
        hidden[i]=f32(v);
    }
    return true;
}
```

<!-- source: psionic/lib/nemotron-h.coax mamba -->
```jai
// One Mamba-2 layer's weights, decoded to f32, in checkpoint layout.
NhMamba :: struct { norm:[]f32; in_proj:[]f32; conv:[]f32; conv_bias:[]f32; a_log:[]f32; dt_bias:[]f32; d:[]f32; gate_norm:[]f32; out_proj:[]f32; }
// f32 work: chapter 162's parameters, every token's input row and output,
// and one token's normed input, gate norm and update.
nh_mamba_work :: (s:NhShape,tokens:int)->int {
    m:=s.ssm;v:=m.heads*m.head_dim;
    return ss_param_count(m)+tokens*(ss_input_count(m)+v)+s.hidden+v+s.hidden;
}
nh_mamba_shape :: (s:NhShape,w:NhMamba)->bool {
    m:=s.ssm;v:=m.heads*m.head_dim;c:=ss_channels(m);h:=s.hidden;
    if !ss_valid(m) || h<1 {return false;}
    if w.norm.count!=h || w.in_proj.count!=(v+c+m.heads)*h || w.conv.count!=4*c || w.conv_bias.count!=c {return false;}
    return w.a_log.count==m.heads && w.dt_bias.count==m.heads && w.d.count==m.heads && w.gate_norm.count==v && w.out_proj.count==h*v;
}
// tokens rows of hidden (tokens x hidden) pass through the layer in chunks
// of chunk tokens: the block norm, in_proj, the convolution and the
// recurrence (chapter 162), the gate norm, out_proj and the residual.
// history and state carry the request between calls. The checkpoint's
// in_proj rows are the gate z, then the convolved channels x, B and C,
// then dt; chapter 162's input row puts the channels first.
nh_mamba :: (s:NhShape,w:NhMamba,tokens:int,chunk:int,hidden:[]f32,history:[]f32,state:[]f32,work:[]f32,fwork:[]f64)->NhResult {
    m:=s.ssm;v:=m.heads*m.head_dim;c:=ss_channels(m);h:=s.hidden;H:=m.heads;row:=ss_input_count(m);
    if !nh_mamba_shape(s,w) {return NhResult.{status=NhStatus.shape};}
    if tokens<1 || hidden.count!=tokens*h || work.count<nh_mamba_work(s,tokens) {return NhResult.{status=NhStatus.capacity};}
    params:=work[0..<ss_param_count(m)];at:=params.count;
    input:=work[at..<at+tokens*row];at+=input.count;
    out:=work[at..<at+tokens*v];at+=out.count;
    normed:=work[at..<at+h];gated:=work[at+h..<at+h+v];update:=work[at+h+v..<at+2*h+v];
    for i:0..<4*c {params[i]=w.conv[i];}
    for i:0..<c {params[4*c+i]=w.conv_bias[i];}
    for i:0..<H {params[5*c+i]=w.a_log[i];params[5*c+H+i]=w.dt_bias[i];params[5*c+2*H+i]=w.d[i];}
    for t:0..<tokens {
        x:=input[t*row..<(t+1)*row];
        if !nh_norm(hidden[t*h..<(t+1)*h],h,w.norm,normed) {return NhResult.{status=NhStatus.arithmetic};}
        ok:=nh_linear(w.in_proj[0..<v*h],h,normed,x[c..<c+v]);
        ok=ok && nh_linear(w.in_proj[v*h..<(v+c)*h],h,normed,x[0..<c]);
        ok=ok && nh_linear(w.in_proj[(v+c)*h..<(v+c+H)*h],h,normed,x[c+v..<row]);
        if !ok {return NhResult.{status=NhStatus.arithmetic};}
    }
    r:=ss_prefill(m,params,tokens,chunk,input,history,state,out,out[0..<0],fwork,false);
    if r.status!=SsStatus.ok {return NhResult.{status=NhStatus.recurrence,ssm=r.status};}
    for t:0..<tokens {
        ok:=nh_norm(out[t*v..<(t+1)*v],v/m.groups,w.gate_norm,gated) && nh_linear(w.out_proj,v,gated,update);
        if !ok || !nh_residual(hidden[t*h..<(t+1)*h],update) {return NhResult.{status=NhStatus.arithmetic};}
    }
    return NhResult.{written=tokens};
}
```

The convolution history (3 × 9,728 values) and the state (96 × 80 × 128 =
983,040 values) belong to one request. A caller continues a request by
passing them back; it starts a new one with chapter 162's `ss_clear`.

## Attention and MLP layers

The attention layers have 40 query heads of width 128 that share 8 key and
value heads, five queries to a key. They have no position encoding: the
config names none and the reference code applies none. Each token's key and
value go into the cache at its position, and its query attends to every
cached position up to its own. Because nothing depends on position except
the cache, the prompt at once and one token at a time from the cache give the
same bits.

<!-- source: psionic/lib/nemotron-h.coax attention -->
```jai
// One attention layer's weights: q, k, v and o projections, no biases.
NhAttention :: struct { norm:[]f32; q:[]f32; k:[]f32; v:[]f32; o:[]f32; }
nh_attention_work :: (s:NhShape)->int {return 2*s.hidden+2*s.attn_heads*s.attn_dim;}
nh_attention_shape :: (s:NhShape,w:NhAttention)->bool {
    q:=s.attn_heads*s.attn_dim;kv:=s.kv_heads*s.attn_dim;h:=s.hidden;
    if h<1 || s.attn_dim<1 || s.kv_heads<1 || s.attn_heads%s.kv_heads!=0 {return false;}
    return w.norm.count==h && w.q.count==q*h && w.k.count==kv*h && w.v.count==kv*h && w.o.count==h*q;
}
// tokens rows of hidden at positions start, start + 1, ... Each token's key
// and value go into the cache at its position; its query attends to every
// cached position up to its own. There is no position encoding, so the
// cache is the only thing the order of the tokens changes.
nh_attention :: (s:NhShape,w:NhAttention,tokens:int,start:int,hidden:[]f32,keys:[]f32,values:[]f32,work:[]f32,fwork:[]f64)->NhResult {
    if !nh_attention_shape(s,w) {return NhResult.{status=NhStatus.shape};}
    h:=s.hidden;D:=s.attn_dim;kv:=s.kv_heads*D;q:=s.attn_heads*D;share:=s.attn_heads/s.kv_heads;
    capacity:=keys.count/kv;
    if tokens<1 || start<0 || hidden.count!=tokens*h || keys.count!=capacity*kv || values.count!=keys.count || start+tokens>capacity {return NhResult.{status=NhStatus.capacity};}
    if work.count<nh_attention_work(s) || fwork.count<capacity {return NhResult.{status=NhStatus.capacity};}
    normed:=work[0..<h];query:=work[h..<h+q];mixed:=work[h+q..<h+2*q];update:=work[h+2*q..<2*h+2*q];
    for t:0..<tokens {
        p:=start+t;
        ok:=nh_norm(hidden[t*h..<(t+1)*h],h,w.norm,normed) && nh_linear(w.q,h,normed,query);
        ok=ok && nh_linear(w.k,h,normed,keys[p*kv..<(p+1)*kv]) && nh_linear(w.v,h,normed,values[p*kv..<(p+1)*kv]);
        if !ok {return NhResult.{status=NhStatus.arithmetic};}
        for head:0..<s.attn_heads {
            at:=(head/share)*D;
            for j:0..<p+1 {
                acc:PsionicAccumulator;
                for d:0..<D {acc=psionic_ops_accumulate(acc,f64(query[head*D+d])*f64(keys[j*kv+at+d]));}
                fwork[j]=psionic_ops_total(acc)/sqrt(f64(D));
            }
            // The largest score, kept by moving top up by the positive part
            // of each difference, so no exp argument is above zero by more
            // than a rounding.
            top:=fwork[0];
            for j:1..<p+1 {top+=(fwork[j]-top+abs(fwork[j]-top))/2;}
            total:PsionicAccumulator;
            for j:0..<p+1 {fwork[j]=exp(fwork[j]-top);total=psionic_ops_accumulate(total,fwork[j]);}
            for d:0..<D {
                acc:PsionicAccumulator;
                for j:0..<p+1 {acc=psionic_ops_accumulate(acc,fwork[j]*f64(values[j*kv+at+d]));}
                mixed[head*D+d]=f32(psionic_ops_total(acc)/psionic_ops_total(total));
            }
        }
        if !nh_linear(w.o,q,mixed,update) || !nh_residual(hidden[t*h..<(t+1)*h],update) {return NhResult.{status=NhStatus.arithmetic};}
    }
    return NhResult.{written=tokens};
}
```

The softmax subtracts the largest score before `exp`. Without it, scores 800
apart would overflow; a check runs exactly that case.

The MLP layers have no gate: `up` to 12,544 values, the squared ReLU, `down`.

<!-- source: psionic/lib/nemotron-h.coax mlp -->
```jai
// One MLP layer: up, the squared ReLU, down. No gate, no biases.
NhMlp :: struct { norm:[]f32; up:[]f32; down:[]f32; }
nh_mlp_work :: (s:NhShape)->int {return 2*s.hidden+s.mlp;}
nh_mlp :: (s:NhShape,w:NhMlp,tokens:int,hidden:[]f32,work:[]f32)->NhResult {
    h:=s.hidden;
    if h<1 || w.norm.count!=h || w.up.count!=s.mlp*h || w.down.count!=h*s.mlp {return NhResult.{status=NhStatus.shape};}
    if tokens<1 || hidden.count!=tokens*h || work.count<nh_mlp_work(s) {return NhResult.{status=NhStatus.capacity};}
    normed:=work[0..<h];inner:=work[h..<h+s.mlp];update:=work[h+s.mlp..<2*h+s.mlp];
    for t:0..<tokens {
        if !nh_norm(hidden[t*h..<(t+1)*h],h,w.norm,normed) || !nh_linear(w.up,h,normed,inner) {return NhResult.{status=NhStatus.arithmetic};}
        for i:0..<s.mlp {
            r:=(f64(inner[i])+abs(f64(inner[i])))/2;r*=r;
            if !psionic_ops_fits_f32(r) {return NhResult.{status=NhStatus.arithmetic};}
            inner[i]=f32(r);
        }
        if !nh_linear(w.down,s.mlp,inner,update) || !nh_residual(hidden[t*h..<(t+1)*h],update) {return NhResult.{status=NhStatus.arithmetic};}
    }
    return NhResult.{written=tokens};
}
```

The model ends with a final norm and a readout of 131,072 rows, separate from
the embedding:

<!-- source: psionic/lib/nemotron-h.coax readout -->
```jai
// The final norm, then rows first, first + 1, ... of the readout into
// logits. Returns the index (from first) of the largest logit, the lowest
// index on a tie, or -1 when the arithmetic fails.
nh_readout :: (norm:[]f32,head:[]f32,hidden:[]f32,first:int,logits:[]f32,normed:[]f32)->int {
    h:=hidden.count;
    if h<1 || norm.count!=h || normed.count!=h || first<0 || head.count<(first+logits.count)*h || logits.count<1 {return -1;}
    if !nh_norm(hidden,h,norm,normed) || !nh_linear(head[first*h..<(first+logits.count)*h],h,normed,logits) {return -1;}
    best:=0;
    for i:1..<logits.count {if logits[i]>logits[best] {best=i;}}
    return best;
}
```

## The profile and the oracle

The profile is nemotron-h-cpu-f64-v1. Weights are BF16 decoded to `f32`
exactly. Activations are `f32` between steps; products, sums, norms, the
softmax and the gates are binary64, and each step's result is narrowed to
`f32` once. The recurrence keeps chapter 162's ssm-cpu-f64-v1. The reference
model runs in BF16 on a GPU; this profile is more precise than that, so its
results differ from a BF16 run by BF16 rounding, not by mistakes.

The oracle, `psionic/tests/nemotron-h-oracle.coax`, writes each layer kind
from its published definition in binary64 with nothing narrowed. Its
Mamba-2 layer convolves the raw inputs directly instead of keeping a history
register and steps the recurrence token by token; it shares no code with the
library or with chapter 162. The tolerance was frozen before the first
comparison:

<!-- source: psionic/tests/nemotron-h-oracle.coax tolerance -->
```jai
NH_TOLERANCE :: 0.00001;
```

The checks run a small model with the same structure: hidden width 6, a
Mamba-2 mixer of 4 heads of width 2 in 2 groups with state width 3, attention
with 4 query heads on 2 key and value heads, and an MLP 5 wide. Every layer
kind matches the oracle on seven tokens, for every chunk size from 1 to 7:

<!-- source: psionic/lib/nemotron-h.coax checks-blocks -->
```jai
#test "each layer kind matches the binary64 oracle on seven tokens" {
    s:=nho_shape();
    for seed:0..<3 {
        a:=nho_weights(seed);hidden:=nho_hidden(seed);
        for chunk:1..<8 {
            h:=hidden;history:[60]f32;state:[24]f32;work:[412]f32;fwork:[2000]f64;
            r:=nh_mamba(s,nho_mamba_view(a[..]),7,chunk,h[..],history[..],state[..],work[..],fwork[..]);
            ref:=nho_mamba(a[..],hidden[..]);g:=nho_gap(h[..],ref[..]);
            #assert r.status==NhStatus.ok && r.written==7 && g.gap<=g.limit;
        }
        h:=hidden;keys:[56]f32;values:[56]f32;work:[44]f32;fwork:[7]f64;
        r:=nh_attention(s,nho_attention_view(a[..]),7,0,h[..],keys[..],values[..],work[..],fwork[..]);
        ref:=nho_attention(a[..],hidden[..]);g:=nho_gap(h[..],ref[..]);
        #assert r.status==NhStatus.ok && r.written==7 && g.gap<=g.limit;
        h=hidden;
        mw:[17]f32;r=nh_mlp(s,nho_mlp_view(a[..]),7,h[..],mw[..]);
        ref=nho_mlp(a[..],hidden[..]);g=nho_gap(h[..],ref[..]);
        #assert r.status==NhStatus.ok && r.written==7 && g.gap<=g.limit;
    }
}
#test "a cached continuation equals the whole prompt, bit for bit" {
    s:=nho_shape();a:=nho_weights(1);hidden:=nho_hidden(1);
    // Mamba-2: 4 tokens then 3 in chunks of 4 are the chunks of 7 in 4.
    whole:=hidden;history:[60]f32;state:[24]f32;work:[420]f32;fwork:[2000]f64;
    #assert nh_mamba(s,nho_mamba_view(a[..]),7,4,whole[..],history[..],state[..],work[..],fwork[..]).written==7;
    split:=hidden;h2:[60]f32;s2:[24]f32;
    #assert nh_mamba(s,nho_mamba_view(a[..]),4,4,split[0..<24],h2[..],s2[..],work[..],fwork[..]).written==4;
    #assert nh_mamba(s,nho_mamba_view(a[..]),3,4,split[24..<42],h2[..],s2[..],work[..],fwork[..]).written==3;
    #assert nho_same(whole[..],split[..]) && nho_same(state[..],s2[..]) && nho_same(history[..],h2[..]);
    // Chunks of one token are the token step.
    one:=hidden;h3:[60]f32;s3:[24]f32;
    for t:0..<7 {#assert nh_mamba(s,nho_mamba_view(a[..]),1,1,one[t*6..<t*6+6],h3[..],s3[..],work[..],fwork[..]).written==1;}
    stepped:=hidden;h4:[60]f32;s4:[24]f32;
    #assert nh_mamba(s,nho_mamba_view(a[..]),7,1,stepped[..],h4[..],s4[..],work[..],fwork[..]).written==7;
    #assert nho_same(one[..],stepped[..]) && nho_same(s3[..],s4[..]);
    // Attention: the prompt at once, or one token at a time from the cache.
    all:=hidden;keys:[64]f32;values:[64]f32;
    #assert nh_attention(s,nho_attention_view(a[..]),7,0,all[..],keys[..],values[..],work[0..<44],fwork[0..<8]).written==7;
    each:=hidden;k2:[64]f32;v2:[64]f32;
    for t:0..<7 {#assert nh_attention(s,nho_attention_view(a[..]),1,t,each[t*6..<t*6+6],k2[..],v2[..],work[0..<44],fwork[0..<8]).written==1;}
    #assert nho_same(all[..],each[..]) && nho_same(keys[..],k2[..]) && nho_same(values[..],v2[..]);
    // The eighth cache row stays untouched.
    for i:56..<64 {#assert keys[i]==0 && values[i]==0;}
}
#test "two requests in sequence equal each alone, and a clear separates them" {
    s:=nho_shape();a:=nho_weights(2);x:=nho_hidden(3);y:=nho_hidden(4);
    alone:=y;history:[60]f32;state:[24]f32;work:[420]f32;fwork:[2000]f64;
    #assert nh_mamba(s,nho_mamba_view(a[..]),7,3,alone[..],history[..],state[..],work[..],fwork[..]).written==7;
    first:=x;h:[60]f32;st:[24]f32;
    #assert nh_mamba(s,nho_mamba_view(a[..]),7,3,first[..],h[..],st[..],work[..],fwork[..]).written==7;
    carried:=y;h1:=h;s1:=st;
    #assert nh_mamba(s,nho_mamba_view(a[..]),7,3,carried[..],h1[..],s1[..],work[..],fwork[..]).written==7;
    #assert !nho_same(carried[..],alone[..]);
    ss_clear(h[..],st[..]);after:=y;
    #assert nh_mamba(s,nho_mamba_view(a[..]),7,3,after[..],h[..],st[..],work[..],fwork[..]).written==7;
    #assert nho_same(after[..],alone[..]) && nho_same(st[..],state[..]);
}
```

## Run it

<!-- source: psionic/examples/nemotron-h.coax example -->
```jai
main :: () {
    print("profile nemotron-h-cpu-f64-v1, tolerance % x (1 + largest |reference value|)\n",NH_TOLERANCE);
    s:=nh_nano();p:=nh_pattern(s,NH_NANO_PATTERN);
    print("pinned revision %\n",NH_PINNED_REVISION);
    print("pattern %: % Mamba-2, % attention, % MLP layers\n",NH_NANO_PATTERN,p.counts[NH_MAMBA],p.counts[NH_ATTENTION],p.counts[NH_MLP]);
    r:=nh_inventory(s,NH_NANO_PATTERN,NH_PINNED_MANIFEST);
    print("inventory: % lines, % expected, status %, % parameters\n",r.lines,nh_expected_count(s,NH_NANO_PATTERN),r.status,r.parameters);
    buf:[1024]u8;n:=nh_config_text(s,NH_NANO_PATTERN,buf[..]);
    print("config fields match the pinned config.json: %\n",cast(string)buf[0..<n]==NH_PINNED_CONFIG);
    m:=s.ssm;
    print("Mamba-2 mixer: in_proj % rows = gate % + channels % (x %, B %, C %) + dt %\n",
        m.heads*m.head_dim+ss_channels(m)+m.heads,m.heads*m.head_dim,ss_channels(m),m.heads*m.head_dim,m.groups*m.state_dim,m.groups*m.state_dim,m.heads);
    print("  state per request % values, convolution history %, gate norm groups of %\n",ss_states(m),3*ss_channels(m),m.heads*m.head_dim/m.groups);

    // The hand trace: the gate norm of (3, 4) in one group, weights (1, 2).
    y:=[2]f32.[3,4];w:=[2]f32.[1,2];g:[2]f32;
    #assert nh_norm(y[..],2,w[..],g[..]);
    print("gate norm of (3, 4), weights (1, 2): (%, %)\n",g[0],g[1]);

    // The checks' model, seven tokens, against the binary64 oracle.
    t:=nho_shape();a:=nho_weights(0);hidden:=nho_hidden(0);
    print("small model: hidden 6, 4 Mamba-2 heads of 2 in 2 groups, state 3; 4 attention heads on 2 key/value heads; MLP 5\n");
    for chunk:1..<8 {
        h:=hidden;history:[60]f32;state:[24]f32;work:[412]f32;fwork:[2000]f64;
        res:=nh_mamba(t,nho_mamba_view(a[..]),7,chunk,h[..],history[..],state[..],work[..],fwork[..]);
        ref:=nho_mamba(a[..],hidden[..]);gap:=nho_gap(h[..],ref[..]);
        print("  Mamba-2 in chunks of %: status %, largest difference % (limit %)\n",chunk,res.status,gap.gap,gap.limit);
    }
    h:=hidden;keys:[56]f32;values:[56]f32;aw:[44]f32;af:[7]f64;
    res:=nh_attention(t,nho_attention_view(a[..]),7,0,h[..],keys[..],values[..],aw[..],af[..]);
    ref:=nho_attention(a[..],hidden[..]);gap:=nho_gap(h[..],ref[..]);
    print("  attention: status %, largest difference % (limit %)\n",res.status,gap.gap,gap.limit);
    h=hidden;mw:[17]f32;
    res=nh_mlp(t,nho_mlp_view(a[..]),7,h[..],mw[..]);
    ref=nho_mlp(a[..],hidden[..]);gap=nho_gap(h[..],ref[..]);
    print("  MLP: status %, largest difference % (limit %)\n",res.status,gap.gap,gap.limit);

    // A cached continuation: 4 tokens, then 3 more from the carried state.
    whole:=hidden;h1:[60]f32;s1:[24]f32;work:[412]f32;fwork:[2000]f64;
    res=nh_mamba(t,nho_mamba_view(a[..]),7,4,whole[..],h1[..],s1[..],work[..],fwork[..]);
    split:=hidden;h2:[60]f32;s2:[24]f32;
    res=nh_mamba(t,nho_mamba_view(a[..]),4,4,split[0..<24],h2[..],s2[..],work[..],fwork[..]);
    res=nh_mamba(t,nho_mamba_view(a[..]),3,4,split[24..<42],h2[..],s2[..],work[..],fwork[..]);
    print("Mamba-2 resumed after 4 tokens equals the prompt at once: %\n",nho_same(whole[..],split[..]) && nho_same(s1[..],s2[..]) && nho_same(h1[..],h2[..]));
    all:=hidden;k1:[56]f32;v1:[56]f32;each:=hidden;k2:[56]f32;v2:[56]f32;
    res=nh_attention(t,nho_attention_view(a[..]),7,0,all[..],k1[..],v1[..],aw[..],af[..]);
    for i:0..<7 {res=nh_attention(t,nho_attention_view(a[..]),1,i,each[i*6..<i*6+6],k2[..],v2[..],aw[..],af[..]);}
    print("attention one token at a time from the cache equals the prompt at once: %\n",nho_same(all[..],each[..]));

    // A deliberately broken input: chunk 0 is chapter 162's range refusal.
    h=hidden;ss_clear(h1[..],s1[..]);
    res=nh_mamba(t,nho_mamba_view(a[..]),7,0,h[..],h1[..],s1[..],work[..],fwork[..]);
    print("refused: chunk 0: %, chapter 162 says %, hidden unchanged %\n",res.status,res.ssm,nho_same(h[..],hidden[..]));
}
```

```text
profile nemotron-h-cpu-f64-v1, tolerance 0.00001 x (1 + largest |reference value|)
pinned revision dfaf35de3e30f1867dd8dbc38a7fc9fb52d3914f
pattern M-M-M-MM-M-M*-M-M*-M-M-M*-M-M-MM*-MMM-M-M-: 21 Mamba-2, 4 attention, 17 MLP layers
inventory: 263 lines, 263 expected, status ok, 3973556832 parameters
config fields match the pinned config.json: true
Mamba-2 mixer: in_proj 17504 rows = gate 7680 + channels 9728 (x 7680, B 1024, C 1024) + dt 96
  state per request 983040 values, convolution history 29184, gate norm groups of 960
gate norm of (3, 4), weights (1, 2): (0.8485278, 2.2627409)
small model: hidden 6, 4 Mamba-2 heads of 2 in 2 groups, state 3; 4 attention heads on 2 key/value heads; MLP 5
  Mamba-2 in chunks of 1: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  Mamba-2 in chunks of 2: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  Mamba-2 in chunks of 3: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  Mamba-2 in chunks of 4: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  Mamba-2 in chunks of 5: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  Mamba-2 in chunks of 6: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  Mamba-2 in chunks of 7: status ok, largest difference 2.870521891296107e-07 (limit 0.00005591095043640443)
  attention: status ok, largest difference 1.5569348521538018e-07 (limit 0.00003896688058578273)
  MLP: status ok, largest difference 2.3891233791317745e-07 (limit 0.00005125748874900987)
Mamba-2 resumed after 4 tokens equals the prompt at once: true
attention one token at a time from the cache equals the prompt at once: true
refused: chunk 0: recurrence, chapter 162 says range, hidden unchanged true
```

The Mamba-2 differences are the same for every chunk size because each
layer narrows the recurrence's output to `f32` before the gate norm and
`out_proj`; the chunked and stepped recurrences differ by less than that
rounding here.

## The tokenizer

The pinned `tokenizer.json` (17 MB) holds a byte-level BPE: 131,072
entries, 269,443 merges and 1,000 added tokens such as `<s>` and
`<|im_start|>`. `psionic/lib/nemotron-tokens.coax` reads it with the bounded
JSON pull parser of `coax/lib/nostr/json.coax` and encodes ASCII text with it.
It shares the pattern of chapter 81's Tekken but is written for this file.

<!-- source: psionic/lib/nemotron-tokens.coax types -->
```jai
// json: not JSON within the limits. format: a part this encoder needs is
// missing or not what it implements. capacity: the caller's storage is too
// small. vocabulary: a repeated entry, or a byte with no entry. merge: a
// merge whose parts are not entries, or a repeated merge. unsupported: text
// outside printable ASCII, tab, line feed and carriage return. range: a
// piece or text longer than its limit.
NtStatus :: enum u8 { ok; json; format; capacity; vocabulary; merge; unsupported; range; }
NtResult :: struct { status:NtStatus; written:int; cause:int; }
// The caller's storage. Entry e has the id ids[e] and the bytes
// bytes[at[e] ..< at[e] + count[e]]; slots and pairs are hash tables (a
// power of two each) holding entry + 1 and rank + 1; merge r joins entries
// left[r] and right[r] into joined[r]. specials holds the added tokens'
// entries and texts.
NtStore :: struct { bytes:[]u8; ids:[]int; at:[]int; count:[]int; slots:[]int; left:[]int; right:[]int; joined:[]int; pairs:[]int;
    specials:[]int; special_text:[]string; }
NtTable :: struct { status:NtStatus; cause:int; store:NtStore; entries:int; merges:int; specials:int; used:int; }
// A file may hold up to 256 bytes for each entry the storage holds.
NT_JSON_PER_ENTRY :: 256;
NT_STRING_MAX :: 4096;
NT_ENTRY_MAX :: 256;
NT_PIECE_MAX :: 256;
NT_TEXT_MAX :: 4096;
```

ByteLevel writes every byte as one character, so that each entry is
printable text. The 188 printable bytes stand for themselves; the other 68,
in byte order, become U+0100 and up. The space, byte 32, is the 33rd of
those (bytes 0 to 32 come first), so it is written U+0100 + 32 = U+0120,
`Ġ`; the line feed, byte 10, is U+010A, `Ċ`. Reading the file undoes this:

<!-- source: psionic/lib/nemotron-tokens.coax bytes -->
```jai
// ByteLevel writes each byte as a character: the 188 printable bytes
// (33..126, 161..172, 174..255) as themselves, the other 68 as U+0100 and
// up, in byte order. This is the way back.
nt_printable :: (b:int)->bool {return (b>=33 && b<=126) || (b>=161 && b<=172) || (b>=174 && b<=255);}
nt_char_byte :: (cp:int)->int #export_c {
    if cp<256 {if nt_printable(cp) {return cp;}return -1;}
    k:=cp-256;
    if k<33 {return k;}
    if k<67 {return k+94;}
    if k==67 {return 173;}
    return -1;
}
// A JSON string's raw bytes (escapes undone) as byte-level characters back
// to bytes, into out: the length, or -1 when it does not fit or a
// character is not one ByteLevel writes.
nt_key :: (raw:string,out:[]u8)->int {
    text:[NT_STRING_MAX]u8;n:=json_unescape(raw,text[..]);
    if n<0 {return -1;}
    i:=0;m:=0;
    while i<n {
        // Byte-level characters are below U+0800: one UTF-8 byte below
        // 0x80, else two.
        c:=cast(int)text[i];
        if c>=0xE0 {return -1;}
        cp:=c;size:=1+(c>>7);
        if size==2 {cp=((c&0x1F)<<6)|(cast(int)text[i+1]&0x3F);}
        b:=nt_char_byte(cp);
        if b<0 || m>=out.count {return -1;}
        out[m]=cast(u8)b;m+=1;i+=size;
    }
    return m;
}
```

Entries and merges go into two hash tables in the caller's storage, with
linear probing. An entry is found by its bytes; a merge by its two entries,
and it gives its rank (its place in the merge list) and the entry it makes.

<!-- source: psionic/lib/nemotron-tokens.coax tables -->
```jai
// A 30-bit hash: h = h * 131 + byte + 1 for each byte.
nt_hash :: (s:string)->int {h:=0;for b:s {h=(h*131+cast(int)b+1)&0x3FFFFFFF;}return h;}
nt_pair_hash :: (a:int,b:int)->int {return (a*131071+b)&0x3FFFFFFF;}
nt_entry :: (t:NtTable,e:int)->[]u8 {return t.store.bytes[t.store.at[e]..<t.store.at[e]+t.store.count[e]];}
// The entry with exactly these bytes, or -1. Linear probing; the table
// always keeps an empty slot.
nt_find :: (t:NtTable,s:string)->int {
    mask:=t.store.slots.count-1;i:=nt_hash(s)&mask;
    while t.store.slots[i]!=0 {
        e:=t.store.slots[i]-1;
        if cast(string)nt_entry(t,e)==s {return e;}
        i=(i+1)&mask;
    }
    return -1;
}
// The rank of the merge of entries a and b, or -1.
nt_rank :: (t:NtTable,a:int,b:int)->int {
    mask:=t.store.pairs.count-1;i:=nt_pair_hash(a,b)&mask;
    while t.store.pairs[i]!=0 {
        r:=t.store.pairs[i]-1;
        if t.store.left[r]==a && t.store.right[r]==b {return r;}
        i=(i+1)&mask;
    }
    return -1;
}
nt_power :: (n:int)->bool {p:=2;while p<n {p*=2;}return p==n;}
```

The reader checks what the encoder relies on and refuses anything else: no
normalizer, exactly one Split pattern and that it is the pinned one, a BPE
model with `ignore_merges`, every merge's two parts and their join in the
vocabulary, and no added token that begins another. The pinned file's 1,000
added tokens pass that last rule, so at most one can match at a place in
the text.

<!-- source: psionic/lib/nemotron-tokens.coax read -->
```jai
// The pattern of the pinned pre-tokenizer's Split step.
NT_PATTERN :: "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]*[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]+|[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]+[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]*|\\p{N}| ?[^\\s\\p{L}\\p{N}]+[\\r\\n/]*|\\s*[\\r\\n]+|\\s+(?!\\S)|\\s+";
nt_fail :: (t:[]NtTable,status:NtStatus,cause:int)->bool {t[0].status=status;t[0].cause=cause;return false;}
nt_is :: (j:[]Json,tok:int,text:string)->bool {return j[0].tok==tok && json_text(j)==text;}
nt_stop :: (t:[]NtTable,status:NtStatus,cause:int)->NtTable {t[0].status=status;t[0].cause=cause;return t[0];}
// The value after a key, skipped whole.
nt_skip :: (j:[]Json)->bool {first:=json_next(j);return json_skip(j);}
// One vocabulary entry: its key was just read.
nt_add_entry :: (j:[]Json,t:[]NtTable)->bool {
    s:=t[0].store;e:=t[0].entries;
    if e>=s.ids.count || e+1>=s.slots.count {return nt_fail(t,NtStatus.capacity,e);}
    n:=nt_key(json_text(j),s.bytes[t[0].used..<s.bytes.count]);
    if n<1 || n>NT_ENTRY_MAX {return nt_fail(t,NtStatus.format,e);}
    bytes:=s.bytes[t[0].used..<t[0].used+n];
    if json_next(j)!=JSON_NUMBER {return nt_fail(t,NtStatus.format,e);}
    id:=json_int(json_text(j));
    if !id.ok || id.value<0 {return nt_fail(t,NtStatus.format,e);}
    if nt_find(t[0],cast(string)bytes)>=0 {return nt_fail(t,NtStatus.vocabulary,e);}
    s.ids[e]=id.value;s.at[e]=t[0].used;s.count[e]=n;
    mask:=s.slots.count-1;i:=nt_hash(cast(string)bytes)&mask;
    while s.slots[i]!=0 {i=(i+1)&mask;}
    s.slots[i]=e+1;t[0].entries=e+1;t[0].used+=n;
    return true;
}
// One merge, [left, right]: the array was just opened.
nt_add_merge :: (j:[]Json,t:[]NtTable)->bool {
    s:=t[0].store;r:=t[0].merges;
    if r>=s.left.count || r+1>=s.pairs.count {return nt_fail(t,NtStatus.capacity,r);}
    joint:[2*NT_ENTRY_MAX]u8;parts:[2]int;n:=0;
    for k:0..<2 {
        if json_next(j)!=JSON_STRING {return nt_fail(t,NtStatus.format,r);}
        m:=nt_key(json_text(j),joint[n..<joint.count]);
        if m<0 {return nt_fail(t,NtStatus.format,r);}
        parts[k]=nt_find(t[0],cast(string)joint[n..<n+m]);n+=m;
    }
    if json_next(j)!=JSON_ARRAY_END {return nt_fail(t,NtStatus.format,r);}
    whole:=nt_find(t[0],cast(string)joint[0..<n]);
    if parts[0]<0 || parts[1]<0 || whole<0 || nt_rank(t[0],parts[0],parts[1])>=0 {return nt_fail(t,NtStatus.merge,r);}
    s.left[r]=parts[0];s.right[r]=parts[1];s.joined[r]=whole;
    mask:=s.pairs.count-1;i:=nt_pair_hash(parts[0],parts[1])&mask;
    while s.pairs[i]!=0 {i=(i+1)&mask;}
    s.pairs[i]=r+1;t[0].merges=r+1;
    return true;
}
// "model": type BPE, ignore_merges true, the vocabulary, then the merges.
nt_model :: (j:[]Json,t:[]NtTable)->bool {
    if json_next(j)!=JSON_OBJECT {return nt_fail(t,NtStatus.format,0);}
    bpe:=false;ignore:=false;
    while json_next(j)==JSON_STRING {
        if nt_is(j,JSON_STRING,"type") {bpe=json_next(j)==JSON_STRING && json_text(j)=="BPE";}
        else if nt_is(j,JSON_STRING,"ignore_merges") {ignore=json_next(j)==JSON_TRUE;}
        else if nt_is(j,JSON_STRING,"vocab") {
            if json_next(j)!=JSON_OBJECT {return nt_fail(t,NtStatus.format,0);}
            while json_next(j)==JSON_STRING {if !nt_add_entry(j,t) {return false;}}
        }
        else if nt_is(j,JSON_STRING,"merges") {
            if t[0].entries==0 || json_next(j)!=JSON_ARRAY {return nt_fail(t,NtStatus.format,0);}
            while json_next(j)==JSON_ARRAY {if !nt_add_merge(j,t) {return false;}}
        }
        else if !nt_skip(j) {return nt_fail(t,NtStatus.json,j[0].error_at);}
    }
    if j[0].tok!=JSON_OBJECT_END {return nt_fail(t,NtStatus.json,j[0].error_at);}
    if !bpe || !ignore || t[0].entries==0 {return nt_fail(t,NtStatus.format,0);}
    return true;
}
// "added_tokens": each object's id and content; content must have no escapes.
nt_added :: (j:[]Json,t:[]NtTable)->bool {
    if json_next(j)!=JSON_ARRAY {return nt_fail(t,NtStatus.format,0);}
    while json_next(j)==JSON_OBJECT {
        k:=t[0].specials;
        if k>=t[0].store.specials.count {return nt_fail(t,NtStatus.capacity,k);}
        id:=-1;text:="";
        while json_next(j)==JSON_STRING {
            if nt_is(j,JSON_STRING,"id") {if json_next(j)==JSON_NUMBER {v:=json_int(json_text(j));if v.ok {id=v.value;}}}
            else if nt_is(j,JSON_STRING,"content") {if json_next(j)==JSON_STRING {text=json_text(j);}}
            else if !nt_skip(j) {return nt_fail(t,NtStatus.json,j[0].error_at);}
        }
        if id<0 || text.count==0 {return nt_fail(t,NtStatus.format,k);}
        for c:text {if c=='\\' {return nt_fail(t,NtStatus.format,k);}}
        t[0].store.specials[k]=id;t[0].store.special_text[k]=text;t[0].specials=k+1;
    }
    return true;
}
// "pre_tokenizer": every "Regex" in it must be NT_PATTERN, and there must be one.
nt_pre :: (j:[]Json,t:[]NtTable)->bool {
    if json_next(j)!=JSON_OBJECT {return nt_fail(t,NtStatus.format,0);}
    depth:=j[0].depth-1;found:=0;
    while j[0].depth>depth {
        tok:=json_next(j);
        if tok==JSON_ERROR {return nt_fail(t,NtStatus.json,j[0].error_at);}
        if nt_is(j,JSON_STRING,"Regex") && j[0].key {
            if json_next(j)!=JSON_STRING {return nt_fail(t,NtStatus.format,1);}
            if !json_is(json_text(j),NT_PATTERN) {return nt_fail(t,NtStatus.format,1);}
            found+=1;
        }
    }
    if found!=1 {return nt_fail(t,NtStatus.format,1);}
    return true;
}
// Whether one of a and b begins the other (or they are the same).
nt_begins :: (a:string,b:string)->bool {
    for i:0..<a.count {
        if i>=b.count {return true;}
        if a[i]!=b[i] {return false;}
    }
    return true;
}
// The whole file. Added tokens are matched to their entries at the end.
nt_read :: (text:string,store:NtStore)->NtTable {
    tb:[1]NtTable;t:=tb[..];t[0].store=store;
    if !nt_power(store.slots.count) || !nt_power(store.pairs.count) || store.ids.count!=store.at.count || store.ids.count!=store.count.count
        || store.left.count!=store.right.count || store.left.count!=store.joined.count || store.specials.count!=store.special_text.count {
        return nt_stop(t,NtStatus.capacity,0);
    }
    for i:0..<store.slots.count {store.slots[i]=0;}
    for i:0..<store.pairs.count {store.pairs[i]=0;}
    j:[1]Json;json_start_bounded(j[..],text,NT_JSON_PER_ENTRY*store.ids.count,NT_STRING_MAX);
    if json_next(j[..])!=JSON_OBJECT {return nt_stop(t,NtStatus.json,j[0].error_at);}
    model:=false;pre:=false;
    while json_next(j[..])==JSON_STRING {
        ok:=true;
        if nt_is(j[..],JSON_STRING,"model") {ok=nt_model(j[..],t);model=true;}
        else if nt_is(j[..],JSON_STRING,"added_tokens") {ok=nt_added(j[..],t);}
        else if nt_is(j[..],JSON_STRING,"pre_tokenizer") {ok=nt_pre(j[..],t);pre=true;}
        else if nt_is(j[..],JSON_STRING,"normalizer") {if json_next(j[..])!=JSON_NULL {ok=nt_fail(t,NtStatus.format,2);}}
        else {skipped:=nt_skip(j[..]);}
        if !ok {return t[0];}
    }
    if j[0].tok!=JSON_OBJECT_END || json_next(j[..])!=JSON_END {return nt_stop(t,NtStatus.json,j[0].error_at);}
    if !model || !pre {return nt_stop(t,NtStatus.format,3);}
    // At most one added token may match at a place: none may begin another.
    for k:0..<t[0].specials {
        for o:0..<k {if nt_begins(store.special_text[o],store.special_text[k]) {return nt_stop(t,NtStatus.format,k);}}
        e:=nt_find(t[0],store.special_text[k]);
        if e<0 || store.ids[e]!=store.specials[k] {return nt_stop(t,NtStatus.format,k);}
        store.specials[k]=e;
    }
    return t[0];
}
```

Encoding splits out added tokens first, then cuts the text between them
into pieces with the pattern. On ASCII its Unicode classes reduce to five:

<!-- source: psionic/lib/nemotron-tokens.coax pieces -->
```jai
// The pattern on ASCII text, where its Unicode classes reduce to upper
// A-Z, lower a-z, digit 0-9, space (tab, line feed, carriage return, ' ')
// and other. In order:
//   [^\r\n L N]? upper* lower+ | [^\r\n L N]? upper+ lower*
//       together: an optional first character that is neither a letter, a
//       digit nor a newline, then the upper-case run, then the lower-case
//       run, with at least one letter
//   \p{N}                       one digit
//   " ?[^\s L N]+[\r\n/]*"       an optional space, other characters, then
//                                newlines and slashes
//   \s*[\r\n]+                  whitespace through its last newline
//   \s+(?!\S) | \s+             whitespace, all but the last before a
//                                non-space, else all of it
NT_UPPER :: 0; NT_LOWER :: 1; NT_DIGIT :: 2; NT_SPACE :: 3; NT_OTHER :: 4;
nt_class :: (c:u8)->int {
    if c>='A' && c<='Z' {return NT_UPPER;}
    if c>='a' && c<='z' {return NT_LOWER;}
    if c>='0' && c<='9' {return NT_DIGIT;}
    if c==' ' || c=='\t' || c=='\n' || c=='\r' {return NT_SPACE;}
    return NT_OTHER;
}
nt_newline :: (c:u8)->bool {return c=='\n' || c=='\r';}
nt_run :: (s:string,at:int,class:int)->int {p:=at;while p<s.count && nt_class(s[p])==class {p+=1;}return p;}
// The end of the piece that starts at at (at < s.count).
nt_piece :: (s:string,at:int)->int {
    c:=nt_class(s[at]);p:=at;
    if c==NT_OTHER || (c==NT_SPACE && !nt_newline(s[at])) {p+=1;}
    end:=nt_run(s,nt_run(s,p,NT_UPPER),NT_LOWER);
    if end>p {return end;}
    if c==NT_DIGIT {return at+1;}
    p=at;if s[at]==' ' {p+=1;}
    end=nt_run(s,p,NT_OTHER);
    if end>p {while end<s.count && (nt_newline(s[end]) || s[end]=='/') {end+=1;}return end;}
    run:=nt_run(s,at,NT_SPACE);last:=at;
    for i:at..<run {if nt_newline(s[i]) {last=i+1;}}
    if last>at {return last;}
    if run==s.count || run-at==1 {return run;}
    return run-1;
}
```

Each piece then becomes ids. Work one by hand with the checks' small file,
whose merges in rank order are `b c`, `a b`, `Ġ a`, `ab c`, `bc b`, `c a`
and `c c`. The piece `abcb` is not an entry, so it starts as bytes
`a b c b`. The adjacent pairs are `a b` (rank 1), `b c` (rank 0) and `c b`
(none); rank 0 wins: `a bc b`. Now `a bc` has no rank and `bc b` has rank
4: `a bcb`. Nothing joins `a` and `bcb`, so the ids are those of `a` and
`bcb`, 0 and 16. The piece `abc` is an entry, so with `ignore_merges` it is
one id, 5, without merging.

<!-- source: psionic/lib/nemotron-tokens.coax bpe -->
```jai
// One piece to ids: the whole piece if it is an entry (ignore_merges);
// otherwise its bytes, then repeatedly the adjacent pair with the lowest
// rank (the leftmost on a tie) joined, until no adjacent pair is a merge.
nt_bpe :: (t:NtTable,piece:string,out:[]int)->NtResult {
    n:=piece.count;
    if n<1 || n>NT_PIECE_MAX {return NtResult.{status=NtStatus.range,cause=n};}
    whole:=nt_find(t,piece);
    if whole>=0 {
        if out.count<1 {return NtResult.{status=NtStatus.capacity};}
        out[0]=t.store.ids[whole];return NtResult.{written=1};
    }
    parts:[NT_PIECE_MAX]int;
    for i:0..<n {
        parts[i]=nt_find(t,piece[i..<i+1]);
        if parts[i]<0 {return NtResult.{status=NtStatus.vocabulary,cause=i};}
    }
    while true {
        best:=-1;at:int;
        for i:0..<n-1 {r:=nt_rank(t,parts[i],parts[i+1]);if r>=0 && (best<0 || r<best) {best=r;at=i;}}
        if best<0 {break;}
        parts[at]=t.store.joined[best];n-=1;
        for i:at+1..<n {parts[i]=parts[i+1];}
    }
    if out.count<n {return NtResult.{status=NtStatus.capacity};}
    for i:0..<n {out[i]=t.store.ids[parts[i]];}
    return NtResult.{written=n};
}
```

<!-- source: psionic/lib/nemotron-tokens.coax encode -->
```jai
// The added token whose text starts at at (at < text.count), or -1. No
// added token begins another (nt_read refuses that), so at most one matches.
nt_special :: (t:NtTable,text:string,at:int)->int {
    first:=text[at];
    for k:0..<t.specials {
        s:=t.store.special_text[k];
        if s[0]==first && at+s.count<=text.count && text[at..<at+s.count]==s {return k;}
    }
    return -1;
}
// Text between added tokens: pieces, then byte-pair merging.
nt_segment :: (t:NtTable,text:string,out:[]int)->NtResult {
    at:=0;n:=0;
    while at<text.count {
        end:=nt_piece(text,at);
        r:=nt_bpe(t,text[at..<end],out[n..<out.count]);
        if r.status!=NtStatus.ok {r.cause+=at;return r;}
        n+=r.written;at=end;
    }
    return NtResult.{written=n};
}
// ASCII text to ids. cause is a byte offset into text.
nt_encode :: (t:NtTable,text:string,out:[]int)->NtResult {
    if t.status!=NtStatus.ok {return NtResult.{status=NtStatus.format};}
    if text.count>NT_TEXT_MAX {return NtResult.{status=NtStatus.range,cause=text.count};}
    for i:0..<text.count {
        c:=text[i];
        if (c<32 && c!='\t' && c!='\n' && c!='\r') || c>126 {return NtResult.{status=NtStatus.unsupported,cause=i};}
    }
    from:=0;at:=0;n:=0;
    while at<text.count {
        k:=nt_special(t,text,at);
        if k<0 {at+=1;continue;}
        r:=nt_segment(t,text[from..<at],out[n..<out.count]);
        if r.status!=NtStatus.ok {r.cause+=from;return r;}
        n+=r.written;
        if n>=out.count {return NtResult.{status=NtStatus.capacity,cause=at};}
        out[n]=t.store.ids[t.store.specials[k]];n+=1;
        at+=t.store.special_text[k].count;from=at;
    }
    r:=nt_segment(t,text[from..<text.count],out[n..<out.count]);
    if r.status!=NtStatus.ok {r.cause+=from;return r;}
    return NtResult.{written=n+r.written};
}
```

The checks read an excerpt of the pinned file: the same structure, cut by
`psionic/tests/nemotron_tokens.py` to the 398 entries and 93 merges that 12
fixture texts touch. The expected ids are what that script's encoder gives
over the full file. It is independent of the Coax encoder: it runs the
file's own pattern through Python's `re` with the Unicode classes rewritten
for ASCII, builds the byte characters GPT-2's way, and merges GPT-2's way
(every occurrence of the best pair in a round). The script also checks that
the excerpt gives the full file's ids.

<!-- source: psionic/lib/nemotron-tokens.coax checks-fixture -->
```jai
#test "the fixture texts encode to the ids of the full pinned file" #steps 400000000 {
    rig:[1]NtRig;r:=rig[..];json:=NT_EXCERPT;
    t:=nt_read(cast(string)json[..],nt_rig_store(r));
    #assert t.status==NtStatus.ok && t.entries==398 && t.merges==93 && t.specials==3;
    out:[64]int;
    for k:0..<NT_TEXTS.count {
        res:=nt_encode(t,NT_TEXTS[k],out[..]);ids:=NT_IDS;
        #assert nt_ids(res,out[..],ids[NT_STARTS[k]..<NT_STARTS[k+1]]);
    }
}
```

<!-- source: psionic/lib/nemotron-tokens.coax checks-small -->
```jai
#test "a small file: whole entries, merges by rank, added tokens, bytes" #steps 100000000 {
    rig:[1]NtRig;r:=rig[..];buf:[2048]u8;
    t:=nt_read(nt_small(buf[..]),nt_rig_store(r));
    #assert t.status==NtStatus.ok && t.entries==19 && t.merges==7 && t.specials==2 && t.used==33;
    // The tables hold one slot per entry and per merge, nothing else.
    #assert nt_nonzero(r[0].slots[..])==19 && nt_nonzero(r[0].pairs[..])==7;
    out:[16]int;
    // "abc" is an entry: one id, whatever the merges say.
    w1:=[1]int.[5];#assert nt_ids(nt_encode(t,"abc",out[..]),out[..],w1[..]);
    // "abcb": b c (rank 0) first, then bc b (rank 4): a, bcb.
    w2:=[2]int.[0,16];#assert nt_ids(nt_encode(t,"abcb",out[..]),out[..],w2[..]);
    // "cab": a b (rank 1); nothing joins c and ab: c, ab.
    w3:=[2]int.[2,3];#assert nt_ids(nt_encode(t,"cab",out[..]),out[..],w3[..]);
    // "bca": b c (rank 0) before c a (rank 5), though c a comes later.
    w9:=[2]int.[4,0];#assert nt_ids(nt_encode(t,"bca",out[..]),out[..],w9[..]);
    // "ccc": the same pair twice; the leftmost is joined first.
    w10:=[2]int.[18,2];#assert nt_ids(nt_encode(t,"ccc",out[..]),out[..],w10[..]);
    // " ab": a b (rank 1) comes before Ġ a (rank 2): Ġ, ab.
    w4:=[2]int.[6,3];#assert nt_ids(nt_encode(t," ab",out[..]),out[..],w4[..]);
    w5:=[2]int.[2,7];#assert nt_ids(nt_encode(t,"c a",out[..]),out[..],w5[..]);
    // Added tokens anywhere in the text, at its end too; "<y>" is plain text.
    w6:=[4]int.[0,10,1,9];#assert nt_ids(nt_encode(t,"a<xy>b<x>",out[..]),out[..],w6[..]);
    w7:=[3]int.[11,14,13];#assert nt_ids(nt_encode(t,"<y>",out[..]),out[..],w7[..]);
    // A line feed is the byte-level character U+010A; a backslash escapes in JSON.
    w8:=[2]int.[8,15];#assert nt_ids(nt_encode(t,"\n\\",out[..]),out[..],w8[..]);
    #assert nt_encode(t,"",out[..]).written==0;
    // Decoding an id gives its bytes back.
    d:[8]u8;n:=nt_decode(t,8,d[..],0);n=nt_decode(t,5,d[..],n);#assert n==4 && d[0]==10 && cast(string)d[1..<4]=="abc";
    #assert nt_decode(t,99,d[..],0)==-1 && nt_decode(t,5,d[0..<2],0)==-1 && nt_decode(t,5,d[0..<3],0)==3 && nt_decode(t,0,d[..],0)==1 && d[0]=='a';
    // The checks' own helper refuses a wrong status, count or id.
    out[0]=5;#assert !nt_ids(NtResult.{status=NtStatus.range,written=1},out[..],w1[..]) && !nt_ids(NtResult.{written=2},out[..],w1[..]);#assert nt_ids(NtResult.{written=1},out[..],w1[..]);out[0]=4;#assert !nt_ids(NtResult.{written=1},out[..],w1[..]);
}
```

The original-weight run below reads the whole pinned file in Coax (0.4 s)
and encodes the same 12 texts; the script compares them with its own
encoder over the full file:

```text
same   5 ids  'The capital of France is'
same  12 ids  'Hello, world! HTMLParser camelCase ABc aB'
same  32 ids  '2024-11-02: pi is 3.14159, e is 2.71828'
same  10 ids  '  leading spaces\n\n  and\ttabs \n'
same  11 ids  "don't won't I'm they're WE'LL"
same   9 ids  '<s>special <|im_start|>user\nhi<|im_end|>\n'
same  10 ids  'a//b /* comment */ x/y\n/z'
same  15 ids  'https://example.com/path?q=1&r=two#frag'
same   4 ids  'trailing space '
same  14 ids  'MixedCASEwords and UPPER lower\r\nCRLF'
same  16 ids  '\t\tindented()\n\treturn {x: [1, 2]};'
same   0 ids  ''
tokenizer over the full file: 138 ids, Coax and Python agree
```

| Status | When |
| --- | --- |
| `json` | not JSON, or longer than 256 bytes for each entry the storage holds, or a string over 4,096 bytes |
| `format` | no model or pre-tokenizer, a normalizer, another pattern, not BPE, no `ignore_merges`, an empty or over-long entry, an entry that is not ByteLevel, an added token that begins another or has no matching entry |
| `capacity` | the caller's storage is too small, or its tables are not a power of two |
| `vocabulary` | a repeated entry, or a byte in the text with no entry |
| `merge` | a merge whose parts or join are not entries, or a repeated merge |
| `unsupported` | a byte that is not printable ASCII, tab, line feed or carriage return |
| `range` | a text over 4,096 bytes or a piece over 256 |

<!-- source: psionic/lib/nemotron-tokens.coax checks-refusals -->
```jai
#test "refusals name their cause" #steps 400000000 {
    rig:[1]NtRig;r:=rig[..];buf:[2048]u8;good:=nt_small(buf[..]);
    t:=nt_read(good,nt_rig_store(r));out:[16]int;
    // Text: a byte outside printable ASCII, a byte with no entry, too long.
    odd:=[4]u8.['a','b',1,'c'];
    res:=nt_encode(t,cast(string)odd[..],out[..]);#assert res.status==NtStatus.unsupported && res.cause==2;
    res=nt_encode(t,"ab~",out[..]);#assert res.status==NtStatus.vocabulary && res.cause==2;
    odd[2]=127;res=nt_encode(t,cast(string)odd[..],out[..]);#assert res.status==NtStatus.unsupported && res.cause==2;
    odd[2]=31;res=nt_encode(t,cast(string)odd[..],out[..]);#assert res.status==NtStatus.unsupported && res.cause==2;
    odd[2]=0;res=nt_encode(t,cast(string)odd[..],out[..]);#assert res.status==NtStatus.unsupported && res.cause==2;
    odd[0]=0;res=nt_encode(t,cast(string)odd[..],out[..]);#assert res.status==NtStatus.unsupported && res.cause==0;
    res=nt_encode(t," \t\n\r~",out[..]);#assert res.status==NtStatus.vocabulary && res.cause==1;
    long:[4097]u8;for i:0..<4097 {long[i]='a';}
    res=nt_encode(t,cast(string)long[..],out[..]);#assert res.status==NtStatus.range && res.cause==4097;
    res=nt_encode(t,cast(string)long[0..<257],out[..]);#assert res.status==NtStatus.range && res.cause==257;
    res=nt_encode(t,cast(string)long[0..<256],out[0..<1]);#assert res.status==NtStatus.capacity;
    for i:0..<4096 {if i%2==1 {long[i]=' ';}}
    res=nt_encode(t,cast(string)long[0..<4096],out[..]);#assert res.status==NtStatus.capacity;
    res=nt_encode(t,cast(string)long[..],out[..]);#assert res.status==NtStatus.range && res.cause==4097;
    res=nt_encode(t,"abc",out[0..<0]);#assert res.status==NtStatus.capacity;
    res=nt_encode(t,"cab",out[0..<1]);#assert res.status==NtStatus.capacity;
    res=nt_encode(t,"cab",out[0..<2]);#assert res.status==NtStatus.ok && res.written==2;
    res=nt_encode(t,"c<x>",out[0..<1]);#assert res.status==NtStatus.capacity && res.cause==1;
    res=nt_encode(t,"c<x>",out[0..<2]);#assert res.status==NtStatus.ok && res.written==2;
    res=nt_encode(t,"<x>~",out[..]);#assert res.status==NtStatus.vocabulary && res.cause==3;
    res=nt_encode(t,"~<x>",out[..]);#assert res.status==NtStatus.vocabulary && res.cause==0;
    res=nt_encode(t,"<x>~<x>",out[..]);#assert res.status==NtStatus.vocabulary && res.cause==3;
    bad:=t;bad.status=NtStatus.json;#assert nt_encode(bad,"a",out[..]).status==NtStatus.format;
    // The file: each change below is refused with its status and cause.
    edits:=[16]string.["\"BPE\"","\"ignore_merges\":true","\"bc\":4","[\"bc\",\"b\"]","\"normalizer\":null","\"Regex\"","\"<xy>\"}","\"vocab\"","[\"b\",\"c\"]","\"id\":10","{\"added","\"model\"","\"c\":2","\"a\":0","[\"c\",\"a\"]","\"content\":\"<xy>\""];
    swaps:=[16]string.["\"WordPiece\"","\"ignore_merges\":false","\"bc\":4,\"bc\":7","[\"bc\",\"b\"],[\"b\",\"c\"]","\"normalizer\":{}","\"regex\"","\"<xy>\"},{\"content\":\"<z>\"}","\"vocabulary\"","[\"b\",\"q\"]","\"id\":11","[{\"added","\"modal\"","\"c\":-2","\"a\":0,\"a\":1","[\"c\",\"b\"]","\"content\":\"<x>y\""];
    wants:=[16]NtStatus.[NtStatus.format,NtStatus.format,NtStatus.vocabulary,NtStatus.merge,NtStatus.format,NtStatus.format,NtStatus.format,NtStatus.format,NtStatus.merge,NtStatus.format,NtStatus.json,NtStatus.format,NtStatus.format,NtStatus.vocabulary,NtStatus.merge,NtStatus.format];
    causes:=[16]int.[0,0,5,5,2,1,2,0,0,1,-1,3,2,1,5,1];
    for k:0..<16 {
        edited:[2100]u8;n:=nt_swap(good,edits[k],swaps[k],edited[..]);
        e:=nt_read(cast(string)edited[0..<n],nt_rig_store(r));
        #assert e.status==wants[k] && e.cause==causes[k];
    }
    // Storage: too few entries, slots, merges, pairs, specials, mismatched views.
    for k:0..<9 {
        s:=nt_rig_store(r);
        if k==0 {s.ids=r[0].ids[0..<16];s.at=r[0].at[0..<16];s.count=r[0].count[0..<16];}
        if k==1 {s.slots=r[0].slots[0..<16];}
        if k==2 {s.left=r[0].left[0..<4];s.right=r[0].right[0..<4];s.joined=r[0].joined[0..<4];}
        if k==3 {s.pairs=r[0].pairs[0..<4];}
        if k==4 {s.specials=r[0].specials[0..<1];s.special_text=r[0].texts[0..<1];}
        if k==5 {s.slots=r[0].slots[0..<1000];}
        if k==6 {s.at=r[0].at[0..<16];}
        if k==7 {s.joined=r[0].joined[0..<4];}
        if k==8 {s.special_text=r[0].texts[0..<1];}
        e:=nt_read(good,s);#assert e.status==NtStatus.capacity;
        causes2:=[9]int.[16,15,4,3,1,0,0,0,0];#assert e.cause==causes2[k];
    }
    s:=nt_rig_store(r);s.ids=r[0].ids[0..<19];s.at=r[0].at[0..<19];s.count=r[0].count[0..<19];s.slots=r[0].slots[0..<32];s.left=r[0].left[0..<7];s.right=r[0].right[0..<7];s.joined=r[0].joined[0..<7];s.pairs=r[0].pairs[0..<8];s.specials=r[0].specials[0..<2];s.special_text=r[0].texts[0..<2];
    #assert nt_read(good,s).status==NtStatus.ok;
    s.slots=r[0].slots[0..<16];#assert nt_read(good,s).status==NtStatus.capacity;
    s.slots=r[0].slots[0..<32];s.pairs=r[0].pairs[0..<4];#assert nt_read(good,s).status==NtStatus.capacity;
    s.pairs=r[0].pairs[0..<8];s.bytes=r[0].bytes[0..<32];#assert nt_read(good,s).status==NtStatus.format;
    s.bytes=r[0].bytes[0..<33];#assert nt_read(good,s).status==NtStatus.ok;
    // Not JSON, trailing bytes, parts in the wrong place or of the wrong kind.
    e:=nt_read("{",nt_rig_store(r));#assert e.status==NtStatus.json && nt_read("[]",nt_rig_store(r)).status==NtStatus.json;
    #assert nt_read("{} x",nt_rig_store(r)).status==NtStatus.json;
    e=nt_read("{}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==3;
    #assert nt_read("{\"x\":[1,}",nt_rig_store(r)).status==NtStatus.json;
    f:[1024]u8;
    e=nt_read(nt_file("\"vocab\":{}",f[..]),nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read(nt_file("\"merges\":[],\"vocab\":{\"a\":0}",f[..]),nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read(nt_file("\"vocab\":{\"a\":0},\"x\":[1,}",f[..]),nt_rig_store(r));#assert e.status==NtStatus.json && e.cause==345;
    e=nt_read(nt_file("\"vocab\":{\"a\":0} x",f[..]),nt_rig_store(r));#assert e.status==NtStatus.json && e.cause==338;
    e=nt_read(nt_file("\"vocab\":[]",f[..]),nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read(nt_file("\"vocab\":{\"a\":\"0\"}",f[..]),nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0.5}",f[..]),nt_rig_store(r)).status==NtStatus.format;
    #assert nt_read(nt_file("\"vocab\":{\" \":0}",f[..]),nt_rig_store(r)).status==NtStatus.format;
    e=nt_read(nt_file("\"vocab\":{\"a\":0,\"\":1}",f[..]),nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==1;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0},\"merges\":{}",f[..]),nt_rig_store(r)).status==NtStatus.format;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0},\"merges\":[[\"a\"]]",f[..]),nt_rig_store(r)).status==NtStatus.format;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0},\"merges\":[[\"a\",\"a\",\"a\"]]",f[..]),nt_rig_store(r)).status==NtStatus.format;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0},\"merges\":[[\"a\",\" \"]]",f[..]),nt_rig_store(r)).status==NtStatus.format;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0},\"merges\":[[\"a\",1]]",f[..]),nt_rig_store(r)).status==NtStatus.format;
    // Merge parts: a left or right that is no entry, an empty part, a join that is no entry.
    e=nt_read(nt_file("\"vocab\":{\"a\":0,\"b\":1,\"ab\":2},\"merges\":[[\"q\",\"b\"]]",f[..]),nt_rig_store(r));#assert e.status==NtStatus.merge && e.cause==0;
    e=nt_read(nt_file("\"vocab\":{\"a\":0,\"b\":1,\"ab\":2},\"merges\":[[\"a\",\"b\"],[\"\",\"a\"]]",f[..]),nt_rig_store(r));#assert e.status==NtStatus.merge && e.cause==1;
    e=nt_read(nt_file("\"vocab\":{\"a\":0,\"b\":1,\"ab\":2},\"merges\":[[\"b\",\"a\"]]",f[..]),nt_rig_store(r));#assert e.status==NtStatus.merge && e.cause==0;
    // An entry of 256 bytes is the longest allowed; 4096 raw bytes the longest string.
    v:[4200]u8;head:="\"vocab\":{\"";n:=0;for c:head {v[n]=c;n+=1;}
    for i:0..<256 {v[n]='a';n+=1;}
    tail:="\":0}";
    big:[4800]u8;
    #assert nt_read(nt_file(nt_with(v[..],n,tail),big[..]),nt_rig_store(r)).status==NtStatus.ok;
    v[n]='a';e=nt_read(nt_file(nt_with(v[..],n+1,tail),big[..]),nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    for i:n..<head.count+4097 {v[i]=0x61;}
    e=nt_read(nt_file(nt_with(v[..],head.count+4096,tail),big[..]),nt_rig_store(r));#assert e.status==NtStatus.format;
    v[head.count+4096]='a';e=nt_read(nt_file(nt_with(v[..],head.count+4097,tail),big[..]),nt_rig_store(r));#assert e.status==NtStatus.json && e.cause==4429;
    // Added tokens: the wrong kinds, no id, no content, an escape, a token that begins another.
    e=nt_read("{\"added_tokens\":{}}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read("{\"added_tokens\":[{\"id\":1}]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read("{\"added_tokens\":[{\"content\":\"a\"}]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read("{\"added_tokens\":[{\"id\":1,\"content\":\"a\\\\\"}]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read("{\"added_tokens\":[{\"id\":1,\"content\":\"\"}]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    e=nt_read("{\"added_tokens\":[{\"id\":0,\"content\":\"]\"}]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==3;
    #assert nt_read("{\"added_tokens\":[{\"id\":\"1\",\"content\":\"a\"}]}",nt_rig_store(r)).cause==0;
    #assert nt_read("{\"added_tokens\":[{\"id\":1,\"content\":2}]}",nt_rig_store(r)).cause==0;
    #assert nt_read("{\"added_tokens\":[{\"id\":1,\"x\":[}]}",nt_rig_store(r)).status==NtStatus.json;
    #assert nt_begins("<x>","<x>y") && nt_begins("<x>y","<x>") && nt_begins("ab","ab") && !nt_begins("ab","ac") && !nt_begins("b","ab") && nt_begins("","a");
    // The pre-tokenizer: the wrong kind, a pattern that is not a string, none, two.
    e=nt_read("{\"model\":[]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    // A different pattern; two added tokens where one begins the other.
    g2:[2100]u8;m2:=nt_swap(good,"(?!","(?=",g2[..]);e=nt_read(cast(string)g2[0..<m2],nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==1;
    g3:[2100]u8;m2=nt_swap(good,"\"content\":\"<xy>\"","\"content\":\"<x>y\"",g2[..]);m2=nt_swap(cast(string)g2[0..<m2],"\"<xy>\":10","\"<x>y\":10",g3[..]);
    e=nt_read(cast(string)g3[0..<m2],nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==1;
    m2=nt_swap(good,"\"content\":\"<xy>\"","\"content\":\"<xz>\"",g2[..]);m2=nt_swap(cast(string)g2[0..<m2],"\"<xy>\":10","\"<xz>\":10",g3[..]);
    #assert nt_read(cast(string)g3[0..<m2],nt_rig_store(r)).status==NtStatus.ok;
    e=nt_read("{\"pre_tokenizer\":[]}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    #assert nt_read("{\"pre_tokenizer\":{\"a\":[}}",nt_rig_store(r)).status==NtStatus.json;
    e=nt_read("{\"pre_tokenizer\":{\"Regex\":1}}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==1;
    e=nt_read("{\"pre_tokenizer\":{\"a\":\"Regex\"}}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==1;
    e=nt_read("{\"pre_tokenizer\":{\"a\":{\"b\":1}},\"model\":{}}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==1;
    // A model without its type or without ignore_merges.
    ff:=nt_file("\"vocab\":{\"a\":0}",f[..]);g:[1100]u8;
    m:=nt_swap(ff,"\"type\"","\"kind\"",g[..]);e=nt_read(cast(string)g[0..<m],nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    m=nt_swap(ff,"\"ignore_merges\"","\"ignore\"",g[..]);e=nt_read(cast(string)g[0..<m],nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==0;
    // A model and no pre-tokenizer.
    e=nt_read("{\"model\":{\"type\":\"BPE\",\"ignore_merges\":true,\"vocab\":{\"a\":0}}}",nt_rig_store(r));#assert e.status==NtStatus.format && e.cause==3;
    #assert nt_read(nt_file("\"vocab\":{\"a\":0}",f[..]),nt_rig_store(r)).status==NtStatus.ok;
    // A file over 256 bytes an entry of storage is refused before it is read;
    // trailing spaces count.
    pad:[4097]u8;small:=nt_file("\"vocab\":{\"a\":0}",f[..]);for i:0..<4097 {pad[i]=' ';}for i:0..<small.count {pad[i]=small[i];}
    s=nt_rig_store(r);s.ids=r[0].ids[0..<16];s.at=r[0].at[0..<16];s.count=r[0].count[0..<16];
    #assert nt_read(cast(string)pad[0..<4096],s).status==NtStatus.ok;
    e=nt_read(cast(string)pad[..],s);#assert e.status==NtStatus.json && e.cause==0;
}
```

## Original weights

`psionic/tests/check_nemotron_slices.sh` checks one layer of each kind on
the pinned weights. It authenticates the download with the manifest script,
compiles the Coax slice entry (`psionic/tests/nemotron-slices-runtime.coax`)
with a small C harness that maps the file and moves bytes, and compares every
output with `psionic/tests/nemotron_slices.py`, a plain-Python binary64
oracle written from the published architecture. The oracle reads only the
header and the tensor bytes; it imports nothing from the download.

The prompt is "The capital of France is" after the `<s>` token: ids 1, 1784,
8961, 1307, 5498, 1395. Each word is one entry of the pinned vocabulary, and
the pinned tokenizer takes a pre-token that is a whole entry as that entry
(`ignore_merges`), so these are the tokenizer's ids for this prompt; the
script looks each one up, and the Coax tokenizer above gives the same ids.

The slices chain: the six embedding rows, Mamba-2 layer 0 on them, MLP
layer 1 on that, attention layer 12 on that (the first attention layer,
given layer 1's output rather than layer 11's), and readout rows 0 to 8,191
for the last token. The original-weight tolerance is 1e-4 × (1 + the largest
|oracle value|), frozen in the script before the first run. Recorded on this
Linux host (28 cores, one thread used), compiled with gcc -O2:

```text
263 tensors: the fixture matches the pinned checkpoint
prompt tokens [1, 1784, 8961, 1307, 5498, 1395]
slice mamba layer 0: 6 tokens in one chunk, 1.0 s
  chunks of 4, the prompt at once or resumed after 4 tokens: outputs, state, history bit for bit true
slice mlp layer 1: 0.9 s
slice attention layer 12: 0.5 s
  the prompt at once or one token at a time from the cache: bit for bit true
slice readout rows 0..8191: largest at 8041
slices: 6.5 s
embedding rows (exact): largest difference 0, limit 0.00012, within
mamba layer 0: largest difference 3.21e-08, limit 0.000168, within
mamba layer 0 carried state: largest difference 1.3e-05, limit 0.0203, within
mamba layer 0 one token at a time: largest difference 3.21e-08, limit 0.000168, within
mlp layer 1: largest difference 5.18e-08, limit 0.000165, within
attention layer 12: largest difference 8.07e-07, limit 0.000837, within
readout rows 0..8191: largest difference 9.87e-07, limit 0.00075, within
largest logit in rows 0..8191: oracle 8041, Coax 8041
Nemotron slices: 78 s
```

The Coax side takes 6.5 seconds, including decoding about 1.3 GB of BF16.
A full Mamba-2 layer over six tokens takes about 1 second on one core.

## The whole model

`psionic/lib/nemotron-model.coax` puts the three layer kinds together. A
model is the embedding, the 42 layers in the order the pattern names them,
the final norm and the readout. A request's memory is every Mamba-2 layer's
history and state and every attention layer's keys and values, one layer
after another; layer `l` uses the slot of its kind, the number of earlier
layers of the same kind. For the pinned pattern that is 21 histories of
29,184 values, 21 states of 983,040 values (20.6 million values, 83 MB as
`f32`) and 4 key and value caches of 1,024 values a position.

<!-- source: psionic/lib/nemotron-model.coax model -->
```jai
// One layer's weights; only the view its kind names is used.
NhLayer :: struct { kind:int; mamba:NhMamba; attention:NhAttention; mlp:NhMlp; }
// The embedding (vocab x hidden), the layers in order, the final norm and
// the readout (vocab x hidden, separate from the embedding), read rows rows
// at a time, at most NH_READOUT_ROWS.
NhModel :: struct { shape:NhShape; pattern:string; embed:[]f32; layers:[]NhLayer; norm:[]f32; head:[]f32; rows:int; }
NH_READOUT_ROWS :: 32768;
// One request's memory for capacity positions: every Mamba-2 layer's
// history and state, every attention layer's keys and values, one layer
// after another in the order the layers come.
NhCache :: struct { capacity:int; history:[]f32; state:[]f32; keys:[]f32; values:[]f32; }
nh_history_size :: (s:NhShape,mambas:int)->int {return mambas*3*ss_channels(s.ssm);}
nh_state_size :: (s:NhShape,mambas:int)->int {return mambas*ss_states(s.ssm);}
nh_keys_size :: (s:NhShape,attentions:int,capacity:int)->int {return attentions*capacity*s.kv_heads*s.attn_dim;}
nh_model_valid :: (m:NhModel)->bool {
    s:=m.shape;p:=nh_pattern(s,m.pattern);
    if p.status!=NhStatus.ok || m.layers.count!=s.layers || s.vocab<1 {return false;}
    for i:0..<s.layers {if m.layers[i].kind!=nh_kind(m.pattern,i) {return false;}}
    if m.rows<1 || m.rows>NH_READOUT_ROWS {return false;}
    return m.embed.count==s.vocab*s.hidden && m.norm.count==s.hidden && m.head.count==s.vocab*s.hidden;
}
nh_cache_valid :: (m:NhModel,c:NhCache)->bool {
    s:=m.shape;p:=nh_pattern(s,m.pattern);
    if c.capacity<1 || c.history.count!=nh_history_size(s,p.counts[NH_MAMBA]) || c.state.count!=nh_state_size(s,p.counts[NH_MAMBA]) {return false;}
    return c.keys.count==nh_keys_size(s,p.counts[NH_ATTENTION],c.capacity) && c.values.count==c.keys.count;
}
// A new request starts from zero history, state, keys and values.
nh_clear :: (c:NhCache) {
    ss_clear(c.history,c.state);
    for i:0..<c.keys.count {c.keys[i]=0;c.values[i]=0;}
}
// f32 and f64 work for tokens rows at once in chunks of chunk.
// Each layer kind takes its work from the front, so the sum covers all.
nh_model_work :: (s:NhShape,tokens:int)->int {return nh_mamba_work(s,tokens)+nh_attention_work(s)+nh_mlp_work(s);}
nh_model_fwork :: (s:NhShape,chunk:int,capacity:int)->int {return ss_work_count(s.ssm,chunk)+capacity;}
```

Work one slot by hand. In `M-*M*-` layer 4 is attention; one attention
layer (layer 2) comes before it, so it uses attention slot 1, the keys from
`capacity x 8` to `2 x capacity x 8` in the small model (8 key values a
position). Layer 3 is the second Mamba-2 layer and uses history values 60 to
119 and state values 24 to 47.

<!-- source: psionic/lib/nemotron-model.coax layer -->
```jai
// Layer `layer` on tokens rows of hidden at positions start, start + 1, ...
// Its memory is the slot of its kind: the number of earlier layers of the
// same kind.
nh_slot :: (pattern:string,layer:int)->int {
    n:=0;for i:0..<layer {if pattern[i]==pattern[layer] {n+=1;}}
    return n;
}
nh_layer :: (m:NhModel,c:NhCache,layer:int,tokens:int,start:int,chunk:int,hidden:[]f32,work:[]f32,fwork:[]f64)->NhResult {
    s:=m.shape;w:=m.layers[layer];k:=nh_slot(m.pattern,layer);
    if w.kind==NH_MAMBA {
        hs:=3*ss_channels(s.ssm);ns:=ss_states(s.ssm);
        return nh_mamba(s,w.mamba,tokens,chunk,hidden,c.history[k*hs..<(k+1)*hs],c.state[k*ns..<(k+1)*ns],work,fwork);
    }
    if w.kind==NH_ATTENTION {
        size:=c.capacity*s.kv_heads*s.attn_dim;
        return nh_attention(s,w.attention,tokens,start,hidden,c.keys[k*size..<(k+1)*size],c.values[k*size..<(k+1)*size],work,fwork);
    }
    return nh_mlp(s,w.mlp,tokens,hidden,work);
}
```

The readout has 131,072 rows. It runs `rows` rows at a time, at most
32,768, and keeps the largest logit across the chunks, the lowest index on a
tie. With 9 rows in chunks of 4 the chunks are rows 0 to 3, 4 to 7 and 8.

<!-- source: psionic/lib/nemotron-model.coax readout -->
```jai
// The final norm and the whole readout for one hidden row, m.rows rows at
// a time; the last chunk takes what is left. Returns the index of the
// largest logit, the lowest on a tie, or -1 when the arithmetic fails.
nh_logits :: (m:NhModel,hidden:[]f32,logits:[]f32,normed:[]f32)->int {
    v:=m.shape.vocab;
    if m.rows<1 || logits.count!=v {return -1;}
    best:int;chunks:=v/m.rows;
    if chunks*m.rows<v {chunks+=1;}
    for k:0..<chunks {
        first:=k*m.rows;n:=ifx k==chunks-1 then v-first else m.rows;
        b:=nh_readout(m.norm,m.head,hidden,first,logits[first..<first+n],normed);
        if b<0 {return -1;}
        if k==0 || logits[first+b]>logits[best] {best=first+b;}
    }
    return best;
}
```

A pass embeds its tokens, runs every layer in order on all of them and
reads out the last token. Greedy decoding passes the prompt in chunks, then
feeds each chosen token back alone at the next position:

<!-- source: psionic/lib/nemotron-model.coax forward -->
```jai
// The outcome of one pass: the largest logit of the last token (best), or
// the refusal, the layer that refused (-1 before the layers) and, for an
// embedding refusal, the offending token's index.
NhRun :: struct { status:NhStatus; layer:int; at:int; best:int; ssm:SsStatus; }
// tokens at positions start, start + 1, ... through the whole model, in
// chunks of chunk for the Mamba-2 layers. hidden holds the tokens' rows
// (tokens x hidden) and is left as the last layer wrote it; logits holds
// the last token's logits.
nh_forward :: (m:NhModel,c:NhCache,tokens:[]int,start:int,chunk:int,hidden:[]f32,logits:[]f32,work:[]f32,fwork:[]f64)->NhRun {
    if !nh_model_valid(m) || !nh_cache_valid(m,c) {return NhRun.{status=NhStatus.shape,layer=-1};}
    s:=m.shape;h:=s.hidden;n:=tokens.count;
    if n<1 || start<0 || start+n>c.capacity || hidden.count!=n*h || logits.count!=s.vocab {return NhRun.{status=NhStatus.capacity,layer=-1};}
    if work.count<nh_model_work(s,n) || fwork.count<nh_model_fwork(s,chunk,c.capacity) {return NhRun.{status=NhStatus.capacity,layer=-1};}
    for t:0..<n {
        id:=tokens[t];
        if id<0 || id>=s.vocab {return NhRun.{status=NhStatus.token,layer=-1,at=t};}
        for i:0..<h {hidden[t*h+i]=m.embed[id*h+i];}
    }
    for layer:0..<s.layers {
        r:=nh_layer(m,c,layer,n,start,chunk,hidden,work,fwork);
        if r.status!=NhStatus.ok {return NhRun.{status=r.status,layer=layer,ssm=r.ssm};}
    }
    best:=nh_logits(m,hidden[(n-1)*h..<n*h],logits,work[0..<h]);
    if best<0 {return NhRun.{status=NhStatus.arithmetic,layer=s.layers};}
    return NhRun.{best=best};
}
```

<!-- source: psionic/lib/nemotron-model.coax greedy -->
```jai
// Greedy decoding: the prompt at once in chunks of chunk, then each chosen
// token fed back one at a time from the cache, steps tokens in all. The
// cache must be cleared by the caller and hold prompt + steps - 1
// positions. hidden holds the prompt's rows. out[i] is the i-th token chosen.
nh_greedy :: (m:NhModel,c:NhCache,prompt:[]int,steps:int,chunk:int,out:[]int,hidden:[]f32,logits:[]f32,work:[]f32,fwork:[]f64)->NhRun {
    p:=prompt.count;h:=m.shape.hidden;
    if steps<1 || out.count<steps || p+steps-1>c.capacity || hidden.count!=p*h {return NhRun.{status=NhStatus.capacity,layer=-1};}
    r:=nh_forward(m,c,prompt,0,chunk,hidden,logits,work,fwork);
    if r.status!=NhStatus.ok {return r;}
    out[0]=r.best;
    for i:1..<steps {
        r=nh_forward(m,c,out[i-1..<i],p+i-1,1,hidden[0..<h],logits,work,fwork);
        if r.status!=NhStatus.ok {return r;}
        out[i]=r.best;
    }
    return r;
}
```

The checks run a six-layer model of the small shape, `M-*M*-`, with a
vocabulary of 9. Its whole pass matches the oracle's layers chained in
binary64 for every chunk size from 1 to 7. Greedy decoding chooses four
tokens from a prompt of four; recomputing the prompt and the first three
choices in one pass from an empty cache chooses the same fourth token, and
with chunks of one token the two are the same arithmetic, so their last
logits agree bit for bit. A pass resumed after 4 tokens equals the 7 tokens
at once in chunks of 4, bit for bit, logits, keys and state:

<!-- source: psionic/lib/nemotron-model.coax checks-model -->
```jai
#test "the whole small model matches the oracle's layers chained" #steps 200000000 {
    for seed:0..<3 {
        rig:[1]NhmRig;r:=rig[..];nhm_rig(r,seed);m:=nhm_model(r);c:=nhm_cache(r);
        for chunk:1..<8 {
            nh_clear(c);hidden:[42]f32;logits:[9]f32;work:[NHM_WORK]f32;fwork:[2000]f64;tokens:=NHM_TOKENS;
            run:=nh_forward(m,c,tokens[..],0,chunk,hidden[..],logits[..],work[..],fwork[..]);
            ref:=nhm_oracle(r,tokens[..]);g:=nho_gap(logits[..],ref[..]);
            #assert run.status==NhStatus.ok && g.gap<=g.limit;
            best:=0;for i:1..<9 {if ref[i]>ref[best] {best=i;}}
            #assert run.best==best;
        }
    }
}
#test "greedy decoding from the cache equals recomputing the whole sequence, bit for bit" #steps 200000000 {
    rig:[1]NhmRig;r:=rig[..];nhm_rig(r,1);m:=nhm_model(r);c:=nhm_cache(r);
    for chunk:1..<4 {
        // Prompt of 4 in chunks of chunk, then 4 tokens chosen one at a time.
        nh_clear(c);prompt:=[4]int.[3,1,4,1];out:[4]int;hidden:[24]f32;logits:[9]f32;work:[NHM_WORK]f32;fwork:[2000]f64;
        need:=nh_model_fwork(m.shape,chunk,NHM_CAPACITY);
        run:=nh_greedy(m,c,prompt[..],4,chunk,out[..],hidden[..],logits[..],work[..],fwork[0..<need]);
        #assert run.status==NhStatus.ok && run.best==out[3];
        last:=logits;
        // The first choice is the prompt's own last logits.
        nh_clear(c);#assert nh_forward(m,c,prompt[..],0,chunk,hidden[..],logits[..],work[..],fwork[..]).best==out[0];
        // The prompt and the first three choices, 7 tokens, in one pass from
        // an empty cache: its last logits choose the fourth token.
        all:=[7]int.[3,1,4,1,out[0],out[1],out[2]];whole:[42]f32;again:[9]f32;
        nh_clear(c);run=nh_forward(m,c,all[..],0,1,whole[..],again[..],work[..],fwork[..]);
        #assert run.status==NhStatus.ok && run.best==out[3];
        // With chunks of one token the two are the same arithmetic.
        if chunk==1 {#assert nho_same(last[..],again[..]);}
        ref:=nhm_oracle(r,all[..]);g:=nho_gap(last[..],ref[..]);#assert g.gap<=g.limit;
        // The cache holds the eighth position untouched.
        for i:7*8..<8*8 {#assert r[0].keys[i]==0 && r[0].values[i]==0;}
    }
    // A pass resumed after 4 tokens equals 7 tokens at once in chunks of 4.
    tokens:=NHM_TOKENS;work:[NHM_WORK]f32;fwork:[2000]f64;
    nh_clear(c);whole:[42]f32;a:[9]f32;
    #assert nh_forward(m,c,tokens[..],0,4,whole[..],a[..],work[..],fwork[..]).status==NhStatus.ok;
    keys:=r[0].keys;state:=r[0].state;
    nh_clear(c);split:[42]f32;b:[9]f32;
    #assert nh_forward(m,c,tokens[0..<4],0,4,split[0..<24],b[..],work[..],fwork[..]).status==NhStatus.ok;
    #assert nh_forward(m,c,tokens[4..<7],4,4,split[24..<42],b[..],work[..],fwork[..]).status==NhStatus.ok;
    #assert nho_same(a[..],b[..]) && nho_same(whole[36..<42],split[36..<42]) && nho_same(keys[..],r[0].keys[..]) && nho_same(state[..],r[0].state[..]);
    // Clearing empties all four parts of the cache.
    nh_clear(c);
    for i:0..<128 {#assert r[0].keys[i]==0 && r[0].values[i]==0;}
    for i:0..<48 {#assert r[0].state[i]==0;}
    for i:0..<120 {#assert r[0].history[i]==0;}
}
```

Each refusal names the layer that refused, or -1 when the pass refused
before the first layer: a token outside the vocabulary (`token`, with its
index), a model or cache of the wrong shape, too few work values, a cache
too short for the request, or a layer's own refusal.

## The whole model on the original weights

`psionic/tests/check_nemotron_run.sh` runs the pinned checkpoint end to end
once. A small C harness (`psionic/tests/nemotron_run.c`) maps the file and
reads `tokenizer.json`; the Coax entry
(`psionic/tests/nemotron-run-runtime.coax`) decodes all 3,973,556,832 BF16
values into one `f32` arena (15.9 GB), reads the tokenizer, and runs this
chapter's model. The run:

1. encodes the 12 fixture texts over the full tokenizer file, and the prompt
   "The capital of France is" after `<s>`;
2. runs the prompt layer by layer and keeps every layer's rows;
3. decodes 8 tokens greedily: the prompt at once, then 7 steps from the
   cache;
4. recomputes the prompt and the first 7 choices, 13 tokens, in one pass
   from an empty cache and compares its last logits with the last step's.

Then `psionic/tests/nemotron_slices.py model` recomputes the whole model in
plain-Python binary64 over the same 13 tokens, its matrix products spread
over every core, and compares every layer's rows for the prompt, the
logits after the prompt and after the last step, and each of the 8 choices.
Its tolerance, frozen before its first comparison, is 1e-3 × (1 + the
largest |oracle value|) for each layer and for each set of logits. The run
is recorded in `psionic/fixtures/nemotron-run.txt`, not run as a routine
check:

```text
tokenizer: 12 texts encoded over the full file, 0.4 s
prompt tokens 1 1784 8961 1307 5498 1395
weights: 3973556832 values decoded from BF16, 12.0 s
prompt through 42 layers, layer by layer: 17.7 s
prefill of 6 tokens: 17.9 s; greedy, prefill and 7 steps: 41.2 s (3.3 s a step)
greedy tokens 6993 2613 1093 4848 1058 44642 29896 1010
first choice equals the prompt pass: true
recomputed 13 tokens in one pass: 38.3 s; last logits differ by at most 2.15e-06 (largest |logit| 27); choice 1010, cached 1010, same
whole run: 128.0 s, peak resident memory 22.2 GB
tokenizer over the full file: 138 ids, Coax and Python agree
42 layers, the prompt's rows: largest difference 0.000333 of each layer's limit, within
logits after the prompt: largest difference 1.95e-06, limit 0.00194, within
logits after the last step: largest difference 2.68e-06, limit 0.0028, within
greedy choices: oracle [6993, 2613, 1093, 4848, 1058, 44642, 29896, 1010], Coax [6993, 2613, 1093, 4848, 1058, 44642, 29896, 1010], same
oracle: 3 min 24 s on 28 cores
```

The fixture also records the continuation as text: " Paris." followed by
`"] output:`, three backticks, `python` and a line feed. The first token is
" Paris" (6993), then "."; after that this checkpoint,
tuned for chat, drifts into code-like text without its chat template. The
cached run and the one-pass recompute choose the same last token and their
logits differ by about 2e-6: the recompute runs the Mamba-2 recurrence in
one chunk of 13 and the cached run in a chunk of 6 then single steps, which
chapter 162 shows agree within its tolerance rather than bit for bit.

A request costs the weights (15.9 GB as `f32`), plus 85 MB of Mamba-2 state
and history and 8 KB a position for each attention layer's keys and values.
The peak of 22.2 GB also holds the decode buffers, the tokenizer tables and
the per-layer trace. On one core a token costs about 3.3 s; the prefill of
6 tokens about 18 s.

## Refusals

Each layer checks its weights' sizes against the shape, then its token count
and views, before it writes anything:

| Status | When |
| --- | --- |
| `shape` | a weight of the wrong size, hidden width 0, or a chapter 162 shape it refuses |
| `pattern` | a pattern of the wrong length or with a character other than `M`, `*` or `-` |
| `checkpoint` | a manifest missing an expected tensor, holding one twice, or holding more |
| `capacity` | no tokens, hidden rows or a cache of the wrong length, a start position past the cache, or too little work space |
| `arithmetic` | a norm, projection, square or residual that does not fit `f32`; a hidden value that is not finite fails the first norm |
| `recurrence` | chapter 162 refused; `ssm` carries its status (`range`, `number`, `arithmetic`) |

<!-- source: psionic/lib/nemotron-h.coax checks-refusals -->
```jai
#test "refusals name their cause and leave the hidden rows alone" {
    s:=nho_shape();a:=nho_weights(0);hidden:=nho_hidden(0);
    #assert nh_mamba_work(s,7)==412 && nh_attention_work(s)==44 && nh_mlp_work(s)==17;
    history:[60]f32;state:[24]f32;work:[412]f32;fwork:[2000]f64;
    m:=nho_mamba_view(a[..]);
    // Each weight one value short is a shape refusal.
    for k:0..<9 {
        w:=m;
        if k==0 {w.norm=m.norm[0..<5];}
        if k==1 {w.in_proj=m.in_proj[0..<191];}
        if k==2 {w.conv=m.conv[0..<79];}
        if k==3 {w.conv_bias=m.conv_bias[0..<19];}
        if k==4 {w.a_log=m.a_log[0..<3];}
        if k==5 {w.dt_bias=m.dt_bias[0..<3];}
        if k==6 {w.d=m.d[0..<3];}
        if k==7 {w.gate_norm=m.gate_norm[0..<7];}
        if k==8 {w.out_proj=m.out_proj[0..<47];}
        h:=hidden;
        #assert nh_mamba(s,w,7,4,h[..],history[..],state[..],work[..],fwork[..]).status==NhStatus.shape;
        #assert nho_same(h[..],hidden[..]);
    }
    bad:=s;bad.ssm.groups=3;h:=hidden;#assert nh_mamba(bad,m,7,4,h[..],history[..],state[..],work[..],fwork[..]).status==NhStatus.shape;
    bad=s;bad.hidden=0;#assert nh_mamba(bad,m,7,4,h[0..<0],history[..],state[..],work[..],fwork[..]).status==NhStatus.shape;
    // Hidden width 0 with weights of matching (empty) sizes is still refused.
    empty:=m;empty.norm=m.norm[0..<0];empty.in_proj=m.in_proj[0..<0];empty.out_proj=m.out_proj[0..<0];
    #assert nh_mamba(bad,empty,7,4,h[0..<0],history[..],state[..],work[..],fwork[..]).status==NhStatus.shape;
    // Hidden width 1 is allowed.
    thin:=m;thin.norm=m.norm[0..<1];thin.in_proj=m.in_proj[0..<32];thin.out_proj=m.out_proj[0..<8];bad.hidden=1;
    h1:=[1]f32.[0.5];hw:[162]f32;
    #assert nh_mamba_work(bad,1)==162 && nh_mamba(bad,thin,1,1,h1[..],history[..],state[..],hw[..],fwork[..]).status==NhStatus.ok;
    ss_clear(history[..],state[..]);
    #assert nh_mamba(s,m,0,4,h[0..<0],history[..],state[..],work[..],fwork[..]).status==NhStatus.capacity;
    #assert nh_mamba(s,m,7,4,h[0..<36],history[..],state[..],work[..],fwork[..]).status==NhStatus.capacity;
    #assert nh_mamba(s,m,7,4,h[..],history[..],state[..],work[0..<411],fwork[..]).status==NhStatus.capacity;
    // Chapter 162's own refusals come back with its status.
    h=hidden;r:=nh_mamba(s,m,7,0,h[..],history[..],state[..],work[..],fwork[..]);
    #assert r.status==NhStatus.recurrence && r.ssm==SsStatus.range && r.written==0;
    loud:=a;loud[298]=81;
    r=nh_mamba(s,nho_mamba_view(loud[..]),7,4,h[..],history[..],state[..],work[..],fwork[..]);
    #assert r.status==NhStatus.recurrence && r.ssm==SsStatus.number;
    for i:0..<24 {#assert state[i]==0;}
    // A token whose projection overflows f32 stops before the recurrence.
    huge:=a;for i:6..<198 {huge[i]=3e38;}
    r=nh_mamba(s,nho_mamba_view(huge[..]),7,4,h[..],history[..],state[..],work[..],fwork[..]);
    #assert r.status==NhStatus.arithmetic;
    for i:0..<24 {#assert state[i]==0;}
    huge=a;for i:318..<366 {huge[i]=3e38;}
    #assert nh_mamba(s,nho_mamba_view(huge[..]),7,4,h[..],history[..],state[..],work[..],fwork[..]).status==NhStatus.arithmetic;

    t:=nho_attention_view(a[..]);keys:[56]f32;values:[56]f32;aw:[44]f32;af:[7]f64;
    for k:0..<5 {
        w:=t;
        if k==0 {w.norm=t.norm[0..<5];}
        if k==1 {w.q=t.q[0..<95];}
        if k==2 {w.k=t.k[0..<47];}
        if k==3 {w.v=t.v[0..<47];}
        if k==4 {w.o=t.o[0..<95];}
        h=hidden;#assert nh_attention(s,w,7,0,h[..],keys[..],values[..],aw[..],af[..]).status==NhStatus.shape;
    }
    bad=s;bad.kv_heads=3;#assert nh_attention(bad,t,7,0,h[..],keys[..],values[..],aw[..],af[..]).status==NhStatus.shape;
    bad=s;bad.kv_heads=0;#assert nh_attention(bad,t,7,0,h[..],keys[..],values[..],aw[..],af[..]).status==NhStatus.shape;
    bad=s;bad.attn_dim=0;#assert nh_attention(bad,t,7,0,h[..],keys[..],values[..],aw[..],af[..]).status==NhStatus.shape;
    bad=s;bad.hidden=0;#assert nh_attention(bad,t,7,0,h[0..<0],keys[..],values[..],aw[..],af[..]).status==NhStatus.shape;
    hollow:=t;hollow.norm=t.norm[0..<0];hollow.q=t.q[0..<0];hollow.k=t.k[0..<0];hollow.v=t.v[0..<0];hollow.o=t.o[0..<0];
    #assert nh_attention(bad,hollow,7,0,h[0..<0],keys[..],values[..],aw[..],af[..]).status==NhStatus.shape;
    #assert nh_attention(s,t,0,0,h[0..<0],keys[..],values[..],aw[..],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,1,-1,h[0..<6],keys[..],values[..],aw[..],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,7,0,h[0..<36],keys[..],values[..],aw[..],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,7,0,h[..],keys[0..<55],values[0..<55],aw[..],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,7,0,h[..],keys[..],values[0..<48],aw[..],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,6,1,h[0..<36],keys[0..<48],values[0..<48],aw[..],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,6,1,h[0..<36],keys[..],values[..],aw[..],af[..]).status==NhStatus.ok;
    #assert nh_attention(s,t,7,0,h[..],keys[..],values[..],aw[0..<43],af[..]).status==NhStatus.capacity;
    #assert nh_attention(s,t,7,0,h[..],keys[..],values[..],aw[..],af[0..<6]).status==NhStatus.capacity;
    huge=a;for i:366..<462 {huge[i]=3e38;}h=hidden;
    #assert nh_attention(s,nho_attention_view(huge[..]),7,0,h[..],keys[..],values[..],aw[..],af[..]).status==NhStatus.arithmetic;
    huge=a;for i:558..<654 {huge[i]=3e38;}
    #assert nh_attention(s,nho_attention_view(huge[..]),7,0,h[..],keys[..],values[..],aw[..],af[..]).status==NhStatus.arithmetic;

    l:=nho_mlp_view(a[..]);mw:[17]f32;
    for k:0..<3 {
        w:=l;
        if k==0 {w.norm=l.norm[0..<5];}
        if k==1 {w.up=l.up[0..<29];}
        if k==2 {w.down=l.down[0..<29];}
        h=hidden;#assert nh_mlp(s,w,7,h[..],mw[..]).status==NhStatus.shape;
    }
    bad=s;bad.hidden=0;#assert nh_mlp(bad,l,7,h[0..<0],mw[..]).status==NhStatus.shape;
    #assert nh_mlp(s,l,0,h[0..<0],mw[..]).status==NhStatus.capacity;
    #assert nh_mlp(s,l,7,h[0..<36],mw[..]).status==NhStatus.capacity;
    #assert nh_mlp(s,l,7,h[..],mw[0..<16]).status==NhStatus.capacity;
    huge=a;for i:654..<684 {huge[i]=3e38;}h=hidden;
    #assert nh_mlp(s,nho_mlp_view(huge[..]),7,h[..],mw[..]).status==NhStatus.arithmetic;
    // Inner values of 2e19 square past f32.
    huge=a;for i:654..<684 {huge[i]=1e19;}
    #assert nh_mlp(s,nho_mlp_view(huge[..]),7,h[..],mw[..]).status==NhStatus.arithmetic;
    huge=a;for i:684..<714 {huge[i]=3e38;}
    #assert nh_mlp(s,nho_mlp_view(huge[..]),7,h[..],mw[..]).status==NhStatus.arithmetic;
    // One hidden value, one inner value, one key and value head are allowed.
    one:=NhShape.{hidden=1,mlp=1,attn_heads=1,kv_heads=1,attn_dim=1};
    w1:=[1]f32.[1];x1:=[1]f32.[2];m1:[3]f32;
    #assert nh_mlp(one,NhMlp.{norm=w1[..],up=w1[..],down=w1[..]},1,x1[..],m1[..]).status==NhStatus.ok;
    k1:[1]f32;v1:[1]f32;a1:[4]f32;f1:[1]f64;
    #assert nh_attention(one,NhAttention.{norm=w1[..],q=w1[..],k=w1[..],v=w1[..],o=w1[..]},1,0,x1[..],k1[..],v1[..],a1[..],f1[..]).status==NhStatus.ok;
    l1:[1]f32;n1:[1]f32;#assert nh_readout(w1[..],w1[..],x1[..],0,l1[..],n1[..])==0;
}
```

The refusal example builds the inventory for a config that says 32 query
heads instead of 40. The first attention layer's `o_proj` is then 3,136 × 4,096
instead of 3,136 × 5,120, and the inventory names it:

<!-- source: psionic/examples/nemotron-h-refuse.coax example -->
```jai
// A checkpoint whose config says 32 query heads instead of 40: the first
// attention tensor no longer matches the pinned header. The refusal names
// the first expected tensor the header does not hold.
main :: () {
    s:=nh_nano();s.attn_heads=32;
    r:=nh_inventory(s,NH_NANO_PATTERN,NH_PINNED_MANIFEST);
    buf:[96]u8;n:=nh_line(nh_expected(s,NH_NANO_PATTERN,r.bad),buf[..]);
    print("refused: status %, tensor % not in the pinned header: %\n",r.status,r.bad,cast(string)buf[0..<n]);
}
```

```text
refused: status checkpoint, tensor 82 not in the pinned header: backbone.layers.12.mixer.o_proj.weight BF16 3136,4096
```

A failure after the recurrence (in the gate norm or `out_proj`) leaves that
request's state advanced and its hidden rows partly written; the caller
starts the request again after `ss_clear`.

## What this does not do yet

- **Text outside ASCII.** The tokenizer refuses any other byte: the
  pattern's Unicode letter, number and mark classes are not built, so text
  with accents, other scripts or emoji is not encoded.
- **A routine whole-model check.** The original-weight run takes about two
  minutes and 22 GB, and its oracle several more; it is a recorded fixture,
  not part of `psionic/check.sh`.
- **Device kernels.** Everything runs on one CPU core.

## Sources

- [Chapter plan](PLAN.md), Part XVII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Nemotron Nano entry.
- [Chapter 162](ch162-a-recurrence-you-can-inspect.md): the recurrence this
  chapter's Mamba-2 layer runs.
- [Chapter 7](ch007-reading-original-weights.md): reading original weights.
- [Chapter 10](ch010-attention-without-training.md): attention and caches.
- [Chapter 17](ch017-qualify-honestly.md): qualification rules.
- The pinned `config.json`, safetensors header and model card of
  `nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16` at `dfaf35de`.
- Dao and Gu, *Transformers are SSMs* (2024): Mamba-2 and its grouped
  normalization.
