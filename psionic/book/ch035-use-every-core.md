# 35. Use every core

> **Status: partly implemented.** A fixed partition, a tree-ordered merge and
> a publish check for a sum, a matrix-vector product and attention heads,
> byte-identical at 1, 2, 4 and 8 workers; a host thread pool that runs the
> Coax workers on disjoint chunks; and one measured speedup receipt on a
> named host. Running chapter 14's Elephant decision on the pool, the same
> gate on Coax jobs, and a quiet-host repeat are not built yet.

A decision on the CPU uses one core today. This chapter spreads the work over
several cores without changing a single output byte. Parallel work counts only
when it reproduces the single-worker result exactly under the named CPU
profile.

Two rules make that possible. The work is cut into chunks that depend on the
shape and a chunk size, never on the worker count; a worker count only decides
who computes each chunk. And partial sums combine in a fixed tree over chunk
indices, so the rounding does not depend on which worker finishes first.

The library is [`cores.coax`](../lib/cores.coax). Coax has no threads yet
([Coax chapter 14](../../coax/book/ch14-jobs.md)), so a small
[native harness](../platform/cores.c) owns the threads, the barrier that
starts and ends each run, the buffers and the clock. Every partition,
ownership and merge decision is a Coax procedure the harness calls.

## Why the worker count changes a sum

Floating-point addition is not associative. Take four `f32` values,
`[1e8, 1, -1e8, 1]`. Near 10^8 the gap between neighbouring `f32` values is
8, so `1e8 + 1` rounds back to `1e8`, and `-1e8 + 1` rounds back to `-1e8`.

The obvious way to use W workers is to give each one an equal share of the
items and add the workers' totals:

<!-- source: psionic/lib/cores.coax split -->
```jai
// What not to do: give each worker an equal share of the items and add the
// workers' totals. The grouping, and so the rounding, follows the worker count.
pc_sum_split :: (x:[]f32,workers:int)->f32 #export_c {
    if workers<1 {return 0;}
    total:f32=0;
    for w:0..<workers {
        t:f32=0;
        for i:w*x.count/workers..<(w+1)*x.count/workers {t+=x[i];}
        total+=t;
    }
    return total;
}
```

Trace it by hand:

- **One worker** adds left to right: `((1e8 + 1) + -1e8) + 1` =
  `(1e8 + -1e8) + 1` = **1**.
- **Two workers** each add two values: `(1e8 + 1) + (-1e8 + 1)` =
  `1e8 + -1e8` = **0**.
- **Four workers** each take one value, and their totals are added left to
  right, the same order as one worker: **1**.

The answer depends on the worker count, so a run on 2 cores and a run on 8
cores of the same model on the same input can disagree. Nothing is wrong with
either sum; they are different sums.

## Fixed chunks

The fix is to choose the grouping before choosing the workers. A plan cuts
`items` into chunks of `chunk` items, the last one possibly short. The chunks
depend on the shape and the chunk size only:

<!-- source: psionic/lib/cores.coax plan -->
```jai
// Cut items into chunks of chunk items. The chunks depend on the shape and
// the chunk size only; the worker count decides who computes them, never
// what they are.
pc_plan :: (items:int,chunk:int,workers:int)->PcPlan #export_c {
    if items<1 || items>PC_MAX_ITEMS {return PcPlan.{status=PcStatus.shape};}
    if workers<1 || workers>PC_MAX_WORKERS {return PcPlan.{status=PcStatus.workers};}
    if chunk<1 || chunk>items {return PcPlan.{status=PcStatus.chunk};}
    chunks:=(items+chunk-1)/chunk;
    if chunks>PC_MAX_CHUNKS {return PcPlan.{status=PcStatus.chunk};}
    return PcPlan.{status=PcStatus.ok,items=items,chunk=chunk,chunks=chunks,workers=workers};
}
// Worker w computes chunks pc_first(p, w) up to pc_first(p, w + 1).
pc_first :: (p:PcPlan,w:int)->int #export_c {return w*p.chunks/p.workers;}
// The one worker whose run holds chunk c.
pc_owner :: (p:PcPlan,c:int)->int #export_c {return ((c+1)*p.workers-1)/p.chunks;}
pc_lo :: (p:PcPlan,c:int)->int {return c*p.chunk;}
pc_hi :: (p:PcPlan,c:int)->int {if c==p.chunks-1 {return p.items;}return c*p.chunk+p.chunk;}
```

Worker `w` computes chunks `w * chunks / workers` up to
`(w + 1) * chunks / workers`. Trace 10 items in chunks of 4 over 4 workers:

- chunks: `(10 + 3) / 4` = 3, covering items 0–3, 4–7 and 8–9.
- runs: `pc_first` gives 0, 0, 1, 2 and 3 for w = 0 to 4. Worker 0 owns no
  chunk, worker 1 owns chunk 0, worker 2 chunk 1, worker 3 chunk 2.
- owners: `pc_owner(c)` = `((c + 1) * 4 - 1) / 3` gives 3 / 3 = 1,
  7 / 3 = 2 and 11 / 3 = 3, the same answer from the other side.

More workers than chunks is allowed; the extra workers do nothing. A check
compares `pc_owner` with a brute-force search over every worker for up to 39
chunks and 11 workers, and checks that the runs tile the chunks.

## A tree-ordered merge

Each worker sums each of its chunks in index order into that chunk's slot and
marks the slot with its worker number plus one. The merge checks that every
chunk was marked by its owner, then adds the chunk sums in a fixed tree:

<!-- source: psionic/lib/cores.coax sum -->
```jai
// Worker w sums each of its chunks in index order into that chunk's slot and
// marks the chunk with its number plus one. It touches no other slot.
pc_sum_worker :: (p:PcPlan,w:int,x:[]f32,part:[]f32,done:[]u8)->PcStatus #export_c {
    s:=pc_admit(p,w);if s!=PcStatus.ok {return s;}
    if x.count!=p.items || part.count!=p.chunks || done.count!=p.chunks {return PcStatus.size;}
    for c:pc_first(p,w)..<pc_first(p,w+1) {
        t:f32=0;
        for i:pc_lo(p,c)..<pc_hi(p,c) {t+=x[i];}
        part[c]=t;done[c]=cast(u8)(w+1);
    }
    return PcStatus.ok;
}
// Every chunk must be marked by its owner; otherwise a worker failed or was
// cancelled, and nothing is published.
pc_publish :: (p:PcPlan,done:[]u8)->PcStatus #export_c {
    if p.status!=PcStatus.ok {return p.status;}
    if done.count!=p.chunks {return PcStatus.size;}
    for c:0..<p.chunks {if cast(int)done[c]!=pc_owner(p,c)+1 {return PcStatus.incomplete;}}
    return PcStatus.ok;
}
// Combine the chunk sums in a fixed tree: split the chunks at the largest
// power of two below their count, total each side, add the two. Four chunks
// give (p0 + p1) + (p2 + p3); five give ((p0 + p1) + (p2 + p3)) + p4.
pc_tree :: (part:[]f32)->f32 {
    if part.count==0 {return 0;}
    if part.count==1 {return part[0];}
    half:=1;while half*2<part.count {half*=2;}
    return pc_tree(part[0..<half])+pc_tree(part[half..<part.count]);
}
pc_sum_merge :: (p:PcPlan,part:[]f32,done:[]u8)->PcValue #export_c {
    if p.status==PcStatus.ok && part.count!=p.chunks {return PcValue.{status=PcStatus.size};}
    s:=pc_publish(p,done);if s!=PcStatus.ok {return PcValue.{status=s};}
    return PcValue.{status=PcStatus.ok,value=pc_tree(part)};
}
```

The tree splits the chunks at the largest power of two below their count. For
five chunk sums `p0` to `p4` it computes `((p0 + p1) + (p2 + p3)) + p4`. The
shape of the tree depends only on the number of chunks, so the bytes of the
total depend only on the plan's chunks and the input.

Trace the four values again with chunks of 2. Chunk 0 is `1e8 + 1` = `1e8`;
chunk 1 is `-1e8 + 1` = `-1e8`. The tree adds them: **0**. The worker count
changes which worker computes each chunk, not what any chunk holds, so the
answer is 0 at 1, 2, 4 and 8 workers.

The tree is also more accurate than a long left-to-right sum. On the
harness's 16,777,216 values below, whose exact total is 8,388,607,288.03, the
tree gives 8,388,605,952, off by 1,336 where neighbouring `f32` values are
512 apart. One worker adding left to right gives 8,574,828,544, off by 2.2%.

A check compares `pc_tree` bit for bit with the same tree written as strides
over one array (neighbours, then pairs of pairs) for 1 to 69 chunks.

## A matrix-vector product and attention heads

A matrix-vector product has no merge at all if its chunks are runs of rows:
each row's dot product runs in column order inside one worker, and the
publish check is the whole merge.

<!-- source: psionic/lib/cores.coax matvec -->
```jai
// y = m x for a row-major m with x.count columns. Chunks are runs of rows;
// each row's dot product runs in column order, so no merge is needed.
pc_matvec_worker :: (p:PcPlan,w:int,m:[]f32,x:[]f32,y:[]f32,done:[]u8)->PcStatus #export_c {
    s:=pc_admit(p,w);if s!=PcStatus.ok {return s;}
    cols:=x.count;
    if cols<1 || m.count!=p.items*cols || y.count!=p.items || done.count!=p.chunks {return PcStatus.size;}
    for c:pc_first(p,w)..<pc_first(p,w+1) {
        for r:pc_lo(p,c)..<pc_hi(p,c) {
            t:f32=0;
            for j:0..<cols {t+=m[r*cols+j]*x[j];}
            y[r]=t;
        }
        done[c]=cast(u8)(w+1);
    }
    return PcStatus.ok;
}
```

Attention is the same with heads in place of rows. Each head takes one query
against `tokens` keys and values, laid out `[token][head][dim]`. The softmax
subtracts the first largest score, so `exp` never overflows, and adds each
token's weighted value in token order:

<!-- source: psionic/lib/cores.coax heads -->
```jai
// One query against tokens keys and values per head, laid out
// [token][head][dim]. Chunks are runs of heads. Softmax subtracts the
// largest score, then each token's weight multiplies its value in token order.
pc_heads_worker :: (p:PcPlan,w:int,q:[]f32,k:[]f32,v:[]f32,out:[]f32,done:[]u8,tokens:int)->PcStatus #export_c {
    s:=pc_admit(p,w);if s!=PcStatus.ok {return s;}
    heads:=p.items;dim:=q.count/heads;
    if tokens<1 || dim<1 || q.count!=heads*dim || k.count!=tokens*heads*dim || v.count!=k.count || out.count!=q.count || done.count!=p.chunks {return PcStatus.size;}
    for c:pc_first(p,w)..<pc_first(p,w+1) {
        for h:pc_lo(p,c)..<pc_hi(p,c) {
            top:=pc_score(q,k,h*dim,(pc_top(q,k,h,heads,dim,tokens)*heads+h)*dim,dim);
            for j:0..<dim {out[h*dim+j]=0;}
            total:f32=0;
            for t:0..<tokens {
                e:=cast(f32)exp(cast(f64)(pc_score(q,k,h*dim,(t*heads+h)*dim,dim)-top));
                total+=e;
                for j:0..<dim {out[h*dim+j]+=e*v[(t*heads+h)*dim+j];}
            }
            for j:0..<dim {out[h*dim+j]/=total;}
        }
        done[c]=cast(u8)(w+1);
    }
    return PcStatus.ok;
}
// The first token with the largest score for head h.
pc_top :: (q:[]f32,k:[]f32,h:int,heads:int,dim:int,tokens:int)->int {
    best:=0;top:=pc_score(q,k,h*dim,h*dim,dim);
    for t:1..<tokens {e:=pc_score(q,k,h*dim,(t*heads+h)*dim,dim);if e>top {best=t;top=e;}}
    return best;
}
pc_score :: (q:[]f32,k:[]f32,a:int,b:int,dim:int)->f32 {
    t:f32=0;
    for j:0..<dim {t+=q[a+j]*k[b+j];}
    return t;
}
```

The scores here are plain dot products; a model's `1 / sqrt(dim)` scale is
folded into its queries. A check runs scores of 200 and 100: without the
subtraction `exp(200)` is infinite in `f32` and the output is NaN, and with it
the output is the first token's value exactly.

## Run it

Coax has no threads yet, so the
[replay example](../examples/cores-replay.coax) calls the workers one after
another, last worker first, and compares every worker count with one worker.
The harness in the next section runs the same procedures on host threads.

<!-- source: psionic/examples/cores-replay.coax example -->
```jai
main :: () {
    p:=pc_plan(10,4,4);
    print("10 items in chunks of 4: % chunks; over 4 workers the runs start at % % % % and end at %\n",
        p.chunks,pc_first(p,0),pc_first(p,1),pc_first(p,2),pc_first(p,3),pc_first(p,4));
    print("chunk owners:");for c:0..<p.chunks {print(" %",pc_owner(p,c));}print("\n");

    x:=[4]f32.[100000000,1,-100000000,1];part:[2]f32;done:[2]u8;
    print("split by workers, workers -> sum: 1 -> %, 2 -> %, 4 -> %\n",pc_sum_split(x[..],1),pc_sum_split(x[..],2),pc_sum_split(x[..],4));
    print("fixed chunks of 2, workers -> sum:");
    for workers:[4]int.[1,2,4,8] {
        q:=pc_plan(4,2,workers);done[0]=0;done[1]=0;
        for w:0..<workers {#assert pc_sum_worker(q,workers-1-w,x[..],part[..],done[..])==PcStatus.ok;}
        print(" % -> %",workers,pc_sum_merge(q,part[..],done[..]).value);
    }
    print("\n");

    // 4,096 values, a 64 x 64 matrix, 8 heads of width 16 over 32 tokens.
    big:[4096]f32;for i:0..<big.count {big[i]=pc_value(i,1)+cast(f32)(1000*(i%2));}
    m:[4096]f32;for i:0..<m.count {m[i]=pc_value(i,2);}
    q:[128]f32;for i:0..<q.count {q[i]=pc_value(i,3)/1000;}
    k:[4096]f32;v:[4096]f32;for i:0..<k.count {k[i]=pc_value(i,4)/1000;v[i]=pc_value(i,5);}
    sums:[64]f32;marks:[64]u8;y:[64]f32;out:[128]f32;
    for workers:[4]int.[1,2,4,8] {
        s:=pc_plan(4096,64,workers);for c:0..<64 {marks[c]=0;}
        for w:0..<workers {#assert pc_sum_worker(s,workers-1-w,big[..],sums[..],marks[..])==PcStatus.ok;}
        total:=pc_sum_merge(s,sums[..],marks[..]);#assert total.status==PcStatus.ok;
        r:=pc_plan(64,8,workers);for c:0..<8 {marks[c]=0;}
        for w:0..<workers {#assert pc_matvec_worker(r,workers-1-w,m[..],big[0..<64],y[..],marks[0..<8])==PcStatus.ok;}
        #assert pc_publish(r,marks[0..<8])==PcStatus.ok;
        h:=pc_plan(8,1,workers);for c:0..<8 {marks[c]=0;}
        for w:0..<workers {#assert pc_heads_worker(h,workers-1-w,q[..],k[..],v[..],out[..],marks[0..<8],32)==PcStatus.ok;}
        #assert pc_publish(h,marks[0..<8])==PcStatus.ok;
        print("workers %: sum ",workers);hex(f32_bits(total.value));
        print(", matvec ");hex(digest(y[..]));print(", heads ");hex(digest(out[..]));
        print(", split sum ");hex(f32_bits(pc_sum_split(big[..],workers)));print("\n");
    }
}
```

```shell
coax/bin/coax run psionic/examples/cores-replay.coax
```

```text
10 items in chunks of 4: 3 chunks; over 4 workers the runs start at 0 0 1 2 and end at 3
chunk owners: 1 2 3
split by workers, workers -> sum: 1 -> 1.0, 2 -> 0.0, 4 -> 1.0
fixed chunks of 2, workers -> sum: 1 -> 0.0 2 -> 0.0 4 -> 0.0 8 -> 0.0
workers 1: sum 0x49fa613e, matvec 0x91e6ccc2, heads 0x484b47d4, split sum 0x49fa613c
workers 2: sum 0x49fa613e, matvec 0x91e6ccc2, heads 0x484b47d4, split sum 0x49fa613d
workers 4: sum 0x49fa613e, matvec 0x91e6ccc2, heads 0x484b47d4, split sum 0x49fa6140
workers 8: sum 0x49fa613e, matvec 0x91e6ccc2, heads 0x484b47d4, split sum 0x49fa613e
```

The third and fourth lines are the hand traces above. In the last four lines
the sum's bits, the matrix-vector product's digest and the attention output's
digest are the same at every worker count, while the split-by-workers sum
gives four different `f32` values, from `0x49fa613c` to `0x49fa6140`.

## Refuse bad plans and incomplete runs

A plan refuses a worker count outside 1 to 64, a chunk outside 1 to the item
count, more than 65,536 chunks, and an item count outside 1 to 2^30. A worker
refuses an index outside its plan and a buffer whose length disagrees with
the plan. The merge publishes nothing unless every chunk was marked by its
owner, so a worker that fails or is cancelled leaves its chunks unmarked and
the caller sees `incomplete`. The
[refusal example](../examples/cores-refuse.coax) shows each refusal, then
cancels worker 2 of 4:

<!-- source: psionic/examples/cores-refuse.coax refuse -->
```jai
main :: () {
    say("0 workers",pc_plan(4096,64,0).status);
    say("65 workers",pc_plan(4096,64,65).status);
    say("a chunk of 0",pc_plan(4096,0,8).status);
    say("a chunk of 4097 for 4096 items",pc_plan(4096,4097,8).status);
    say("chunks of 1 for 65537 items",pc_plan(65537,1,8).status);
    say("0 items",pc_plan(0,1,1).status);
    x:[4096]f32;part:[64]f32;done:[64]u8;
    p:=pc_plan(4096,64,4);
    say("worker 4 of 4",pc_sum_worker(p,4,x[..],part[..],done[..]));
    say("63 partial slots for 64 chunks",pc_sum_worker(p,0,x[..],part[0..<63],done[..]));
    // Worker 2 is cancelled: it never runs, so chunks 32 to 47 stay unmarked.
    for w:[3]int.[0,1,3] {#assert pc_sum_worker(p,w,x[..],part[..],done[..])==PcStatus.ok;}
    v:=pc_sum_merge(p,part[..],done[..]);
    say("worker 2 cancelled",v.status);
    print("published value: %\n",v.value);
    #assert pc_sum_worker(p,2,x[..],part[..],done[..])==PcStatus.ok;
    say("after worker 2 runs",pc_sum_merge(p,part[..],done[..]).status);
}
```

```text
0 workers: workers: outside 1..64
65 workers: workers: outside 1..64
a chunk of 0: chunk: size outside 1..items or more than 65536 chunks
a chunk of 4097 for 4096 items: chunk: size outside 1..items or more than 65536 chunks
chunks of 1 for 65537 items: chunk: size outside 1..items or more than 65536 chunks
0 items: shape: items outside 1..2^30
worker 4 of 4: worker: index outside the plan's workers
63 partial slots for 64 chunks: size: a buffer's length disagrees with the plan
worker 2 cancelled: incomplete: a chunk was not published by its owner
published value: 0.0
after worker 2 runs: ok
```

A chunk marked by the wrong worker is refused the same way; the checks cover
both.

## Run it on host threads

The [harness](../platform/cores.c) keeps one thread per worker waiting on a
barrier. For each run it hands every thread the same job, releases them, and
waits on a second barrier for all of them. Each thread calls the Coax worker
for its own index; the main thread then calls `pc_sum_merge` or
`pc_publish`. The harness:

- runs a sum over 16,777,216 values in 256 chunks, a 4,096 × 4,096
  matrix-vector product in 64 chunks of 64 rows, and 32 attention heads of
  width 128 over 2,048 tokens, one head per chunk;
- compares every run's output bytes at 2, 4 and 8 workers with the
  one-worker bytes and exits with status 1 on any difference;
- runs worker 2 of 4 as cancelled (it returns without computing) and exits
  with status 1 unless the merge refuses with `incomplete`;
- first runs small shapes under AddressSanitizer with
  UndefinedBehaviorSanitizer, and under ThreadSanitizer, which reports a data
  race if two threads touch the same slot.

```shell
psionic/tests/cores.sh --record
```

The [receipt for coderos-4080](../fixtures/cores/coderos-4080.json), an
Intel Core i7-14700K (28 hardware threads) under Linux 6.18 with GCC 15.2 at
`-O2`, recorded 2026-11-01 at a load average of 6.6, best of 7 runs after one
warm-up:

| Kernel | 1 worker | 2 workers | 4 workers | 8 workers |
| --- | --- | --- | --- | --- |
| Sum, 16,777,216 values | 7.24 ms | 4.18 ms (1.73×) | 2.80 ms (2.58×) | 1.73 ms (4.18×) |
| Matrix-vector, 4,096 × 4,096 | 7.41 ms | 3.77 ms (1.97×) | 2.14 ms (3.46×) | 1.97 ms (3.76×) |
| Attention, 32 heads | 23.37 ms | 15.54 ms (1.50×) | 8.62 ms (2.71×) | 5.45 ms (4.29×) |

Every kernel's digest is the same in all four columns, and the receipt's
`split_sum_bits` shows the split-by-workers sum giving four different values.

The sum and the matrix-vector product each read 64 MiB per run. One thread
reaches about 23 GB/s on this host ([chapter 33](ch033-measure-the-work-and-plan-the-memory.md)'s
triad), so they are bound by memory, and the matrix-vector product gains
little from 4 to 8 workers. Attention reads 64 MiB of keys and values too but
does more arithmetic per byte, and keeps scaling at 8 workers.

The numbers are not stable. The harness does not pin threads to cores, and
the i7-14700K mixes 8 performance cores (16 hardware threads) with 12
efficiency cores, so a worker's speed depends on where the scheduler puts it.
A run a minute earlier, at a load average of 7.0, measured 8-worker
speedups of 4.37×, 5.27× and 4.27×. That is why no measured number appears in
an exact transcript, and why the receipt keeps the best, median and worst runs
and names the machine, compiler, flags and load.

## Check it

```shell
coax/bin/coax test psionic/lib/cores.coax
coax/bin/coax mutate psionic/lib/cores.coax
coax/bin/coax run psionic/examples/cores-replay.coax
coax/bin/coax run psionic/examples/cores-refuse.coax
psionic/tests/cores.sh
```

The independent checks: a `#forall` runs the sum, the matrix-vector product
and the attention heads for 1 to 96 items and chunk sizes 1 to 8 at 1, 2, 4
and 8 workers, workers called last first, and compares every output bit for
bit with one worker; the owner formula is compared with a brute-force search;
the recursive tree is compared with the strided tree; and the expected values
in the hand traces are written out above. `coax mutate` leaves no survivors
in `cores.coax` (231 mutants).

## What is not built yet

- **Chapter 14's Elephant decision on the pool.** The kernels here have the
  shapes of a decision's products and attention, but the decision path itself
  still runs on one core; running its BF16 matrix-vector products and
  attention through this partition, with the same byte check against the
  one-worker decision, is next.
- **The same gate on Coax jobs.** When Coax has jobs and a memory model
  ([Coax chapter 14](../../coax/book/ch14-jobs.md)), the host pool is replaced
  and this chapter's checks must pass again unchanged.
- **A quiet-host repeat.** The receipt above is one run on a shared machine.
  The same runs on a quiet host, with threads pinned and the spread stated,
  are still open.

## Sources

- [Chapter plan](PLAN.md), Part V and the prerequisite map ("Decision
  throughput").
- [Coax specification](../../coax/spec.md), section 12: deterministic
  reductions combine in a specified tree order and are tested at 1, 2, 4 and
  8 workers.
- [Coax roadmap](../../coax/ROADMAP.md).
