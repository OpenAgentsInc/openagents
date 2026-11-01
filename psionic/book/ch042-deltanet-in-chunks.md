# 42. DeltaNet in chunks

> **Status: partly implemented.**

[Chapter 11](ch011-a-hybrid-sequence-model.md) runs Gated DeltaNet one token
at a time: each token reads the recurrent state, corrects it, and writes it
back before the next token may start. That is correct, but a long prompt then
needs one dependent step per token, each touching the whole state. This
chapter processes the prompt in **chunks**. Inside a chunk, the corrections
the tokens make to one another form a small lower-triangular system, solved
by forward substitution; only the chunk's last token writes the state. The
additions happen in a different order, so the result gets its own named
numerical profile and a tolerance against chapter 11's step, frozen before
any comparison.

What runs is a bounded CPU implementation over chapter 11's packed per-token
inputs: the convolution, the triangular solve, the gated output
normalization, snapshots at chunk boundaries and resuming from them, each
compared with chapter 11's `pd_step`. The device kernel in
[chapter 37](ch037-a-coax-compute-subset.md)'s compute subset and the
prefill timing on Elephant's prompts are not built yet; the last section
lists what remains.

```shell
coax/bin/coax run psionic/examples/delta-chunks.coax
coax/bin/coax test psionic/lib/delta-chunks.coax
```

```text
profile delta-chunks-cpu-f64-v1, tolerance 0.00001 x (1 + largest |reference value|)
decay through each token 0.5, 0.25; decay after token 0 through token 1 0.5
solved updates u 1.0, 2.0; outputs 2.0, 3.0; carried state 3.0
13 tokens, 3 key heads, 6 value heads, key and value width 2:
  log A 0.0:
    chunk 1: outputs 0.0, state 0.0, snapshots 0.0 (13 of them, histories equal true), bit for bit true
    chunk 2: outputs 2.9802322387695312e-08, state 7.450580596923828e-09, snapshots 1.4901161193847656e-08 (7 of them, histories equal true), bit for bit false
    chunk 3: outputs 4.6566128730773926e-08, state 1.4901161193847656e-08, snapshots 2.9802322387695312e-08 (5 of them, histories equal true), bit for bit false
    chunk 4: outputs 2.9802322387695312e-08, state 1.4901161193847656e-08, snapshots 1.4901161193847656e-08 (4 of them, histories equal true), bit for bit false
    chunk 5: outputs 5.9604644775390625e-08, state 1.4901161193847656e-08, snapshots 1.4901161193847656e-08 (3 of them, histories equal true), bit for bit false
    chunk 8: outputs 5.9604644775390625e-08, state 1.4901161193847656e-08, snapshots 1.4901161193847656e-08 (2 of them, histories equal true), bit for bit false
    chunk 64: outputs 5.9604644775390625e-08, state 1.4901161193847656e-08, snapshots 1.4901161193847656e-08 (1 of them, histories equal true), bit for bit false
  log A -4.0:
    chunk 1: outputs 0.0, state 0.0, snapshots 0.0 (13 of them, histories equal true), bit for bit true
    chunk 2: outputs 5.9604644775390625e-08, state 1.4901161193847656e-08, snapshots 2.9802322387695312e-08 (7 of them, histories equal true), bit for bit false
    chunk 3: outputs 4.563480615615845e-08, state 2.9802322387695312e-08, snapshots 2.9802322387695312e-08 (5 of them, histories equal true), bit for bit false
    chunk 4: outputs 7.264316082000732e-08, state 2.9802322387695312e-08, snapshots 2.9802322387695312e-08 (4 of them, histories equal true), bit for bit false
    chunk 5: outputs 7.35744833946228e-08, state 2.9802322387695312e-08, snapshots 2.9802322387695312e-08 (3 of them, histories equal true), bit for bit false
    chunk 8: outputs 8.940696716308594e-08, state 2.9802322387695312e-08, snapshots 2.9802322387695312e-08 (2 of them, histories equal true), bit for bit false
    chunk 64: outputs 7.450580596923828e-08, state 2.9802322387695312e-08, snapshots 2.9802322387695312e-08 (1 of them, histories equal true), bit for bit false
resumed at token 8 from a snapshot: outputs, state and history bit for bit true
Elephant's shape: work at chunk 64 1364288 f64 elements, one snapshot 548864 f32 elements
refused: chunk 0: range, output -7.0
refused: an infinite input at token 5: number, output -7.0, written 0
refused: a convolution past f32 at token 9: arithmetic, tokens committed 8, snapshots 2, state equals snapshot 2 true
```

The [library](../lib/delta-chunks.coax), its
[check oracle](../tests/delta-chunks-oracle.coax) and the
[example](../examples/delta-chunks.coax) are the whole implementation.
Inputs are synthetic; no model weights are read.

## The recurrence, one token at a time

For one value head, the state `S` is a matrix with a row per key coordinate
and a column per value coordinate. Chapter 11's step, for a token with
normalized query `q`, normalized key `k`, value `v`, write strength `beta`
and gate decay `g`, is:

```text
prediction = (g S)^T k
u          = beta (v - prediction)
S'         = g S + k u^T
output     = S'^T q
```

`u` is the correction this token writes. Every quantity depends on the state
the previous token left, which is why the steps are sequential.

## Work a two-token chunk by hand

Take one key and one value coordinate, so the state is a single number. Start
from `S0 = 2`. Two tokens both have `q = k = 1`, `beta = 0.5` and gate decay
`g = 0.5`; their values are `v = 3` and `v = 5`.

Token by token:

```text
token 0: prediction = 0.5 * 2 * 1 = 1    u0 = 0.5 * (3 - 1) = 1
         S1 = 0.5 * 2 + 1 * 1 = 2        output 2
token 1: prediction = 0.5 * 2 * 1 = 1    u1 = 0.5 * (5 - 1) = 2
         S2 = 0.5 * 2 + 1 * 2 = 3        output 3
```

The chunked form never builds `S1`. It writes each state in terms of the
chunk's starting state and the corrections:

```text
G_t        = g_0 g_1 ... g_t                 decay from the chunk start through token t
D_{t,i}    = g_{i+1} ... g_t (1 when i = t)  decay after token i through token t
S_{t+1}    = G_t S0 + sum over i <= t of D_{t,i} k_i u_i^T
```

Here `G = 0.5, 0.25` and `D_{1,0} = 0.5`. Substituting into
`u_t = beta_t (v_t - S_t'^T k_t)` gives one equation per token:

```text
u_t + beta_t sum over i < t of D_{t,i} (k_t . k_i) u_i = beta_t (v_t - G_t S0^T k_t)
```

The left side is a unit lower-triangular matrix times the corrections. For
the two tokens:

```text
[ 1                        0 ] [u0]   [0.5 * (3 - 0.5  * 2)]   [1   ]
[ 0.5 * 0.5 * (1 * 1)      1 ] [u1] = [0.5 * (5 - 0.25 * 2)] = [2.25]
```

Forward substitution: `u0 = 1`, then `u1 = 2.25 - 0.25 * 1 = 2`. The outputs
and the carried state follow without any intermediate state:

```text
output_0 = G_0 S0 q_0 + D_{0,0} (q_0 . k_0) u0           = 0.5 * 2 + 1 = 2
output_1 = G_1 S0 q_1 + D_{1,0} (q_1 . k_0) u0 + (q_1 . k_1) u1
                                         = 0.25 * 2 + 0.5 + 2 = 3
S2       = G_1 S0 + D_{1,0} k_0 u0 + k_1 u1 = 0.5 + 0.5 + 2 = 3
```

The same numbers as token by token. The example's second and third lines
print `G`, `D_{1,0}`, `u`, the outputs and the carried state from the code
below, and a check asserts each of them, together with the scratch matrices
`D`, `k . k` and `q . k`.

This is the **WY form** (also called the UT transform) of the delta rule:
the corrections of a whole chunk come from one triangular solve, and the
products `D (k . k)` and `D (q . k)` are dense small matrices that a device
computes as matrix multiplications.

## One head's chunk

`dk_head` solves one value head's chunk. Its rows are the chunk's convolved
channels; `q`, `k` and `v` say where this head's query, key and value start
in a row, and `qdiv` and `kdiv` are chapter 11's normalizers, so the
normalized key is `conv / kdiv`:

<!-- source: psionic/lib/delta-chunks.coax types -->
```jai
DkStatus :: enum u8 { ok; shape; range; capacity; alias; number; arithmetic; }
// One head's chunk: tokens rows of stride convolved channels; q, k and v are
// where this head's query, key and value start in a row.
DkChunk :: struct { key_dim:int; value_dim:int; tokens:int; stride:int; q:int; k:int; v:int; }
// written: tokens whose outputs, state and history were committed.
// snapshots: chunk-boundary snapshots written.
DkResult :: struct { status:DkStatus; written:int; snapshots:int; }
```

<!-- source: psionic/lib/delta-chunks.coax head -->
```jai
dk_scratch_count :: (value_dim:int,chunk:int)->int {return 3*chunk*chunk+chunk*value_dim+chunk;}
dk_head :: (h:DkChunk,conv:[]f64,qdiv:[]f64,kdiv:[]f64,beta:[]f64,decay:[]f64,start:[]f32,draft:[]f64,end:[]f64,scratch:[]f64)->bool {
    n:=h.tokens;dk:=h.key_dim;dv:=h.value_dim;w:=h.stride;
    a1:=n*n;a2:=a1+n*n;a3:=a2+n*n;a4:=a3+n*dv;
    ratio:=scratch[0..<a1];kk:=scratch[a1..<a2];qk:=scratch[a2..<a3];u:=scratch[a3..<a4];total:=scratch[a4..<a4+n];
    // total[t]: the decay from the chunk start through token t.
    // ratio[t*n+i]: the decay after token i through token t (1 when i = t).
    g:=1.0;
    for t:0..<n {
        g=g*decay[t];total[t]=g;ratio[t*n+t]=1;
        for j:0..<t {i:=t-1-j;ratio[t*n+i]=ratio[t*n+i+1]*decay[i+1];}
    }
    // Normalized key-key and query-key dot products, for i <= t.
    for t:0..<n {for i:0..<t+1 {
        a:PsionicAccumulator;b:PsionicAccumulator;
        for row:0..<dk {
            key:=conv[i*w+h.k+row]/kdiv[i];
            a=psionic_ops_accumulate(a,conv[t*w+h.k+row]/kdiv[t]*key);
            b=psionic_ops_accumulate(b,conv[t*w+h.q+row]/qdiv[t]*key);
        }
        kk[t*n+i]=psionic_ops_total(a);qk[t*n+i]=psionic_ops_total(b);
    }}
    // Forward substitution: u[t] = beta[t] (v[t] - total[t] S0^T k[t]
    //   - sum over i < t of ratio[t][i] (k[t] . k[i]) u[i]).
    for col:0..<dv {for t:0..<n {
        memory:PsionicAccumulator;
        for row:0..<dk {memory=psionic_ops_accumulate(memory,f64(start[row*dv+col])*total[t]*conv[t*w+h.k+row]/kdiv[t]);}
        for i:0..<t {memory=psionic_ops_accumulate(memory,ratio[t*n+i]*kk[t*n+i]*u[i*dv+col]);}
        u[t*dv+col]=(conv[t*w+h.v+col]-psionic_ops_total(memory))*beta[t];
    }}
    // The state the chunk carries out, narrowed to f32.
    for row:0..<dk {for col:0..<dv {
        x:=f64(start[row*dv+col])*total[n-1];
        for i:0..<n {x+=ratio[(n-1)*n+i]*(conv[i*w+h.k+row]/kdiv[i])*u[i*dv+col];}
        if !psionic_ops_fits_f32(x) {return false;}
        end[row*dv+col]=f64(f32(x));
    }}
    // Outputs: the last token reads the carried state; the others read
    // total[t] S0^T q[t] + sum over i <= t of ratio[t][i] (q[t] . k[i]) u[i].
    for t:0..<n {for col:0..<dv {
        acc:PsionicAccumulator;
        if t==n-1 {
            for row:0..<dk {acc=psionic_ops_accumulate(acc,conv[t*w+h.q+row]/qdiv[t]*end[row*dv+col]);}
        } else {
            for row:0..<dk {acc=psionic_ops_accumulate(acc,conv[t*w+h.q+row]/qdiv[t]*f64(start[row*dv+col])*total[t]);}
            for i:0..<t+1 {acc=psionic_ops_accumulate(acc,ratio[t*n+i]*qk[t*n+i]*u[i*dv+col]);}
        }
        x:=psionic_ops_total(acc);
        if !psionic_ops_fits_f32(x) {return false;}
        draft[t*dv+col]=f64(f32(x));
    }}
    return true;
}
```

Three details keep it equal to chapter 11's step where it can be:

- The decays are products, not differences of logarithms, and nothing is
  divided by a decay. A decay of exactly 0 (a large log A) makes products
  of 0, never `0 / 0`.
- The prediction term multiplies in chapter 11's order:
  `S * G * conv / kdiv`. With one token, `G_0` is `1 * g`, which is `g`
  exactly, and `D_{0,0} * k` is `k` exactly.
- The last token of a chunk reads the carried state after it has been
  narrowed to `f32`, as chapter 11's step reads the state it has just
  written. The other tokens read the WY form in `f64`.

So a chunk of one token performs chapter 11's arithmetic operation for
operation, and the result is equal bit for bit.

## Prefill in chunks

`dk_prefill` takes a prompt of packed chapter-11 inputs, one per token. Per
chunk it runs chapter 11's convolution token by token (it is a short shift
register, not the expensive part), computes each head's normalizers, beta
and decay, solves the head with `dk_head`, applies chapter 11's gated RMS
normalization, and then commits the chunk's outputs, convolution history and
state together:

<!-- source: psionic/lib/delta-chunks.coax prefill -->
```jai
dk_prefill :: (s:PdShape,tokens:int,chunk:int,input:[]f32,history:[]f32,state:[]f32,out:[]f32,snaps:[]f32,work:[]f64,overlap:bool)->DkResult {
    status:=dk_admit(s,tokens,chunk,input,history,state,out,snaps,work,overlap);
    if status!=DkStatus.ok {return DkResult.{status=status};}
    c:=pd_channels(s);m:=pd_input_count(s);n:=pd_states(s);h:=s.value_heads;dk:=s.key_dim;dv:=s.value_dim;v:=h*dv;
    aoff:=5*c;boff:=aoff+h;logoff:=boff+h;dtoff:=logoff+h;zoff:=dtoff+h;gammaoff:=zoff+v;
    w1:=chunk*c;w2:=w1+3*c;w3:=w2+n;w4:=w3+chunk*v;w5:=w4+chunk*dv;w6:=w5+chunk;w7:=w6+chunk;w8:=w7+chunk;w9:=w8+chunk;
    conv:=work[0..<w1];hist:=work[w1..<w2];next:=work[w2..<w3];outs:=work[w3..<w4];draft:=work[w4..<w5];
    qdiv:=work[w5..<w6];kdiv:=work[w6..<w7];beta:=work[w7..<w8];decay:=work[w8..<w9];
    scratch:=work[w9..<w9+dk_scratch_count(dv,chunk)];
    done:DkResult;last:=dk_chunks(tokens,chunk)-1;
    for b:0..<last+1 {
        lo:=b*chunk;size:=ifx b==last then tokens-lo else chunk;
        for i:0..<3*c {hist[i]=f64(history[i]);}
        for l:0..<size {
            if !dk_convolve(c,input[(lo+l)*m..<(lo+l+1)*m],hist,conv[l*c..<(l+1)*c]) {done.status=DkStatus.arithmetic;return done;}
        }
        for head:0..<h {
            key_head:=head/(h/s.key_heads);qbase:=key_head*dk;kbase:=pd_key_base(s.key_heads,dk,qbase);vbase:=2*s.key_heads*dk+head*dv;
            for l:0..<size {
                row:=input[(lo+l)*m..<(lo+l+1)*m];
                qsq:PsionicAccumulator;ksq:PsionicAccumulator;
                for i:0..<dk {x:=conv[l*c+qbase+i];y:=conv[l*c+kbase+i];qsq=psionic_ops_accumulate(qsq,x*x);ksq=psionic_ops_accumulate(ksq,y*y);}
                qdiv[l]=sqrt(psionic_ops_total(qsq)+0.000001)*sqrt(f64(dk));kdiv[l]=sqrt(psionic_ops_total(ksq)+0.000001);
                beta[l]=pd_sigmoid(f64(row[boff+head]));
                decay[l]=exp(-exp(f64(row[logoff+head]))*pd_softplus(f64(row[aoff+head])+f64(row[dtoff+head])));
            }
            one:=DkChunk.{key_dim=dk,value_dim=dv,tokens=size,stride=c,q=qbase,k=kbase,v=vbase};
            if !dk_head(one,conv,qdiv,kdiv,beta,decay,state[head*dk*dv..<(head+1)*dk*dv],draft,next[head*dk*dv..<(head+1)*dk*dv],scratch) {done.status=DkStatus.arithmetic;return done;}
            // Chapter 11's gated RMS normalization, per token.
            for l:0..<size {
                row:=input[(lo+l)*m..<(lo+l+1)*m];
                squares:PsionicAccumulator;
                for col:0..<dv {x:=draft[l*dv+col];squares=psionic_ops_accumulate(squares,x*x);}
                denom:=sqrt(psionic_ops_total(squares)/f64(dv)+f64(s.epsilon));
                for col:0..<dv {
                    idx:=head*dv+col;
                    x:=draft[l*dv+col]/denom*f64(row[gammaoff+col])*pd_silu(f64(row[zoff+idx]));
                    if !psionic_ops_fits_f32(x) {done.status=DkStatus.arithmetic;return done;}
                    outs[l*v+idx]=f64(f32(x));
                }
            }
        }
        for i:0..<3*c {history[i]=f32(hist[i]);}
        for i:0..<n {state[i]=f32(next[i]);}
        for i:0..<size*v {out[lo*v+i]=f32(outs[i]);}
        done.written=lo+size;
        if snaps.count!=0 {
            snap:=snaps[b*dk_snapshot_size(s)..<(b+1)*dk_snapshot_size(s)];
            for i:0..<3*c {snap[i]=history[i];}
            for i:0..<n {snap[3*c+i]=state[i];}
            done.snapshots+=1;
        }
    }
    return done;
}
```

The work a call needs depends on the chunk, not on the prompt length:

<!-- source: psionic/lib/delta-chunks.coax plan -->
```jai
dk_work_count :: (s:PdShape,chunk:int)->int {
    c:=pd_channels(s);v:=s.value_heads*s.value_dim;
    return chunk*c+3*c+pd_states(s)+chunk*v+chunk*s.value_dim+4*chunk+dk_scratch_count(s.value_dim,chunk);
}
dk_chunks :: (tokens:int,chunk:int)->int {return (tokens+chunk-1)/chunk;}
// One snapshot is the convolution history (3 per channel) and then the state.
dk_snapshot_size :: (s:PdShape)->int {return 3*pd_channels(s)+pd_states(s);}
```

For Elephant's linear-layer shape (16 key heads, 32 value heads, width 128)
with chunks of 64, the work is 1,364,288 `f64` elements. The chunk's
convolved channels (64 x 8,192) and the staged state (524,288) are three
quarters of it. A snapshot is
548,864 `f32` elements: the convolution history and the state. These are
logical element counts, not allocator measurements.

## Snapshots and resuming

When `snaps` is not empty, each committed chunk appends a snapshot: the
convolution history, then the state, as chapter 11's step would hold them
after the chunk's last token. Chapter 12's forks and speculative rollback can
restart from any of them.

Resuming from a snapshot with the same chunk size reproduces the uninterrupted
run bit for bit: the remaining tokens fall into the same chunks, so every
chunk performs the same operations from the same `f32` state. The example
restarts 13 tokens at the boundary after token 8 and compares outputs, state
and history. Restarting at a token that is not a boundary moves the later
chunk edges; the checks show that result differs in its last bits and stays
within the tolerance.

## The profile and its tolerance

Profile `delta-chunks-cpu-f64-v1`:

- `f32` inputs, convolution history, state and outputs.
- Chapter 11's convolution, normalizers, beta, decay and gated output
  normalization, unchanged and token by token.
- Inside a chunk, `f64` decays, dot products, the triangular solve and the
  outputs, with chapter 4's compensated sums. The state is narrowed to `f32`
  once per chunk, at its end, and the chunk's last token reads that narrowed
  state.

The tolerance was frozen in the check oracle before the first comparison ran:

<!-- source: psionic/tests/delta-chunks-oracle.coax tolerance -->
```jai
DK_TOLERANCE :: 0.00001;
```

The oracle runs chapter 11's `pd_step`, loaded unchanged, over 13 tokens with
3 key heads shared by 6 value heads, key and value width 2, and keeps its
output, state and history at every position. Two decay settings are checked:
log A of 0, where each token keeps between about 0.15 and 0.85 of the state,
and -4, where each token keeps more than 0.96, so the state remembers the
whole prompt. The checks compare:

- Chunk size 1 with the recurrence: outputs, final state, final history and
  all 13 snapshots equal bit for bit.
- Chunk sizes 2, 3, 4, 5, 8 and 64 with the recurrence: outputs, final state
  and every boundary snapshot's state within the tolerance, scaled by
  `1 + the largest |reference value|`; every snapshot's convolution history
  equal bit for bit.
- Resuming at a boundary with continuing: bit for bit.

The largest differences in the example's table are below `1e-7`, a few `f32`
units in the last place, far inside `1e-5`. The tolerance stays the
contract for longer prompts and larger widths, where more terms are summed.

`coax mutate psionic/lib/delta-chunks.coax --root-only` changes the library
one site at a time; every mutant fails a check.

## Refusals

Each call checks its shape, ranges, views and numbers before it writes. A
refusal at that stage changes nothing and names one cause:

| Status | When |
| --- | --- |
| `shape` | a shape chapter 11's `pd_valid` refuses: up to 16 key heads, 48 value heads, widths 128 |
| `range` | tokens outside 1–4,096, or a chunk outside 1–64 |
| `capacity` | an input, history, state, output, snapshot or work view of the wrong length |
| `alias` | the caller reports overlapping views |
| `number` | a non-finite input, history or state, or a log A above 80 for any token and head |
| `arithmetic` | a convolution, state or output that does not fit `f32` |

`arithmetic` can only be found while computing, so it is reported per chunk:
the chunk that overflows does not commit, and `written` and `snapshots` say
how many tokens and snapshots did. The state and history are then the last
snapshot's. The example ends with three refusals: a chunk size of 0, an
infinite input at token 5 (nothing written), and a convolution past `f32` at
token 9, which leaves 8 tokens and 2 snapshots committed. A check builds a
state column `(max, -max, -max)` whose correction pushes one entry past
`f32`; chapter 11's step refuses the same token with its own `arithmetic`. A
non-finite value cannot be built inside a Coax check, so only the compiled
example exercises the non-finite `number` refusal.

## What this does not do yet

- **A device kernel.** No chunked kernel in chapter 37's compute subset runs
  yet. On the CPU the chunked form does the same work as the recurrence; its
  value is the matrix-shaped work a device can run in parallel, and that is
  not measured here.
- **Elephant's prefill time.** Nothing here reads Elephant's weights or
  replaces `pd_step` inside chapter 11's linear layer, so there is no before
  and after timing on a named device, and no measured chunk size. llama.cpp
  uses 64.
- **Real model widths in the checks.** The checks use widths of 2 and 13
  tokens; chapter 11's shape bounds are enforced, but the tolerance has not
  been exercised at width 128 or at thousands of tokens.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [Hybrid forward primitives](../docs/hybrid-kernels.md).
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): chunked Gated
  DeltaNet.
- Yang et al., *Parallelizing Linear Transformers with the Delta Rule over
  Sequence Length* (2024), and *Gated Delta Networks* (2024): the WY form and
  the gated chunk recurrence.
