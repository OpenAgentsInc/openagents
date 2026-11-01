# 162. A recurrence you can inspect

State-space models such as Mamba-2 carry a fixed-size state from token to
token instead of attending to every earlier token. This chapter builds a tiny
Mamba-2 style layer and runs it three ways: one token at a time, in chunks,
and as one matrix over the whole sequence. All three give the same outputs
within a named numerical profile, and a chunk of one token gives the token
step's result bit for bit.

The layer is small enough to inspect. You can read each head's step size and
how much of its state it keeps per token, the whole matrix that says how much
each earlier input reaches each output, and the state at every chunk
boundary. The checks compare each of these readouts with a closed form or with
a separate binary64 reference.
[Chapter 11](ch011-a-hybrid-sequence-model.md)'s Gated DeltaNet support does
not establish Mamba-2 support, so this chapter starts from a small example
rather than from a model's weights.

```shell
coax/bin/coax run psionic/examples/ssm.coax
coax/bin/coax test psionic/lib/ssm.coax
```

```text
profile ssm-cpu-f64-v1, tolerance 0.00001 x (1 + largest |reference value|)
kept through each token 0.5, 0.25; token 0's write kept through token 1 0.5
mixing matrix [0.5, 0.0; 0.5, 1.0]; outputs 2.5, 7.5; carried state 3.75
13 tokens, 4 heads of width 2 in 2 groups, state width 3:
  log A -4.0: each token keeps between 0.9495932350040605 and 0.9959810650243408 of the state
    chunk 1: outputs 1.2513778513412177e-08, state 7.813031022152472e-08, snapshots 1.0996067811852583e-07 (13 of them, histories equal true), step bit for bit true
    chunk 2: outputs 3.325652581098737e-08, state 7.813031022152472e-08, snapshots 7.813031022152472e-08 (7 of them, histories equal true), step bit for bit false
    chunk 3: outputs 1.2513778513412177e-08, state 7.813031022152472e-08, snapshots 7.813031022152472e-08 (5 of them, histories equal true), step bit for bit false
    chunk 4: outputs 2.7414939707259833e-08, state 5.320083557025512e-08, snapshots 9.459774075182281e-08 (4 of them, histories equal true), step bit for bit false
    chunk 5: outputs 1.9705715267881452e-08, state 1.09978593343385e-07, snapshots 1.09978593343385e-07 (3 of them, histories equal true), step bit for bit false
    chunk 8: outputs 2.7414939707259833e-08, state 4.364375683074684e-08, snapshots 6.17624258403282e-08 (2 of them, histories equal true), step bit for bit false
    chunk 13: outputs 1.6411539266458774e-08, state 4.364375683074684e-08, snapshots 4.364375683074684e-08 (1 of them, histories equal true), step bit for bit false
    chunk 64: outputs 1.6411539266458774e-08, state 4.364375683074684e-08, snapshots 4.364375683074684e-08 (1 of them, histories equal true), step bit for bit false
    whole sequence: outputs 1.6411539266458774e-08, state 4.364375683074684e-08, mixing matrix 1.7488975101453974e-08 (limit 0.000014403773717122286), zero above the diagonal true
  log A 1.0: each token keeps between 0.00046375225677373867 and 0.5500948122646999 of the state
    chunk 1: outputs 1.6093639476633115e-08, state 9.240374937435902e-09, snapshots 2.9118943756234472e-08 (13 of them, histories equal true), step bit for bit true
    chunk 2: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, snapshots 2.9118943756234472e-08 (7 of them, histories equal true), step bit for bit false
    chunk 3: outputs 1.6093639476633115e-08, state 9.240374937435902e-09, snapshots 2.9118943756234472e-08 (5 of them, histories equal true), step bit for bit false
    chunk 4: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, snapshots 2.260044623669799e-08 (4 of them, histories equal true), step bit for bit false
    chunk 5: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, snapshots 1.1417310208372555e-08 (3 of them, histories equal true), step bit for bit false
    chunk 8: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, snapshots 9.66735941299035e-09 (2 of them, histories equal true), step bit for bit false
    chunk 13: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, snapshots 9.240374937435902e-09 (1 of them, histories equal true), step bit for bit false
    chunk 64: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, snapshots 9.240374937435902e-09 (1 of them, histories equal true), step bit for bit false
    whole sequence: outputs 6.1111742333253005e-09, state 9.240374937435902e-09, mixing matrix 1.104397034934479e-08 (limit 0.000013166591813826473), zero above the diagonal true
log A 1, head 0's mixing matrix, tokens 0 to 3:
  -0.0361646434134415 0.0 0.0 0.0
  -0.002786749442908085 -0.06775848766197574 0.0 0.0
  0.00028853892567490676 -0.0048821442303570205 0.06202647813957964 0.0
  0.0003761058768097783 0.0453615986105941 0.052925812687482456 0.1420849183647983
resumed at token 8 from a snapshot: outputs, state and history bit for bit true
request b after request a without a clear: equal to b alone false
request b after a clear: equal to b alone true
refused: chunk 0: range, output -7.0
refused: an infinite input at token 5: number, output -7.0, written 0
refused: a convolution past f32 at token 9: arithmetic, tokens committed 8, snapshots 2, state equals snapshot 2 true
```

The [library](../lib/ssm.coax), its [check oracle](../tests/ssm-oracle.coax)
and the [example](../examples/ssm.coax) are the whole implementation. Inputs
and parameters are synthetic; no model weights are read.

## The recurrence, one token at a time

A Mamba-2 layer splits its channels into heads. Each head has an input `x`
with `head_dim` coordinates. Heads share their **B** (write) and **C** (read)
vectors, each `state_dim` wide, within a group. The state of one head is a
`head_dim x state_dim` matrix `S`. For one token, with step size `dt` and the
head's decay parameter `A` (a negative number):

```text
keep   = exp(dt A)
S'     = keep S + dt x B^T
y      = S' C + D x
output = y silu(z)
```

`keep` is between 0 and 1: the fraction of the old state that survives this
token. `dt` comes from the token (`dt = softplus(raw dt + dt bias)`), so each
token chooses how much to forget. `D` is a skip weight and `z` is an output
gate. Before the recurrence, `x`, `B` and `C` pass through a causal
convolution of width 4 with a bias and a silu, as in Mamba-2. The convolution
keeps the last three raw inputs of every channel as its own carried state,
like chapter 11's.

Unlike chapter 11's delta rule, this update does not read the state before
writing it: each token's write `dt x B^T` depends only on that token. That is
what makes the whole sequence a single matrix product.

## Work two tokens by hand

Take one input coordinate and one state coordinate, so `S` is a number. Start
from `S0 = 2`. Both tokens have `dt = 0.5`, `keep = 0.5` and `B = 1`. Their
inputs are `x = 3` and `x = 5`, and their read vectors are `C = 1` and `C = 2`.
Leave out `D` and the gate for now.

Token by token:

```text
token 0: S1 = 0.5 * 2   + 0.5 * 3 * 1 = 2.5     y0 = 1 * 2.5  = 2.5
token 1: S2 = 0.5 * 2.5 + 0.5 * 5 * 1 = 3.75    y1 = 2 * 3.75 = 7.5
```

Now unroll the recurrence. Write `G_t` for what the start state keeps through
token `t`, and `R_{t,i}` for what token `i`'s write keeps through token `t`:

```text
G_t      = keep_0 keep_1 ... keep_t
R_{t,i}  = keep_{i+1} ... keep_t        (1 when i = t)
y_t      = G_t C_t . S0 + sum over i <= t of M_{t,i} x_i
M_{t,i}  = R_{t,i} (C_t . B_i) dt_i     (0 when i > t)
```

Here `G = 0.5, 0.25` and `R_{1,0} = 0.5`, so:

```text
M_{0,0} = 1   * (1 * 1) * 0.5 = 0.5      M_{0,1} = 0
M_{1,0} = 0.5 * (2 * 1) * 0.5 = 0.5      M_{1,1} = 1 * (2 * 1) * 0.5 = 1

y0 = 0.5 * 1 * 2  + 0.5 * 3           = 1 + 1.5     = 2.5
y1 = 0.25 * 2 * 2 + 0.5 * 3 + 1.0 * 5 = 1 + 1.5 + 5 = 7.5
S2 = 0.25 * 2 + 0.5 * 0.5 * 3 * 1 + 1 * 0.5 * 5 * 1 = 0.5 + 0.75 + 2.5 = 3.75
```

These are the same numbers, and no intermediate state was built. `M` is the
recurrence written as a lower-triangular matrix: row `t` says how much of
each earlier input reaches output `t`. The example's second and third lines
print `G`, `R_{1,0}`, `M`, the outputs and the carried state from the code
below, and a check asserts each of them.

## Layout

<!-- source: psionic/lib/ssm.coax types -->
```jai
SsShape :: struct { heads:int; head_dim:int; state_dim:int; groups:int; }
SsStatus :: enum u8 { ok; shape; range; capacity; alias; number; arithmetic; }
// written: tokens whose outputs, state and history were committed.
// snapshots: chunk-boundary snapshots written.
SsResult :: struct { status:SsStatus; written:int; snapshots:int; }
// One head's chunk: tokens rows of stride convolved channels; x, b and c are
// where this head's input, its group's B and its group's C start in a row.
SsChunk :: struct { head_dim:int; state_dim:int; tokens:int; stride:int; x:int; b:int; c:int; }
```

<!-- source: psionic/lib/ssm.coax layout -->
```jai
// Convolved channels per token: x (heads x head_dim), then B and C
// (groups x state_dim each).
ss_valid :: (s:SsShape)->bool {
    if s.heads>128 || s.head_dim<1 || s.head_dim>128 || s.state_dim<1 || s.state_dim>256 {return false;}
    // groups >= 1 and groups <= heads also bound heads below.
    return s.groups>=1 && s.groups<=s.heads && s.heads%s.groups==0;
}
ss_channels :: (s:SsShape)->int {return s.heads*s.head_dim+2*s.groups*s.state_dim;}
ss_states :: (s:SsShape)->int {return s.heads*s.head_dim*s.state_dim;}
// One token's input: the raw channels, the gate z (heads x head_dim), raw dt (heads).
ss_input_count :: (s:SsShape)->int {return ss_channels(s)+s.heads*s.head_dim+s.heads;}
// Parameters: 4 convolution taps per channel (oldest first), a bias per
// channel, then log A, the dt bias and the skip D, one per head.
ss_param_count :: (s:SsShape)->int {return 5*ss_channels(s)+3*s.heads;}
// One snapshot is the convolution history (3 per channel) and then the state.
ss_snapshot_size :: (s:SsShape)->int {return 3*ss_channels(s)+ss_states(s);}
ss_chunks :: (tokens:int,chunk:int)->int {return (tokens+chunk-1)/chunk;}
ss_scratch_count :: (chunk:int)->int {return 2*chunk*chunk+chunk;}
ss_step_work_count :: (s:SsShape)->int {return 4*ss_channels(s)+ss_states(s)+s.heads*s.head_dim;}
ss_work_count :: (s:SsShape,chunk:int)->int {
    c:=ss_channels(s);v:=s.heads*s.head_dim;
    return chunk*c+3*c+ss_states(s)+chunk*v+chunk*s.head_dim+2*chunk+ss_scratch_count(chunk);
}
```

A token's input is its raw convolution channels (`x`, then `B`, then `C`),
the gate `z` and one raw `dt` per head. The parameters are the convolution
taps and bias per channel, then `log A`, the `dt` bias and `D` per head. A
snapshot is the convolution history followed by the state, so restoring both
restores the layer exactly.

## Gates you can read

<!-- source: psionic/lib/ssm.coax gate -->
```jai
// A head's step size and how much of its state it keeps for one token:
// dt = softplus(raw dt + dt bias), keep = exp(-exp(log A) dt).
SsGate :: struct { dt:f64; keep:f64; }
ss_gate :: (s:SsShape,params:[]f32,row:[]f32,head:int)->SsGate {
    c:=ss_channels(s);g:SsGate;
    g.dt=pd_softplus(f64(row[c+s.heads*s.head_dim+head])+f64(params[5*c+s.heads+head]));
    g.keep=exp(-exp(f64(params[5*c+head]))*g.dt);
    return g;
}
```

`ss_gate` is the first inspection point: for any token and head it returns
`dt` and `keep`. The example prints the range of `keep` over 13 tokens and 4
heads for two settings of `log A`. At `log A = -4` each token keeps between
0.95 and 0.996 of the state, so the layer remembers the whole prompt. At
`log A = 1` it keeps between 0.0005 and 0.55, so it mostly forgets. A check
compares every `dt` and `keep` with the oracle's own `softplus` and
`exp(-exp(log A) dt)`, written without chapter 11's helpers.

## The convolution and the token step

<!-- source: psionic/lib/ssm.coax convolve -->
```jai
ss_convolve :: (c:int,params:[]f32,raw:[]f32,hist:[]f64,conv:[]f64)->bool {
    for ch:0..<c {
        acc:PsionicAccumulator;
        for tap:0..<4 {
            item:=f64(raw[ch]);if tap<3 {item=hist[ch*3+tap];}
            acc=psionic_ops_accumulate(acc,item*f64(params[ch*4+tap]));
        }
        x:=pd_silu(psionic_ops_total(acc)+f64(params[4*c+ch]));
        if !psionic_ops_fits_f32(x) {return false;}
        conv[ch]=f64(f32(x));
        hist[ch*3]=hist[ch*3+1];hist[ch*3+1]=hist[ch*3+2];hist[ch*3+2]=f64(raw[ch]);
    }
    return true;
}
// The skip and the output gate: (y + D x) silu(z).
ss_finish :: (y:f64,d:f32,x:f64,z:f32)->f64 {return (y+f64(d)*x)*pd_silu(f64(z));}
```

<!-- source: psionic/lib/ssm.coax step -->
```jai
ss_step :: (s:SsShape,params:[]f32,row:[]f32,history:[]f32,state:[]f32,out:[]f32,work:[]f64,overlap:bool)->SsResult {
    // A step is one token in a chunk of one token.
    tokens:=1;
    status:=ss_admit(s,tokens,tokens,true,params,row,history,state,out,out[0..<0],work[0..<0],work,overlap);
    if status!=SsStatus.ok {return SsResult.{status=status};}
    c:=ss_channels(s);P:=s.head_dim;N:=s.state_dim;H:=s.heads;v:=H*P;n:=ss_states(s);
    w1:=3*c;w2:=w1+c;w3:=w2+n;
    hist:=work[0..<w1];conv:=work[w1..<w2];next:=work[w2..<w3];outs:=work[w3..<w3+v];
    for i:0..<3*c {hist[i]=f64(history[i]);}
    if !ss_convolve(c,params,row,hist,conv) {return SsResult.{status=SsStatus.arithmetic};}
    for head:0..<H {
        g:=ss_gate(s,params,row,head);group:=head/(H/s.groups);b:=v+group*N;cc:=v+s.groups*N+group*N;
        for p:0..<P {
            u:=g.dt*conv[head*P+p];
            for k:0..<N {
                i:=(head*P+p)*N+k;
                x:=f64(state[i])*g.keep;
                x+=conv[b+k]*u;
                if !psionic_ops_fits_f32(x) {return SsResult.{status=SsStatus.arithmetic};}
                next[i]=f64(f32(x));
            }
            acc:PsionicAccumulator;
            for k:0..<N {acc=psionic_ops_accumulate(acc,conv[cc+k]*next[(head*P+p)*N+k]);}
            y:=ss_finish(psionic_ops_total(acc),params[5*c+2*H+head],conv[head*P+p],row[c+head*P+p]);
            if !psionic_ops_fits_f32(y) {return SsResult.{status=SsStatus.arithmetic};}
            outs[head*P+p]=y;
        }
    }
    for i:0..<3*c {history[i]=f32(hist[i]);}
    for i:0..<n {state[i]=f32(next[i]);}
    for i:0..<v {out[i]=f32(outs[i]);}
    return SsResult.{status=SsStatus.ok,written=1};
}
```

`ss_step` is the decode form. It works in a private work view and commits the
outputs, convolution history and state together, so a refusal leaves all
three unchanged. The state is narrowed to `f32` once per token, and the output
reads the narrowed state.

## One head's chunk

<!-- source: psionic/lib/ssm.coax head -->
```jai
ss_head :: (h:SsChunk,conv:[]f64,dt:[]f64,keep:[]f64,start:[]f32,draft:[]f64,end:[]f64,scratch:[]f64)->bool {
    n:=h.tokens;P:=h.head_dim;N:=h.state_dim;w:=h.stride;
    ratio:=scratch[0..<n*n];mix:=scratch[n*n..<2*n*n];total:=scratch[2*n*n..<2*n*n+n];
    // total[t]: what the chunk's start state keeps through token t.
    // ratio[t*n+i]: what token i's write keeps through token t (1 when i = t).
    g:=1.0;
    for t:0..<n {
        g=g*keep[t];total[t]=g;ratio[t*n+t]=1;
        for j:0..<t {i:=t-1-j;ratio[t*n+i]=ratio[t*n+i+1]*keep[i+1];}
    }
    // mix[t*n+i] = ratio (C_t . B_i) dt_i: how much of x_i reaches y_t.
    for t:0..<n {for i:0..<n {
        mix[t*n+i]=0;
        if i<=t {
            acc:PsionicAccumulator;
            for k:0..<N {acc=psionic_ops_accumulate(acc,conv[t*w+h.c+k]*conv[i*w+h.b+k]);}
            mix[t*n+i]=ratio[t*n+i]*psionic_ops_total(acc)*dt[i];
        }
    }}
    // The state the chunk carries out, narrowed to f32.
    for p:0..<P {for k:0..<N {
        x:=f64(start[p*N+k])*total[n-1];
        for i:0..<n {x+=ratio[(n-1)*n+i]*conv[i*w+h.b+k]*(dt[i]*conv[i*w+h.x+p]);}
        if !psionic_ops_fits_f32(x) {return false;}
        end[p*N+k]=f64(f32(x));
    }}
    // Outputs: the last token reads the carried state; the others read
    // total[t] C_t . S0 + the row of M times x (zero above the diagonal).
    for t:0..<n {for p:0..<P {
        acc:PsionicAccumulator;
        if t==n-1 {
            for k:0..<N {acc=psionic_ops_accumulate(acc,conv[t*w+h.c+k]*end[p*N+k]);}
        } else {
            for k:0..<N {acc=psionic_ops_accumulate(acc,conv[t*w+h.c+k]*f64(start[p*N+k])*total[t]);}
            for i:0..<n {acc=psionic_ops_accumulate(acc,mix[t*n+i]*conv[i*w+h.x+p]);}
        }
        draft[t*P+p]=psionic_ops_total(acc);
    }}
    return true;
}
```

`ss_head` builds `G`, `R` and `M` for one head's chunk exactly as in the hand
calculation, carries the state out, and computes outputs as a row of `M` times
`x` plus what the start state contributes. Two details keep a chunk of one
token equal to `ss_step` bit for bit:

- The carried state multiplies in the step's order: `S0 * keep` first, then
  `B * (dt * x)` is added (`R = 1` and `G = 1 * keep` are exact).
- The chunk's last token reads the carried state after it is narrowed to
  `f32`, as the step reads its own.

The other tokens in a chunk read `M` and the start state instead, so their
additions happen in a different order. That is why longer chunks get a
tolerance instead of bit equality.

## Three forms

<!-- source: psionic/lib/ssm.coax forms -->
```jai
// Prefill in chunks of up to 64 tokens. Each chunk commits its outputs,
// history and state together, then writes its snapshot when snaps is not
// empty. An f32 overflow stops before the failing chunk commits.
ss_prefill :: (s:SsShape,params:[]f32,tokens:int,chunk:int,input:[]f32,history:[]f32,state:[]f32,out:[]f32,snaps:[]f32,work:[]f64,overlap:bool)->SsResult {
    return ss_run(s,params,tokens,chunk,input,history,state,out,snaps,work[0..<0],work,overlap);
}
// The whole sequence (up to 64 tokens) as one chunk: y = M x plus what the
// start state contributes, and each head's tokens x tokens mixing matrix M
// written to mix, row t holding what every x_i adds to y_t.
ss_whole :: (s:SsShape,params:[]f32,tokens:int,input:[]f32,history:[]f32,state:[]f32,out:[]f32,mix:[]f64,work:[]f64,overlap:bool)->SsResult {
    if mix.count==0 {return SsResult.{status=SsStatus.capacity};}
    return ss_run(s,params,tokens,tokens,input,history,state,out,out[0..<0],mix,work,overlap);
}
// A new request starts from zero history and zero state.
ss_clear :: (history:[]f32,state:[]f32) {
    for i:0..<history.count {history[i]=0;}
    for i:0..<state.count {state[i]=0;}
}
```

`ss_prefill` runs chunks of up to 64 tokens and can write a snapshot at every
chunk boundary. `ss_whole` is one chunk spanning the sequence (up to 64
tokens), and it also writes each head's `tokens x tokens` matrix `M` into
`mix`, the second inspection point. The example prints the top-left corner of
head 0's matrix at `log A = 1`: the entries shrink quickly down each column,
because little of an input survives a few tokens of forgetting. A check
compares every entry of every head's matrix with the closed form
`exp(A (dt_{i+1} + ... + dt_t)) (C_t . B_i) dt_i`, computed by the oracle from
a sum of step sizes and a single exponential instead of a product of `keep`
values, and asserts that every entry above the diagonal is exactly 0.

`ss_clear` zeroes a history and a state. Each request owns its own history and
state views; the layer has no state of its own.

## The profile and its tolerance

The profile is **ssm-cpu-f64-v1**: `f32` parameters, inputs, convolution
history, state and outputs; binary64 gates, products and compensated sums;
each convolved channel narrowed to `f32` once; the state narrowed to `f32`
once per chunk (once per token in the step). The tolerance was fixed before
any comparison:

<!-- source: psionic/tests/ssm-oracle.coax tolerance -->
```jai
SS_TOLERANCE :: 0.00001;
```

The reference is the [check oracle](../tests/ssm-oracle.coax), written from
the definition above in binary64. It reads the convolution's raw inputs
directly instead of keeping a shift register, computes `exp(dt A)` per token,
never narrows to `f32`, and shares no code with the library. The checks use
4 heads of width 2 in 2 groups, state width 3 and 13 tokens, at
`log A = -4` and `log A = 1`. They compare:

- The token step and chunks of 1, 2, 3, 4, 5, 8, 13 and 64 tokens with the
  reference: outputs, final state and every snapshot's state within
  `SS_TOLERANCE * (1 + the largest |reference value|)`.
- Chunks of one token with the token step: outputs, final state and every
  snapshot's state, bit for bit.
- Every snapshot's convolution history with the raw inputs of the three
  tokens before its boundary, bit for bit.
- The whole-sequence form with the reference (outputs and state), and its
  mixing matrices with the closed form, within the tolerance.

The largest differences in the example's table are near `1e-7`, a few `f32`
units in the last place, far inside `1e-5`.

`coax mutate psionic/lib/ssm.coax --root-only` changes the library one site
at a time; all 541 mutants fail a check.

## Snapshots, resuming and separate requests

A chunked run with snapshots writes one per chunk boundary. Restarting from
the snapshot after token 8 and prefilling tokens 8 to 12 gives the same
outputs, state and history as the run that continued, bit for bit, because
the restarted run has the same chunk boundaries. Decoding tokens 8 to 12 with
`ss_step` from that snapshot stays within the tolerance of the reference.
With chunks of one token, every snapshot's state equals the token step's
state at that token, bit for bit.

Two requests run in sequence on the same buffers, with `ss_clear` between
them, give the second request exactly the result it gets alone. Without the
clear, the first request's state leaks into the second's outputs, and the
example shows they differ. Two requests interleaved token by token, each
with its own buffers, each equal their separate runs bit for bit.

## Refusals

Each call checks its shape, ranges, views and numbers before it writes. A
refusal at that stage changes nothing and names one cause:

| Status | When |
| --- | --- |
| `shape` | more than 128 heads, a head width outside 1–128, a state width outside 1–256, or groups that do not divide the heads |
| `range` | tokens outside 1–4,096, or a chunk outside 1–64 (the whole-sequence form: up to 64 tokens) |
| `capacity` | a parameter, input, history, state, output, snapshot, matrix or work view of the wrong length |
| `alias` | the caller reports overlapping views |
| `number` | a non-finite parameter, input, history or state, or a `log A` above 80 for any head |
| `arithmetic` | a convolution, state or output that does not fit `f32` |

`arithmetic` can only be found while computing, so a prefill reports it per
chunk: the chunk that overflows does not commit, and `written` and
`snapshots` say how many tokens and snapshots did. The state and history are
then the last snapshot's. The step either commits its token or changes
nothing. The example ends with three refusals: a chunk size of 0, an infinite
input at token 5 (nothing written), and a convolution past `f32` at token 9,
which leaves 8 tokens and 2 snapshots committed. Checks also build a state
pushed past `f32` and an output gate past `f32`, and the step and prefill
refuse both. A non-finite value cannot be built inside a Coax check, so only
the compiled example exercises the non-finite `number` refusal.

## What this does not do yet

This chapter is the state-space core that
[chapter 163](ch163-execute-nemotron-nano.md) builds on. It does not include:

- **The rest of a Mamba-2 block.** The input and output projections and the
  grouped RMS normalization after the gate belong to the model and are not
  here; the layer starts at the projected channels and ends at the gated
  output.
- **Real model widths.** Shapes up to 128 heads, head width 128 and state
  width 256 are admitted, but the tolerance has been exercised only at the
  checks' small widths and 13 tokens.
- **A device kernel.** Everything runs on the CPU. The chunked form's matrix
  work is the part a device would run in parallel; it is not measured here.

## Sources

- [Chapter plan](PLAN.md), Part XVII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Nemotron Nano entry.
- [Chapter 42](ch042-deltanet-in-chunks.md): the chunk layout, snapshots and
  profile pattern this chapter follows.
- Dao and Gu, *Transformers are SSMs: Generalized Models and Efficient
  Algorithms Through Structured State Space Duality* (2024): Mamba-2, the
  scalar-decay state space and its matrix form.
