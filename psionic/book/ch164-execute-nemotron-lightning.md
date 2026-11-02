# 164. Execute Nemotron Lightning

> **Status: partly implemented.** The checkpoint inventory (from its headers
> alone), the mixture-of-experts layer and its router, a small model of all
> three layer kinds against an oracle, and the memory plan on this host run.
> The original weights, the whole 52-layer model, its tokens, logits and
> measured speed do not yet; see
> [What this does not do yet](#what-this-does-not-do-yet).

NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16 is
[chapter 163](ch163-execute-nemotron-nano.md)'s hybrid with a mixture of
experts (MoE) in place of the dense MLP layers. Of its 52 layers, 23 are
Mamba-2 layers, 6 are attention layers and 23 are MoE layers. Each MoE layer
holds 128 routed experts and one shared expert; a router chooses 6 routed
experts for each token. The checkpoint holds 32.9 billion values, but a
token multiplies only about 3.2 billion of them. This chapter shows why that
difference matters for memory: the plan must hold every expert, not only
the ones a token uses.

```shell
coax/bin/coax run psionic/examples/nemotron-lightning.coax
coax/bin/coax test psionic/lib/nemotron-lightning.coax
python3 -I psionic/tests/nemotron_lightning_manifest.py --fetch DIR
python3 -I psionic/tests/nemotron_lightning_manifest.py DIR
```

The [library](../lib/nemotron-lightning.coax), its
[check oracle](../tests/nemotron-lightning-oracle.coax), the
[example](../examples/nemotron-lightning.coax), the
[refusal example](../examples/nemotron-lightning-refuse.coax) and the
[manifest tool](../tests/nemotron_lightning_manifest.py) are the
implementation. Chapter 163's Mamba-2 and attention layers run unchanged;
this chapter adds the MoE layer, the router, the inventory and the plan.
Floating point comes first. NVFP4 weights and speculative decoding are later,
separate profiles.

## The pinned checkpoint, read by its headers

The weights come from Hugging Face at revision
`a9904d24bcc1d289a1950fa9d2b978c47cf903b9` of
`nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16`. The model card
identifies the OpenMDW-1.1 license. The checkpoint is 14 safetensors shards,
65,827,374,264 bytes in all. That is more than the disk budget on this host,
so nothing but headers was downloaded.

A safetensors file starts with 8 bytes that give the length of a JSON
header, then the header, then the tensor bytes. `--fetch` downloads
`config.json`, `model.safetensors.index.json`, and, with HTTP range
requests, the first 8 bytes and the header of each shard: about 1 MB. The
second form of the tool checks what it fetched:

- `config.json`, the index and the 14 headers hash to their pins;
- in each shard, every tensor's byte span is exactly its shape times its
  dtype size, the spans tile the data with no gap or overlap, and the last
  one ends at the shard's pinned size;
- the index names the same shard for every tensor as the headers do.

A header read by range cannot be checked against the shard's SHA-256; that
needs the whole shard. The header pin is the hash observed on first fetch,
and the shard hashes are recorded in the tool for the download a later run
makes.

The tool writes `psionic/fixtures/nemotron-lightning-manifest.coax`. There
are 6,513 tensors, 5,888 of them experts. When a layer holds experts 0 to
127 of one kind, all with the same dtype and shape, the tool writes them as
one line named `experts.0..127`. That leaves 417 lines, small enough for
chapter 163's inventory check.

The shape is chapter 163's `NhShape` plus the mixture. The config's
`layers_block_type` list is written one letter a layer, as in chapter 163,
with `E` for a MoE layer. The model also ships two multi-token prediction
(MTP) layers, an attention layer and a MoE layer, used for speculative
decoding.

<!-- source: psionic/lib/nemotron-lightning.coax shape -->
```jai
// Lightning's shape: chapter 163's NhShape (its mlp field unused), the
// layer pattern, the multi-token prediction (MTP) layers' pattern, and the
// mixture: experts routed experts of width expert_mlp, top_k chosen for
// each token, and one shared expert of width shared_mlp that every token
// uses.
NlShape :: struct { base:NhShape; pattern:string; mtp:string; experts:int; top_k:int; expert_mlp:int; shared_mlp:int; }
// Layer kinds: chapter 163's M, * and -, and E, a mixture of experts.
NL_MOE :: 3;
// The pinned config: hidden_size 2688, mamba_num_heads 64, mamba_head_dim
// 64, ssm_state_size 128, n_groups 8, num_attention_heads 32,
// num_key_value_heads 2, head_dim 128, n_routed_experts 128,
// num_experts_per_tok 6, moe_intermediate_size 1856,
// moe_shared_expert_intermediate_size 3712; layers_block_type and
// mtp_layers_block_type written one letter a layer.
nl_lightning :: ()->NlShape {
    b:=NhShape.{hidden=2688,vocab=131072,layers=52,ssm=SsShape.{heads=64,head_dim=64,state_dim=128,groups=8},attn_heads=32,kv_heads=2,attn_dim=128};
    return NlShape.{base=b,pattern=NL_LIGHTNING_PATTERN,mtp="*E",experts=128,top_k=6,expert_mlp=1856,shared_mlp=3712};
}
NL_LIGHTNING_PATTERN :: "MEMEM*EMEMEM*EMEMEM*EMEMEM*EMEMEM*EMEMEMEM*EMEMEMEME";
// routed_scaling_factor: the chosen experts' weights sum to 2.5.
NL_SCALE :: 2.5;
nl_kind :: (pattern:string,layer:int)->int {
    if pattern[layer]=='E' {return NL_MOE;}
    return nh_kind(pattern,layer);
}
// counts: layers of each kind in the main pattern; bad: the first unknown
// letter (MTP letters count from the end of the main pattern), or -1.
NlPattern :: struct { status:NhStatus; counts:[4]int; bad:int; }
NL_MTP_MAX :: 4;
nl_pattern :: (s:NlShape)->NlPattern {
    r:NlPattern;r.bad=-1;
    if s.pattern.count!=s.base.layers || s.mtp.count>NL_MTP_MAX {r.status=NhStatus.pattern;return r;}
    for i:0..<s.pattern.count+s.mtp.count {
        k:=ifx i<s.pattern.count then nl_kind(s.pattern,i) else nl_kind(s.mtp,i-s.pattern.count);
        if k<0 {r.status=NhStatus.pattern;r.bad=i;return r;}
        if i<s.pattern.count {r.counts[k]+=1;}
    }
    return r;
}
```

The expected inventory follows the pattern. A MoE layer has 7 lines: the
two expert lines, the router's correction bias (the only F32 tensor kind),
the router's gate rows, the shared expert's two matrices and the block norm.
The first MTP layer adds `eh_proj`, which joins an embedding row and a
hidden row back into one hidden row, and two norms; the last adds a final
norm.

<!-- source: psionic/lib/nemotron-lightning.coax checkpoint -->
```jai
// One expected manifest line. experts > 0 names experts 0 .. experts - 1
// of one kind on one line; wide marks the one F32 tensor kind.
NlTensor :: struct { mtp:bool; layer:int; experts:int; suffix:string; wide:bool; rank:int; dims:[3]int; }
nl_from :: (t:NhTensor)->NlTensor {return NlTensor.{layer=t.layer,suffix=t.suffix,rank=t.rank,dims=t.dims};}
nl_d1 :: (layer:int,suffix:string,a:int)->NlTensor {return nl_from(nh_d1(layer,suffix,a));}
nl_d2 :: (layer:int,suffix:string,a:int,b:int)->NlTensor {return nl_from(nh_d2(layer,suffix,a,b));}
NL_MOE_NAMES :: [7]string.[".down_proj.weight",".up_proj.weight",".mixer.gate.e_score_correction_bias",".mixer.gate.weight",
    ".mixer.shared_experts.down_proj.weight",".mixer.shared_experts.up_proj.weight",".norm.weight"];
NL_PER_KIND :: [4]int.[9,5,3,7];
nl_layer_tensor :: (s:NlShape,kind:int,layer:int,k:int)->NlTensor {
    if kind!=NL_MOE {return nl_from(nh_layer_tensor(s.base,kind,layer,k));}
    h:=s.base.hidden;E:=s.experts;name:=NL_MOE_NAMES[k];
    if k==0 {t:=nl_d2(layer,name,h,s.expert_mlp);t.experts=E;return t;}
    if k==1 {t:=nl_d2(layer,name,s.expert_mlp,h);t.experts=E;return t;}
    if k==2 {t:=nl_d1(layer,name,E);t.wide=true;return t;}
    if k==3 {return nl_d2(layer,name,E,h);}
    if k==4 {return nl_d2(layer,name,h,s.shared_mlp);}
    if k==5 {return nl_d2(layer,name,s.shared_mlp,h);}
    return nl_d1(layer,name,h);
}
// The first MTP layer also holds eh_proj (the embedding and the hidden row
// joined, back to hidden) and their two norms; the last holds a final norm.
nl_mtp_count :: (s:NlShape,j:int)->int {
    n:=NL_PER_KIND[nl_kind(s.mtp,j)];
    if j==0 {n+=3;}
    if j==s.mtp.count-1 {n+=1;}
    return n;
}
nl_mtp_tensor :: (s:NlShape,j:int,k:int)->NlTensor {
    h:=s.base.hidden;at:=k;
    if j==0 {
        if k==0 {t:=nl_d2(j,".eh_proj.weight",h,2*h);t.mtp=true;return t;}
        if k<3 {t:=nl_d1(j,ifx k==1 then ".enorm.weight" else ".hnorm.weight",h);t.mtp=true;return t;}
        at=k-3;
    }
    t:=ifx at==NL_PER_KIND[nl_kind(s.mtp,j)] then nl_d1(j,".final_layernorm.weight",h) else nl_layer_tensor(s,nl_kind(s.mtp,j),j,at);
    t.mtp=true;return t;
}
nl_expected_count :: (s:NlShape)->int {
    n:=3;for i:0..<s.base.layers {n+=NL_PER_KIND[nl_kind(s.pattern,i)];}
    for j:0..<s.mtp.count {n+=nl_mtp_count(s,j);}
    return n;
}
// Position i: the embedding, the final norm and the readout, each main
// layer's tensors, then each MTP layer's.
nl_expected :: (s:NlShape,i:int)->NlTensor {
    if i<3 {return nl_from(nh_expected(s.base,"",i));}
    j:=i-3;layer:=0;
    while layer<s.base.layers && j>=NL_PER_KIND[nl_kind(s.pattern,layer)] {j-=NL_PER_KIND[nl_kind(s.pattern,layer)];layer+=1;}
    if layer<s.base.layers {return nl_layer_tensor(s,nl_kind(s.pattern,layer),layer,j);}
    m:=0;while j>=nl_mtp_count(s,m) {j-=nl_mtp_count(s,m);m+=1;}
    return nl_mtp_tensor(s,m,j);
}
```

The check counts each expected line in the manifest. It must appear exactly
once, and the manifest must hold nothing else.

<!-- source: psionic/lib/nemotron-lightning.coax inventory -->
```jai
// One manifest line: the name, the dtype, the dimensions joined by commas.
nl_line :: (t:NlTensor,out:[]u8)->int {
    n:=0;
    if t.layer>=0 {n=nh_put(out,n,ifx t.mtp then "mtp.layers." else "backbone.layers.");n=nh_put_int(out,n,t.layer);}
    if t.experts!=0 {n=nh_put(out,n,".mixer.experts.0..");n=nh_put_int(out,n,t.experts-1);}
    n=nh_put(out,n,t.suffix);n=nh_put(out,n,ifx t.wide then " F32 " else " BF16 ");
    for i:0..<t.rank {if i>0 {n=nh_put(out,n,",");}n=nh_put_int(out,n,t.dims[i]);}
    return n;
}
// The values a line stands for: experts lines count every expert.
nl_values :: (t:NlTensor)->int {
    size:=1;for k:0..<t.rank {size*=t.dims[k];}
    if t.experts!=0 {size*=t.experts;}
    return size;
}
// A manifest matches when it holds every expected line exactly once and
// nothing else, as in chapter 163. parameters counts the values, bytes their
// storage (2 a BF16 value, 4 an F32 value). More than NL_LINES_MAX lines are
// refused.
NlInventory :: struct { status:NhStatus; lines:int; parameters:int; bytes:int; bad:int; }
NL_LINES_MAX :: 1024;
nl_inventory :: (s:NlShape,manifest:string)->NlInventory {
    p:=nl_pattern(s);
    if p.status!=NhStatus.ok {return NlInventory.{status=p.status,bad=p.bad};}
    ends:[1025]int;lines:=0;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==NL_LINES_MAX {return NlInventory.{status=NhStatus.checkpoint,lines=lines+1,bad=-1};}
        lines+=1;ends[lines]=i;
    }}
    r:NlInventory;r.lines=lines;r.bad=-1;buf:[128]u8;
    for e:0..<nl_expected_count(s) {
        t:=nl_expected(s,e);n:=nl_line(t,buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {return NlInventory.{status=NhStatus.checkpoint,lines=lines,bad=e};}
        v:=nl_values(t);r.parameters+=v;r.bytes+=ifx t.wide then 4*v else 2*v;
    }
    if lines!=nl_expected_count(s) {return NlInventory.{status=NhStatus.checkpoint,lines=lines,bad=-1};}
    return r;
}
```

The 417 lines stand for 32,913,266,240 values in 65,826,538,624 bytes. With
the 835,640 bytes of headers and length prefixes, that is the pinned total
of the 14 shards, to the byte.

<!-- source: psionic/lib/nemotron-lightning.coax checks-checkpoint -->
```jai
#test "the pinned headers hold exactly the tensors the config names" {
    s:=nl_lightning();p:=nl_pattern(s);
    #assert p.status==NhStatus.ok && p.counts[NH_MAMBA]==23 && p.counts[NH_ATTENTION]==6 && p.counts[NL_MOE]==23 && p.counts[NH_MLP]==0;
    #assert nl_expected_count(s)==417 && NL_PINNED_LINES==417;
    r:=nl_inventory(s,NL_PINNED_MANIFEST);
    // The header survey's totals: 32,913,266,240 values in 65,826,538,624
    // bytes, plus 835,640 bytes of headers, are the 14 pinned shard sizes.
    #assert r.status==NhStatus.ok && r.lines==417 && r.bad==-1 && r.parameters==32913266240 && r.bytes==65826538624;
    #assert r.bytes+835640==NL_PINNED_BYTES;
    // 6,513 tensors: 48 lines stand for 128 experts each.
    #assert 417-48+48*128==NL_PINNED_TENSORS;
    buf:[2048]u8;n:=nl_config_text(s,buf[..]);
    #assert cast(string)buf[0..<n]==NL_PINNED_CONFIG;
}
```

## The router, by hand

NemotronH's router (read in transformers 5.19.0's
`models/nemotron_h/modeling_nemotron_h.py`, not run) works like this:

1. Multiply the gate rows by the normed input: one logit an expert.
2. Score each expert with the sigmoid of its logit.
3. Add the expert's correction bias to its score to get its choice. The
   bias steers which experts are chosen; it does not change their weights.
4. Choose the 6 experts with the largest choices. With one expert group
   (`n_group` 1), group selection chooses everything.
5. Give each chosen expert its score divided by the sum of the chosen
   scores plus 1e-20, times `routed_scaling_factor` 2.5.

Take four experts, one input value 1, gate rows 0, 2, 1 and -1, and a bias
of 0.5 on expert 3 only. Choose two.

| Expert | Logit | Score | Choice |
| --- | --- | --- | --- |
| 0 | 0 | 0.5 | 0.5 |
| 1 | 2 | 0.8808 | 0.8808 |
| 2 | 1 | 0.7311 | 0.7311 |
| 3 | -1 | 0.2689 | 0.7689 |

The two largest choices are experts 1 and 3. Without the bias, expert 2
would have been chosen. The weights use the scores: the sum is
0.8808 + 0.2689 = 1.1497, so expert 1 gets 0.8808 / 1.1497 × 2.5 = 1.9152
and expert 3 gets 0.2689 / 1.1497 × 2.5 = 0.5848. The weights add up to
2.5, not 1.

The margin is the last chosen choice minus the largest unchosen one:
0.7689 − 0.7311 = 0.0379. It records how far the selection is from changing.
When two choices are equal, the lower index is chosen and the margin is 0.

<!-- source: psionic/lib/nemotron-lightning.coax route -->
```jai
// The router for one normed input x. Each expert's score is the sigmoid of
// its gate row times x; the choice adds the expert's correction bias. The
// top_k largest choices are chosen, the lower index first on a tie. Their
// weights are their scores, not their choices, divided by the chosen
// scores' sum plus 1e-20, times NL_SCALE. margin is the last chosen choice
// minus the largest unchosen one: how far the selection is from changing.
NL_TOP_MAX :: 8;NL_EXPERTS_MAX :: 256;
NlRoute :: struct { status:NhStatus; count:int; ids:[8]int; weights:[8]f32; margin:f64; }
nl_route :: (gate:[]f32,bias:[]f32,x:[]f32,k:int,fwork:[]f64)->NlRoute {
    E:=bias.count;h:=x.count;
    if E>NL_EXPERTS_MAX || k<1 || k>NL_TOP_MAX || k>=E || h<1 || gate.count!=E*h {return NlRoute.{status=NhStatus.shape};}
    if fwork.count<2*E {return NlRoute.{status=NhStatus.capacity};}
    scores:=fwork[0..<E];choice:=fwork[E..<2*E];
    for e:0..<E {
        acc:PsionicAccumulator;
        for i:0..<h {acc=psionic_ops_accumulate(acc,f64(gate[e*h+i])*f64(x[i]));}
        // The sigmoid exp(z/2) / (exp(z/2) + exp(-z/2)), both scaled by
        // exp(-|z|/2) so that no exp argument is above 0.
        z:=psionic_ops_total(acc);up:=exp((z-abs(z))/2);
        scores[e]=up/(up+exp((-z-abs(z))/2));
        if (f32_bits(bias[e])&0x7fffffff)>=0x7f800000 {return NlRoute.{status=NhStatus.arithmetic};}
        choice[e]=scores[e]+f64(bias[e]);
    }
    r:NlRoute;r.count=k;used:[256]bool;sum:=0.0;
    for slot:0..<k {r.ids[slot]=nl_best(choice,used[..]);used[r.ids[slot]]=true;sum+=scores[r.ids[slot]];}
    r.margin=choice[r.ids[k-1]]-choice[nl_best(choice,used[..])];
    for slot:0..<k {r.weights[slot]=f32(scores[r.ids[slot]]/(sum+1e-20)*NL_SCALE);}
    return r;
}
// The unused expert with the largest choice, the lowest index on a tie.
nl_best :: (choice:[]f64,used:[]bool)->int {
    best:=-1;
    for e:0..<choice.count {if !used[e] && (best<0 || choice[e]>choice[best]) {best=e;}}
    return best;
}
// A route whose margin is below NL_MARGIN is a near tie: a reference that
// rounds its scores differently may choose another expert, so its selection
// is compared only as a set of near-best experts (see the checks).
NL_MARGIN :: 0.00001;
```

The sigmoid is written as exp(z/2) / (exp(z/2) + exp(-z/2)) with both terms
scaled by exp(-|z|/2), so no exp argument is ever above 0. A bias that is not
finite is refused.

<!-- source: psionic/lib/nemotron-lightning.coax checks-route -->
```jai
#test "the router, worked by hand" {
    // One input value 1 (already normed), four experts with gate 0, 2, 1, -1:
    // scores 0.5, 0.8808, 0.7311, 0.2689. The bias 0.5 on expert 3 makes its
    // choice 0.7689, above expert 2's 0.7311, so experts 1 and 3 are chosen;
    // their weights use the scores: 0.8808 / 1.1497 * 2.5 = 1.9152 and
    // 0.2689 / 1.1497 * 2.5 = 0.5848.
    gate:=[4]f32.[0,2,1,-1];bias:=[4]f32.[0,0,0,0.5];x:=[1]f32.[1];fwork:[8]f64;
    r:=nl_route(gate[..],bias[..],x[..],2,fwork[..]);
    s1:=1/(1+exp(-2.0));s3:=exp(-1.0)/(1+exp(-1.0));s2:=1/(1+exp(-1.0));
    #assert r.status==NhStatus.ok && r.count==2 && r.ids[0]==1 && r.ids[1]==3;
    #assert r.weights[0]==f32(s1/(s1+s3+1e-20)*2.5) && r.weights[1]==f32(s3/(s1+s3+1e-20)*2.5);
    #assert abs(r.weights[0]-1.915212)<1e-6 && abs(r.weights[1]-0.584788)<1e-6 && r.margin==(s3+0.5)-s2 && abs(r.margin-0.037883)<1e-6;
    // Without the bias, experts 1 and 2.
    bias[3]=0;r=nl_route(gate[..],bias[..],x[..],2,fwork[..]);#assert r.ids[0]==1 && r.ids[1]==2 && r.margin==s2-0.5;
    // A tie goes to the lower index, with margin 0.
    tie:=[4]f32.[1,2,1,1];r=nl_route(tie[..],bias[..],x[..],2,fwork[..]);
    #assert r.ids[0]==1 && r.ids[1]==0 && r.margin==0 && r.margin<NL_MARGIN;
    r=nl_route(tie[..],bias[..],x[..],3,fwork[..]);#assert r.ids[2]==2 && r.margin==0;
    r=nl_route(tie[..],bias[..],x[..],1,fwork[..]);#assert r.ids[0]==1 && r.weights[0]==2.5 && r.margin==s1-s2;
    // Scores that underflow to 0 give weights of exactly +0.
    low:=[4]f32.[-1000,-1000,-1000,-1000];r=nl_route(low[..],bias[..],x[..],2,fwork[..]);
    #assert r.status==NhStatus.ok && f32_bits(r.weights[0])==0 && f32_bits(r.weights[1])==0;
    // Refusals: shapes, room and a bias that is not finite.
    #assert nl_route(gate[..],bias[..],x[..],0,fwork[..]).status==NhStatus.shape && nl_route(gate[..],bias[..],x[..],4,fwork[..]).status==NhStatus.shape;
    #assert nl_route(gate[..],bias[..],x[..],3,fwork[..]).status==NhStatus.ok && nl_route(gate[0..<3],bias[..],x[..],2,fwork[..]).status==NhStatus.shape;
    #assert nl_route(gate[..],bias[..],x[0..<0],2,fwork[..]).status==NhStatus.shape && nl_route(gate[..],bias[..],x[..],2,fwork[0..<7]).status==NhStatus.capacity;
    ten:[10]f32;big:[257]f32;one:[1]f32;w10:[20]f64;w257:[514]f64;
    #assert nl_route(ten[..],ten[..],one[..],8,w10[..]).status==NhStatus.ok && nl_route(ten[..],ten[..],one[..],9,w10[..]).status==NhStatus.shape;
    #assert nl_route(big[0..<256],big[0..<256],one[..],8,w257[..]).status==NhStatus.ok && nl_route(big[..],big[..],one[..],8,w257[..]).status==NhStatus.shape;
    huge:f32=3e38;inf:=huge*huge;bias[2]=inf;#assert nl_route(gate[..],bias[..],x[..],2,fwork[..]).status==NhStatus.arithmetic;
    bias[2]=3e38;#assert nl_route(gate[..],bias[..],x[..],2,fwork[..]).status==NhStatus.ok;
}
```

## A mixture layer

Each expert is chapter 163's MLP without its residual: up, the squared ReLU,
down, with no gate and no biases. The layer adds the chosen experts' outputs,
each times its weight, and the shared expert's output, unweighted. The sum
is kept in binary64 and narrowed to f32 once.

<!-- source: psionic/lib/nemotron-lightning.coax moe -->
```jai
// One mixture layer's weights, decoded to f32, in checkpoint layout: the
// block norm, the gate rows and correction bias, every expert's up and down
// matrices one after another, and the shared expert's.
NlMoe :: struct { norm:[]f32; gate:[]f32; bias:[]f32; up:[]f32; down:[]f32; shared_up:[]f32; shared_down:[]f32; }
nl_moe_work :: (s:NlShape)->int {return 2*s.base.hidden+rp_max(s.expert_mlp,s.shared_mlp);}
nl_moe_fwork :: (s:NlShape)->int {return s.base.hidden+2*s.experts;}
nl_moe_shape :: (s:NlShape,w:NlMoe)->bool {
    h:=s.base.hidden;E:=s.experts;em:=s.expert_mlp;sm:=s.shared_mlp;
    if h<1 || w.norm.count!=h || w.gate.count!=E*h || w.bias.count!=E {return false;}
    return w.up.count==E*em*h && w.down.count==E*h*em && w.shared_up.count==sm*h && w.shared_down.count==h*sm;
}
// down(relu(up x)^2), as chapter 163's MLP without its residual.
nl_expert :: (up:[]f32,down:[]f32,x:[]f32,inner:[]f32,out:[]f32)->bool {
    if !nh_linear(up,x.count,x,inner) {return false;}
    for i:0..<inner.count {
        r:=(f64(inner[i])+abs(f64(inner[i])))/2;r*=r;
        if !psionic_ops_fits_f32(r) {return false;}
        inner[i]=f32(r);
    }
    return nh_linear(down,inner.count,inner,out);
}
// tokens rows of hidden through the layer: the block norm, the route, each
// chosen expert's output times its weight, plus the shared expert's, summed
// in binary64 and narrowed once; then the residual. routes[t] records token
// t's route. On a refusal, written counts the tokens already done.
nl_moe :: (s:NlShape,w:NlMoe,tokens:int,hidden:[]f32,routes:[]NlRoute,work:[]f32,fwork:[]f64)->NhResult {
    if !nl_moe_shape(s,w) {return NhResult.{status=NhStatus.shape};}
    h:=s.base.hidden;em:=s.expert_mlp;E:=s.experts;
    if tokens<1 || hidden.count!=tokens*h || routes.count<tokens || work.count<nl_moe_work(s) || fwork.count<nl_moe_fwork(s) {return NhResult.{status=NhStatus.capacity};}
    normed:=work[0..<h];out:=work[h..<2*h];inner:=work[2*h..<work.count];sums:=fwork[0..<h];
    for t:0..<tokens {
        for i:0..<h {sums[i]=0;}
        if !nh_norm(hidden[t*h..<(t+1)*h],h,w.norm,normed) {return NhResult.{status=NhStatus.arithmetic,written=t};}
        r:=nl_route(w.gate,w.bias,normed,s.top_k,fwork[h..<h+2*E]);routes[t]=r;
        if r.status!=NhStatus.ok {return NhResult.{status=r.status,written=t};}
        for slot:0..<r.count {
            e:=r.ids[slot];
            if !nl_expert(w.up[e*em*h..<(e+1)*em*h],w.down[e*h*em..<(e+1)*h*em],normed,inner[0..<em],out) {return NhResult.{status=NhStatus.arithmetic,written=t};}
            for i:0..<h {sums[i]+=f64(r.weights[slot])*f64(out[i]);}
        }
        if !nl_expert(w.shared_up,w.shared_down,normed,inner[0..<s.shared_mlp],out) {return NhResult.{status=NhStatus.arithmetic,written=t};}
        for i:0..<h {
            v:=sums[i]+f64(out[i]);
            if !psionic_ops_fits_f32(v) {return NhResult.{status=NhStatus.arithmetic,written=t};}
            out[i]=f32(v);
        }
        if !nh_residual(hidden[t*h..<(t+1)*h],out) {return NhResult.{status=NhStatus.arithmetic,written=t};}
    }
    return NhResult.{written=tokens};
}
```

With one hidden value it can be worked by hand:

<!-- source: psionic/lib/nemotron-lightning.coax checks-hand -->
```jai
#test "a mixture of one hidden value, worked by hand" {
    // Hidden 2 normalizes to 2 / sqrt(4 + 1e-5), just under 1. Gate rows 1,
    // 2 and -1 give scores 0.731, 0.881 and 0.269, so expert 1 is chosen
    // alone, with weight 2.5. Its up 1, square and down 3 give 3, times 2.5
    // is 7.5; the shared expert's up 2, square 4 and down 0.5 give 2. The
    // residual adds 9.5 to 2.
    s:=NlShape.{base=NhShape.{hidden=1},experts=3,top_k=1,expert_mlp=1,shared_mlp=1};
    norm:=[1]f32.[1];gate:=[3]f32.[1,2,-1];bias:[3]f32;up:=[3]f32.[5,1,5];down:=[3]f32.[5,3,5];su:=[1]f32.[2];sd:=[1]f32.[0.5];
    w:=NlMoe.{norm=norm[..],gate=gate[..],bias=bias[..],up=up[..],down=down[..],shared_up=su[..],shared_down=sd[..]};
    h:=[1]f32.[2];routes:[1]NlRoute;work:[3]f32;fwork:[7]f64;
    r:=nl_moe(s,w,1,h[..],routes[..],work[..],fwork[..]);
    #assert r.status==NhStatus.ok && r.written==1 && routes[0].ids[0]==1 && routes[0].weights[0]==2.5 && abs(h[0]-11.5)<0.0001;
}
```

## The profile and the oracle

The profile is chapter 163's `nemotron-h-cpu-f64-v1`, with its tolerance: an
output may differ from the oracle by at most 1e-5 × (1 + the largest
|reference value|). A route agrees with the oracle's when it chooses the same
experts with the same weights, within the tolerance. A route whose margin is
below `NL_MARGIN` (1e-5) is a near tie. A reference that rounds its scores
differently may choose the other expert, so a near-tie route agrees when
every expert it chose is within the margin of the oracle's last chosen one.

The [oracle](../tests/nemotron-lightning-oracle.coax) is written from the
published definition in binary64 with nothing narrowed to f32. It chooses
experts by rank (the number of experts with a larger choice, or an equal
choice and a smaller index) instead of by repeated maximum. It shares no code
with the library.

The checks' model is chapter 163's small model (hidden 6) with a mixture of
5 experts of width 3, 2 chosen, and a shared expert of width 4. The stack
`M * E` runs chapter 163's Mamba-2 and attention layers, then the MoE layer,
then the readout. The oracle stack runs chapter 163's oracles for `M` and
`*` and this chapter's for `E`, rounding each layer's binary64 output to f32
before the next.

<!-- source: psionic/lib/nemotron-lightning.coax checks-moe -->
```jai
#test "the mixture layer matches the binary64 oracle, token by token and route by route" {
    s:=nlo_shape();
    for seed:0..<6 {
        a:=nlo_weights(seed);hidden:=nho_hidden(seed);h:=hidden;
        routes:[7]NlRoute;work:[16]f32;fwork:[16]f64;
        r:=nl_moe(s,nlo_view(a[..]),7,h[..],routes[..],work[..],fwork[..]);
        ref:[42]f64;nlo_moe(a[..],hidden[..],ref[..]);g:=nlo_gap(h[..],ref[..]);
        #assert r.status==NhStatus.ok && r.written==7 && g.gap<=g.limit;
        for t:0..<7 {
            x:[6]f64;n:[6]f64;for i:0..<6 {x[i]=f64(hidden[t*6+i]);}
            nlo_rms(x[..],nlo_part(a[..],0),n[..]);
            #assert routes[t].status==NhStatus.ok && routes[t].count==2 && nlo_agrees(routes[t],nlo_route(nlo_part(a[..],1),nlo_part(a[..],2),n[..]));
        }
    }
}
#test "a small Lightning, M * E, matches its oracle stack and its greedy tokens" {
    s:=nlo_shape();
    for seed:0..<3 {
        m:=nho_weights(seed);a:=nlo_weights(seed);hidden:=nho_hidden(seed+5);
        // The library: each layer in turn on one hidden array.
        h:=hidden;history:[60]f32;state:[24]f32;work:[412]f32;fwork:[2000]f64;keys:[56]f32;values:[56]f32;routes:[7]NlRoute;
        #assert nh_mamba(s.base,nho_mamba_view(m[..]),7,3,h[..],history[..],state[..],work[..],fwork[..]).written==7;
        #assert nh_attention(s.base,nho_attention_view(m[..]),7,0,h[..],keys[..],values[..],work[0..<44],fwork[0..<7]).written==7;
        #assert nl_moe(s,nlo_view(a[..]),7,h[..],routes[..],work[0..<16],fwork[0..<16]).written==7;
        // The oracles: chapter 163's for M and *, this chapter's for E, each
        // layer's binary64 output rounded to f32 before the next.
        o:=hidden;ref:=nho_mamba(m[..],o[..]);nlo_round(ref[..],o[..]);
        ref=nho_attention(m[..],o[..]);nlo_round(ref[..],o[..]);
        last:[42]f64;nlo_moe(a[..],o[..],last[..]);g:=nlo_gap(h[..],last[..]);
        #assert g.gap<=g.limit;
        for t:0..<7 {
            logits:[9]f32;normed:[6]f32;ol:[9]f64;
            best:=nh_readout(nlo_part(a[..],7),nlo_part(a[..],8),h[t*6..<t*6+6],0,logits[..],normed[..]);
            want:=nlo_readout(a[..],last[t*6..<t*6+6],ol[..]);lg:=nlo_gap(logits[..],ol[..]);
            #assert best==want && lg.gap<=lg.limit;
        }
    }
}
#test "a mixture of one token at a time equals the prompt at once" {
    s:=nlo_shape();a:=nlo_weights(2);hidden:=nho_hidden(1);
    all:=hidden;routes:[7]NlRoute;work:[16]f32;fwork:[16]f64;
    #assert nl_moe(s,nlo_view(a[..]),7,all[..],routes[..],work[..],fwork[..]).written==7;
    each:=hidden;one:[1]NlRoute;
    for t:0..<7 {
        #assert nl_moe(s,nlo_view(a[..]),1,each[t*6..<t*6+6],one[..],work[..],fwork[..]).written==1;
        #assert one[0].ids[0]==routes[t].ids[0] && one[0].ids[1]==routes[t].ids[1] && one[0].margin==routes[t].margin;
    }
    #assert nho_same(all[..],each[..]);
}
```

## The memory plan

Chapter 33's planner admits a request as resident, admits it with streamed
weights, or refuses it. This chapter counts Lightning's bytes and hands them
to the same decision, `rp_admit`, which chapter 33's `rp_plan` now calls
too. The plan leaves out the MTP layers (2,670,652,160 bytes), since
floating-point execution does not run them.

<!-- source: psionic/lib/nemotron-lightning.coax plan -->
```jai
// Chapter 33's accounting for the main model; the MTP layers are left out,
// since floating-point execution does not run them (speculation is a later
// profile). Bytes as stored: BF16, and the F32 correction bias.
// vectors: every rank-1 tensor and the convolution weights. slot: the
// largest matrix that may stream, the readout. expert: one routed expert.
nl_weights :: (s:NlShape)->RpWeights {
    b:=s.base;m:=b.ssm;h:=b.hidden;v:=m.heads*m.head_dim;c:=ss_channels(m);q:=b.attn_heads*b.attn_dim;kv:=b.kv_heads*b.attn_dim;
    p:=nl_pattern(s);M:=p.counts[NH_MAMBA];A:=p.counts[NH_ATTENTION];X:=p.counts[NL_MOE];E:=s.experts;
    expert:=2*s.expert_mlp*h;
    matrices:=M*((v+c+m.heads)*h+h*v)+A*((q+2*kv)*h+h*q)+X*(E*h+2*s.shared_mlp*h+E*expert)+2*b.vocab*h;
    vectors:=M*(3*m.heads+5*c+v+h)+A*h+X*(h+2*E)+h;   // the F32 bias counts as two BF16 words
    return RpWeights.{total=2*(matrices+vectors),vectors=2*vectors,slot=2*b.vocab*h,embedding=2*b.vocab*h,expert=2*expert};
}
// f32 state for capacity tokens: each Mamba-2 layer's convolution history
// (3 rows of channels) and recurrent state, each attention layer's keys and
// values at every position.
nl_state :: (s:NlShape,capacity:int)->int {
    b:=s.base;m:=b.ssm;p:=nl_pattern(s);
    return 4*(p.counts[NH_MAMBA]*(3*ss_channels(m)+ss_states(m))+p.counts[NH_ATTENTION]*capacity*2*b.kv_heads*b.attn_dim);
}
// f32 scratch for chunk tokens: the residual and its normed copy, the
// widest intermediate (in_proj's rows, q k v, or the router and an
// expert), one score row per attention head, and the logits.
nl_scratch :: (s:NlShape,chunk:int,capacity:int)->int {
    b:=s.base;m:=b.ssm;
    widest:=rp_max(m.heads*m.head_dim+ss_channels(m)+m.heads,rp_max((b.attn_heads+2*b.kv_heads)*b.attn_dim,s.experts+rp_max(s.expert_mlp,s.shared_mlp)));
    return 4*(chunk*(2*b.hidden+widest)+b.attn_heads*capacity+b.vocab);
}
// max_position_embeddings.
NL_TOKENS :: 262144;
nl_plan :: (s:NlShape,r:RpRequest,budget:int)->RpPlan {
    if nl_pattern(s).status!=NhStatus.ok {return RpPlan.{status=RpStatus.model};}
    if r.capacity<1 || r.capacity>NL_TOKENS || r.chunk<1 || r.chunk>r.capacity {return RpPlan.{status=RpStatus.request};}
    return rp_admit(nl_weights(s),nl_state(s,r.capacity),nl_scratch(s,r.chunk,r.capacity),r.stream,budget);
}
// Values a decode token multiplies: every matrix but the embedding table
// (one row of it is read), with top_k of the experts in each mixture layer.
nl_active :: (s:NlShape)->int {
    w:=nl_weights(s);p:=nl_pattern(s);
    unused:=p.counts[NL_MOE]*(s.experts-s.top_k)*w.expert;
    return (w.total-w.vectors-w.embedding-unused)/2;
}
// This host, measured: nvidia-smi's memory.total for the RTX 4080 (16,376
// MiB) and free -b's total memory.
NL_HOST_GPU :: 17171480576;
NL_HOST_RAM :: 134828441600;
```

The counts by hand: the weights are 63,155,886,464 bytes. One routed expert
is 2 × 1,856 × 2,688 values, 19,955,712 bytes. A token multiplies every
matrix but the embedding table, with 6 of 128 experts in each MoE layer:
3,226,804,224 values, the "3B active" of the name. The plan still has to
hold all 31.6 billion values, because the next token may choose any expert.

For 4,096 tokens in chunks of 256, the state is 23 Mamba-2 layers of
3 × 6,144 history values and 64 × 64 × 128 state values, plus 6 attention
layers of 4,096 × 2 × 2 × 128 keys and values: 100,261,888 bytes as f32. The
scratch is 17,104,896 bytes. On this host, measured with `nvidia-smi` and
`free -b`:

- The RTX 4080 (17,171,480,576 bytes) refuses the model resident: it needs
  63,273,253,248 bytes, 46,101,772,672 more than it has.
- With streaming, the GPU admits it: 15,644,827,648 bytes of weights stay
  resident, and 47,511,058,816 bytes stream through two slots the size of
  the readout matrix.
- Host memory (134,828,441,600 bytes) holds the whole model resident, with
  71,555,188,352 bytes to spare.

So the verdict is: stream from host memory to the GPU, or run resident on the
CPU. The plan does not yet say how fast either is.

<!-- source: psionic/lib/nemotron-lightning.coax checks-plan -->
```jai
// The weight bytes equal the header survey's: 63,155,886,464 bytes without
// the MTP layers (2,670,652,160 bytes), 1,907,072 of them vectors.
#test "the plan counts every expert, and the host refuses it on the GPU alone" {
    s:=nl_lightning();w:=nl_weights(s);
    #assert w.total==63155886464 && w.total+2670652160==65826538624 && w.vectors==1907072;
    #assert w.slot==704643072 && w.embedding==704643072 && w.expert==19955712 && 23*128*w.expert/2==29374808064;
    // Active per token: 3,227,754,816 values, the card's "3B active"; the
    // memory plan still has to hold all 31.6B.
    #assert nl_active(s)==3226804224;
    #assert nl_state(s,4096)==4*(23*(3*6144+64*64*128)+6*4096*2*2*128);
    #assert nl_scratch(s,256,4096)==4*(256*(2*2688+10304)+32*4096+131072);
    // Narrower Mamba-2 mixers leave q, k and v, then the router and the
    // widest expert, as the widest intermediate.
    d:=s;d.base.ssm.heads=1;d.base.kv_heads=4;#assert nl_scratch(d,1,1)==4*(2*2688+(32+2*4)*128+32+131072);
    d.base.attn_heads=2;#assert nl_scratch(d,1,1)==4*(2*2688+128+3712+2+131072);
    r:=RpRequest.{capacity=4096,chunk=256,stream=true};
    gpu:=nl_plan(s,r,NL_HOST_GPU);live:=nl_state(s,4096)+nl_scratch(s,256,4096);
    #assert gpu.status==RpStatus.ok && gpu.mode==RpMode.streamed && gpu.resident==NL_HOST_GPU-live-2*704643072 && gpu.streamed==w.total-gpu.resident;
    #assert gpu.need==1907072+live+2*704643072;
    r.stream=false;off:=nl_plan(s,r,NL_HOST_GPU);
    #assert off.status==RpStatus.weights && off.mode==RpMode.refused && off.need==w.total+live && off.need-NL_HOST_GPU==46101772672;
    ram:=nl_plan(s,r,NL_HOST_RAM);#assert ram.status==RpStatus.ok && ram.mode==RpMode.resident && ram.resident==w.total;
}
```

## Run it

<!-- source: psionic/examples/nemotron-lightning.coax example -->
```jai
main :: () {
    print("profile nemotron-h-cpu-f64-v1, tolerance % x (1 + largest |reference value|), near-tie margin %\n",NH_TOLERANCE,NL_MARGIN);
    s:=nl_lightning();p:=nl_pattern(s);
    print("pinned revision %, % shard bytes, header only\n",NL_PINNED_REVISION,NL_PINNED_BYTES);
    print("pattern %\n  % Mamba-2, % attention, % mixture layers; MTP layers %\n",s.pattern,p.counts[NH_MAMBA],p.counts[NH_ATTENTION],p.counts[NL_MOE],s.mtp);
    r:=nl_inventory(s,NL_PINNED_MANIFEST);
    print("inventory: % lines for % tensors, % expected, status %, % values in % bytes\n",r.lines,NL_PINNED_TENSORS,nl_expected_count(s),r.status,r.parameters,r.bytes);
    buf:[2048]u8;n:=nl_config_text(s,buf[..]);
    print("config fields match the pinned config.json: %\n",cast(string)buf[0..<n]==NL_PINNED_CONFIG);

    // The hand trace: four experts, gate (0, 2, 1, -1), bias 0.5 on expert 3.
    gate:=[4]f32.[0,2,1,-1];bias:=[4]f32.[0,0,0,0.5];x:=[1]f32.[1];fwork:[8]f64;
    route:=nl_route(gate[..],bias[..],x[..],2,fwork[..]);
    print("hand route: experts % and %, weights % and %, margin %\n",route.ids[0],route.ids[1],route.weights[0],route.weights[1],route.margin);

    // The small Lightning, M * E, seven tokens, against the oracles.
    t:=nlo_shape();
    print("small model: hidden 6; 5 experts of width 3, 2 chosen; shared expert of width 4\n");
    for seed:0..<3 {
        m:=nho_weights(seed);a:=nlo_weights(seed);hidden:=nho_hidden(seed+5);
        h:=hidden;history:[60]f32;state:[24]f32;work:[412]f32;fw:[2000]f64;keys:[56]f32;values:[56]f32;routes:[7]NlRoute;
        r0:=nh_mamba(t.base,nho_mamba_view(m[..]),7,3,h[..],history[..],state[..],work[..],fw[..]);
        r1:=nh_attention(t.base,nho_attention_view(m[..]),7,0,h[..],keys[..],values[..],work[0..<44],fw[0..<7]);
        res:=nl_moe(t,nlo_view(a[..]),7,h[..],routes[..],work[0..<16],fw[0..<16]);
        o:=hidden;ref:=nho_mamba(m[..],o[..]);nlo_round(ref[..],o[..]);
        ref=nho_attention(m[..],o[..]);nlo_round(ref[..],o[..]);
        last:[42]f64;nlo_moe(a[..],o[..],last[..]);g:=nlo_gap(h[..],last[..]);
        tokens:[7]int;same:=0;smallest:=1.0;
        for i:0..<7 {
            logits:[9]f32;normed:[6]f32;ol:[9]f64;
            tokens[i]=nh_readout(nlo_part(a[..],7),nlo_part(a[..],8),h[i*6..<i*6+6],0,logits[..],normed[..]);
            if tokens[i]==nlo_readout(a[..],last[i*6..<i*6+6],ol[..]) {same+=1;}
            if routes[i].margin<smallest {smallest=routes[i].margin;}
        }
        print("  seed %: status %, largest difference % (limit %), greedy tokens %, % of 7 match, smallest router margin %\n",
            seed,ifx r0.written==7 && r1.written==7 then res.status else NhStatus.capacity,g.gap,g.limit,tokens,same,smallest);
    }

    // The memory plan on this host, for 4,096 tokens in chunks of 256.
    w:=nl_weights(s);
    print("weights % bytes without MTP (% vectors); one expert %; % values active a token\n",w.total,w.vectors,w.expert,nl_active(s));
    q:=RpRequest.{capacity=4096,chunk=256,stream=false};
    gpu:=nl_plan(s,q,NL_HOST_GPU);
    print("GPU % bytes, resident: %, need %, short by %\n",NL_HOST_GPU,rp_status_name(gpu.status),gpu.need,gpu.need-NL_HOST_GPU);
    q.stream=true;gpu=nl_plan(s,q,NL_HOST_GPU);
    print("GPU streaming: % %, % bytes resident, % streamed through two slots of %\n",gpu.status,rp_mode_name(gpu.mode),gpu.resident,gpu.streamed,gpu.slot);
    ram:=nl_plan(s,q,NL_HOST_RAM);
    print("RAM % bytes: % %, % bytes to spare\n",NL_HOST_RAM,ram.status,rp_mode_name(ram.mode),NL_HOST_RAM-ram.weights-ram.state-ram.scratch);
}
```

```text
$ coax/bin/coax run psionic/examples/nemotron-lightning.coax
profile nemotron-h-cpu-f64-v1, tolerance 0.00001 x (1 + largest |reference value|), near-tie margin 0.00001
pinned revision a9904d24bcc1d289a1950fa9d2b978c47cf903b9, 65827374264 shard bytes, header only
pattern MEMEM*EMEMEM*EMEMEM*EMEMEM*EMEMEM*EMEMEMEM*EMEMEMEME
  23 Mamba-2, 6 attention, 23 mixture layers; MTP layers *E
inventory: 417 lines for 6513 tensors, 417 expected, status ok, 32913266240 values in 65826538624 bytes
config fields match the pinned config.json: true
hand route: experts 1 and 3, weights 1.9152118 and 0.58478826, margin 0.03788284273999021
small model: hidden 6; 5 experts of width 3, 2 chosen; shared expert of width 4
  seed 0: status ok, largest difference 1.0888519477170178e-06 (limit 0.00007299071481600506), greedy tokens [2, 6, 8, 1, 6, 3, 1], 7 of 7 match, smallest router margin 0.17416040688721435
  seed 1: status ok, largest difference 6.105275947021482e-07 (limit 0.00005268262520012458), greedy tokens [3, 4, 3, 1, 3, 2, 2], 7 of 7 match, smallest router margin 0.047170450041394085
  seed 2: status ok, largest difference 8.055699778353187e-07 (limit 0.00009484047267815843), greedy tokens [2, 2, 5, 1, 2, 5, 7], 7 of 7 match, smallest router margin 0.006951999832841005
weights 63155886464 bytes without MTP (1907072 vectors); one expert 19955712; 3226804224 values active a token
GPU 17171480576 bytes, resident: weights: the weights, state and scratch exceed the budget and streaming is off, need 63273253248, short by 46101772672
GPU streaming: ok streamed, 15644827648 bytes resident, 47511058816 streamed through two slots of 704643072
RAM 134828441600 bytes: ok resident, 71555188352 bytes to spare
```

## Refusals

A config that does not match the headers is refused before any weight is
read, and the refusal names the first expected tensor the headers do not
hold. Here the config says 64 routed experts instead of 128:

<!-- source: psionic/examples/nemotron-lightning-refuse.coax example -->
```jai
// A config that says 64 routed experts instead of 128: the first mixture
// layer's expert line no longer matches the pinned headers. The refusal
// names the first expected tensor the headers do not hold.
main :: () {
    s:=nl_lightning();s.experts=64;
    r:=nl_inventory(s,NL_PINNED_MANIFEST);
    buf:[128]u8;n:=nl_line(nl_expected(s,r.bad),buf[..]);
    print("refused: status %, tensor % not in the pinned headers: %\n",r.status,r.bad,cast(string)buf[0..<n]);
}
```

```text
$ coax/bin/coax run psionic/examples/nemotron-lightning-refuse.coax
refused: status checkpoint, tensor 12 not in the pinned headers: backbone.layers.1.mixer.experts.0..63.down_proj.weight BF16 2688,1856
```

The example exits with status 0 after printing the refusal. The checks also
refuse a missing or extra MTP layer, a moved attention layer, an unknown
layer letter, a repeated or foreign manifest line, more than 1,024 lines,
router shapes outside 1 ≤ k < experts ≤ 256 and k ≤ 8, a bias that is not
finite, and a mixture whose matrices do not match its shape or whose
arithmetic does not fit f32. On a refusal partway through, `written` counts
the tokens already done.

## What this does not do yet

- The original weights: the checkpoint (65.8 GB) is larger than this host's
  disk budget, so no shard was downloaded or hashed, and no original-weight
  slice of a router, an expert or a layer has run.
- The whole 52-layer model, the tokenizer, and token, router and logit
  references for a prompt, including near-tie router margins on real
  inputs.
- Measured memory and tokens per second, resident on the CPU or streamed to
  the GPU.
- The MTP layers' forward pass, NVFP4 weights and speculative decoding.

## Sources

- [Chapter 163](ch163-execute-nemotron-nano.md): the hybrid model's layers.
- [Chapter 61](ch061-a-few-experts-then-a-router.md): a first router.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the memory
  plan.
- [Chapter 17](ch017-qualify-honestly.md): qualification rules.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Nemotron Lightning
  entry.
- transformers 5.19.0, `models/nemotron_h/modeling_nemotron_h.py`
  (`NemotronHTopkRouter`, `NemotronHMoE`, `NemotronHExperts`,
  `NemotronHAttention`), read and not run.
