# 8. Decisions have a different mask

A normal causal decoder lets each token attend only to itself and earlier
tokens. The pinned decision models use a different rule for their full-attention
layers: every token can see the **whole block** it belongs to, plus earlier
blocks. The shared state prefix is one block; each question suffix is its own
later block. A suffix cannot affect the cached prefix or a sibling suffix.
The DeltaNet and convolution layers remain causal.

[Chapter 7](ch07-a-hybrid-sequence-model.md) gave each layer its own state.
Here we evaluate a full-attention block and fork every kind of state at the
prefix boundary. We use bounded, synthetic CPU examples; real-weight execution
and complete end-to-end decision qualification remain later work.

## Run two independent branches

```shell
coax/bin/coax run psionic/examples/decision-forks.coax
psionic/check.sh
```

```jai
prefix: [1.0, 2.0, 3.0, 4.0], recurrent state 1.0
branch 0: [5.0, 6.0], recurrent state 0.5
branch 1: [-1.0, -2.0], recurrent state 0.5
shared prefix unchanged: 1.0; independent forks: 2
```

The [program](../examples/decision-forks.coax) uses zero matrices to make the
state changes easy to see. It first processes a shared prefix, copies the
prefix's linear and attention state into two independent branches, and then
processes each suffix. The independent numerical fixture also uses nonzero
weights and checks the bidirectional prefix and both different suffixes.

## A block is not a causal triangle

For a prefix of length `P` followed by one suffix of length `S`, the visible
positions are:

```jai
prefix query: keys [0, P)
suffix query: keys [0, P + S)
```

Every query in either block sees all keys in its own block. A prefix query
never sees the suffix. Each suffix is evaluated separately from the same
prefix snapshot, so it cannot see another suffix. This follows the pinned
[Elephant serving patch](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/serve.patch).
Padding, batched branches and other model contracts require their own checks.

A causal one-token API cannot implement the bidirectional prefix by simply
changing its mask: the future keys in that block do not exist yet. We first
normalize and project **every** input in the current block and prepare all of
its rotated keys and values. Then each query attends to the earlier cached
prefix and all newly prepared keys. Finally we finish every residual/FFN and
publish the entire output and new cache entries together.

<!-- source: psionic/lib/qwen-decision.coax block -->
```jai
qd_block :: (s:QhShape,w:QhWeights,x:[]f32,rope:[]f32,start:int,count:int,cache:[]f32,out:[]f32,work:[]f32,overlap:bool)->QhResult #export_c {
    if !qh_valid(s) {return QhResult.{status=QhStatus.shape};}
    if !qd_block_valid(start,count,s.capacity) {return QhResult.{status=QhStatus.capacity};}
    d:=s.width;f:=s.hidden;q:=s.attention.heads*s.attention.head_dim;k:=s.attention.kv_heads*s.attention.head_dim;dim:=s.attention.head_dim;rot:=s.attention.rotary_dim;tokens:=start+count;
    p:=qd_layout(s,start,count);one:=qf_output_count(s.attention);
    if !qh_weights_valid(s,w) || x.count!=count*d || rope.count!=count*rot || cache.count!=qh_cache_count(s) || out.count<count*d || work.count<p.total {return QhResult.{status=QhStatus.capacity};}
    if overlap {return QhResult.{status=QhStatus.alias};}
    norm:=work[p.norm..<p.temp];temp:=work[p.temp..<p.prepare];prep:=work[p.prepare..<p.prepared];prepared:=work[p.prepared..<p.attend];
    attend:=work[p.attend..<p.att_out];att_out:=work[p.att_out..<p.att_work];att_work:=work[p.att_work..<p.mixed];mixed:=work[p.mixed..<p.draft];draft:=work[p.draft..<p.block];block:=work[p.block..<p.total];
    kstart:=2*q;vstart:=kstart+k;scalebase:=vstart+k;
    for t:0..<count {
        row:=x[t*d..<(t+1)*d];br:=qb_begin(d,s.attention.epsilon,row,w.input_norm,norm,temp,false);if br.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(br.status)};}
        qr:=qm_project(s.dtype,kstart,d,w.q,norm,prep[0..<kstart],temp,false);if qr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(qr.status)};}
        kr:=qm_project(s.dtype,k,d,w.k,norm,prep[kstart..<vstart],temp,false);if kr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(kr.status)};}
        vr:=qm_project(s.dtype,k,d,w.v,norm,prep[vstart..<scalebase],temp,false);if vr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(vr.status)};}
        for i:0..<dim {prep[scalebase+i]=w.q_norm[i];prep[scalebase+dim+i]=w.k_norm[i];}
        for i:0..<rot {prep[qh_rope_offset(scalebase,dim)+i]=rope[t*rot+i];}
        pr:=qf_prepare(s.attention,prep,prepared[t*one..<(t+1)*one],false);if pr.status!=QfStatus.ok {return QhResult.{status=qh_fstatus(pr.status)};}
    }
    for t:0..<tokens {
        for i:0..<k {
            key:f32=0;value:f32=0;
            if t<start {key=cache[t*2*k+i];value=cache[t*2*k+k+i];}
            else {row:=(t-start)*one;key=prepared[row+2*q+i];value=prepared[row+2*q+k+i];}
            attend[2*q+t*k+i]=key;attend[2*q+tokens*k+t*k+i]=value;
        }
    }
    mask:[16384]u8;for i:0..<tokens {mask[i]=1;}
    for t:0..<count {
        for i:0..<2*q {attend[i]=prepared[t*one+i];}
        ar:=qf_attend(s.attention,tokens,attend,mask[0..<tokens],att_out,att_work,false);if ar.status!=QfStatus.ok {return QhResult.{status=qh_fstatus(ar.status)};}
        mr:=qm_project(s.dtype,d,q,w.mixer_out,att_out,mixed,temp,false);if mr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(mr.status)};}
        end:=qb_finish(s.dtype,d,f,s.attention.epsilon,x[t*d..<(t+1)*d],mixed,w.post_norm,w.gate,w.up,w.down,draft[t*d..<(t+1)*d],block,false);if end.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(end.status)};}
    }
    for t:0..<count {for i:0..<k {cache[(start+t)*2*k+i]=prepared[t*one+2*q+i];cache[(start+t)*2*k+k+i]=prepared[t*one+2*q+k+i];}}
    for i:0..<count*d {out[i]=draft[i];}return QhResult.{written=count*d};
}
```

`start` is the number of existing prefix positions and `count` is the current
block length. The caller supplies exactly `count` input vectors and rotary
rows. The output is staged for all rows, and the cache is changed only if the
whole block succeeds. Scratch may be dirty after a failure. A caller must keep
cache, position, weights, shape and rotary convention consistent. The
implementation admits up to the 16,384-position configured ceiling but that
bound alone does not qualify whole-model memory or latency at that length.

## Snapshot the whole hybrid state

A snapshot needs more than KV. Every linear layer owns a convolution history
and recurrent matrix; every full-attention layer owns a KV allocation. The
snapshot profile lays them out as:

```jai
[linear layer 0: convolution history, recurrent matrix]
[linear layer 1: convolution history, recurrent matrix]
...
[full layer 0: KV capacity]
[full layer 1: KV capacity]
...
```

At a prefix boundary, copy every linear state and only the **live prefix** of
each full layer's KV capacity. The unused KV tails in a destination stay
unchanged. The returned claim includes the prefix length, the exact shape and
a nonzero 32-byte caller-supplied model identity. The fork compares these
against the expected identity and shape before touching destination storage.
It does not independently authenticate the identity or prove that the cache
was generated from those weights; that remains the model admission and caller
contract.

<!-- source: psionic/lib/qwen-prefix.coax fork -->
```jai
qp_fork :: (shape:QpShape,model:[32]u8,source:QpClaim,src:[]f32,dst:[]f32,scratch:[]f32,overlap:bool)->QpResult #export_c {
    if !qp_valid(shape) {return QpResult.{status=QpStatus.shape};}
    if !qp_same_shape(shape,source.shape) || !qp_identity(model,source.model) {return QpResult.{status=QpStatus.identity};}
    if source.prefix<0 || source.prefix>shape.capacity {return QpResult.{status=QpStatus.capacity};}
    total:=qp_total(shape);live:=qp_live(shape,source.prefix);
    if src.count!=total || dst.count<total || scratch.count<live {return QpResult.{status=QpStatus.capacity};}
    if overlap {return QpResult.{status=QpStatus.alias};}
    linear:=shape.linear_layers*qp_linear_count(shape);full:=qp_full_count(shape);prefix:=qp_live_full(shape,source.prefix);
    for i:0..<linear {if !psionic_tensor_finite_bits(f32_bits(src[i])) {return QpResult.{status=QpStatus.number};}scratch[i]=src[i];}
    for layer:0..<shape.full_layers {base:=linear+layer*full;target:=linear+layer*prefix;for i:0..<prefix {if !psionic_tensor_finite_bits(f32_bits(src[qp_index(base,i)])) {return QpResult.{status=QpStatus.number};}scratch[target+i]=src[base+i];}}
    for i:0..<linear {dst[i]=scratch[i];}
    for layer:0..<shape.full_layers {base:=linear+layer*full;target:=linear+layer*prefix;for i:0..<prefix {dst[base+i]=scratch[target+i];}}
    return QpResult.{written=live,claim=source};
}
```

The snapshot destination and scratch must be disjoint from the original and
from each other. Live values must be finite. Scratch can contain partial data
on refusal; destination and returned success claim are published only on
success. The native adapter observes ranges; truthful extents, lifetimes,
alignment and exclusive access remain caller obligations.

## Put the pieces together

<!-- source: psionic/examples/decision-forks.coax example -->
```jai
main :: () {
    linear:=QlShape.{width=2,hidden=1,delta=PdShape.{key_heads=1,value_heads=1,key_dim=1,value_dim=1,epsilon=1},dtype=QmDtype.bf16};
    full:=QhShape.{width=2,hidden=1,capacity=3,attention=QfShape.{heads=1,kv_heads=1,head_dim=2,rotary_dim=2,epsilon=1},dtype=QmDtype.bf16};
    bytes:[16]u8;norm:[2]f32;conv:[12]f32;scalar:[1]f32;gain:=[1]f32.[1];
    lw:=QlWeights.{input_norm=norm[..],post_norm=norm[..],conv=conv[..],a_log=scalar[..],dt=scalar[..],delta_norm=gain[..],qkv=bytes[0..<12],z=bytes[0..<4],a=bytes[0..<4],b=bytes[0..<4],mixer_out=bytes[0..<4],gate=bytes[0..<4],up=bytes[0..<4],down=bytes[0..<4]};
    fw:=QhWeights.{input_norm=norm[..],post_norm=norm[..],q_norm=norm[..],k_norm=norm[..],q=bytes[..],k=bytes[0..<8],v=bytes[0..<8],mixer_out=bytes[0..<8],gate=bytes[0..<4],up=bytes[0..<4],down=bytes[0..<4]};
    shape:=QpShape.{linear_layers=1,full_layers=1,capacity=3,delta=linear.delta,attention=full.attention};id:[32]u8;id[0]=1;
    state:[22]f32;state[9]=4;prefix:=[4]f32.[1,2,3,4];linear_out:[4]f32;rope:=[4]f32.[1,0,1,0];out:[4]f32;work:[200]f32;
    for t:0..<2 {
        r:=ql_step(linear,lw,prefix[t*2..<(t+1)*2],state[0..<9],state[9..<10],linear_out[t*2..<(t+1)*2],work[..],false);#assert r.status==QlStatus.ok;
    }
    p:=qd_block(full,fw,linear_out[..],rope[..],0,2,state[10..<22],out[..],work[..],false);#assert p.status==QhStatus.ok;
    #assert state[9]==1;print("prefix: [%, %, %, %], recurrent state %\n",out[0],out[1],out[2],out[3],state[9]);
    claim:=QpClaim.{model=id,prefix=2,shape=shape};a:[22]f32;b:[22]f32;scratch:[18]f32;
    fa:=qp_fork(shape,id,claim,state[..],a[..],scratch[..],false);fb:=qp_fork(shape,id,claim,state[..],b[..],scratch[..],false);
    #assert fa.status==QpStatus.ok && fb.status==QpStatus.ok && fa.written==18;
    suffix:=[4]f32.[5,6,-1,-2];one_rope:=rope[0..<2];tmp:[2]f32;
    for branch:0..<2 {
        fork:=a[..];if branch==1 {fork=b[..];}
        l:=ql_step(linear,lw,suffix[branch*2..<(branch+1)*2],fork[0..<9],fork[9..<10],tmp[..],work[..],false);#assert l.status==QlStatus.ok;
        f:=qd_block(full,fw,tmp[..],one_rope,2,1,fork[10..<22],out[0..<2],work[..],false);#assert f.status==QhStatus.ok;
        print("branch %: [%, %], recurrent state %\n",branch,out[0],out[1],fork[9]);
    }
    #assert state[9]==1 && a[9]==0.5f32 && b[9]==0.5f32;
    print("shared prefix unchanged: %; independent forks: 2\n",state[9]);
}
```

The program keeps the prefix source separate from the two copies. Each suffix
updates its own convolution/recurrent state and KV cache. A real model will
perform that sequence for every layer and must retain the exact token split
from [chapter 5](ch05-text-becomes-a-model-request.md). A block-level test and
snapshot operation alone do not establish whole-model branch isolation.

The [verification record](../../docs/verification/psionic-ch08-2026-10-29.md)
records independent nonzero block references, the snapshot boundaries, Coax
mutation checks and native CPU runs on both machines. Slot readout, complete
checkpoint admission and original-weight end-to-end serving remain next.
