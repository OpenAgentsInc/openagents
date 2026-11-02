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
inputs (the convolution, the triangular solve, the gated output
normalization, snapshots at chunk boundaries and resuming from them, each
compared with chapter 11's `pd_step`), and a group kernel in
[chapter 37](ch037-a-coax-compute-subset.md)'s compute subset that runs the
same chunked form on an RTX 4080 inside
[chapter 44](ch044-elephant-on-cuda.md)'s linear-layer kernels, at
Elephant's real shape, on synthetic prompts of 64 to 2,048 tokens and on
linear layer 0 of the original weights. Chapter 44's decision program does
not use the kernel yet; the last section lists what remains.

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

## On the device

On the CPU the chunked form does the same work as the recurrence. Its value
is the shape of that work: within a chunk, every token's prediction from the
starting state, every key-key and query-key product and the state update are
independent sums a device can run at once. Chapter 44's Elephant path runs
the recurrence as `dl_scan`: one work item per value column of each head
(32 x 128 = 4,096 items), each walking every token in order with its state
column in registers.

`dkd_chunk` is a **group kernel** (chapter 68): one block of 128 threads per
value head and block of 16 value columns, 256 groups for Elephant. Thread
(j, p) keeps rows `16p ..< 16p + 16` of column j of the head's state in its
own registers for the whole prompt. Per chunk of up to 16 tokens:

1. the group copies the chunk's keys into shared memory, and 16 threads
   take each token's `G`, `D[n-1][i]` and beta;
2. the threads fill the two 16 x 16 tables `D[t][i] (k[t] . k[i])` for
   `i < t` and `D[t][i] (q[t] . k[i])` for `i <= t`;
3. each thread adds its 16 rows' share of every token's prediction
   `G[t] S^T k[t]` and read `G[t] S^T q[t]` into two shared partial-sum
   tables;
4. each thread adds its column's 8 partial sums, solves the column's
   corrections u by forward substitution into its own registers, and
   writes the reads of two of the chunk's tokens;
5. each thread carries its 16 state rows to the chunk's end.

<!-- source: psionic/lib/device/delta-chunks-kernels.coax shape -->
```jai
DKD_THREADS :: 128;  // threads per group: 16 columns x 8 row slices
DKD_COLS :: 16;      // value columns per group
DKD_SLICES :: 8;     // row slices per column, 16 key rows each
DKD_CHUNK :: 16;     // most tokens per chunk: the shared tiles' rows
DKD_WIDTH :: 128;    // key and value width per head
```

<!-- source: psionic/lib/device/delta-chunks-kernels.coax kernel -->
```jai
// Group `item` is columns 16*(item%8) ..< +16 of value head item/8; key head
// h/(heads/key_heads). Token t's normalized key and query are k and
// q[(t*key_heads + kh)*128 ...], its value conv[t*channels + key_heads*256
// + h*128 ...], its beta and decay beta and decay[t*heads + h].
// out[(t*heads + h)*128 + c] receives the read before the gated norm, as
// dl_scan writes it. state holds the starting state in chapter 11's layout,
// state[(h*128 + r)*128 + c] for key row r and value column c, and receives
// the carried state.
dkd_chunk :: (item:int,tokens:int,chunk:int,heads:int,key_heads:int,channels:int,conv:[]f32,q:[]f32,k:[]f32,
    beta:[]f32,decay:[]f32,out:[]f32,state:[]f32) #export_c {
    // Registers: st, each thread's 16 state rows, and uu, its column's
    // corrections for the chunk. Shared: the chunk's keys; per-slice partial
    // sums pu (predictions) and py (reads); D (k . k) and D (q . k); G,
    // D[n-1][i] and beta.
    st:[2048]f32;uu:[2048]f32;
    kt:[2048]f32;pu:[2048]f32;py:[2048]f32;a:[256]f32;b:[256]f32;total:[16]f32;last:[16]f32;bt:[16]f32;
    h:=item/8;col:=item%8*DKD_COLS;kh:=h/(heads/key_heads);vbase:=key_heads*256+h*128+col;sb:=h*DKD_WIDTH*DKD_WIDTH+col;
    for thread:0..<DKD_THREADS {
        for i:0..<16 {st[thread*16+i]=state[sb+(thread/16*16+i)*DKD_WIDTH+thread%16];}
    }
    last_block:=(tokens-1)/chunk;
    for block:0..<last_block+1 {
        lo:=block*chunk;n:=ifx block==last_block then tokens-lo else chunk;
        // Thread i stages an eighth of token i/8's key (zero past the
        // chunk); thread i < 16 takes token i's G, D[n-1][i] and beta, zero
        // past the chunk.
        for thread:0..<DKD_THREADS {
            t:=thread/8;kb:=((lo+t)*key_heads+kh)*DKD_WIDTH+thread%8*16;
            for c:0..<16 {x:f32=0;if t<n {x=k[kb+c];}kt[thread*16+c]=x;}
            if thread<DKD_CHUNK {
                g:f32=0;ratio:f32=0;w:f32=0;
                if thread<n {
                    g=1;for j:0..<thread+1 {g=g*decay[(lo+j)*heads+h];}
                    ratio=1;for j:0..<n-1-thread {ratio=ratio*decay[(lo+n-1-j)*heads+h];}
                    w=beta[(lo+thread)*heads+h];
                }
                total[thread]=g;last[thread]=ratio;bt[thread]=w;
            }
        }
        // Thread i fills entries 2i and 2i+1 of the 16 x 16 tables: row t,
        // column j holds D[t][j] (k[t] . k[j]) for j < t and D[t][j]
        // (q[t] . k[j]) for j <= t, both below n; every other entry is 0.
        for thread:0..<DKD_THREADS {
            for c:0..<2 {
                t:=thread/8;i:=thread%8*2+c;x:f32=0;z:f32=0;
                if t<n && i<=t {
                    ratio:f32=1;for j:0..<t-i {ratio=ratio*decay[(lo+t-j)*heads+h];}
                    qb:=((lo+t)*key_heads+kh)*DKD_WIDTH;kk:f32=0;qk:f32=0;
                    for r:0..<DKD_WIDTH {key:=kt[i*DKD_WIDTH+r];kk+=kt[t*DKD_WIDTH+r]*key;qk+=q[qb+r]*key;}
                    if i<t {x=ratio*kk;}
                    z=ratio*qk;
                }
                a[thread*2+c]=x;b[thread*2+c]=z;
            }
        }
        // Thread (j, p): its slice's share of the chunk's starting state's
        // prediction and read for every token of the chunk.
        for thread:0..<DKD_THREADS {
            r0:=thread/16*16;
            for t:0..<DKD_CHUNK {
                pv:f32=0;yv:f32=0;
                if t<n {
                    g:=total[t];qb:=((lo+t)*key_heads+kh)*DKD_WIDTH+r0;
                    for i:0..<16 {x:=st[thread*16+i];pv+=x*g*kt[t*DKD_WIDTH+r0+i];yv+=q[qb+i]*x*g;}
                }
                pu[thread*16+t]=pv;py[thread*16+t]=yv;
            }
        }
        // Thread (j, p): forward substitution for column j's corrections
        // (each of the column's 8 threads solves it, into its own
        // registers), then the reads of tokens p and p + 8.
        for thread:0..<DKD_THREADS {
            j:=thread%16;
            for t:0..<DKD_CHUNK {
                memory:f32=0;
                for s:0..<DKD_SLICES {memory+=pu[(s*16+j)*16+t];}
                for i:0..<DKD_CHUNK {memory+=a[t*DKD_CHUNK+i]*uu[thread*16+i];}
                u:f32=0;if t<n {u=(conv[(lo+t)*channels+vbase+j]-memory)*bt[t];}
                uu[thread*16+t]=u;
            }
            for c:0..<2 {
                t:=c*8+thread/16;read:f32=0;
                for s:0..<DKD_SLICES {read+=py[(s*16+j)*16+t];}
                for i:0..<DKD_CHUNK {read+=b[t*DKD_CHUNK+i]*uu[thread*16+i];}
                if t<n {out[((lo+t)*heads+h)*DKD_WIDTH+col+j]=read;}
            }
        }
        // Thread (j, p): the carried state of its 16 rows.
        for thread:0..<DKD_THREADS {
            r0:=thread/16*16;g:=total[n-1];
            for i:0..<16 {
                x:=st[thread*16+i]*g;
                for s:0..<DKD_CHUNK {x+=last[s]*kt[s*DKD_WIDTH+r0+i]*uu[thread*16+s];}
                st[thread*16+i]=x;
            }
        }
    }
    for thread:0..<DKD_THREADS {
        for i:0..<16 {state[sb+(thread/16*16+i)*DKD_WIDTH+thread%16]=st[thread*16+i];}
    }
}
```

The inputs are exactly what chapter 44's `dl_conv`, `dl_gates` and `dl_qk`
write and the output is what `dl_scan` writes, so `dkd_chunk` replaces
`dl_scan` between them with no other change; `dl_gate_norm` follows. The
state uses chapter 11's layout, so the carried state compares directly with
`pd_step`'s.

Two details keep the kernel inside chapter 37's subset. Every loop over the
chunk's tokens runs to the constant `DKD_CHUNK` and skips tokens past the
chunk's end, because the compiler only accepts register and shared indices
it can prove a thread owns. Entries of the tables and corrections past the
chunk's end are written as 0, so later sums over all 16 slots add exact
zeros.

### The profile and its tolerance

Profile `delta-chunks-cuda-f32-v1`:

- `f32` values and sums throughout, each sum in index order; a sum over the
  128 key rows adds 8 slices of 16 rows, each in row order, then the slices
  in order.
- Per chunk, `G` and `D` are products of decays in token order, as on the
  CPU.
- Every token reads the WY form; the state is carried once per chunk.
- `--fmad=false` and no transcendental functions in the kernel, so the
  device and the C build of the same source compute the same bits.

The tolerance was frozen in the device oracle before any comparison ran:

<!-- source: psionic/tests/delta-chunks-device-oracle.coax tolerance -->
```jai
DKD_TOLERANCE :: 0.001;
```

An output or carried state may differ from chapter 11's recurrence, from this
chapter's CPU profile, from chapter 44's `dl_scan` path or from a binary64
oracle by at most `0.001 x (1 + the largest |reference value|)`, the bound
chapter 41's device kernel uses.

### Admission and launch checks

`delta-chunks-plan.coax` is an `#authority` file. `dkp_admit` asks chapter
68's `da_admit` whether the reported device can run 128 threads with 26,816
shared bytes; `dkp_check` refuses a launch whose shape, prompt, chunk or
views do not fit before any byte moves:

<!-- source: psionic/lib/device/delta-chunks-plan.coax plan -->
```jai
// What dkd_chunk needs: 128 threads; shared keys and two partial-sum
// tables (16 x 128 f32 each), two 16 x 16 f32 tables and three rows of 16.
DKP_NEEDS :: DaNeeds.{threads=128,shared=26816,mma=false};
dkp_admit :: (major:int,minor:int,max_threads:int,max_shared:int)->DaStatus #export_c {
    return da_admit(DKP_NEEDS,DaDevice.{major=major,minor=minor,max_threads=max_threads,max_shared=max_shared});
}
// The checks a launch must pass, in order: chapter 11's head bounds, the
// prompt and chunk ranges, then every view's exact length. counts: conv, q,
// k, beta, decay, out and state view lengths.
dkp_check :: (tokens:int,chunk:int,heads:int,key_heads:int,conv:int,q:int,k:int,beta:int,decay:int,out:int,state:int)->DkStatus #export_c {
    if key_heads<1 || key_heads>16 || heads<key_heads || heads>48 || heads%key_heads!=0 {return DkStatus.shape;}
    if tokens<1 || tokens>4096 || chunk<1 || chunk>DKD_CHUNK {return DkStatus.range;}
    if conv!=tokens*(key_heads*256+heads*128) || q!=tokens*key_heads*128 || k!=q {return DkStatus.capacity;}
    if beta!=tokens*heads || decay!=beta || out!=beta*128 || state!=heads*16384 {return DkStatus.capacity;}
    return DkStatus.ok;
}
// Groups of a launch: one per value head and block of 16 value columns.
dkp_groups :: (heads:int)->int #export_c {return heads*8;}
```

A chunk of 17, a prompt of 4,097 tokens, 49 value heads, 3 value heads over
2 key heads, or any view one element too long or too short is refused with
its own status, and checks pin each boundary. `coax mutate --root-only`
leaves no surviving mutant in either device file.

### Checks before the device

The kernel's checks run its C build against the hand trace above (both chunk
sizes give outputs 2 and 3 and state 3 exactly), against `dl_scan_state`
from a nonzero carried state with four value heads over two key heads
(chunks of 1 and 3), against the binary64 oracle over 20 tokens with chunks
of 6 and 16, and against itself split at a chunk boundary (bit for bit).
The harness's runtime file adds one more: chapter 44's `dl_conv`,
`dl_gates`, `dl_qk`, then `dkd_chunk`, then `dl_gate_norm`, from chapter
11's packed inputs at width 128, against `pd_step` and this chapter's CPU
profile.

### On the RTX 4080

```shell
COAX_CC=gcc psionic/tests/check_delta_chunks_device.sh ~/models/elephant-1-b3f8a934
```

The harness runs those checks, emits the CUDA source with `coax cuda`, and
for each prompt length runs chapter 44's `dl_conv`, `dl_gates` and `dl_qk`
once, then compares and times the scan step alone: `dl_scan` as chapter 44's
program launches it, and `dkd_chunk` with chunks of 2, 4, 8 and 16 tokens,
each followed by `dl_gate_norm`. Each layer output is compared with the
`dl_scan` path, with this chapter's CPU profile (chunks of 64) and with
chapter 11's recurrence, and the carried state with chapter 11's; the device
draft of value heads 0 and 31 is compared with the C build of `dkd_chunk`.
Times are chapter 34's warm runs: one warm-up, 10 recorded runs,
`cm_two_pass` and `cm_compare`, the best chunk size against `dl_scan`.

The synthetic prompts are chapter 11's packed inputs at Elephant's linear-
layer shape (16 key heads, 32 value heads, width 128) with log A between
-4 and 0, so some heads keep almost all of their state across the prompt.
Lengths of 64, 256 and 1,024 tokens print the same comparisons; the run's
end, the 2,048-token prompt and the original weights:

```text
device: NVIDIA GeForce RTX 4080 (sm_89); dkd_chunk (128 threads, 26816 shared bytes): admitted
synthetic packed inputs at Elephant's linear-layer shape: 16 key heads, 32 value heads, width 128, log A in [-4, 0]
2048 tokens:
  dl_scan (chapter 44's recurrence, one item per value column): mean 13.274 ms, sd 0.369 ms, min 13.077 ms (10 warm runs)
  dl_scan path against chapter 11's recurrence: largest difference 2.15e-06, 1004607 of 8388608 bit-identical, within tolerance
 chunk 2:
  against the dl_scan path: largest difference 2.74e-06, 914000 of 8388608 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 1.42e-06, 1556487 of 8388608 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 1.55e-06, 1464497 of 8388608 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 2.83e-07, 118572 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 47.299 ms, sd 2.897 ms, min 45.455 ms (10 warm runs)
 chunk 4:
  against the dl_scan path: largest difference 2.86e-06, 889553 of 8388608 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 1.39e-06, 1506449 of 8388608 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 1.52e-06, 1420425 of 8388608 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 2.68e-07, 115073 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 26.040 ms, sd 1.159 ms, min 25.433 ms (10 warm runs)
 chunk 8:
  against the dl_scan path: largest difference 2.74e-06, 862287 of 8388608 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 1.42e-06, 1432303 of 8388608 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 1.55e-06, 1356108 of 8388608 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 2.98e-07, 116596 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 15.067 ms, sd 0.479 ms, min 14.845 ms (10 warm runs)
 chunk 16:
  against the dl_scan path: largest difference 2.74e-06, 835444 of 8388608 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 1.43e-06, 1351409 of 8388608 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 1.56e-06, 1288451 of 8388608 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 3.87e-07, 115890 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 9.716 ms, sd 0.354 ms, min 9.477 ms (10 warm runs)
  chunk 16, heads 0 and 31: device draft against the C build: largest difference 0, 524288 of 524288 bit-identical, within tolerance
  best chunk 16; chapter 34 verdict: dkd_chunk is faster than dl_scan (time ratio dl_scan/dkd_chunk 1.37, standard error 0.162 ms)
mean scan times (ms): tokens, dl_scan, chunk 2, chunk 4, chunk 8, chunk 16
  64, 0.244, 1.572, 0.885, 0.509, 0.324
  256, 0.966, 6.263, 3.513, 2.020, 1.287
  1024, 3.978, 23.648, 13.187, 7.620, 4.810
  2048, 13.274, 47.299, 26.040, 15.067, 9.716
original Elephant linear layer 0 over the 86-token recorded decision prompt:
  dl_scan (chapter 44's recurrence, one item per value column): mean 0.329 ms, sd 0.002 ms, min 0.328 ms (10 warm runs)
  dl_scan path against chapter 11's recurrence: largest difference 2.98e-07, 43948 of 352256 bit-identical, within tolerance
 chunk 2:
  against the dl_scan path: largest difference 2.09e-07, 40704 of 352256 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 2.38e-07, 83333 of 352256 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 2.38e-07, 78397 of 352256 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 2.29e-05, 127766 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 2.113 ms, sd 0.001 ms, min 2.111 ms (10 warm runs)
 chunk 4:
  against the dl_scan path: largest difference 2.09e-07, 40617 of 352256 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 2.38e-07, 81803 of 352256 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 2.38e-07, 77073 of 352256 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 1.53e-05, 128402 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 1.209 ms, sd 0.002 ms, min 1.205 ms (10 warm runs)
 chunk 8:
  against the dl_scan path: largest difference 2.38e-07, 39792 of 352256 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 2.38e-07, 79572 of 352256 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 2.98e-07, 74642 of 352256 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 1.14e-05, 126348 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 0.695 ms, sd 0.001 ms, min 0.693 ms (10 warm runs)
 chunk 16:
  against the dl_scan path: largest difference 2.38e-07, 38674 of 352256 bit-identical, within tolerance
  against chapter 42's CPU chunked profile (chunk 64): largest difference 2.38e-07, 76166 of 352256 bit-identical, within tolerance
  against chapter 11's recurrence: largest difference 2.38e-07, 72424 of 352256 bit-identical, within tolerance
  carried state against chapter 11's: largest difference 5.72e-06, 126419 of 524288 bit-identical, within tolerance
  dkd_chunk: mean 0.464 ms, sd 0.001 ms, min 0.463 ms (10 warm runs)
  chunk 16, heads 0 and 31: device draft against the C build: largest difference 0, 22016 of 22016 bit-identical, within tolerance
  best chunk 16; chapter 34 verdict: dkd_chunk is slower than dl_scan (time ratio dl_scan/dkd_chunk 0.71, standard error 0.001 ms)
chapter 42 on CUDA: every comparison within the frozen tolerance
```

Every comparison at every length and chunk size stayed within the frozen
bound; the largest output difference from chapter 11's recurrence on the
synthetic prompts was below `3e-6`. The device and the C build of
`dkd_chunk` agreed bit for bit.

The chunk-size sweep, mean milliseconds for the scan step:

| Tokens | `dl_scan` | chunk 2 | chunk 4 | chunk 8 | chunk 16 |
| --- | --- | --- | --- | --- | --- |
| 64 | 0.244 | 1.572 | 0.885 | 0.509 | 0.324 |
| 256 | 0.966 | 6.263 | 3.513 | 2.020 | 1.287 |
| 1024 | 3.978 | 23.648 | 13.187 | 7.620 | 4.810 |
| 2048 | 13.274 | 47.299 | 26.040 | 15.067 | 9.716 |
| 86 (layer 0, original weights) | 0.329 | 2.113 | 1.209 | 0.695 | 0.464 |

Chunks of 16, the largest the shared tiles allow, were fastest at every
length, and that is the recorded choice. The time falls almost in proportion
to the chunk size, so each chunk's fixed cost (five barriers, the decay
products, the 16 x 16 tables and the serial forward substitution of each
column) dominates the work that grows with the chunk. Against `dl_scan`,
chapter 34's verdict is: slower at 64 and 256 tokens (time ratio 0.75),
noisy at 1,024 (`dl_scan`'s own runs varied, ratio 0.83), and faster at 2,048
tokens, 9.7 ms against 13.3 ms (ratio 1.37). On the 86-token original-weight
prompt the chunked kernel is slower (0.46 ms against 0.33 ms). `dl_scan` stays
the better kernel for Elephant's short decision prompts; the chunked kernel
starts to pay for itself between 1,024 and 2,048 tokens.

### Original weights

With Elephant's original files the harness maps linear layer 0's tensors
with the model-io layer map (`pm_layer`), gathers the embedding rows of the
86 tokens of the recorded decision request (`psionic/tests/elephant_reference_fixture.py`),
and runs `ql_step`'s input norm and BF16 projections in Coax on the CPU
(`dkr_pack` in the harness's runtime file packs them exactly as `ql_step`
does). The device then runs the layer core above. The outputs agree with
chapter 11's recurrence within `3e-7` and the carried state within `2.3e-5`
at every chunk size: the real state holds larger values than the synthetic
one, and the bound scales with them.

## What this does not do yet

- **Chapter 44's decision program.** `dkd_chunk` runs between chapter 44's
  own kernels in this chapter's harness, but chapter 44's plan still
  launches `dl_scan` and `dl_scan_state`; adopting the kernel needs the
  plan, its forked-prefix state (which `dl_scan_state` keeps column by
  column) and the whole decision's qualification. On Elephant's 86-token
  decision prompt it would be slower, so adoption should depend on the
  prompt length.
- **Snapshots on the device.** The kernel carries the state between chunks
  but does not write chunk-boundary snapshots.
- **A faster kernel.** Chunks of 16 are fastest here and still pay a large
  fixed cost per chunk; larger chunks need more than chapter 37's 32 KiB of
  shared arrays, and tensor-core products need chapter 68's group kernels to
  accept the matrix primitive. llama.cpp uses chunks of 64.
- **Other devices and layers.** One RTX 4080, one original layer on one
  prompt; the kernel has not been compiled with `coax metal-compute`.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [Hybrid forward primitives](../docs/hybrid-kernels.md).
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): chunked Gated
  DeltaNet.
- Yang et al., *Parallelizing Linear Transformers with the Delta Rule over
  Sequence Length* (2024), and *Gated Delta Networks* (2024): the WY form and
  the gated chunk recurrence.
