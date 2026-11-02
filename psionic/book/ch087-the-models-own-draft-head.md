# 87. The model's own draft head

> **Status: partly implemented.** A draft head on a small synthetic target,
> its drafts verified by chapter 75's block rule, greedy parity, the exact
> distribution check on a tiny vocabulary, measured acceptance and chapter
> 76's enablement rule on a CPU timing all run. The inventory of Mistral
> Large 4's real draft head is read from one shard header and checked
> against its config. Running the real head does not yet; see
> [What this does not do yet](#what-this-does-not-do-yet).

Chapter 75 let a separate small model guess tokens for a large one. Some
models ship their own guesser instead: a few extra layers, trained with the
model, that read the model's last hidden state and guess the tokens after
the next one. This is multi-token prediction (MTP). Mistral Large 4 ships
such a head, and so does chapter 164's Nemotron Lightning. Because the head
reads the target's hidden state, it needs no separate prompt pass and no
second tokenizer, and it can be much smaller than a separate draft model.

The head changes where drafts come from, not who decides. Every draft still
goes through chapter 75's acceptance rule, so the output is exactly what the
target alone would produce: the same tokens under greedy decoding, and the
same distribution under sampling. This chapter builds that on a model small
enough to check by hand and by exact enumeration, measures how often the
head is right, applies chapter 76's rule to a CPU timing, and lists the real
Large 4 head's tensors from the checkpoint's header.

```shell
coax/bin/coax run psionic/examples/draft-head.coax
coax/bin/coax run psionic/examples/draft-head-refuse.coax
coax/bin/coax test psionic/lib/draft-head.coax
coax/bin/coax mutate psionic/lib/draft-head.coax --root-only --jobs 8
python3 -I psionic/tests/large4_draft_head_manifest.py --fetch DIR
python3 -I psionic/tests/large4_draft_head_manifest.py DIR
```

The [library](../lib/draft-head.coax), its
[check oracle](../tests/draft-head-oracle.coax), the
[Large 4 inventory](../fixtures/mistral-large4-draft-head.coax) with
[the tool that writes it](../tests/large4_draft_head_manifest.py), the
[example](../examples/draft-head.coax) and the
[refusal example](../examples/draft-head-refuse.coax) are the whole
implementation. Chapter 75's [`gx_block`](../lib/gptoss/speculation.coax)
verifies every block, and chapter 76's
[`sg_decide`](../lib/speculation-gate.coax) decides whether speculation is
worth turning on.

## The target and its head

The target is a recurrent cell. Its state is a hidden vector `h` of `width`
values. Reading token `x` gives the next state, and the next state gives the
distribution of the token after `x`:

```text
s    = recur * h + embed[x]
next = s / (1 + |s|)            one value at a time, so every value is in (-1, 1)
p    = softmax(out * next)
```

A transformer's KV cache plays the same role as `h`: it is everything the
model has read. The draft head is a second cell of the same shape. It starts
from the target's hidden state `h` and the token the target just produced,
and runs its own cell on its own guesses for `k` steps. It shares the
target's readout `out`, as MTP heads usually share the model's output layer.

Profile `draft-head-cpu-f64-v1` fixes the arithmetic: binary64 on the CPU,
plain sums in a fixed order, and weights at most 8 in size. With every hidden
value in (-1, 1) and a width of at most 16, a logit is at most 128 in size,
so the softmax needs no shift to stay finite and positive. Both
models call the same two procedures, so a target step computed while
verifying is the same arithmetic, bit for bit, as the step computed without
speculation. That is what makes greedy parity exact rather than approximate.

<!-- source: psionic/lib/draft-head.coax types -->
```jai
DhStatus :: enum u8 {ok;shape;token;uniform;weight;manifest;}
DH_WEIGHT_MAX :: 8.0;
DH_VOCAB_MAX :: 64;
DH_WIDTH_MAX :: 16;
DH_DRAFTS_MAX :: 8;
DH_STEPS_MAX :: 4096;
// One recurrent cell: s = recur * h + embed[token], next = s / (1 + abs(s)), and the
// next-token distribution softmax(out * next). recur is width x width,
// embed and out are vocab x width, row-major.
DhCell :: struct {vocab:int;width:int;recur:[]f64;embed:[]f64;out:[]f64;}
// A generation's outcome and its counts. written: tokens in out. blocks:
// draft-and-verify rounds. drafted and accepted: draft tokens proposed and
// kept. committed: tokens the blocks emitted before truncation to steps.
// target and head: cell steps each model ran. used: uniforms consumed.
DhRun :: struct {status:DhStatus;written:int;blocks:int;drafted:int;accepted:int;committed:int;target:int;head:int;used:int;}
```

<!-- source: psionic/lib/draft-head.coax cell -->
```jai
// The cell's step and its distribution. Both models use these two
// procedures, so a target step computed while verifying is the same
// arithmetic, bit for bit, as the step computed without speculation.
dh_step :: (c:DhCell,h:[]f64,token:int,next:[]f64) {
    w:=c.width;
    for i:0..<w {s:=c.embed[token*w+i];for j:0..<w {s+=c.recur[i*w+j]*h[j];}next[i]=s/(1+abs(s));}
}
// Weights are at most 8 in size and every hidden value is in (-1, 1), so a
// logit is at most 8 x 16 = 128 in size: exp needs no shift to stay finite
// and positive.
dh_probs :: (c:DhCell,h:[]f64,p:[]f64) {
    w:=c.width;total:=0.0;
    for v:0..<c.vocab {s:=0.0;for j:0..<w {s+=c.out[v*w+j]*h[j];}p[v]=exp(s);total+=p[v];}
    for v:0..<c.vocab {p[v]/=total;}
}
```

The state between steps is a pair: the hidden state after every committed
token except the last, and that last token, already emitted but not yet
read. After a prompt, the pending token is the prompt's last token. The
target alone reads the pending token, picks the next one, and repeats:

<!-- source: psionic/lib/draft-head.coax generate -->
```jai
// Target alone: read the pending token, emit the next one, repeat. greedy
// takes the lowest-ID maximum (gx_argmax); otherwise each token draws one
// uniform through chapter 64's gs_pick.
dh_generate :: (t:DhCell,prompt:[]int,steps:int,greedy:bool,uniforms:[]f64,out:[]int)->DhRun {
    r:=DhRun.{status=dh_check(t,t,prompt,steps,uniforms,out)};if r.status!=DhStatus.ok {return r;}
    h:[DH_WIDTH_MAX]f64;next:[DH_WIDTH_MAX]f64;p:[DH_VOCAB_MAX]f64;dh_prefill(t,prompt,h[..]);
    pending:=prompt[prompt.count-1];
    while r.written<steps {
        dh_step(t,h[..],pending,next[..]);dh_probs(t,next[..],p[..]);r.target+=1;
        if greedy {pending=gx_argmax(p[0..<t.vocab]);}
        else {
            if r.used==uniforms.count {r.status=DhStatus.uniform;return r;}
            pick:=gs_pick(p[0..<t.vocab],uniforms[r.used]);r.used+=1;#assert pick.status==GStatus.ok;
            pending=pick.token;
        }
        out[r.written]=pending;r.written+=1;for j:0..<t.width {h[j]=next[j];}
    }
    return r;
}
```

## A block by hand

Take width 1 and two tokens, with `out = [1, -1]`, so token 0 wins when
`h > 0`, token 1 wins when `h < 0`, and token 0 wins the tie at `h = 0`
(`gx_argmax` keeps the lowest ID). The target has `recur = -4` and
`embed = [1, -1]`. Start from the prompt `[0]`, so `h = 0` and token 0 is
pending.

| Step | Reads | s | next | Emits |
| --- | --- | --- | --- | --- |
| 1 | 0 at h = 0 | 0 + 1 = 1 | 1/2 | 0 |
| 2 | 0 at h = 1/2 | -2 + 1 = -1 | -1/2 | 1 |
| 3 | 1 at h = -1/2 | 2 - 1 = 1 | 1/2 | 0 |

The target alone emits 0 1 0 1 0 1. Every value in the table is exact in
binary64.

Head A has `recur = -3` and the same `embed`. From `h = 0` and pending token
0 it computes `s = 1`, `g = 1/2`, and drafts 0. From `g = 1/2` and its own
draft 0 it computes `s = -3/2 + 1 = -1/2`, `g = -1/3`, and drafts 1.

Now the target verifies, with `k = 2`. It reads the pending token and both
drafts in order, keeping every hidden state, and each state gives one row:

- Row 0, after reading 0 from `h = 0`: `h = 1/2`, so the target's choice is
  0. Draft 0 is 0: accepted.
- Row 1, after reading draft 0: `h = -1/2`, choice 1. Draft 1 is 1: accepted.
- Row 2, the bonus row, after reading draft 1: `h = 1/2`, choice 0.

Both drafts pass, so the block emits 0 1 0: three tokens for three target
steps and two head steps. The next state is the hidden state after the
accepted prefix, `h = 1/2`, with 0 pending. It was computed during
verification, so nothing is recomputed.

Head B has `recur = -1`. It drafts 0 (`g = 1/2`), then 0 again
(`s = -1/2 + 1 = 1/2`, `g = 1/3`). Row 0 accepts the first draft. Row 1 says
1, so the second draft is rejected and row 1's choice, 1, is emitted in its
place. The block emits 0 1, and the next state is the hidden state after reading
the accepted draft, `h = -1/2`, with 1 pending. A wrong guess costs a target step but
never changes the output: six tokens through head B are 0 1 0 1 0 1, the
same as the target alone. The checks run exactly this trace.

## Drafting and verifying

The head runs its own cell `k` times. Greedy drafts take the head's maximum;
sampled drafts draw one uniform each from the head's own distribution, which
becomes the draft row `q` that the acceptance rule needs:

<!-- source: psionic/lib/draft-head.coax draft -->
```jai
// The draft head: from the target's hidden state h and the pending token,
// run the head's own cell k times on its own guesses. Row j of q is the
// head's distribution for draft j. Greedy drafts take the head's maximum;
// sampled drafts draw one uniform each. Returns the uniforms used.
dh_draft :: (head:DhCell,h:[]f64,pending:int,k:int,greedy:bool,uniforms:[]f64,drafts:[]int,q:[]f64)->int {
    g:[DH_WIDTH_MAX]f64;next:[DH_WIDTH_MAX]f64;for i:0..<head.width {g[i]=h[i];}
    token:=pending;used:=0;v:=head.vocab;
    for j:0..<k {
        dh_step(head,g[..],token,next[..]);row:=q[j*v..<(j+1)*v];dh_probs(head,next[..],row);
        token=gx_argmax(row);
        if !greedy {token=gs_pick(row,uniforms[used]).token;used+=1;}
        drafts[j]=token;for i:0..<head.width {g[i]=next[i];}
    }
    return used;
}
```

Verification is chapter 75's `gx_block`, unchanged. The target computes
`k + 1` rows: one for each draft, conditioned on the pending token and the
drafts before it, and a bonus row. Under greedy decoding a draft is accepted
when it equals the row's lowest-ID maximum. Under sampling, draft `x` is
accepted with probability `min(1, p[x] / q[x])`; on the first rejection the
replacement is drawn from the normalized positive residual
`max(p - q, 0)`. If every draft passes, the bonus row adds one more token.
A sampled block uses `3k + 1` uniforms: `k` for drafts, two for each
decision and one for the bonus.

<!-- source: psionic/lib/draft-head.coax speculate -->
```jai
// Draft k tokens with the head, then verify them with chapter 75's block
// rule. The target reads the pending token and the k drafts in order, one
// step each, keeping every hidden state: row i of p is its distribution
// after the pending token and drafts 0..<i. gx_block accepts a prefix and
// adds one correction or, when all k pass, one bonus token. The next state
// is the hidden state after the accepted prefix, which was already
// computed, so nothing is recomputed on a rejection.
dh_speculate :: (t:DhCell,head:DhCell,prompt:[]int,steps:int,k:int,greedy:bool,uniforms:[]f64,out:[]int)->DhRun {
    r:=DhRun.{status=dh_check(t,head,prompt,steps,uniforms,out)};if r.status!=DhStatus.ok {return r;}
    if k<1 || k>DH_DRAFTS_MAX {r.status=DhStatus.shape;return r;}
    w:=t.width;v:=t.vocab;
    hs:[(DH_DRAFTS_MAX+2)*DH_WIDTH_MAX]f64;p:[(DH_DRAFTS_MAX+1)*DH_VOCAB_MAX]f64;q:[DH_DRAFTS_MAX*DH_VOCAB_MAX]f64;
    drafts:[DH_DRAFTS_MAX]int;emitted:[DH_DRAFTS_MAX+1]int;stage:[DH_DRAFTS_MAX+1]int;residual:[DH_VOCAB_MAX]f64;scratch:[DH_VOCAB_MAX]f64;
    zeros:[2*DH_DRAFTS_MAX+1]f64;
    dh_prefill(t,prompt,hs[0..<w]);pending:=prompt[prompt.count-1];
    while r.written<steps {
        need:=ifx greedy then 0 else 3*k+1;
        if r.used+need>uniforms.count {r.status=DhStatus.uniform;return r;}
        r.used+=dh_draft(head,hs[0..<w],pending,k,greedy,uniforms[r.used..<uniforms.count],drafts[..],q[..]);
        token:=pending;
        for i:0..<k+1 {
            dh_step(t,hs[i*w..<hs.count],token,hs[(i+1)*w..<hs.count]);dh_probs(t,hs[(i+1)*w..<hs.count],p[i*v..<p.count]);r.target+=1;
            if i<k {token=drafts[i];}
        }
        u:=zeros[0..<2*k+1];if !greedy {u=uniforms[r.used..<r.used+2*k+1];r.used+=2*k+1;}
        b:=gx_block(v,p[0..<(k+1)*v],q[0..<k*v],drafts[0..<k],u,greedy,emitted[..],stage[..],residual[..],scratch[..],false);
        #assert b.status==GStatus.ok;
        for i:0..<b.written {if r.written<steps {out[r.written]=emitted[i];r.written+=1;}}
        r.blocks+=1;r.drafted+=k;r.accepted+=b.accepted;r.committed+=b.written;r.head+=k;
        a:=b.accepted+1;for j:0..<w {hs[j]=hs[a*w+j];}pending=emitted[b.written-1];
    }
    return r;
}
```

Chapter 75 kept complete-cache snapshots so a rejection could roll the
target back. Here the target's state is small, so verification keeps every
hidden state it computes and the rollback is a choice among them: the state
after the accepted prefix. A KV cache would instead truncate to that length
(or restore a ring's overwritten slots, as chapter 75 does).

Refusals come before any step: a cell whose sizes or slices do not match, a
head whose vocabulary or width differs from the target's, a draft count
outside 1 to 8, a token outside the vocabulary, a weight that is not finite
or is larger than 8 in size, a uniform outside [0, 1), or too few uniforms
for the next block. Bounded weights keep every distribution finite and every
draft's probability positive, which `gx_block` requires.

## The workload

Workload `dh-walk-v1` has a vocabulary of 8 and width 4. Its weights come
from a fixed integer recurrence. The head is the target's own cell with
`recur` and `embed` rounded to multiples of 1/2: a cheaper, coarser copy,
standing in for a head trained to imitate the target. The prompt is 1 2 3
and the run is 48 tokens.

<!-- source: psionic/lib/draft-head.coax fixtures -->
```jai
// Weights from a fixed integer recurrence, scaled to [-scale, scale):
// value i is (x_i - 32768) / 32768 * scale, x_i = (seed * 40503 + i * 2654435761 + 12345) mod 65536.
dh_fill :: (seed:int,scale:f64,out:[]f64) {
    for i:0..<out.count {x:=(seed*40503+i*2654435761+12345)%65536;out[i]=cast(f64)(x-32768)/32768*scale;}
}
// A head that is the target's cell with every weight rounded to a multiple
// of step: a cheaper, less exact copy that shares the target's readout.
dh_round :: (src:[]f64,step:f64,out:[]f64) {for i:0..<src.count {out[i]=floor(src[i]/step+0.5)*step;}}
// Uniforms in [0, 1) from x = (x * 1103515245 + 12345) mod 2^31.
dh_uniforms :: (seed:int,out:[]f64) {
    x:=seed%2147483648;for i:0..<out.count {x=(x*1103515245+12345)%2147483648;out[i]=cast(f64)x/2147483648;}
}
```

<!-- source: psionic/examples/draft-head.coax example -->
```jai
main :: () {
    // Workload dh-walk-v1: vocabulary 8, width 4, prompt 1 2 3, 48 tokens.
    recur:[16]f64;embed:[32]f64;out:[32]f64;hr:[16]f64;he:[32]f64;
    dh_fill(1,4,recur[..]);dh_fill(2,1,embed[..]);dh_fill(3,4,out[..]);
    dh_round(recur[..],0.5,hr[..]);dh_round(embed[..],0.5,he[..]);
    t:=DhCell.{vocab=8,width=4,recur=recur[..],embed=embed[..],out=out[..]};
    head:=DhCell.{vocab=8,width=4,recur=hr[..],embed=he[..],out=out[..]};
    prompt:=[3]int.[1,2,3];none:[1]f64;base:[48]int;
    g:=dh_generate(t,prompt[..],48,true,none[0..<0],base[..]);
    print("target alone, greedy: ");for x:base {print("%",x);}print(" (% target steps)\n",g.target);
    print("draft head, greedy:\n  k  same  blocks  accepted/drafted  committed  target steps  head steps\n");
    for k:1..<5 {
        o:[48]int;r:=dh_speculate(t,head,prompt[..],48,k,true,none[0..<0],o[..]);
        same:=true;for i:0..<48 {if o[i]!=base[i] {same=false;}}
        print("  %  %  %      %/%            %         %            %\n",k,same,r.blocks,r.accepted,r.drafted,r.committed,r.target,r.head);
    }
    exact:[48]int;r:=dh_speculate(t,t,prompt[..],48,4,true,none[0..<0],exact[..]);
    print("the target as its own head, k 4: accepted %/% in % blocks\n",r.accepted,r.drafted,r.blocks);
    u:[4096]f64;dh_uniforms(7,u[..]);sampled:[48]int;
    r=dh_speculate(t,head,prompt[..],48,2,false,u[..],sampled[..]);
    print("draft head, sampled, k 2, uniforms seed 7: accepted %/% in % blocks, % uniforms\n",r.accepted,r.drafted,r.blocks,r.used);

    // Chapter 76's rule on CPU times: seven paired runs, each 200 greedy
    // generations of 48 tokens, target alone and with the head at k 4.
    plain:[7]int;spec:[7]int;same:=true;
    for run:0..<7 {
        a:[48]int;b:[48]int;n:=0;
        start:=clock();for i:0..<200 {n+=dh_generate(t,prompt[..],48,true,none[0..<0],a[..]).written;}plain[run]=(clock()-start)*1000+1;
        start=clock();for i:0..<200 {n-=dh_speculate(t,head,prompt[..],48,4,true,none[0..<0],b[..]).written;}spec[run]=(clock()-start)*1000+1;
        for i:0..<48 {if a[i]!=b[i] {same=false;}}if n!=0 {same=false;}
    }
    d:=sg_decide(same,plain[..],spec[..]);
    print("chapter 76's rule on this CPU: speculation enabled: %\n",sg_enabled(d));

    // Large 4's draft head, from one shard header and params.json.
    v:=dh_large4_inventory(DH_LARGE4,DH_PINNED_MANIFEST);
    print("Large 4 mtp_dspark: %, % tensors, % BF16 values, % bytes = shard % - header %\n",v.status,v.tensors,v.values,
        2*v.values,DH_PINNED_SHARD_BYTES,DH_PINNED_HEADER_BYTES);
}
```

Expected output (the timing decides only the enabled line, which is false on
any run; see below):

```text
target alone, greedy: 501155401554016540165401654016540165401655116551 (48 target steps)
draft head, greedy:
  k  same  blocks  accepted/drafted  committed  target steps  head steps
  1  true  29      20/29            49         58            29
  2  true  21      29/42            50         63            42
  3  true  19      32/57            51         76            57
  4  true  13      39/52            52         65            52
the target as its own head, k 4: accepted 40/40 in 10 blocks
draft head, sampled, k 2, uniforms seed 7: accepted 30/36 in 18 blocks, 126 uniforms
chapter 76's rule on this CPU: speculation enabled: false
Large 4 mtp_dspark: ok, 38 tensors, 2099969024 BF16 values, 4199938048 bytes = shard 4199942840 - header 4792
```

Every row of the table reports `same true`: speculative output equals
target-only output, token for token, at every `k`. The `committed` column can
pass 48 because the last block may emit more tokens than are still needed.

Acceptance is between 56% and 75% of drafts, and a block commits between 1.7
and 4 tokens. The target, used as its own head, is always right: 40 of 40
drafts, every block four drafts and a bonus. The target steps column shows
the cost on a serial CPU: at `k = 4` the target runs 65 steps for 48 tokens,
plus 52 head steps, where the target alone runs 48. Verification only pays
when the target can score a block's `k + 1` rows for about the price of one,
which is what a batched device does (chapter 76) and a serial loop does not.

## Chapter 76's rule on this CPU

The example times seven paired runs, each 200 greedy generations of 48
tokens, with the target alone and with the head at `k = 4`, using the C
host's processor clock. Chapter 76's `sg_decide` turns speculation on only
if the saving, less two standard errors, is at least 5% of the target-only
time, and only if both outputs match. On an Intel i7-14700K the debug build
measured a target-only mean of 7.0 ms and a speculative mean of 20.4 ms per
200 generations: speculation took 2.9 times as long, and the verdict was
off. The release build measured 0.80 ms and 2.24 ms; its spread was over a
tenth of the mean, so the verdict was "too noisy". Either way speculation
stays off. The transcript records only that outcome, because the times
change from run to run. No speedup is claimed.

## Sampling keeps the target's distribution

Greedy parity is checked token for token. Sampling needs a stronger check:
the speculative process must emit every sequence with exactly the
probability the target gives it. The [oracle](../tests/draft-head-oracle.coax)
computes that probability by enumeration. For each possible draft block it
multiplies the head's probabilities of the drafts, then follows every
branch: accepted up to draft `i` and rejected there, with each residual
token, or all accepted, with each bonus token. Branches that disagree with
the sequence end; branches that commit only part of it continue from their
new state. The oracle computes the acceptance and the residual itself; it
does not call `gx_decide`.

On a cell with 3 tokens and width 2, for `k` of 1, 2 and 3 and two weight
seeds, the speculative probability of every one of the 27 three-token
sequences equals the target's own product of conditionals within `1e-14`,
and the 27 probabilities sum to 1 within `1e-14`. A second check runs the
sampler itself: 2,048 seeded two-token runs at `k = 3`, with a head whose
weights are rounded to multiples of 4 so that it is often wrong, and every
one of the 9 counts within 5 standard deviations (plus 1) of the exact
expectation.

## Refusals

<!-- source: psionic/examples/draft-head-refuse.coax example -->
```jai
main :: () {
    recur:[16]f64;embed:[32]f64;out:[32]f64;
    dh_fill(1,4,recur[..]);dh_fill(2,1,embed[..]);dh_fill(3,4,out[..]);
    t:=DhCell.{vocab=8,width=4,recur=recur[..],embed=embed[..],out=out[..]};
    // A head of width 3 cannot read the target's width-4 hidden state.
    head:=DhCell.{vocab=8,width=3,recur=recur[0..<9],embed=embed[0..<24],out=out[0..<24]};
    prompt:=[3]int.[1,2,3];none:[1]f64;o:[48]int;
    r:=dh_speculate(t,head,prompt[..],48,4,true,none[0..<0],o[..]);
    print("head of width 3 on a width-4 target: %, % tokens written\n",r.status,r.written);
    // A uniform of exactly 1 is outside [0, 1): refused before any step.
    u:[64]f64;dh_uniforms(7,u[..]);u[10]=1;
    r=dh_speculate(t,t,prompt[..],48,2,false,u[..],o[..]);
    print("uniform 10 is 1: %, % target steps\n",r.status,r.target);
    // Large 4's params.json says mtp.n_layers 3; a config claiming 2 stops at
    // the first tensor the header holds that 2 layers would not.
    s:=DH_LARGE4;s.layers=2;v:=dh_large4_inventory(s,DH_PINNED_MANIFEST);
    buf:[160]u8;n:=dh_line(dh_large4_tensor(s,v.bad),buf[..]);
    print("2 draft-head layers: %, expected tensor % is %\n",v.status,v.bad,cast(string)buf[0..<n]);
}
```

```text
head of width 3 on a width-4 target: shape, 0 tokens written
uniform 10 is 1: uniform, 0 target steps
2 draft-head layers: manifest, expected tensor 25 is backbone.mask_embedding BF16 2,6144
```

## Large 4's draft head

Large 4's `params.json` (2,954 bytes, SHA-256
`dc6617d800bacc9d3caa41570f885467f733a7cee25f8eef3a60e01c1ff98343`) at
revision `395730aefd2f928daaf7b69fbd9fefab0ee69b64` describes the head:

```text
mtp: n_layers 3, depth 3, norm_strategy "eagle",
     target_activation_layers "1,18,36,54,71",
     ragged_attention "2048,2048,2048",
     dspark: head_type "rnn", markov_rank 512, draft_input "position_specific"
```

The checkpoint's index places all 38 `mtp_dspark.` tensors, and nothing
else, in the last of its 54 shards. That shard is 4,199,942,840 bytes. Its
safetensors header is 4,792 bytes, so [the tool](../tests/large4_draft_head_manifest.py)
reads only that header and the two JSON files, about 4.8 MB, and writes the
[inventory](../fixtures/mistral-large4-draft-head.coax). The tensors are
BF16, not FP8, and they tile the shard exactly.

The library derives every expected shape from the config's numbers and
compares the inventory line by line:

<!-- source: psionic/lib/draft-head.coax large4 -->
```jai
// The numbers in Large 4's params.json that set the draft head's shapes:
// dim, vocab_size, n_heads, qk_nope_head_dim, qk_rope_head_dim, v_head_dim,
// kv_lora_rank, hidden_dim, mtp.n_layers, the count of
// mtp.target_activation_layers ("1,18,36,54,71") and mtp.dspark.markov_rank.
DhLarge4 :: struct {dim:int;vocab:int;heads:int;nope:int;rope:int;value:int;rank:int;hidden:int;layers:int;targets:int;markov:int;}
DH_LARGE4 :: DhLarge4.{dim=6144,vocab=204800,heads=64,nope=64,rope=64,value=128,rank=512,hidden=24576,layers=3,targets=5,markov=512};
DH_LAYER_NAMES :: [10]string.["attention.kv_a_norm.weight","attention.wkv_a_with_mqa.weight","attention.wkv_b.weight","attention.wo.weight",
    "attention.wq.weight","attention_norm.weight","feed_forward.w1.weight","feed_forward.w2.weight","feed_forward.w3.weight","ffn_norm.weight"];
DhTensor :: struct {name:string;layer:int;rows:int;cols:int;}
// Expected tensor e, in name order: 5 head and backbone tensors, 10 a
// layer, then 3 more. cols 0 means one dimension.
dh_large4_tensor :: (s:DhLarge4,e:int)->DhTensor {
    m:=s.markov;d:=s.dim;
    names:=[8]string.["autoregressive_head.embedding.weight","autoregressive_head.joint_proj.bias","autoregressive_head.joint_proj.weight",
        "autoregressive_head.output.weight","backbone.final_norm.weight","backbone.mask_embedding","backbone.target_norm.weight","backbone.target_projection.weight"];
    outer:=[8]int.[s.vocab,3*m,3*m,s.vocab,d,2,d,d];inner:=[8]int.[m,0,d+2*m,m,0,d,0,s.targets*d];
    top:=e;k:=e-5-10*s.layers;if k>=0 {top=5+k;}
    if e<5 || k>=0 {return DhTensor.{name=names[top],layer=-1,rows=outer[top],cols=inner[top]};}
    l:=(e-5)/10;i:=e-5-10*l;qk:=s.nope+s.rope;
    rows:=[10]int.[s.rank,s.rank+s.rope,s.heads*(s.nope+s.value),d,s.heads*qk,d,s.hidden,d,s.hidden,d];
    cols:=[10]int.[0,d,s.rank,s.heads*s.value,d,0,d,s.hidden,d,0];
    return DhTensor.{name=DH_LAYER_NAMES[i],layer=l,rows=rows[i],cols=cols[i]};
}
```

<!-- source: psionic/lib/draft-head.coax inventory -->
```jai
// The manifest must be exactly the expected lines, in order. On a mismatch
// bad names the first line that differs; values counts the parameters.
DhInventory :: struct {status:DhStatus;tensors:int;values:int;bad:int;}
dh_large4_inventory :: (s:DhLarge4,manifest:string)->DhInventory {
    r:=DhInventory.{bad=-1};expected:=8+10*s.layers;at:=0;buf:[160]u8;
    for e:0..<expected {
        t:=dh_large4_tensor(s,e);n:=dh_line(t,buf[..]);
        if at+n>=manifest.count || manifest[at..<at+n]!=cast(string)buf[0..<n] || manifest[at+n]!='\n' {r.status=DhStatus.manifest;r.bad=e;return r;}
        size:=t.rows;if t.cols!=0 {size*=t.cols;}at+=n+1;r.tensors+=1;r.values+=size;
    }
    if at!=manifest.count {r.status=DhStatus.manifest;r.bad=expected;}
    return r;
}
```

The shapes say what the head is made of:

- `target_projection` is 6,144 x 30,720: it reads the hidden states of the
  five target layers named in `target_activation_layers` (5 x 6,144), not
  only the last one.
- Each of the three backbone layers is a latent-attention layer like the
  target's (chapter 84): `wq` is 64 heads x (64 + 64) = 8,192 rows,
  `wkv_a_with_mqa` is 512 + 64 = 576 rows, `wkv_b` is 64 x (64 + 128) =
  12,288 rows, and a dense SwiGLU block of 24,576.
- The `autoregressive_head` works in `markov_rank` 512: its embedding and
  output tables are 204,800 x 512. `joint_proj` is 1,536 x 7,168, which is
  3 x 512 by 6,144 + 2 x 512. Those products match the config; what the
  recurrent head does with them is not confirmed against Mistral's code.
- `mask_embedding` is 2 x 6,144.

In all, 2,099,969,024 values, which is 4,199,938,048 bytes: the shard minus
its header. Each config number moved by one is refused at its first
dependent tensor, which the checks run for every number.

## What this does not do yet

- **The real head.** No Large 4 weight is read. Running the head needs its
  4.2 GB shard, the hidden states of target layers 1, 18, 36, 54 and 71, and
  Large 4's layers themselves, which are 1,060.6 GB (chapter 86).
- **The head's own design.** The synthetic head is one recurrent cell that
  reads the last hidden state. Large 4's reads five layers' states through a
  projection and a normalization ("eagle"), runs three attention layers with
  a mask embedding and position-specific inputs, and drafts through a
  rank-512 recurrent head. None of that is implemented, and its exact
  semantics are not pinned.
- **Measured acceptance on Large 4.** The acceptance reported here is for the
  synthetic workload. The draft's goal of acceptance on a named Large 4
  workload needs the real head and reference drafts.
- **A training run.** The head is set, not trained: the target's own weights
  rounded to multiples of 1/2.
- **Batched verification.** The target scores the block's rows one step at a
  time. Chapter 76's device path scores a block in one batch; joining it to
  a draft head, and a timing where speculation can pay, remain.

[Previous: sparse attention and shared layers](ch086-sparse-attention-and-shared-layers.md) ·
[Contents](SUMMARY.md)
