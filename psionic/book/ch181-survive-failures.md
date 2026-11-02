# 181. Survive failures

> **Status: partly implemented.** The failure policies (profile
> sf-fault-v1) run on the CPU against a simulation of replicas in one
> process: injected crashes, hangs, slow replicas, network partitions and
> corrupt answers; health checks that eject a replica after consecutive
> failures and probe it back in; retries with an attempt limit and a retry
> budget; a hedge whose loser is cancelled; deadlines carried by every
> attempt; late answers fenced off; and one settlement and one usage record
> per request key. They are checked by a hand trace, a brute-force health
> checker, invariants recomputed from every simulated attempt over random
> fault schedules, and a frozen fault mix against a no-retry baseline. No
> real process fails: killing real devices, processes and machines, failover
> between machines or regions, data-residency routing, isolating tool
> execution and verifying weights on load are not built; the last section
> lists them.

At scale a failing device, process or machine is the normal case, not the
exception. This chapter keeps a service answering through five kinds of
failure, and keeps its bill right while it does: a request that took three
attempts is still one request, answered once and charged once.

The rules decide who is served and what is charged, so they are pure Coax
procedures in an `#authority` file,
[`serve-fault.coax`](../lib/serve-fault.coax), checked with zero surviving
mutants. They run against a discrete-event simulation in the style of
[chapter 177](ch177-route-queue-and-scale-replicas.md): replicas, requests
and faults are values, and time is integer microseconds. An attempt takes
chapter 177's time on [chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md)'s step
model, one prefill step over the prompt and one decode step for each further
output token. Prices and usage records are
[chapter 180](ch180-observe-meter-and-price.md)'s `bl_quote`, `bl_admit`,
`bl_settle` and `um_observe`, unchanged.

## Faults and what the client sees

A fault is a span of time on one replica. A *crash* kills the replica and
every attempt it holds, with their cached pages
([chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md)); its
supervisor ([chapter 22](ch022-run-it-live.md)) has it serving again at the
end of the span, so the span's length is the restart and cold start
([chapter 178](ch178-start-fast.md)). A *hang* stops answers to attempts it
starts. A *slow* replica takes `pct` percent of the modeled time. A
*partition* swallows attempts and cancels sent to the replica, and the
answers it sends, while it lasts. A *corrupt* replica's answers fail their
checksum.

The client never sees a fault, only its effect on one attempt: an answer,
a reset (`crashed`), a refusal (`down`, `deadline`), a corrupt answer, or
silence until its timeout.

<!-- source: psionic/lib/serve-fault.coax types -->
```jai
// crash: at `from` the replica dies with every attempt it holds; its
// supervisor has it serving again at `until`. hang: attempts it starts in
// the span never answer. slow: attempts it starts in the span take pct
// percent of their time. partition: attempts and cancels sent to it in the
// span never arrive, and answers it sends in the span are lost. corrupt:
// answers it sends in the span fail their checksum.
SfFault :: enum u8 {none;crash;hang;slow;partition;corrupt;}
SfSpan :: struct {replica:int;kind:SfFault;from:int;until:int;pct:int;}
// attempt_us: one attempt's timeout. tries: attempts per request, the first
// included. hedge_us: 0, or send a second attempt when the first has not
// answered after this long. budget_pct, reserve: retries and hedges stay at
// most budget_pct percent of first attempts, plus reserve. fails:
// consecutive failures that eject a replica; min_up: never eject below this
// many healthy replicas. probe_us: an ejected replica is probed at every
// multiple of it; oks: consecutive probe successes that readmit it.
SfPolicy :: struct {attempt_us:int;tries:int;hedge_us:int;budget_pct:int;reserve:int;fails:int;min_up:int;probe_us:int;oks:int;}
// A request: its idempotency key, arrival, tokens, and deadline after arrival.
SfRequest :: struct {key:int;arrive:int;prompt:int;output:int;deadline:int;}
SfKind :: enum u8 {first;retry;hedge;probe;}
// What the client saw of one attempt. down: sent to a crashed replica,
// refused at once. deadline: the replica refused it, since it could not
// finish by the request's deadline. An answer that comes after the client
// closed its attempt is fenced: the attempt is marked late and nothing else
// changes.
SfResult :: enum u8 {pending;ok;crashed;down;deadline;timeout;corrupt;cancelled;}
// answered; failed: no attempt left (limit, budget or no healthy replica);
// expired: the deadline came first; refused: the account cannot hold the
// quote; duplicate: the key was seen before, and its first request answers it.
SfEnd :: enum u8 {pending;answered;failed;expired;refused;duplicate;}
SfHealth :: struct {up:bool;fails:int;oks:int;}
// The ledger: the keys settled, the account and the usage metrics.
SfLedger :: struct {n:int;keys:[64]int;account:BlAccount;usage:UmMetrics;}
// twice: the key was settled before. full: 64 keys. bill, meter: chapter
// 180's settlement or usage record refused it.
SfCharge :: enum u8 {none;twice;full;bill;meter;}
SfSettle :: struct {cause:SfCharge;charge:int;ledger:SfLedger;}
// Why a request gets no further attempt.
SfWhy :: enum u8 {go;tries;time;budget;}
```

## The policy machines

Four small procedures decide everything that is policy. `sf_health` is
the health checker for one replica: client results count while it is up,
and `fails` consecutive failures eject it unless that would leave fewer than
`min_up` replicas up; while it is out only probes count, and `oks`
consecutive probe successes readmit it. `sf_extra_ok` is the retry budget:
retries and hedges together may not exceed `budget_pct` percent of first
attempts plus a small `reserve`. This is what stops a retry storm: when
every replica fails, a client without a budget triples the load at the
worst moment; with one, the extra load is bounded however bad things get.
`sf_again` decides whether a request may send another attempt: its attempt
limit, then its deadline (an attempt that cannot finish in time is not
sent), then the budget. `sf_pick` chooses a healthy replica the request has
not tried, with the fewest attempts open, then the fewest sent.

<!-- source: psionic/lib/serve-fault.coax machines -->
```jai
sf_health :: (p:SfPolicy,h:SfHealth,ok:bool,probe:bool,ups:int)->SfHealth {
    n:=h;
    // Up: client results count; probes are not sent.
    if h.up && !probe {
        if ok {n.fails=0;}
        else {
            n.fails=cb_min(h.fails+1,p.fails);
            if n.fails==p.fails && ups>p.min_up {n.up=false;}
        }
    }
    // Out: only probes count.
    if !h.up && probe {
        n.oks=ifx ok then h.oks+1 else 0;
        if n.oks==p.oks {n=SfHealth.{up=true};}
    }
    return n;
}
// Retries and hedges together stay within budget_pct percent of first attempts, plus reserve.
sf_extra_ok :: (p:SfPolicy,firsts:int,extras:int)->bool {return (extras+1)*100<=p.budget_pct*firsts+p.reserve*100;}
// Whether a request may send another attempt now: its attempt limit, then
// its deadline (an attempt that cannot finish in time is not sent), then,
// for any attempt after the first, the budget.
sf_again :: (p:SfPolicy,tries:int,firsts:int,extras:int,now:int,deadline:int,cost:int)->SfWhy {
    if tries>=p.tries {return SfWhy.tries;}
    if now+cost>deadline {return SfWhy.time;}
    if tries>0 && !sf_extra_ok(p,firsts,extras) {return SfWhy.budget;}
    return SfWhy.go;
}
// A healthy replica this request has not tried (tried is a bit mask), with
// the fewest attempts open, then the one sent a request longest ago (last
// holds when, as a count of attempts; 0 is never), then the lowest number;
// -1 when there is none.
sf_pick :: (h:[]SfHealth,load:[]int,last:[]int,tried:int)->int {
    best:=-1;
    for k:0..<h.count {
        if h[k].up && (tried>>k)%2==0 && (best<0 || load[k]<load[best] || (load[k]==load[best] && last[k]<last[best])) {best=k;}
    }
    return best;
}
```

The chapter's checks use `sfx_policy`: 3 tries, 2 failures eject, 2 probe
successes readmit, a budget of 10 percent plus 1. Worked by hand:

| Call | Arithmetic | Result |
| --- | --- | --- |
| `sf_health`, a failure, 3 up | fails 0 + 1 = 1 < 2 | up, fails 1 |
| a second failure, 3 up | fails 2 = 2 and 3 > 1 | out |
| the same with 1 up | 1 is not more than `min_up` 1 | up, fails stays 2 |
| `sf_extra_ok`, 9 first attempts, 1 extra so far | (1 + 1) × 100 = 200 > 10 × 9 + 100 = 190 | denied |
| 10 first attempts | 200 ≤ 10 × 10 + 100 = 200 | allowed |
| `sf_again` at 901 µs, deadline 1,000, cost 100 | 901 + 100 > 1,000 | `time` |

<!-- source: psionic/lib/serve-fault.coax check-machines -->
```jai
#test "health by hand: two failures eject, two probe successes readmit" {
    p:=sfx_policy();h:=SfHealth.{up=true};
    h=sf_health(p,h,false,false,3);#assert h.up && h.fails==1;
    h=sf_health(p,h,true,false,3);#assert h.up && h.fails==0;
    h=sf_health(p,h,false,false,3);h=sf_health(p,h,false,false,3);#assert !h.up && h.fails==2 && h.oks==0;
    // Out: client answers do not count, only probes; a failed probe starts the count again.
    #assert sfx_same(sf_health(p,h,true,false,2),h) && sfx_same(sf_health(p,h,false,false,2),h);
    h=sf_health(p,h,true,true,2);#assert !h.up && h.oks==1;
    h=sf_health(p,h,false,true,2);#assert !h.up && h.oks==0;
    h=sf_health(p,h,true,true,2);h=sf_health(p,h,true,true,2);#assert h.up && h.fails==0 && h.oks==0;
    // Up: a probe result changes nothing. At min_up healthy replicas, no ejection; the count stops at the limit.
    #assert sfx_same(sf_health(p,h,false,true,3),h);
    h=sf_health(p,h,false,false,1);h=sf_health(p,h,false,false,1);h=sf_health(p,h,false,false,1);#assert h.up && h.fails==2;
    h=sf_health(p,h,false,false,2);#assert !h.up;
}
#test "the retry budget and the retry decision by hand" {
    p:=sfx_policy();  // 10 percent of first attempts, plus 1
    #assert sf_extra_ok(p,0,0) && !sf_extra_ok(p,0,1) && !sf_extra_ok(p,9,1) && sf_extra_ok(p,10,1) && !sf_extra_ok(p,19,2) && sf_extra_ok(p,20,2);
    // 3 tries; deadline 1,000; an attempt costs 100.
    #assert sf_again(p,0,0,5,900,1000,100)==SfWhy.go;
    #assert sf_again(p,0,0,5,901,1000,100)==SfWhy.time;
    #assert sf_again(p,1,10,1,900,1000,100)==SfWhy.go && sf_again(p,1,10,2,900,1000,100)==SfWhy.budget;
    #assert sf_again(p,2,10,1,901,1000,100)==SfWhy.time && sf_again(p,3,100,0,0,1000,100)==SfWhy.tries;
    // The pick: healthy, not tried by this request, least load, sent to longest ago, lowest index.
    h:[4]SfHealth;for k:0..<4 {h[k].up=true;}
    load:=[4]int.[2,1,1,0];last:=[4]int.[0,5,4,9];
    #assert sf_pick(h[..],load[..],last[..],0)==3 && sf_pick(h[..],load[..],last[..],8)==2 && sf_pick(h[..],load[..],last[..],12)==1 && sf_pick(h[..],load[..],last[..],14)==0;
    last[2]=5;#assert sf_pick(h[..],load[..],last[..],8)==1;last[2]=6;load[2]=0;#assert sf_pick(h[..],load[..],last[..],8)==2;
    h[0].up=false;#assert sf_pick(h[..],load[..],last[..],14)== -1;
}
```

`sfx_replay`, in the checks file, is an independent health checker. It
keeps no counters: at every step it replays the replica's whole history,
finds where the current up or out stretch began, and counts the trailing
run of failures or probe successes from there. A `#forall` drives 1,000
random histories of 24 results through both and requires the same state
after every one.

## Exactly-once billing

A request carries an idempotency key. At arrival the service quotes its
worst case and holds the quote against the account (chapter 180's
`bl_admit`); if the account cannot hold it, the request is refused before
any attempt. When the request ends, `sf_settle` settles its key once:
`bl_settle` charges the smaller of the price and the quote and releases the
hold, and `um_observe` folds one usage record into the metrics. A key
already in the ledger is refused and nothing changes, so a second
settlement of one request is impossible however many attempts it took.

<!-- source: psionic/lib/serve-fault.coax ledger -->
```jai
// Settle one key: chapter 180 charges the smaller of the price and the
// quote and releases the hold, and the usage record is folded into the
// metrics. A key already settled is refused and changes nothing.
sf_settle :: (l:SfLedger,ev:UmEvidence,s:BlSchedule,key:int,q:BlPrice,u:BlUse,latency:int)->SfSettle {
    r:=SfSettle.{cause=SfCharge.twice,ledger=l};
    for i:0..<l.n {if l.keys[i]==key {return r;}}
    r.cause=SfCharge.full;
    if l.n==l.keys.count {return r;}
    b:=bl_settle(s,l.account,q,u);
    r.cause=SfCharge.bill;
    if b.cause!=BlCause.none {return r;}
    m:=um_observe(l.usage,UmRecord.{seq=l.n+1,evidence=ev,use=u,latency_us=latency,quote_msat=q.msat,charge_msat=b.charge});
    r.cause=SfCharge.meter;
    if m.cause!=UmCause.none {return r;}
    n:=l;n.keys[n.n]=key;n.n+=1;n.account=b.account;n.usage=m.metrics;
    return SfSettle.{charge=b.charge,ledger=n};
}
```

Only the answer that won is priced: its tokens and its replica's device
time. A failed attempt, a hedge that lost and a retry that never ran are
the service's cost, not the client's. A request that ends without an answer
settles as `busy`, which chapter 180 prices at 0, and still leaves its one
usage record. A request that resends a key already seen is answered by the
first request with that key: no attempt, no hold, no charge, no record.

With chapter 180's test schedule (1,000 msat a request, 2,000 per 1,000
input tokens, 8,000 per 1,000 output tokens, 50,000 per device-second), a
request of 8 prompt tokens and 2 output tokens with a 300,000 µs deadline:

- quote: 1,000 + ⌈(8 × 2,000 + 2 × 8,000) × 1,000 + 300,000 × 50,000) /
  1,000,000⌉ = 1,000 + ⌈15,032,000,000 / 1,000,000⌉ = 16,032 msat;
- one attempt: (17,792,472 + 7 × 1,459,182 + 17,792,472) / 1,000 =
  45,799 µs;
- charge: 1,000 + ⌈(32,000,000 + 45,799 × 50,000) / 1,000,000⌉ = 1,000 +
  ⌈2,321.95⌉ = 3,322 msat.

<!-- source: psionic/lib/serve-fault.coax check-ledger -->
```jai
#test "the ledger settles a key once" {
    s:=bl_test_schedule();ev:=um_test_evidence();l:=SfLedger.{account=BlAccount.{budget=100000}};
    q:=bl_quote(s,8,2,500000);a:=bl_admit(l.account,q);l.account=a.account;
    #assert q.msat==26032 && l.account.held==26032;
    u:=BlUse.{status=BlStatus.answered,tokens_in=8,tokens_out=2,device_us=45799};
    r:=sf_settle(l,ev,s,7,q,u,45799);
    // 1,000 + ceil((8 x 2,000 + 2 x 8,000) x 1,000 + 45,799 x 50,000, over 1,000,000) = 1,000 + 2,322.
    #assert r.cause==SfCharge.none && r.charge==3322 && r.ledger.n==1 && r.ledger.keys[0]==7 && r.ledger.account.spent==3322 && r.ledger.account.held==0;
    #assert r.ledger.usage.requests==1 && r.ledger.usage.charged==3322 && r.ledger.usage.quoted==26032;
    // The same key again: refused, nothing changes.
    t:=sf_settle(r.ledger,ev,s,7,BlPrice.{},BlUse.{status=BlStatus.refused},0);
    #assert t.cause==SfCharge.twice && t.charge==0 && sfx_ledger_same(t.ledger,r.ledger);
    // Another key with nothing held: a refusal record, charge 0.
    t=sf_settle(r.ledger,ev,s,8,BlPrice.{},BlUse.{status=BlStatus.refused},0);
    #assert t.cause==SfCharge.none && t.ledger.n==2 && t.ledger.usage.requests==2 && t.ledger.usage.status[2]==1 && t.ledger.account.spent==3322;
    // A quote never held, or bad evidence: refused by chapter 180, nothing changes.
    t=sf_settle(r.ledger,ev,s,9,q,u,45799);#assert t.cause==SfCharge.bill && sfx_ledger_same(t.ledger,r.ledger);
    bad:=ev;bad.backend="";t=sf_settle(r.ledger,bad,s,9,BlPrice.{},u,45799);#assert t.cause==SfCharge.meter && sfx_ledger_same(t.ledger,r.ledger);
    // 64 keys fill it.
    f:=SfLedger.{account=BlAccount.{budget=100}};
    for k:0..<64 {f=sf_settle(f,ev,s,k,BlPrice.{},BlUse.{status=BlStatus.refused},0).ledger;}
    #assert f.n==64 && sf_settle(f,ev,s,64,BlPrice.{},BlUse.{status=BlStatus.refused},0).cause==SfCharge.full;
}
```

## The simulation

`sf_sim` runs requests against replicas under a list of faults. Each
replica runs one attempt at a time, oldest first. Every attempt carries its
request's deadline, so a replica about to start one that cannot finish in
time refuses it unrun instead of spending device time on an answer nobody
will take ([chapter 28](ch028-a-decision-arrives-while-play-continues.md)'s
deadline). The client gives up on an attempt at its own timeout or at the
request's deadline, whichever comes first, and cancels it; a cancel sent
into a partition is lost, and the replica keeps working.

An answer is taken only if its attempt is still open. Answering a request
closes every other attempt it has, at once, so an answer that comes later,
from a replica that never heard the cancel, finds its attempt closed: it
is fenced, marked late, and changes nothing. That is the only way the
simulation lets an answer arrive twice, and the checks require that it
never counts twice.

<!-- source: psionic/lib/serve-fault.coax sim -->
```jai
// Close one attempt as the client sees it.
sf_close :: (att:[]SfAttempt,out:[]SfOutcome,load:[]int,probing:[]bool,a:int,res:SfResult,now:int) {
    x:=att[a];x.result=res;x.ended=now;att[a]=x;load[x.replica]-=1;
    if x.req>=0 {out[x.req].open-=1;} else {probing[x.replica]=false;}
}
// A result reaches the health checker.
sf_heard :: (p:SfPolicy,h:[]SfHealth,k:int,ok:bool,probe:bool,c:SfCount)->SfCount {
    ups:=0;for j:0..<h.count {if h[j].up {ups+=1;}}
    n:=c;was:=h[k].up;h[k]=sf_health(p,h[k],ok,probe,ups);
    if was && !h[k].up {n.ejects+=1;}
    if !was && h[k].up {n.readmits+=1;}
    return n;
}
// Send an attempt, unless the record is full. A crashed replica refuses it
// at once; a partitioned one never hears of it.
sf_send :: (r:SfRun,att:[]SfAttempt,out:[]SfOutcome,load:[]int,probing:[]bool,last:[]int,h:[]SfHealth,req:int,k:int,kind:SfKind,now:int,c:SfCount)->SfCount {
    n:=c;a:=n.na;
    if a==att.count {n.full=true;return n;}
    n.na+=1;
    att[a]=SfAttempt.{req=req,replica=k,kind=kind,sent=now,start=-1,finish=-1,live=true};load[k]+=1;
    if req>=0 {o:=out[req];o.tries+=1;o.open+=1;o.tried+=1<<k;out[req]=o;last[k]=n.na;} else {probing[k]=true;}
    if sf_in(r,k,SfFault.partition,now) {att[a].live=false;}
    else if sf_in(r,k,SfFault.crash,now) {att[a].live=false;sf_close(att,out,load,probing,a,SfResult.down,now);n=sf_heard(r.policy,h,k,false,kind==SfKind.probe,n);}
    return n;
}
// The deadline an attempt carries, and when the client gives up on it.
sf_due :: (r:SfRun,reqs:[]SfRequest,x:SfAttempt)->int {
    if x.req<0 {return x.sent+r.policy.attempt_us;}
    q:=reqs[x.req];return cb_min(x.sent+r.policy.attempt_us,q.arrive+q.deadline);
}
// Chapter 177's attempt time on chapter 175's step model: one prefill step
// over the prompt, then one decode step per further output token.
sf_time :: (prompt:int,output:int)->int {return (cb_step_ns(prompt)+(output-1)*cb_step_ns(1))/1000;}
sf_cost :: (reqs:[]SfRequest,req:int)->int {
    if req<0 {return SF_PROBE_US;}
    return sf_time(reqs[req].prompt,reqs[req].output);
}
// The discrete-event simulation. At each event time, in this order:
// crashes kill what their replica holds; attempts finish and their answers
// are delivered, lost, rejected as corrupt or fenced; attempts past their
// timeout are cancelled; requests arrive; ejected replicas are probed;
// then, until nothing changes, requests with no attempt open are retried or
// settled, due hedges are sent, and idle replicas start their next attempt
// or refuse it for its deadline.
sf_sim :: (r:SfRun,reqs:[]SfRequest)->SfSim {
    s:SfSim;s.status=sf_check(r,reqs);
    if s.status!=SfStatus.ok {return s;}
    p:=r.policy;s.n=reqs.count;s.ledger.account.budget=r.budget;
    for k:0..<r.replicas {s.health[k].up=true;}
    h:=s.health[0..<r.replicas];busy_on:[8]int;load:[8]int;probing:[8]bool;last:[8]int;c:SfCount;a:=0;
    for k:0..<SF_REPLICAS {busy_on[k]=-1;}
    while true {
        busy:=a<s.n;t:=SF_NEVER;
        if busy {t=reqs[a].arrive;}
        for i:0..<a {
            o:=s.out[i];
            if o.end==SfEnd.pending && p.hedge_us>0 && !o.hedged {t=cb_min(t,reqs[i].arrive+p.hedge_us);}
        }
        for j:0..<c.na {
            x:=s.att[j];
            if x.result==SfResult.pending {busy=true;t=cb_min(t,sf_due(r,reqs,x));}
        }
        for k:0..<r.replicas {if busy_on[k]>=0 {t=cb_min(t,s.att[busy_on[k]].finish);}}
        // Probes go out only while an attempt is open; a crash matters only
        // while something is under way.
        if busy {for k:0..<r.replicas {if !h[k].up {t=cb_min(t,(s.end/p.probe_us+1)*p.probe_us);}}}
        if t<SF_NEVER {for j:0..<r.nfaults {if r.faults[j].kind==SfFault.crash && r.faults[j].from>s.end {t=cb_min(t,r.faults[j].from);}}}
        if t==SF_NEVER {break;}
        s.events+=1;
        if s.events>r.events {s.status=SfStatus.events;return sf_done(s,c);}
        now:=t;s.end=now;
        // Crashes: what the replica holds is gone, its cached pages with it.
        for f:0..<r.nfaults {
            k:=r.faults[f].replica;
            if r.faults[f].kind==SfFault.crash && r.faults[f].from==now {
                busy_on[k]=-1;
                for j:0..<c.na {
                    x:=s.att[j];
                    if x.replica==k && x.live {
                        s.att[j].live=false;
                        if x.result==SfResult.pending && !sf_in(r,k,SfFault.partition,now) {sf_close(s.att[..],s.out[..],load[..],probing[..],j,SfResult.crashed,now);c=sf_heard(p,h,k,false,x.kind==SfKind.probe,c);}
                    }
                }
            }
        }
        // Finishes: the answer travels unless a partition swallows it.
        for k:0..<r.replicas {
            j:=busy_on[k];
            if j>=0 && s.att[j].finish==now {
                busy_on[k]=-1;s.att[j].live=false;x:=s.att[j];
                if !sf_in(r,k,SfFault.partition,now) {
                    if x.result!=SfResult.pending {s.att[j].late=true;c.fenced+=1;}
                    else if sf_in(r,k,SfFault.corrupt,now) {sf_close(s.att[..],s.out[..],load[..],probing[..],j,SfResult.corrupt,now);c=sf_heard(p,h,k,false,x.kind==SfKind.probe,c);}
                    else {
                        c=sf_heard(p,h,k,true,x.kind==SfKind.probe,c);
                        // An attempt still open belongs to a request still open: answering
                        // a request closes all its attempts at once.
                        if x.req<0 {sf_close(s.att[..],s.out[..],load[..],probing[..],j,SfResult.ok,now);}
                        else {
                            sf_close(s.att[..],s.out[..],load[..],probing[..],j,SfResult.ok,now);
                            o:=s.out[x.req];o.end=SfEnd.answered;o.at=now;o.winner=j;s.out[x.req]=o;
                            // The other attempts are cancelled; a partitioned replica never hears it.
                            for i:0..<c.na {
                                y:=s.att[i];
                                if y.req==x.req && y.result==SfResult.pending {
                                    sf_close(s.att[..],s.out[..],load[..],probing[..],i,SfResult.cancelled,now);
                                    if !sf_in(r,y.replica,SfFault.partition,now) {s.att[i].live=false;if busy_on[y.replica]==i {busy_on[y.replica]=-1;}}
                                }
                            }
                            s.ledger=sf_bill(r,reqs,s.out[..],s.att[..],s.ledger,x.req,now);
                        }
                    }
                }
            }
        }
        // Timeouts: the client gives up and cancels.
        for j:0..<c.na {
            x:=s.att[j];
            if x.result==SfResult.pending && sf_due(r,reqs,x)==now {
                sf_close(s.att[..],s.out[..],load[..],probing[..],j,SfResult.timeout,now);c=sf_heard(p,h,x.replica,false,x.kind==SfKind.probe,c);
                if !sf_in(r,x.replica,SfFault.partition,now) {s.att[j].live=false;if busy_on[x.replica]==j {busy_on[x.replica]=-1;}}
            }
        }
        // Arrivals. A key seen before is answered by its first request.
        while a<s.n && reqs[a].arrive==now {
            o:SfOutcome;
            for i:0..<a {if reqs[i].key==reqs[a].key {o.end=SfEnd.duplicate;o.at=now;}}
            if o.end==SfEnd.pending {
                q:=bl_quote(r.price,reqs[a].prompt,reqs[a].output,reqs[a].deadline);
                b:=bl_admit(s.ledger.account,q);
                if b.cause==BlCause.none {s.ledger.account=b.account;o.quote=q.msat;}
                else {o.end=SfEnd.refused;o.at=now;}
            }
            s.out[a]=o;
            if o.end==SfEnd.refused {s.ledger=sf_bill(r,reqs,s.out[..],s.att[..],s.ledger,a,now);}
            a+=1;
        }
        // Probes, one at a time to each ejected replica.
        if now%p.probe_us==0 {
            for k:0..<r.replicas {
                if !h[k].up && !probing[k] {c=sf_send(r,s.att[..],s.out[..],load[..],probing[..],last[..],h,-1,k,SfKind.probe,now,c);}
            }
        }
        moved:=true;
        while moved && !c.full {
            moved=false;
            // Requests with nothing open: another attempt, or the end.
            for i:0..<a {
                o:=s.out[i];
                if o.end==SfEnd.pending && o.open==0 {
                    why:=sf_again(p,o.tries,c.firsts,c.extras,now,reqs[i].arrive+reqs[i].deadline,sf_cost(reqs,i));
                    k:=sf_pick(h,load[..],last[..],o.tried);
                    if why==SfWhy.go && k>=0 {
                        if o.tries==0 {c.firsts+=1;c=sf_send(r,s.att[..],s.out[..],load[..],probing[..],last[..],h,i,k,SfKind.first,now,c);}
                        else {c.extras+=1;c=sf_send(r,s.att[..],s.out[..],load[..],probing[..],last[..],h,i,k,SfKind.retry,now,c);}
                    } else {
                        if why==SfWhy.budget {c.denied+=1;}
                        o.end=ifx why==SfWhy.time then SfEnd.expired else SfEnd.failed;o.at=now;s.out[i]=o;
                        s.ledger=sf_bill(r,reqs,s.out[..],s.att[..],s.ledger,i,now);
                    }
                    moved=true;
                }
            }
            // Hedges: one, when the only attempt has not answered after hedge_us.
            for i:0..<a {
                o:=s.out[i];
                if o.end==SfEnd.pending && p.hedge_us>0 && !o.hedged && reqs[i].arrive+p.hedge_us==now {
                    s.out[i].hedged=true;
                    why:=sf_again(p,o.tries,c.firsts,c.extras,now,reqs[i].arrive+reqs[i].deadline,sf_cost(reqs,i));
                    k:=sf_pick(h,load[..],last[..],o.tried);
                    if why==SfWhy.budget {c.denied+=1;}
                    if why==SfWhy.go && k>=0 {c.extras+=1;c=sf_send(r,s.att[..],s.out[..],load[..],probing[..],last[..],h,i,k,SfKind.hedge,now,c);}
                }
            }
            // Idle replicas start their oldest attempt. The deadline travels
            // with it: one that cannot finish in time is refused unrun.
            for k:0..<r.replicas {
                if busy_on[k]<0 && !sf_in(r,k,SfFault.crash,now) {
                    j:=-1;
                    for i:0..<c.na {if s.att[i].replica==k && s.att[i].live && s.att[i].start== -1 {j=i;break;}}
                    if j>=0 {
                        x:=s.att[j];moved=true;
                        if x.req>=0 && now+sf_cost(reqs,x.req)>reqs[x.req].arrive+reqs[x.req].deadline {
                            s.att[j].live=false;
                            if x.result==SfResult.pending && !sf_in(r,k,SfFault.partition,now) {sf_close(s.att[..],s.out[..],load[..],probing[..],j,SfResult.deadline,now);}
                        } else {
                            s.att[j].start=now;busy_on[k]=j;
                            s.att[j].finish=ifx sf_in(r,k,SfFault.hang,now) then SF_NEVER else now+sf_cost(reqs,x.req)*sf_pct(r,k,now)/100;
                        }
                    }
                }
            }
        }
        if c.full {s.status=SfStatus.attempts;return sf_done(s,c);}
    }
    return sf_done(s,c);
}
// Settle a request that just ended: the winner's tokens and device time if
// it was answered, nothing otherwise (refused before any attempt, or busy:
// no attempt answered).
sf_bill :: (r:SfRun,reqs:[]SfRequest,out:[]SfOutcome,att:[]SfAttempt,l:SfLedger,i:int,now:int)->SfLedger {
    o:=out[i];q:=reqs[i];u:=BlUse.{status=ifx o.end==SfEnd.refused then BlStatus.refused else BlStatus.busy};
    if o.end==SfEnd.answered {w:=att[o.winner];u=BlUse.{status=BlStatus.answered,tokens_in=q.prompt,tokens_out=q.output,device_us=w.finish-w.start};}
    t:=sf_settle(l,r.evidence,r.price,q.key,BlPrice.{msat=o.quote},u,now-q.arrive);
    out[i].charge=t.charge;
    return t.ledger;
}
```

Probes are one prompt token and one output token, 17,792 µs, sent at every
multiple of `probe_us` to each ejected replica that has no probe open. They
go out only while requests are still to arrive or an attempt (a probe
included) is open, so an idle service stops probing.

## A crash, traced by hand

Two replicas. Replica 0 crashes at 120,000 µs and serves again at
400,000 µs. Policy: 2 tries, no hedge, 2 failures eject, 1 probe success
readmits, probes every 200,000 µs, a budget of 10 percent plus 1, attempts
time out after 200,000 µs. Seven requests of 8 prompt tokens and 2 output
tokens with a 300,000 µs deadline arrive at 0, 10, 100, 130, 200, 400 and
410 ms; the last resends request 2's key.

| Time (µs) | What happens |
| --- | --- |
| 0 | request 0 to replica 0 (both idle, none sent; lowest number); answered at 45,799 |
| 10,000 | request 1 to replica 1 (replica 0 is busy); answered at 55,799 |
| 100,000 | request 2 to replica 0 (both idle, one sent each), to finish at 145,799 |
| 120,000 | replica 0 crashes: request 2's attempt is `crashed`, replica 0's first failure. Retry: (0 + 1) × 100 = 100 ≤ 10 × 3 + 100 = 130. Replica 0 was tried, so replica 1; answered at 165,799 |
| 130,000 | request 3 to replica 0 (replica 1 is busy): `down` at once, the second failure in a row; 2 up > 1, so replica 0 is ejected. Retry: 200 > 10 × 4 + 100 = 140: denied. Request 3 fails, charged 0 |
| 200,000 | the probe tick: replica 0 is still down, the probe fails. Request 4 to replica 1, the only healthy one |
| 400,000 | the probe tick: replica 0 serves again; the probe answers at 417,792 and readmits it. Request 5 had already gone to replica 1 |
| 410,000 | request 6, key 2: a duplicate, answered by request 2 |

Five answers at 3,322 msat each: 16,610 msat charged over six usage
records, nothing left held.

<!-- source: psionic/lib/serve-fault.coax check-hand -->
```jai
#test "the hand trace: a crash, an ejection, a retry, a probe, one charge per key" {
    w:=sfx_hand();s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    #assert s.status==SfStatus.ok && s.na==9 && s.firsts==6 && s.extras==1 && s.denied==1 && s.ejects==1 && s.readmits==1 && s.events==14;
    a:=s.att;o:=s.out;
    // Requests 0 and 1 on replicas 0 and 1: 45,799 us each, 3,322 msat each against a 16,032 msat quote.
    #assert sfx_att(a[0],0,0,SfKind.first,0,0,45799,SfResult.ok) && o[0].end==SfEnd.answered && o[0].at==45799 && o[0].quote==16032 && o[0].charge==3322;
    #assert sfx_att(a[1],1,1,SfKind.first,10000,10000,55799,SfResult.ok) && o[1].charge==3322;
    // Request 2 on replica 0 dies with it at 120,000 us; the retry on replica 1 runs 120,000 to 165,799.
    #assert sfx_att(a[2],2,0,SfKind.first,100000,100000,145799,SfResult.crashed) && a[2].ended==120000;
    #assert sfx_att(a[3],2,1,SfKind.retry,120000,120000,165799,SfResult.ok) && o[2].end==SfEnd.answered && o[2].at==165799;
    #assert o[2].tries==2 && o[2].winner==3 && o[2].charge==3322;
    // Request 3 meets the crashed replica at 130,000 us: refused at once, its second failure, ejected.
    // A retry would be the second extra attempt over 4 first ones: 200 > 10 x 4 + 100. Failed, charged nothing.
    #assert sfx_att(a[4],3,0,SfKind.first,130000,-1,-1,SfResult.down) && o[3].end==SfEnd.failed && o[3].at==130000 && o[3].charge==0;
    // The probe at 200,000 us finds it still down; request 4 goes to replica 1, the only one healthy.
    #assert sfx_att(a[5],-1,0,SfKind.probe,200000,-1,-1,SfResult.down) && sfx_att(a[6],4,1,SfKind.first,200000,200000,245799,SfResult.ok);
    // At 400,000 us it serves again: the probe answers in 17,792 us and readmits it; request 5 went to replica 1.
    #assert sfx_att(a[7],-1,0,SfKind.probe,400000,400000,417792,SfResult.ok) && sfx_att(a[8],5,1,SfKind.first,400000,400000,445799,SfResult.ok);
    #assert s.health[0].up && s.health[1].up;
    // Request 6 resends key 2: answered by request 2, no attempt, no charge, no record.
    #assert o[6].end==SfEnd.duplicate && o[6].tries==0 && o[6].charge==0;
    // One usage record per key: 6 records, 5 x 3,322 = 16,610 msat charged, nothing left held.
    #assert s.ledger.n==6 && s.ledger.usage.requests==6 && s.ledger.usage.charged==16610 && s.ledger.account.spent==16610 && s.ledger.account.held==0;
}
#test "a hedge wins during a partition; the loser's late answer is fenced" {
    w:=sfx_fence();s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    a:=s.att;o:=s.out[0];
    // Replica 0 runs 3 times slow, 0 to 137,397 us. The hedge at 30,000 us answers at 75,799 on replica 1.
    #assert s.na==2 && sfx_att(a[0],0,0,SfKind.first,0,0,137397,SfResult.cancelled) && sfx_att(a[1],0,1,SfKind.hedge,30000,30000,75799,SfResult.ok);
    // The cancel at 75,799 us is lost in the partition; replica 0's answer arrives at 137,397 and is fenced.
    #assert a[0].ended==75799 && a[0].late && !a[1].late && s.fenced==1 && o.end==SfEnd.answered && o.at==75799 && o.winner==1;
    // One charge, for the hedge's 45,799 us.
    #assert o.charge==3322 && s.ledger.n==1 && s.ledger.usage.charged==3322 && s.events==4;
}
#test "a hedge after 1 us, and a hedge the budget denies" {
    w:=sfx_fence();w.run.policy.hedge_us=1;s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    #assert sfx_att(s.att[1],0,1,SfKind.hedge,1,1,45800,SfResult.ok) && s.att[0].late && s.out[0].at==45800;
    w=sfx_fence();w.run.policy.budget_pct=0;w.run.policy.reserve=0;s=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // No hedge: the slow answer leaves after the partition and arrives at 137,397 us.
    #assert s.na==1 && s.denied==1 && s.extras==0 && sfx_att(s.att[0],0,0,SfKind.first,0,0,137397,SfResult.ok) && s.out[0].at==137397;
}
#test "a crash under a partition is heard only as a timeout; a probe into the partition times out" {
    w:=sfx_fence();w.run.policy.hedge_us=0;w.run.policy.fails=1;w.run.policy.probe_us=100000;
    w.run.faults[1].until=400000;w.run.nfaults=3;w.run.faults[2]=SfSpan.{replica=0,kind=SfFault.crash,from=50000,until=60000};
    s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // The crash at 50,000 us kills attempt 0 unheard; the client gives up at 200,000 and ejects replica 0.
    #assert sfx_att(s.att[0],0,0,SfKind.first,0,0,137397,SfResult.timeout) && s.att[0].ended==200000 && !s.att[0].late;
    #assert sfx_att(s.att[2],0,1,SfKind.retry,200000,200000,245799,SfResult.ok) && s.ejects==1 && s.out[0].at==245799;
    // The probe sent at 200,000 us into the partition is still open at 300,000, so no second one;
    // it times out at 400,000, and the probe sent then answers and readmits.
    #assert s.na==4 && sfx_att(s.att[1],-1,0,SfKind.probe,200000,-1,-1,SfResult.timeout) && s.att[1].ended==400000;
    #assert sfx_att(s.att[3],-1,0,SfKind.probe,400000,400000,417792,SfResult.ok) && s.readmits==1 && s.health[0].up;
}
#test "a crash after a lost cancel ends the late answer; a crash after the run changes nothing" {
    w:=sfx_fence();w.run.nfaults=3;w.run.faults[2]=SfSpan.{replica=0,kind=SfFault.crash,from=100000,until=110000};
    s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    #assert s.att[0].result==SfResult.cancelled && s.att[0].ended==75799 && !s.att[0].late && s.fenced==0 && s.out[0].charge==3322;
    h:=sfx_hand();h.run.nfaults=2;h.run.faults[1]=SfSpan.{replica=1,kind=SfFault.crash,from=900000,until=950000};
    s=sf_sim(h.run,h.req[0..<h.n]);#assert s.events==14 && s.end==445799;
}
#test "min_up keeps a failing replica in rotation" {
    w:=sfx_hand();w.run.policy.min_up=2;s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // Two failures in a row on replica 0, but ejecting it would leave 1 of the 2 that must stay: no probe, and
    // request 5 goes to it once it serves again.
    #assert s.ejects==0 && s.na==7 && sfx_att(s.att[4],3,0,SfKind.first,130000,-1,-1,SfResult.down) && sfx_att(s.att[6],5,0,SfKind.first,400000,400000,445799,SfResult.ok);
}
#test "the deadline travels with the attempt" {
    w:=sfx_hand();w.run.replicas=1;w.run.nfaults=0;w.n=3;
    w.req[1].deadline=60000;w.req[2]=SfRequest.{key=2,arrive=20000,prompt=8,output=2,deadline=71598};
    s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // At 45,799 us request 1 would finish at 91,598, past its 70,000: the replica refuses it unrun, and no time is left.
    #assert sfx_att(s.att[1],1,0,SfKind.first,10000,-1,-1,SfResult.deadline) && s.att[1].ended==45799 && s.out[1].end==SfEnd.expired && s.out[1].at==45799;
    // Request 2 finishes exactly at its deadline, 91,598 us: answered.
    #assert sfx_att(s.att[2],2,0,SfKind.first,20000,45799,91598,SfResult.ok) && s.out[2].at==91598;
    p:=sf_report(s,w.req[0..<w.n]);#assert p.answered==2 && p.expired==1 && p.failed==0 && p.p95_us==71598;
}
#test "an account that cannot hold a quote is refused before any attempt" {
    w:=sfx_hand();w.run.budget=20000;w.n=2;s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // 16,032 held for request 0 leaves 3,968: request 1 is refused, with a record and no charge.
    #assert s.out[1].end==SfEnd.refused && s.out[1].at==10000 && s.na==1 && s.ledger.n==2 && s.ledger.account.spent==3322;
    p:=sf_report(s,w.req[0..<w.n]);#assert p.answered==1 && p.refused==1 && p.p95_us==45799 && p.keys==2;
    // Request 0 settled at 45,799 us for 3,322 msat, leaving 16,678: request 2 at 100,000 is admitted.
    w.n=3;s=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    p=sf_report(s,w.req[0..<w.n]);#assert p.answered==2 && p.refused==1 && s.ledger.account.spent==6644;
}
#test "a corrupt answer is a failure: ejected, retried elsewhere, charged once" {
    w:=sfx_hand();w.n=1;w.run.policy.fails=1;w.run.faults[0]=SfSpan.{replica=0,kind=SfFault.corrupt,from=0,until=100000};
    s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    #assert sfx_att(s.att[0],0,0,SfKind.first,0,0,45799,SfResult.corrupt) && s.ejects==1 && sfx_att(s.att[1],0,1,SfKind.retry,45799,45799,91598,SfResult.ok);
    #assert s.out[0].at==91598 && s.out[0].charge==3322 && s.ledger.n==1;
}
#test "a hedge may go to replica 0" {
    w:=sfx_hand();w.n=2;w.req[1].arrive=1000;w.run.policy.hedge_us=50000;
    w.run.faults[0]=SfSpan.{replica=1,kind=SfFault.slow,from=0,until=200000,pct=300};
    s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // Request 1 goes to the idle replica 1, slow; its hedge at 51,000 us goes to replica 0, idle again, and wins at 96,799.
    #assert sfx_att(s.att[1],1,1,SfKind.first,1000,1000,138397,SfResult.cancelled) && sfx_att(s.att[2],1,0,SfKind.hedge,51000,51000,96799,SfResult.ok) && s.out[1].at==96799;
}
#test "a full record stops the run" {
    w:=sfx_hand();w.run.policy.probe_us=1;#assert sf_sim(w.run,w.req[0..<w.n]).status==SfStatus.attempts;
}
```

The second trace, `sfx_fence`, is one request on two replicas. Replica 0
runs three times slow (137,397 µs) and is cut off from 20,000 to
100,000 µs. The hedge at 30,000 µs goes to replica 1 and answers at
75,799 µs; the cancel to replica 0 is lost in the partition, so replica 0
finishes at 137,397 µs, after the partition, and its answer arrives. The
attempt is already closed, so the answer is fenced. One charge, 3,322 msat,
for the hedge's 45,799 µs; replica 0's 137,397 µs are not billed.

## Running it

The example, [`serve-fault.coax`](../examples/serve-fault.coax), prints
both traces attempt by attempt, then the fault mix under both policies and
without faults.

<!-- source: psionic/examples/serve-fault.coax main -->
```jai
main :: () {
    w:=sfx_hand();q:=w.req[0..<w.n];s:=sf_sim(w.run,q);
    print("hand trace (%): replica 0 crashed from 120,000 to 400,000 us\n",SF_PROFILE);
    list_attempts(s);list_requests(s,q);

    f:=sfx_fence();s=sf_sim(f.run,f.req[0..<f.n]);
    print("fence: replica 0 three times slow, cut off from 20,000 to 100,000 us; hedge after 30,000 us\n");
    list_attempts(s);list_requests(s,f.req[0..<f.n]);

    m:=sfx_mix(sfx_resilient());
    show("fault mix, 3 tries, hedge after 60,000 us",sf_sim(m.run,m.req[0..<m.n]),m.req[0..<m.n]);
    m=sfx_mix(sfx_baseline());
    show("fault mix, 1 try, no hedge",sf_sim(m.run,m.req[0..<m.n]),m.req[0..<m.n]);
    m=sfx_mix(sfx_resilient());m.run.nfaults=0;
    show("no faults, 3 tries, hedge after 60,000 us",sf_sim(m.run,m.req[0..<m.n]),m.req[0..<m.n]);
}
```

```shell
coax/bin/coax run psionic/examples/serve-fault.coax
```

```text
hand trace (sf-fault-v1): replica 0 crashed from 120,000 to 400,000 us
  attempt 0: request 0 first to replica 0, sent 0, ran 0 to 45799: ok at 45799
  attempt 1: request 1 first to replica 1, sent 10000, ran 10000 to 55799: ok at 55799
  attempt 2: request 2 first to replica 0, sent 100000, ran 100000 to 145799: crashed at 120000
  attempt 3: request 2 retry to replica 1, sent 120000, ran 120000 to 165799: ok at 165799
  attempt 4: request 3 first to replica 0, sent 130000: down at 130000
  attempt 5: probe to replica 0, sent 200000: down at 200000
  attempt 6: request 4 first to replica 1, sent 200000, ran 200000 to 245799: ok at 245799
  attempt 7: probe to replica 0, sent 400000, ran 400000 to 417792: ok at 417792
  attempt 8: request 5 first to replica 1, sent 400000, ran 400000 to 445799: ok at 445799
  request 0 (key 0): answered at 45799, 1 tries, quote 16032 msat, charge 3322 msat
  request 1 (key 1): answered at 55799, 1 tries, quote 16032 msat, charge 3322 msat
  request 2 (key 2): answered at 165799, 2 tries, quote 16032 msat, charge 3322 msat
  request 3 (key 3): failed at 130000, 1 tries, quote 16032 msat, charge 0 msat
  request 4 (key 4): answered at 245799, 1 tries, quote 16032 msat, charge 3322 msat
  request 5 (key 5): answered at 445799, 1 tries, quote 16032 msat, charge 3322 msat
  request 6 (key 2): duplicate at 410000, 0 tries, quote 0 msat, charge 0 msat
  ledger: 6 usage records, 16610 msat charged, 0 msat held
fence: replica 0 three times slow, cut off from 20,000 to 100,000 us; hedge after 30,000 us
  attempt 0: request 0 first to replica 0, sent 0, ran 0 to 137397: cancelled at 75799; its answer came at 137397 and was fenced
  attempt 1: request 0 hedge to replica 1, sent 30000, ran 30000 to 75799: ok at 75799
  request 0 (key 0): answered at 75799, 2 tries, quote 16032 msat, charge 3322 msat
  ledger: 1 usage records, 3322 msat charged, 0 msat held
fault mix, 3 tries, hedge after 60,000 us: 64 keys, 64 answered, 0 failed, 0 expired, 0 refused; P95 105799 us
  74 request attempts, 10 of them retries or hedges, 0 denied by the budget; 1 ejected, 1 readmitted; 0 answers fenced; 212608 msat charged
fault mix, 1 try, no hedge: 64 keys, 58 answered, 6 failed, 0 expired, 0 refused; P95 45799 us
  64 request attempts, 0 of them retries or hedges, 0 denied by the budget; 2 ejected, 2 readmitted; 0 answers fenced; 201836 msat charged
no faults, 3 tries, hedge after 60,000 us: 64 keys, 64 answered, 0 failed, 0 expired, 0 refused; P95 45799 us
  64 request attempts, 0 of them retries or hedges, 0 denied by the budget; 0 ejected, 0 readmitted; 0 answers fenced; 212608 msat charged
```

## A fault mix against a baseline

`sfx_mix` sends 64 requests 30 ms apart to four replicas, one fault at a
time: replica 0 crashes from 100 to 600 ms, replica 1 hangs from 700 to
900 ms, replica 2 runs three times slow from 1,000 to 1,400 ms, replica 3 is
cut off from 1,500 to 1,700 ms and replica 1 answers corrupt from 1,800 to
1,900 ms. The resilient policy has 3 tries, a hedge after 60,000 µs,
attempts that time out after 150,000 µs and a budget of 20 percent plus 3.
The baseline has one try and no hedge, with the same health checks.

| Policy | Answered | P95 of answered | Attempts | Charged |
| --- | --- | --- | --- | --- |
| 3 tries, hedge after 60,000 µs | 64 of 64 | 105,799 µs | 74 | 212,608 msat |
| 1 try, no hedge | 58 of 64 | 45,799 µs | 64 | 201,836 msat |

The resilient policy answers every key with 10 extra attempts, 16 percent
over the 64 first ones. Its P95 is one hedge: 60,000 µs of waiting plus
45,799 µs on a healthy replica. The baseline's P95 looks better only
because it counts the 58 it answered; it lost 6 keys outright. Its charge
is higher per answer than it looks: two of its answers came from the slow
replica, at 7,902 msat each instead of 3,322, because device time is
billed. With hedging, the healthy replica's answer won every time and every
answer cost 3,322 msat: 64 × 3,322 = 212,608.

<!-- source: psionic/lib/serve-fault.coax report -->
```jai
// Availability and latency over the distinct keys: answered, failed,
// expired and refused requests, the nearest-rank P95 of the answered ones'
// latency from arrival, and the millisatoshis charged.
SfReport :: struct {keys:int;answered:int;failed:int;expired:int;refused:int;p95_us:int;charged:int;attempts:int;}
sf_report :: (s:SfSim,reqs:[]SfRequest)->SfReport {
    p:SfReport;lat:[64]int;
    for i:0..<s.n {
        o:=s.out[i];
        if o.end!=SfEnd.duplicate {p.keys+=1;}
        if o.end==SfEnd.answered {lat[p.answered]=o.at-reqs[i].arrive;p.answered+=1;}
        if o.end==SfEnd.failed {p.failed+=1;}
        if o.end==SfEnd.expired {p.expired+=1;}
        if o.end==SfEnd.refused {p.refused+=1;}
    }
    for j:0..<s.na {if s.att[j].req>=0 {p.attempts+=1;}}
    p.p95_us=bn_stats(lat[0..<p.answered]).p[2];p.charged=s.ledger.usage.charged;
    return p;
}
```

<!-- source: psionic/lib/serve-fault.coax check-frozen -->
```jai
#test "frozen: the fault mix with retries, a hedge and health checks" {
    w:=sfx_mix(sfx_resilient());s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // Every key answered: 10 retries and hedges, none denied; P95 105,799 us, a hedge sent at 60,000 us and its 45,799.
    #assert sfx_frozen(s,w.req[0..<w.n],64,0,0,105799,212608,74) && s.extras==10 && s.denied==0 && s.ejects==1 && s.readmits==1 && s.events==129;
}
#test "frozen: the same mix with one try and no hedge" {
    w:=sfx_mix(sfx_baseline());s:=sf_sim(w.run,w.req[0..<w.n]);sfx_check(s,w.run,w.req[0..<w.n]);
    // 6 of 64 keys lost and charged nothing; the P95 of the 58 answered is one clean attempt.
    #assert sfx_frozen(s,w.req[0..<w.n],58,6,0,45799,201836,64) && s.extras==0 && s.ejects==2 && s.readmits==2 && s.events==132;
}
```

## Independent checks

`sfx_random` builds 300 random runs: 2 to 5 replicas, 8 to 40 requests
with some keys resent, up to 8 fault spans of every kind (partitions laid
over the span before them, so lost cancels and late answers happen), random
policies and accounts small enough to refuse. `sfx_check` recomputes, from
the requests, the fault spans and chapter 180's prices alone, what every
finished run must keep:

- each request is answered at most once: at most one attempt answered, and
  it is the winner; every other attempt is closed by the answer time;
- each key is charged at most once: the ledger holds each key once, with
  one usage record whose status and latency match the request; an answered
  request is charged exactly `min(price, quote)` for its winner's device
  time, anything else 0, and nothing is left held;
- nothing ends past its deadline, and no replica starts an attempt that
  cannot finish by it;
- every retry and hedge fits the budget at the moment it was sent;
- every attempt's result follows from the spans: its time from `slow`, no
  answer after a `hang` start, `down` only inside a crash, `crashed` only at
  a crash's start, no answer taken inside a partition, `corrupt` exactly
  inside a corrupt span, and a fenced answer only after a cancel lost in a
  partition.

## Refusals

A run out of bounds serves nothing: a policy, a fault span or a request
outside its limits is `config`, arrivals out of order are `order`, and a
run that needs more event times than allowed stops with `events`. A full
attempt record stops with `attempts`. The ledger refuses a key settled
before (`twice`), a quote never held (`bill`, from chapter 180) and a
record chapter 180 would not take (`meter`).

<!-- source: psionic/lib/serve-fault.coax check-refuse -->
```jai
#test "runs the simulation refuses" {
    w:=sfx_hand();q:=w.req[0..<w.n];
    #assert sf_sim(w.run,q).status==SfStatus.ok;
    for k:0..<24 {
        r:=w.run;
        if k==0 {r.replicas=0;}if k==1 {r.replicas=9;}if k==2 {r.policy.attempt_us=0;}if k==3 {r.policy.tries=0;}if k==4 {r.policy.tries=5;}
        if k==5 {r.policy.hedge_us=-1;}if k==6 {r.policy.budget_pct=-1;}if k==7 {r.policy.budget_pct=101;}if k==8 {r.policy.reserve=-1;}if k==9 {r.policy.fails=0;}
        if k==10 {r.policy.min_up=0;}if k==11 {r.policy.min_up=3;}if k==12 {r.policy.probe_us=0;}if k==13 {r.policy.oks=0;}if k==14 {r.events=0;}
        if k==15 {r.events=SF_EVENTS+1;}if k==16 {r.nfaults=9;}if k==17 {r.faults[0].replica=2;}if k==18 {r.faults[0].until=120000;}
        if k==19 {r.faults[0].from=-1;}if k==20 {r.nfaults=2;r.faults[1]=SfSpan.{kind=SfFault.slow,until=1,pct=99};}
        if k==21 {r.nfaults=2;r.faults[1]=SfSpan.{kind=SfFault.slow,until=1,pct=1001};}if k==22 {r.price.base=-1;}if k==23 {r.budget=-1;}
        #assert sf_sim(r,q).status==SfStatus.config;
    }
    r:=w.run;r.nfaults=2;r.faults[1]=SfSpan.{kind=SfFault.slow,until=1,pct=1000};r.faults[0].replica=1;r.policy.min_up=2;r.policy.tries=4;r.events=SF_EVENTS;
    #assert sf_sim(r,q).status==SfStatus.ok;
    r=w.run;r.nfaults=2;r.faults[1]=SfSpan.{kind=SfFault.none,until=1};#assert sf_sim(r,q).status==SfStatus.config;
    r=w.run;r.nfaults=2;r.faults[1]=SfSpan.{kind=SfFault.crash,from=399999,until=500000};#assert sf_sim(r,q).status==SfStatus.config;
    r.faults[1].from=400000;#assert sf_sim(r,q).status==SfStatus.ok;
    r.faults[1].from=0;r.faults[1].until=120000;#assert sf_sim(r,q).status==SfStatus.ok;
    r.faults[1].until=120001;#assert sf_sim(r,q).status==SfStatus.config;
    // Requests: a bad field, out of arrival order, too many; too few events.
    for k:0..<6 {
        x:=w.req;
        if k==0 {x[1].prompt=0;}if k==1 {x[1].output=0;}if k==2 {x[1].output=SF_OUTPUT+1;}if k==3 {x[1].deadline=0;}if k==4 {x[1].key=-1;}if k==5 {x[1].arrive=-1;}
        #assert sf_sim(w.run,x[0..<w.n]).status==SfStatus.config;
    }
    x:=w.req;x[1].prompt=SF_PROMPT;x[1].output=SF_OUTPUT;x[1].key=0;#assert sf_sim(w.run,x[0..<w.n]).status==SfStatus.ok;
    x=w.req;x[3].arrive=99999;#assert sf_sim(w.run,x[0..<w.n]).status==SfStatus.order;
    x[3].arrive=100000;#assert sf_sim(w.run,x[0..<w.n]).status==SfStatus.ok;
        r=w.run;r.events=13;#assert sf_sim(r,q).status==SfStatus.events;
    r.events=14;#assert sf_sim(r,q).status==SfStatus.ok;
    r.events=1;#assert sf_sim(r,q).status==SfStatus.events;
    x=w.req;x[0].prompt=0;#assert sf_sim(w.run,x[0..<w.n]).status==SfStatus.config;
    x=w.req;x[0].arrive=20000;#assert sf_sim(w.run,x[0..<w.n]).status==SfStatus.order;
    // Every bound at its edge is accepted.
    e:=w.run;e.replicas=1;e.nfaults=0;e.policy=SfPolicy.{attempt_us=1,tries=4,budget_pct=100,fails=1,min_up=1,probe_us=1,oks=1};
    #assert sf_sim(e,q).status==SfStatus.ok;
    e=w.run;e.replicas=8;e.nfaults=8;
    for j:0..<8 {e.faults[j]=SfSpan.{replica=j,kind=SfFault.slow,from=0,until=1,pct=ifx j==0 then 100 else 1000};}
    #assert sf_sim(e,q).status==SfStatus.ok;
    e.nfaults=9;#assert sf_sim(e,q).status==SfStatus.config;e.nfaults=-1;#assert sf_sim(e,q).status==SfStatus.config;
    many:[65]SfRequest;for i:0..<65 {many[i]=SfRequest.{key=i,prompt=1,output=1,deadline=1};}
    many[0].prompt=SF_PROMPT;many[1].output=SF_OUTPUT;
    #assert sf_sim(w.run,many[0..<64]).status==SfStatus.ok && sf_sim(w.run,many[..]).status==SfStatus.config;
    many[0].prompt=SF_PROMPT+1;#assert sf_sim(w.run,many[0..<64]).status==SfStatus.config;
}
```

[`serve-fault-refuse.coax`](../examples/serve-fault-refuse.coax) shows
each refusal; the program itself exits 0.

<!-- source: psionic/examples/serve-fault-refuse.coax main -->
```jai
main :: () {
    w:=sfx_hand();q:=w.req[0..<w.n];
    sim("the hand trace",w.run,q);
    r:=w.run;r.policy.tries=5;sim("5 tries per request",r,q);
    r=w.run;r.policy.min_up=3;sim("keep 3 of 2 replicas healthy",r,q);
    r=w.run;r.policy.budget_pct=101;sim("a retry budget of 101 percent",r,q);
    r=w.run;r.nfaults=2;r.faults[1]=SfSpan.{replica=0,kind=SfFault.crash,from=399999,until=500000};sim("a second crash of replica 0 before its restart",r,q);
    r=w.run;r.faults[0].replica=2;sim("a crash on replica 2 of 2",r,q);
    r=w.run;r.nfaults=2;r.faults[1]=SfSpan.{replica=1,kind=SfFault.slow,until=100000,pct=99};sim("a replica slow at 99 percent",r,q);
    x:=w.req;x[1].output=0;sim("a request for 0 output tokens",w.run,x[0..<w.n]);
    x=w.req;x[3].arrive=99999;sim("request 3 before request 2",w.run,x[0..<w.n]);
    r=w.run;r.events=13;sim("13 event times for a run that needs 14",r,q);

    s:=bl_test_schedule();ev:=um_test_evidence();l:=SfLedger.{account=BlAccount.{budget=100000}};
    qt:=bl_quote(s,8,2,300000);l.account=bl_admit(l.account,qt).account;
    u:=BlUse.{status=BlStatus.answered,tokens_in=8,tokens_out=2,device_us=45799};
    t:=sf_settle(l,ev,s,7,qt,u,45799);settle("settle key 7",t);
    settle("settle key 7 again",sf_settle(t.ledger,ev,s,7,qt,u,45799));
    settle("settle key 8 against a quote never held",sf_settle(t.ledger,ev,s,8,qt,u,45799));
    bad:=ev;bad.digest="0";
    settle("settle key 8 under a malformed weights digest",sf_settle(t.ledger,bad,s,8,BlPrice.{},u,45799));
}
```

```shell
coax/bin/coax run psionic/examples/serve-fault-refuse.coax
```

```text
the hand trace: ok
5 tries per request: config
keep 3 of 2 replicas healthy: config
a retry budget of 101 percent: config
a second crash of replica 0 before its restart: config
a crash on replica 2 of 2: config
a replica slow at 99 percent: config
a request for 0 output tokens: config
request 3 before request 2: order
13 event times for a run that needs 14: events
settle key 7: settled, 3322 msat; 1 keys, 3322 msat charged
settle key 7 again: refused, twice; 1 keys, 3322 msat charged
settle key 8 against a quote never held: refused, bill; 1 keys, 3322 msat charged
settle key 8 under a malformed weights digest: refused, meter; 1 keys, 3322 msat charged
```

## What remains

This chapter builds the policies and checks them in simulation. These
parts of the plan do not run yet:

- **Real failures.** Fault injection that kills a real device, process
  or machine, measured with [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s
  benchmark for error rate and recovery time.
- **Cordoning with drain.** Taking a failing machine out of rotation while
  its in-flight work drains, joining this health checker to
  [chapter 179](ch179-ship-without-downtime.md)'s draining.
- **Failover.** Active-active and active-passive failover between machines
  or regions, with the switch time measured. Regions and providers are owner
  decisions.
- **Data residency.** Routing a request only to machines in its allowed
  region, and failing closed when none is available.
- **Tool isolation.** Running tools apart from the model process.
- **Weight protection.** Verifying weight files on load and stopping
  before serving on a mismatch; chapter 178's readiness machine has the
  hook (`cs_step` with `verified`), but no real weight file is checked here.

## Run the checks

```shell
coax/bin/coax test psionic/lib/serve-fault.coax
coax/bin/coax mutate psionic/lib/serve-fault.coax --root-only
psionic/check.sh --only serve-fault
```

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
