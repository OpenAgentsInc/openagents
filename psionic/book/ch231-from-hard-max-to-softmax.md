# 231. From hard-max to softmax

[Chapter 225](ch225-attention-as-exact-memory.md) read memory with a hard
maximum over integer scores. A transformer doesn't take a hard maximum; it
takes a softmax, and it computes in a floating-point format. This chapter asks
when that still gives the exact stored value. The answer has two parts: a
bound that doesn't depend on the trace length at all, and a number-format
limit that does.

```shell
coax/bin/coax run psionic/examples/softmax-limits.coax
psionic/tests/check_soft.sh
```

```jai
values in [-1000, 1000]: exact for every trace length once beta > ln(4V) = 8.294049640102028
binary64 keys up to 16: exact through 65536 writes
binary64 keys up to 256: exact through 65536 writes
binary64 keys up to 4096: exact through 65536 writes
binary64 keys up to 65536: exact through 65536 writes
binary32 keys up to 16: exact through 8192 writes, wrong at 16384
binary32 keys up to 256: exact through 32 writes, wrong at 64
binary32 keys up to 4096: exact through 1 writes, wrong at 2
binary32 keys up to 65536: exact through 1 writes, wrong at 2
bfloat16 keys up to 16: exact through 1 writes, wrong at 2
bfloat16 keys up to 256: exact through 1 writes, wrong at 2
bfloat16 keys up to 4096: exact through 1 writes, wrong at 2
bfloat16 keys up to 65536: exact through 1 writes, wrong at 2
binary64 at beta = ln(4V) - 1: exact through 1 writes, wrong at 2
```

The [example](../examples/softmax-limits.coax) takes about five seconds on a
CPU. Nothing here needs model weights or a GPU.

## Scores a softmax can use

A hard maximum can break ties by rule. A softmax can't: two writes of the same
key with the same score get the same weight, and the read returns their
average. So the soft read uses the recency-weighted score that chapter 225
described: the parabola score times `T + 1`, plus the write's position `t`.
Every score is then a distinct integer, and the latest write of the queried
key wins by a margin of at least 1 over every other write.

## The bound

Multiply the scores by a sharpness `β` and take the softmax. Let `b` be the
winning write and `v` its value. Because the weights sum to 1, the soft read
`y` differs from `v_b` by

```jai
y - v_b = sum over j != b of w_j * (v_j - v_b)
```

With values in `[-V, V]`, each difference is at most `2V`. Each other write
trails the winner by a distinct positive integer margin, so its weight is at
most `e^(-βm)` over a total that includes the winner's 1. Summing the
geometric series, the weight left over for all other writes is below `e^(-β)`
however many there are. The error is therefore below `2V·e^(-β)`, and
rounding to the nearest integer gives exactly `v_b` whenever that is at most
one half:

```jai
β ≥ ln(4V)
```

The trace length doesn't appear. More writes can't push the error past the
bound, because each one is further behind than the last. For values up to
1,000 the bound is 8.29. Below it, the same construction fails quickly: at
`β = ln(4V) − 1`, a single earlier write of the opposite value is already
enough.

<!-- source: psionic/lib/soft-memory.coax bound -->
```jai
// The smallest sharpness that keeps every read exact, for values in [-V, V].
sm_beta_bound :: (v:int)->f64 #export_c {
    if v<1 || v>EM_VALUE_MAX {return 0;}
    return log(4.0*f64(v));
}
```

## Every quantity in a format

A real model rounds every number it holds. The read rounds each score, each
scaled logit, each probability and each stored value to the format under test
before using it. The exponentials and sums are binary64 with compensated
addition, chapter 4's arithmetic profile. bfloat16 is binary32 rounded to its
top 16 bits, to nearest, ties to even.

<!-- source: psionic/lib/soft-memory.coax format -->
```jai
// A number format rounds every value a transformer would hold: scores,
// logits, probabilities and the stored values themselves.
sm_round :: (x:f64,format:SmFormat)->f64 {
    if format==SmFormat.f64 {return x;}
    bits:=f32_bits(f32(x));
    if format==SmFormat.bf16 {bits=(bits+0x7fff+((bits>>16)&1))&0xffff0000;}
    return f64(f32_from_bits(bits));
}
```

<!-- source: psionic/lib/soft-memory.coax read -->
```jai
// Softmax over recency-weighted scores at sharpness `beta`, every quantity
// rounded to `format`, then the weighted value rounded to the nearest integer.
// Exact means that integer is the value a hard-max read returns.
sm_read :: (keys:[]int,values:[]int,count:int,q:int,beta:f64,format:SmFormat)->SmRead #export_c {
    hard:=em_read(keys,values,count,q);
    if hard.status==EmStatus.capacity {return SmRead.{status=SmStatus.capacity};}
    if hard.status==EmStatus.range {return SmRead.{status=SmStatus.range};}
    if hard.status==EmStatus.missing {return SmRead.{status=SmStatus.missing};}
    if !(beta>0) || beta>f64(SM_BETA_MAX) {return SmRead.{status=SmStatus.beta};}
    for t:0..<count {if values[t]< -EM_VALUE_MAX || values[t]>EM_VALUE_MAX {return SmRead.{status=SmStatus.range};}}
    b:=hard.position;
    top:=sm_logit(keys[b],q,b,count,beta,format);
    z:PsionicAccumulator;
    for t:0..<count {
        d:=sm_logit(keys[t],q,t,count,beta,format)-top;
        if !sm_shift_ok(d) {return SmRead.{status=SmStatus.unstable};}
        z=psionic_ops_accumulate(z,exp(d));
    }
    total:=psionic_ops_total(z);y:PsionicAccumulator;
    for t:0..<count {
        w:=sm_round(exp(sm_logit(keys[t],q,t,count,beta,format)-top)/total,format);
        y=psionic_ops_accumulate(y,w*sm_round(f64(values[t]),format));
    }
    mixed:=psionic_ops_total(y);value:=sm_nearest(mixed);
    return SmRead.{value=value,mixed=mixed,exact=value==values[b]};
}
// The chapter 225 score with the recency term a transformer needs: scaled by
// count + 1 so that position t breaks ties but never outweighs a key mismatch.
sm_logit :: (k:int,q:int,t:int,count:int,beta:f64,format:SmFormat)->f64 {
    score:=em_score(k,q)*(count+1)+t;
    return sm_round(beta*sm_round(f64(score),format),format);
}
```

Before exponentiating, the read subtracts the winner's own logit. A rounded
logit more than 700 above it would overflow, and that case is refused as
`unstable` rather than reported as a number.

## Where the format breaks it

The limit scan builds two adversarial streams at the largest key `K`:
- **Same key:** `T` writes to the queried key, where only the latest holds `+V`
  and the rest hold `−V`.
- **Neighbouring key:** one write of `+V`, then `T − 1` writes of `−V` to the
  key next to it.

Each stream is scanned on its own, `T` doubling until its read is wrong, and
the example reports whichever fails first.

<!-- source: psionic/lib/soft-memory.coax limit -->
```jai
// Two adversarial streams of length n at the largest key magnitude k. Near:
// one write of +v to k, then n - 1 writes of -v to the neighbouring key k - 1.
// Same: n writes to k, the latest holding +v and the rest -v. Each stream's n
// doubles from 1 up to the scratch length until its read is wrong.
sm_scan :: (near:bool,k:int,v:int,beta:f64,format:SmFormat,keys:[]int,values:[]int)->SmScan {
    result:SmScan;n:=1;
    while n<=keys.count {
        if near {
            keys[0]=k;values[0]=v;
            for t:1..<n {keys[t]=k-1;values[t]=-v;}
        } else {
            for t:0..<n {keys[t]=k;values[t]=-v;}
            values[n-1]=v;
        }
        if !sm_read(keys,values,n,k,beta,format).exact {result.failure=n;return result;}
        result.until=n;n*=2;
    }
    return result;
}
sm_limit :: (k:int,v:int,beta:f64,format:SmFormat,keys:[]int,values:[]int,overlap:bool)->SmLimit #export_c {
    if keys.count<1 || values.count<keys.count {return SmLimit.{status=SmStatus.capacity};}
    if k<1 || k>EM_KEY_MAX || v<1 || v>EM_VALUE_MAX {return SmLimit.{status=SmStatus.range};}
    if overlap {return SmLimit.{status=SmStatus.alias};}
    r:=SmLimit.{near=sm_scan(true,k,v,beta,format,keys,values)};
    r.same=sm_scan(false,k,v,beta,format,keys,values);
    return r;
}
```

The measurements show what breaks first:

- **binary64** stays exact through 65,536 writes at every key size tested.
  Its 53-bit significand gives a limit roughly 2²⁹ times further out than
  binary32's, well beyond this scan.
- **binary32** fails when the scaled logits grow so large that the rounding
  step (its unit in the last place) exceeds the margin between neighbouring
  writes. The scores grow as `K² × (T + 1)`, so the limit is set by that
  product. Keys up to 16 fail at 16,384 writes, and keys up to 256 at 64 writes:
  both near `K² × T ≈ 2²²` at this sharpness. Keys of 4,096 already fail at
  the second write.
  The neighbouring-key stream holds out longer, because a wrong key trails by
  a margin of at least 2. At keys up to 256 it fails at 512 writes, eight
  times later.
- **bfloat16** keeps 8 significant bits. Even keys up to 16 score 768 at two
  writes, where bfloat16 can't tell 768 from 769, so the two writes tie and
  their values average.

## What this means for the rest of Part XXIII

Softmax by itself isn't the problem: above `ln(4V)` it's exact at any length.
The number format is. A compiled machine whose attention runs in bfloat16
can't use this encoding at all. One in binary32 is limited to small memories
and short traces. Its address range squared times its trace length must stay
under about four million. Chapters 227–229 must therefore choose one of
these, and state which:

- run memory attention in binary64;
- keep `K² × T` inside binary32's limit;
- encode addresses so scores stay small, for example by splitting an address
  across several heads, each with a small key range.

## How it is checked

- **Coax checks** cover the format rounding (including bfloat16 ties to even),
  nearest-integer rounding, the shift limit, the bound, exact reads at the
  bound in every format, the leak below it, the bfloat16 tie, the limit scan
  and every refusal.
- **An independent oracle** in [soft_oracle.py](../tests/soft_oracle.py)
  recomputes 160 random reads (119 exact, 41 not) and seven per-stream limit
  scans with
  60-digit decimal arithmetic in place of floating-point sums. The native
  harness [soft.c](../tests/soft.c) agrees on every rounded value, every
  exactness verdict and every scan. It runs plain and under ASan/UBSan, in
  debug and release.
- **Mutation testing** of `soft-memory.coax` kills all 1,593 mutants. The
  earlier runs left 14 and then 5 alive, which led to the half-up rounding
  rewrite and the bound checks. They also exposed scans that reused scratch a
  previous scan had filled, which hid broken writes. Each stream now gets its
  own scan, tested on fresh scratch.

The [verification record](../../docs/verification/psionic-ch172-2026-10-29.md)
has the commands and results.

## What this does not claim

The format model rounds the integer score directly. A transformer instead
forms it as a dot product of query and key vectors, so the plan's measurement
under chapter 41's attention kernel is still to be run when that kernel
exists. The scan stops at 65,536 writes, so binary64's own limit is estimated,
not measured.
