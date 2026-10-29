# 7. A hybrid sequence model

[Chapter 6](ch06-attention-without-training.md) built a causal attention block.
Qwen3.5 uses two different kinds of decoder layer: a causal gated DeltaNet
layer and a full-attention layer. Both have input normalization, a token mixer,
a residual, another normalization, and a feed-forward residual. Three linear
layers precede each full-attention layer in the pinned Elephant and Mammoth
configurations.

We will build and check each layer in Coax, then connect a small three-to-one
sequence. The code in this chapter is a bounded CPU reference for the
**causal, text-only, single-token** path. It admits the real target dimensions,
but the example uses tiny hand-selected weights. Full checkpoints, the
decision-specific bidirectional mask, and serving are not yet qualified.

## Run a small hybrid

```shell
coax/bin/coax run psionic/examples/hybrid-sequence.coax
psionic/check.sh
```

```jai
position 0: [1.0, 2.0], recurrent states [4.0, 2.0, 1.0]
position 1: [3.0, 4.0], recurrent states [2.0, 1.0, 0.5]
full-attention cache entries: 0.0, 0.0
```

The [example](../examples/hybrid-sequence.coax) uses zero projection and
feed-forward matrices to make the arithmetic visible: each layer preserves
the residual, while the three independent DeltaNet states decay by one half.
The full-attention layer appends zero keys and values to its own cache. Our
independent fixtures also use nonzero projections, gates, rotations and
feed-forward weights over multiple tokens.

## What differs from the teaching block?

A Qwen3.5 layer uses learned **offset** RMS normalization, multiplying each
normalized coordinate by `1 + weight`. In a full-attention layer, the query
projection places query and output-gate coordinates together **per head**;
query and key are normalized again per head. Only part of each head is rotary
encoded, using split halves, and the attended result is multiplied by the
sigmoid of that head's gate. The chapter 6 example used ordinary learned RMS
scales and interleaved pairs. A model-specific implementation must distinguish
these conventions.

The pinned Elephant and Mammoth text configurations use 256-wide
full-attention heads with 64 rotary coordinates, a rotary base of 10,000,000,
and RMS epsilon `1e-6`. Their query-head counts are 16 and 24 respectively;
both use four KV heads. In text-only use the positional axes coincide. This
chapter accepts the per-position cosine/sine table as input; the caller still
owns correct table construction. Vision positions are separate work.

The reference for the operations is the [Transformers 5.3 Qwen3.5 source](https://github.com/huggingface/transformers/blob/v5.3.0/src/transformers/models/qwen3_5/modeling_qwen3_5.py).
Our independent scalar fixtures are authored separately and do not run that
source. The CPU profile uses compensated binary64 products and reductions with
explicit F32 intermediates; it is not a bitwise claim about BF16 or upstream
GPU kernels.

## The linear layer remembers in two places

Project the normalized input to Q/K/V, then apply a four-tap causal depthwise
convolution and SiLU. Each channel carries its last three projected inputs. The
separate value heads share Q/K heads in groups. Within each head, normalize Q
and K with an L2 epsilon of `1e-6`, and scale Q by `1 / sqrt(key_dim)`.

At one time step, with beta from a sigmoid and decay from `A_log`, `dt_bias`
and the projected time parameter, the recurrent matrix changes as follows:

```jai
S_decay = decay * S_old
prediction = transpose(key) * S_decay
delta = beta * (value - prediction)
S_new = S_decay + outer(key, delta)
output = transpose(query) * S_new
```

The per-head result receives RMS normalization, a learned scale, and a SiLU
output gate. It is then projected back to the hidden width. The implementation
holds the current matrix as `[value_head, key_dim, value_dim]` and the
convolution history as three oldest-to-newest samples per channel. For
Elephant it admits 32 value heads; for Mammoth, 48. Both use 16 key heads and
128-wide recurrent coordinates.

<!-- source: psionic/lib/delta.coax step -->
```jai
pd_step :: (s:PdShape,input:[]f32,history:[]f32,state:[]f32,out:[]f32,work:[]f32,overlap:bool)->PdResult #export_c {
    if !pd_valid(s) {return PdResult.{status=PdStatus.shape};}
    c:=pd_channels(s);n:=pd_states(s);v:=s.value_heads*s.value_dim;h:=s.value_heads;
    if input.count!=pd_input_count(s) || history.count!=3*c || state.count!=n || out.count<v || work.count<pd_work_count(s) {return PdResult.{status=PdStatus.capacity};}
    if overlap {return PdResult.{status=PdStatus.alias};}
    if pd_data_status(input)!=PdStatus.ok {return PdResult.{status=PdStatus.number};}
    if pd_data_status(history)!=PdStatus.ok {return PdResult.{status=PdStatus.number};}
    if pd_data_status(state)!=PdStatus.ok {return PdResult.{status=PdStatus.number};}
    aoff:=5*c;boff:=aoff+h;logoff:=boff+h;dtoff:=logoff+h;zoff:=dtoff+h;gammaoff:=zoff+v;
    conv:=work[0..<c];next_history:=work[c..<4*c];next_state:=work[4*c..<4*c+n];draft:=work[4*c+n..<4*c+n+v];
    for i:0..<h {if input[logoff+i]>80 {return PdResult.{status=PdStatus.number};}}
    for channel:0..<c {
        acc:PsionicAccumulator;
        for t:0..<4 {
            item:=input[channel];if t<3 {item=history[channel*3+t];}
            acc=psionic_ops_accumulate(acc,f64(item)*f64(input[c+channel*4+t]));
        }
        if !pd_put(pd_silu(psionic_ops_total(acc)),conv,channel) {return PdResult.{status=PdStatus.arithmetic};}
        next_history[channel*3]=history[channel*3+1];next_history[channel*3+1]=history[channel*3+2];next_history[channel*3+2]=input[channel];
    }
    // Each value head shares the corresponding query/key head.
    for head:0..<h {
        key_head:=head/(h/s.key_heads);qbase:=key_head*s.key_dim;kbase:=pd_key_base(s.key_heads,s.key_dim,qbase);vbase:=2*s.key_heads*s.key_dim+head*s.value_dim;
        qsq:PsionicAccumulator;ksq:PsionicAccumulator;
        for i:0..<s.key_dim {qsq=psionic_ops_accumulate(qsq,f64(conv[qbase+i])*f64(conv[qbase+i]));ksq=psionic_ops_accumulate(ksq,f64(conv[kbase+i])*f64(conv[kbase+i]));}
        qdiv:=sqrt(psionic_ops_total(qsq)+0.000001)*sqrt(f64(s.key_dim));kdiv:=sqrt(psionic_ops_total(ksq)+0.000001);
        beta:=pd_sigmoid(f64(input[boff+head]));decay:=exp(-exp(f64(input[logoff+head]))*pd_softplus(f64(input[aoff+head])+f64(input[dtoff+head])));
        for col:0..<s.value_dim {
            memory:PsionicAccumulator;
            for row:0..<s.key_dim {idx:=(head*s.key_dim+row)*s.value_dim+col;memory=psionic_ops_accumulate(memory,f64(state[idx])*decay*f64(conv[kbase+row])/kdiv);}
            delta:=(f64(conv[vbase+col])-psionic_ops_total(memory))*beta;
            result:PsionicAccumulator;
            for row:0..<s.key_dim {
                idx:=(head*s.key_dim+row)*s.value_dim+col;
                updated:=f64(state[idx])*decay+f64(conv[kbase+row])/kdiv*delta;
                if !pd_put(updated,next_state,idx) {return PdResult.{status=PdStatus.arithmetic};}
                result=psionic_ops_accumulate(result,f64(conv[qbase+row])/qdiv*f64(next_state[idx]));
            }
            if !pd_put(psionic_ops_total(result),draft,head*s.value_dim+col) {return PdResult.{status=PdStatus.arithmetic};}
        }
        squares:PsionicAccumulator;
        for col:0..<s.value_dim {x:=f64(draft[head*s.value_dim+col]);squares=psionic_ops_accumulate(squares,x*x);}
        denom:=sqrt(psionic_ops_total(squares)/f64(s.value_dim)+f64(s.epsilon));
        for col:0..<s.value_dim {
            idx:=head*s.value_dim+col;
            result:=f64(draft[idx])/denom*f64(input[gammaoff+col])*pd_silu(f64(input[zoff+idx]));
            if !pd_put(result,draft,idx) {return PdResult.{status=PdStatus.arithmetic};}
        }
    }
    for i:0..<3*c {history[i]=next_history[i];}
    for i:0..<n {state[i]=next_state[i];}
    for i:0..<v {out[i]=draft[i];}
    return PdResult.{written=v};
}
```

`pd_step` stages the new matrix, history and result in caller scratch; a
refusal leaves all three public spans unchanged. A caller retains the same
weights and ordered history across steps. The reusable cell alone expects
projected inputs; the [linear layer](../lib/qwen-linear-layer.coax) performs
the surrounding input normalization, projections, output projection,
feed-forward path and residuals.

<!-- source: psionic/lib/qwen-linear-layer.coax layer -->
```jai
ql_step :: (s:QlShape,w:QlWeights,x:[]f32,history:[]f32,state:[]f32,out:[]f32,work:[]f32,overlap:bool)->QlResult #export_c {
    if !ql_valid(s) {return QlResult.{status=QlStatus.shape};}
    d:=s.width;f:=s.hidden;c:=pd_channels(s.delta);n:=pd_states(s.delta);v:=s.delta.value_heads*s.delta.value_dim;h:=s.delta.value_heads;
    if !ql_weights_valid(s,w) || x.count!=d || history.count!=3*c || state.count!=n || out.count<d || work.count<ql_work_count(s) {return QlResult.{status=QlStatus.capacity};}
    if overlap {return QlResult.{status=QlStatus.alias};}
    layout:=ql_layout(s);norm:=work[layout.norm..<layout.temp];temp:=work[layout.temp..<layout.packed];
    packed:=work[layout.packed..<layout.history];next_history:=work[layout.history..<layout.state];next_state:=work[layout.state..<layout.delta_out];
    delta_out:=work[layout.delta_out..<layout.delta_work];delta_work:=work[layout.delta_work..<layout.mixed];
    mixed:=work[layout.mixed..<layout.draft];draft:=work[layout.draft..<layout.block_work];block_work:=work[layout.block_work..<layout.total];
    norm_result:=qb_begin(d,s.delta.epsilon,x,w.input_norm,norm,temp,false);
    if norm_result.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(norm_result.status)};}
    qkv:=qm_project(s.dtype,c,d,w.qkv,norm,packed[0..<c],temp,false);if qkv.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(qkv.status)};}
    aoff:=5*c;boff:=aoff+h;logoff:=boff+h;dtoff:=logoff+h;zoff:=dtoff+h;gammaoff:=zoff+v;
    for i:0..<4*c {packed[c+i]=w.conv[i];}
    for i:0..<h {packed[logoff+i]=w.a_log[i];packed[dtoff+i]=w.dt[i];}
    for i:0..<s.delta.value_dim {packed[gammaoff+i]=w.delta_norm[i];}
    ar:=qm_project(s.dtype,h,d,w.a,norm,packed[aoff..<aoff+h],temp,false);if ar.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(ar.status)};}
    br:=qm_project(s.dtype,h,d,w.b,norm,packed[boff..<boff+h],temp,false);if br.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(br.status)};}
    zr:=qm_project(s.dtype,v,d,w.z,norm,packed[zoff..<zoff+v],temp,false);if zr.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(zr.status)};}
    for i:0..<3*c {next_history[i]=history[i];}for i:0..<n {next_state[i]=state[i];}
    dr:=pd_step(s.delta,packed,next_history,next_state,delta_out,delta_work,false);if dr.status!=PdStatus.ok {return QlResult.{status=ql_dstatus(dr.status)};}
    mr:=qm_project(s.dtype,d,v,w.mixer_out,delta_out,mixed,temp,false);if mr.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(mr.status)};}
    end:=qb_finish(s.dtype,d,f,s.delta.epsilon,x,mixed,w.post_norm,w.gate,w.up,w.down,draft,block_work,false);if end.status!=QmStatus.ok {return QlResult.{status=ql_mstatus(end.status)};}
    for i:0..<3*c {history[i]=next_history[i];}for i:0..<n {state[i]=next_state[i];}for i:0..<d {out[i]=draft[i];}
    return QlResult.{written=d};
}
```

A late failure in the feed-forward matrix must not advance the recurrence.
The layer therefore copies both states into draft storage and publishes them
only after the whole layer succeeds. Scratch can change on refusal.

## Full attention still needs a KV cache

The full layer projects normalized input to Q/gate, K and V. It normalizes Q/K,
rotates the selected split halves, appends K/V, attends to the causal prefix,
and gates each attended coordinate before its output projection. The cache
stores each position's keys followed by its values; it is a different kind of
state from DeltaNet's fixed-size matrix and convolution history.

<!-- source: psionic/lib/qwen-full-layer.coax layer -->
```jai
qh_step :: (s:QhShape,w:QhWeights,x:[]f32,rope:[]f32,position:int,cache:[]f32,out:[]f32,work:[]f32,overlap:bool)->QhResult #export_c {
    if !qh_valid(s) {return QhResult.{status=QhStatus.shape};}
    if position<0 || position>=s.capacity {return QhResult.{status=QhStatus.capacity};}
    d:=s.width;f:=s.hidden;q:=s.attention.heads*s.attention.head_dim;k:=s.attention.kv_heads*s.attention.head_dim;dim:=s.attention.head_dim;tokens:=position+1;
    p:=qh_layout(s,position);
    if !qh_weights_valid(s,w) || x.count!=d || rope.count!=s.attention.rotary_dim || cache.count!=qh_cache_count(s) || out.count<d || work.count<p.total {return QhResult.{status=QhStatus.capacity};}
    if overlap {return QhResult.{status=QhStatus.alias};}
    norm:=work[p.norm..<p.temp];temp:=work[p.temp..<p.prepare];prep:=work[p.prepare..<p.prepared];prepared:=work[p.prepared..<p.attend];
    attend:=work[p.attend..<p.att_out];att_out:=work[p.att_out..<p.att_work];att_work:=work[p.att_work..<p.mixed];mixed:=work[p.mixed..<p.draft];draft:=work[p.draft..<p.block];block:=work[p.block..<p.total];
    b:=qb_begin(d,s.attention.epsilon,x,w.input_norm,norm,temp,false);if b.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(b.status)};}
    kstart:=2*q;vstart:=kstart+k;scalebase:=vstart+k;
    qr:=qm_project(s.dtype,kstart,d,w.q,norm,prep[0..<kstart],temp,false);if qr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(qr.status)};}
    kr:=qm_project(s.dtype,k,d,w.k,norm,prep[kstart..<vstart],temp,false);if kr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(kr.status)};}
    vr:=qm_project(s.dtype,k,d,w.v,norm,prep[vstart..<scalebase],temp,false);if vr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(vr.status)};}
    for i:0..<dim {prep[scalebase+i]=w.q_norm[i];prep[scalebase+dim+i]=w.k_norm[i];}
    for i:0..<s.attention.rotary_dim {prep[qh_rope_offset(scalebase,dim)+i]=rope[i];}
    pr:=qf_prepare(s.attention,prep,prepared,false);if pr.status!=QfStatus.ok {return QhResult.{status=qh_fstatus(pr.status)};}
    for i:0..<kstart {attend[i]=prepared[i];}
    for t:0..<tokens {
        for i:0..<k {
            key:=prepared[2*q+i];value:=prepared[2*q+k+i];
            if t<position {key=cache[t*2*k+i];value=cache[t*2*k+k+i];}
            attend[2*q+t*k+i]=key;attend[2*q+tokens*k+t*k+i]=value;
        }
    }
    mask:[16384]u8;for i:0..<tokens {mask[i]=1;}
    ar:=qf_attend(s.attention,tokens,attend,mask[0..<tokens],att_out,att_work,false);if ar.status!=QfStatus.ok {return QhResult.{status=qh_fstatus(ar.status)};}
    mr:=qm_project(s.dtype,d,q,w.mixer_out,att_out,mixed,temp,false);if mr.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(mr.status)};}
    end:=qb_finish(s.dtype,d,f,s.attention.epsilon,x,mixed,w.post_norm,w.gate,w.up,w.down,draft,block,false);if end.status!=QmStatus.ok {return QhResult.{status=qh_mstatus(end.status)};}
    for i:0..<k {cache[position*2*k+i]=prepared[2*q+i];cache[position*2*k+k+i]=prepared[2*q+k+i];}
    for i:0..<d {out[i]=draft[i];}return QhResult.{written=d};
}
```

The current API processes one causal token and stages its new KV entry until
all later calculations succeed. It takes an explicit rotary table and assumes
that the caller passes the correct position and prior cache. A decision-model
prefix and suffix require a different block-bidirectional full-attention mask;
that is chapter 8. DeltaNet remains causal even when full attention uses that
mask.

Both layers use [original BF16/F32 row projections](../lib/qwen-matvec.coax)
and the shared [offset-RMS and feed-forward tail](../lib/qwen-block.coax).
Matrices are row-major and have no biases in this profile. Scalar vectors such
as norm and convolution weights arrive as F32 views; the original BF16 decoder
can provide them. Admission checks dimensions, exact input lengths, finite
values, output capacities and writable alias observations. Each CPU product
uses wide accumulation; computed non-finite or out-of-range values refuse.

## Compose the two states

<!-- source: psionic/examples/hybrid-sequence.coax example -->
```jai
main :: () {
    linear:=QlShape.{width=2,hidden=1,delta=PdShape.{key_heads=1,value_heads=1,key_dim=1,value_dim=1,epsilon=1},dtype=QmDtype.bf16};
    full:=QhShape.{width=2,hidden=1,capacity=2,attention=QfShape.{heads=1,kv_heads=1,head_dim=2,rotary_dim=2,epsilon=1},dtype=QmDtype.bf16};
    zero:[16]u8;norm:[2]f32;conv:[12]f32;scalar:[1]f32;gain:=[1]f32.[1];
    lw:=QlWeights.{input_norm=norm[..],post_norm=norm[..],conv=conv[..],a_log=scalar[..],dt=scalar[..],delta_norm=gain[..],qkv=zero[0..<12],z=zero[0..<4],a=zero[0..<4],b=zero[0..<4],mixer_out=zero[0..<4],gate=zero[0..<4],up=zero[0..<4],down=zero[0..<4]};
    fw:=QhWeights.{input_norm=norm[..],post_norm=norm[..],q_norm=norm[..],k_norm=norm[..],q=zero[..],k=zero[0..<8],v=zero[0..<8],mixer_out=zero[0..<8],gate=zero[0..<4],up=zero[0..<4],down=zero[0..<4]};
    tokens:=[4]f32.[1,2,3,4];rope:=[2]f32.[1,0];history:[27]f32;states:=[3]f32.[8,4,2];cache:[8]f32;vector:[2]f32;next:[2]f32;work:[128]f32;
    for position:0..<2 {
        for j:0..<2 {vector[j]=tokens[position*2+j];}
        for layer:0..<3 {
            r:=ql_step(linear,lw,vector[..],history[layer*9..<(layer+1)*9],states[layer..<layer+1],next[..],work[..],false);
            #assert r.status==QlStatus.ok;for j:0..<2 {vector[j]=next[j];}
        }
        r:=qh_step(full,fw,vector[..],rope[..],position,cache[..],next[..],work[..],false);
        #assert r.status==QhStatus.ok;
        print("position %: [%, %], recurrent states [%, %, %]\n",position,next[0],next[1],states[0],states[1],states[2]);
    }
    print("full-attention cache entries: %, %\n",cache[0],cache[4]);
}
```

Each of the three linear layers owns its own convolution and recurrent state;
the fourth owns a KV cache. A prefix snapshot must retain all of them, not only
the KV cache. The example calls each layer in order. It does not yet implement
a general model loader or branch scheduler.

[The verification record](../../docs/verification/psionic-ch07-2026-10-29.md)
reports the independent tiny full-layer fixtures, zero-survivor mutation gates,
native debug/release and sanitizer runs, and CoderOS CPU checks. These are
mathematical and storage checks on this named CPU profile. Captured upstream
intermediate tensors, full original-weight execution and actual request-level
model behavior remain later gates. Full attention's decision mask, isolated
prefix forks and readout are the next steps.
