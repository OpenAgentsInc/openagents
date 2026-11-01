# 121. Feed the accelerator

> **Status: partly implemented.** Packed text batches, worker threads, a bounded prefetch ring, resume and the stall model run on the CPU. Image pipelines, padding with masks, per-example transforms and pinned-memory transfers to a GPU do not exist yet.

A device that waits for data does no work. While it runs step *n*, the next
batch should already be loading, so that when step *n* ends the batch for
*n* + 1 is ready. This chapter builds that loader for
[chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md)'s packed
token stream over [chapter 120](ch120-data-and-resumable-state.md)'s leak-free
split, and it keeps one promise: whatever the number of worker threads, the
number of prefetched batches, and the order in which threads happen to run,
the batches that come out are byte for byte the batches that one thread
calling `pk_next` row after row would make. A run stopped and resumed
continues with exactly the batch the uninterrupted run would take next.

The pieces:

- a plan: rows per batch, tokens per row, workers, and ring depth, with the
  ring's memory checked against a stated budget;
- row cursors found by seeking through document lengths, so rows can be
  filled independently;
- workers that fill only their own rows, using
  [chapter 35](ch035-use-every-core.md)'s fixed partition, and a batch that
  is published only when every row is marked by its owner;
- a ring of `depth` slots with backpressure, whose consumer cursor is the
  resume point;
- a stall model that says how long the device waits, checked by hand and
  then compared with a measured run on this host.

The profile is `psionic-feed-v1`: CPU only, at most 64 rows of 2 to 256
tokens, rings of 1 to 8 batches (at most 128 KiB), and chapter 122's
`psionic-pack-v1` stream. There is no GPU in this chapter. The "device" in the
measured run is one CPU thread doing a fixed matrix-vector product per batch.

```shell
coax/bin/coax test psionic/lib/data/feed.coax
coax/bin/coax run psionic/examples/feed.coax
coax/bin/coax run psionic/examples/feed-refuse.coax
psionic/tests/feed.sh
```

## Dependencies

- [Chapter 120](ch120-data-and-resumable-state.md): the grouped split that
  becomes packed documents (`sp_docs`).
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the packed
  stream (`pk_next`), its cursor, and the checkpoint that stores the cursor.
- [Chapter 35](ch035-use-every-core.md): fixed partitions (`pc_plan`,
  `pc_first`) and the publish check (`pc_publish`), and its host-thread
  harness pattern.
- Coax has no threads yet. The native harness owns the threads, the lock and
  the clock, and calls Coax for every decision.

## The plan

A batch is `rows` packed rows of `length` tokens. The rows are cut among
workers by chapter 35's partition with one row per chunk, so worker `w` owns
rows `pc_first(w)` up to `pc_first(w + 1)`. The ring holds `depth` batches,
which is all the memory the loader ever holds: `depth * rows * length` bytes.
The plan refuses a ring that does not fit the budget the caller states.

<!-- source: psionic/lib/data/feed.coax plan -->
```jai
// A batch is rows packed rows of length tokens. Rows are cut among workers by
// chapter 35's partition, one row per chunk. The ring holds depth batches,
// depth*rows*length bytes, and must fit the stated budget.
fd_plan :: (rows:int,length:int,workers:int,depth:int,budget:int)->FdPlan #export_c {
    if rows<1 || rows>FD_ROWS {return FdPlan.{status=FdStatus.rows};}
    if length<2 || length>PK_ROW {return FdPlan.{status=FdStatus.length};}
    cores:=pc_plan(rows,1,workers);if cores.status!=PcStatus.ok {return FdPlan.{status=FdStatus.workers};}
    if depth<1 || depth>FD_DEPTH {return FdPlan.{status=FdStatus.depth};}
    bytes:=depth*rows*length;if bytes>budget {return FdPlan.{status=FdStatus.budget};}
    return FdPlan.{rows=rows,length=length,workers=workers,depth=depth,bytes=bytes,cores=cores};
}
```

The example uses 2 rows of 8 tokens and 4 slots: `4 * 2 * 8 = 64` bytes. The
largest plan, 8 slots of 64 rows of 256 tokens, is 131,072 bytes.

## Where each row starts

`pk_next` fills a row from a cursor and returns the cursor after it. Called
row after row, each call needs the previous one's result, so the rows cannot
be filled at the same time. But where a row starts depends only on how many
positions came before it, and that can be found without reading any token: a
training document fills `count + 1` positions in the stream, its boundary
token (15) and then its tokens. `fd_seek` walks whole documents until fewer
positions remain than the current document has left.

<!-- source: psionic/lib/data/feed.coax seek -->
```jai
// The cursor n positions further along the packed stream, found from document
// lengths alone: a training document fills count + 1 positions, its BOS and
// its tokens. Reads no token. The cursor must be admitted.
fd_seek :: (docs:[]PkDoc,cursor:PkCursor,n:int)->FdNext {
    order:[64]int;pk_order(docs,cursor.seed,cursor.epoch,order[..]);trains:=pk_trains(docs);c:=cursor;left:=n;
    while left>0 {
        rest:=docs[order[c.order]].count+1-c.offset;
        if left<rest {c.offset+=left;left=0;}
        else {
            left-=rest;c.offset=0;c.order+=1;
            if c.order==trains {c.order=0;c.epoch+=1;if c.epoch==PK_EPOCHS {return FdNext.{status=FdStatus.cursor,cursor=cursor};}pk_order(docs,c.seed,c.epoch,order[..]);}
        }
    }
    return FdNext.{cursor=c};
}
// Row r of a batch starts at cursors[r]; cursors[rows] is where the next
// batch starts. Nothing is written unless the whole batch is in the stream.
fd_cursors :: (docs:[]PkDoc,tokens:[]u8,p:FdPlan,start:PkCursor,cursors:[]PkCursor)->FdNext #export_c {
    if p.status!=FdStatus.ok {return FdNext.{status=p.status,cursor=start};}
    if cursors.count!=p.rows+1 {return FdNext.{status=FdStatus.buffer,cursor=start};}
    if pk_dataset(docs,tokens)!=PkStatus.ok {return FdNext.{status=FdStatus.dataset,cursor=start};}
    if !pk_cursor(docs,start) {return FdNext.{status=FdStatus.cursor,cursor=start};}
    stage:[65]PkCursor;stage[0]=start;
    for r:0..<p.rows {s:=fd_seek(docs,stage[r],p.length);if s.status!=FdStatus.ok {return FdNext.{status=s.status,cursor=start};}stage[r+1]=s.cursor;}
    for r:0..<p.rows+1 {cursors[r]=stage[r];}
    return FdNext.{cursor=stage[p.rows]};
}
```

By hand, on chapter 120's grouped split. Its four training documents are
`11 10 9 8`, `6 5 4`, `13 12 1 0` and `14 0 13 1`, so one epoch is
5 + 4 + 5 + 5 = 19 positions. With seed 3, epoch 0's order is
`14 0 13 1`, `13 12 1 0`, `6 5 4`, `11 10 9 8`, which is what the example's
first batch shows:

```text
positions 0..7    15 14 0 13 1 | 15 13 12        row 0
positions 8..15   1 0 | 15 6 5 4 | 15 11         row 1
```

Row 1's cursor, 8 positions from the start (epoch 0, order 0, offset 0):

```text
order 0: 5 positions left; 8 >= 5, so 3 remain     -> order 1, offset 0
order 1: 5 positions left; 3 < 5                    -> order 1, offset 3
```

Offset 3 of `13 12 1 0` is past its boundary token and two tokens, so row 1
begins with `1 0`, as shown. The next batch's cursor, 8 more positions:

```text
order 1 from offset 3: 2 left; 8 >= 2, so 6 remain  -> order 2, offset 0
order 2: 4 left; 6 >= 4, so 2 remain                -> order 3, offset 0
order 3: 5 left; 2 < 5                              -> order 3, offset 2
```

Row 1 ends with `15 11`, the boundary and first token of order 3, so the next
batch starts at offset 2. `fd_cursors` writes these cursors for a whole batch,
plus the cursor after it, and writes nothing when the batch would run past the
stream's last epoch.

## Fill the rows

Each worker fills its own rows with `pk_next` from each row's cursor and
checks that the row ends where the next row's cursor says it should. That
check ties the seek arithmetic to the real stream on every row. A row is
marked `w + 1`, and a batch is published only when every row carries its
owner's mark, so a worker that failed or was cancelled leaves the batch
unpublished rather than half filled.

<!-- source: psionic/lib/data/feed.coax worker -->
```jai
// Worker w fills each row it owns with pk_next from that row's cursor,
// checks that the row ends where the next one starts, and marks it w + 1.
// It touches no other row.
fd_worker :: (docs:[]PkDoc,tokens:[]u8,p:FdPlan,w:int,cursors:[]PkCursor,batch:[]u8,done:[]u8)->FdStatus #export_c {
    if p.status!=FdStatus.ok {return p.status;}
    if w<0 || w>=p.workers {return FdStatus.worker;}
    if cursors.count!=p.rows+1 || batch.count!=p.rows*p.length || done.count!=p.rows {return FdStatus.buffer;}
    for r:pc_first(p.cores,w)..<pc_first(p.cores,w+1) {
        n:=pk_next(docs,tokens,cursors[r],batch[r*p.length..<(r+1)*p.length],false);
        if n.status==PkStatus.cursor {return FdStatus.cursor;}
        if n.status!=PkStatus.ok {return FdStatus.dataset;}
        if !fd_same(n.cursor,cursors[r+1]) {return FdStatus.cursor;}
        done[r]=cast(u8)(w+1);
    }
    return FdStatus.ok;
}
// A batch is published only when every row is marked by its owner.
fd_publish :: (p:FdPlan,done:[]u8)->FdStatus #export_c {
    if p.status!=FdStatus.ok {return p.status;}
    s:=pc_publish(p.cores,done);
    if s==PcStatus.size {return FdStatus.buffer;}
    if s!=PcStatus.ok {return FdStatus.incomplete;}
    return FdStatus.ok;
}
```

Because a row's bytes depend only on its cursor, and each row is written by
exactly one worker, the batch does not depend on the worker count or on the
order in which workers run.

## The ring and backpressure

Filled batches wait in a ring of `depth` slots. The producer asks `fd_free`
for a slot, fills it, and commits it with the cursor after the batch; the
consumer asks `fd_ready` for a slot, reads it, and releases it with
`fd_consume`. `fd_free` returns no slot when the producer is already `depth`
batches ahead, so a fast loader waits instead of growing: backpressure. The
ring never holds more than the plan's `bytes`.

<!-- source: psionic/lib/data/feed.coax ring -->
```jai
// The producer fills slot fd_free and commits it with the cursor after the
// batch; the consumer reads slot fd_ready and releases it. The producer may
// be at most depth batches ahead, so a fast loader waits instead of growing.
// resume is the cursor after the last batch the consumer took.
fd_ring :: (p:FdPlan,start:PkCursor)->FdTurn #export_c {
    if p.status!=FdStatus.ok {return FdTurn.{status=p.status,slot=-1};}
    return FdTurn.{slot=-1,ring=FdRing.{depth=p.depth,resume=start}};
}
fd_free :: (r:FdRing)->int #export_c {
    if r.depth<1 || r.depth>FD_DEPTH || r.produced-r.consumed>=r.depth {return -1;}
    return r.produced%r.depth;
}
fd_ready :: (r:FdRing)->int #export_c {
    if r.depth<1 || r.depth>FD_DEPTH || r.consumed==r.produced {return -1;}
    return r.consumed%r.depth;
}
fd_produce :: (r:FdRing,after:PkCursor)->FdTurn #export_c {
    if r.depth<1 || r.depth>FD_DEPTH {return FdTurn.{status=FdStatus.depth,slot=-1,ring=r};}
    slot:=fd_free(r);if slot<0 {return FdTurn.{status=FdStatus.full,slot=-1,ring=r};}
    q:=r;q.after[slot]=after;q.produced+=1;return FdTurn.{slot=slot,ring=q};
}
fd_consume :: (r:FdRing)->FdTurn #export_c {
    if r.depth<1 || r.depth>FD_DEPTH {return FdTurn.{status=FdStatus.depth,slot=-1,ring=r};}
    slot:=fd_ready(r);if slot<0 {return FdTurn.{status=FdStatus.empty,slot=-1,ring=r};}
    q:=r;q.resume=r.after[slot];q.consumed+=1;return FdTurn.{slot=slot,ring=q};
}
```

The ring keeps two positions in the stream: where the producer will start its
next batch, and `resume`, the cursor after the last batch the consumer took.
Only `resume` may go into a checkpoint (chapter 122's `RsRun.cursor`). Batches
that were prefetched but not taken are thrown away when the run stops and are
filled again after the resume.

By hand, with 4 slots, three batches asked for, and a producer that fills
whenever a slot is free:

```text
turn  action        produced  consumed  slots in use
1-4   fill 0..3     4         0         4 (full)
5     take 0        4         1         3    resume = after batch 0
6     fill 4        5         1         4
7     take 1        5         2         3    resume = after batch 1
8     fill 5        6         2         4
9     take 2        6         3         3    resume = after batch 2: stop
```

The producer stopped after batch 5, which ends 6 × 16 = 96 positions in:
96 = 5 × 19 + 1, epoch 5, order 0, offset 1. The consumer took three batches,
48 positions: 48 = 2 × 19 + 10, which is epoch 2 after its first two
documents (5 + 5 positions), order 2, offset 0. Resuming from the producer's
cursor would skip batches 3, 4 and 5.

## One thread playing every part

`fd_drive` runs a producer, its workers and a consumer on one thread. A
64-bit pattern picks, turn by turn, whether to fill a batch (when a slot is
free) or take one; with nothing ready it must fill. Workers are called last to
first. The checks run it under many patterns, worker counts and depths and
compare the bytes with an independent stream.

<!-- source: psionic/lib/data/feed.coax drive -->
```jai
// One thread playing producer, workers and consumer. Turn k fills a batch
// (its workers called last to first) when bit k % 64 of pattern is set and
// a slot is free, and takes one otherwise; with nothing ready it must fill.
// It stops once steps batches are taken; the producer may have filled up to
// depth more. The batches taken, in order, go to out. The threaded harness
// must give the same bytes.
fd_drive :: (docs:[]PkDoc,tokens:[]u8,p:FdPlan,start:PkCursor,steps:int,pattern:u64,out:[]u8)->FdTurn {
    t:=fd_ring(p,start);if t.status!=FdStatus.ok {return t;}
    if out.count!=steps*p.rows*p.length {return FdTurn.{status=FdStatus.buffer,slot=-1,ring=t.ring};}
    r:=t.ring;slots:[131072]u8;next:=start;ended:=false;turn:u64=0;size:=p.rows*p.length;
    while r.consumed<steps {
        fill:=((pattern>>(turn&63))&1)==1;turn+=1;
        if ended || fd_free(r)<0 {fill=false;}
        if r.consumed==r.produced {fill=true;}
        if fill {
            slot:=fd_free(r);cur:[65]PkCursor;done:[64]u8;
            n:=fd_cursors(docs,tokens,p,next,cur[0..<p.rows+1]);
            if n.status!=FdStatus.ok {
                // Past the batches asked for, the end of the stream only stops prefetching.
                if r.produced<steps {return FdTurn.{status=n.status,slot=-1,ring=r};}
                ended=true;continue;
            }
            for k:0..<p.workers {
                #assert fd_worker(docs,tokens,p,p.workers-1-k,cur[0..<p.rows+1],slots[slot*size..<(slot+1)*size],done[0..<p.rows])==FdStatus.ok;
            }
            #assert fd_publish(p,done[0..<p.rows])==FdStatus.ok;
            next=n.cursor;r=fd_produce(r,next).ring;
        } else {
            slot:=fd_ready(r);at:=r.consumed*size;
            for i:0..<size {out[at+i]=slots[slot*size+i];}
            r=fd_consume(r).ring;
        }
    }
    return FdTurn.{slot=-1,ring=r};
}
```

## How long the device waits

Let batch *i* take `load[i]` to fill and `compute[i]` to use. With a ring of
`depth` slots, batch *i* can start loading once batch *i* − 1 is loaded and
its slot is free, which is when batch *i* − `depth`'s compute has ended.
Compute *i* starts once compute *i* − 1 has ended and batch *i* is loaded. The
device is idle whenever compute waits for a load.

With one slot, loading and compute take turns, and a step costs
`compute + load`. With two or more and steady times, the loader runs while the
device computes, and a step costs the larger of the two. By hand, with
compute 3 and load 5 for four steps:

```text
depth 1:  load 0-5, compute 5-8; load 8-13, compute 13-16; ...  total 4 × 8 = 32, idle 4 × 5 = 20
depth 2:  loads end at 5, 10, 15, 20 (the loader never waits)
          computes run 5-8, 10-13, 15-18, 20-23               total 5 + 3 + 3 × 5 = 23
          idle 5 + 2 + 2 + 2 = 11
```

Idle is 20/32 = 625 thousandths of the run with one slot, and 11/23 = 478
thousandths with two. More slots do not help with steady times: the loader is
the bottleneck, and depth 4 also gives 23. A deeper ring helps when load times
vary, because loads that ran ahead absorb a slow one. With compute 4 and loads
1, 1, 9, 1, two slots take 22 and three take 19.

<!-- source: psionic/lib/data/feed.coax overlap -->
```jai
// Device idle time for steps whose compute and load times are given, with a
// ring of depth slots. Batch i loads once batch i - 1 is loaded and its slot
// is free (batch i - depth's compute has ended); compute i starts once
// compute i - 1 has ended and batch i is loaded. Depth 1 runs them in turn.
fd_overlap :: (compute:[]int,load:[]int,depth:int)->FdTime #export_c {
    if compute.count<1 || compute.count!=load.count {return FdTime.{status=FdStatus.shape};}
    if depth<1 || depth>FD_DEPTH {return FdTime.{status=FdStatus.depth};}
    for i:0..<compute.count {if compute[i]<0 || load[i]<0 {return FdTime.{status=FdStatus.shape};}}
    ends:[8]int;loaded:=0;end:=0;idle:=0;
    for i:0..<compute.count {
        begin:=loaded;if i>=depth {begin=fd_max(begin,ends[(i-depth)%8]);}
        loaded=begin+load[i];
        start:=fd_max(end,loaded);idle+=start-end;
        end=start+compute[i];ends[i%8]=end;
    }
    return FdTime.{total=end,idle=idle};
}
// The steady per-step time: compute plus load with one slot, the larger of
// the two with two or more.
fd_steady :: (compute:int,load:int,depth:int)->int #export_c {
    if depth==1 {return compute+load;}
    return fd_max(compute,load);
}
fd_max :: (a:int,b:int)->int {return (a+b+cast(int)abs(f64(a-b)))/2;}
// Idle time as thousandths of the run.
fd_lost :: (t:FdTime)->int #export_c {if t.total<=0 {return 0;}return t.idle*1000/t.total;}
```

## Run the example

The example builds chapter 120's grouped split, makes the reference stream of
twelve rows of 8 with `pk_next` one row after another, and compares six
batches of 2 rows made at 1, 2, 4 and 8 workers, depths 1, 2 and 4, and three
patterns: take as soon as a batch is ready, fill whenever a slot is free, and
alternate. It then stops after three batches with 4 slots, resumes from each
of the two cursors, and prints the stall model's hand trace.

[Run the example](../examples/feed.coax):

<!-- source: psionic/examples/feed.coax example -->
```jai
main :: () {
    docs:[14]PkDoc;tokens:[51]u8;fd_split(docs[..],tokens[..]);start:=PkCursor.{seed=3};
    // The reference: twelve rows of 8 straight from pk_next, one after another.
    want:[96]u8;c:=start;
    for k:0..<12 {n:=pk_next(docs[..],tokens[..],c,want[k*8..<k*8+8],false);#assert n.status==PkStatus.ok;c=n.cursor;}
    print("batch 0: ");for i:0..<16 {print("% ",want[i]);}print("\n");
    // Six batches of 2 rows, at every worker count, ring depth and pattern:
    // take as soon as one is ready, fill whenever a slot is free, alternate.
    same:=0;runs:=0;
    for workers:[4]int.[1,2,4,8] {for depth:[3]int.[1,2,4] {for pattern:[3]u64.[0,0xffffffffffffffff,0x5555555555555555] {
        p:=fd_plan(2,8,workers,depth,64);#assert p.status==FdStatus.ok;
        got:[96]u8;d:=fd_drive(docs[..],tokens[..],p,start,6,pattern,got[..]);#assert d.status==FdStatus.ok;
        runs+=1;equal:=true;for i:0..<96 {if got[i]!=want[i] {equal=false;}}if equal {same+=1;}
    }}}
    print("% of % runs (workers 1, 2, 4, 8; depth 1, 2, 4; three patterns) equal the reference\n",same,runs);
    // Stop after three batches with four slots, filling whenever a slot is free.
    p:=fd_plan(2,8,4,4,64);print("ring of 4 batches of 2 x 8: % bytes\n",p.bytes);
    got:[96]u8;d:=fd_drive(docs[..],tokens[..],p,start,3,0xffffffffffffffff,got[0..<48]);r:=d.ring;
    ahead:=r.after[(r.produced-1)%r.depth];
    print("stopped: produced %, consumed %; producer at epoch % order % offset %, consumer at epoch % order % offset %\n",
        r.produced,r.consumed,ahead.epoch,ahead.order,ahead.offset,r.resume.epoch,r.resume.order,r.resume.offset);
    // Resuming from the producer's cursor skips the prefetched batches.
    for from:[2]PkCursor.[ahead,r.resume] {
        e:=fd_drive(docs[..],tokens[..],p,from,3,0,got[48..<96]);#assert e.status==FdStatus.ok;
        first:=-1;for i:0..<96 {if first<0 && got[i]!=want[i] {first=i;}}
        if first<0 {print("resume: all 96 tokens equal the reference\n");} else {print("resume: first difference at token % (batch %)\n",first,first/16);}
    }
    // The stall model's hand trace: compute 3, load 5, four steps.
    cs:=[4]int.[3,3,3,3];ls:=[4]int.[5,5,5,5];
    for depth:[3]int.[1,2,4] {t:=fd_overlap(cs[..],ls[..],depth);print("depth %: total % idle % (% per thousand), steady step %\n",depth,t.total,t.idle,fd_lost(t),fd_steady(3,5,depth));}
}
```

The [transcript](../fixtures/feed/feed.out) is exact in debug and release
builds:

```text
batch 0: 15 14 0 13 1 15 13 12 1 0 15 6 5 4 15 11 
36 of 36 runs (workers 1, 2, 4, 8; depth 1, 2, 4; three patterns) equal the reference
ring of 4 batches of 2 x 8: 64 bytes
stopped: produced 6, consumed 3; producer at epoch 5 order 0 offset 1, consumer at epoch 2 order 2 offset 0
resume: first difference at token 48 (batch 3)
resume: all 96 tokens equal the reference
depth 1: total 32 idle 20 (625 per thousand), steady step 8
depth 2: total 23 idle 11 (478 per thousand), steady step 5
depth 4: total 23 idle 11 (478 per thousand), steady step 5
```

## What the loader refuses

[The refusal example](../examples/feed-refuse.coax) asks for plans outside
the profile, buffers that disagree with the plan, a cursor past the end of
its document, a worker that does not exist, a batch whose second worker was
cancelled, and ring operations the ring cannot do:

<!-- source: psionic/examples/feed-refuse.coax refuse -->
```jai
main :: () {
    say("0 workers",fd_plan(2,8,0,2,64).status);
    say("65 workers",fd_plan(2,8,65,2,64).status);
    say("rows of 1 token",fd_plan(2,1,2,2,64).status);
    say("65 rows",fd_plan(65,8,2,2,FD_BYTES).status);
    say("a ring of 9",fd_plan(2,8,2,9,FD_BYTES).status);
    say("2 slots of 2 x 8 in 31 bytes",fd_plan(2,8,2,2,31).status);
    docs:[14]PkDoc;tokens:[51]u8;fd_split(docs[..],tokens[..]);p:=fd_plan(2,8,2,2,32);
    cur:[3]PkCursor;batch:[16]u8;done:[2]u8;
    say("2 cursors for 2 rows",fd_cursors(docs[..],tokens[..],p,PkCursor.{seed=3},cur[0..<2]).status);
    say("a cursor past its document",fd_cursors(docs[..],tokens[..],p,PkCursor.{seed=3,offset=40},cur[..]).status);
    #assert fd_cursors(docs[..],tokens[..],p,PkCursor.{seed=3},cur[..]).status==FdStatus.ok;
    say("a 15-byte batch",fd_worker(docs[..],tokens[..],p,0,cur[..],batch[0..<15],done[..]));
    say("worker 2 of 2",fd_worker(docs[..],tokens[..],p,2,cur[..],batch[..],done[..]));
    // Worker 1 is cancelled: row 1 stays unmarked.
    #assert fd_worker(docs[..],tokens[..],p,0,cur[..],batch[..],done[..])==FdStatus.ok;
    say("worker 1 cancelled",fd_publish(p,done[..]));
    r:=fd_ring(p,PkCursor.{seed=3}).ring;
    say("taking from an empty ring",fd_consume(r).status);
    r=fd_produce(r,cur[2]).ring;r=fd_produce(r,cur[2]).ring;
    say("a third batch in 2 slots",fd_produce(r,cur[2]).status);
    one:=[1]int.[3];two:=[2]int.[5,5];
    say("1 compute time, 2 load times",fd_overlap(one[..],two[..],2).status);
}
```

It exits with status 0, and its [transcript](../fixtures/feed/feed-refuse.out)
is exact in both builds:

```text
0 workers: workers: outside 1..64
65 workers: workers: outside 1..64
rows of 1 token: length: outside 2..256
65 rows: rows: outside 1..64
a ring of 9: depth: outside 1..8
2 slots of 2 x 8 in 31 bytes: budget: depth * rows * length is over the budget
2 cursors for 2 rows: buffer: a buffer's length disagrees with the plan
a cursor past its document: cursor: outside the dataset, or the stream ends inside the batch
a 15-byte batch: buffer: a buffer's length disagrees with the plan
worker 2 of 2: worker: index outside the plan's workers
worker 1 cancelled: incomplete: a row was not marked by its owner
taking from an empty ring: empty: no batch is ready
a third batch in 2 slots: full: the ring holds depth batches
1 compute time, 2 load times: shape: times disagree in count, are negative, or are empty
```

A refused call writes nothing: `fd_cursors` stages its cursors and copies them
only when the whole batch is in the stream, and a worker marks a row only
after checking it.

## Run it on host threads

The [harness](../platform/feed.c) runs the same procedures with real threads:
a loader thread with a pool of 1, 2, 4 or 8 worker threads, and the main
thread as the consumer. A lock and two condition variables guard the ring;
the loader fills a slot outside the lock, and the consumer reads a different
slot outside the lock, which the ring guarantees. The consumer stands in for
an accelerator: it copies each batch out, then runs a single-threaded 768 ×
768 matrix-vector product (`pc_matvec_worker`) as its compute. The data is 64
documents of 31 to 95 tokens, 48 of them training (`fd_synthetic`); a batch is
64 rows of 256 tokens, 16 KiB; a run is 64 batches.

The harness:

- compares every run's 1 MiB of batches with the single-threaded `pk_next`
  stream and exits with status 1 on any difference;
- stops a run with 4 workers and 4 slots after 21 batches, resumes a fresh
  pipeline from the consumer's cursor and requires the rest to match; resuming
  from the producer's cursor is recorded as the first byte that differs;
- feeds the measured per-batch load and compute times to `fd_overlap` and
  records its total next to the measured one;
- first runs a small shape under AddressSanitizer with
  UndefinedBehaviorSanitizer, and under ThreadSanitizer.

```shell
psionic/tests/feed.sh --record
```

The [receipt for coderos-4080](../fixtures/feed/coderos-4080.json), an Intel
Core i7-14700K (28 hardware threads) under Linux 6.18 with GCC 15.2 at `-O2`,
recorded 2026-11-01 at a load average of 32, best of 3 runs after one warm-up.
Every one of the twelve runs gave the reference bytes, and the resume from the
consumer's cursor matched. Times are per batch, totals are for 64 batches:

| Workers | Depth | Load | Compute | Total | Model total | Device idle |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 1 | 141 µs | 286 µs | 27.8 ms | 27.4 ms | 339‰ |
| 1 | 2 | 233 µs | 301 µs | 19.4 ms | 19.4 ms | 7‰ |
| 1 | 4 | 233 µs | 296 µs | 19.2 ms | 19.1 ms | 8‰ |
| 2 | 1 | 121 µs | 420 µs | 35.3 ms | 34.6 ms | 236‰ |
| 2 | 2 | 129 µs | 424 µs | 27.6 ms | 27.3 ms | 11‰ |
| 4 | 1 | 79 µs | 349 µs | 28.0 ms | 27.3 ms | 199‰ |
| 4 | 2 | 92 µs | 298 µs | 19.3 ms | 19.2 ms | 7‰ |
| 8 | 1 | 60 µs | 371 µs | 28.1 ms | 27.6 ms | 152‰ |
| 8 | 2 | 58 µs | 297 µs | 19.1 ms | 19.1 ms | 3‰ |

(The receipt also has depth 4 for 2, 4 and 8 workers; they look like depth 2.)

What it shows on this host, at this size:

- With one slot the device waits for every load: 15 to 34% of the run is idle.
  More workers shorten the load (141 µs at one worker, 60 µs at eight) and so
  shorten the wait, but never remove it.
- With two slots the load hides behind compute, because each load is shorter
  than each compute: idle falls below 1.2% at every worker count. A third or
  fourth slot changes nothing here; it would matter if loads varied more.
- The model, given the measured times, predicts the measured total to within
  about 2.5%. The rest is locking and waking threads, which the model does not
  count.

The numbers are not stable. The machine was shared with other jobs (load
average 32 on 28 hardware threads), the harness does not pin threads, and the
i7-14700K mixes performance and efficiency cores: compute for the same work
ranged from 286 to 511 µs per batch across runs depending on where the
scheduler put the consumer. A run minutes earlier, at a load average of 44,
had loads up to 1.7 ms per batch and was load-bound everywhere. That is why no
measured number appears in an exact transcript, and why the receipt names the
machine, compiler, flags and load.

## Checks

The checks in [`feed.coax`](../lib/data/feed.coax) compare against values
derived by hand and against independent references:

- `fd_seek` by 2 to 47 positions lands where `pk_next` does, on chapter
  122's fixture, including whole epochs (12 positions) and the refusal at the
  last epoch;
- an oracle stream, written out epoch by epoch with no cursor arithmetic,
  equals chapter 122's recorded stream, cut inside a document and at an
  epoch's end, and from a later epoch;
- a `#forall` over 12 shapes (1 to 5 rows of 2 to 10 tokens, starting inside
  an epoch, a document and a row) runs `fd_drive` at 1, 2, 4 and 8 workers and
  depths 1 to 4 under drawn patterns, and requires the oracle's bytes; it then
  stops each run partway, resumes from the consumer's cursor, and requires the
  same bytes again;
- a worker writes only its rows, a batch with an unmarked or wrongly marked row
  is not published, and a cursor list that disagrees with the stream is
  refused;
- the ring's hand trace above, every refusal, and the drive's patterns
  (all ones, all zeros, bit 0 clear, bit 1 only, which repeats at turn 65);
- the stall model's hand traces, and a `#forall` over constant times (compute
  and load 0 to 6, 1 to 19 steps, every depth) against the closed forms
  `n × (c + l)` for one slot and `l + c + (n − 1) × max(c, l)` for more;
  against compute-after-load-prefix-sums for a ring as deep as the run; and
  that more slots never make a run slower.

`coax mutate psionic/lib/data/feed.coax --root-only` leaves no survivor (366
of 366 killed).

## What this does not cover

- Only packed token batches exist. There is no image pipeline, no padding
  with masks (packing makes them unnecessary for this text stream), no
  per-example transforms, and no samplers other than chapter 122's seeded
  epoch shuffle.
- There is no GPU. Pinned host memory and copies to a device
  ([chapter 6](ch006-one-tensor-several-devices-and-dtypes.md)) are not used;
  the consumer is a CPU thread, and "GPU time lost to loading" is measured
  only for that stand-in.
- Each row's `pk_next` admits the whole dataset again. That keeps every row
  independently checked but makes loading cost grow with the dataset; the
  measured load times include it.
- Workers are host threads in C until Coax has jobs.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
- [nanochat target notes](../docs/targets/nanochat.md)
