# 177. Route, queue and scale replicas

> **Status: partly implemented.** The router, bounded queues, typed
> refusals and the autoscaler (profile rr-route-v1) run on the CPU in a
> discrete-event simulation of several replicas in one process, each with
> chapter 176's prefix trie and chapter 175's step-time model. They are
> checked by hand traces, a brute-force router, invariants recomputed from
> every simulated request, and frozen runs on synthetic and recorded
> traffic. No real replica runs: separate processes or machines, a live run
> that reproduces the simulated percentiles, batching inside a replica,
> adapter-aware routing and decode balancing by cache length are not
> built; the last section lists them.

One replica serves a few users. More traffic needs several replicas, a
router that sends each request to one of them, a queue in front of each
that cannot grow without bound, and an autoscaler that changes how many
replicas run. Each of these decides where work and money go, so this
chapter writes them as pure Coax procedures in an `#authority` file,
[`serve-route.coax`](../lib/serve-route.coax), and runs them against a
deterministic simulation before any machine does.

The chapter builds on three earlier ones. Chapter 176's prefix trie is
what the router reads: a replica that already holds a request's prompt
blocks can skip computing them. Chapter 175's step-time model, a straight
line through chapter 71's two gpt-oss-20b receipts, prices each request.
Chapter 178's startup receipt gives the cost of starting a replica, which
the autoscaler weighs before it adds one.

## Requests and replicas

A request names its client, its arrival time in microseconds, and its
prompt and output lengths; its tokens sit in a separate array, 32 per
request. A replica is off, starting or ready. It holds a bounded queue of
request numbers, the predicted cost of what waits there, the request it is
serving and when that ends, and its own prefix trie and frames
(`PgStore`, chapter 176).

<!-- source: psionic/lib/serve-route.coax types -->
```jai
// Why a request was refused: outside the request bounds, its client already
// holds its share of the requests in flight, every replica's queue is full,
// or no replica is running or starting.
RrRefusal :: enum u8 {none;shape;client;full;cold;}
// affinity: the smallest predicted finish, counting cached blocks. least:
// the same prediction, blind to the cache. round: the next replica in turn
// with room.
RrPolicy :: enum u8 {affinity;least;round;}
RrPhase :: enum u8 {off;starting;ready;}
// replicas: running at the start. depth: most requests waiting at one
// replica. share: most requests one client may have queued or running.
// events: the most event times a simulation may take.
RrConfig :: struct {policy:RrPolicy;replicas:int;depth:int;clients:int;share:int;pages:PgConfig;events:int;}
RrRequest :: struct {client:int;arrive:int;prompt:int;output:int;}
// One replica: its queue (request numbers, oldest first), the predicted
// cost of what waits there, the request it is serving and when that ends,
// and its own prefix trie and frames.
RrReplica :: struct {phase:RrPhase;ready_at:int;running:int;busy_until:int;nq:int;queue:[8]int;work:int;st:PgStore;}
// The router's choice: a replica, the cached blocks it counted there, and
// the predicted time from now until the request would finish.
RrPick :: struct {refusal:RrRefusal;replica:int;cached:int;est:int;turn:int;}
```

Every replica in this profile serves one request at a time. Batching
inside a replica is chapter 175's job; composing it with the router is not
built here.

## The cost of a request

The router compares replicas by the time each would take to finish the
request. That needs a price for one request. A request runs its uncached
prompt positions as the rows of one step, which yields its first output
token, and then one step of one row for each further output token.
Chapter 175's model prices a step of `r` rows at 17,792,472 ns plus
1,459,182 ns for each row after the first.

<!-- source: psionic/lib/serve-route.coax cost -->
```jai
// Modeled microseconds: the uncached prompt positions run as rows of one
// step that yields the first token; each further output token is a step of
// one row (chapter 175's cb_step_ns). Cached positions are not run.
rr_prefill :: (prompt:int,cached:int)->int {return cb_step_ns(prompt-cached)/1000;}
rr_cost :: (prompt:int,output:int,cached:int)->int {return (cb_step_ns(prompt-cached)+(output-1)*cb_step_ns(1))/1000;}
// Predicted microseconds before a new request could start here: the rest
// of the start, the rest of the running request, and the queued work.
rr_wait :: (rep:[]RrReplica,k:int,now:int)->int {
    w:=rep[k].work;
    if rep[k].phase==RrPhase.starting {w+=rep[k].ready_at-now;}
    if rep[k].running>=0 {w+=rep[k].busy_until-now;}
    return w;
}
```

For a prompt of 13 tokens, 2 output tokens and pages of 4 positions:

| Cached blocks | Rows run | Prefill (ns) | Total (µs) |
| --- | --- | --- | --- |
| 0 | 13 | 17,792,472 + 12 × 1,459,182 = 35,302,656 | (35,302,656 + 17,792,472) / 1000 = 53,095 |
| 2 | 5 | 17,792,472 + 4 × 1,459,182 = 23,629,200 | 41,421 |
| 3 | 1 | 17,792,472 | 35,584 |

At most 3 blocks can be reused: chapter 176's lookup never reuses the
block that holds the last prompt position, because that position must run
to produce the first token, and (13 − 1) / 4 = 3. The checks pin these
numbers:

<!-- source: psionic/lib/serve-route.coax check-cost -->
```jai
#test "costs follow chapter 175's step model: one step of prefill rows, then one per output token" {
    // 17,792,472 ns for one row, 1,459,182 ns per further row.
    #assert rr_cost(13,2,0)==53095 && rr_cost(13,2,8)==41421 && rr_cost(13,2,12)==35584;
    #assert rr_prefill(13,0)==35302 && rr_prefill(13,12)==17792 && rr_cost(1,1,0)==17792;
}
```

## The router

The router first refuses what it must. A request outside the bounds (an
unknown client, a prompt of 0 or more than 32 tokens, an output of 0 or
more than 64) is refused as `shape`. A client that already holds `share`
requests queued or running is refused as `client`: that is the fairness
rule, and it keeps one client from filling every queue. If no replica is
running or starting the refusal is `cold`; if every one has a full queue it
is `full`.

Otherwise it predicts, for every replica with room, the time from now
until the request would finish there: the rest of the replica's start if
it is still starting, the rest of the request it is serving, the predicted
cost of its queue, and the request's own cost after the blocks that
replica's trie holds. It takes the smallest, and the lowest-numbered
replica on a tie. Two baselines share the code. `least` makes the same
prediction blind to the cache. `round` takes the next replica with room in
turn.

<!-- source: psionic/lib/serve-route.coax route -->
```jai
// Refuse a request outside the bounds or over its client's share (held: each
// client's requests queued or running); otherwise take the replica, running or
// starting with room in its queue, with the smallest predicted finish, the
// lowest number on a tie. Round robin takes the first with room from turn.
rr_route :: (rep:[]RrReplica,cfg:RrConfig,r:RrRequest,tok:[]int,now:int,held:[8]int,turn:int)->RrPick {
    pick:=RrPick.{refusal=RrRefusal.shape,replica=-1,turn=turn};
    if r.client<0 || r.client>=cfg.clients || r.prompt<1 || r.prompt>PG_PROMPT || r.output<1 || r.output>RR_OUTPUT {return pick;}
    pick.refusal=RrRefusal.client;
    if held[r.client]>=cfg.share {return pick;}
    pick.refusal=RrRefusal.cold;
    for k:0..<rep.count {if rep[k].phase!=RrPhase.off {pick.refusal=RrRefusal.full;}}
    for j:0..<rep.count {
        k:=j;
        if cfg.policy==RrPolicy.round {k=(turn+j)%rep.count;}
        if rep[k].phase==RrPhase.off || rep[k].nq>=cfg.depth {continue;}
        c:=0;
        if cfg.policy==RrPolicy.affinity {c=pg_match(rep[k].st,0,tok,r.prompt).n;}
        e:=rr_wait(rep,k,now)+rr_cost(r.prompt,r.output,c*cfg.pages.page);
        if pick.replica<0 || e<pick.est {pick=RrPick.{replica=k,cached=c,est=e,turn=(k+1)%rep.count};}
        if cfg.policy==RrPolicy.round {return pick;}
    }
    return pick;
}
```

The router reads each replica's trie with chapter 176's `pg_match`, the
longest cached chain of whole blocks under model 0. When a replica takes a
request off its queue, the simulation serves its prompt through the same
trie: the cached blocks' frames are held, the rest get new frames, every
whole block is committed, and the least recently used blocks make room.
Each replica uses chapter 176's device tier only. A full device drops its
least recently used block instead of moving it to host or disk, and since
no model runs, no frame is ever copied or written.

<!-- source: psionic/lib/serve-route.coax cache -->
```jai
// An empty trie and free frames, as chapter 176 starts them.
rr_store :: (c:PgConfig)->PgStore {
    s:PgStore;s.cfg=c;s.pool=pg_pool(c);
    for n:0..<PG_NODES {s.node[n].frame=-1;}
    for i:0..<PG_REQUESTS {s.live[i]=-1;}
    return s;
}
RrCached :: struct {st:PgStore;reused:int;}
// Serve a prompt's blocks through chapter 176's trie, device tier only:
// hold the cached prefix's frames (a frame a request holds is never
// moved), give the rest new frames, commit every whole block, and let go
// of the request's table. The trie's least recently used blocks make room.
rr_cache :: (from:PgStore,mem:[]f64,tok:[]int,prompt:int,stamp:int)->RrCached {
    s:=from;p:=s.cfg.page;path:=pg_match(s,0,tok,prompt);
    for k:0..<prompt/p {
        if k<path.n {f:=s.node[path.node[k]].frame;s.pool=pg_retain(s.pool,f);s.table[0].page[k]=f;}
        else {s=pg_take(s,mem);s.table[0].page[k]=s.pool.got;}
    }
    s.table[0].n=prompt/p;
    for k:0..<prompt/p {s=pg_commit(s,0,k,0,tok,stamp);}
    return RrCached.{st=pg_free_table(s,0),reused=path.n};
}
```

The blocks a request reuses are counted when it starts, not when it is
routed. Between the two the trie can gain blocks or lose them.

## A trace by hand

Three replicas, pages of 4 positions, prompt tokens 1 to 32 for prefix A.
Replica 0 has served A's first 8 tokens, so its trie holds 2 blocks.
Replica 1 has served A's first 12, 3 blocks, and has 10 ms of work queued.
Replica 2 holds nothing. The request is A's first 13 tokens with 2 output
tokens, from client 0.

| Replica | Wait (µs) | Cached | Cost (µs) | Finish (µs) |
| --- | --- | --- | --- | --- |
| 0 | 0 | 2 | 41,421 | 41,421 |
| 1 | 10,000 | 3 | 35,584 | 45,584 |
| 2 | 0 | 0 | 53,095 | 53,095 |

Replica 0 wins. With replica 1's work cut to 5 ms its finish is
5,000 + 35,584 = 40,584 and it wins instead. Blind to the cache, replicas
0 and 2 tie at 53,095 and the lower number goes. If replica 1 is still
starting and ready in 1,000 µs, its finish is 1,000 + 5,000 + 35,584 =
41,584 and replica 0 wins; at 800 µs it is 41,384 and replica 1 wins. With
replica 1 off and replica 0 busy until 12,000 µs, replica 0's finish at
time 0 is 12,000 + 41,421 = 53,421 and replica 2 wins with 53,095; at time
2,000 replica 0's is 10,000 + 41,421 = 51,421 and it wins back.

<!-- source: psionic/lib/serve-route.coax check-hand -->
```jai
#test "the hand-traced routes" {
    f:=rrx_hand();cfg:=rrx_hand_config();tok:=rrx_hand_tok();
    // Replica 0 holds A's first 2 blocks and is idle; replica 1 holds 3 and has 10 ms of work; replica 2 is empty.
    #assert pg_match(f[0].st,0,tok[..],13).n==2 && pg_match(f[1].st,0,tok[..],13).n==3 && pg_match(f[2].st,0,tok[..],13).n==0;
    r:=RrRequest.{client=0,prompt=13,output=2};
    p:=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);#assert p.refusal==RrRefusal.none && p.replica==0 && p.cached==2 && p.est==41421;
    // With 5 ms of work, replica 1's 5,000 + 35,584 beats replica 0's 41,421.
    f[1].work=5000;p=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);#assert p.replica==1 && p.cached==3 && p.est==40584;
    // Blind to the cache, the idle replicas tie at 53,095 and the lowest goes; in turn, replica 2 is next after 1.
    c:=cfg;c.policy=RrPolicy.least;p=rr_route(f[..],c,r,tok[..],0,rrx_held(0),0);#assert p.replica==0 && p.cached==0 && p.est==53095;
    c.policy=RrPolicy.round;p=rr_route(f[..],c,r,tok[..],0,rrx_held(0),2);#assert p.replica==2 && p.turn==0 && p.est==53095;
    // Replica 1 still starting: 1,000 us to go costs it the request; 800 does not.
    f[1].phase=RrPhase.starting;f[1].ready_at=1000;p=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);#assert p.replica==0 && p.est==41421;
    f[1].ready_at=800;p=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);#assert p.replica==1 && p.est==41384;
    // Replica 0 busy until 12 ms: at 0 replica 2 wins, at 2 ms replica 0 does again.
    f[1].phase=RrPhase.off;f[0].running=7;f[0].busy_until=12000;
    p=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);#assert p.replica==2 && p.est==53095;
    p=rr_route(f[..],cfg,r,tok[..],2000,rrx_held(0),0);#assert p.replica==0 && p.est==51421;
}
```

## The autoscaler

Chapter 178's receipt measured two cold starts of the same 256 MiB of
weights with an empty file cache on coderos-4080:

<!-- source: psionic/lib/serve-route.coax cold -->
```jai
// Measured cold starts on coderos-4080, chapter 178's receipt
// (psionic/fixtures/startup/coderos-4080.json, 256 MiB of weights, an empty
// file cache): 520,740 us when a verification record lets the start skip
// hashing ("cold, record"), 1,264,892 us when every byte is hashed ("cold,
// no record").
RR_COLD_RECORD_US :: 520740;
RR_COLD_HASH_US :: 1264892;
```

The autoscaler runs at every tick. It sees the replicas ready and
starting, the load (requests queued or running, plus requests refused as
`cold` since the last tick), the predicted work the replicas hold, and how
many ticks in a row the load has been low or high.

<!-- source: psionic/lib/serve-route.coax load-types -->
```jai
// floor and budget: fewest and most replicas. target: requests in flight per
// replica. band: percent of hysteresis around the target. down: consecutive
// low ticks before one replica is removed. cold_us: the measured start cost.
RrScaler :: struct {on:bool;floor:int;budget:int;target:int;band:int;down:int;cold_us:int;tick_us:int;}
// What the autoscaler sees at a tick: replicas ready and starting, requests
// queued or running, the predicted work they hold, and the low and high
// ticks in a row so far.
RrLoad :: struct {ready:int;starting:int;load:int;backlog:int;low:int;high:int;}
RrAct :: enum u8 {hold;up;down;}
// steady: inside the band. floor: below the floor. load: the load asks for
// it. cold: a new replica would start after the backlog has drained, and
// the load has not been high for as long as a cold start.
// budget: more are wanted than the budget allows. delay: low, but not for
// long enough yet.
RrWhy :: enum u8 {steady;floor;load;cold;budget;delay;}
RrScale :: struct {act:RrAct;count:int;why:RrWhy;low:int;high:int;}
```

The rule has a band of hysteresis around the target. With `n` replicas
provisioned (ready or starting), the load is high when
`load × 100 > n × target × (100 + band)`. It is low when it would fit one
replica fewer below the band, `load × 100 ≤ (n − 1) × target × (100 − band)`.
Between the two nothing changes. A high load asks for
`ceil(load / target)` replicas, capped by the budget. The measured start
cost enters once: if the backlog would drain on the replicas already
provisioned before a new one finished starting (`backlog ≤ n × cold_us`),
the autoscaler holds, until the load has been high for as long as a cold
start. A low load must persist for `down` ticks before one replica stops.
Below the floor, the autoscaler goes straight to it.

<!-- source: psionic/lib/serve-route.coax autoscale -->
```jai
// Up when the load is above the band over the target: to the replicas the
// load asks for, within the budget, unless the predicted backlog drains on
// the replicas already provisioned before a new one would finish starting
// and the load has been high for less than a cold start.
// Down by one when the load fits one replica fewer below the band, after
// that has held for `down` ticks in a row. Never below the floor.
rr_scale :: (c:RrScaler,o:RrLoad)->RrScale {
    n:=o.ready+o.starting;d:=RrScale.{why=RrWhy.floor};
    if n<c.floor {d.act=RrAct.up;d.count=c.floor-n;return d;}
    d.why=RrWhy.budget;
    if o.load*100>n*c.target*(100+c.band) {
        d.high=o.high+1;want:=cb_min((o.load+c.target-1)/c.target,c.budget);
        if want<=n {return d;}
        d.why=RrWhy.cold;
        if n>0 && o.backlog<=n*c.cold_us && d.high*c.tick_us<c.cold_us {return d;}
        d.act=RrAct.up;d.count=want-n;d.why=RrWhy.load;d.high=0;
        return d;
    }
    d.why=RrWhy.steady;
    if n>c.floor && o.load*100<=(n-1)*c.target*(100-c.band) {
        d.low=o.low+1;d.why=RrWhy.delay;
        if d.low<c.down {return d;}
        d.act=RrAct.down;d.count=1;d.low=0;d.why=RrWhy.load;
    }
    return d;
}
```

Worked with floor 1, budget 4, target 2, band 25, down after 3 ticks,
ticks of 100 ms and the 520,740 µs start:

- 2 replicas are high above 2 × 2 × 1.25 = 5 requests. A load of 6 asks
  for ceil(6 / 2) = 3 replicas, one more; a load of 7 asks for 4, two more.
- A backlog of 2 × 520,740 = 1,041,480 µs or less drains before a new
  replica is up, so the first high tick holds as `cold`. The sixth high
  tick in a row goes up anyway: 6 × 100,000 µs ≥ 520,740 µs. With the
  1,264,892 µs start it takes the thirteenth.
- 2 replicas are low at or below 1 × 2 × 0.75 = 1.5 requests. A load of 1
  holds as `delay` on the first and second low ticks and stops one replica
  on the third; a load of 2 resets the count.
- At budget 4, 4 replicas with a load of 20 hold as `budget`.
- With floor 0, a fleet at zero that sees one request refused as `cold`
  starts one replica, whatever the start costs.

<!-- source: psionic/lib/serve-route.coax check-scale -->
```jai
#test "the hand-worked autoscaler ticks" {
    s:=rrx_scaler();  // floor 1, budget 4, target 2, band 25, down after 3 ticks, cold start 520,740 us
    // 2 replicas: up above 2 * 2 * 1.25 = 5 requests, down at or below 1 * 2 * 0.75 = 1.5.
    d:=rr_scale(s,RrLoad.{ready=2,load=5,backlog=9000000});#assert d.act==RrAct.hold && d.why==RrWhy.steady && d.low==0;
    d=rr_scale(s,RrLoad.{ready=2,load=6,backlog=9000000});#assert d.act==RrAct.up && d.count==1 && d.why==RrWhy.load;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=9000000});#assert d.act==RrAct.up && d.count==2 && d.why==RrWhy.load;
    // A backlog of 2 * 520,740 us or less drains before a new replica would be up:
    // hold, until the load has been high for 6 ticks of 100 ms, longer than the start.
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1041480});#assert d.act==RrAct.hold && d.why==RrWhy.cold && d.high==1;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1041481,high=3});#assert d.act==RrAct.up && d.count==2 && d.high==0;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1041480,high=4});#assert d.act==RrAct.hold && d.high==5;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1041480,high=5});#assert d.act==RrAct.up && d.count==2;
    // A start that hashes every byte, 1,264,892 us, waits 13 ticks.
    s.cold_us=RR_COLD_HASH_US;d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1041481,high=11});#assert d.act==RrAct.hold && d.why==RrWhy.cold;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=2529784,high=11});#assert d.act==RrAct.hold;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=2529785,high=11});#assert d.act==RrAct.up;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1041481,high=12});#assert d.act==RrAct.up;
    s=rrx_scaler();
    // Starting replicas count: 3 provisioned hold 7.
    d=rr_scale(s,RrLoad.{ready=2,starting=1,load=7,backlog=9000000});#assert d.act==RrAct.hold && d.why==RrWhy.steady;
    // The budget caps the count, then refuses to grow.
    d=rr_scale(s,RrLoad.{ready=3,load=20,backlog=9000000});#assert d.act==RrAct.up && d.count==1;
    d=rr_scale(s,RrLoad.{ready=4,load=20,backlog=9000000,high=2});#assert d.act==RrAct.hold && d.why==RrWhy.budget && d.high==3;
    d=rr_scale(s,RrLoad.{ready=4,load=8,high=2});#assert d.act==RrAct.hold && d.why==RrWhy.steady && d.high==0;
    // Down only after 3 low ticks in a row.
    d=rr_scale(s,RrLoad.{ready=2,load=1});#assert d.act==RrAct.hold && d.why==RrWhy.delay && d.low==1;
    d=rr_scale(s,RrLoad.{ready=2,load=1,low=1});#assert d.act==RrAct.hold && d.low==2;
    d=rr_scale(s,RrLoad.{ready=2,load=1,low=2});#assert d.act==RrAct.down && d.count==1 && d.why==RrWhy.load && d.low==0;
    d=rr_scale(s,RrLoad.{ready=2,load=2,low=2});#assert d.act==RrAct.hold && d.why==RrWhy.steady && d.low==0;
    // Never below the floor; below it, up to it at once.
    d=rr_scale(s,RrLoad.{ready=1,load=0,low=5});#assert d.act==RrAct.hold && d.why==RrWhy.steady;
    d=rr_scale(s,RrLoad.{load=0});#assert d.act==RrAct.up && d.count==1 && d.why==RrWhy.floor;
    // Floor 0: one replica goes after the delay, and the first request starts one, cold or not.
    s.floor=0;d=rr_scale(s,RrLoad.{ready=1,load=0,low=2});#assert d.act==RrAct.down;
    d=rr_scale(s,RrLoad.{load=1,backlog=1});#assert d.act==RrAct.up && d.count==1 && d.why==RrWhy.load;
    d=rr_scale(s,RrLoad.{load=0});#assert d.act==RrAct.hold && d.why==RrWhy.steady;
    // Exact edges: 200 > 1 * 1 * 199 is high; 13,900 <= 7 * 20 * 99 = 13,860 is not low.
    x:=RrScaler.{on=true,floor=0,budget=8,target=1,band=99,down=1,cold_us=0,tick_us=1};
    d=rr_scale(x,RrLoad.{ready=1,load=2});#assert d.act==RrAct.up && d.count==1;
    x.target=20;x.band=1;d=rr_scale(x,RrLoad.{ready=8,load=139});#assert d.act==RrAct.hold && d.why==RrWhy.steady;
    d=rr_scale(x,RrLoad.{ready=8,load=138});#assert d.act==RrAct.down;
    // Six high ticks of 100,000 us reach a 600,000 us start exactly.
    s.cold_us=600000;d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1,high=5});#assert d.act==RrAct.up;
    d=rr_scale(s,RrLoad.{ready=2,load=7,backlog=1,high=4});#assert d.act==RrAct.hold;
}
```

When the autoscaler stops a replica, it picks one with nothing queued or
running, a starting one before a ready one. If every replica is busy, it
waits for the next tick. A stopped replica's cache is gone; a started one
begins empty.

<!-- source: psionic/lib/serve-route.coax tick -->
```jai
// The autoscaler's tick. The load is the requests queued or running plus
// those refused since the last tick because nothing was running or
// starting, so a fleet at zero sees its demand. Up starts the
// lowest-numbered off replicas, each ready after the cold start with an
// empty cache. Down stops one replica with nothing queued or running, a
// starting one first, else the highest ready one; when every replica is
// busy the step waits for the next tick.
rr_tick :: (rep:[]RrReplica,sc:RrScaler,cfg:RrConfig,now:int,low:int,high:int,cold:int)->RrScale {
    o:=RrLoad.{low=low,high=high,load=cold};
    for k:0..<rep.count {
        if rep[k].phase==RrPhase.ready {o.ready+=1;}
        if rep[k].phase==RrPhase.starting {o.starting+=1;}
        if rep[k].phase!=RrPhase.off {o.backlog+=rr_wait(rep,k,now);o.load+=rep[k].nq;}
        if rep[k].running>=0 {o.load+=1;}
    }
    d:=rr_scale(sc,o);
    if d.act==RrAct.up {
        left:=d.count;
        for k:0..<rep.count {
            if left>0 && rep[k].phase==RrPhase.off {
                rep[k]=RrReplica.{phase=RrPhase.starting,ready_at=now+sc.cold_us,running=-1,st=rr_store(cfg.pages)};
                left-=1;
            }
        }
    }
    if d.act==RrAct.down {
        v:=-1;
        for k:0..<rep.count {
            if rep[k].phase==RrPhase.off || rep[k].nq>0 || rep[k].running>=0 {continue;}
            if v<0 || rep[v].phase==RrPhase.ready {v=k;}
        }
        if v<0 {d.act=RrAct.hold;d.low=sc.down-1;return d;}
        rep[v].phase=RrPhase.off;
    }
    return d;
}
```

## The simulation

The simulation keeps, for every event time, a fixed order: requests
finish, started replicas become ready, arrivals are routed and queued or
refused, idle ready replicas take their queue's head, and the autoscaler
ticks. It ends when every request is done and, with the autoscaler on,
the fleet is back at its floor. It refuses a configuration out of bounds
(queues deeper than 8, more than 8 replicas or clients, a device that
cannot hold a 32-token prompt, a host or disk tier, a hybrid model) and
arrivals out of order, and it stops with `events` when it takes more event
times than the configuration allows.

<!-- source: psionic/lib/serve-route.coax sim -->
```jai
// A discrete-event simulation. At each event time, in this order: requests
// finish; started replicas become ready; arrivals are routed and queued or
// refused; idle ready replicas take their queue's head; the autoscaler
// ticks. It ends when every request is done and, with the autoscaler on,
// the fleet is back at its floor. (A queue waits on a running or starting
// replica, whose events keep the loop going.)
rr_sim :: (cfg:RrConfig,sc:RrScaler,reqs:[]RrRequest,tok:[]int)->RrSim {
    s:RrSim;s.status=rr_check(cfg,sc,reqs,tok);mem:[1728]f64;
    if s.status!=RrStatus.ok {return s;}
    rep:[8]RrReplica;
    for k:0..<RR_REPLICAS {
        rep[k]=RrReplica.{running=-1,st=rr_store(cfg.pages)};
        if k<cfg.replicas {rep[k].phase=RrPhase.ready;}
    }
    s.n=reqs.count;s.peak=cfg.replicas;
    a:=0;now:=0;tick:=sc.tick_us;held:[8]int;turn:=0;low:=0;high:=0;cold:=0;
    while true {
        t:=RR_NEVER;busy:=a<s.n;live:=0;
        if a<s.n {t=reqs[a].arrive;}
        for k:0..<RR_REPLICAS {
            if rep[k].phase!=RrPhase.off {live+=1;}
            if rep[k].running>=0 {t=cb_min(t,rep[k].busy_until);busy=true;}
            if rep[k].phase==RrPhase.starting {t=cb_min(t,rep[k].ready_at);}
        }
        if sc.on && (busy || live>sc.floor) {t=cb_min(t,tick);}
        if t==RR_NEVER {break;}
        s.events+=1;
        if s.events>cfg.events {s.status=RrStatus.events;return s;}
        s.replica_us+=live*(t-now);now=t;s.end=now;
        for k:0..<RR_REPLICAS {
            if rep[k].running>=0 && rep[k].busy_until==now {held[reqs[rep[k].running].client]-=1;rep[k].running=-1;}
            if rep[k].phase==RrPhase.starting && rep[k].ready_at==now {rep[k].phase=RrPhase.ready;}
        }
        while a<s.n && reqs[a].arrive==now {
            r:=reqs[a];
            p:=rr_route(rep[..],cfg,r,tok[a*PG_PROMPT..<a*PG_PROMPT+PG_PROMPT],now,held,turn);
            s.fate[a]=RrFate.{refusal=p.refusal,replica=p.replica,start=-1,first=-1,finish=-1};
            if p.refusal!=RrRefusal.none {s.refused[cast(int)p.refusal]+=1;if p.refusal==RrRefusal.cold {cold+=1;}}
            else {
                k:=p.replica;turn=p.turn;held[r.client]+=1;s.fate[a].pred=rr_cost(r.prompt,r.output,p.cached*cfg.pages.page);
                rep[k].queue[rep[k].nq]=a;rep[k].nq+=1;rep[k].work+=s.fate[a].pred;
                s.deepest=cb_max(s.deepest,rep[k].nq);
            }
            a+=1;
        }
        for k:0..<RR_REPLICAS {
            if rep[k].phase==RrPhase.ready && rep[k].running<0 && rep[k].nq>0 {
                i:=rep[k].queue[0];u:=rr_serve(rep[..],s.fate[..],k,reqs[i],tok[i*PG_PROMPT..<i*PG_PROMPT+PG_PROMPT],mem[..],now);
                s.hit+=u;s.blocks+=(reqs[i].prompt-1)/cfg.pages.page;
            }
        }
        if sc.on && now==tick {
            d:=rr_tick(rep[..],sc,cfg,now,low,high,cold);low=d.low;high=d.high;cold=0;tick+=sc.tick_us;s.why[cast(int)d.why]+=1;
            if d.act==RrAct.up {s.ups+=d.count;s.peak=cb_max(s.peak,live+d.count);}
            if d.act==RrAct.down {s.downs+=1;}
        }
    }
    return s;
}
// Replica k starts the head of its queue: its cached blocks are counted
// now, when it runs, not when it was routed. Returns the blocks reused.
rr_serve :: (rep:[]RrReplica,fate:[]RrFate,k:int,r:RrRequest,tok:[]int,mem:[]f64,now:int)->int {
    i:=rep[k].queue[0];
    for j:1..<rep[k].nq {rep[k].queue[j-1]=rep[k].queue[j];}
    rep[k].nq-=1;rep[k].work-=fate[i].pred;
    c:=rr_cache(rep[k].st,mem,tok,r.prompt,now);
    rep[k].st=c.st;u:=c.reused*c.st.cfg.page;
    fate[i].reused=c.reused;fate[i].start=now;fate[i].first=now+rr_prefill(r.prompt,u);
    fate[i].finish=now+rr_cost(r.prompt,r.output,u);
    rep[k].running=i;rep[k].busy_until=fate[i].finish;
    return c.reused;
}
```

The traffic comes from chapter 172's load schedule: `content` picks one of
the shared prefixes, `sampling` the client, `input` the prompt length and
`output` the output length. The recorded traffic is chapter 172's recorded
schedule against the gpt-oss server,
`psionic/fixtures/bench/2026-10-29/gpt-115.sched`, replayed ten times as
fast through `bn_replay`; there, requests with the same content share
their first 8 tokens. Percentiles are chapter 172's nearest-rank
`bn_stats`.

## Run it

```shell
coax/bin/coax run psionic/examples/serve-route.coax
```

The example, [`serve-route.coax`](../examples/serve-route.coax), prints
the hand trace, runs the three routers on 160 synthetic requests at two
loads and on the recorded schedule, then runs the autoscaler from one
replica with each measured start. Each synthetic request has a 32-token
prompt whose first 24 tokens come from one of 6 shared prefixes, and 4 or
8 output tokens. Each of the 4 replicas has 16 device pages of 4
positions: room for two prompts.

<!-- source: psionic/examples/serve-route.coax main -->
```jai
main :: () {
    f:=rrx_hand();cfg:=rrx_hand_config();tok:=rrx_hand_tok();
    r:=RrRequest.{client=0,prompt=13,output=2};
    p:=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);
    print("hand trace: replica %, % cached blocks, predicted finish % us\n",p.replica,p.cached,p.est);
    f[1].work=5000;p=rr_route(f[..],cfg,r,tok[..],0,rrx_held(0),0);
    print("replica 1 with 5 ms of work: replica %, % cached blocks, % us\n",p.replica,p.cached,p.est);

    w:=rrx_workload();reqs:[160]RrRequest;toks:[5120]int;
    for gap:[2]int.[60000,40000] {
        w.gap_us=gap;
        if rr_traffic(w,24,reqs[..],toks[..])!=BnStatus.ok {print("traffic refused\n");return;}
        print("synthetic, mean gap % ms: 160 requests, 6 shared prefixes of 24 tokens, 4 replicas (%)\n",gap/1000,RR_PROFILE);
        routers(reqs[..],toks[..],4);
    }
    rec:=RRX_RECORDED;
    if rr_recorded(rec[..],1000,8,reqs[..],toks[..])!=BnStatus.ok {print("recording refused\n");return;}
    print("recorded (chapter 172, 8 requests at 10 times speed), 2 replicas:\n");
    routers(reqs[0..<8],toks[..],2);

    if rr_traffic(w,24,reqs[..],toks[..])!=BnStatus.ok {return;}
    for cold:[2]int.[RR_COLD_RECORD_US,RR_COLD_HASH_US] {
        sc:=rrx_scaler();sc.budget=6;sc.cold_us=cold;c:=rrx_fleet_config(RrPolicy.affinity);c.replicas=1;
        s:=rr_sim(c,sc,reqs[..],toks[..]);
        print("autoscaled from 1 replica, cold start % us: started %, stopped %, at most %, % replica-seconds, last event at ",cold,s.ups,s.downs,s.peak,s.replica_us/1000000);
        ms(s.end);print(" ms\n  ticks:");
        for k:0..<6 {print(" % %",s.why[k],rr_why_name(cast(RrWhy)cast(u8)k));}
        print("\n");
        line("prefix affinity",s,reqs[..]);
    }
}
```

```text
hand trace: replica 0, 2 cached blocks, predicted finish 41421 us
replica 1 with 5 ms of work: replica 1, 3 cached blocks, 40584 us
synthetic, mean gap 60 ms: 160 requests, 6 shared prefixes of 24 tokens, 4 replicas (rr-route-v1)
  prefix affinity: served 160, refused 0; reused 648 of 1120 blocks (57 percent)
    first token P50 28.0 P95 63.0 ms; latency P50 152.5 P95 187.5 ms
  least work: served 160, refused 0; reused 314 of 1120 blocks (28 percent)
    first token P50 63.0 P95 63.0 ms; latency P50 152.5 P95 187.5 ms
  round robin: served 160, refused 0; reused 316 of 1120 blocks (28 percent)
    first token P50 63.0 P95 63.0 ms; latency P50 152.5 P95 187.5 ms
synthetic, mean gap 40 ms: 160 requests, 6 shared prefixes of 24 tokens, 4 replicas (rr-route-v1)
  prefix affinity: served 160, refused 0; reused 476 of 1120 blocks (42 percent)
    first token P50 63.0 P95 200.2 ms; latency P50 179.8 P95 310.5 ms
  least work: served 160, refused 0; reused 342 of 1120 blocks (30 percent)
    first token P50 70.9 P95 216.9 ms; latency P50 187.5 P95 311.5 ms
  round robin: served 160, refused 0; reused 316 of 1120 blocks (28 percent)
    first token P50 122.8 P95 295.8 ms; latency P50 219.3 P95 400.9 ms
recorded (chapter 172, 8 requests at 10 times speed), 2 replicas:
  prefix affinity: served 8, refused 0; reused 8 of 32 blocks (25 percent)
    first token P50 28.0 P95 51.3 ms; latency P50 318.2 P95 602.9 ms
  least work: served 8, refused 0; reused 6 of 32 blocks (18 percent)
    first token P50 28.0 P95 63.0 ms; latency P50 318.2 P95 614.5 ms
  round robin: served 8, refused 0; reused 8 of 32 blocks (25 percent)
    first token P50 28.0 P95 51.3 ms; latency P50 318.2 P95 602.9 ms
autoscaled from 1 replica, cold start 520740 us: started 5, stopped 5, at most 6, 27 replica-seconds, last event at 7000.0 ms
  ticks: 36 steady 0 floor 8 load 6 cold 7 budget 13 delay
  prefix affinity: served 160, refused 0; reused 412 of 1120 blocks (36 percent)
    first token P50 182.2 P95 548.8 ms; latency P50 282.4 P95 660.0 ms
autoscaled from 1 replica, cold start 1264892 us: started 5, stopped 5, at most 6, 31 replica-seconds, last event at 6800.0 ms
  ticks: 12 steady 0 floor 8 load 15 cold 21 budget 12 delay
  prefix affinity: served 160, refused 0; reused 408 of 1120 blocks (36 percent)
    first token P50 553.4 P95 1210.2 ms; latency P50 651.1 P95 1314.4 ms
```

What it shows, against round robin:

- **Light load (a request every 60 ms on average).** Prefix affinity
  reuses 648 of 1,120 reusable blocks (57 percent) against 316 (28
  percent), and its median time to first token is 28.0 ms against 63.0 ms.
  P95 time to first token and both latency percentiles are equal: at this
  load nothing queues, and decoding 4 or 8 tokens dominates the latency.
- **Heavy load (every 40 ms).** Affinity reuses 42 percent against 28. Its
  time to first token is 63.0 ms against 122.8 ms at P50 and 200.2 ms
  against 295.8 ms at P95, and its P95 latency is 310.5 ms against
  400.9 ms, 23 percent lower. Against `least`, which balances the same
  predicted work blind to the cache, the gain is smaller: P95 first token
  200.2 ms against 216.9 ms, P95 latency 310.5 ms against 311.5 ms.
- **Recorded (8 requests).** Affinity and round robin make the same
  choices; `least` reuses 6 blocks instead of 8. Eight requests are too few
  for the percentiles to say more.
- **The autoscaler.** From one replica, the 520,740 µs start reaches 6
  replicas and spends 27 replica-seconds; P95 time to first token is
  548.8 ms. The fixed fleet of 4 spent 25.3 replica-seconds for 200.2 ms:
  for this steady traffic, sizing the fleet in advance wins, because the
  autoscaler climbs while requests wait and keeps replicas through its
  scale-down delay. The 1,264,892 µs start that hashes every byte spends
  31 replica-seconds and raises P95 time to first token to 1,210.2 ms.

These numbers come from a model of one host's step times, not from
replicas that ran. The `#test`s freeze them, so a change to the router,
the queue or the autoscaler that moves any of them fails until the chapter
is updated.

## Refusals

[`serve-route-refuse.coax`](../examples/serve-route-refuse.coax) shows
each refusal: a configuration and an arrival order the simulation refuses
as a whole, three requests outside the bounds, an overload where 40
requests arrive 5 ms apart on 2 replicas with queues of 2, three of every
four from client 0 and each client allowed 4 in flight, and a fleet that
scales to zero.

<!-- source: psionic/examples/serve-route-refuse.coax main -->
```jai
main :: () {
    off:RrScaler;cfg:=rrx_sim_config();tok:[1280]int;
    q:[40]RrRequest;
    for i:0..<40 {q[i]=RrRequest.{client=i%2,arrive=i*5000,prompt=8,output=2};}
    c:=cfg;c.depth=9;
    print("depth 9: %\n",rr_status_name(rr_sim(c,off,q[0..<4],tok[..]).status));
    b:=q;b[1].arrive=-1;
    print("an arrival before the one ahead of it: %\n",rr_status_name(rr_sim(cfg,off,b[0..<4],tok[..]).status));
    b=q;b[0].prompt=33;b[1].client=2;b[2].output=0;
    print("a 33-token prompt, client 2 of 2, no output:\n");
    fates(rr_sim(cfg,off,b[0..<4],tok[..]),4);

    // 40 requests 5 ms apart from two clients, on 2 replicas with queues of
    // 2; each client may hold 4 requests in flight; 3 in 4 come from client 0.
    c=cfg;c.depth=2;c.share=4;
    for i:0..<40 {q[i].client=ifx i%4==3 then 1 else 0;}
    s:=rr_sim(c,off,q[..],tok[..]);
    print("overload, 40 requests 5 ms apart, 2 replicas, queues of 2, share 4:\n");
    for k:1..<5 {print("  refused, %: %\n",rr_refusal_name(cast(RrRefusal)cast(u8)k),s.refused[k]);}
    print("  served %, deepest queue %\n",40-s.refused[1]-s.refused[2]-s.refused[3]-s.refused[4],s.deepest);

    // Floor 0: after 3 quiet ticks of 100 ms the last replica stops. Three
    // requests at 1 s find nothing running; the tick at that instant counts
    // them as load and starts ceil(3 / 2) = 2 replicas.
    sc:=rrx_scaler();sc.floor=0;c=cfg;c.replicas=1;
    z:=[6]RrRequest.[RrRequest.{arrive=0,prompt=8,output=2},RrRequest.{arrive=1000000,prompt=8,output=2},RrRequest.{arrive=1000000,prompt=8,output=2},
        RrRequest.{arrive=1000000,prompt=8,output=2},RrRequest.{arrive=1050000,prompt=8,output=2},RrRequest.{arrive=1700000,prompt=8,output=2}];
    s=rr_sim(c,sc,z[..],tok[..]);
    print("scaled to zero, cold start % us: started %, stopped %, % replica-seconds\n",sc.cold_us,s.ups,s.downs,s.replica_us/1000000);
    fates(s,6);
}
```

```text
depth 9: configuration or traffic out of bounds
an arrival before the one ahead of it: arrivals out of order
a 33-token prompt, client 2 of 2, no output:
  request 0: refused, request outside the bounds
  request 1: refused, request outside the bounds
  request 2: refused, request outside the bounds
  request 3: replica 0, started at 15000 us
overload, 40 requests 5 ms apart, 2 replicas, queues of 2, share 4:
  refused, request outside the bounds: 0
  refused, client over its share: 4
  refused, every queue full: 22
  refused, no replica running or starting: 0
  served 14, deepest queue 2
scaled to zero, cold start 520740 us: started 2, stopped 3, 1 replica-seconds
  request 0: replica 0, started at 0 us
  request 1: refused, no replica running or starting
  request 2: refused, no replica running or starting
  request 3: refused, no replica running or starting
  request 4: replica 0, started at 1520740 us
  request 5: replica 0, started at 1700000 us
```

The overload serves 14 requests. It refuses 22 because every queue was
full and 4 because client 0 already held its share; no queue ever held
more than 2. In the last run, the replica stops after 3 quiet ticks. The
three requests at 1 s are refused as `cold`; the tick at that instant
counts them as load and starts ceil(3 / 2) = 2 replicas, the first of them
replica 0 again. It is ready at 1,520,740 µs and serves the request that
arrived at 1.05 s.

## Independent checks

- **A brute-force router.** For 200 random fleets (every phase, random
  queues, work, running requests and tries filled by up to 10 earlier
  prompts) and a random request, sometimes outside the bounds, each of the
  three policies picks exactly what a brute-force router picks. The
  brute-force router counts cached blocks with chapter 176's brute-force
  lookup, which rebuilds each cached prefix by walking to the root, and
  writes out the step model's constants by hand.
- **Autoscaler properties.** For 2,000 random configurations and loads:
  never above the budget; never below the floor once at it, and straight
  to it from below; up only when the load is above the band and the
  backlog or the high ticks justify a cold start, and never past what the
  load asks for; down only after `down` low ticks and only to a count the
  load fits below the band; and after any step, the same load does not ask
  for the step back.
- **Simulation invariants.** For 24 random traffic, fleet, policy and
  autoscaler settings, recomputed from the fates alone: refusal counts by
  reason; a client's requests in flight below its share when served and at
  it when refused as `client`; fewer than `depth` requests waiting ahead of
  each one on its replica; one request at a time per replica, in routed
  order; each request's first-token and finish times equal to the step
  model with the blocks it reused; hit counts; and the peak replica count
  within the budget.
- **Hand-worked cases.** The costs, routes and autoscaler ticks above,
  their exact edges, which replica a tick stops or starts, every refusal at
  its bound, the event limit, and the frozen runs.

`coax mutate psionic/lib/serve-route.coax --root-only` kills 571 of
571 mutants.

## What remains

- **Real replicas.** Every replica is a region of one process's memory,
  timed by a model, with chapter 176's device tier only. Separate processes or machines, a network between
  router and replica, and a live run on owner-approved machines that
  reproduces the simulated percentiles within a stated tolerance are not
  built. Hosted machines need an owner-approved budget, recorded in
  `NEEDS_OWNER.md`.
- **Batching inside a replica.** Each replica serves one request at a
  time. Running chapter 175's scheduler inside each simulated replica, and
  routing on its admission state, is not built.
- **Adapter-aware routing.** Every request uses model 0; preferring a
  replica that holds a request's adapter (chapter 140) is not built.
- **Priority classes and pull-based engines.** The queue is first in,
  first out per replica; priorities, length buckets and engines that pull
  work are not built.
- **Decode balancing by cache length.** DeepSeek's load balancers weigh
  each replica's total cache length as well as its request count; this
  router weighs predicted time only.
- **Capability admission and metering.** Chapter 173's capability check
  and chapter 180's meter are not wired into the router; refusals here are
  counted, not billed.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- Zheng et al., "SGLang: efficient execution of structured language model
  programs", 2024 (cache-aware routing to the replica with the longest
  cached prefix).
