# 171. Decode in logarithmic time

[Chapter 166](ch166-attention-as-exact-memory.md) gave the machine a memory
made of attention. Each store in the trace is a key at the point
`(2k, -k²)`, a read of address `q` is the direction `(q, 1)`, and the read
returns the store with the highest dot product. That is exact, but every read
compares every store. A run that has made `n` stores pays `n - 1`
comparisons per read, 64 reads per step. The longer the program runs, the
slower each step gets.

This chapter replaces the scan with a lookup that costs about log₂ of the
number of distinct addresses. It returns exactly what the scan returns:
the same status, value and write position.

```shell
coax/bin/coax run psionic/examples/hull-decode.coax
coax/bin/coax test psionic/lib/hull-memory.coax
python3 -I psionic/tests/hull_oracle.py
```

```jai
squares: 53 steps, output 17, the interpreter's digest: true
  4 stores, 3392 reads; comparisons: scan 4608, hull 3121
fill 64 cells 8 times: 4678 steps, output 1, the interpreter's digest: true
  512 stores, 299392 reads; comparisons: scan 76362048, hull 1753004
one read after 512 writes to 64 keys: write 511, scan 511 comparisons, hull 6
```

The [example](../examples/hull-decode.coax) runs two programs on chapter
165's machine with the hull memory, and checks each digest against chapter
165's interpreter. For the second program, the hull makes about 44 times
fewer comparisons than the scan, and the gap grows with every store.

## Why a hull

The best key for a direction is always a vertex of the keys' upper convex
hull, and along that hull the score rises to a single peak and then falls.
So the peak can be found by binary search: compare a vertex with its right
neighbour, and keep the half that still rises.

Chapter 166's keys make this simple, because every point `(2k, -k²)` lies on
one parabola. A parabola curves the same way everywhere, so no key is ever
inside the hull: every distinct key is a vertex, and the hull in key order is
just the sorted keys. Adding a key never removes another.

Two details decide whether the lookup is *exactly* the scan:
- **Repeated keys.** The scan returns the latest of equal-scoring stores.
  Stores to the same address score the same for every query, so the index
  keeps one entry per address, holding its latest write.
- **Ties between different keys.** Two keys tie only when the query lies
  exactly midway between them. Then neither key is the query, and the scan
  refuses the read as missing whichever one it picked. So which tied vertex
  the search stops on never changes a result.

## Inserting a store

<!-- source: psionic/lib/hull-memory.coax insert -->
```jai
// Records write number x.writes, key k, value v. Finding the key's place is
// a binary search; a key already present keeps its place and takes the
// later write, because a later write wins every tie the scan would see.
hm_insert :: (x:HmIndex,k:int,v:int)->HmInsert {
    r:=HmInsert.{index=x};
    if k< -EM_KEY_MAX || k>EM_KEY_MAX {r.status=EmStatus.range;return r;}
    lo:=0;hi:=x.count;
    while lo<hi {
        mid:=(lo+hi)/2;r.compares+=1;
        if x.keys[mid]<k {lo=mid+1;} else {hi=mid;}
    }
    if lo==x.count || x.keys[lo]!=k {
        if x.count>=x.keys.count || x.count>=x.values.count || x.count>=x.positions.count {r.status=EmStatus.capacity;return r;}
        i:=x.count;
        while i>lo {x.keys[i]=x.keys[i-1];x.values[i]=x.values[i-1];x.positions[i]=x.positions[i-1];i-=1;}
        x.keys[lo]=k;r.index.count+=1;
    }
    x.values[lo]=v;x.positions[lo]=x.writes;r.index.writes+=1;
    return r;
}
```

A binary search finds the key's place. A new key is shifted in. A repeated
key takes the new value and write number in place.

The shift moves up to the whole index. Here that is at most 64 entries,
because the machine has 64 cells. An index for wider keys would use a
balanced tree to keep insertion logarithmic too. The comparisons counted
here are only the search's.

## Reading

<!-- source: psionic/lib/hull-memory.coax read -->
```jai
// The hard max over the hull: em_score(keys[i],q) rises to one peak along
// the sorted keys and then falls, so comparing each midpoint with its right
// neighbour finds the peak. Two different keys tie only when q lies midway
// between them, where neither is q and the read is refused anyway; so the
// result, value and position, is the scan's.
hm_read :: (x:HmIndex,q:int)->HmRead {
    r:HmRead;
    if q< -EM_KEY_MAX || q>EM_KEY_MAX {r.read.status=EmStatus.range;return r;}
    if x.count==0 {r.read.status=EmStatus.missing;return r;}
    lo:=0;hi:=x.count-1;
    while lo<hi {
        mid:=(lo+hi)/2;r.compares+=1;
        if em_score(x.keys[mid+1],q)>em_score(x.keys[mid],q) {lo=mid+1;} else {hi=mid;}
    }
    r.vertex=lo;
    if x.keys[lo]!=q {r.read.status=EmStatus.missing;return r;}
    r.read.value=x.values[lo];r.read.position=x.positions[lo];
    return r;
}
```

The search compares scores, the same dot products the scan compares, so
the two methods' counts measure the same operation.

## The machine with a hull memory

<!-- source: psionic/lib/hull-memory.coax run -->
```jai
// Chapter 166's machine with the hull index in place of the scan: before
// every step, each of the 64 cells is read through the hull. scan counts
// the comparisons chapter 166's read makes over the same stores (one fewer
// than the number of stores, per read); hull counts this chapter's.
hm_run :: (program:[]TmOp,budget:int,trace:[]int,index:HmIndex)->HmRun {
    out:HmRun;x:=index;x.count=0;
    admitted:=tm_admit(program);if admitted!=TmStatus.ok {out.result.status=admitted;return out;}
    if budget<1 || budget>TM_STEPS_MAX {out.result.status=TmStatus.budget;return out;}
    s:TmState;ip:=0;row:[4]int;bytes:[16]u8;sha:[112]u8;
    #assert sha256_init(sha[..])==SHA_OK;
    while true {
        if ip==program.count {out.result=TmResult.{status=TmStatus.fell,ip=ip,steps=out.result.steps};return out;}
        if out.result.steps>=budget {out.result=TmResult.{status=TmStatus.budget,ip=ip,steps=out.result.steps};return out;}
        view:[64]int;
        for c:0..<TM_CELLS {
            h:=hm_read(x,c);view[c]=h.read.value;
            out.reads+=1;out.hull+=h.compares;
            if out.stores!=0 {out.scan+=out.stores-1;}
        }
        t:=tm_step(program,ip,s,view[..],row[..]);
        if t.status!=TmStatus.ok {out.result=TmResult.{status=t.status,ip=ip,steps=out.result.steps};return out;}
        if program[ip].code==TmCode.store {
            w:=hm_insert(x,row[3],row[2]);
            if w.status!=EmStatus.ok {out.result=TmResult.{status=TmStatus.capacity,ip=ip,steps=out.result.steps};return out;}
            x=w.index;out.stores+=1;out.hull+=w.compares;
        }
        n:=out.result.steps;
        if (n+1)*TM_TOKENS>trace.count {out.result=TmResult.{status=TmStatus.capacity,ip=ip,steps=n};return out;}
        for k:0..<TM_TOKENS {trace[n*TM_TOKENS+k]=row[k];}
        tm_bytes(row[..],bytes[..]);#assert sha256_update_checked(sha[..],bytes[..],false)==SHA_OK;
        s=t.state;out.result.steps=n+1;
        if t.next<0 {break;}
        ip=t.next;
    }
    out.result.ip=ip;out.result.outputs=s.outputs;
    #assert sha256_final_checked(sha[..],out.result.digest[..],false)==SHA_OK;
    return out;
}
```

`hm_run` is chapter 166's machine with the hull index in place of the scan.
It also counts the comparisons chapter 166's read would have made over the
same stores. Both programs give chapter 165's digest, row for row.

| Program | Stores | Reads | Scan comparisons | Hull comparisons |
| --- | ---: | ---: | ---: | ---: |
| squares (53 steps) | 4 | 3,392 | 4,608 | 3,121 |
| fill 64 cells 8 times (4,678 steps) | 512 | 299,392 | 76,362,048 | 1,753,004 |

With 4 stores there's little to gain. The hull's count also includes its
inserts, and a 4-key search still takes 2 comparisons. With 512 stores, one
read takes 6 comparisons instead of 511. The scan's cost per read grows with
the number of stores. The hull's grows with the logarithm of the number of
distinct addresses, here at most 6.

## No wall-clock figures

These are comparison counts, not timings. A CPU running this index says
nothing about how fast a model's weights would run the same lookup. Timings
for Part XXIII come only from running the weights, which
[chapter 170](PLAN.md) builds. Until then, this chapter claims only what
it measures: the same results with fewer comparisons.

## Supported profile

`hull-v1`:
- chapter 166's keys, scores and range of ±2²⁰;
- one index entry per distinct key, sorted, holding its latest write;
- the caller's storage, refused as `capacity` when a new key doesn't fit;
- reads and inserts counted in score and key comparisons.

## How it is checked

- **Coax checks:** this chapter's 11 checks, with the 153 from chapters 165 and 166 that `hull-memory.coax` loads, all 164 passing in debug and release. They cover:
  - inserts in order, out of order and repeated, at the key bounds and at
    capacity (including each of the three storage arrays);
  - comparison counts for small indexes and a full one of 64 keys;
  - after every write of three generated sequences, every read in a range
    around the keys equals `em_read`'s status, value and position, including
    wide keys and keys at ±2²⁰;
  - `hm_run` against chapter 165's interpreter, trace and digest, and each
    of its refusals.
- **An independent oracle,** [hull_oracle.py](../tests/hull_oracle.py), runs
  both programs on its own machine. It checks every one of the 302,784 hull
  reads against a brute-force scan, and prints the example's lines, byte for
  byte.
- **Mutation testing** of `hull-memory.coax`: 1,433 of 1,433 mutants killed.

The [verification record](../../docs/verification/psionic-ch171-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Keys on a parabola only.** Keys in general position need a hull that
  drops points falling inside it, and handles points on its edges for
  latest-write ties. Chapter 166's keys never need either.
- **No timings.** See above.
- **Not in weights.** The lookup runs as code beside the machine. Building
  it into weights is the work of chapter 170's interpreter.
