# 190. Stage lifecycle and fencing

> **Status: partly implemented.** A stage lifecycle, fencing tokens and a
> coordinator run on the CPU against simulated stages and peers (profile
> life-v1). Every stage instance of every placement moves through a
> checked state machine; every boundary message, report and command
> carries a placement epoch and a generation, and a stage refuses any
> token that is not its own. The route moves to a new placement only when
> every stage of it is warm, the old placement drains, and when a peer
> leaves its placement breaks and each of its requests retries or fails as
> it declared. Still to build: stages on live peers, with chapter 188's
> transport, chapter 187's loader doing the fetch, verify and load, and
> heartbeats that detect a frozen or disconnected stage within a stated
> time; restoring a sequence from saved state rather than running it again
> from its prompt; and checking each peer's memory budget during a
> cutover, when an old and a new instance share it.

A split model is only as available as its least ready stage. [Chapter
189](ch189-place-the-model.md) decides which peer runs which stage, and
replans when a peer leaves. This chapter decides how a placement starts,
when it may take traffic, how it hands over to the next placement without
dropping a request, and how a stage that belongs to an old placement is
kept from doing harm.

The danger has a name: split brain. A peer stops answering, the
coordinator gives up on it and replans, but the peer was only frozen. It
wakes up and carries on sending boundary rows and answers for the old
placement. If anything downstream accepts them, two placements now serve
the same request, and one of them is working from a cache the other never
saw. The fix is a fencing token on everything that crosses between
machines, checked by the receiver.

The library is [`stage-life.coax`](../lib/stage-life.coax). It is an
`#authority` file, because it decides which stage may serve which request.
It reuses chapter 189's `PlcJob`, `PlcFleet` and `PlcPlacement` and its
solver, and chapter 186's boundary message (`stg_pack`, `stg_check`,
`stg_receive`). Its phases are chapter 178's fetch, verify and load; its
drain is chapter 179's (a request finishes on the version it started on);
its late-answer fence and exactly-once answer are chapter 181's. The
examples are [`stage-life.coax`](../examples/stage-life.coax) and
[`stage-life-refuse.coax`](../examples/stage-life-refuse.coax).

```shell
coax/bin/coax run psionic/examples/stage-life.coax
coax/bin/coax run psionic/examples/stage-life-refuse.coax
psionic/check.sh --only stage-life
```

## Epochs, instances and generations

Each placement the coordinator acts on opens a new **epoch**, numbered 1,
2, 3 and so on. An epoch holds one **instance** of each stage. An instance
is a stage on a peer for one epoch: when a placement keeps stage 1 on peer
2, the new epoch still gets a new instance there. Each instance gets a
**generation** number, counted across the whole mesh, so no two instances
ever share one. Its **token** is the pair (epoch, generation).

Each instance moves through nine states:

<!-- source: psionic/lib/stage-life.coax life -->
```jai
// One stage instance's lifecycle. assigned: placed, nothing loaded.
// fetching, verifying, loading: chapter 178's phases for its package.
// warm: loaded and checked, taking no traffic. serving: the entry route
// sends it requests. draining: no new requests; it finishes those it
// holds. retired: stopped cleanly, memory released. failed: lost or
// faulted, memory released. retired and failed are final.
LfState :: enum u8 {assigned;fetching;verifying;loading;warm;serving;draining;retired;failed;}
// fetch, hold (the peer holds the package, so it only hashes it), adopt
// (this peer already has the same stage loaded for a live epoch) and the
// coordinator's serve, drain, idle and stop; the peer reports fetched,
// verified, loaded or fault.
LfEvent :: enum u8 {fetch;hold;adopt;fetched;verified;loaded;serve;drain;idle;stop;fault;}
LfMove :: struct {ok:bool;to:LfState;}
// Event k of the first nine moves from LF_FROM[k] to LF_TO[k]. stop and
// fault move any live state to retired or failed.
LF_FROM :: [9]int.[0,0,0,1,2,3,4,5,6];
LF_TO :: [9]int.[1,2,4,2,3,4,5,6,7];
lf_live :: (s:LfState)->bool {return s!=LfState.retired && s!=LfState.failed;}
lf_move :: (s:LfState,e:LfEvent)->LfMove {
    r:=LfMove.{to=s};k:=cast(int)e;
    if !lf_live(s) {return r;}
    if e==LfEvent.stop {return LfMove.{ok=true,to=LfState.retired};}
    if e==LfEvent.fault {return LfMove.{ok=true,to=LfState.failed};}
    if cast(int)s==LF_FROM[k] {r.ok=true;r.to=cast(LfState)LF_TO[k];}
    return r;
}
```

Written out as a table, with a dash for a refused move:

| From | fetch | hold | adopt | fetched | verified | loaded | serve | drain | idle | stop | fault |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| assigned | fetching | verifying | warm | – | – | – | – | – | – | retired | failed |
| fetching | – | – | – | verifying | – | – | – | – | – | retired | failed |
| verifying | – | – | – | – | loading | – | – | – | – | retired | failed |
| loading | – | – | – | – | – | warm | – | – | – | retired | failed |
| warm | – | – | – | – | – | – | serving | – | – | retired | failed |
| serving | – | – | – | – | – | – | – | draining | – | retired | failed |
| draining | – | – | – | – | – | – | – | – | retired | retired | failed |
| retired | – | – | – | – | – | – | – | – | – | – | – |
| failed | – | – | – | – | – | – | – | – | – | – | – |

The checks hold this table as a literal, typed by hand, and compare
`lf_move` with it on every one of the 99 cells and on random sequences.

## Fencing tokens

<!-- source: psionic/lib/stage-life.coax fence -->
```jai
// A fencing token: the placement epoch and the instance's generation.
// Generations count up across the whole mesh, so no two instances share one.
LfToken :: struct {epoch:int;gen:int;}
LfRefusal :: enum u8 {none;stale;ahead;order;closed;partial;boundary;request;index;ready;busy;plan;done;peer;}
// own against got: older, the same, or newer.
lf_fence :: (own:LfToken,got:LfToken)->LfRefusal {
    if got.epoch<own.epoch || (got.epoch==own.epoch && got.gen<own.gen) {return LfRefusal.stale;}
    if got.epoch==own.epoch && got.gen==own.gen {return LfRefusal.none;}
    return LfRefusal.ahead;
}
// An instance: its stage index and peer, its own token, the token its
// upstream sends with (generation 0 for the entry), its state, and the
// stage's five numbers from chapter 189.
LfStage :: struct {index:int;peer:int;token:LfToken;up:LfToken;state:LfState;stage:PlcStage;}
LfDone :: struct {stage:LfStage;refusal:LfRefusal;}
// A command or report for an instance: the fence first, then the move.
lf_command :: (st:LfStage,token:LfToken,e:LfEvent)->LfDone {
    d:=LfDone.{stage=st,refusal=lf_fence(st.token,token)};
    if d.refusal!=LfRefusal.none {return d;}
    m:=lf_move(st.state,e);
    if !m.ok {d.refusal=LfRefusal.order;return d;}
    d.stage.state=m.to;return d;
}
// A boundary message: chapter 186's message, then a trailer of three
// words: epoch, sender generation, request. -1 when a word is out of range,
// chapter 186 refuses, or out has no room for the trailer.
lf_pack :: (token:LfToken,request:int,model:u64,layer:int,start:int,width:int,rows:[]f32,out:[]u32)->int {
    if !stg_in(token.epoch,1,LF_WORD) || !stg_in(token.gen,0,LF_WORD) || !stg_in(request,0,LF_REQUESTS-1) {return -1;}
    n:=stg_pack(model,layer,start,width,rows,out);
    if n== -1 || out.count<n+LF_FENCE {return -1;}
    out[n]=cast(u32)token.epoch;out[n+1]=cast(u32)token.gen;out[n+2]=cast(u32)request;
    return n+LF_FENCE;
}
// The receiver's checks, in order: the trailer, the sender's token
// against the upstream token it was wired with, its own state (serving or
// draining), the request, then chapter 186's checks. Nothing is written
// on a refusal.
LfGot :: struct {refusal:LfRefusal;cause:StgRefusal;request:int;count:int;}
lf_receive :: (st:LfStage,want:StgExpect,wire:[]u32,out:[]f32)->LfGot {
    g:=LfGot.{refusal=LfRefusal.partial};
    if wire.count<LF_FENCE {return g;}
    t:=wire.count-LF_FENCE;
    g.refusal=lf_fence(st.up,LfToken.{epoch=cast(int)wire[t],gen=cast(int)wire[t+1]});
    if g.refusal!=LfRefusal.none {return g;}
    g.refusal=LfRefusal.closed;
    if st.state!=LfState.serving && st.state!=LfState.draining {return g;}
    g.refusal=LfRefusal.request;
    if cast(int)wire[t+2]>=LF_REQUESTS {return g;}
    r:=stg_receive(want,wire[0..<t],out);
    g.cause=r.refusal;g.refusal=ifx r.refusal==StgRefusal.none then LfRefusal.none else LfRefusal.boundary;
    if g.refusal==LfRefusal.none {g.request=cast(int)wire[t+2];g.count=r.count;}
    return g;
}
```

Three rules follow from the code:

- **Exact match, not "at least as new".** A stage accepts a command only
  with its own token, and a boundary message only with the token of the
  upstream instance it was wired to. An older token is `stale`, a newer
  one `ahead`. A newer token reaching an old instance means the sender
  belongs to a placement this instance is not part of, so it is refused
  too.
- **The fence comes before the state.** A stale message to a stage that is
  only warm is refused as stale, not as "not serving", so the log names
  the real cause.
- **Chapter 186's message is unchanged.** The token rides in a trailer of
  three 32-bit words after it: epoch, sender generation and request. A
  receiver reads the trailer, fences, and only then hands the rest to
  `stg_receive`, which checks the model digest, the first layer, the
  position, the geometry and the checksum as before. A 4-value row is 13
  words of message and 3 of trailer: 16 words.

## The coordinator

The coordinator runs on this machine, where tokens enter and leave. It
holds every epoch still alive, the epoch the entry route admits to, and
each request's state.

<!-- source: psionic/lib/stage-life.coax mesh -->
```jai
// A request is bound to one epoch at a time. On losing it, a request that
// declared retry runs again from its prompt on the next epoch; one that
// declared fail ends failed.
LfReqState :: enum u8 {none;queued;running;answered;failed;}
LfLoss :: enum u8 {retry;fail;}
LfReq :: struct {state:LfReqState;loss:LfLoss;epoch:int;tries:int;}
LfEpoch :: struct {epoch:int;stages:int;inflight:int;stage:[8]LfStage;}
// The coordinator on this machine: the newest epoch, the one the entry
// route admits to (0: none), the last generation handed out, the epochs
// alive, and the requests.
LfMesh :: struct {epoch:int;serving:int;gen:int;slot:[4]LfEpoch;req:[32]LfReq;}
LfDo :: struct {mesh:LfMesh;refusal:LfRefusal;}
lf_slot :: (m:LfMesh,epoch:int)->int {
    for k:0..<LF_EPOCHS {if m.slot[k].epoch==epoch {return k;}}
    return -1;
}
lf_alive :: (e:LfEpoch)->bool {
    for s:0..<e.stages {if lf_live(e.stage[s].state) {return true;}}
    return false;
}
// The coordinator's own command to stage s of slot k, with its own token.
lf_to :: (m:LfMesh,k:int,s:int,e:LfEvent)->LfMesh {
    n:=m;st:=n.slot[k].stage[s];n.slot[k].stage[s]=lf_command(st,st.token,e).stage;return n;
}
lf_halt :: (m:LfMesh,k:int)->LfMesh {
    n:=m;
    for s:0..<n.slot[k].stages {if lf_live(n.slot[k].stage[s].state) {n=lf_to(n,k,s,LfEvent.stop);}}
    return n;
}
lf_same :: (a:PlcStage,b:PlcStage)->bool {return a.bytes==b.bytes && a.read==b.read && a.macs==b.macs && a.package==b.package && a.out==b.out;}
// Has this peer the same stage loaded for another live epoch?
lf_loaded :: (m:LfMesh,k:int,s:int)->bool {
    st:=m.slot[k].stage[s];
    for q:0..<LF_EPOCHS {
        o:=m.slot[q].stage[s];
        // After lf_plan stops a warming epoch, every other live instance serves or drains.
        if q!=k && s<m.slot[q].stages && o.peer==st.peer && lf_same(o.stage,st.stage) && lf_live(o.state) {return true;}
    }
    return false;
}
// Bind request r to the serving epoch, or queue it while there is none.
lf_bind :: (m:LfMesh,r:int)->LfMesh {
    n:=m;n.req[r].state=LfReqState.queued;n.req[r].epoch=0;
    if n.serving==0 {return n;}
    k:=lf_slot(n,n.serving);
    n.req[r].state=LfReqState.running;n.req[r].epoch=n.serving;n.req[r].tries+=1;n.slot[k].inflight+=1;
    return n;
}
// A draining epoch with nothing in flight retires. (Only a draining epoch
// has draining stages.)
lf_settle :: (m:LfMesh,k:int)->LfMesh {
    n:=m;
    if n.slot[k].inflight>0 {return n;}
    for s:0..<n.slot[k].stages {if n.slot[k].stage[s].state==LfState.draining {n=lf_to(n,k,s,LfEvent.idle);}}
    return n;
}
// An epoch breaks: its live stages stop and release their memory, the
// route is withdrawn if it was this epoch's, and its requests retry or fail.
lf_break :: (m:LfMesh,k:int)->LfMesh {
    n:=lf_halt(m,k);E:=n.slot[k].epoch;
    if n.serving==E {n.serving=0;}
    n.slot[k].inflight=0;
    for r:0..<LF_REQUESTS {
        q:=n.req[r];
        if q.state!=LfReqState.running || q.epoch!=E {continue;}
        if q.loss==LfLoss.fail {n.req[r].state=LfReqState.failed;continue;}
        n=lf_bind(n,r);
    }
    return n;
}
// A new placement opens a new epoch. A newer plan stops an epoch still
// warming. Each stage's first step: adopt the same stage loaded on the
// same peer, hash a package the peer holds, or fetch.
lf_plan :: (m:LfMesh,j:PlcJob,f:PlcFleet,p:PlcPlacement)->LfDo {
    d:=LfDo.{mesh=m,refusal=LfRefusal.plan};
    if p.status!=PlcStatus.ok || p.stages!=j.stages {return d;}
    for s:0..<j.stages {if !stg_in(p.peer[s],0,f.peers-1) || !f.peer[p.peer[s]].live {return d;}}
    n:=m;
    for k:0..<LF_EPOCHS {if n.slot[k].epoch>n.serving {n=lf_halt(n,k);}}
    free:=-1;
    for k:0..<LF_EPOCHS {if free<0 && !lf_alive(n.slot[k]) {free=k;}}
    d.refusal=LfRefusal.busy;
    if free<0 {return d;}
    E:=n.epoch+1;e:=LfEpoch.{epoch=E,stages=j.stages};
    for s:0..<j.stages {
        n.gen+=1;
        e.stage[s]=LfStage.{index=s,peer=p.peer[s],token=LfToken.{epoch=E,gen=n.gen},up=LfToken.{epoch=E,gen=ifx s==0 then 0 else n.gen-1},stage=j.stage[s]};
    }
    n.epoch=E;n.slot[free]=e;
    for s:0..<j.stages {
        first:=ifx (f.peer[p.peer[s]].holds>>s)&1==1 then LfEvent.hold else LfEvent.fetch;
        if lf_loaded(n,free,s) {first=LfEvent.adopt;}
        n=lf_to(n,free,s,first);
    }
    d.mesh=n;d.refusal=LfRefusal.none;return d;
}
// A peer reports on its own instance: fetched, verified, loaded or fault.
// A fault breaks the instance's epoch.
lf_report :: (m:LfMesh,s:int,token:LfToken,e:LfEvent)->LfDo {
    d:=LfDo.{mesh=m,refusal=LfRefusal.order};
    if e!=LfEvent.fetched && e!=LfEvent.verified && e!=LfEvent.loaded && e!=LfEvent.fault {return d;}
    d.refusal=LfRefusal.ahead;
    if token.epoch>m.epoch {return d;}
    k:=lf_slot(m,token.epoch);d.refusal=LfRefusal.stale;
    if k<0 {return d;}
    d.refusal=LfRefusal.index;
    if !stg_in(s,0,m.slot[k].stages-1) {return d;}
    c:=lf_command(m.slot[k].stage[s],token,e);d.refusal=c.refusal;
    if c.refusal!=LfRefusal.none {return d;}
    n:=m;n.slot[k].stage[s]=c.stage;
    if e==LfEvent.fault {n=lf_break(n,k);}
    d.mesh=n;return d;
}
// Publish the newest epoch's route once every stage of it is warm. Queued
// requests go to it; the epoch that served before drains.
lf_open :: (m:LfMesh)->LfDo {
    d:=LfDo.{mesh=m,refusal=LfRefusal.order};k:=lf_slot(m,m.epoch);
    if m.serving==m.epoch {return d;}
    d.refusal=LfRefusal.ready;
    for s:0..<m.slot[k].stages {if m.slot[k].stage[s].state!=LfState.warm {return d;}}
    n:=m;
    for s:0..<n.slot[k].stages {n=lf_to(n,k,s,LfEvent.serve);}
    old:=lf_slot(n,n.serving);n.serving=n.epoch;
    if old>=0 {
        for s:0..<n.slot[old].stages {n=lf_to(n,old,s,LfEvent.drain);}
        n=lf_settle(n,old);
    }
    for r:0..<LF_REQUESTS {if n.req[r].state==LfReqState.queued {n=lf_bind(n,r);}}
    d.mesh=n;d.refusal=LfRefusal.none;return d;
}
lf_admit :: (m:LfMesh,r:int,loss:LfLoss)->LfDo {
    d:=LfDo.{mesh=m,refusal=LfRefusal.request};
    if !stg_in(r,0,LF_REQUESTS-1) {return d;}
    d.refusal=LfRefusal.done;
    if m.req[r].state!=LfReqState.none {return d;}
    n:=m;n.req[r].loss=loss;d.mesh=lf_bind(n,r);d.refusal=LfRefusal.none;return d;
}
// The exit takes an answer only from the last stage of the request's
// bound epoch, and only once.
lf_answer :: (m:LfMesh,r:int,token:LfToken)->LfDo {
    d:=LfDo.{mesh=m,refusal=LfRefusal.request};
    if !stg_in(r,0,LF_REQUESTS-1) || m.req[r].state==LfReqState.none {return d;}
    q:=m.req[r];d.refusal=LfRefusal.done;
    if q.state==LfReqState.answered || q.state==LfReqState.failed {return d;}
    d.refusal=LfRefusal.stale;
    if q.state==LfReqState.queued {return d;}
    k:=lf_slot(m,q.epoch);e:=m.slot[k];
    d.refusal=lf_fence(e.stage[e.stages-1].token,token);
    if d.refusal!=LfRefusal.none {return d;}
    n:=m;n.req[r].state=LfReqState.answered;n.slot[k].inflight-=1;
    d.mesh=lf_settle(n,k);return d;
}
// A peer leaves: every live instance on it fails, and their epochs break.
lf_lose :: (m:LfMesh,peer:int)->LfDo {
    d:=LfDo.{mesh=m,refusal=LfRefusal.peer};
    if !stg_in(peer,1,7) {return d;}
    n:=m;
    for k:0..<LF_EPOCHS {
        hit:=false;
        for s:0..<n.slot[k].stages {
            if n.slot[k].stage[s].peer==peer && lf_live(n.slot[k].stage[s].state) {n=lf_to(n,k,s,LfEvent.fault);hit=true;}
        }
        if hit {n=lf_break(n,k);}
    }
    d.mesh=n;d.refusal=LfRefusal.none;return d;
}
```

The rules, in the order a placement lives them:

1. **Plan.** `lf_plan` takes a placement from chapter 189 and opens epoch
   `E + 1`. Any epoch newer than the one serving is still warming and has
   never taken a request, so the new plan stops it. Each new instance's
   first step is chosen per stage: `adopt` if a live instance of the same
   stage (all five of chapter 189's numbers equal) is on the same peer,
   `hold` if the peer advertises the package, and `fetch` otherwise.
2. **Warm, in any order.** Peers report `fetched`, `verified` and `loaded`
   on their own instances, with their own tokens. The order across stages
   does not matter, because nothing is routed yet.
3. **Open.** `lf_open` refuses until every stage of the newest epoch is
   warm. Then, in one step, every stage of it serves, the entry route
   moves to it, every queued request is bound to it, and the epoch that
   served before is told to drain.
4. **Drain.** A draining epoch takes no new request, since the entry
   admits only to the serving epoch. It keeps every request already bound
   to it, and those finish on it. When its last one is answered, its
   stages go idle and retire.
5. **Break.** When a peer leaves (`lf_lose`) or a peer reports a fault,
   that instance fails and its epoch breaks: the other stages of it stop,
   the route is withdrawn if it was this epoch's, and each request bound
   to it either is bound again to the serving epoch (or queued until one
   opens) or ends failed, as the request declared when it was admitted.
6. **Answer.** The exit accepts an answer only with the token of the last
   stage of the request's bound epoch, and only once. A request that has
   been moved to another epoch, or is queued, refuses every answer from
   the old one.

### Who owns what

- **Weights belong to the peer.** Every live instance of the same stage on
  one peer shares one copy. A new instance adopts it instead of fetching,
  verifying and loading again. It is released when the last instance using
  it retires or fails.
- **A cache belongs to one instance**, and holds only its epoch's
  sequences. It is never handed to another epoch: the new instance starts
  empty, and a request that loses its epoch runs again from its prompt.
  Because a request's output depends only on its prompt and the weights,
  the retry gives the same answer as an uninterrupted run; the simulation
  checks this.
- **A request belongs to one epoch at a time**, and the exit's fence
  enforces it.

<!-- source: psionic/lib/stage-life.coax memory -->
```jai
// A peer's bytes: each live instance's cache (its resident bytes less its
// package), and each distinct stage's weights once, however many live
// instances share them. Retired and failed instances hold nothing.
lf_memory :: (m:LfMesh,peer:int)->int {
    t:=0;
    for k:0..<LF_EPOCHS {for s:0..<m.slot[k].stages {
        st:=m.slot[k].stage[s];
        if st.peer!=peer || !lf_live(st.state) {continue;}
        t+=st.stage.bytes-st.stage.package;
        seen:=false;
        for q:0..<k {if s<m.slot[q].stages {
            o:=m.slot[q].stage[s];
            if o.peer==peer && lf_live(o.state) && lf_same(o.stage,st.stage) {seen=true;}
        }}
        if !seen {t+=st.stage.package;}
    }}
    return t;
}
```

## A worked trace

The trace uses chapter 189's split of Ministral 3 3B and its three peers,
plus a peer 3 that has not joined yet. Peer 3 is like peer 1 with twice
its memory rate. It links to this machine like peer 1 does and to peer 2
like peer 1 does, and has no link to peer 1.

<!-- source: psionic/examples/stage-life.coax fleet -->
```jai
// Chapter 189's three peers, and peer 3, not yet joined: like peer 1 with
// twice its memory rate, linked to this machine and to peer 2.
life_fleet :: ()->PlcFleet {
    f:PlcFleet;f.peers=4;f.deadline=60000000000;
    f.peer[0]=PlcPeer.{live=true,budget=5000000000,macs=500000,read=100000,fetch=100,verify=1000,holds=3};
    f.peer[1]=PlcPeer.{live=true,budget=6000000000,macs=20000000,read=400000,fetch=1000,verify=2000};
    f.peer[2]=PlcPeer.{live=true,budget=4500000000,macs=1000000,read=200000,fetch=250,verify=500};
    f.peer[3]=PlcPeer.{live=false,budget=6000000000,macs=20000000,read=800000,fetch=1000,verify=2000};
    f=life_link(f,0,1,500000,125);f=life_link(f,0,2,2000000,25);f=life_link(f,1,2,250000,1250);
    f=life_link(f,0,3,500000,125);f=life_link(f,3,2,250000,1250);
    return f;
}
```

The two stages, from chapter 189:

| | Resident bytes | Package bytes | Cache and buffers (resident − package) |
| --- | --- | --- | --- |
| Stage 0 | 4,735,541,248 | 4,064,452,608 | 671,088,640 |
| Stage 1 | 4,203,907,072 | 3,598,866,432 | 605,040,640 |

**Epoch 1.** With peer 3 absent, chapter 189 places the stages on (1, 2).
The coordinator makes two instances: stage 0 on peer 1 with token (1, 1)
and stage 1 on peer 2 with token (1, 2). Stage 0 is wired to accept from
the entry, generation 0, and stage 1 from generation 1. Neither peer holds
a package, so both fetch. Each peer now holds its stage's resident bytes:
4,735,541,248 on peer 1 and 4,203,907,072 on peer 2. Opening is refused
until both are warm. Then both serve, and requests 0 and 1 (declared
retry) and 2 (declared fail) are bound to epoch 1. Request 0 crosses both
stages and leaves with token (1, 2): answered. An answer with (1, 1) for
request 1 is stale: that is stage 0's token, not the last stage's.

**Peer 3 joins.** A fresh plan is (3, 2): stage 0 on peer 3 costs
500,032 + ceil(3,728,908,288 × 1000 / 800,000) + 259,860 + 20,007,599 +
2,000,160 = 500,032 + 4,661,136 + 259,860 + 20,007,599 + 2,000,160 =
27,428,787 ns a token, against 32,089,922 ns on (1, 2). Epoch 2's stage 0
on peer 3 gets token (2, 3) and fetches. Its stage 1 on peer 2 gets (2, 4)
and **adopts**: epoch 1's stage 1 is serving on the same peer with the
same five numbers, so it is warm at once. Peer 2 holds the weights once and
two caches: 3,598,866,432 + 2 × 605,040,640 = 4,808,947,712 bytes.

**Cutover.** Once stage 0 is warm, epoch 2 opens. Epoch 1 drains with two
requests in flight (1 and 2), and new requests 3 (retry) and 4 (fail) go
to epoch 2. An answer for request 1 with epoch 2's token (2, 4) is
refused as ahead: request 1 is bound to epoch 1. With (1, 2) it is
answered, and epoch 1 keeps draining because request 2 is still in
flight. When request 2 is answered, epoch 1 retires: peer 1 holds 0 bytes
and peer 2 holds 4,203,907,072, epoch 2's instance alone.

**Peer 3 leaves.** It was frozen, not dead, so its stage 0 keeps running.
The coordinator fails instance (2, 3), and epoch 2 breaks: stage 1 (2, 4)
stops, the route is withdrawn, request 3 waits in the queue and request 4
fails, as each declared. Peer 2 now holds 0 bytes. An answer for request
3 from the old epoch is stale: it is queued.

**Epoch 3.** Chapter 189 replans from (3, 2) with peer 3 gone: (1, 2),
one stage moved. Tokens (3, 5) and (3, 6). Nothing adopts, because epoch
2's instance on peer 2 has stopped, so both fetch. Once both are warm,
epoch 3 opens and request 3 is bound to it, on its second try.

**The ghost.** Peer 3's stage 0 still sends request 3 on, with its old
token (2, 3). Epoch 3's stage 1 was wired to accept (3, 5): the epoch is
older, so it refuses the message as stale and writes nothing. If the ghost
had answered request 3 as epoch 2's last stage, (2, 4), the exit would
refuse that as stale too. Epoch 3's own stage 0, (3, 5), is accepted, and
request 3 is answered once. A `drain` command from epoch 1 that arrives
late, (1, 2), is refused by every instance of epoch 3.

## Running it

The example sends real chapter 186 messages between the stages; each
simulated stage adds 1 to every value it receives, so a row of 1, 2, 3, 4
comes out as 3, 4, 5, 6.

<!-- source: psionic/examples/stage-life.coax life-example -->
```jai
main :: () {
    mm:=stg_ministral(MS_3B);two:=[2]int.[6000000000,6000000000];
    j:=plc_job(mm,stg_plan(mm,two[..],4096),4096,MS_3B.width);
    f:=life_fleet();zero:[1]int;none:=zero[0..<0];m:LfMesh;
    p:=plc_place(j,f,PlcGoal.latency,none);m=lf_plan(m,j,f,p).mesh;life_show("planned",m,1);
    life_why("open epoch 1",lf_open(m).refusal);
    m=life_warm(m,1);m=lf_open(m).mesh;life_show("opened",m,1);
    for r:0..<3 {m=lf_admit(m,r,ifx r==2 then LfLoss.fail else LfLoss.retry).mesh;}
    m=life_run(m,0);
    // Peer 3 joins; a fresh plan moves stage 0 there. Stage 1 stays on peer 2.
    g:=f;g.peer[3].live=true;p=plc_place(j,g,PlcGoal.latency,none);
    m=lf_plan(m,j,g,p).mesh;life_show("peer 3 joins",m,2);life_memory(m);
    m=life_warm(m,2);m=lf_open(m).mesh;life_show("opened",m,2);life_show("draining",m,1);
    m=lf_admit(m,3,LfLoss.retry).mesh;m=lf_admit(m,4,LfLoss.fail).mesh;
    print("in flight: epoch 1 %, epoch 2 %\n",m.slot[lf_slot(m,1)].inflight,m.slot[lf_slot(m,2)].inflight);
    m=life_run(m,1);m=life_run(m,2);life_show("drained",m,1);life_memory(m);
    // Peer 3 leaves, but its stage keeps running.
    ghost:=m.slot[lf_slot(m,2)].stage[0];
    m=lf_lose(m,3).mesh;life_show("peer 3 leaves",m,2);
    print("route: %; request 3 %, request 4 %\n",ifx m.serving==0 then "withdrawn" else "open",
        ifx m.req[3].state==LfReqState.queued then "queued" else "lost",ifx m.req[4].state==LfReqState.failed then "failed" else "lost");
    g.peer[3].live=false;old:=[2]int.[3,2];p=plc_place(j,g,PlcGoal.latency,old[..]);
    m=lf_plan(m,j,g,p).mesh;life_show("replanned",m,3);m=life_warm(m,3);m=lf_open(m).mesh;life_show("opened",m,3);
    row:=[4]f32.[1,2,3,4];wire:[16]u32;n:=lf_pack(ghost.token,3,MODEL,14,0,4,row[..],wire[..]);
    want:=StgExpect.{model=MODEL,layer=14,start=0,width=4};
    life_why("ghost stage 0 (epoch 2, gen 3) sends request 3",lf_receive(m.slot[lf_slot(m,3)].stage[1],want,wire[0..<n],row[..]).refusal);
    life_why("ghost answers request 3 as epoch 2",lf_answer(m,3,LfToken.{epoch=2,gen=4}).refusal);
    m=life_run(m,3);print("request 3 took % tries\n",m.req[3].tries);
    life_why("a late drain for epoch 1",lf_command(m.slot[lf_slot(m,3)].stage[1],LfToken.{epoch=1,gen=2},LfEvent.drain).refusal);
    life_memory(m);
}
```

The transcript, the same in debug and release builds:

```text
planned: epoch 1: stage 0 on peer 1 fetching, gen 1, stage 1 on peer 2 fetching, gen 2
open epoch 1: not every stage is warm
opened: epoch 1: stage 0 on peer 1 serving, gen 1, stage 1 on peer 2 serving, gen 2
request 0 on epoch 1: 3.0 4.0 5.0 6.0, answered
peer 3 joins: epoch 2: stage 0 on peer 3 fetching, gen 3, stage 1 on peer 2 warm, gen 4
memory: peer 0 0, peer 1 4735541248, peer 2 4808947712, peer 3 4735541248 bytes
opened: epoch 2: stage 0 on peer 3 serving, gen 3, stage 1 on peer 2 serving, gen 4
draining: epoch 1: stage 0 on peer 1 draining, gen 1, stage 1 on peer 2 draining, gen 2
in flight: epoch 1 2, epoch 2 2
request 1 on epoch 1: 3.0 4.0 5.0 6.0, answered
request 2 on epoch 1: 3.0 4.0 5.0 6.0, answered
drained: epoch 1: stage 0 on peer 1 retired, gen 1, stage 1 on peer 2 retired, gen 2
memory: peer 0 0, peer 1 0, peer 2 4203907072, peer 3 4735541248 bytes
peer 3 leaves: epoch 2: stage 0 on peer 3 failed, gen 3, stage 1 on peer 2 retired, gen 4
route: withdrawn; request 3 queued, request 4 failed
replanned: epoch 3: stage 0 on peer 1 fetching, gen 5, stage 1 on peer 2 fetching, gen 6
opened: epoch 3: stage 0 on peer 1 serving, gen 5, stage 1 on peer 2 serving, gen 6
ghost stage 0 (epoch 2, gen 3) sends request 3: older epoch or generation
ghost answers request 3 as epoch 2: older epoch or generation
request 3 on epoch 3: 3.0 4.0 5.0 6.0, answered
request 3 took 2 tries
a late drain for epoch 1: older epoch or generation
memory: peer 0 0, peer 1 4735541248, peer 2 4203907072, peer 3 0 bytes
```

Request 3 gives the same row on its second try as request 0 did on its
first. The last line shows that the failed and retired instances hold
nothing: each peer holds exactly epoch 3's stage.

## Refusals

| Refusal | When |
| --- | --- |
| `stale` | a token older than the receiver's: a lower epoch, or the same epoch and a lower generation; an answer for a request that has been moved or queued |
| `ahead` | a token newer than the receiver's |
| `order` | a move the table refuses, a peer reporting a coordinator's event, or opening the epoch that already serves |
| `closed` | a boundary message to a stage that is not serving or draining |
| `partial` | a message shorter than its trailer |
| `boundary` | chapter 186 refused the message; `cause` says why |
| `request`, `index`, `peer` | a request, stage or peer out of range; an answer for a request never admitted |
| `ready` | opening before every stage of the newest epoch is warm |
| `busy` | four epochs are alive (three draining and one serving), so no fifth can open |
| `plan` | a placement that is not ok, is for another job, or names a peer out of range or gone |
| `done` | admitting a request twice, or answering one that is over |

<!-- source: psionic/examples/stage-life-refuse.coax refuse -->
```jai
main :: () {
    mm:=stg_ministral(MS_3B);two:=[2]int.[6000000000,6000000000];
    j:=plc_job(mm,stg_plan(mm,two[..],4096),4096,MS_3B.width);
    f:=refuse_fleet();zero:[1]int;none:=zero[0..<0];m:LfMesh;
    p:=plc_place(j,f,PlcGoal.latency,none);m=lf_plan(m,j,f,p).mesh;
    a:=m.slot[0].stage[0];
    refuse_why("open before the stages are warm",lf_open(m).refusal);
    refuse_why("stage 0 reports loaded while fetching",lf_report(m,0,a.token,LfEvent.loaded).refusal);
    refuse_why("stage 0 reports serve",lf_report(m,0,a.token,LfEvent.serve).refusal);
    refuse_why("a report for stage 2 of 2",lf_report(m,2,a.token,LfEvent.fetched).refusal);
    refuse_why("a report from epoch 2, not made yet",lf_report(m,0,LfToken.{epoch=2,gen=1},LfEvent.fetched).refusal);
    m=refuse_warm(m,1);
    b:=m.slot[0].stage[1];row:=[4]f32.[1,2,3,4];wire:[16]u32;out:[4]f32;
    want:=StgExpect.{model=MODEL,layer=14,start=0,width=4};
    n:=lf_pack(a.token,0,MODEL,14,0,4,row[..],wire[..]);
    refuse_why("a message to stage 1 while it is warm",lf_receive(b,want,wire[0..<n],out[..]).refusal);
    m=lf_open(m).mesh;b=m.slot[0].stage[1];
    refuse_why("open again",lf_open(m).refusal);
    refuse_why("a command with generation 1 to stage 1 (generation 2)",lf_command(b,LfToken.{epoch=1,gen=1},LfEvent.drain).refusal);
    refuse_why("a command with generation 3 to stage 1",lf_command(b,LfToken.{epoch=1,gen=3},LfEvent.drain).refusal);
    refuse_why("idle to a serving stage",lf_command(b,b.token,LfEvent.idle).refusal);
    c:=wire;c[10]+=1;g:=lf_receive(b,want,c[0..<n],out[..]);
    print("a message with one value changed: %, %\n",lf_refusal_name(g.refusal),stg_refusal_name(g.cause));
    w:=want;w.start=1;g=lf_receive(b,w,wire[0..<n],out[..]);
    print("a message for position 0 where 1 is next: %, %\n",lf_refusal_name(g.refusal),stg_refusal_name(g.cause));
    refuse_why("a message with its trailer cut off",lf_receive(b,want,wire[0..<2],out[..]).refusal);
    refuse_why("pack for request 32",ifx lf_pack(a.token,32,MODEL,14,0,4,row[..],wire[..])<0 then LfRefusal.request else LfRefusal.none);
    m=lf_admit(m,0,LfLoss.retry).mesh;
    refuse_why("admit request 0 again",lf_admit(m,0,LfLoss.retry).refusal);
    refuse_why("admit request 32",lf_admit(m,32,LfLoss.retry).refusal);
    refuse_why("an answer for request 0 from stage 0",lf_answer(m,0,a.token).refusal);
    m=lf_answer(m,0,b.token).mesh;
    refuse_why("the same answer again",lf_answer(m,0,b.token).refusal);
    refuse_why("an answer for request 1, never admitted",lf_answer(m,1,b.token).refusal);
    q:=p;q.status=PlcStatus.memory;refuse_why("a placement that was refused",lf_plan(m,j,f,q).refusal);
    g2:=f;g2.peer[2].live=false;refuse_why("a placement on a peer that has left",lf_plan(m,j,g2,p).refusal);
    refuse_why("this machine leaves",lf_lose(m,0).refusal);
    for e:0..<3 {
        m=lf_plan(m,j,f,p).mesh;m=refuse_warm(m,m.epoch);m=lf_admit(m,1+e,LfLoss.retry).mesh;m=lf_open(m).mesh;
    }
    refuse_why("a fifth epoch while four hold requests",lf_plan(m,j,f,p).refusal);
}
```

The program exits with status 0; every line is a refusal the library
returned, and the transcript is compared in debug and release builds:

```text
open before the stages are warm: not every stage is warm
stage 0 reports loaded while fetching: not allowed in this state
stage 0 reports serve: not allowed in this state
a report for stage 2 of 2: no such stage
a report from epoch 2, not made yet: newer epoch or generation
a message to stage 1 while it is warm: not serving
open again: not allowed in this state
a command with generation 1 to stage 1 (generation 2): older epoch or generation
a command with generation 3 to stage 1: newer epoch or generation
idle to a serving stage: not allowed in this state
a message with one value changed: boundary message refused, checksum
a message for position 0 where 1 is next: boundary message refused, position
a message with its trailer cut off: partial
pack for request 32: no such request
admit request 0 again: already admitted or finished
admit request 32: no such request
an answer for request 0 from stage 0: older epoch or generation
the same answer again: already admitted or finished
an answer for request 1, never admitted: no such request
a placement that was refused: placement not usable
a placement on a peer that has left: placement not usable
this machine leaves: no such peer
a fifth epoch while four hold requests: no free epoch
```

The last refusal needs four epochs alive. Each of epochs 2, 3 and 4 opens
while the one before still holds a request, so epochs 1, 2 and 3 drain
and epoch 4 serves. A fifth would have nowhere to go, and dropping a
draining epoch would drop its requests, so the coordinator refuses to plan
until one finishes.

## Checks

- **Lifecycle and fencing** (`stage-life.coax`, `#authority`):
  - The worked trace above, with every token, state, memory figure and
    refusal.
  - Every state × event × token: three older and three newer tokens are
    refused with the state unchanged, and the stage's own token moves
    exactly as the hand-typed table says. 200 random event sequences
    follow the table.
  - Boundary messages: the trailer's words and the receiver's checks in
    order, each refusal with chapter 186's cause, both ends of every
    word's range, and a buffer one word short of the trailer.
  - The coordinator: placements refused for each reason, `hold` against
    `fetch` by the package bit, a newer plan stopping a warming one,
    adoption only when all five stage numbers and the peer match, a fault
    in a draining epoch moving its retry request to the serving epoch,
    one peer breaking two epochs, entries past an epoch's stage count
    never read, and the `busy` refusal.
  - A `#forall` over 120 random start orders of a three-stage epoch: the
    route opens exactly when every stage is warm, never before.
  - A discrete-event simulation over 60 seeds. A pipeline of two or three
    simulated stages on five peers takes 24 requests; one or two peers
    leave at random times, and half of them keep running as ghosts that
    accept messages addressed to them and send each one on three times
    with their old token; one plan is a fresh rebalance; a third of runs
    have a fault while an epoch warms; messages take one to four ticks.
    Every message is offered to every live instance at its peer and
    stage. At the end, across every seed: no request dropped, none
    answered twice, every answer equal to the uninterrupted run, no
    request that declared retry failed, no message accepted by two
    instances, never two epochs serving, the route never on a stage that
    is not serving, and each peer holding exactly the serving epoch's
    bytes. Seed 2 is pinned: 17 answered, 7 failed as declared, 24
    messages from ghosts and 32 messages and answers fenced.
  - `coax mutate --root-only` leaves no survivors (333 mutants).

## What remains

- **Live peers.** The stages here are records in one process and the
  messages a list. On live peers, each instance runs on its peer behind
  [chapter 188](ch188-peers-transport-and-discovery.md)'s transport, and
  the `fetched`, `verified` and `loaded` reports come from [chapter
  187](ch187-stage-packages.md)'s loader running chapter 178's phases.
- **Detecting loss within a stated time.** Here a test says when a peer
  leaves. Killing, freezing or disconnecting a live stage needs
  heartbeats with a timeout, and a stated bound on how long the route
  stays up after a stage stops answering.
- **Restore from saved state.** A request that loses its epoch runs again
  from its prompt. Restoring a long sequence from a saved cache, so that
  it resumes where it stopped, is not built.
- **Memory headroom during a cutover.** Chapter 189 checks that each
  stage fits its peer. During a cutover an old and a new instance can
  share a peer, as on peer 2 above (two caches); that sum is reported by
  `lf_memory` but not checked against the peer's budget.

## Dependencies

- [Chapter 178](ch178-start-fast.md): the fetch, verify and load phases.
- [Chapter 179](ch179-ship-without-downtime.md): draining, where a request
  finishes on the version it started on.
- [Chapter 181](ch181-survive-failures.md): retries, late-answer fencing
  and exactly-once answers.
- [Chapter 186](ch186-split-a-model-into-stages.md): the boundary message.
- [Chapter 187](ch187-stage-packages.md): stage packages and readiness.
- [Chapter 188](ch188-peers-transport-and-discovery.md): peers and the
  control plane.
- [Chapter 189](ch189-place-the-model.md): placement and minimal-move
  replanning.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
