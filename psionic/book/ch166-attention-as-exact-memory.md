# 166. Attention as exact memory

Attention compares a query with every key and mixes the values by how well
they match. Usually the mixture is soft and approximate. With keys placed
carefully and a hard maximum instead of a softmax, the same operation becomes
an exact lookup: the read returns precisely the value that was stored, or
says there is none.

That lookup is what lets a transformer act as the machine of
[chapter 165](ch165-a-machine-whose-state-is-a-trace.md). Its memory is not a
separate array; it is the earlier tokens of its own trace.

```shell
coax/bin/coax run psionic/examples/exact-trace.coax
psionic/tests/check_trace.sh
```

## Keys on a parabola

Put key `k` at the point `(2k, -k²)` and turn query `q` into the direction
`(q, 1)`. Their dot product is

```jai
2kq - k² = q² - (k - q)²
```

which is largest exactly when `k = q`, and smaller by at least 1 for every
other integer key. The best match always wins, by a margin an integer can see.

Memory can hold several writes to the same address, and a read must return the
latest. Every write of the same key gets the same score, so the read needs a
rule for ties: a later write replaces an earlier one. In a transformer, a
small recency term added to the score does this. Multiply the score by
`T + 1`, where `T` is the number of writes, and add the write's position `t`.
Because `t` is less than `T + 1`, time can break ties but never outweigh a key
mismatch. Here the read states the rule directly, which keeps it visible and
testable.

<!-- source: psionic/lib/exact-memory.coax score -->
```jai
// Key k sits at the point (2k, -k*k); query q is the direction (q, 1).
// Their dot product is q*q - (k-q)*(k-q), so the match k = q scores highest
// by at least 1 over every other integer key.
em_score :: (k:int,q:int)->int {return 2*k*q-k*k;}
```

Keys are bounded to ±2²⁰, so every score fits in a 64-bit integer, and so does
the recency-scaled form for any trace this machine can produce.

## A hard-max read

The read scans the writes, keeps the highest score (the later write on a tie)
and checks that the winning key is the one asked for. A near miss, where the closest key differs from the
query, is a typed refusal (`missing`), not an approximate answer.

<!-- source: psionic/lib/exact-memory.coax read -->
```jai
em_read :: (keys:[]int,values:[]int,count:int,q:int)->EmRead #export_c {
    if count<0 || count>keys.count || count>values.count {return EmRead.{status=EmStatus.capacity};}
    if q< -EM_KEY_MAX || q>EM_KEY_MAX {return EmRead.{status=EmStatus.range};}
    for t:0..<count {if keys[t]< -EM_KEY_MAX || keys[t]>EM_KEY_MAX {return EmRead.{status=EmStatus.range};}}
    if count==0 {return EmRead.{status=EmStatus.missing};}
    // Hard max; on a tie the later write replaces the earlier one.
    best:=0;
    for t:1..<count {if em_score(keys[t],q)>=em_score(keys[best],q) {best=t;}}
    // The read is exact only if the winning key is the one asked for.
    if keys[best]!=q {return EmRead.{status=EmStatus.missing};}
    return EmRead.{value=values[best],position=best};
}
```

## Uniform keys sum

If every key scores the same, attention gives each earlier position the weight
`1/t`, and the read returns the mean. The mean times `t` is the running sum,
which is how a transformer can count. Here the mean is kept as an exact
fraction. A floating-point softmax over equal scores would produce the same
mean only approximately; chapter 172 measures where that stops being exact.

<!-- source: psionic/lib/exact-memory.coax sum -->
```jai
// Equal scores give every earlier position the same weight 1/t, so attention
// returns the mean; the mean times t is the running sum, exact as a fraction.
em_mean :: (values:[]int,count:int)->EmSum #export_c {
    if count<1 || count>values.count {return EmSum.{status=EmStatus.capacity};}
    n:=0;for t:0..<count {n+=values[t];if n< -EM_VALUE_MAX || n>EM_VALUE_MAX {return EmSum.{status=EmStatus.range};}}
    return EmSum.{numerator=n,denominator=count};
}
em_prefix_sums :: (values:[]int,sums:[]int)->EmStatus #export_c {
    stage:[256]int;if sums.count<values.count || values.count>256 {return EmStatus.capacity;}
    for t:0..<values.count {
        m:=em_mean(values,t+1);if m.status!=EmStatus.ok {return m.status;}
        stage[t]=m.numerator;  // mean (numerator/denominator) times t+1
    }
    for t:0..<values.count {sums[t]=stage[t];}
    return EmStatus.ok;
}
```

## The machine with attention for memory

`em_run` is chapter 165's machine with no memory of its own. Before every
step, its view of all 64 addresses is rebuilt from nothing but hard-max reads
over the `store` rows already in its trace, and an address never stored reads
as zero. A `load` sees only that view. If attention returned a wrong value,
the trace would differ and so would the digest.

<!-- source: psionic/lib/exact-memory.coax run -->
```jai
// The chapter 165 machine with no memory of its own: before every step its
// view of memory is rebuilt from nothing but hard-max reads over the stores
// already in its trace, and an address never stored reads as zero.
em_pass :: (program:[]TmOp,budget:int,trace:[]int,outputs:[]int,keys:[]int,values:[]int,publish:bool)->TmResult {
    s:TmState;ip:=0;steps:=0;stores:=0;stage_out:[64]int;row:[4]int;bytes:[16]u8;
    sha:[112]u8;ok:=sha256_init(sha[..])==SHA_OK;
    while true {
        if ip==program.count {return TmResult.{status=TmStatus.fell,ip=ip,steps=steps};}
        if steps>=budget {return TmResult.{status=TmStatus.budget,ip=ip,steps=steps};}
        view:[64]int;
        for c:0..<TM_CELLS {view[c]=em_read(keys,values,stores,c).value;}
        r:=tm_step(program,ip,s,view[..],row[..]);
        if r.status!=TmStatus.ok {return TmResult.{status=r.status,ip=ip,steps=steps};}
        if program[ip].code==TmCode.store {
            if stores>=keys.count || stores>=values.count {return TmResult.{status=TmStatus.capacity,ip=ip,steps=steps};}
            keys[stores]=row[3];values[stores]=row[2];stores+=1;
        }
        if r.state.outputs>s.outputs {stage_out[s.outputs]=row[2];}
        if (steps+1)*TM_TOKENS>trace.count || r.state.outputs>outputs.count {return TmResult.{status=TmStatus.capacity,ip=ip,steps=steps};}
        if publish {for t:0..<TM_TOKENS {trace[steps*TM_TOKENS+t]=row[t];}}
        tm_bytes(row[..],bytes[..]);ok=ok && sha256_update_checked(sha[..],bytes[..],false)==SHA_OK;
        s=r.state;steps+=1;
        if r.next<0 {break;}
        ip=r.next;
    }
    result:=TmResult.{ip=ip,steps=steps,outputs=s.outputs};
    ok=ok && sha256_final_checked(sha[..],result.digest[..],false)==SHA_OK;
    if !ok {return TmResult.{status=TmStatus.digest};}
    for i:0..<s.outputs {outputs[i]=stage_out[i];}
    return result;
}
em_run :: (program:[]TmOp,budget:int,trace:[]int,outputs:[]int,keys:[]int,values:[]int,overlap:bool)->TmResult #export_c {
    admitted:=tm_admit(program);if admitted!=TmStatus.ok {return TmResult.{status=admitted};}
    if budget<1 || budget>TM_STEPS_MAX {return TmResult.{status=TmStatus.budget};}
    if outputs.count<1 {return TmResult.{status=TmStatus.capacity};}
    if overlap {return TmResult.{status=TmStatus.alias};}
    first:=em_pass(program,budget,trace,outputs,keys,values,false);
    if first.status!=TmStatus.ok {return first;}
    return em_pass(program,budget,trace,outputs,keys,values,true);
}
```

The example stores `i * i` at addresses 4 down to 1, then prints
`mem[4] + mem[1]`. The plain interpreter and the attention-memory run take the
same 53 steps, print 17 and produce the same digest.

<!-- source: psionic/examples/exact-trace.coax example -->
```jai
main :: () {
    // Store i*i at address i for i = 4..1, then print mem[4] + mem[1].
    program:=[18]TmOp.[TmOp.{code=TmCode.push,arg=4},TmOp.{code=TmCode.set,arg=0},
        TmOp.{code=TmCode.get,arg=0},TmOp.{code=TmCode.get,arg=0},TmOp.{code=TmCode.get,arg=0},
        TmOp.{code=TmCode.mul},TmOp.{code=TmCode.store},
        TmOp.{code=TmCode.get,arg=0},TmOp.{code=TmCode.push,arg=1},TmOp.{code=TmCode.sub},
        TmOp.{code=TmCode.set,arg=0},TmOp.{code=TmCode.get,arg=0},TmOp.{code=TmCode.brif,arg=2},
        TmOp.{code=TmCode.push,arg=4},TmOp.{code=TmCode.load},TmOp.{code=TmCode.push,arg=1},
        TmOp.{code=TmCode.load},TmOp.{code=TmCode.add}];
    full:[20]TmOp;for i:0..<18 {full[i]=program[i];}
    full[18]=TmOp.{code=TmCode.out};full[19]=TmOp.{code=TmCode.ret};
    trace:[256]int;outputs:[1]int;a:=tm_run(full[..],64,trace[..],outputs[..],false);
    #assert a.status==TmStatus.ok;
    print("machine: % steps, output %, trace digest ",a.steps,outputs[0]);hex8(a.digest);
    keys:[8]int;values:[8]int;b:=em_run(full[..],64,trace[..],outputs[..],keys[..],values[..],false);
    #assert b.status==TmStatus.ok;
    print("memory by attention: % steps, output %, trace digest ",b.steps,outputs[0]);hex8(b.digest);
    k:=[4]int.[7,3,7,9];v:=[4]int.[10,20,30,40];r:=em_read(k[..],v[..],4,7);
    print("read key 7 from [7,3,7,9]: % (latest write, position %)\n",r.value,r.position);
    #assert em_read(k[..],v[..],4,8).status==EmStatus.missing;
    print("read key 8: refused as missing\n");
    spin:=[3]TmOp.[TmOp.{code=TmCode.push,arg=1},TmOp.{code=TmCode.brif,arg=0},TmOp.{code=TmCode.ret}];
    s:=tm_run(spin[..],64,trace[..],outputs[..],false);
    #assert s.status==TmStatus.budget;
    print("endless loop: refused after % steps, nothing published\n",s.steps);
}
// The first eight bytes of a digest in hexadecimal, then a newline.
hex8 :: (d:[32]u8) {
    digits:="0123456789abcdef";
    for i:0..<8 {hi:=cast(int)(d[i]>>4);lo:=cast(int)(d[i]&15);print("%",digits[hi..<hi+1]);print("%",digits[lo..<lo+1]);}
    print("\n");
}
```

## How it is checked

- **Coax checks:**
  - The parabola's margin, for every pair of small keys.
  - Latest-write reads and refusals for missing keys.
  - A seeded comparison with a map that keeps the latest write, for 24 streams
    of 40 writes, each read after every write.
  - Running sums.
  - The same digest from both machines.
- **The independent oracle** generates 40 more streams and their expected
  reads. The native harness checks all 600 reads.
- **All 300 oracle programs** run through `em_run` and must match the plain
  interpreter's digest.
- **The 49 boundary fixtures** run again through `em_run` in
  [memory-vectors.coax](../fixtures/memory-vectors.coax), with exactly enough
  key and value storage for the program's stores, and one write short.
- **Mutation testing** of `exact-memory.coax` kills all 1,191 mutants. The
  first run left 54 alive and showed a real weakness. The machine still kept
  an ordinary memory beside the attention one, so a broken attention read
  could have gone unnoticed. Memory became a parameter of each step, and the
  attention machine was left with no other source.

## What this does not do yet

This read scans every write, so a step costs time proportional to the trace
length. Chapter 170 answers the same hard-max query as a convex-hull search in
logarithmic time. The scores here are integers. A transformer computes them in
floating point with a very sharp softmax, and chapter 172 states when that is
still exact.
