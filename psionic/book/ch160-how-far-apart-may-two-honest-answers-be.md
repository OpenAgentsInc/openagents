# 160. How far apart may two honest answers be?

A Pylon that answers decisions for pay can be checked by asking another
machine the same thing. Two honest machines never answer to the last bit: a
GPU sums in a different order from a CPU, and half precision rounds more
than single. A cheap substitute differs as well: a quantized copy of the
weights, another runtime, or a sibling model. A checker has to tell these two
kinds of difference apart, and it has to do so without an answer key.

This chapter measures that boundary on a simulated world:
- **a distance** between two answers in centred log space;
- **honest pairs and cheap substitutes**, each measured against a reference;
- **limits** set between the two from simulated rounds;
- **a live round** on fresh items, with the checked subset drawn only after
  the answers arrive.

```shell
coax/bin/coax run psionic/examples/honest-distance.coax
coax/bin/coax test psionic/lib/distance.coax
```

```jai
400 fresh items, no answer key; each backend against the reference:
  CPU: median 0.0002, p90 0.0003, p99 0.0005, max 0.0005; top answer agrees on 400 of 400
  GPU, f16: median 0.004, p90 0.0072, p99 0.0097, max 0.0108; top answer agrees on 400 of 400
  quantized copy: median 0.1454, p90 0.245, p99 0.3092, max 0.3303; top answer agrees on 382 of 400
  another runtime: median 0.0017, p90 0.3684, p99 0.4703, max 0.5005; top answer agrees on 387 of 400
  sibling model: median 0.7553, p90 1.3257, p99 1.7687, max 2.0316; top answer agrees on 316 of 400
worst honest round 0.0073, worst honest item 0.0108: a round is lost above 0.0145, struck above 0.0436 or with one item above 0.0217
  CPU: of 20,000 rounds 20000 passed, 0 lost, 0 struck
  GPU, f16: of 20,000 rounds 20000 passed, 0 lost, 0 struck
  quantized copy: of 20,000 rounds 0 passed, 0 lost, 20000 struck
  another runtime: of 20,000 rounds 111 passed, 0 lost, 19889 struck
  sibling model: of 20,000 rounds 0 passed, 0 lost, 20000 struck
the limits separate the pairs: true
  a live round with the GPU, f16: items 7, 1 and 3 checked; mean 0.0055, worst 0.0081 -> passed
  a live round with the quantized copy: items 7, 0 and 2 checked; mean 0.2061, worst 0.2738 -> struck
```

The [example](../examples/honest-distance.coax) runs in about a second.

## Fresh items, many backends

<!-- source: psionic/lib/distance.coax items -->
```jai
// answer-sim-v1. Item i draws its kind, its options (2 to 8 for a choice, 2
// for yes or no, 3 to 5 for a score) and a favourite option; the reference's
// logit for each option is uniform in [-2, 2], plus 2 for the favourite.
dd_items :: (seed:u64,out:[]DdItem)->DdStatus #export_c {
    if out.count<1 || out.count>DD_ITEMS_MAX {return DdStatus.shape;}
    for i:0..<out.count {
        b:=cast(u64)(4*i);kind:=cast(int)(3*tb_uniform(seed,b));r:=tb_uniform(seed,b+1);
        options:=ifx kind==0 then 2+cast(int)(7*r) else ifx kind==1 then 2 else 3+cast(int)(3*r);
        out[i]=DdItem.{kind=cast(u8)kind,options=options,favourite=cast(int)(f64(options)*tb_uniform(seed,b+2))};
    }
    return DdStatus.ok;
}
dd_backend_ok :: (b:DdBackend)->bool {
    return b.mix>=0 && b.mix<=1 && b.grid>=0 && b.grid<=DD_PARAM_MAX && b.noise>=0 && b.noise<=DD_PARAM_MAX && b.share>=0
        && b.share<=1 && b.shift>=0 && b.shift<=DD_PARAM_MAX;
}
dd_items_ok :: (items:[]DdItem)->bool {
    for x:items {if x.options<2 || x.options>DD_OPTIONS || x.favourite<0 || x.favourite>=x.options {return false;}}
    return true;
}
// A backend's answers, DD_OPTIONS slots to an item; slots past an item's
// options are left as they were.
dd_answer :: (seed:u64,items:[]DdItem,b:DdBackend,out:[]f64)->DdStatus #export_c {
    if items.count<1 || items.count>DD_ITEMS_MAX || !dd_items_ok(items) || !dd_backend_ok(b) {return DdStatus.shape;}
    if out.count<DD_OPTIONS*items.count {return DdStatus.capacity;}
    for item,i:items {
        shifted:=cast(int)(tb_uniform(b.seed,cast(u64)(16384+i))+b.share)>0;
        w:[DD_OPTIONS]f64;
        for j:0..<item.options {
            z:=4*tb_uniform(seed,cast(u64)(4096+8*i+j))-2+(ifx j==item.favourite then 2.0 else 0.0);
            y:=b.mix*z+(1-b.mix)*(4*tb_uniform(b.seed,cast(u64)(8*i+j))-2);
            if b.grid>0 {y=f64(cast(int)(y/b.grid))*b.grid;}
            y+=b.noise*(tb_uniform(b.seed,cast(u64)(8192+8*i+j))-tb_uniform(b.seed,cast(u64)(12288+8*i+j)));
            if shifted {y+=b.shift*(2*tb_uniform(b.seed,cast(u64)(24576+8*i+j))-1);}
            w[j]=y;
        }
        // Relative to the first logit, which is within about 100 of the rest.
        t:=0.0;first:=w[0];for j:0..<item.options {w[j]=exp(w[j]-first);t+=w[j];}
        for j:0..<item.options {out[i*DD_OPTIONS+j]=w[j]/t;}
    }
    return DdStatus.ok;
}
```

Every item is drawn from a seed, so a checker can make new ones for each
round and nobody can have practised on them. There is no answer key. The
question is only whether two machines that claim to run the same model
answer alike.

The simulated backends differ from the reference in known ways:

| Backend | How it differs |
| --- | --- |
| CPU | jitter of less than 0.0005 a logit |
| GPU, f16 | jitter of less than 0.01 a logit |
| quantized copy | logits truncated toward zero to a grid of 0.25 |
| another runtime | on half the items, a shift of up to 0.4 a logit, as when its prompt differs |
| sibling model | 40% of its logits its own |

The first two are honest; the last three are substitutes.

## The distance

<!-- source: psionic/lib/distance.coax distance -->
```jai
// The distance: subtract each answer's mean log probability, then take the
// largest difference between the two, and the first option where it is.
// Probabilities below 10^-6 count as 10^-6, and the result says how many did.
dd_distance :: (p:[]f64,q:[]f64)->DdDistance {
    r:=DdDistance.{};a:[DD_OPTIONS]f64;b:[DD_OPTIONS]f64;sa:=0.0;sb:=0.0;
    for j:0..<p.count {
        x:=p[j];y:=q[j];
        if x<DD_FLOOR {x=DD_FLOOR;r.floored+=1;}
        if y<DD_FLOOR {y=DD_FLOOR;r.floored+=1;}
        a[j]=log(x);b[j]=log(y);sa+=a[j];sb+=b[j];
    }
    n:=f64(p.count);
    for j:0..<p.count {e:=abs((a[j]-sa/n)-(b[j]-sb/n));if j==0 || e>r.d {r.d=e;r.option=j;}}
    return r;
}
```

Subtracting the mean log probability removes what doesn't matter: a
softmax gives the same answer for logits shifted by a constant. What is left
is the shape of the answer. The largest difference over the options is in
log units, so 0.01 means some option's odds moved by about 1%. The same
distance is used by REAX's validators and was measured by the pre-reset
Psionic ([notes](../docs/decisions.md)).

## Comparing two backends

<!-- source: psionic/lib/distance.coax compare -->
```jai
// Two backends' answers to the same items: the distance for each item, how
// many top answers agree, and how many probabilities were floored.
dd_compare :: (items:[]DdItem,a:[]f64,b:[]f64,out:[]f64)->DdCompare #export_c {
    n:=items.count;
    if n<1 || n>DD_ITEMS_MAX || !dd_items_ok(items) {return DdCompare.{status=DdStatus.shape};}
    if a.count<DD_OPTIONS*n || b.count<DD_OPTIONS*n || out.count<n {return DdCompare.{status=DdStatus.capacity};}
    status:=dd_answers_ok(items,a);if status==DdStatus.ok {status=dd_answers_ok(items,b);}
    if status!=DdStatus.ok {return DdCompare.{status=status};}
    r:=DdCompare.{};
    for item,i:items {
        p:=a[i*DD_OPTIONS..<i*DD_OPTIONS+item.options];q:=b[i*DD_OPTIONS..<i*DD_OPTIONS+item.options];
        d:=dd_distance(p,q);out[i]=d.d;r.floored+=d.floored;
        if dd_top(p)==dd_top(q) {r.agree+=1;}
    }
    return r;
}
// The pct percentile by nearest rank, k = ceil(pct n / 100) in integers:
// the value x with fewer than k distances below it and at least k at or
// below it.
dd_rank_is :: (d:[]f64,x:f64,k:int)->bool {
    below:=0;atmost:=0;
    for y:d {if y<x {below+=1;}if y<=x {atmost+=1;}}
    return below<k && atmost>=k;
}
dd_quantile :: (d:[]f64,pct:int)->f64 {
    k:=(pct*d.count+99)/100;i:=0;
    while !dd_rank_is(d,d[i],k) {i+=1;}
    return d[i];
}
dd_summary :: (d:[]f64)->DdSummary #export_c {
    if d.count<1 || d.count>DD_ITEMS_MAX {return DdSummary.{status=DdStatus.shape};}
    for x:d {if !(x>=0 && x<=DD_BIG) {return DdSummary.{status=DdStatus.number};}}
    return DdSummary.{median=dd_quantile(d,50),p90=dd_quantile(d,90),p99=dd_quantile(d,99),max=dd_quantile(d,100)};
}
```

The honest pairs stay small: the GPU's worst item is 0.0108. The top answer
is a poor test. The quantized copy and the other runtime agree with the
reference's top answer on 382 and 387 of 400 items. A checker that compared
only top answers would pass both.

The other runtime's median is 0.0017, smaller than the honest GPU's: half its
items match all but exactly. Its p90 is 0.37. A limit set on medians would miss it,
so the limits come from rounds, as a checker sees them.

## Rounds and their limits

<!-- source: psionic/lib/distance.coax round -->
```jai
// A round's verdict from its checked distances: a strike when the mean
// passes the strike limit or any one item passes the item limit; lost when
// the mean passes the lost limit; otherwise passed.
dd_check :: (d:[]f64,limits:DdLimits)->DdCheck {
    r:=DdCheck.{worst=dd_quantile(d,100)};t:=0.0;
    for x:d {t+=x;}
    r.mean=t/f64(d.count);
    r.verdict=ifx r.mean>limits.strike || r.worst>limits.item then DdVerdict.strike
        else ifx r.mean>limits.lost then DdVerdict.lost else DdVerdict.pass;
    return r;
}
// v distinct positions of n, by a partial shuffle on counters base, base+1, ...
dd_pick :: (seed:u64,n:int,v:int,base:int,out:[]int) {
    idx:[DD_ITEMS_MAX]int;for i:1..<n {idx[i]=i;}   // idx[0] is 0 already
    for k:0..<v {
        j:=k+cast(int)(f64(n-k)*tb_uniform(seed,cast(u64)(base+k)));
        x:=idx[k];idx[k]=idx[j];idx[j]=x;out[k]=idx[k];
    }
}
dd_rounds_ok :: (rounds:int)->bool {return rounds>=1 && rounds<=DD_ROUNDS_MAX;}
// Many rounds drawn from measured distances: each checks v items at random.
dd_simulate :: (d:[]f64,limits:DdLimits,seed:u64,rounds:int,v:int)->DdRounds #export_c {
    if d.count<1 || d.count>DD_ITEMS_MAX || v<1 || v>d.count || v>DD_VERIFIED_MAX || !dd_rounds_ok(rounds) {
        return DdRounds.{status=DdStatus.shape};
    }
    for x:d {if !(x>=0 && x<=DD_BIG) {return DdRounds.{status=DdStatus.number};}}
    r:=DdRounds.{};at:[DD_VERIFIED_MAX]int;ds:[DD_VERIFIED_MAX]f64;
    for k:0..<rounds {
        dd_pick(seed,d.count,v,k*v,at[..]);
        for i:0..<v {ds[i]=d[at[i]];}
        c:=dd_check(ds[0..<v],limits);
        if c.verdict==DdVerdict.pass {r.pass+=1;}
        if c.verdict==DdVerdict.lost {r.lost+=1;}
        if c.verdict==DdVerdict.strike {r.strike+=1;}
        if k==0 || c.mean>r.worst {r.worst=c.mean;r.worst_at=k;}
    }
    return r;
}
```

<!-- source: psionic/lib/distance.coax limits -->
```jai
// Limits between the two kinds of pair: a round is lost above twice the
// worst honest round, struck above three times that, and struck when one
// item is above twice the worst honest item. They stand only when every
// honest pair passes every round and each substitute passes at most 5 in 100.
dd_limits :: (honest_round:f64,honest_item:f64)->DdLimits {
    return DdLimits.{lost=2*honest_round,strike=6*honest_round,item=2*honest_item};
}
dd_gap :: (honest:[]DdRounds,substitutes:[]DdRounds,rounds:int)->bool {
    for h:honest {if h.pass!=rounds {return false;}}
    for s:substitutes {if 100*s.pass>5*rounds {return false;}}
    return true;
}
```

A round checks 8 items. The limits come from the honest pairs alone:
- the worst of 20,000 simulated honest rounds has a mean of 0.0073, so a
  round is lost above twice that, 0.0145;
- a round is struck above three times the lost limit, 0.0436;
- the worst honest item is 0.0108, so a round is also struck when one item
  is above 0.0217.

The rule is written before the substitutes are scored. They are then scored
against it:

| Pair | Passed | Lost | Struck |
| --- | ---: | ---: | ---: |
| CPU | 20,000 | 0 | 0 |
| GPU, f16 | 20,000 | 0 | 0 |
| quantized copy | 0 | 0 | 20,000 |
| another runtime | 111 | 0 | 19,889 |
| sibling model | 0 | 0 | 20,000 |

The limits stand because every honest round passes and no substitute passes
more than 5 rounds in 100. If either failed, there would be no limit to
publish: honest machines would be struck, or substitutes paid.

The other runtime passes 0.56% of rounds: those where all 8 checked items
happen to be ones its prompt didn't change. More checked items make that
rarer.

## A live round

<!-- source: psionic/lib/distance.coax live -->
```jai
// One checked round. The checker draws which items it checks from the digest
// of the answers it received and its own secret, so the subset doesn't exist
// until the answers do.
dd_round :: (items:[]DdItem,reference:[]f64,answers:[]f64,secret:u64,checked:int,limits:DdLimits,out:[]int)->DdRound #export_c {
    n:=items.count;
    if n<1 || n>SU_ITEMS_MAX || checked<1 || checked>n || !dd_items_ok(items) {return DdRound.{status=DdStatus.shape};}
    if reference.count<DD_OPTIONS*n || answers.count<DD_OPTIONS*n || out.count<checked {return DdRound.{status=DdStatus.capacity};}
    status:=dd_answers_ok(items,reference);if status==DdStatus.ok {status=dd_answers_ok(items,answers);}
    if status!=DdStatus.ok {return DdRound.{status=status};}
    digest:=su_values(answers[0..<DD_OPTIONS*n]);seed:=cast(u64)digest[0];
    for k:1..<8 {seed|=cast(u64)digest[k]<<(8*cast(u64)k);}
    r:=DdRound.{seed=seed^secret};
    dd_pick(r.seed,n,checked,0,out);
    ds:[DD_VERIFIED_MAX]f64;
    for k:0..<checked {
        item:=items[out[k]];o:=out[k]*DD_OPTIONS;
        ds[k]=dd_distance(reference[o..<o+item.options],answers[o..<o+item.options]).d;
    }
    c:=dd_check(ds[0..<checked],limits);
    r.verdict=c.verdict;r.mean=c.mean;r.worst=c.worst;
    return r;
}
```

If the answerer knew which items would be checked, it could answer those
honestly and the rest cheaply. So the checker draws the subset from a digest
of the answers it received, using
[chapter 178](ch178-a-suite-is-three-digests.md)'s values encoding, mixed
with a secret of its own. The subset doesn't exist until the answers do.
Changing any answer changes the subset.

In the example, the GPU's round checks items 7, 1 and 3, and passes with a
mean of 0.0055. The quantized copy's round checks items 7, 0 and 2. Its mean
of 0.2061 and its worst item of 0.2738 are both past the strike limits, so the
round is struck.

## Supported profile

`cpu-f64-distance-v1`:
- binary64, plain sums in a fixed order, natural logarithms;
- probabilities floored at 10⁻⁶;
- percentiles by nearest rank, computed in integers;
- answer-sim-v1 on chapter 70's SplitMix64 counters, with backend settings
  up to 10;
- up to 1,024 items of 2 to 8 options, 64 checked items, 100,000 simulated
  rounds;
- live rounds of up to 64 items.

## How it is checked

- **Coax checks:** 110 checks in debug and release, counting the units
  it loads. They cover:
  - items and answers against the oracle, and every backend and item bound;
  - the distance by hand: ties between options, the first option named, and
    the floor at exactly 10⁻⁶;
  - comparisons against the oracle, a flipped top answer, a floored
    probability, and every refusal;
  - percentiles with repeated values, and summaries;
  - a round's verdict at each limit;
  - simulated rounds against the oracle, including ties for the worst round;
  - the limits and the gap rule at 5 in 100;
  - a live round against the oracle, and how its subset changes with the
    answers and the secret.
- **An independent oracle,** [distance_oracle.py](../tests/distance_oracle.py),
  writes the items, backends, distance, percentiles, rounds, limits and live
  round again. The example's output equals its figures.
- **Mutation testing** of `distance.coax`: the first run killed 2,372 of
  2,395 mutants. The fixes below followed; the rerun's result is in the
  verification record.
  Writing it for mutation changed several things:
  - **The distance names its option.** A running maximum whose `>=` variant
    stores the same value can't be told apart; naming the option can.
  - **Percentiles are found by rank, not by sorting.** Sorting equal values
    either way gives the same list.
  - **A round's worst item comes from the percentile function,** for the
    same reason.
  - **The softmax subtracts the first logit,** not the largest, with backend
    settings capped so the exponent can't overflow.
  - **The world has no shifts a softmax can't see.** Rounding to the nearest
    grid point and jitter of the form 2u − 1 could each be changed into a
    constant shift of every logit, which leaves the answer unchanged. The
    quantized copy now truncates toward zero, and jitter is the difference of
    two uniforms.
  - **Bounds are tested at both edges:** one item and 1,024, a hundred
    distinct distances for the 99th percentile, and an output exactly as long
    as the checked subset.

There is no native harness yet. The
[verification record](../../docs/verification/psionic-ch160-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **A simulated world.** The distances of real backends come once chapters
  19 and 20 serve the same model on more than one machine. The pre-reset
  figures (CPU against Metal at most 0.0035, a Q8_0 copy passing 3.6% of
  rounds) are the targets to reproduce.
- **No self-check.** A Pylon checking itself against shipped reference
  decisions before it serves is chapter 159.
- **No penalties.** What a lost round or a strike costs the answerer belongs
  with payment, not here.
- **One reference.** Here the checker's own answers are taken as right. A
  checker on a drifting machine would strike honest answerers; chapter 159's
  self-check is what guards the checker.
