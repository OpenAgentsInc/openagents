# 47. Let the small model guess

[Previous: gpt-oss-120b](ch046-execute-gpt-oss-120b.md) · [Contents](SUMMARY.md) ·
[Next: tools](ch048-call-a-tool.md)

A draft model proposes tokens. The target keeps authority over the output
distribution. Because our two original models share the same tokenizer, a
20b token ID has the same byte meaning for 120b. The target and draft must
condition on exactly the same committed prefix and use explicitly defined
sampling distributions.

This chapter implements standard rejection-based speculative decoding in
Coax, a block verifier and complete-cache snapshots. It also adds prompt
n-gram drafts. The original-model harness measures a serial CPU implementation;
it does not claim the acceleration that a batched target backend can provide.

## A wrong draft still contributes useful work

Let p be the target distribution and q the draft distribution for the current
prefix. Draw x from q, then accept with probability:

```jai
acceptance(x) = min(1, p[x] / q[x])
```

A proposal with q[x]=0 is invalid. After a rejection, draw the replacement from
the normalized positive residual:

```jai
residual[i] = max(p[i] - q[i], 0)
residual[i] /= sum(residual)
```

The accepted mass at i is `min(p[i], q[i])`. Rejection contributes the positive
remainder, so the resulting mass is p[i]. Sampling directly from p after a
rejection would generally break that identity.

Start with binary-rational checks: p=[1/2,1/4,1/4], q=[1/4,1/2,1/4]. The
acceptance probabilities are [1,1/2,1], rejection mass is 1/4 and the residual
is [1,0,0]. Combining accepted and corrected mass gives p exactly. Separate
acceptance and correction uniforms avoid coupling the two decisions. Native
checks also sample 65,536 seeded trials against a fixed count bound of 1,024;
the algebraic check is the exact evidence, while the sample checks integration.

<!-- source: psionic/lib/gptoss/speculation.coax decide -->
```jai
gx_decide :: (target:[]f64,draft:[]f64,proposal:int,accept_uniform:f64,correct_uniform:f64,greedy:bool,residual:[]f64,stage:[]f64,overlap:bool)->GSpeculation #export_c {
    n:=target.count;if !gs_size(n) || proposal<0 || proposal>=n || !(accept_uniform>=0 && accept_uniform<1) || !(correct_uniform>=0 && correct_uniform<1) {return GSpeculation.{status=GStatus.shape};}
    if draft.count!=n || residual.count<n || stage.count<n {return GSpeculation.{status=GStatus.capacity};}if overlap {return GSpeculation.{status=GStatus.alias};}
    if gs_pick(target,0).status!=GStatus.ok || gs_pick(draft,0).status!=GStatus.ok {return GSpeculation.{status=GStatus.number};}
    if draft[proposal]==0 {return GSpeculation.{status=GStatus.token};}
    if greedy {id:=gx_argmax(target);return GSpeculation.{accepted=proposal==id,token=id,acceptance=ifx proposal==id then 1.0 else 0.0};}
    certain:=target[proposal]>=draft[proposal];acceptance:=1.0;if !certain {acceptance=target[proposal]/draft[proposal];}
    if accept_uniform<acceptance {return GSpeculation.{accepted=true,token=proposal,acceptance=acceptance,certain=certain};}
    sum:PsionicAccumulator;for i:0..<n {v:=target[i]-draft[i];stage[i]=gx_positive(v);sum=psionic_ops_accumulate(sum,stage[i]);}total:=psionic_ops_total(sum);
    for i:0..<n {stage[i]/=total;}pick:=gs_pick(stage[0..<n],correct_uniform);#assert pick.status==GStatus.ok;
    for i:0..<n {residual[i]=stage[i];}return GSpeculation.{token=pick.token,acceptance=acceptance,certain=certain};
}
```

Greedy verification is its own mode: accept a draft token only if it equals
the target's lowest-ID maximum; otherwise emit that maximum. It must reproduce
target-only greedy output token for token. Greedy mode does not pretend to be
a sample from a nonzero-temperature distribution.

## Verify a block, publish a committed prefix

For K proposals, the target supplies K conditional rows and one bonus row.
Each row must be conditioned on the draft prefix preceding it, including rows
that will become unreachable after rejection. Validate all rows and uniforms
before publication. The first rejection emits the accepted prefix plus one
correction. If all K match, emit an extra target bonus token. A bonus is
essential to the usual all-accepted algorithm.

`gx_block` supports 1–16 drafts and vocabulary up to 201,088. It keeps public
IDs unchanged on refusal. Proposal, conditional distributions and tokenizer
identity are caller obligations; this function cannot establish that a row was
computed from the supplied prefix. It is therefore paired with complete-model
qualification, not treated as sufficient model evidence on its own.

<!-- source: psionic/lib/gptoss/speculation.coax block -->
```jai
gx_block :: (vocab:int,target:[]f64,draft:[]f64,proposals:[]int,uniforms:[]f64,greedy:bool,out:[]int,stage:[]int,residual:[]f64,scratch:[]f64,overlap:bool)->GSpecBlock #export_c {
    k:=proposals.count;if !gs_size(vocab) || !gx_draft_count(k) {return GSpecBlock.{status=GStatus.shape};}
    if target.count!=(k+1)*vocab || draft.count!=k*vocab || uniforms.count!=2*k+1 || out.count<k+1 || stage.count<k+1 || residual.count<vocab || scratch.count<vocab {return GSpecBlock.{status=GStatus.capacity};}if overlap {return GSpecBlock.{status=GStatus.alias};}
    for row:1..<k+1 {if gs_pick(target[row*vocab..<(row+1)*vocab],0).status!=GStatus.ok {return GSpecBlock.{status=GStatus.number};}}
    for row:1..<k {if gs_pick(draft[row*vocab..<(row+1)*vocab],0).status!=GStatus.ok {return GSpecBlock.{status=GStatus.number};}}
    for u:uniforms {if !(u>=0 && u<1) {return GSpecBlock.{status=GStatus.number};}}
    accepted:=0;written:=k+1;
    for i:0..<k {decision:=gx_decide(target[i*vocab..<(i+1)*vocab],draft[i*vocab..<(i+1)*vocab],proposals[i],uniforms[2*i],uniforms[2*i+1],greedy,residual,scratch,false);
        if decision.status!=GStatus.ok {return GSpecBlock.{status=decision.status};}stage[i]=decision.token;
        if !decision.accepted {written=i+1;break;}accepted+=1;
    }
    bonus:=accepted==k;if bonus {p:=target[k*vocab..<(k+1)*vocab];stage[k]=ifx greedy then gx_argmax(p) else gs_pick(p,uniforms[2*k]).token;}
    for i:0..<written {out[i]=stage[i];}return GSpecBlock.{written=written,accepted=accepted,bonus=bonus};
}
```

Every random draw has an explicit input slot: two per proposal, then one bonus
uniform. The supplied model harness uses greedy rows; the tiny sampled fixtures
exercise rejection distributions. A service can generate those uniforms with
chapter 45's explicit seed/counter API and retain its counter after each block.

## Rolling back a ring requires its overwritten slots

A speculative future may wrap an even layer's cache. Restoring only an absolute
position would retain overwritten history and silently change subsequent
attention. `gx_snapshot` copies the complete selected cache capacity. Restore
both draft and target snapshots, then replay exactly the committed IDs. This
teaching path favors a directly checkable state transition over speed. It adds
model evaluations and memory traffic, both included in the measurement.

Model identity, capacity and sequential prefix must match the snapshot.
Snapshots include raw float bits, with no reinterpretation or cache reordering.
They are separate caller-owned buffers, bounded to 3,221,225,472 floats; an
allocation still has to succeed. Stage/cache lifetimes and observed disjoint
addresses remain part of the native contract.

`gx_ngram` looks for the longest suffix already present earlier in the prompt,
using the most recent occurrence on ties. Its next observed token becomes a
draft. No match yields -1 and the target proceeds alone. The draft is a point
mass for standard verification; it cannot acquire authority over the target.

[Run the small example](../examples/speculative-tokens.coax):

```shell
coax/bin/coax run psionic/examples/speculative-tokens.coax
```

<!-- source: psionic/examples/speculative-tokens.coax example -->
```jai
main :: () {
    target:=[9]f64.[1,0,0,0.25,0.25,0.5,0,0,1];draft:=[6]f64.[1,0,0,0,1,0];proposal:=[2]int.[0,1];uniforms:=[5]f64.[0,0,0.8,0.5,0.1];
    emitted:[3]int;stage:[3]int;residual:[3]f64;scratch:[3]f64;
    result:=gx_block(3,target[..],draft[..],proposal[..],uniforms[..],false,emitted[..],stage[..],residual[..],scratch[..],false);
    #assert result.status==GStatus.ok && !result.bonus;
    print("draft: 0 1; accepted prefix: %\n",result.accepted);
    print("committed: % %; second token drawn from the positive residual\n",emitted[0],emitted[1]);
    history:=[8]int.[1,2,3,1,2,4,1,2];print("prompt n-gram draft: %\n",gx_ngram(history[..],2));
    live:=[4]f32.[1,2,3,4];saved:[4]f32;#assert gx_snapshot(live[..],saved[..],false).status==GStatus.ok;
    live[0]=99;#assert gx_snapshot(saved[..],live[..],false).status==GStatus.ok && live[0]==1;
    print("rollback: the complete cache, including overwritten ring slots\n");
}
```

Expected output:

```jai
draft: 0 1; accepted prefix: 1
committed: 0 2; second token drawn from the positive residual
prompt n-gram draft: 4
rollback: the complete cache, including overwritten ring slots
```

## Measure acceptance and the cost of verification

The opt-in original harness authenticates both complete checkpoints, prefills
one common Harmony request, then retains prefix snapshots. It compares target
alone with 20b draft limits 1, 2 and 4, and prompt n-grams, for the same six
committed greedy IDs. Every committed ID must equal the retained target-only
answer. Timings exclude shared authentication/prefill and include verification,
rollback and replay. Acceptance counts and actual proposals are reported beside
latency, requested draft limit and target batch size.

```shell
psionic/tests/check_gptoss_original.sh speculation
```

[The retained measurements](../../docs/verification/psionic-ch47-2026-10-29.md)
state the outcome rather than assuming a speedup. The target batch is one in
this scalar implementation: there is no batched evaluation saving to offset
the draft and replay costs. A batched target backend must be measured separately
before claiming a useful crossover. Larger K alone does not create that backend.
GPU speculation, multi-request batching and snapshot compression remain open.
