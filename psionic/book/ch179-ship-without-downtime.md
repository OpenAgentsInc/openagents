# 179. Ship without downtime

> **Status: partly implemented.** The rollout controller (profile
> ro-rollout-v1) runs on the CPU: canary steps by traffic share, chapter
> 178's readiness machine and golden warm-up, a verdict per step from
> golden answers, an error budget and a P95 objective, promotion only after
> every step passed, an automatic revert, draining, and a capability
> document that switches with the traffic in one transition. It drives
> chapter 177's simulation of replicas in one process and is checked by a
> hand trace, a brute-force transition table, invariants recomputed from
> every simulated request, and frozen runs on synthetic and recorded
> traffic. No real deployment runs: separate replica processes or
> machines, a real build's golden replays against retained evidence, and
> chapter 218's self-check are not wired in; the last section lists them.

A new build must reach users without dropping requests and without
changing answers by accident. This chapter ships version 8 of a model over
version 7 with a canary: the new build takes a small share of the traffic,
then a larger one, and each step must pass a verdict before the next. A
failed verdict sends every new request back to version 7 at once. Neither
change touches a request already in flight: it finishes on the build that
started it.

The rules decide what users are served, so they are pure Coax procedures
in an `#authority` file, [`serve-rollout.coax`](../lib/serve-rollout.coax),
run against chapter 177's discrete-event simulation before any machine
runs them. The chapter rolls out new builds of one model under one
numerical profile. Replacing one model with a different one is
[chapter 224](ch224-shadow-a-new-model-then-promote-it.md)'s job, and this
controller refuses it.

## Phases, events and refusals

A rollout moves through six phases. In `idle` one build serves
everything. `begin` names the new build; its replicas start (`warming`).
When every one of them has passed readiness and its golden warm-up, the
canary takes the first step's share (`canary`). Each `window` event closes
a step with a verdict. When the last step passes, the rollout is
`verified` and waits for `promote`, which makes the new build stable; the
old build then drains (`retire`). A failed verdict, a failed warm-up or an
operator's `rollback` reverts instead: the old build stays stable and the
new one drains (`revert`). `drained` returns to `idle` when the losing
build holds no request.

<!-- source: psionic/lib/serve-rollout.coax types -->
```jai
// idle: one build serves everything. warming: the new build's replicas are
// starting. canary: the new build takes the current step's share. verified:
// every step passed; traffic holds until an operator promotes. retire: the
// new build is stable and the old one drains. revert: the old build kept
// its place and the new one drains.
RoPhase :: enum u8 {idle;warming;canary;verified;retire;revert;}
RoEvent :: enum u8 {begin;warm;window;promote;rollback;drained;}
// phase: the event is not accepted in this phase. capability: the new
// build's document names another model or profile, or another digest than
// the weights it loads. stale: its version is not newer than the stable
// one. shape: a plan or an observation out of bounds. verdict: promotion
// without a passing verdict at every step. unknown: a rollback to a version
// that is not the stable one.
RoRefusal :: enum u8 {none;phase;capability;stale;shape;verdict;unknown;}
// wait: too few canary requests finished in the window to judge. golden:
// a golden request's answer differed from its retained evidence. errors:
// the canary's errors so far exceed the budget. slow: the window's P95
// latency exceeds the objective.
RoVerdict :: enum u8 {none;wait;pass;golden;errors;slow;}
// The capability document a build is served under (chapter 173): its
// version, model, numerical profile and weights digest.
RoDoc :: struct {version:int;model:string;profile:string;digest:int;}
// steps: the canary's shares in percent, ascending. replicas: new replicas
// that must pass warm-up. samples: canary requests a window needs before it
// is judged. p95_us: the latency objective. error_pct: the error budget.
RoPlan :: struct {nsteps:int;steps:[4]int;replicas:int;samples:int;p95_us:int;error_pct:int;window_us:int;}
// One window of canary requests: finished, finished with an error, their
// nearest-rank P95 latency, and golden replays whose answers differed.
RoObs :: struct {served:int;errors:int;p95_us:int;golden_bad:int;}
// The controller. doc is the published capability document. other is the
// build that is not stable: the candidate during a rollout, the old build
// while it retires. share is the percent of new requests sent to other;
// seen and sent count this step's requests and those sent to other.
RoState :: struct {phase:RoPhase;stable:RoDoc;other:RoDoc;doc:RoDoc;share:int;step:int;warm:int;served:int;errors:int;seen:int;sent:int;verdict:RoVerdict;}
// begin carries the new build's document and the digest of the weights its
// replicas load; warm, whether one replica passed readiness and its golden
// warm-up; window, the observation; rollback, the version to return to.
RoInput :: struct {event:RoEvent;doc:RoDoc;digest:int;ok:bool;obs:RoObs;to:int;}
RoStep :: struct {state:RoState;refusal:RoRefusal;}
```

The state keeps two builds, `stable` and `other`, and the published
capability document, `doc`. `share` is the percent of new requests sent
to `other`. During a rollout `other` is the candidate; after promotion it
is the old build, retiring with a share of 0. So the published document is
always the stable build's, and the only transition that changes the
stable build, `promote`, changes the document and the traffic together.

## The controller

`ro_step` takes one input and returns the next state, or the same state
with a typed refusal.

<!-- source: psionic/lib/serve-rollout.coax step -->
```jai
ro_init :: (stable:RoDoc)->RoState {return RoState.{phase=RoPhase.idle,stable=stable,doc=stable};}
ro_plan_ok :: (p:RoPlan)->bool {
    if p.nsteps<1 || p.nsteps>p.steps.count || p.replicas<1 || p.replicas>RR_REPLICAS || p.samples<1 || p.p95_us<1 {return false;}
    if p.error_pct<0 || p.error_pct>100 || p.window_us<1 {return false;}
    last:=0;
    for k:0..<p.nsteps {if p.steps[k]<=last || p.steps[k]>99 {return false;}last=p.steps[k];}
    return true;
}
ro_obs_ok :: (o:RoObs)->bool {return o.served>=0 && o.errors>=0 && o.errors<=o.served && o.p95_us>=0 && o.golden_bad>=0;}
// A golden mismatch fails at once. Otherwise a window needs `samples`
// finished canary requests; then the canary's errors so far must stay
// within the budget and the window's P95 within the objective.
ro_verdict :: (p:RoPlan,o:RoObs,served:int,errors:int)->RoVerdict {
    if o.golden_bad>0 {return RoVerdict.golden;}
    if o.served<p.samples {return RoVerdict.wait;}
    if errors*100>p.error_pct*served {return RoVerdict.errors;}
    if o.p95_us>p.p95_us {return RoVerdict.slow;}
    return RoVerdict.pass;
}
// Give up the canary: no new request goes to it; it drains.
ro_revert :: (s:RoState,v:RoVerdict)->RoState {n:=s;n.phase=RoPhase.revert;n.share=0;n.seen=0;n.sent=0;n.verdict=v;return n;}
// One input. A refused input returns the state unchanged with its reason.
ro_step :: (p:RoPlan,s:RoState,inp:RoInput)->RoStep {
    r:=RoStep.{state=s,refusal=RoRefusal.phase};n:=s;
    if inp.event==RoEvent.begin {
        if s.phase!=RoPhase.idle {return r;}
        r.refusal=RoRefusal.shape;
        if !ro_plan_ok(p) {return r;}
        r.refusal=RoRefusal.capability;
        if inp.doc.model!=s.stable.model || inp.doc.profile!=s.stable.profile || inp.doc.digest!=inp.digest {return r;}
        r.refusal=RoRefusal.stale;
        if inp.doc.version<=s.stable.version {return r;}
        n=RoState.{phase=RoPhase.warming,stable=s.stable,other=inp.doc,doc=s.doc};
    }
    if inp.event==RoEvent.warm {
        if s.phase!=RoPhase.warming {return r;}
        if !inp.ok {n=ro_revert(s,RoVerdict.golden);}
        else {
            n.warm+=1;
            if n.warm==p.replicas {n.phase=RoPhase.canary;n.share=p.steps[0];}
        }
    }
    if inp.event==RoEvent.window {
        if s.phase!=RoPhase.canary {return r;}
        r.refusal=RoRefusal.shape;
        if !ro_obs_ok(inp.obs) {return r;}
        n.served+=inp.obs.served;n.errors+=inp.obs.errors;
        n.verdict=ro_verdict(p,inp.obs,n.served,n.errors);
        if n.verdict!=RoVerdict.wait {
            if n.verdict!=RoVerdict.pass {n=ro_revert(n,n.verdict);}
            else {
                n.step+=1;n.seen=0;n.sent=0;
                if n.step==p.nsteps {n.phase=RoPhase.verified;} else {n.share=p.steps[n.step];}
            }
        }
    }
    // The capability document and every new request move to the new build
    // in this one transition; the old build keeps what it holds.
    if inp.event==RoEvent.promote {
        if s.phase==RoPhase.warming || s.phase==RoPhase.canary {r.refusal=RoRefusal.verdict;}
        if s.phase!=RoPhase.verified {return r;}
        n=RoState.{phase=RoPhase.retire,stable=s.other,other=s.stable,doc=s.other,verdict=s.verdict,served=s.served,errors=s.errors};
    }
    if inp.event==RoEvent.rollback {
        if s.phase!=RoPhase.warming && s.phase!=RoPhase.canary && s.phase!=RoPhase.verified {return r;}
        r.refusal=RoRefusal.unknown;
        if inp.to!=s.stable.version {return r;}
        n=ro_revert(s,RoVerdict.none);
    }
    if inp.event==RoEvent.drained {
        if s.phase!=RoPhase.retire && s.phase!=RoPhase.revert {return r;}
        n=RoState.{phase=RoPhase.idle,stable=s.stable,doc=s.doc,verdict=s.verdict};
    }
    return RoStep.{state=n};
}
// The traffic split: with a share above 0, a new request goes to the other
// build when that keeps its count at or below the step's share of the
// requests seen this step.
ro_pick :: (s:RoState)->RoPick {
    if s.share==0 {return RoPick.{state=s};}
    n:=s;n.seen+=1;
    if (n.sent+1)*100<=n.share*n.seen {n.sent+=1;return RoPick.{state=n,other=true};}
    return RoPick.{state=n};
}
```

The order inside a verdict matters. A golden mismatch fails at once,
however few requests the window saw: one wrong answer is enough. Then a
window needs `samples` finished canary requests, or the verdict is `wait`
and the step stays. Then the error budget, counted over the whole canary
so far, and last the window's nearest-rank P95 latency (chapter 172's
`bn_stats`) against the objective.

The split, `ro_pick`, sends a request to `other` when that keeps the
step's count at or below its share of the requests seen this step. At 25
percent the 4th and 8th requests go; at 50 percent every second one. It
counts only while the share is above 0, so a step starts from zero.

The plan the chapter's checks use, `rox_plan`, has two steps of 10 and
50 percent, two new replicas, 5 samples per window, a P95 objective of
250,000 µs and an error budget of 5 percent. Worked by hand:

| Input | Arithmetic | Next |
| --- | --- | --- |
| `begin` version 8, digest 8008 | same model and profile; 8 > 7 | `warming`, share 0, document still 7 |
| `warm` ok, twice | 2 of 2 replicas | `canary`, share 10 |
| `window` 4 served | 4 < 5 samples | `wait`, still step 0 |
| `window` 20 served, P95 250,000 | 0 errors in 24; 250,000 ≤ 250,000 | `pass`, step 1, share 50 |
| `promote` | step 1 of 2 has no verdict yet | refused: no passing verdict |
| `window` 5 served, 1 error | 1 × 100 = 100 ≤ 5 × 29 = 145 | `pass`, `verified` |
| `promote` | | `retire`: stable 8, document 8, share 0 |
| `drained` | | `idle`, stable 8 |

<!-- source: psionic/lib/serve-rollout.coax check-step -->
```jai
#test "the controller by hand: warm, two steps, promote, retire" {
    p:=rox_plan();s:=ro_init(rox_doc(7));
    r:=ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8008});
    #assert r.refusal==RoRefusal.none && r.state.phase==RoPhase.warming && r.state.other.version==8 && r.state.doc.version==7 && r.state.share==0;
    r=ro_step(p,r.state,RoInput.{event=RoEvent.warm,ok=true});#assert r.state.phase==RoPhase.warming && r.state.warm==1;
    r=ro_step(p,r.state,RoInput.{event=RoEvent.warm,ok=true});#assert r.state.phase==RoPhase.canary && r.state.warm==2 && r.state.share==10;
    // Too few samples: wait, same step. Then 20 served, 0 errors, P95 at the objective: pass.
    r=ro_step(p,r.state,RoInput.{event=RoEvent.window,obs=RoObs.{served=4,p95_us=900000}});
    #assert r.refusal==RoRefusal.none && r.state.verdict==RoVerdict.wait && r.state.step==0 && r.state.share==10 && r.state.served==4;
    r=ro_step(p,r.state,RoInput.{event=RoEvent.window,obs=RoObs.{served=20,p95_us=250000}});
    #assert r.state.verdict==RoVerdict.pass && r.state.step==1 && r.state.share==50 && r.state.served==24;
    // Promotion before the last verdict is refused, and changes nothing.
    q:=ro_step(p,r.state,RoInput.{event=RoEvent.promote});#assert q.refusal==RoRefusal.verdict && rox_same(q.state,r.state);
    r=ro_step(p,r.state,RoInput.{event=RoEvent.window,obs=RoObs.{served=5,errors=1,p95_us=1}});
    // 1 error in 29 is 3.4 percent, inside the 5 percent budget.
    #assert r.state.verdict==RoVerdict.pass && r.state.phase==RoPhase.verified && r.state.share==50 && r.state.errors==1;
    r=ro_step(p,r.state,RoInput.{event=RoEvent.promote});
    #assert r.state.phase==RoPhase.retire && r.state.stable.version==8 && r.state.doc.version==8 && r.state.other.version==7 && r.state.share==0;
    r=ro_step(p,r.state,RoInput.{event=RoEvent.drained});
    #assert r.state.phase==RoPhase.idle && r.state.stable.version==8 && r.state.doc.version==8 && r.state.other.version==0 && r.state.served==0;
}
```

## Two builds in one simulation

`ro_sim` is chapter 177's simulation with a build number on every
replica. The old build's replicas start ready. At `begin_us` the
controller hears `begin`; if it accepts, the new build's replicas start
and are ready after `cold_us`, by default chapter 178's measured
520,740 µs cold start with a verification record, whose last 229,304 µs
are the golden warm-up. At that moment each replica runs chapter 178's
readiness machine, `cs_start` then `cs_step` through verify, map,
allocate and one golden warm-up, and `cs_ready` decides. A build whose
golden answer differs from its evidence fails there, before any request
reaches it.

Each arrival is split by `ro_pick` and routed by chapter 177's `rr_route`
among its build's replicas; the others are hidden from the router for
that one call. Every replica slot belongs to one build for the whole run,
so every request finishes on the build it was routed to. When the
controller enters `retire` or `revert`, the share of the losing build is
0, so it takes nothing new, and its replicas drain: a starting one stops
at once; a ready one finishes its queue, then stops. Two properties of a build exist only
in the simulation: `slow_pct` stretches its requests' times against
chapter 175's step model, and `err_every` makes every that-many-th
request it serves answer with an error.

<!-- source: psionic/lib/serve-rollout.coax sim -->
```jai
// Chapter 177's discrete-event simulation with two builds. At each event
// time, in this order: requests finish; starting replicas finish chapter
// 178's readiness machine and golden warm-up; the rollout begins; a canary
// window closes (and a verified rollout is promoted); arrivals are split
// by share and routed among their build's replicas; idle ready replicas
// take their queue's head; the losing build's replicas with nothing left
// stop, and when it has none left the controller hears drained.
ro_sim :: (ro:RoRun,reqs:[]RrRequest,tok:[]int)->RoSim {
    s:RoSim;off:RrScaler;cfg:=ro.cfg;mem:[1728]f64;
    s.status=rr_check(cfg,off,reqs,tok);
    if s.status!=RrStatus.ok {return s;}
    s.status=RrStatus.config;
    if !ro_plan_ok(ro.plan) || cfg.replicas+ro.plan.replicas>RR_REPLICAS || ro.cold_us<0 || ro.begin_us<0 {return s;}
    if !ro_build_ok(ro.stable) || !ro_build_ok(ro.cand) {return s;}
    s.status=RrStatus.ok;
    // Slots below cfg.replicas hold the stable build, ready; the rest the new one, off until it begins.
    rep:[8]RrReplica;ver:[8]int;
    for k:0..<RR_REPLICAS {
        rep[k]=RrReplica.{running=-1,st=rr_store(cfg.pages)};ver[k]=ro.cand.doc.version;
        if k<cfg.replicas {rep[k].phase=RrPhase.ready;ver[k]=ro.stable.doc.version;}
    }
    s.run.n=reqs.count;tr:=RoTrack.{state=ro_init(ro.stable.doc)};
    a:int;now:int;held:[8]int;turn:int;begun:bool;window:int;opened:int;done:int;
    while true {
        t:=RR_NEVER;busy:=a<s.run.n;
        if busy {t=reqs[a].arrive;}
        if !begun {t=cb_min(t,ro.begin_us);}
        for k:0..<RR_REPLICAS {
            if rep[k].running>=0 {t=cb_min(t,rep[k].busy_until);}
            if rep[k].phase==RrPhase.starting {t=cb_min(t,rep[k].ready_at);}
        }
        // A canary's windows close while requests arrive or run (a request
        // that runs finishes after the window opened), and once more after the last finishes.
        if (busy || done>opened) && tr.state.phase==RoPhase.canary {t=cb_min(t,window);}
        if t==RR_NEVER {break;}
        s.run.events+=1;
        if s.run.events>cfg.events {s.status=RrStatus.events;return ro_track(s,tr);}
        now=t;s.run.end=now;
        for k:0..<RR_REPLICAS {
            if rep[k].running>=0 && rep[k].busy_until==now {held[reqs[rep[k].running].client]-=1;rep[k].running=-1;}
        }
        for k:cfg.replicas..<cfg.replicas+ro.plan.replicas {
            if rep[k].phase==RrPhase.starting && rep[k].ready_at==now && tr.state.phase==RoPhase.warming {
                ok:=ro_ready(ro.cand.golden_ok);
                if ok {rep[k].phase=RrPhase.ready;}
                tr=ro_apply(tr,ro.plan,RoInput.{event=RoEvent.warm,ok=ok},now);
                if tr.state.phase==RoPhase.canary {window=now+ro.plan.window_us;}
            }
        }
        if !begun && now==ro.begin_us {
            begun=true;
            tr=ro_apply(tr,ro.plan,RoInput.{event=RoEvent.begin,doc=ro.cand.doc,digest=ro.cand.digest},now);
            if tr.state.phase==RoPhase.warming {
                for k:cfg.replicas..<cfg.replicas+ro.plan.replicas {rep[k].phase=RrPhase.starting;rep[k].ready_at=now+ro.cold_us;}
            }
        }
        if tr.state.phase==RoPhase.canary && now==window {
            o:=ro_observe(s.run.fate[..],s.served_by[..],s.err[..],reqs,tr.state.other.version,opened,now);
            tr=ro_apply(tr,ro.plan,RoInput.{event=RoEvent.window,obs=o},now);
            if tr.state.phase==RoPhase.verified {tr=ro_apply(tr,ro.plan,RoInput.{event=RoEvent.promote},now);}
            window=now+ro.plan.window_us;opened=now;
        }
        while a<s.run.n && reqs[a].arrive==now {
            r:=reqs[a];k:=ro_pick(tr.state);tr.state=k.state;
            v:=ifx k.other then tr.state.other.version else tr.state.stable.version;
            // Only this build's replicas may take it: hide the others from the router.
            phase:[8]RrPhase;
            for j:0..<RR_REPLICAS {phase[j]=rep[j].phase;if ver[j]!=v {rep[j].phase=RrPhase.off;}}
            p:=rr_route(rep[..],cfg,r,tok[a*PG_PROMPT..<a*PG_PROMPT+PG_PROMPT],now,held,turn);
            for j:0..<RR_REPLICAS {rep[j].phase=phase[j];}
            s.run.fate[a]=RrFate.{refusal=p.refusal,replica=p.replica,start=-1,first=-1,finish=-1};
            s.ver[a]=-1;s.served_by[a]=-1;s.doc[a]=tr.state.doc.version;
            if p.refusal!=RrRefusal.none {s.run.refused[cast(int)p.refusal]+=1;}
            else {
                j:=p.replica;turn=p.turn;held[r.client]+=1;s.ver[a]=v;s.run.fate[a].pred=rr_cost(r.prompt,r.output,p.cached*cfg.pages.page);
                rep[j].queue[rep[j].nq]=a;rep[j].nq+=1;rep[j].work+=s.run.fate[a].pred;
            }
            a+=1;
        }
        for k:0..<RR_REPLICAS {
            if rep[k].phase==RrPhase.ready && rep[k].running<0 && rep[k].nq>0 {
                i:=rep[k].queue[0];s.run.hit+=rr_serve(rep[..],s.run.fate[..],k,reqs[i],tok[i*PG_PROMPT..<i*PG_PROMPT+PG_PROMPT],mem[..],now);
                b:=ifx ver[k]==ro.cand.doc.version then ro.cand else ro.stable;
                // A build slower than the step model: its first token and its finish come slow_pct percent as late.
                f:=s.run.fate[i];f.first=now+(f.first-now)*b.slow_pct/100;f.finish=now+(f.finish-now)*b.slow_pct/100;
                s.run.fate[i]=f;rep[k].busy_until=f.finish;done=cb_max(done,f.finish);
                s.served_by[i]=ver[k];s.err[i]=b.err_every>0 && (i+1)%b.err_every==0;
            }
        }
        // Retire or revert: the other build takes nothing new; each of its
        // replicas stops once it holds nothing, a starting one at once.
        if tr.state.phase==RoPhase.retire || tr.state.phase==RoPhase.revert {
            left:=false;
            for k:0..<RR_REPLICAS {
                if ver[k]==tr.state.other.version && rep[k].phase!=RrPhase.off && rep[k].nq==0 && rep[k].running<0 {rep[k].phase=RrPhase.off;s.run.downs+=1;}
                if ver[k]==tr.state.other.version && rep[k].phase!=RrPhase.off {left=true;}
            }
            if !left {tr=ro_apply(tr,ro.plan,RoInput.{event=RoEvent.drained},now);}
        }
    }
    return ro_track(s,tr);
}
```

A window closes every `window_us` from the moment the canary starts,
while requests arrive or run, and once more after the last one finishes.
If the traffic ends before the last step passes, the rollout stays in its
canary; the simulation reports that rather than promoting. Its observation counts the canary's requests
that finished since the window opened, their errors, and their P95
latency from arrival.

## A canary failure, traced by hand

One old replica, one new one that starts in 100,000 µs and runs three
times as slow as modeled, one step of 50 percent, 1 sample per window, a
P95 objective of 100,000 µs, windows of 200,000 µs. Six requests of 8
prompt tokens and 2 output tokens arrive at 0, 150, 160, 170, 290 and
320 ms; none shares a prefix, so none reuses a cached block.

One request costs, by chapter 175's step model, (17,792,472 + 7 ×
1,459,182 + 17,792,472) / 1000 = 45,799 µs, and 3 × 45,799 = 137,397 µs on
the slow build.

| Time (µs) | What happens |
| --- | --- |
| 0 | `begin`: warming; request 0 goes to version 7 (share 0) and finishes at 45,799 |
| 100,000 | the new replica passes readiness and the golden warm-up: canary, share 50 |
| 150,000 | request 1, 1st of the step: (0 + 1) × 100 > 50 × 1, version 7, finishes at 195,799 |
| 160,000 | request 2, 2nd: 100 ≤ 100, version 8, finishes at 160,000 + 137,397 = 297,397 |
| 170,000 | request 3, 3rd: 200 > 150, version 7, waits for request 1, runs 195,799 to 241,598 |
| 290,000 | request 4, 4th: 200 ≤ 200, version 8, waits for request 2, starts at 297,397 |
| 300,000 | window: request 2 finished, latency 137,397 > 100,000: `slow`, revert, share 0 |
| 320,000 | request 5 goes to version 7, finishes at 365,799 |
| 434,794 | request 4 finishes on version 8 (297,397 + 137,397); the new replica stops: `drained`, idle |

Request 4 was in flight on the canary when the canary failed. It was not
moved or dropped: it finished on version 8, the build it was routed to,
and only then did the replica stop. The published document stayed
version 7 throughout.

<!-- source: psionic/lib/serve-rollout.coax check-hand -->
```jai
#test "the hand-traced canary: a slow build fails its window and reverts" {
    hq:=rox_hand_requests();ht:=rox_tok();s:=ro_sim(rox_hand_run(),hq[..],ht[..]);
    #assert s.status==RrStatus.ok && s.inputs==4 && s.run.events==14 && s.run.downs==1;
    // 100,000 us: the new replica passes readiness and its golden warm-up; the canary takes 50 percent.
    #assert rox_log(s.log[0],0,RoEvent.begin,RoRefusal.none,RoPhase.warming,0,RoVerdict.none);
    #assert rox_log(s.log[1],100000,RoEvent.warm,RoRefusal.none,RoPhase.canary,50,RoVerdict.none);
    // 300,000 us: one canary request finished, P95 137,397 us over the 100,000 us objective.
    #assert rox_log(s.log[2],300000,RoEvent.window,RoRefusal.none,RoPhase.revert,0,RoVerdict.slow);
    // 434,794 us: the request the canary started at 297,397 us finishes there, and the new replica stops.
    #assert rox_log(s.log[3],434794,RoEvent.drained,RoRefusal.none,RoPhase.idle,0,RoVerdict.slow);
    f:=s.run.fate;
    #assert s.ver[0]==7 && f[0].finish==45799 && s.ver[1]==7 && f[1].finish==195799 && s.ver[2]==8 && f[2].start==160000 && f[2].finish==297397;
    #assert s.ver[3]==7 && f[3].start==195799 && f[3].finish==241598 && s.ver[4]==8 && f[4].start==297397 && f[4].finish==434794;
    #assert s.ver[5]==7 && f[5].finish==365799 && s.state.stable.version==7 && s.state.doc.version==7;
    for i:0..<6 {#assert s.served_by[i]==s.ver[i] && s.doc[i]==7;}
}
```

## What is refused

A rollout of the wrong thing must not start, and a promotion must not
skip its evidence. `begin` is refused as `capability` when the new
build's document names another model or another profile (that is chapter
224's rollout, not this one), or when its replicas load weights whose
digest differs from the document's; as `stale` when its version is not
newer than the stable one; as `shape` when the plan's steps are not
ascending inside 1 to 99 percent. `promote` before every step has passed
is refused as `verdict`. `rollback` names the version to return to, which
must be the stable one; any other is refused as `unknown`. A window whose
counts are impossible (more errors than requests, a negative count) is
refused as `shape`. Every refusal leaves the state as it was.

<!-- source: psionic/lib/serve-rollout.coax check-refuse -->
```jai
#test "refusals: no verdict, a mismatched capability, an unknown rollback target" {
    p:=rox_plan();s:=ro_init(rox_doc(7));
    // Promote or roll back with no rollout under way.
    #assert ro_step(p,s,RoInput.{event=RoEvent.promote}).refusal==RoRefusal.phase;
    #assert ro_step(p,s,RoInput.{event=RoEvent.rollback,to=7}).refusal==RoRefusal.phase;
    // Another model, another profile, a digest other than the weights, an old version.
    d:=rox_doc(8);d.model="gpt-oss-120b";#assert ro_step(p,s,RoInput.{event=RoEvent.begin,doc=d,digest=8008}).refusal==RoRefusal.capability;
    d=rox_doc(8);d.profile="rr-route-v2";#assert ro_step(p,s,RoInput.{event=RoEvent.begin,doc=d,digest=8008}).refusal==RoRefusal.capability;
    #assert ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8009}).refusal==RoRefusal.capability;
    #assert ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(7),digest=7007}).refusal==RoRefusal.stale;
    #assert ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(6),digest=6006}).refusal==RoRefusal.stale;
    r:=ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8008});
    #assert ro_step(p,r.state,RoInput.{event=RoEvent.begin,doc=rox_doc(9),digest=9009}).refusal==RoRefusal.phase;
    // Promote while warming; roll back to the candidate or to a version never served.
    #assert ro_step(p,r.state,RoInput.{event=RoEvent.promote}).refusal==RoRefusal.verdict;
    #assert ro_step(p,r.state,RoInput.{event=RoEvent.rollback,to=8}).refusal==RoRefusal.unknown;
    #assert ro_step(p,r.state,RoInput.{event=RoEvent.rollback,to=5}).refusal==RoRefusal.unknown;
    #assert ro_step(p,r.state,RoInput.{event=RoEvent.window}).refusal==RoRefusal.phase;
    #assert ro_step(p,r.state,RoInput.{event=RoEvent.drained}).refusal==RoRefusal.phase;
    b:=ro_step(p,r.state,RoInput.{event=RoEvent.rollback,to=7});
    #assert b.refusal==RoRefusal.none && b.state.phase==RoPhase.revert && b.state.verdict==RoVerdict.none && b.state.doc.version==7;
    #assert ro_step(p,b.state,RoInput.{event=RoEvent.rollback,to=7}).refusal==RoRefusal.phase;
    #assert ro_step(p,b.state,RoInput.{event=RoEvent.warm,ok=true}).refusal==RoRefusal.phase;
    // Impossible observations.
    c:=ro_step(p,ro_step(p,r.state,RoInput.{event=RoEvent.warm,ok=true}).state,RoInput.{event=RoEvent.warm,ok=true}).state;
    #assert c.phase==RoPhase.canary;
    for k:0..<4 {
        o:=RoObs.{served=5,errors=1,p95_us=1,golden_bad=0};
        if k==0 {o.served=-1;}if k==1 {o.errors=-1;}if k==2 {o.errors=6;}if k==3 {o.p95_us=-1;}
        #assert ro_step(p,c,RoInput.{event=RoEvent.window,obs=o}).refusal==RoRefusal.shape;
    }
    o:=RoObs.{served=5,errors=5,golden_bad=-1};#assert ro_step(p,c,RoInput.{event=RoEvent.window,obs=o}).refusal==RoRefusal.shape;
    o.golden_bad=0;#assert ro_step(p,c,RoInput.{event=RoEvent.window,obs=o}).refusal==RoRefusal.none;
    o.p95_us=0;o.errors=0;o.served=0;#assert ro_step(p,c,RoInput.{event=RoEvent.window,obs=o}).state.verdict==RoVerdict.wait;
    // A plan out of bounds refuses the begin.
    x:=p;x.nsteps=0;#assert ro_step(x,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8008}).refusal==RoRefusal.shape;
}
```

The second example,
[`serve-rollout-refuse.coax`](../examples/serve-rollout-refuse.coax),
prints each refusal with its reason:

```shell
coax/bin/coax run psionic/examples/serve-rollout-refuse.coax
```

<!-- source: psionic/examples/serve-rollout-refuse.coax main -->
```jai
main :: () {
    p:=rox_plan();s:=ro_init(rox_doc(7));
    d:=rox_doc(8);d.model="gpt-oss-120b";
    say("begin version 8 whose document names gpt-oss-120b",ro_step(p,s,RoInput.{event=RoEvent.begin,doc=d,digest=8008}));
    d=rox_doc(8);d.profile="rr-route-v2";
    say("begin version 8 under profile rr-route-v2",ro_step(p,s,RoInput.{event=RoEvent.begin,doc=d,digest=8008}));
    say("begin version 8 whose replicas load digest 8009",ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8009}));
    say("begin version 6",ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(6),digest=6006}));
    say("promote with no rollout",ro_step(p,s,RoInput.{event=RoEvent.promote}));
    w:=ro_step(p,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8008});
    say("begin version 8",w);
    say("promote while warming",ro_step(p,w.state,RoInput.{event=RoEvent.promote}));
    c:=ro_step(p,ro_step(p,w.state,RoInput.{event=RoEvent.warm,ok=true}).state,RoInput.{event=RoEvent.warm,ok=true});
    say("two replicas warm",c);
    say("promote in the canary, before its verdict",ro_step(p,c.state,RoInput.{event=RoEvent.promote}));
    say("a window with 6 errors in 5 requests",ro_step(p,c.state,RoInput.{event=RoEvent.window,obs=RoObs.{served=5,errors=6}}));
    say("roll back to version 5",ro_step(p,c.state,RoInput.{event=RoEvent.rollback,to=5}));
    say("roll back to version 8",ro_step(p,c.state,RoInput.{event=RoEvent.rollback,to=8}));
    say("roll back to version 7",ro_step(p,c.state,RoInput.{event=RoEvent.rollback,to=7}));
    x:=p;x.steps[1]=10;
    say("begin with steps of 10 and 10 percent",ro_step(x,s,RoInput.{event=RoEvent.begin,doc=rox_doc(8),digest=8008}));
    q:=rox_hand_requests();t:=rox_tok();ro:=rox_hand_run();ro.plan.replicas=8;
    print("simulate 1 old and 8 new replicas: %\n",rr_status_name(ro_sim(ro,q[..],t[..]).status));
}
```

```text
begin version 8 whose document names gpt-oss-120b: refused, capability document does not match; still idle, version 7 published
begin version 8 under profile rr-route-v2: refused, capability document does not match; still idle, version 7 published
begin version 8 whose replicas load digest 8009: refused, capability document does not match; still idle, version 7 published
begin version 6: refused, version not newer than the stable one; still idle, version 7 published
promote with no rollout: refused, not in this phase; still idle, version 7 published
begin version 8: accepted, warming now
promote while warming: refused, no passing verdict at every step; still warming, version 7 published
two replicas warm: accepted, canary now
promote in the canary, before its verdict: refused, no passing verdict at every step; still canary, version 7 published
a window with 6 errors in 5 requests: refused, plan or observation out of bounds; still canary, version 7 published
roll back to version 5: refused, rollback target is not the stable version; still canary, version 7 published
roll back to version 8: refused, rollback target is not the stable version; still canary, version 7 published
roll back to version 7: accepted, revert now
begin with steps of 10 and 10 percent: refused, plan or observation out of bounds; still idle, version 7 published
simulate 1 old and 8 new replicas: configuration or traffic out of bounds
```

## Run it

```shell
coax/bin/coax run psionic/examples/serve-rollout.coax
```

The example, [`serve-rollout.coax`](../examples/serve-rollout.coax),
prints the hand trace, then rolls version 8 out over version 7 on
chapter 177's synthetic traffic at a mean gap of 60 ms: 160 requests of
32 prompt tokens, 6 shared prefixes, 4 or 8 output tokens. The fleet has 3
replicas of version 7; the rollout starts 3 of version 8 at 500 ms, with
steps of 20 and 50 percent, 3 samples per window, windows of 1.5 s, a P95
objective of 250 ms and an error budget of 5 percent. It runs four builds:
a good one, one three times slower than the step model, one whose golden
warm-up differs, and one that answers every 8th request with an error.
Last, it rolls a good build out on chapter 172's recorded schedule.

<!-- source: psionic/examples/serve-rollout.coax main -->
```jai
main :: () {
    q:=rox_hand_requests();t:=rox_tok();
    s:=ro_sim(rox_hand_run(),q[..],t[..]);
    print("hand trace (%):\n",RO_PROFILE);
    for i:0..<6 {f:=s.run.fate[i];print("  request %: version %, start % us, finish % us\n",i,s.ver[i],f.start,f.finish);}
    show("hand trace",s,q[..]);

    w:=rox_world(60000);off:RrScaler;c:=rrx_fleet_config(RrPolicy.affinity);c.replicas=3;
    p:=rr_report(rr_sim(c,off,w.req[..],w.tok[..]),w.req[..]);
    print("synthetic, mean gap 60 ms, 160 requests; no rollout on 3 replicas: latency P95 ");ms(p.latency.p[2]);print(" ms\n");
    show("good build",ro_sim(rox_run(rox_good()),w.req[..],w.tok[..]),w.req[..]);
    b:=rox_good();b.slow_pct=300;show("slow build (3 times the step model)",ro_sim(rox_run(b),w.req[..],w.tok[..]),w.req[..]);
    b=rox_good();b.golden_ok=false;show("wrong build (golden warm-up differs)",ro_sim(rox_run(b),w.req[..],w.tok[..]),w.req[..]);
    b=rox_good();b.err_every=8;show("build answering every 8th request with an error",ro_sim(rox_run(b),w.req[..],w.tok[..]),w.req[..]);

    r:=rox_recorded();ro:=rox_run(rox_good());ro.cfg.replicas=2;ro.plan.replicas=2;ro.plan.samples=1;
    ro.plan.nsteps=1;ro.plan.steps[0]=50;ro.plan.window_us=600000;ro.plan.p95_us=700000;ro.begin_us=0;
    print("recorded (chapter 172, 8 requests at 10 times speed), 2 replicas per build, one step of 50 percent:\n");
    show("good build",ro_sim(ro,r.req[0..<8],r.tok[..]),r.req[0..<8]);
}
```

```text
hand trace (ro-rollout-v1):
  request 0: version 7, start 0 us, finish 45799 us
  request 1: version 7, start 150000 us, finish 195799 us
  request 2: version 8, start 160000 us, finish 297397 us
  request 3: version 7, start 195799 us, finish 241598 us
  request 4: version 8, start 297397 us, finish 434794 us
  request 5: version 7, start 320000 us, finish 365799 us
hand trace: 6 served, 0 refused, 2 on version 8; latency P95 144.7 ms; stable version 7, document version 7
  0.0 ms begin -> warming
  100.0 ms warm -> canary, share 50 percent
  300.0 ms window -> revert, verdict P95 over the objective
  434.7 ms drained -> idle, verdict P95 over the objective
synthetic, mean gap 60 ms, 160 requests; no rollout on 3 replicas: latency P95 211.9 ms
good build: 160 served, 0 refused, 107 on version 8; latency P95 187.5 ms; stable version 8, document version 8
  500.0 ms begin -> warming
  1020.7 ms warm -> warming
  1020.7 ms warm -> warming
  1020.7 ms warm -> canary, share 20 percent
  2520.7 ms window -> canary, share 50 percent, verdict pass
  4020.7 ms window -> verified, share 50 percent, verdict pass
  4020.7 ms promote -> retire, verdict pass
  4161.8 ms drained -> idle, verdict pass
slow build (3 times the step model): 160 served, 0 refused, 5 on version 8; latency P95 212.6 ms; stable version 7, document version 7
  500.0 ms begin -> warming
  1020.7 ms warm -> warming
  1020.7 ms warm -> warming
  1020.7 ms warm -> canary, share 20 percent
  2520.7 ms window -> revert, verdict P95 over the objective
  3042.8 ms drained -> idle, verdict P95 over the objective
wrong build (golden warm-up differs): 160 served, 0 refused, 0 on version 8; latency P95 211.9 ms; stable version 7, document version 7
  500.0 ms begin -> warming
  1020.7 ms warm -> revert, verdict golden mismatch
  1020.7 ms drained -> idle, verdict golden mismatch
build answering every 8th request with an error: 160 served, 0 refused, 5 on version 8; latency P95 187.5 ms; stable version 7, document version 7
  500.0 ms begin -> warming
  1020.7 ms warm -> warming
  1020.7 ms warm -> warming
  1020.7 ms warm -> canary, share 20 percent
  2520.7 ms window -> revert, verdict error budget spent
  2667.6 ms drained -> idle, verdict error budget spent
recorded (chapter 172, 8 requests at 10 times speed), 2 replicas per build, one step of 50 percent:
good build: 8 served, 0 refused, 4 on version 8; latency P95 614.5 ms; stable version 8, document version 8
  0.0 ms begin -> warming
  520.7 ms warm -> warming
  520.7 ms warm -> canary, share 50 percent
  1120.7 ms window -> canary, share 50 percent, verdict wait
  1720.7 ms window -> verified, share 50 percent, verdict pass
  1720.7 ms promote -> retire, verdict pass
  1830.5 ms drained -> idle, verdict pass
```

What it shows:

- **Nothing is dropped.** In all five rollouts every request is served:
  0 refused, and every request finishes on the build it was routed to.
- **The good build is promoted.** It passes 20 percent at 2,520.7 ms and
  50 percent at 4,020.7 ms; promotion moves the document and the traffic
  to version 8 at that instant, and the old replicas finish what they hold
  by 4,161.8 ms. 107 of 160 requests run on version 8. P95 latency over
  the whole run is 187.5 ms, below the 211.9 ms of the same traffic on 3
  replicas with no rollout, because 6 replicas serve during the rollout.
- **The slow build is reverted** at its first window, 2,520.7 ms, with P95
  over the 250 ms objective. Five requests ran on it; the last one in
  flight finishes at 3,042.8 ms before its replica stops. P95 over the run
  is 212.6 ms against 211.9 ms with no rollout.
- **The wrong build never takes a request.** Its golden warm-up differs
  at 1,020.7 ms, the rollout reverts, and every request's replica, start
  and finish are exactly those of the run with no rollout (the frozen
  check compares all 160).
- **The error budget reverts** the build that errors, at its first
  window: at least 3 finished canary requests with an error among them is
  over 5 percent. The request still in flight on it finishes at 2,667.6 ms.
- **Recorded traffic.** Eight requests at 10 times speed, 2 replicas per
  build, one step of 50 percent and a 700 ms objective (this schedule's
  outputs of 16 and 32 tokens take longer). The first window has no
  finished canary request and waits; the second passes; 4 of 8 requests
  run on version 8. P95 is 614.5 ms against 602.9 ms with no rollout in
  chapter 177; eight requests are too few for the percentile to say more.

## How it is checked

<!-- source: psionic/lib/serve-rollout.coax check-frozen -->
```jai
#test "frozen: a good build promoted under light synthetic load" {
    w:=rox_world(60000);s:=ro_sim(rox_run(rox_good()),w.req[..],w.tok[..]);
    rox_check(s,rox_run(rox_good()),w.req[..]);
    #assert rox_frozen(s,w.req[..],8,107,0,187574,9519712) && s.state.verdict==RoVerdict.pass;
    // Six replicas during the rollout: P95 below the 211,960 us of the same traffic on three.
    off:RrScaler;c:=rrx_fleet_config(RrPolicy.affinity);c.replicas=3;#assert rr_report(rr_sim(c,off,w.req[..],w.tok[..]),w.req[..]).latency.p[2]==211960;
}
#test "frozen: a slow build reverted" {
    w:=rox_world(60000);b:=rox_good();b.slow_pct=300;s:=ro_sim(rox_run(b),w.req[..],w.tok[..]);
    rox_check(s,rox_run(b),w.req[..]);
    #assert rox_frozen(s,w.req[..],7,5,0,212669,9519712) && s.state.verdict==RoVerdict.slow;
}
#test "frozen: a wrong build fails its golden warm-up and never takes a request" {
    w:=rox_world(60000);b:=rox_good();b.golden_ok=false;s:=ro_sim(rox_run(b),w.req[..],w.tok[..]);
    rox_check(s,rox_run(b),w.req[..]);
    #assert rox_frozen(s,w.req[..],7,0,0,211960,9519712) && s.state.verdict==RoVerdict.golden && s.inputs==3;
    // Every request's fate is the one it has with no rollout at all.
    off:RrScaler;c:=rrx_fleet_config(RrPolicy.affinity);c.replicas=3;z:=rr_sim(c,off,w.req[..],w.tok[..]);
    for i:0..<160 {f:=s.run.fate[i];g:=z.fate[i];#assert s.ver[i]==7 && f.replica==g.replica && f.start==g.start && f.finish==g.finish && f.reused==g.reused;}
}
#test "frozen: a build that errors spends its budget and is reverted" {
    w:=rox_world(60000);b:=rox_good();b.err_every=8;s:=ro_sim(rox_run(b),w.req[..],w.tok[..]);
    rox_check(s,rox_run(b),w.req[..]);
    #assert rox_frozen(s,w.req[..],7,5,0,187574,9519712) && s.state.verdict==RoVerdict.errors;
}
#test "frozen: chapter 172's recorded schedule through a promotion" {
    w:=rox_recorded();r:=rox_run(rox_good());r.cfg.replicas=2;r.plan.replicas=2;r.plan.samples=1;r.plan.nsteps=1;r.plan.steps[0]=50;r.plan.window_us=600000;r.plan.p95_us=700000;r.begin_us=0;
    s:=ro_sim(r,w.req[0..<8],w.tok[..]);
    rox_check(s,r,w.req[0..<8]);
    #assert rox_frozen(s,w.req[0..<8],8,4,0,614593,2972111);
}
```

- **A brute-force controller.** `rox_step` in
  [`serve-rollout-checks.coax`](../tests/serve-rollout-checks.coax)
  writes which events each phase accepts as a 6 × 6 table and recomputes
  each transition separately, with the verdict's order expressed as
  overrides. For 3,000 random walks of 16 inputs each (random plans,
  documents with the wrong model, profile or digest, impossible
  observations, golden mismatches, rollbacks to wrong versions), the
  controller's state and refusal equal the table's at every input, and
  the invariants hold after each: a refusal changes nothing; the published
  document is the stable build's; the share is a planned step's only in
  `canary` or `verified` and 0 otherwise; the stable build changes only by
  promotion from `verified`, to a newer version.
- **Simulation invariants.** For 24 random rollouts (8 to 40 requests from
  chapter 172's schedule, 1 to 4 replicas per build, random plans, starts,
  speeds, errors and failed warm-ups), recomputed from the requests alone:
  every accepted request finishes, after it starts, on the build it was
  routed to; its time is the step model's for its reused blocks, stretched
  by its build's speed; its error flag is its build's; a request under the
  new build's document arrived after promotion and ran on the new build; a
  request on the new build under the old document arrived while the canary
  had traffic; a client within its share when served and at it when
  refused; the losing build's replicas, and only those, stopped once the
  rollout is over; one request at a time per replica.
- **Hand-worked cases.** The trace and the controller steps above, a
  client over its share refused on either build while the canary waits
  for a sample, a window's counts at its edges (a request that finished
  as the window opened is not counted; one that finished as it closed is),
  a long request that keeps an old replica up after promotion, the event
  limit at 14 event times, round robin among one build's replicas,
  every refusal at its bound, the verdict's edges (1 error in 20 is
  inside 5 percent, 1 in 19 is not), the split at 25 and 99 percent, the
  simulation's own bounds, and the frozen runs.

`coax mutate psionic/lib/serve-rollout.coax --root-only` kills 453 of
453 mutants.

## What remains

- **Real deployment.** Every replica is a region of one process's memory,
  timed by chapter 175's model. Starting real replica processes or
  machines, moving traffic between them over a network, and draining a
  real process before it exits are not built. Hosted machines need an
  owner-approved budget, recorded in `NEEDS_OWNER.md`.
- **Real golden evidence.** A build's golden answer is a flag in the
  simulation. Replaying golden requests through a real build and comparing
  their outputs with retained execution evidence, and chapter 218's
  self-check against shipped reference decisions, are not wired in.
- **Capability admission.** The document here is a version, model,
  profile and digest. Publishing chapter 173's full capability document,
  and admitting each request against it, is not wired in.
- **The autoscaler during a rollout.** Chapter 177's autoscaler is off
  while a rollout runs; how it should size each build is not decided.
- **Blue-green swaps.** Only canaries by share are built; a blue-green
  swap is a plan with one step, but switching all traffic at once without
  a canary is not offered.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
  (section 7.4 prefers canaries to blue-green swaps at GPU scale)
