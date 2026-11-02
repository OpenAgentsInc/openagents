# 102. Verify drafts by expected gain

> **Status: partly implemented.** The rule that picks each block's draft
> length from expected tokens per unit of time runs, with its cost line and
> acceptance model fitted from measurements. It is checked on the CPU on
> chapter 87's synthetic draft head and measured on the original gpt-oss-20b
> on one RTX 4080 with chapter 75's prompt n-gram drafter. There, the rule
> chooses no speculation. DeepSeek V4.1's own draft layers, a trained
> confidence head and chapter 99's late speculative-length path are not
> built; see [What is not built yet](#what-is-not-built-yet).

[Chapter 76](ch076-verify-a-draft-block-in-parallel.md) verified a fixed
block of four drafts on the RTX 4080 and found it 1.84 times slower than the
target alone. A fixed length is a guess made once for the whole generation.
This chapter makes the length a decision made at every step: verify the
number of drafts k, from 0 to the number drafted, that gives the most
expected committed tokens per unit of time. k = 0 means no speculation for
that step, so the rule can never choose to be slower than its own cost model
says the target alone is.

The rule needs two inputs: the probability that each draft is accepted, and
the time to verify k drafts. Both come from measurements. The decision is an
`#authority` file, [`draft-length.coax`](../lib/draft-length.coax), because it
decides how the engine spends its time. The CPU harness on chapter 87's head
is [`draft-length-run.coax`](../lib/draft-length-run.coax), and the RTX 4080
harness is [`gptoss_draft_length.c`](../tests/gptoss_draft_length.c), run by
[`check_draft_length.sh`](../tests/check_draft_length.sh).

```shell
coax/bin/coax run psionic/examples/draft-length.coax
coax/bin/coax run psionic/examples/draft-length-refuse.coax
coax/bin/coax test psionic/lib/draft-length.coax
coax/bin/coax mutate psionic/lib/draft-length.coax --root-only --jobs 8
```

## Expected tokens, by hand

A block of k drafts commits the accepted prefix plus one more token: the
target's correction at the first mismatch, or the bonus token when all k
match. So a block always commits at least one token. Let a[i] be the
probability that draft i is accepted, given that drafts 0 to i-1 were. Draft
i survives with probability a[0]·a[1]·…·a[i], and the expected committed
tokens are

```text
E(k) = 1 + a[0] + a[0]a[1] + ... + a[0]a[1]...a[k-1]
E(0) = 1                       (a plain decode step)
```

Take a = 0.75, 0.5, 0.5, 0.25, a decode step that costs 10 time units, and a
block that costs 12 + 4k:

```text
k   E(k)                                  c(k)   E(k) / c(k)
0   1                                     10     0.1
1   1 + 0.75 = 1.75                       16     0.109375
2   1.75 + 0.75·0.5 = 2.125               20     0.10625
3   2.125 + 0.375·0.5 = 2.3125            24     0.0963541...
4   2.3125 + 0.1875·0.25 = 2.359375       28     0.0842633...
```

k = 1 has the highest rate, so this step verifies one draft. Longer blocks
add expected tokens, but each extra draft adds less than the 4 units it
costs.

One bound follows directly. E(k) is at most k + 1, so when every block costs
at least k + 1 decode steps, no acceptance can make drafting pay, and the
rule always picks k = 0. `dl_never` reports that case.

<!-- source: psionic/lib/draft-length.coax types -->
```jai
DL_MAX :: 16;
DlStatus :: enum u8 {ok;shape;probability;cost;count;}
// The chosen length, its expected tokens and cost, and their ratio.
DlChoice :: struct {status:DlStatus;k:int;expected:f64;cost:f64;rate:f64;}
// A cost line fitted to measured block times: c(k) = base + slope * k.
DlLine :: struct {status:DlStatus;base:f64;slope:f64;}
```

<!-- source: psionic/lib/draft-length.coax expected -->
```jai
// E(k): one token always (correction or bonus), plus the chance that each
// of the first k drafts survives with all those before it.
dl_expected :: (accept:[]f64,k:int)->f64 #export_c {
    total:=1.0;alive:=1.0;
    for i:0..<k {alive*=accept[i];total+=alive;}
    return total;
}
// Chapter 75's sampled acceptance for one position whose target and draft
// distributions do not depend on earlier drafts: the overlap sum min(p, q).
dl_overlap :: (target:[]f64,draft:[]f64)->f64 #export_c {
    s:=0.0;for i:0..<target.count {s+=(target[i]+draft[i]-abs(target[i]-draft[i]))/2;}return s;
}
```

`dl_overlap` gives a[i] for sampled verification under chapter 75's rule:
a draft x drawn from q is kept with probability min(1, p(x)/q(x)), so the
chance of keeping it is the sum over x of q(x)·min(1, p(x)/q(x)), which is
the sum of min(p(x), q(x)). (p + q − |p − q|) / 2 is that minimum.

## The rule

<!-- source: psionic/lib/draft-length.coax choose -->
```jai
// accept[i] for drafts 0..n-1 and cost[k] for k = 0..n: the k in 0..n with
// the highest E(k) / c(k). Only a strictly higher rate moves the choice, so
// ties go to the shorter block and k = 0 is kept unless drafting gains.
dl_choose :: (accept:[]f64,cost:[]f64)->DlChoice #export_c {
    n:=accept.count;
    if n>DL_MAX || cost.count!=n+1 {return DlChoice.{status=DlStatus.shape};}
    for a:accept {if !dl_probability(a) {return DlChoice.{status=DlStatus.probability};}}
    for c:cost {if !dl_time(c) {return DlChoice.{status=DlStatus.cost};}}
    best:=DlChoice.{expected=1,cost=cost[0],rate=1/cost[0]};
    for k:1..<n+1 {
        e:=dl_expected(accept,k);r:=e/cost[k];
        if r>best.rate {best=DlChoice.{k=k,expected=e,cost=cost[k],rate=r};}
    }
    return best;
}
// When every block costs at least as much as k + 1 decode steps, no
// acceptance can pay: E(k) <= k + 1, so E(k) / c(k) <= 1 / c(0).
dl_never :: (cost:[]f64)->bool #export_c {
    for k:1..<cost.count {if cost[k]<cast(f64)(k+1)*cost[0] {return false;}}return true;
}
```

The engine calls `dl_choose` once per step, after drafting and before
verifying, with as many acceptance estimates as there are drafts. A drafter
that finds nothing passes zero estimates, and the rule returns k = 0.

## Where the inputs come from

The cost table is measured. A calibration pass times every plain decode
step and every block, grouped by its draft count. A least-squares line
through the block means gives c(k) for k ≥ 1, and the mean plain step gives
c(0). The line is fitted rather than read off the table so that one noisy
length does not make the rule jump.

The acceptance model counts. In the calibration pass at k = 4, `reached[i]`
counts blocks that proposed draft i after drafts 0 to i-1 were all accepted,
and `kept[i]` those that kept it. Laplace's rule, (kept + 1) / (reached + 2),
turns the counts into probabilities, so a position seen rarely is pulled
toward one half rather than trusted at 0 or 1.

<!-- source: psionic/lib/draft-length.coax fit -->
```jai
// The measured inputs. mean[k-1] is the mean time of a k-draft block, for
// k = 1..n; a least-squares line through those n points. At least two.
dl_fit :: (mean:[]f64)->DlLine #export_c {
    n:=mean.count;if n<2 || n>DL_MAX {return DlLine.{status=DlStatus.shape};}
    for m:mean {if !dl_time(m) {return DlLine.{status=DlStatus.cost};}}
    sx:=0.0;sy:=0.0;for i:0..<n {sx+=cast(f64)(i+1);sy+=mean[i];}
    mx:=sx/cast(f64)n;my:=sy/cast(f64)n;sxy:=0.0;sxx:=0.0;
    for i:0..<n {dx:=cast(f64)(i+1)-mx;sxy+=dx*mean[i];sxx+=dx*dx;}
    slope:=sxy/sxx;return DlLine.{base=my-slope*mx,slope=slope};
}
// cost[0] is the measured decode step; cost[k] = base + slope * k. Refused
// when the line gives a time that is not positive.
dl_costs :: (step:f64,line:DlLine,cost:[]f64)->DlStatus #export_c {
    if line.status!=DlStatus.ok || cost.count<1 || cost.count>DL_MAX+1 {return DlStatus.shape;}
    if !dl_time(step) {return DlStatus.cost;}
    cost[0]=step;
    for k:1..<cost.count {c:=line.base+line.slope*cast(f64)k;if !dl_time(c) {return DlStatus.cost;}cost[k]=c;}
    return DlStatus.ok;
}
// Acceptance per position from counted blocks: reached[i] blocks proposed a
// draft i after drafts 0..i-1 were accepted, and accepted[i] of them kept it.
// Laplace's rule, (accepted + 1) / (reached + 2), so a position never seen
// is a coin flip rather than certain.
dl_calibrate :: (reached:[]int,accepted:[]int,accept:[]f64)->DlStatus #export_c {
    n:=reached.count;if n>DL_MAX || accepted.count!=n || accept.count<n {return DlStatus.shape;}
    for i:0..<n {if accepted[i]<0 || accepted[i]>reached[i] {return DlStatus.count;}}
    for i:0..<n {accept[i]=cast(f64)(accepted[i]+1)/cast(f64)(reached[i]+2);}
    return DlStatus.ok;
}
```

A draft that reports its own probabilities can supply a[i] directly. On
chapter 87's head, `dl_run` uses the head's probability of each token it
drafted as that draft's acceptance estimate:

<!-- source: psionic/lib/draft-length-run.coax run -->
```jai
// written: tokens in out. rounds: plain steps plus blocks. chosen[k]: rounds
// that verified k drafts. drafted and accepted: draft tokens verified and
// kept. target: target rows evaluated, one per round plus one per draft.
// time: the modeled cost of every round.
DlRun :: struct {status:DlStatus;written:int;rounds:int;drafted:int;accepted:int;target:int;chosen:[DL_MAX+1]int;time:f64;}
// fixed < 0: adaptive. fixed = 0..most: always that length.
dl_run :: (t:DhCell,head:DhCell,prompt:[]int,steps:int,most:int,fixed:int,cost:[]f64,out:[]int)->DlRun {
    r:=DlRun.{status=DlStatus.shape};
    if most<1 || most>DH_DRAFTS_MAX || fixed>most || cost.count!=most+1 {return r;}
    if dh_check(t,head,prompt,steps,cost[0..<0],out)!=DhStatus.ok {return r;}
    for c:cost {if !dl_time(c) {r.status=DlStatus.cost;return r;}}
    r.status=DlStatus.ok;w:=t.width;v:=t.vocab;
    hs:[(DH_DRAFTS_MAX+2)*DH_WIDTH_MAX]f64;p:[(DH_DRAFTS_MAX+1)*DH_VOCAB_MAX]f64;q:[DH_DRAFTS_MAX*DH_VOCAB_MAX]f64;
    drafts:[DH_DRAFTS_MAX]int;emitted:[DH_DRAFTS_MAX+1]int;stage:[DH_DRAFTS_MAX+1]int;residual:[DH_VOCAB_MAX]f64;scratch:[DH_VOCAB_MAX]f64;
    accept:[DH_DRAFTS_MAX]f64;zeros:[2*DH_DRAFTS_MAX+1]f64;
    dh_prefill(t,prompt,hs[0..<w]);pending:=prompt[prompt.count-1];
    while r.written<steps {
        used:=dh_draft(head,hs[0..<w],pending,most,true,accept[..],drafts[..],q[..]);#assert used==0;
        for j:0..<most {accept[j]=q[j*v+drafts[j]];}
        k:=fixed;if k<0 {k=dl_choose(accept[0..<most],cost).k;}
        r.rounds+=1;r.chosen[k]+=1;r.time+=cost[k];
        token:=pending;
        for i:0..<k+1 {
            dh_step(t,hs[i*w..<hs.count],token,hs[(i+1)*w..<hs.count]);dh_probs(t,hs[(i+1)*w..<hs.count],p[i*v..<p.count]);r.target+=1;
            if i<k {token=drafts[i];}
        }
        if k==0 {pending=gx_argmax(p[0..<v]);out[r.written]=pending;r.written+=1;for j:0..<w {hs[j]=hs[w+j];}continue;}
        b:=gx_block(v,p[0..<(k+1)*v],q[0..<k*v],drafts[0..<k],zeros[0..<2*k+1],true,emitted[..],stage[..],residual[..],scratch[..],false);
        #assert b.status==GStatus.ok;
        for i:0..<b.written {if r.written<steps {out[r.written]=emitted[i];r.written+=1;}}
        r.drafted+=k;r.accepted+=b.accepted;
        a:=b.accepted+1;for j:0..<w {hs[j]=hs[a*w+j];}pending=emitted[b.written-1];
    }
    return r;
}
```

The modeled time is the sum of the cost table's entries for the lengths
used. It is not a clock reading; it shows what the rule does with a given
cost curve.

## Run it

The [example](../examples/draft-length.coax) works the hand example, fits
the RTX 4080's calibration pass from its
[receipt](../fixtures/draft-length/rtx4080.coax), runs chapter 87's head
under three cost tables and applies chapter 76's rule to the timed runs:

```text
k=0: E=1.0 cost=10.0 rate=0.1
k=1: E=1.75 cost=16.0 rate=0.109375
k=2: E=2.125 cost=20.0 rate=0.10625
k=3: E=2.3125 cost=24.0 rate=0.09635416666666667
k=4: E=2.359375 cost=28.0 rate=0.08426339285714286
chosen: k=1
RTX 4080 cost line: c(k) = 43997699.5 + 6787647.1 k ns (ok); in steps: 1.0 3.3634123239195963 3.812944668089068 4.262477012258539 4.712009356428012
acceptance model (ok): 81/90 -> 0.8913043478260869 4/81 -> 0.060240963855421686 3/4 -> 0.6666666666666666 2/3 -> 0.6
chosen with all four drafts: k=0; with certain drafts: k=4
blocks of 1.2 + 0.2k steps (48 steps alone):
  adaptive: 23.200000000000003 steps of time, same true, accepted 34/41, lengths 0 1 6 4 4
  fixed k=0: 48.0 steps of time, same true, accepted 0/0, lengths 48 0 0 0 0
  fixed k=1: 34.8 steps of time, same true, accepted 20/29, lengths 0 29 0 0 0
  fixed k=2: 29.399999999999988 steps of time, same true, accepted 29/42, lengths 0 0 21 0 0
  fixed k=3: 30.40000000000001 steps of time, same true, accepted 32/57, lengths 0 0 0 19 0
  fixed k=4: 23.400000000000006 steps of time, same true, accepted 39/52, lengths 0 0 0 0 13
the RTX 4080 line (48 steps alone):
  adaptive: 48.0 steps of time, same true, accepted 0/0, lengths 48 0 0 0 0
  fixed k=0: 48.0 steps of time, same true, accepted 0/0, lengths 48 0 0 0 0
  fixed k=1: 97.53895739366824 steps of time, same true, accepted 20/29, lengths 0 29 0 0 0
  fixed k=2: 80.07183802987043 steps of time, same true, accepted 29/42, lengths 0 0 21 0 0
  fixed k=3: 80.98706323291223 steps of time, same true, accepted 32/57, lengths 0 0 0 19 0
  fixed k=4: 61.256121633564156 steps of time, same true, accepted 39/52, lengths 0 0 0 0 13
blocks of 1 + 0.1k steps (48 steps alone):
  adaptive: 17.8 steps of time, same true, accepted 39/48, lengths 0 0 1 2 10
  fixed k=0: 48.0 steps of time, same true, accepted 0/0, lengths 48 0 0 0 0
  fixed k=1: 31.900000000000016 steps of time, same true, accepted 20/29, lengths 0 29 0 0 0
  fixed k=2: 25.199999999999992 steps of time, same true, accepted 29/42, lengths 0 0 21 0 0
  fixed k=3: 24.700000000000006 steps of time, same true, accepted 32/57, lengths 0 0 0 19 0
  fixed k=4: 18.2 steps of time, same true, accepted 39/52, lengths 0 0 0 0 13
RTX 4080 target only: 65.055805011965 tokens/s
RTX 4080 fixed k=1: 45.417583110998756 tokens/s, saving -0.43239249109692207 against target only: off
RTX 4080 fixed k=2: 38.87333229097862 tokens/s, saving -0.6735330155131205 against target only: off
RTX 4080 fixed k=3: 37.17467256539575 tokens/s, saving -0.750003443810358 against target only: off
RTX 4080 fixed k=4: 35.84202803989559 tokens/s, saving -0.8150704234579498 against target only: off
RTX 4080 adaptive: 64.76830008677965 tokens/s, saving -0.004438975931129096 against target only: off
RTX 4080 adaptive against fixed k=1, the fastest fixed length: saving 0.29876833188232327, on
```

<!-- source: psionic/examples/draft-length.coax example -->
```jai
main :: () {
    // By hand: acceptance 0.75, 0.5, 0.5, 0.25; a step costs 10, a block 12 + 4k.
    a:=[4]f64.[0.75,0.5,0.5,0.25];c:=[5]f64.[10,16,20,24,28];
    for k:0..<5 {e:=dl_expected(a[..],k);print("k=%: E=% cost=% rate=%\n",k,e,c[k],e/c[k]);}
    d:=dl_choose(a[..],c[..]);print("chosen: k=%\n",d.k);

    // The RTX 4080 calibration pass: mean block times for k = 1..4, the plain
    // step, and per-position acceptance counts at k = 4.
    means:=DL_R1_BLOCK_NS;line:=dl_fit(means[..]);costs:[5]f64;
    st:=dl_costs(DL_R1_STEP_NS,line,costs[..]);
    print("RTX 4080 cost line: c(k) = % + % k ns (%); in steps:",line.base,line.slope,dl_status_name(st));
    for k:0..<5 {print(" %",costs[k]/costs[0]);}print("\n");
    reached:=DL_R1_REACHED;kept:=DL_R1_KEPT;accept:[4]f64;st=dl_calibrate(reached[..],kept[..],accept[..]);
    print("acceptance model (%):",dl_status_name(st));for i:0..<4 {print(" %/% -> %",kept[i],reached[i],accept[i]);}print("\n");
    one:=[4]f64.[1,1,1,1];
    print("chosen with all four drafts: k=%; with certain drafts: k=%\n",dl_choose(accept[..],costs[..]).k,dl_choose(one[..],costs[..]).k);

    // Chapter 87's dh-walk-v1 (48 greedy tokens), its head's own probabilities
    // as acceptance, under three cost tables in step units. Modeled time.
    x:=dl_pair(1,0.5);
    t:=DhCell.{vocab=8,width=4,recur=x.recur[..],embed=x.embed[..],out=x.out[..]};
    head:=DhCell.{vocab=8,width=4,recur=x.hr[..],embed=x.he[..],out=x.out[..]};
    prompt:=[3]int.[1,2,3];none:[1]f64;base:[48]int;o:[48]int;
    g:=dh_generate(t,prompt[..],48,true,none[0..<0],base[..]);
    tables:=[15]f64.[1,1.2,1.4,1.6,1.8, 1,0,0,0,0, 1,1.1,1.2,1.3,1.4];
    for k:0..<5 {tables[5+k]=costs[k]/costs[0];}
    names:=[3]string.["blocks of 1.2 + 0.2k steps","the RTX 4080 line","blocks of 1 + 0.1k steps"];
    for ti:0..<3 {
        table:=tables[ti*5..<ti*5+5];print("% (% steps alone):\n",names[ti],g.written);
        for fixed:-1..<5 {
            r:=dl_run(t,head,prompt[..],48,4,fixed,table,o[..]);same:=true;for i:0..<48 {if o[i]!=base[i] {same=false;}}
            if fixed<0 {print("  adaptive");}else {print("  fixed k=%",fixed);}
            print(": % steps of time, same %, accepted %/%, lengths % % % % %\n",r.time,same,r.accepted,r.drafted,r.chosen[0],r.chosen[1],r.chosen[2],r.chosen[3],r.chosen[4]);
        }
    }

    // The timed runs on the RTX 4080: every path against target only.
    paths:=[6]string.["target only","fixed k=1","fixed k=2","fixed k=3","fixed k=4","adaptive"];runs:=DL_R1_RUNS;
    base_runs:[5]int;for r:0..<5 {base_runs[r]=runs[r*6];}
    for p:0..<6 {
        mine:[5]int;sum:=0;for r:0..<5 {mine[r]=runs[r*6+p];sum+=mine[r];}
        print("RTX 4080 %: % tokens/s",paths[p],cast(f64)DL_R1_IDS*5000000000/cast(f64)sum);
        if p>0 {v:=sg_decide(DL_R1_SAME,base_runs[..],mine[..]);print(", saving % against target only: %",v.gain,sg_verdict_name(v.verdict));}
        print("\n");
    }
    fixed:[5]int;adaptive:[5]int;for r:0..<5 {fixed[r]=runs[r*6+1];adaptive[r]=runs[r*6+5];}
    v:=sg_decide(DL_R1_SAME,fixed[..],adaptive[..]);
    print("RTX 4080 adaptive against fixed k=1, the fastest fixed length: saving %, %\n",v.gain,sg_verdict_name(v.verdict));
}
```

The first block repeats the hand example. The second fits the RTX 4080's
calibration pass. A one-draft block costs 3.36 decode steps, and the
line's fixed part (44.0 ms) is almost three steps on its own: the
batched prefill, the cache save and restore, and a host read per readout
row. Acceptance has a sharp shape: the prompt n-gram's first draft is kept
81 times in 90, the second only 4 times in 81. So E(1) is about 1.89 tokens
for 3.36 steps of time, and the rule picks k = 0. With certain drafts it
would pick k = 4, since 5 tokens for 4.7 steps beats 1 for 1, so the cost
curve alone does not rule speculation out; this drafter's acceptance does.

The third block runs chapter 87's workload dh-walk-v1 under three cost
tables, in units of one decode step. With blocks of 1.2 + 0.2k steps, the
adaptive length (23.2 steps of time) narrowly beats the best fixed length
(k = 4, 23.4) and takes under half the target alone's 48. With blocks of
1 + 0.1k steps it takes 17.8 against 18.2.
Under the RTX 4080's own line, every fixed length is slower than the target
alone, and the adaptive rule chooses k = 0 at all 48 steps, which is exactly
the target alone. The output is the target's greedy output in every case.

## On the RTX 4080

```shell
COAX_CC=gcc psionic/tests/check_draft_length.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee 5
```

The harness uses chapter 76's device programs unchanged and chapter 71's two
fixture prompts (greedy, up to 256 IDs). It first runs a calibration pass,
untimed for the verdicts, that fits the cost line and the acceptance model
above. Then it times five runs of six paths, rotating the path order each
run: target only, fixed k = 1, 2, 3 and 4, and adaptive k (prompt n-gram
drafts of up to 4, then `dl_choose`). Every path's IDs equal the target-only
IDs on both prompts (output words `447e9a1f919e90b3`, `7c6705e4ab014dfd`,
chapter 71's reference words). Chapter 76's rule, `sg_decide`, compares each
path with the target alone.

| Path | Mean per run | Tokens/s | Against target only |
| --- | ---: | ---: | --- |
| target only | 4.304 s | 65.06 | |
| fixed k = 1 | 6.165 s | 45.42 | saving −43.2%: off |
| fixed k = 2 | 7.203 s | 38.87 | saving −67.4%: off |
| fixed k = 3 | 7.532 s | 37.17 | saving −75.0%: off |
| fixed k = 4 | 7.812 s | 35.84 | saving −81.5%: off |
| adaptive | 4.323 s | 64.77 | saving −0.4% (se 67 ms): off |

The adaptive rule chose k = 0 at every step: 23 plain steps on the first
prompt and 255 on the second, no block. It is 29.9% faster than the fastest
fixed length (k = 1), which chapter 76's rule calls a clear gain, and
indistinguishable from the target alone, which is what it ran; the 0.4% is
the host-side n-gram drafting it still does before deciding not to use the
drafts. So the honest result on this host and workload is k = 0: speculation
with this drafter does not pay, and the rule finds that without a separate
switch.

Other agents were using the machine's processors during the timed runs (load
average 5 to 26), which widens every spread; the verdicts above all clear
chapter 34's spread rule. The
[record](../../docs/verification/psionic-ch102-draft-length-2026-11-02.md)
has the full log.

## Checks

- The hand example, ties, the `dl_never` boundary and every refusal, as
  `#test`s.
- `dl_choose` equals a brute-force argmax over every length (280 cases), in
  which E(k) is summed from the distribution of the accepted count,
  Σ (m + 1)·P(exactly m accepted), rather than from survival products. With
  probabilities in multiples of 1/8 and integer costs, both sums are exact,
  so ties are real ties and the two must agree exactly.
- On vocabularies of 2 to 4 and blocks of 1 to 3, E(k) from `dl_overlap`
  equals the expectation over every draft sequence, weighted by its draft
  probability, of the tokens committed, with each position's acceptance
  taken from chapter 75's `gx_decide` (54 cases, within 1e-12).
- Greedy parity: on three synthetic pairs, four cost tables, draft limits 1
  to 4 and every fixed length plus adaptive, `dl_run`'s output equals the
  target alone's greedy output token for token. Fixed k = 4 reproduces
  chapter 87's counts (13 blocks, 39 of 52 accepted, 65 target rows).
- On the RTX 4080, every path's output equals the target-only output.
- `coax mutate --root-only` kills every mutant in both files: 111 of 111 in
  `draft-length.coax`, 121 of 121 in `draft-length-run.coax`.

The [refusal example](../examples/draft-length-refuse.coax) breaks one input
at a time:

```text
a valid choice: ok (k=1)
three drafts, three costs: shape
acceptance 1.5: probability outside [0, 1]
a block that costs 0: cost not positive and finite
a cost line from one length: shape
a line that falls below zero by k=3: cost not positive and finite
5 kept of 4 reached: accepted count outside 0..reached
a fixed length of 5 with 4 drafted: shape
```

## What is not built yet

- **DeepSeek V4.1's draft layers.** The draft wanted to verify V4.1's own
  draft layers ([chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md)).
  They are not run; the measured drafter is chapter 75's prompt n-gram.
- **A trained confidence head.** Acceptance here is per-position counts
  (n-gram) or the draft's own probability (chapter 87's head). A small head
  trained to predict acceptance from features, with chapter 103's trainer
  and [chapter 215](ch215-fit-our-own-calibration.md)'s calibration, is not
  built.
- **Chapter 99's late speculative-length path.** Each step here waits for
  the previous block's accepted length before choosing the next. Starting a
  step before that is known is not built.
- **Sampled verification on the device.** As in chapter 76, only greedy
  blocks run on the GPU.
- **A cheaper verification pass.** The fixed 44.0 ms of every block is
  what keeps k = 0 optimal here. Chapter 76 lists its parts.

Until a measurement shows drafting gains over the target alone, speculation
stays off by default.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md)
- [Record of the RTX 4080 run](../../docs/verification/psionic-ch102-draft-length-2026-11-02.md)
