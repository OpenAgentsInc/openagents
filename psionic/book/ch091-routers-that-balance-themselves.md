# 91. Routers that balance themselves

DeepSeek’s correction bias changes which experts receive work. It does not
increase their contribution to the answer. Keeping selection scores and
combination weights in separate arrays makes that rule visible and testable.
This chapter implements `deepseek-router-cpu-wide-v1`, including V2-Lite’s
softmax router, V3’s grouped sigmoid router, and V4.1’s square-root-softplus
router. It precedes chapter 90’s complete decoder because routing is one of
that decoder’s prerequisites.

The contract comes from the pinned original
[V2-Lite configuration](https://huggingface.co/deepseek-ai/DeepSeek-V2-Lite-Chat/blob/85864749cd611b4353ce1decdb286193298f64c7/config.json),
[V3 gate](https://github.com/deepseek-ai/DeepSeek-V3/blob/9b4e9788e4a3a731f7567338ed15d3ec549ce03b/inference/model.py),
and [V4.1 reference gate](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/model.py).
[Source identities and configuration values](../fixtures/deepseek/router-contracts.json)
are retained as data. Upstream code is a read-only reference, never a runtime
dependency. The correction vector is an input: this chapter implements its
inference meaning, not an online training algorithm to update it.

## Separate the three profiles

| Profile | Original experts / selected | Selection | Combination weights |
| --- | --- | --- | --- |
| V2-Lite | 64 / 6 | Full-expert softmax, no bias | Selected original probabilities × scale 1; discarded mass stays discarded |
| V3 | 256 / 8 | Sigmoid plus correction bias; four of eight groups | Selected unbiased sigmoid scores normalized, then × 2.5 |
| V4.1 Flash | 384 / 6 | Square root of softplus plus correction bias | For k > 1, selected unbiased scores divided by their sum plus 1e-20, then × 1.5 |

V3 scores a group by the sum of its two largest biased expert scores. V2’s
optional grouped form uses the largest probability in each group. V4.1’s
text gate chooses globally; its output-projection `o_groups` is unrelated to
routing groups. We refuse grouped V4.1 routing. Image-specific biases are
outside this text profile.

At every tie, the lower group or expert ID wins. The upstream device’s tie
order is not specified by its top-k primitive; matching an original device
capture is a separate qualification gate. We do not disguise a tie-order
difference as an arithmetic match.

All three profiles accept F32 logits/biases and evaluate scores, group sums,
normalization and weights in binary64. The implementation uses a stabilized
full-expert softmax and sigmoid, softplus’s identity branch strictly above 20,
and compensated `log(1+x)` for small probabilities. It preserves the reference
V4.1 epsilon and its unnormalized k=1 case. Binary64 `exp` underflow is part
of this profile: an all-zero V3 selected sum refuses atomically; V4.1’s
epsilon produces zero weights. This is a CPU equation reference, not upstream
F32/BF16 arithmetic, original model execution or device throughput parity.

## Run the router

```shell
coax/bin/coax run psionic/examples/deepseek-router.coax
coax/bin/coax run psionic/examples/deepseek-router.coax --release
psionic/tests/check_deepseek_router.sh
psionic/check.sh --only deepseek/router --jobs 8
```

The [example](../examples/deepseek-router.coax) makes a low-probability expert
win through a large correction bias, then changes that bias while keeping
the selected set fixed. Its weights remain identical. It also shows a V4.1
tie: IDs 0 and 1 receive 0.75 each, with a zero cutoff margin.

<!-- source: psionic/examples/deepseek-router.coax example -->
```jai
main :: () {
    x:=[4]f32.[-2,0,2,4];bias:=[4]f32.[100,0,0,0];scores:[4]f64;ranked:[4]f64;
    c:=DrConfig.{profile=DrProfile.v3,k=2,scale=2.5};
    a:=dr_route(c,x[..],bias[..],scores[..],ranked[..],false);#assert a.status==DrStatus.ok;
    print("V3 selected %, %; correction bias never enters the weights\n",a.ids[0],a.ids[1]);
    bias[0]=200;b:=dr_route(c,x[..],bias[..],scores[..],ranked[..],false);comparison:=dr_compare(a,b);
    #assert comparison.identical_ids && comparison.weight_l1==0;
    print("bias changed, same experts: weight difference %\n",comparison.weight_l1);
    c.profile=DrProfile.v41;c.scale=1.5;for i:0..<4 {x[i]=0;bias[i]=0;}
    a=dr_route(c,x[..],bias[..],scores[..],ranked[..],false);#assert a.status==DrStatus.ok;
    print("V4.1 tied experts %, %; weight each %; cutoff margin %\n",a.ids[0],a.ids[1],a.weights[0],a.margin);
    x[1]=0.000001f32;b=dr_route(c,x[..],bias[..],scores[..],ranked[..],false);comparison=dr_compare(a,b);
    print("comparisons retain both expert and group margin records\n");
    #assert comparison.reference.margin==a.margin && comparison.candidate.margin==b.margin;
}
```

## Retain the runner as well as the winners

`dr_route` returns ordered selected IDs and their weights. It also returns
the weakest selected ranking score, the strongest eligible excluded expert,
and their difference. A zero margin means an exact cutoff tie; a small margin
exposes a selection sensitive to small arithmetic changes. Group selection
gets its own excluded group and margin. When every eligible expert or group
is selected, the corresponding `has_margin` flag is false and the excluded ID
is -1; zero then does not claim a tie.

The public score diagnostics retain every unbiased and biased expert score.
Group masking affects selection only, so diagnostics and weights remain
independently inspectable. Expert margins are conditional on the selected
groups; the group margin exposes a different source of instability.

The [router](../lib/deepseek/router.coax) stages its calculations in bounded
local arrays. It validates dimensions, capacities, overlap, finite inputs
and a finite positive scale at most 65,536 before publishing either diagnostic
array. A failure returns no successful route and preserves both public arrays.
Unused public tails stay unchanged. There is no heap allocation.

Bounds are 1–384 experts, 1–8 selected experts, 1–8 equal-sized groups, and
enough retained groups to supply k distinct experts. V3 groups with more than
one group require at least two experts each. V2 accepts no correction vector;
V3/V4.1 require one entry per expert. The caller provides valid typed values,
truthful extents, immutable inputs, exclusive disjoint writable spans, and
matching model/history identity. The native
[adapter](../platform/deepseek-router-memory.c) observes address overlap only.

## A comparison carries both margins

Comparing only selected IDs hides how close a disagreement was. Comparing
only weights hides a changed route. `dr_compare` retains both complete route
records, reports ordered-ID equality and the number of replaced experts, and
computes L1 weight difference after matching weights by expert ID. A reordered
set may have identical weights by identity while differing in order.

Its inputs are unmodified successful reports for the same model, request and
configuration. It checks statuses, expert/profile/count agreement and bounded
counts before indexing. It does not authenticate model identity or turn a
comparison into an admission verdict.

<!-- source: psionic/lib/deepseek/router-compare.coax compare -->
```jai
dr_compare :: (a:DrRoute,b:DrRoute)->DrComparison #export_c {
    if a.status!=DrStatus.ok || b.status!=DrStatus.ok || a.experts!=b.experts || a.count!=b.count || a.profile!=b.profile || a.count<1 || a.count>8 {return DrComparison.{status=DrStatus.shape};}
    r:=DrComparison.{reference=a,candidate=b,identical_ids=true};
    for i:0..<a.count {if a.ids[i]!=b.ids[i] {r.identical_ids=false;}
        match:=-1;for j:0..<b.count {if a.ids[i]==b.ids[j] {match=j;}}
        if match==-1 {r.changed+=1;r.weight_l1+=a.weights[i];}else {r.weight_l1+=abs(a.weights[i]-b.weights[match]);}
    }
    for j:0..<b.count {found:=false;for i:0..<a.count {if b.ids[j]==a.ids[i] {found=true;}}if !found {r.weight_l1+=b.weights[j];}}
    return r;
}
```

The independent [oracle](../tests/deepseek_router_oracle.py) uses 220-digit
Decimal arithmetic and full sorting, rather than the Coax repeated-selection
algorithm. Thirteen cases include all original expert dimensions, a
one-F32-step cutoff, group ties, a case where maximum and top-two group sums
choose different groups, tiny probabilities exposing the epsilon, the
softplus threshold, extreme finite logits, and both k=1 rules.

Weights and score components use a relative 1e-12 bound plus 1e-300. Margin
error bounds propagate those component errors through subtraction; applying
a relative bound to a nearly zero difference would ignore cancellation.
Selected and excluded IDs must match exactly. These are component numerical
checks; they are not a replacement for a frozen whole-model qualification
policy.

Native checks run the same independent vectors in debug/release and plain/
ASan/UBSan builds. They print both expert and group margins for each comparison,
and check NaN/Inf at the first/final input entries, all public tails, capacity,
aliased buffers, and atomic all-underflow refusal. Mutation of the router and
comparison root has zero survivors. The
[verification record](../../docs/verification/psionic-ch59-2026-10-29.md)
binds the checked sources and counts.

Chapter 58 uses the V2 profile while adding latent attention and complete
expert layers. Later V4.1 chapters reuse the selection/weight distinction;
expert residency never gets permission to change the chosen route.
