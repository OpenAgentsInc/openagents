# 189. Place the model

> **Status: partly implemented.** A placement solver runs on the CPU
> against simulated peers (profile place-v1). It assigns each stage of
> [chapter 186](ch186-split-a-model-into-stages.md)'s split to its own
> peer under hard memory and readiness limits, minimizes one token's
> latency or the pipeline's period, refuses with the binding constraint
> and the smallest change, and replans with the fewest stages moved when a
> peer leaves. Still to build: rates and link costs measured on live peers
> rather than written into a simulated fleet, a check that a live
> placement's measured latency or throughput is within a stated tolerance
> of the estimate, and choosing the cut together with the peers.

A mesh must decide where a model runs. The order is short: run the whole
model on this machine if it fits, else the whole model on a peer that has
room, else split it into stages and put each stage on its own peer.
Mesh-LLM uses the same order. The first two cases are a lookup. The third
is a small search, and this chapter builds it.

The library is [`place.coax`](../lib/place.coax). It is an `#authority`
file, because it decides which machine runs which part of a model. It
reuses chapter 186's planner for the stages, chapter 187's package sizes,
chapter 33's roofline for a stage's time on a peer, chapter 178's fetch
and verify phases for the time until a stage is ready, and chapter 188's
measured round trips for the links. The examples are
[`place.coax`](../examples/place.coax) and
[`place-refuse.coax`](../examples/place-refuse.coax).

```shell
coax/bin/coax run psionic/examples/place.coax
coax/bin/coax run psionic/examples/place-refuse.coax
psionic/check.sh --only place
```

## What the solver knows

The solver sees a stage as five numbers. They come from chapter 186's
plan for one model at one context length.

<!-- source: psionic/lib/place.coax model -->
```jai
// One stage, as the solver sees it: the bytes it keeps resident, the bytes
// it reads for each token (its layers' weights and cache, and the readout
// on the last stage), its multiply-adds for each token, its package's
// bytes, and the bytes it sends for each token: the boundary rows to the
// next stage, or the sampled token back from the last.
PlcStage :: struct {bytes:int;read:int;macs:int;package:int;out:int;}
// A job: its stages in pipeline order, and the bytes this machine sends
// the first stage for each token.
PlcJob :: struct {stages:int;stage:[8]PlcStage;entry:int;}
// A peer's advertisement: its memory budget, its rates in millions a second
// (multiply-adds, memory bytes read, bytes downloaded, bytes hashed), and
// a bit for each stage whose package it already holds.
PlcPeer :: struct {live:bool;budget:int;macs:int;read:int;fetch:int;verify:int;holds:int;}
// The fleet: peer 0 is this machine, where tokens enter and leave. lat is
// the one-way latency from peer to peer in nanoseconds, bw the bandwidth
// in MB/s (0: no link). deadline: a moved stage must be ready this soon.
PlcFleet :: struct {peers:int;peer:[8]PlcPeer;lat:[8][8]int;bw:[8][8]int;deadline:int;}
// Build the job for plan p of model m at `context` positions, with
// boundary rows of `width` values and token ids of 4 bytes.
plc_job :: (m:StgModel,p:StgPlan,context:int,width:int)->PlcJob {
    j:=PlcJob.{stages=p.stages,entry=4};
    for i:0..<p.stages {
        a:=p.cut[i];b:=p.cut[i+1];last:=i==p.stages-1;
        s:=PlcStage.{bytes=p.bytes[i],macs=p.macs[i],package=stg_bytes(m,a,b,i==0,last,0)};
        for l:a..<b {s.read+=m.weight[l]+m.kv[l]*context;}
        if last {s.read+=m.head+m.norm;s.package-=m.out;s.out=4;} else {s.out=stg_message_bytes(1,width);}
        j.stage[i]=s;
    }
    return j;
}
```

A first stage reads one row of the embedding per token, not the whole
matrix, so its read bytes leave the embedding out. The last stage reads
the whole readout. The package is the stage's weights, which is chapter
186's byte count with no context and without the logits buffers, as
chapter 187 checks.

### Worked stages: Ministral 3 3B in two

Chapter 186 splits Ministral 3 3B after layer 14 when each of two stages
has 6 GB at 4,096 positions. One layer holds 232,796,160 weight bytes and
8,192 × 4,096 = 33,554,432 cache bytes, so it reads 266,350,592 bytes for
each token.

| | Stage 0 (layers 0–13) | Stage 1 (layers 14–25) |
| --- | --- | --- |
| Resident bytes (chapter 186) | 4,735,541,248 | 4,203,907,072 |
| Read for each token | 14 × 266,350,592 = 3,728,908,288 | 12 × 266,350,592 + 805,306,368 + 6,144 = 4,001,519,616 |
| Multiply-adds for each token | 2,099,249,152 | 2,202,009,600 |
| Package bytes (chapter 187) | 4,064,452,608 | 3,598,866,432 |
| Sends for each token | one row, 12,324 bytes | one token id, 4 bytes |

This machine sends each token id, 4 bytes, to stage 0.

## What an assignment costs

Rates are integer millions a second, as in chapter 33's roofline: a peer
that does 0.5 trillion multiply-adds a second advertises 500,000. Times are
nanoseconds, rounded up. The time for `x` units at `rate` is
`ceil(x × 1000 / rate)`.

<!-- source: psionic/lib/place.coax cost -->
```jai
// x units at `rate` millions a second, in nanoseconds rounded up.
plc_ns :: (x:int,rate:int)->int {return (x*1000+rate-1)/rate;}
// A stage's time for one token on a peer: the slower of its arithmetic and
// its memory traffic (chapter 33's roofline).
plc_compute :: (s:PlcStage,p:PlcPeer)->int {return stg_max(plc_ns(s.macs,p.macs),plc_ns(s.read,p.read));}
// Sending b bytes over a link, or from peer a to peer c: nothing on one
// peer, -1 with no link.
plc_send :: (lat:int,bw:int,b:int)->int {
    if bw==0 {return -1;}
    return lat+plc_ns(b,bw);
}
plc_hop :: (f:PlcFleet,a:int,c:int,b:int)->int {return ifx a==c then 0 else plc_send(f.lat[a][c],f.bw[a][c],b);}
// Time until stage `index` is ready on a peer: fetch its package unless the
// peer holds it, then hash every byte.
plc_ready :: (s:PlcStage,p:PlcPeer,index:int)->int {
    t:=plc_ns(s.package,p.verify);
    if (p.holds>>index)&1==0 {t+=plc_ns(s.package,p.fetch);}
    return t;
}
// What an assignment costs. One token's latency: in to the first stage,
// every stage and every hop, back out. The period: the busiest step, where
// a stage's step is its compute and its send.
PlcCost :: struct {latency:int;period:int;ready:int;compute:[8]int;hop:[9]int;}
plc_eval :: (j:PlcJob,f:PlcFleet,peer:[]int)->PlcCost {
    e:PlcCost;e.hop[0]=plc_hop(f,0,peer[0],j.entry);e.latency=e.hop[0];e.period=e.hop[0];
    for s:0..<j.stages {
        next:=ifx s==j.stages-1 then 0 else peer[s+1];
        e.compute[s]=plc_compute(j.stage[s],f.peer[peer[s]]);e.hop[s+1]=plc_hop(f,peer[s],next,j.stage[s].out);
        e.latency+=e.compute[s]+e.hop[s+1];e.period=stg_max(e.period,e.compute[s]+e.hop[s+1]);
        e.ready=stg_max(e.ready,plc_ready(j.stage[s],f.peer[peer[s]],s));
    }
    return e;
}
```

Two objectives are stated, and both charge every transfer:

- **Latency** is one token's trip: the hop in, every stage's time, every
  hop between stages, and the hop back to this machine. It is the sum.
- **Period** is the time between tokens when many sequences fill the
  pipeline. Each step is a stage's time plus its send, and the period is
  the largest step. Tokens a second is 10⁹ divided by the period.

Splitting removes the per-layer collectives of tensor parallelism
([chapter 183](ch183-split-a-model-across-devices.md)). It does not remove the
boundary traffic. Both objectives count it.

### Worked placement: three simulated peers

The example's fleet has this machine and two peers:

| Peer | Budget | Multiply-adds | Memory read | Fetch | Hash | Holds |
| --- | --- | --- | --- | --- | --- | --- |
| 0, this machine | 5 GB | 500,000 (0.5 T/s) | 100,000 MB/s | 100 MB/s | 1,000 MB/s | both packages |
| 1 | 6 GB | 20,000,000 (20 T/s) | 400,000 MB/s | 1,000 MB/s | 2,000 MB/s | none |
| 2 | 4.5 GB | 1,000,000 (1 T/s) | 200,000 MB/s | 250 MB/s | 500 MB/s | none |

The links are the same both ways: 0–1 has 0.5 ms one way and 125 MB/s,
0–2 has 2 ms and 25 MB/s, and 1–2 has 0.25 ms and 1,250 MB/s. A stage
must be ready within 60 s.

<!-- source: psionic/examples/place.coax fleet -->
```jai
// This machine and two peers, as they advertise themselves: memory budget,
// rates in millions a second, and which stage packages they hold.
place_fleet :: ()->PlcFleet {
    f:PlcFleet;f.peers=3;f.deadline=60000000000;
    f.peer[0]=PlcPeer.{live=true,budget=5000000000,macs=500000,read=100000,fetch=100,verify=1000,holds=3};
    f.peer[1]=PlcPeer.{live=true,budget=6000000000,macs=20000000,read=400000,fetch=1000,verify=2000};
    f.peer[2]=PlcPeer.{live=true,budget=4500000000,macs=1000000,read=200000,fetch=250,verify=500};
    // One-way latency in nanoseconds and bandwidth in MB/s, both ways.
    f=place_link(f,0,1,500000,125);f=place_link(f,0,2,2000000,25);f=place_link(f,1,2,250000,1250);
    return f;
}
```

**Memory.** The whole model, 7,932,815,360 bytes, fits no peer. Stage 0
(4,735,541,248 bytes) fits peers 0 and 1, but not peer 2. Stage 1
(4,203,907,072 bytes) fits all three. So four assignments of (stage 0,
stage 1) remain: (0, 1), (0, 2), (1, 0) and (1, 2).

**Stage times.** Every stage here is memory-bound. For stage 0 on peer 0,
the arithmetic takes ceil(2,099,249,152 × 1000 / 500,000) = 4,198,499 ns,
and the reads take ceil(3,728,908,288 × 1000 / 100,000) = 37,289,083 ns.
The slower one is the stage's time. The other cells divide the same read
bytes by each peer's memory rate: for stage 1 on peer 2,
ceil(4,001,519,616 × 1000 / 200,000) = 20,007,599 ns, against
2,202,010 ns of arithmetic.

| | Peer 0 | Peer 1 | Peer 2 |
| --- | --- | --- | --- |
| Stage 0 | 37,289,083 | 9,322,271 | does not fit |
| Stage 1 | 40,015,197 | 10,003,800 | 20,007,599 |

**Hops.** A hop costs its one-way latency plus its bytes at the link's
bandwidth:

- 0 → 1 with 4 bytes: 500,000 + ceil(4,000 / 125) = 500,032 ns.
- 0 → 1 with 12,324 bytes: 500,000 + ceil(12,324,000 / 125) = 598,592 ns.
- 1 → 2 with 12,324 bytes: 250,000 + ceil(12,324,000 / 1,250) = 259,860 ns.
- 0 → 2 with 12,324 bytes: 2,000,000 + 492,960 = 2,492,960 ns.
- 2 → 0 with 4 bytes: 2,000,000 + 160 = 2,000,160 ns.
- A hop that stays on one machine costs nothing.

**Latency and period**, in nanoseconds:

| (stage 0, stage 1) | Latency: in + stage 0 + hop + stage 1 + out | Period: largest step |
| --- | --- | --- |
| (0, 1) | 0 + 37,289,083 + 598,592 + 10,003,800 + 500,032 = 48,391,507 | 37,289,083 + 598,592 = 37,887,675 |
| (0, 2) | 0 + 37,289,083 + 2,492,960 + 20,007,599 + 2,000,160 = 61,789,802 | 37,289,083 + 2,492,960 = 39,782,043 |
| (1, 0) | 500,032 + 9,322,271 + 598,592 + 40,015,197 + 0 = 50,436,092 | 40,015,197 + 0 = 40,015,197 |
| (1, 2) | 500,032 + 9,322,271 + 259,860 + 20,007,599 + 2,000,160 = 32,089,922 | 20,007,599 + 2,000,160 = 22,007,759 |

(1, 2) wins on both objectives: about 32.1 ms a token alone, and about 45
tokens a second with the pipeline full.

**Readiness.** Stage 0 on peer 1 fetches 4,064,452,608 bytes at 1,000 MB/s
(4,064,452,608 ns) and hashes them at 2,000 MB/s (2,032,226,304 ns):
6,096,678,912 ns. Stage 1 on peer 2 fetches at 250 MB/s (14,395,465,728
ns) and hashes at 500 MB/s (7,197,732,864 ns): 21,593,198,592 ns. So (1, 2)
is ready in about 21.6 s, within the 60 s deadline. With a 20 s deadline,
stage 1 cannot go to peer 2, and the best of the rest is (0, 1) at
48,391,507 ns. This machine already holds both packages, so stage 0 only
hashes there (4,064,452,608 ns), and stage 1 is ready on peer 1 in
3,598,866,432 + 1,799,433,216 = 5,398,299,648 ns.

## The solver

A fleet has at most 8 peers, so the solver can afford exact answers. It
runs a dynamic program over the set of peers used so far. Stages are
placed in pipeline order, so a set of `k` peers means stages 0 to `k − 1`
are placed. The entry for (set, peer) is the best cost of the remaining
stages when the last placed stage runs on that peer. Both objectives
combine step by step: latency adds, and period takes the larger value. So
the best tail for a set and a peer is also part of the best whole.

<!-- source: psionic/lib/place.coax solve -->
```jai
// The solver's answer. On ok: the peer of each stage, the objective's cost,
// the stages moved from `old`, and the assignment's latency, period, the
// time until its moved stages are ready, and its per-stage times. On a
// refusal: the binding constraint and the smallest change. memory and
// deadline: `stage` fits on no peer, and `who` is the peer it misses by the
// least, by `gap` bytes or nanoseconds. peers: the stages in the bit set
// `set` fit on `gap` fewer peers than they number. link: every stage has
// a peer of its own, but no chain of them is linked.
PlcPlacement :: struct {
    status:PlcStatus;goal:PlcGoal;stages:int;peer:[8]int;cost:int;moves:int;
    latency:int;period:int;ready:int;compute:[8]int;hop:[9]int;
    stage:int;who:int;gap:int;set:int;
}
plc_count :: (mask:int)->int {n:=0;m:=mask;while m!=0 {n+=m&1;m=m>>1;}return n;}
// May stage s run on peer a? It must be live and fit; unless it stays where
// it was, it must be ready by the deadline.
plc_fits :: (j:PlcJob,f:PlcFleet,old:[]int,s:int,a:int)->bool {
    p:=f.peer[a];
    if !p.live || j.stage[s].bytes>p.budget {return false;}
    return (old.count>0 && old[s]==a) || plc_ready(j.stage[s],p,s)<=f.deadline;
}
// Is (m, c) better than the best so far (bm, bc)? Fewer moves, then lower
// cost; an empty best (bc < 0) loses to anything.
plc_better :: (m:int,c:int,bm:int,bc:int)->bool {return bc<0 || m<bm || (m==bm && c<bc);}
plc_place :: (j:PlcJob,f:PlcFleet,goal:PlcGoal,old:[]int)->PlcPlacement {
    r:=PlcPlacement.{goal=goal,stages=j.stages};n:=j.stages;P:=f.peers;
    if !plc_valid(j,f,old) {r.status=PlcStatus.input;return r;}
    // Each stage needs a peer it fits on; else name the closest miss.
    hosts:[9]int;
    for s:0..<n {
        for a:0..<P {if plc_fits(j,f,old,s,a) {hosts[s]|=1<<a;}}
        if hosts[s]!=0 {continue;}
        // Memory binds unless some live peer has the bytes; then the deadline does.
        room:=false;
        for a:0..<P {if f.peer[a].live && j.stage[s].bytes<=f.peer[a].budget {room=true;}}
        r.status=ifx room then PlcStatus.deadline else PlcStatus.memory;r.stage=s;r.who=-1;
        for a:0..<P {
            p:=f.peer[a];
            if !p.live || (room && j.stage[s].bytes>p.budget) {continue;}
            over:=ifx room then plc_ready(j.stage[s],p,s)-f.deadline else j.stage[s].bytes-p.budget;
            if r.who<0 || over<r.gap {r.gap=over;r.who=a;}
        }
        return r;
    }
    // Hall's condition: every set of stages needs as many peers as stages.
    // The set that falls shortest says how many peers to add; the search
    // runs from the set of all stages down, and the first such set is kept.
    for k:0..<(1<<n) {
        set:=(1<<n)-1-k;
        near:=0;for s:0..<n {if (set>>s)&1==1 {near|=hosts[s];}}
        lack:=plc_count(set)-plc_count(near);
        if lack>r.gap {r.gap=lack;r.set=set;}
    }
    if r.gap>0 {r.status=PlcStatus.peers;return r;}
    // Each stage's time on each peer, and each hop's: hop 0 carries the
    // token in, hop s + 1 what stage s sends.
    own:[8][8]int;hop:[9][8][8]int;
    for s:0..<n {for a:0..<P {if (hosts[s]>>a)&1==1 {own[s][a]=plc_compute(j.stage[s],f.peer[a]);}}}
    for k:0..<n+1 {
        sent:=ifx k==0 then j.entry else j.stage[k-1].out;
        for a:0..<P {for c:0..<P {hop[k][a][c]=ifx a==c then 0 else plc_send(f.lat[a][c],f.bw[a][c],sent);}}
    }
    // bm[used][a] and bc[used][a]: the fewest moves, then the lowest cost,
    // of stages s..<n when stage s runs on peer a and `used` holds the
    // peers of stages 0..s (so s is one less than its count); bc is -1 when
    // there is no way on. next: the peer of stage s + 1 on that best way.
    // A larger `used` is filled first, so each entry reads finished ones.
    bm:[256][8]int;bc:[256][8]int;next:[256][8]int;
    for k:0..<(1<<P)-1 {
        used:=(1<<P)-1-k;s:=plc_count(used)-1;
        for a:0..<P {
            bc[used][a]=-1;
            // A stage past the last has no hosts. Entries for a peer outside
            // `used` are never read.
            if (hosts[s]>>a)&1==0 {continue;}
            t:=own[s][a];dm:=ifx old.count>0 && old[s]!=a then 1 else 0;
            if s==n-1 {
                h:=hop[n][a][0];
                if h>=0 {bm[used][a]=dm;bc[used][a]=t+h;}
                continue;
            }
            for c:0..<P {
                if (used>>c)&1==1 {continue;}
                h:=hop[s+1][a][c];w:=used|(1<<c);
                if h<0 || bc[w][c]<0 {continue;}
                cost:=ifx goal==PlcGoal.latency then t+h+bc[w][c] else stg_max(t+h,bc[w][c]);
                if plc_better(dm+bm[w][c],cost,bm[used][a],bc[used][a]) {bm[used][a]=dm+bm[w][c];bc[used][a]=cost;next[used][a]=c;}
            }
        }
    }
    r.cost=-1;first:int;
    for a:0..<P {
        h:=hop[0][0][a];
        if h<0 || bc[1<<a][a]<0 {continue;}
        cost:=ifx goal==PlcGoal.latency then h+bc[1<<a][a] else stg_max(h,bc[1<<a][a]);
        if plc_better(bm[1<<a][a],cost,r.moves,r.cost) {r.moves=bm[1<<a][a];r.cost=cost;first=a;}
    }
    if r.cost<0 {r.status=PlcStatus.link;return r;}
    used:=0;a:=first;
    for s:0..<n {r.peer[s]=a;used|=1<<a;a=next[used][a];}
    e:=plc_eval(j,f,r.peer[0..<n]);
    r.latency=e.latency;r.period=e.period;r.compute=e.compute;r.hop=e.hop;
    for s:0..<n {if old.count==0 || old[s]!=r.peer[s] {r.ready=stg_max(r.ready,plc_ready(j.stage[s],f.peer[r.peer[s]],s));}}
    return r;
}
```

Three rules make the answer deterministic and explainable:

- **Hard limits first.** A stage may run on a peer only if the peer is
  live, has the bytes, and can have the stage ready by the deadline. Cost
  never trades against these.
- **Ties go to the lower peer number.** The program keeps the first best
  entry it finds, in peer order. So among equal-cost assignments, the
  answer is the first in lexicographic order of peers.
- **Refusals in a fixed order.** First a stage that fits on no peer
  (memory, or the deadline if some peer has the bytes). Then Hall's
  condition: if some set of stages can use fewer peers than it has
  stages, no assignment gives each stage its own peer. Only then the
  links.

The table is 2⁸ sets × 8 peers. Each entry tries at most 8 next peers, so a
full fleet costs about 16,000 steps.

## Replanning when a peer leaves

When a peer leaves, the stages on other peers are already loaded. Moving
one means fetching and hashing its package again. So a replan passes the
old assignment, and the solver ranks assignments by moved stages first,
then by the objective. Both orders still combine step by step, so the
same program finds the answer. A stage that stays where it was is already
ready, so the deadline applies only to stages that move.

In the example's fleet, with stages on (1, 2):

- **Peer 1 leaves.** Stage 0 fits only on peer 0 now. Stage 1 stays on
  peer 2: (0, 2), one move, 61,789,802 ns. Stage 0 is ready again in
  4,064,452,608 ns, the hash on this machine.
- **Peer 2 leaves.** Fresh, the best is (0, 1) at 48,391,507 ns, but that
  moves both stages. (1, 0) moves only stage 1 and costs 50,436,092 ns.
  The replan picks (1, 0).

## Whole first

<!-- source: psionic/lib/place.coax choose -->
```jai
// The order a mesh tries: the whole model on this machine, then the whole
// model on a peer, then each split in turn. jobs[0] is the whole model when
// it has one stage; later jobs split it into more. The first that places
// wins; when none does, the last job's refusal says what to change.
PlcChoice :: struct {status:PlcStatus;which:int;placed:PlcPlacement;}
plc_choose :: (jobs:[]PlcJob,f:PlcFleet,goal:PlcGoal)->PlcChoice {
    c:=PlcChoice.{status=PlcStatus.input};zero:[1]int;none:=zero[0..<0];
    for k:0..<jobs.count {
        j:=jobs[k];c.which=k;
        if j.stages==1 && plc_valid(j,f,none) && plc_fits(j,f,none,0,0) {
            self:=f;for a:1..<f.peers {self.peer[a].live=false;}
            c.placed=plc_place(j,self,goal,none);c.status=c.placed.status;return c;
        }
        c.placed=plc_place(j,f,goal,none);c.status=c.placed.status;
        if c.status==PlcStatus.ok {return c;}
    }
    return c;
}
```

A whole model on this machine needs no network at all, so it wins even
when a peer would be faster. If peer 1 had 8 GB, the whole model would run
there: ceil(7,730,427,904 × 1000 / 400,000) = 19,326,070 ns plus two 4-byte
hops of 500,032 ns, 20,326,134 ns. If this machine also had 8 GB, the
model would stay here at 77,304,280 ns, even though that is slower.

## Running it

<!-- source: psionic/examples/place.coax place-example -->
```jai
main :: () {
    m:=stg_ministral(MS_3B);C:=4096;
    two:=[2]int.[6000000000,6000000000];j:=plc_job(m,stg_plan(m,two[..],C),C,MS_3B.width);
    for s:0..<j.stages {
        t:=j.stage[s];
        print("stage %: % bytes resident, % read and % multiply-adds a token, package % bytes, sends % bytes\n",s,t.bytes,t.read,t.macs,t.package,t.out);
    }
    f:=place_fleet();
    for a:0..<3 {for c:0..<3 {
        if a==c {continue;}
        if j.stage[0].bytes>f.peer[a].budget || j.stage[1].bytes>f.peer[c].budget {print("(%, %): does not fit\n",a,c);continue;}
        pair:=[2]int.[a,c];e:=plc_eval(j,f,pair[..]);
        print("(%, %): latency % ns, period % ns, ready in % ns\n",a,c,e.latency,e.period,e.ready);
    }}
    zero:[1]int;none:=zero[0..<0];
    place_show("fastest token",plc_place(j,f,PlcGoal.latency,none));
    place_show("most tokens a second",plc_place(j,f,PlcGoal.throughput,none));
    g:=f;g.deadline=20000000000;place_show("ready within 20 s",plc_place(j,g,PlcGoal.latency,none));
    one:=[1]int.[8000000000];w:=plc_job(m,stg_plan(m,one[..],C),C,MS_3B.width);
    jobs:=[2]PlcJob.[w,j];c:=plc_choose(jobs[..],f,PlcGoal.latency);
    print("whole, else split: job % of 2 (% stages)\n",c.which,jobs[c.which].stages);
    g=f;g.peer[1].budget=8000000000;c=plc_choose(jobs[..],g,PlcGoal.latency);place_show("peer 1 with 8 GB",c.placed);
    g.peer[0].budget=8000000000;c=plc_choose(jobs[..],g,PlcGoal.latency);place_show("this machine with 8 GB too",c.placed);
    old:=[2]int.[1,2];
    g=f;g.peer[1].live=false;place_show("peer 1 leaves",plc_place(j,g,PlcGoal.latency,old[..]));
    g=f;g.peer[2].live=false;place_show("peer 2 leaves",plc_place(j,g,PlcGoal.latency,old[..]));
    place_show("peer 2 leaves, planned afresh",plc_place(j,g,PlcGoal.latency,none));
}
```

The transcript, the same in debug and release builds:

```text
stage 0: 4735541248 bytes resident, 3728908288 read and 2099249152 multiply-adds a token, package 4064452608 bytes, sends 12324 bytes
stage 1: 4203907072 bytes resident, 4001519616 read and 2202009600 multiply-adds a token, package 3598866432 bytes, sends 4 bytes
(0, 1): latency 48391507 ns, period 37887675 ns, ready in 5398299648 ns
(0, 2): latency 61789802 ns, period 39782043 ns, ready in 21593198592 ns
(1, 0): latency 50436092 ns, period 40015197 ns, ready in 6096678912 ns
(1, 2): latency 32089922 ns, period 22007759 ns, ready in 21593198592 ns
(2, 0): does not fit
(2, 1): does not fit
fastest token: stages on peers 1 2, latency 32089922 ns, 0 moved, ready in 21593198592 ns
most tokens a second: stages on peers 1 2, period 22007759 ns, 0 moved, ready in 21593198592 ns
ready within 20 s: stages on peers 0 1, latency 48391507 ns, 0 moved, ready in 5398299648 ns
whole, else split: job 1 of 2 (2 stages)
peer 1 with 8 GB: stages on peers 1, latency 20326134 ns, 0 moved, ready in 10287019008 ns
this machine with 8 GB too: stages on peers 0, latency 77304280 ns, 0 moved, ready in 6858012672 ns
peer 1 leaves: stages on peers 0 2, latency 61789802 ns, 1 moved, ready in 4064452608 ns
peer 2 leaves: stages on peers 1 0, latency 50436092 ns, 1 moved, ready in 3598866432 ns
peer 2 leaves, planned afresh: stages on peers 0 1, latency 48391507 ns, 0 moved, ready in 5398299648 ns
```

The `ready in` column of the assignment table counts every stage, because
nothing is loaded yet. After a replan, it counts only the stages that
moved.

## Refusals

Each refusal names the binding constraint and the smallest change that
would remove it:

| Status | Binding constraint | Smallest change reported |
| --- | --- | --- |
| `memory` | `stage` fits on no live peer | `who`, the peer it misses by the fewest bytes, and `gap`, those bytes |
| `deadline` | some peer has the bytes, but none has the stage ready in time | `who` and `gap`, the fewest nanoseconds over the deadline |
| `peers` | the stages in bit set `set` fit on fewer peers than they number | `gap`, the number of peers to add |
| `link` | every stage has its own peer, but no assignment has every hop linked | a link; no single number |
| `input` | a count, rate or amount out of bounds, or this machine missing | nothing to place |

<!-- source: psionic/examples/place-refuse.coax refuse -->
```jai
main :: () {
    zero:[1]int;none:=zero[0..<0];f:=place_fleet();
    one:=[1]int.[8000000000];place_why("the whole model",plc_place(place_job(one[..]),f,PlcGoal.latency,none));
    two:=[2]int.[6000000000,6000000000];j:=place_job(two[..]);
    g:=f;g.deadline=3000000000;place_why("two stages, ready within 3 s",plc_place(j,g,PlcGoal.latency,none));
    four:=[4]int.[3000000000,3000000000,3000000000,3000000000];place_why("four stages",plc_place(place_job(four[..]),f,PlcGoal.latency,none));
    g=f;g.peer[0].budget=4000000000;g.bw[1][2]=0;place_why("this machine with 4 GB, no link from 1 to 2",plc_place(j,g,PlcGoal.latency,none));
    g=f;g.peer[0].live=false;place_why("this machine gone",plc_place(j,g,PlcGoal.latency,none));
    old:=[2]int.[1,2];g=f;g.peer[1].live=false;g.deadline=0;place_why("peer 1 leaves, no time to load",plc_place(j,g,PlcGoal.latency,old[..]));
}
```

```text
the whole model: no peer has the memory: stage 0 misses peer 1 by 1932815360 bytes
two stages, ready within 3 s: no peer is ready in time: stage 0 misses peer 0 by 1064452608 ns
four stages: too few peers: stages in set 15 need 1 more peer(s)
this machine with 4 GB, no link from 1 to 2: no linked chain of peers
this machine gone: input
peer 1 leaves, no time to load: no peer is ready in time: stage 0 misses peer 0 by 4064452608 ns
```

The numbers check by hand. The whole model needs 7,932,815,360 bytes and
peer 1 has 6,000,000,000, so it is 1,932,815,360 bytes short; a peer with
that much more memory would take it. Within 3 s, stage 0 is ready soonest
on this machine, in 4,064,452,608 ns: 1,064,452,608 ns late. Four stages
of chapter 186's 3 GB split fit on any of the three peers, but four stages
need four peers: one more. With 4 GB on this machine, stage 0 fits only
on peer 1 and stage 1 only on peers 1 and 2, so (1, 2) is the only
assignment, and its 1 → 2 link is gone. When peer 1 leaves with no time to
load, stage 0 must move, and this machine is the closest miss, by the
whole of its hash time. Stage 1 stays on peer 2, so no deadline applies to
it.

## Checks

- **Placement** (`place.coax`, `#authority`):
  - The hand-worked numbers above: Ministral 3 3B's two stages from
    chapter 186's plan and chapter 187's package sizes, each stage time,
    hop and readiness, and the latency and period of all four
    assignments.
  - The placements for both objectives, the 20 s deadline, both replans,
    and a stage that stays put needing no deadline.
  - Every refusal kind, with its stage, peer, gap and set: ties between
    equally close peers go to the first, a peer that has left is never
    named, a peer with exactly the bytes is a candidate, two sets equally
    short keep the set of all stages, and each missing hop (in, between,
    out) refuses with `link`.
  - Whole on this machine, whole on a peer, then the split; a deadline
    that forces the split; an invalid whole job that falls through to the
    split; and the last job's refusal when none places.
  - Every input bound, just inside and just outside, for stages, peers,
    links, the deadline and the old assignment. Eight stages on eight
    peers that fit only one way.
  - A `#forall` over 1,200 random fleets (1 to 5 peers, 1 to 4 stages,
    missing links, peers that have left, small numbers so that ties are
    common, both objectives, with and without an old assignment). An
    independent brute force tries every assignment of stages to distinct
    peers with its own arithmetic. The solver's peers, cost and moves
    equal its first best assignment; latency, period and readiness equal
    its evaluation. When nothing places, the refusal kind equals the one
    found by enumeration.
  - `coax mutate --root-only` leaves no survivors (401 mutants).

## What remains

- **Live peers.** Every rate, budget and link here is written into a
  simulated fleet. On live peers, each rate comes from chapter 33's
  roofline harness, each budget from the peer's advertisement, and each
  link from [chapter 188](ch188-peers-transport-and-discovery.md)'s
  pings, which run in process today.
- **Estimate against measurement.** The draft's last test, that a live
  placement's measured latency or throughput is within a stated tolerance
  of the estimate, needs live peers and a running split
  ([chapter 190](ch190-stage-lifecycle-and-fencing.md)).
- **The cut and the peers together.** Chapter 186 cuts the layers for
  given budgets first, and this solver places the stages it is given. A
  fast peer could take more layers than an even cut gives it. Searching
  cuts and peers together is not built.
- **Prefill and batches.** The costs are for one decode token. Prefill
  of a long prompt and batched decoding change both objectives.

## Dependencies

- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the roofline
  that turns a stage's arithmetic and reads into time.
- [Chapter 178](ch178-start-fast.md): the fetch and verify phases that
  make a stage ready.
- [Chapter 186](ch186-split-a-model-into-stages.md): the planner, its byte
  and multiply-add counts, and the boundary message's size.
- [Chapter 187](ch187-stage-packages.md): the package sizes.
- [Chapter 188](ch188-peers-transport-and-discovery.md): peers and
  measured link costs.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
