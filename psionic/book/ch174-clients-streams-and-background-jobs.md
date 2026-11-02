# 174. Clients, streams and background jobs

> **Status: partly implemented.** The client-facing request lifecycle
> (profile sc-client-v1) runs on the CPU with clients simulated in one
> process: idempotent submission, per-client limits on unfinished jobs
> feeding [chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md)'s
> fair scheduler, a job lifecycle checked against its table, server-sent
> events that resume from a Last-Event-ID with nothing lost or repeated,
> cancellation that frees the batch slot the next step and settles once on
> [chapter 181](ch181-survive-failures.md)'s ledger, background jobs that
> are polled, and a retention window after which a result is gone. The model
> is a stand-in that emits fixed token numbers. Not built yet: the live HTTP
> server of [chapter 79](ch079-serve-responses.md) carrying these events,
> audio streams, session reuse and its benchmark, and the Responses edge;
> the last section lists them.

A served model needs clients that do more than send one request and wait
for one answer. A client may want the answer on the connection it asked
on, as a stream of events, or later, from a job it checks on. Connections
drop. Clients resend requests they are not sure arrived. Some cancel
halfway. A service has to handle each of these without losing an answer,
running the same request twice, or charging for it twice.

This chapter builds that lifecycle in an `#authority` file,
[`serve-client.coax`](../lib/serve-client.coax), because it decides who is
admitted and what is charged. It reuses three earlier chapters unchanged:

- [Chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md)'s
  `cb_step` schedules the work. Each job is one scheduler request, and
  client weights give each client its fair share.
- [Chapter 180](ch180-observe-meter-and-price.md)'s `bl_quote` and
  `bl_admit` hold a quote at submission.
- [Chapter 181](ch181-survive-failures.md)'s `sf_settle` settles each job
  once, with one usage record, when the job ends.

## Jobs and their states

A client submits a request with its own idempotency key. The request
carries a mode. In `stream` mode, events are pushed as they happen. In
`background` mode, the client polls for status or reads events when it
wants them. In `sync` mode, the answer comes back on the connection, and a
dropped connection cancels the job. `body` stands in for the request's
content as a digest. Two submissions with the same key are the same
request only if every field matches.

<!-- source: psionic/lib/serve-client.coax types -->
```jai
// stream: events are pushed as they happen; a dropped connection does not
// cancel, and the client resumes. background: the client polls, or reads
// the events whenever it likes. sync: the answer comes back on the
// connection, and a dropped connection cancels the job.
ScMode :: enum u8 {stream;background;sync;}
// none is also the answer to a transition the table refuses.
ScState :: enum u8 {none;queued;running;done;cancelled;failed;gone;}
ScEdge :: enum u8 {submit;admit;finish;cancel;deadline;expire;}
ScKind :: enum u8 {queued;started;delta;completed;cancelled;failed;}
ScEvent :: struct {kind:ScKind;token:int;}
ScWhy :: enum u8 {none;config;client;shape;conflict;full;busy;budget;unknown;over;gone;cursor;ahead;steps;}
// batch: chapter 175's scheduler, whose clients and weights are ours.
// limit: the most unfinished jobs a client may have. retain: steps a
// finished job's result is kept. budget: each client's account, in msat.
ScConfig :: struct {batch:CbConfig;limit:[4]int;retain:int;price:BlSchedule;budget:[4]int;evidence:UmEvidence;}
// key: the client's idempotency key. body: the request's content, as a
// digest; the stand-in model reads it. deadline: steps after submission.
ScRequest :: struct {client:int;key:int;mode:ScMode;body:int;prompt:int;output:int;deadline:int;}
```

Every job moves through one small state machine. A job is `queued` when it
is submitted, `running` when the scheduler admits it, and `done` when its
last token comes out. A cancel ends a queued or running job as
`cancelled`. A deadline ends it as `failed`. When the retention window
passes, a job that has ended becomes `gone`. Any other transition is
refused, which the code reports as `none`:

| State | submit | admit | finish | cancel | deadline | expire |
| --- | --- | --- | --- | --- | --- | --- |
| none | queued | — | — | — | — | — |
| queued | — | running | — | cancelled | failed | — |
| running | — | — | done | cancelled | failed | — |
| done, cancelled, failed | — | — | — | — | — | gone |
| gone | — | — | — | — | — | — |

<!-- source: psionic/lib/serve-client.coax next -->
```jai
// The lifecycle. A job is queued at submission, runs when the scheduler
// admits it, and ends done, cancelled or failed; a cancel or a deadline can
// end it queued or running. An ended job's result expires into gone.
// Anything else is refused.
sc_next :: (s:ScState,e:ScEdge)->ScState {
    live:=s==ScState.queued || s==ScState.running;
    over:=s==ScState.done || s==ScState.cancelled || s==ScState.failed;
    if s==ScState.none && e==ScEdge.submit {return ScState.queued;}
    if s==ScState.queued && e==ScEdge.admit {return ScState.running;}
    if s==ScState.running && e==ScEdge.finish {return ScState.done;}
    if live && e==ScEdge.cancel {return ScState.cancelled;}
    if live && e==ScEdge.deadline {return ScState.failed;}
    if over && e==ScEdge.expire {return ScState.gone;}
    return ScState.none;
}
sc_live :: (s:ScState)->bool {return s==ScState.queued || s==ScState.running;}
```

The checks write the same table out again as 42 numbers in
[`serve-client-checks.coax`](../tests/serve-client-checks.coax). They
compare every cell, then follow 400 random walks of 16 edges each through
both. The server also records each job's trail of states, and the
invariant checker later confirms that every step in every trail is
allowed by the table.

<!-- source: psionic/lib/serve-client.coax check-table -->
```jai
#test "every transition equals the table" {
    for s:0..<7 {for e:0..<6 {
        #assert sc_next(cast(ScState)cast(u8)s,cast(ScEdge)cast(u8)e)==scx_table(s,e);
    }}
}
#forall "a random walk of edges follows the table" seed:0..<400 #steps 4000000 {
    scx_walk(seed);
}
```

## Admission

The server opens over a scheduler configuration, per-client limits and
accounts, a price schedule and a retention window. It asks chapter 175 to
check the scheduler configuration. Given no requests, `cb_start` refuses
with `order` only after the configuration itself has passed.

<!-- source: psionic/lib/serve-client.coax open -->
```jai
// Chapter 175 checks its own configuration: given no requests it refuses
// with order only when the configuration itself passed.
sc_open :: (cfg:ScConfig)->ScServer {
    v:ScServer;v.cfg=cfg;v.why=ScWhy.config;
    none:[1]CbRequest;s:=cb_start(cfg.batch,none[0..<0]);
    if s.status!=CbStatus.order || cfg.batch.clients>SC_CLIENTS || cfg.retain<1 || !bl_schedule_valid(cfg.price) || !um_evidence_valid(cfg.evidence) {return v;}
    for c:0..<cfg.batch.clients {
        if cfg.limit[c]<1 || cfg.limit[c]>SC_JOBS || !bl_account_valid(BlAccount.{budget=cfg.budget[c]}) {return v;}
        v.ledger[c].account.budget=cfg.budget[c];
    }
    s.status=CbStatus.ok;v.s=s;v.why=ScWhy.none;
    return v;
}
```

A submission is checked in a fixed order. The client must be known, and
the request must fit: a key of 0 or more, at least one prompt token, 1 to
32 output tokens, a deadline of at least one step, and a whole state that
fits the scheduler's position budget. Next comes the key. If the same
client has used this key before, an identical request gets its first job
back and nothing new is held. A different request under the same key is
refused as `conflict`. This check comes before the limits, so a client
resending a request the server already holds is never turned away as busy.
After that come room in the job table, the client's limit on unfinished
jobs (`busy`), and an account that can hold the quote (`budget`).

The quote is chapter 180's worst case. Its device time is every step of
the deadline at the scheduler's widest step, `deadline` times
`cb_step_ns(tokens)`. An admitted job enters the scheduler's request list
with its arrival set to the next step.

<!-- source: psionic/lib/serve-client.coax submit -->
```jai
// The quote's device time: every step of the deadline at the scheduler's
// widest step, rounded up to a microsecond.
sc_quote :: (cfg:ScConfig,r:ScRequest)->BlPrice {
    us:=bl_ceil(r.deadline*cb_step_ns(cfg.batch.tokens),1000);
    return bl_quote(cfg.price,r.prompt,r.output,us);
}
sc_same :: (a:ScRequest,b:ScRequest)->bool {
    return a.mode==b.mode && a.body==b.body && a.prompt==b.prompt && a.output==b.output && a.deadline==b.deadline;
}
// Admission, in order: a known client and a request that fits; then the
// key (the same request again is its first job, a different one under the
// same key is refused); then room in the table, the client's limit on
// unfinished jobs, and an account that holds the quote.
sc_submit :: (sv:[]ScServer,r:ScRequest)->ScSubmit {
    if sv[0].why!=ScWhy.none {return ScSubmit.{why=sv[0].why,id= -1};}
    b:=sv[0].cfg.batch;
    if r.client<0 || r.client>=b.clients {return ScSubmit.{why=ScWhy.client,id= -1};}
    if r.key<0 || r.prompt<1 || r.output<1 || r.output>SC_OUTPUT || r.deadline<1 || r.prompt+r.output+b.state>b.kv {return ScSubmit.{why=ScWhy.shape,id= -1};}
    for i:0..<sv[0].n {
        q:=sv[0].jobs[i].req;
        if q.client==r.client && q.key==r.key {
            if sc_same(q,r) {return ScSubmit.{id=i};}
            return ScSubmit.{why=ScWhy.conflict,id=i};
        }
    }
    if sv[0].n==SC_JOBS {return ScSubmit.{why=ScWhy.full,id= -1};}
    if sv[0].active[r.client]>=sv[0].cfg.limit[r.client] {return ScSubmit.{why=ScWhy.busy,id= -1};}
    q:=sc_quote(sv[0].cfg,r);a:=bl_admit(sv[0].ledger[r.client].account,q);
    if a.cause!=BlCause.none {return ScSubmit.{why=ScWhy.budget,id= -1};}
    i:=sv[0].n;sv[0].n+=1;sv[0].ledger[r.client].account=a.account;sv[0].active[r.client]+=1;
    sv[0].jobs[i]=ScJob.{req=r,at=sv[0].s.step,at_ns=sv[0].s.clock,quote=q};
    sc_move(sv,i,ScEdge.submit);sc_emit(sv,i,ScKind.queued,0);
    // The scheduler hears of it at its next step.
    sv[0].s.req[i]=CbRequest.{client=r.client,arrive=sv[0].s.step,prompt=r.prompt,output=r.output};
    sv[0].s.fate[i]=CbFate.{admit= -1,finish= -1};sv[0].s.n=i+1;
    return ScSubmit.{id=i,created=true};
}
```

## One step

A server step runs in four parts. Deadlines are checked first, so a job
that is out of time never takes a slot. Then chapter 175's `cb_step` runs,
and every admission, token and finish it made becomes an event on that
job. Last, any result past its retention window expires.

Cancellation reaches into the scheduler's own state. A job withdrawn
before it arrives never arrives. A job withdrawn while queued leaves its
client's queue and is marked as admitted, so chapter 175's admission skips
it, and its client's virtual time is never charged. A running job gives up
its batch slot and its state positions, which are free for the next step's
admission.

A job is settled when it ends, and only then. A job that ran pays chapter
180's price for the device time it held, from the clock before its
admission step to the clock when it ended, capped at its quote. A done job
also pays for its tokens. A cancelled or failed job pays only for device
time, which is chapter 180's rule for cancellation. Its usage record still
counts the tokens it produced. A job that never ran pays nothing.

<!-- source: psionic/lib/serve-client.coax step -->
```jai
sc_move :: (sv:[]ScServer,i:int,e:ScEdge) {
    t:=sc_next(sv[0].jobs[i].state,e);n:=sv[0].jobs[i].nt;
    sv[0].jobs[i].state=t;sv[0].jobs[i].trail[n]=t;sv[0].jobs[i].nt=n+1;
}
sc_emit :: (sv:[]ScServer,i:int,kind:ScKind,token:int) {
    n:=sv[0].jobs[i].n;sv[0].jobs[i].ev[n]=ScEvent.{kind=kind,token=token};sv[0].jobs[i].n=n+1;
}
// The stand-in model: token k of a body.
sc_token :: (body:int,k:int)->int {return (body*31+k*7)%1000;}
// Take a job out of the scheduler: before it arrives, it never will;
// queued, it leaves its client's queue unadmitted, so its client's virtual
// time is not charged; running, its slot and its state are free for the
// next step's admission.
sc_withdraw :: (sv:[]ScServer,i:int) {
    r:=sv[0].s.req[i];step:=sv[0].s.step;
    if sv[0].s.fate[i].admit>=0 {
        m:=0;
        for k:0..<sv[0].s.nrun {if sv[0].s.run[k]!=i {sv[0].s.run[m]=sv[0].s.run[k];m+=1;}}
        sv[0].s.nrun=m;sv[0].s.kv-=r.prompt+r.output+sv[0].cfg.batch.state;
    } else {
        if r.arrive<step {sv[0].s.queued[r.client]-=1;} else {sv[0].s.req[i].arrive= -1;}
        // Marked admitted, so chapter 175's admission passes over it.
        sv[0].s.fate[i].admit=step;
    }
    sv[0].s.fate[i].finish=step;sv[0].s.done+=1;
}
// End a job: its last event, and its one settlement. A job that ran pays
// chapter 180's price for the device time it held (answered: plus its
// tokens), capped at its quote; one that never ran pays nothing.
sc_end :: (sv:[]ScServer,i:int,e:ScEdge,kind:ScKind) {
    sc_move(sv,i,e);sc_emit(sv,i,kind,0);
    c:=sv[0].jobs[i].req.client;clock:=sv[0].s.clock;
    device:=0;if sv[0].jobs[i].ran {device=(clock-sv[0].jobs[i].admit_ns)/1000;}
    u:=BlUse.{status=BlStatus.cancelled,tokens_in=sv[0].jobs[i].req.prompt,tokens_out=sv[0].jobs[i].produced,device_us=device};
    if e==ScEdge.finish {u.status=BlStatus.answered;}
    t:=sf_settle(sv[0].ledger[c],sv[0].cfg.evidence,sv[0].cfg.price,i,sv[0].jobs[i].quote,u,(clock-sv[0].jobs[i].at_ns)/1000);
    sv[0].ledger[c]=t.ledger;sv[0].jobs[i].charge=t.charge;sv[0].jobs[i].ended=sv[0].s.step;sv[0].jobs[i].end_ns=clock;sv[0].active[c]-=1;
}
// One scheduler step. Deadlines first, so a job out of time never takes a
// slot; then chapter 175's step; then every admission, token and finish it
// made becomes an event; then results past the retention window expire.
sc_tick :: (sv:[]ScServer)->ScWhy {
    if sv[0].why!=ScWhy.none {return sv[0].why;}
    now:=sv[0].s.step;
    if now==sv[0].cfg.batch.steps {sv[0].why=ScWhy.steps;return ScWhy.steps;}
    for i:0..<sv[0].n {
        if sc_live(sv[0].jobs[i].state) && now-sv[0].jobs[i].at>=sv[0].jobs[i].req.deadline {sc_withdraw(sv,i);sc_end(sv,i,ScEdge.deadline,ScKind.failed);}
    }
    before:=sv[0].s.clock;sv[0].s=cb_step(sv[0].s);
    for i:0..<sv[0].n {
        if sc_live(sv[0].jobs[i].state) {
            f:=sv[0].s.fate[i];
            if f.admit==now {sc_move(sv,i,ScEdge.admit);sc_emit(sv,i,ScKind.started,0);sv[0].jobs[i].ran=true;sv[0].jobs[i].admit_ns=before;}
            while sv[0].jobs[i].produced<sv[0].s.emitted[i] {
                k:=sv[0].jobs[i].produced;sc_emit(sv,i,ScKind.delta,sc_token(sv[0].jobs[i].req.body,k));sv[0].jobs[i].produced=k+1;
            }
            if f.finish==now {sc_end(sv,i,ScEdge.finish,ScKind.completed);}
        }
    }
    for i:0..<sv[0].n {
        t:=sv[0].jobs[i].state;
        if t!=ScState.gone && !sc_live(t) && sv[0].s.step-sv[0].jobs[i].ended>=sv[0].cfg.retain {sc_move(sv,i,ScEdge.expire);}
    }
    return ScWhy.none;
}
```

## Streams, cursors and polls

Each job keeps its events in order: `queued`, then `started` if it ran,
one `delta` for each output token, and one ending: `completed`,
`cancelled` or `failed`. The event at position *k* (counting from 1) is
always the same event. A reader therefore needs only a cursor: the last
event it has. `sc_read` returns the events after the cursor, as many as
fit. A client that reconnects sends its cursor back as `Last-Event-ID` and
gets exactly the events it missed. A cursor below 0 is refused as
`cursor`, a cursor past the last event as `ahead`, and a job whose result
has expired as `gone`.

<!-- source: psionic/lib/serve-client.coax client -->
```jai
// A client cancels a job, or its connection drops: a sync job is cancelled,
// a stream or background job keeps running.
sc_cancel :: (sv:[]ScServer,i:int)->ScWhy {
    if i<0 || i>=sv[0].n {return ScWhy.unknown;}
    if !sc_live(sv[0].jobs[i].state) {return ScWhy.over;}
    sc_withdraw(sv,i);sc_end(sv,i,ScEdge.cancel,ScKind.cancelled);
    return ScWhy.none;
}
sc_drop :: (sv:[]ScServer,i:int)->ScWhy {
    if i<0 || i>=sv[0].n {return ScWhy.unknown;}
    if sv[0].jobs[i].req.mode==ScMode.sync {return sc_cancel(sv,i);}
    return ScWhy.none;
}
// Read a job's events after the cursor (a stream's Last-Event-ID, 0 for
// the start), as many as fit in out.
sc_read :: (sv:[]ScServer,i:int,after:int,out:[]ScEvent)->ScRead {
    if i<0 || i>=sv[0].n {return ScRead.{why=ScWhy.unknown};}
    t:=sv[0].jobs[i].state;n:=sv[0].jobs[i].n;
    if t==ScState.gone {return ScRead.{why=ScWhy.gone};}
    if after<0 {return ScRead.{why=ScWhy.cursor};}
    if after>n {return ScRead.{why=ScWhy.ahead};}
    count:=cb_min(n-after,out.count);
    for k:0..<count {out[k]=sv[0].jobs[i].ev[after+k];}
    last:=after+count;
    return ScRead.{count=count,last=last,end=last==n && !sc_live(t)};
}
sc_poll :: (sv:[]ScServer,i:int)->ScPoll {
    if i<0 || i>=sv[0].n {return ScPoll.{why=ScWhy.unknown};}
    j:=sv[0].jobs[i];
    return ScPoll.{state=j.state,produced=j.produced,charge=j.charge};
}
```

On the wire, an event is a server-sent event whose `id` is its position.
That `id` is what a browser's `EventSource` sends back as `Last-Event-ID`.
Event names follow chapter 79's Responses streams. A token is shown by its
number, because the stand-in model has no vocabulary.

<!-- source: psionic/lib/serve-client.coax sse -->
```jai
// One event as a server-sent event: its sequence number is its id, the
// cursor a reconnecting client sends back as Last-Event-ID. -1 when out
// is too short.
sc_sse :: (id:int,e:ScEvent,out:[]u8,at:int)->int {
    names:=[6]string.["response.queued","response.in_progress","response.output_text.delta","response.completed","response.cancelled","response.failed"];
    name:=names[cast(int)e.kind];
    n:=um_int(out,um_put(out,at,"id: "),id);n=um_put(out,um_put(out,um_put(out,n,"\nevent: "),name),"\ndata: {\"type\":\"");
    n=um_int(out,um_put(out,um_put(out,n,name),"\",\"sequence_number\":"),id);
    if e.kind==ScKind.delta {n=um_int(out,um_put(out,n,",\"token\":"),e.token);}
    return um_put(out,n,"}\n\n");
}
```

## The hand trace

Two clients of weight 1 share one batch slot with a step budget of 8
tokens and 64 positions. Client 0 may have 2 unfinished jobs, and client 1
may have 1. Results are kept for 3 steps. Prices are chapter 180's test
schedule: 1,000 msat base, 2,000 msat per thousand input tokens, 8,000
msat per thousand output tokens, and 50,000 msat per device second. The
stand-in model's token *k* for a body is `(body × 31 + 7k) mod 1000`.

**Admission.** Client 0 submits A (key 1, stream, body 7, 4 prompt tokens,
3 output tokens, a 20-step deadline) and B (key 2, background, body 9, 2
and 2). The widest step, 8 rows, takes 17,792,472 + 7 × 1,459,182 =
28,006,746 ns. A's quote therefore covers 20 × 28,006,746 ns, which is
560,135 µs rounded up:

1,000 + ⌈(4 × 2,000 × 1,000 + 3 × 8,000 × 1,000 + 560,135 × 50,000) / 10⁶⌉
= 1,000 + ⌈28,038.75⌉ = 29,039 msat.

B's quote works out to 29,027 msat in the same way. Key 3 would be client
0's third unfinished job, so it is refused as `busy`. Resending key 2
returns job 1. Key 2 with body 10 is a `conflict`. Client 1 submits C
(key 1, sync, body 5, 3 and 4), quoted at 29,045 msat.

**Steps 0 to 2: A.** All three jobs arrive at step 0 with virtual time 0.
On the tie, the lower client number goes first, so A takes the slot and
client 0's virtual time grows by 7 positions × 840 / 1 = 5,880. A's
4-token prompt runs in one step of 4 rows: 17,792,472 + 3 × 1,459,182 =
22,170,018 ns. That step yields token 7 × 31 = 217. Steps 1 and 2 are one
row each, 17,792,472 ns, and yield tokens 224 and 231. The clock reads
57,754,962 ns, and A is done, having ended after 3 steps. Its charge, for
57,754 µs of device time:

1,000 + ⌈(8,000,000 + 24,000,000 + 57,754 × 50,000) / 10⁶⌉ = 1,000 +
⌈2,919.7⌉ = 3,920 msat.

**A's stream drops.** After step 0 the stream has events 1 to 3. The
connection drops with only events 1 and 2 received. On reconnecting with
`Last-Event-ID: 2`, the client gets events 3 to 6: the three tokens and
`completed`. Event 3 is sent again because the client never got it.
Nothing it did get is repeated.

**Steps 3 and 4: C.** B (client 0, virtual time 5,880) and C (client 1,
virtual time 0) are both waiting, so C goes next. Its 3-row prefill takes
20,710,836 ns, and one decode step follows. The clock reads 96,258,270 ns
with 2 tokens out.

**C's connection drops.** C is a sync job, so the drop cancels it. It
leaves the batch at once. It held the slot from 57,754,962 ns to
96,258,270 ns, which is 38,503 µs, and it is charged ⌈38,503 × 50,000 /
10⁶⌉ = ⌈1,925.15⌉ = 1,926 msat. Its usage record shows 2 output tokens. A
second cancel is refused as `over`, and the charge does not change.

**Steps 5 and 6: B.** The slot C freed goes to B at the very next step,
step 5. B's 2-row prefill takes 19,251,654 ns, and the clock reads
133,302,396 ns after step 6. B produced tokens 9 × 31 = 279 and 286, and
it held the slot for 37,044 µs:

1,000 + ⌈(4,000,000 + 16,000,000 + 37,044 × 50,000) / 10⁶⌉ = 1,000 +
⌈1,872.2⌉ = 2,873 msat.

A poll finds B done. Resending key 2 still returns job 1 and charges
nothing more.

**Retention.** A ended after 3 steps, so it is gone after step 5. C ended
after 5 steps and is gone after step 7. B ended after 7 steps and is gone
after step 9. Client 0 paid 3,920 + 2,873 = 6,793 msat. Client 1 paid
1,926 msat. Nothing is left held.

<!-- source: psionic/lib/serve-client.coax check-hand -->
```jai
#test "the hand trace: admission, a resumed stream, a cancel, a background job, retention" {
    v:=[1]ScServer.[sc_open(scx_config())];
    a:=sc_submit(v[..],scx_a());b:=sc_submit(v[..],scx_b());
    #assert a.why==ScWhy.none && a.id==0 && a.created && b.why==ScWhy.none && b.id==1 && b.created;
    // A third unfinished job for client 0 is past its limit of 2; key 2 again is job 1, created before.
    x:=scx_b();x.key=3;r:=sc_submit(v[..],x);#assert r.why==ScWhy.busy && r.id== -1 && !r.created;
    r=sc_submit(v[..],scx_b());#assert r.why==ScWhy.none && r.id==1 && !r.created;
    x=scx_b();x.body=10;r=sc_submit(v[..],x);#assert r.why==ScWhy.conflict && r.id==1 && !r.created;
    c:=sc_submit(v[..],scx_c());#assert c.id==2 && c.created;
    // Quotes: 20 steps of the 8-row step, 560,135 us; 29,039 msat for A, held.
    #assert v[0].jobs[0].quote.msat==29039 && v[0].ledger[0].account.held==29039+v[0].jobs[1].quote.msat;
    // Step 0: A takes the one slot and its 4-token prompt yields token 217.
    #assert sc_tick(v[..])==ScWhy.none && v[0].jobs[0].state==ScState.running && v[0].jobs[2].state==ScState.queued;
    ev:[35]ScEvent;g:=sc_read(v[..],0,0,ev[..]);
    #assert g.why==ScWhy.none && g.count==3 && g.last==3 && !g.end && ev[0].kind==ScKind.queued && ev[1].kind==ScKind.started && ev[2].kind==ScKind.delta && ev[2].token==217;
    // Steps 1 and 2: tokens 224 and 231; A is done at 57,754,962 ns.
    scx_tick(v[..]);scx_tick(v[..]);
    // The stream had acknowledged event 2: resuming gives 3 to 6, the ending last.
    g=sc_read(v[..],0,2,ev[..]);
    #assert g.count==4 && g.last==6 && g.end && ev[0].token==217 && ev[1].token==224 && ev[2].token==231 && ev[3].kind==ScKind.completed;
    o:=sc_poll(v[..],0);
    #assert o.state==ScState.done && o.produced==3 && o.charge==3920 && v[0].jobs[0].ended==3 && v[0].s.clock==57754962;
    // Steps 3 and 4: C (client 1, virtual time 0) beats B (virtual time 5,880); two tokens.
    scx_tick(v[..]);scx_tick(v[..]);
    #assert v[0].jobs[2].state==ScState.running && v[0].jobs[1].state==ScState.queued && v[0].jobs[2].produced==2 && v[0].s.clock==96258270;
    // The sync client drops: C is cancelled with 2 tokens produced, charged 38,503 us of device time.
    #assert sc_drop(v[..],2)==ScWhy.none;o=sc_poll(v[..],2);
    #assert o.state==ScState.cancelled && o.produced==2 && o.charge==1926 && v[0].jobs[2].ended==5;
    #assert v[0].s.nrun==0 && v[0].s.kv==0 && v[0].s.done==2 && v[0].s.fate[2].finish==5 && v[0].active[1]==0 && v[0].ledger[1].account.held==0;
    #assert sc_cancel(v[..],2)==ScWhy.over && sc_drop(v[..],2)==ScWhy.over && v[0].ledger[1].account.spent==1926 && v[0].ledger[1].n==1;
    // Its usage record: 2 tokens out, 38,503 us of device time, 96,258 us since it was submitted.
    u:=v[0].ledger[1].usage;#assert u.tokens_out==2 && u.device_us==38503 && u.latency_most==96258 && u.status[4]==1;
    // Step 5: the freed slot goes to B at once; step 6 finishes it. A is gone after step 5.
    scx_tick(v[..]);#assert v[0].jobs[1].state==ScState.running && v[0].s.fate[1].admit==5 && v[0].jobs[0].state==ScState.gone;
    #assert sc_read(v[..],0,0,ev[..]).why==ScWhy.gone && sc_poll(v[..],0).state==ScState.gone;
    scx_tick(v[..]);o=sc_poll(v[..],1);
    #assert o.state==ScState.done && o.produced==2 && o.charge==2873 && v[0].jobs[1].ended==7 && v[0].s.clock==133302396;
    // Resubmitting a finished job's key returns it and charges nothing more.
    r=sc_submit(v[..],scx_b());#assert r.id==1 && !r.created && v[0].ledger[0].account.spent==3920+2873 && v[0].ledger[0].n==2;
    u=v[0].ledger[0].usage;#assert u.tokens_out==5 && u.device_us==57754+37044 && u.latency_most==133302 && u.status[1]==2;
    // C is gone after step 7, B after step 9.
    scx_tick(v[..]);#assert v[0].jobs[2].state==ScState.gone && v[0].jobs[1].state==ScState.done;
    scx_tick(v[..]);#assert v[0].jobs[1].state==ScState.done;
    scx_tick(v[..]);#assert v[0].jobs[1].state==ScState.gone && sc_read(v[..],1,4,ev[..]).why==ScWhy.gone;
    scx_check(v[..]);
    // Every job's states, in order.
    #assert scx_trail(v[0].jobs[0],"qrdg") && scx_trail(v[0].jobs[1],"qrdg") && scx_trail(v[0].jobs[2],"qrcg");
}
```

The withdrawal paths and deadlines each have their own hand-checked
cases. One job is cancelled before it arrives, one while it waits, and
one after one step, which costs 22,170 µs, or ⌈1,108.5⌉ = 1,109 msat. A
deadline fails a waiting job before it can take a slot. Another deadline
fails a running job after its second step, at 39,962 µs, or ⌈1,998.1⌉ =
1,999 msat.

<!-- source: psionic/lib/serve-client.coax check-cancel -->
```jai
#test "cancel before arrival, while queued, while running: the slot is free the next step" {
    v:=[1]ScServer.[sc_open(scx_config())];
    scx_add(v[..],scx_a());scx_add(v[..],scx_b());scx_add(v[..],scx_c());
    // B is cancelled before the scheduler has seen it: it never arrives, and costs nothing.
    #assert sc_cancel(v[..],1)==ScWhy.none && v[0].s.req[1].arrive== -1 && v[0].s.fate[1].admit==0 && v[0].s.fate[1].finish==0 && v[0].s.done==1;
    #assert sc_poll(v[..],1).charge==0 && v[0].active[0]==1 && v[0].ledger[0].n==1;
    scx_tick(v[..]);
    // C waits behind A: cancelled while queued, it leaves the queue unadmitted and is charged nothing.
    #assert sc_cancel(v[..],2)==ScWhy.none && v[0].s.queued[1]==0 && v[0].s.fate[2].admit==1 && v[0].s.fate[2].finish==1 && v[0].s.done==2;
    #assert sc_poll(v[..],2).charge==0 && sc_poll(v[..],2).produced==0 && v[0].jobs[2].n==2;
    // A is cancelled after one step: 22,170 us of device time, 1 token, 1,109 msat.
    #assert sc_cancel(v[..],0)==ScWhy.none && v[0].s.nrun==0 && v[0].s.kv==0 && v[0].s.done==3;
    o:=sc_poll(v[..],0);#assert o.produced==1 && o.charge==1109;
    // A new job takes the free slot at the very next step.
    x:=scx_a();x.key=9;scx_add(v[..],x);scx_tick(v[..]);#assert v[0].s.fate[3].admit==1 && v[0].jobs[3].state==ScState.running;
    #assert sc_cancel(v[..],4)==ScWhy.unknown && sc_cancel(v[..],-1)==ScWhy.unknown && sc_drop(v[..],4)==ScWhy.unknown && sc_drop(v[..],-1)==ScWhy.unknown;
    // Dropping a stream or a background job's connection cancels nothing.
    #assert sc_drop(v[..],3)==ScWhy.none && v[0].jobs[3].state==ScState.running;
    x.key=10;x.mode=ScMode.background;scx_add(v[..],x);#assert sc_drop(v[..],4)==ScWhy.none && v[0].jobs[4].state==ScState.queued;
    scx_check(v[..]);
    #assert scx_trail(v[0].jobs[0],"qrc") && scx_trail(v[0].jobs[1],"qc") && scx_trail(v[0].jobs[2],"qc");
    // Job 3 was submitted at 22,170,018 ns and is cancelled at 44,340,036: 22,170 us in its usage record, as A's.
    #assert sc_cancel(v[..],3)==ScWhy.none && v[0].ledger[0].usage.latency_most==22170 && v[0].ledger[0].n==3;
}
#test "a deadline fails a waiting job before it takes a slot, and a running one" {
    v:=[1]ScServer.[sc_open(scx_config())];
    x:=scx_a();x.deadline=4;scx_add(v[..],x);
    y:=scx_c();y.deadline=2;scx_add(v[..],y);
    // A runs steps 0 to 2 and is done; C waits; at step 2 its 2 steps are up.
    scx_tick(v[..]);scx_tick(v[..]);#assert v[0].jobs[1].state==ScState.queued;
    scx_tick(v[..]);
    #assert v[0].jobs[1].state==ScState.failed && v[0].jobs[1].ended==2 && v[0].jobs[1].charge==0 && v[0].jobs[0].state==ScState.done;
    #assert v[0].jobs[1].ev[1].kind==ScKind.failed && v[0].s.queued[1]==0;
    // A running job: 2 steps, then failed with 2 tokens, before its third step runs.
    w:=[1]ScServer.[sc_open(scx_config())];x.deadline=2;scx_add(w[..],x);
    scx_tick(w[..]);scx_tick(w[..]);#assert w[0].jobs[0].state==ScState.running;
    scx_tick(w[..]);o:=sc_poll(w[..],0);
    #assert o.state==ScState.failed && o.produced==2 && w[0].jobs[0].ended==2 && w[0].s.clock==39962490 && o.charge==1999 && w[0].s.nrun==0;
    // 39,962 us at 50,000 msat a second: 1,998.1, rounded up.
    scx_check(v[..]);scx_check(w[..]);
    #assert scx_trail(v[0].jobs[1],"qf") && scx_trail(w[0].jobs[0],"qrf");
}
```

## Running it

The example, [`serve-client.coax`](../examples/serve-client.coax), runs the
hand trace and prints each submission, each step, the server-sent events
and the settlements.

<!-- source: psionic/examples/serve-client.coax main -->
```jai
main :: () {
    v:=[1]ScServer.[sc_open(scx_config())];
    print("%: one slot, 8 tokens a step, 64 positions; client 0 may hold 2 unfinished jobs, client 1 one\n",SC_PROFILE);
    submit(v[..],"A",scx_a());submit(v[..],"B",scx_b());
    x:=scx_b();x.key=3;submit(v[..],"a third job",x);
    submit(v[..],"B resent",scx_b());
    x=scx_b();x.body=10;submit(v[..],"key 2 with another body",x);
    submit(v[..],"C",scx_c());

    advance(v[..]);
    print("A's stream, connected:\n");stream(v[..],0,0);
    print("  (the connection drops; the client got events 1 and 2)\n");
    advance(v[..]);advance(v[..]);
    print("A's stream, reconnected with Last-Event-ID: 2\n");stream(v[..],0,2);poll(v[..],0);

    advance(v[..]);advance(v[..]);
    print("C's connection drops after % tokens\n",v[0].jobs[2].produced);
    w:=sc_drop(v[..],2);print("  drop: %\n",sc_why_name(w));poll(v[..],2);
    print("  cancel C again: %\n",sc_why_name(sc_cancel(v[..],2)));

    advance(v[..]);advance(v[..]);
    print("B, polled in the background:\n");poll(v[..],1);stream(v[..],1,0);
    submit(v[..],"B resent after it finished",scx_b());
    advance(v[..]);advance(v[..]);advance(v[..]);
    print("B after its 3 steps of retention:\n");poll(v[..],1);stream(v[..],1,0);

    for c:0..<2 {
        l:=v[0].ledger[c];
        print("client %: % jobs settled, % msat charged, % held; % tokens out, % us of device time\n",c,l.n,l.account.spent,l.account.held,l.usage.tokens_out,l.usage.device_us);
    }
}
```

```shell
coax/bin/coax run psionic/examples/serve-client.coax
```

```text
sc-client-v1: one slot, 8 tokens a step, 64 positions; client 0 may hold 2 unfinished jobs, client 1 one
A (client 0, key 1, stream): job 0, quote 29039 msat held
B (client 0, key 2, background): job 1, quote 29027 msat held
a third job (client 0, key 3, background): refused, busy
B resent (client 0, key 2, background): job 1 again, nothing new held
key 2 with another body (client 0, key 2, background): refused, conflict with job 1
C (client 1, key 1, sync): job 2, quote 29045 msat held
step 0: clock 22170018 ns; 0=running 1=queued 2=queued
A's stream, connected:
id: 1
event: response.queued
data: {"type":"response.queued","sequence_number":1}

id: 2
event: response.in_progress
data: {"type":"response.in_progress","sequence_number":2}

id: 3
event: response.output_text.delta
data: {"type":"response.output_text.delta","sequence_number":3,"token":217}

  (the connection drops; the client got events 1 and 2)
step 1: clock 39962490 ns; 0=running 1=queued 2=queued
step 2: clock 57754962 ns; 0=done 1=queued 2=queued
A's stream, reconnected with Last-Event-ID: 2
id: 3
event: response.output_text.delta
data: {"type":"response.output_text.delta","sequence_number":3,"token":217}

id: 4
event: response.output_text.delta
data: {"type":"response.output_text.delta","sequence_number":4,"token":224}

id: 5
event: response.output_text.delta
data: {"type":"response.output_text.delta","sequence_number":5,"token":231}

id: 6
event: response.completed
data: {"type":"response.completed","sequence_number":6}

  poll job 0: done, 3 tokens, charged 3920 msat
step 3: clock 78465798 ns; 0=done 1=queued 2=running
step 4: clock 96258270 ns; 0=done 1=queued 2=running
C's connection drops after 2 tokens
  drop: none
  poll job 2: cancelled, 2 tokens, charged 1926 msat
  cancel C again: over
step 5: clock 115509924 ns; 0=gone 1=running 2=cancelled
step 6: clock 133302396 ns; 0=gone 1=done 2=cancelled
B, polled in the background:
  poll job 1: done, 2 tokens, charged 2873 msat
id: 1
event: response.queued
data: {"type":"response.queued","sequence_number":1}

id: 2
event: response.in_progress
data: {"type":"response.in_progress","sequence_number":2}

id: 3
event: response.output_text.delta
data: {"type":"response.output_text.delta","sequence_number":3,"token":279}

id: 4
event: response.output_text.delta
data: {"type":"response.output_text.delta","sequence_number":4,"token":286}

id: 5
event: response.completed
data: {"type":"response.completed","sequence_number":5}

B resent after it finished (client 0, key 2, background): job 1 again, nothing new held
step 7: clock 133302396 ns; 0=gone 1=done 2=gone
step 8: clock 133302396 ns; 0=gone 1=done 2=gone
step 9: clock 133302396 ns; 0=gone 1=gone 2=gone
B after its 3 steps of retention:
  poll job 1: gone, 2 tokens, charged 2873 msat
  read job 1 after 0: refused, gone
client 0: 2 jobs settled, 6793 msat charged, 0 held; 5 tokens out, 94798 us of device time
client 1: 1 jobs settled, 1926 msat charged, 0 held; 2 tokens out, 38503 us of device time
```

## Independent checks

Each check below recomputes its expected result in a different way from the
code it checks:

- **The lifecycle table.** The table above is written out a second time as
  numbers, compared cell by cell, and walked at random (`scx_walk`).
- **Every invariant after every call.** `scx_check` recomputes these from
  the jobs alone: every trail follows the table; each job's events are
  `queued`, `started` if it ran, one delta per token with the stand-in's
  token (from its formula, written again), then the right ending; a done
  job produced all its tokens; a failed job ended exactly its deadline
  after submission; each ended job is charged chapter 180's exact
  `bl_oracle` price for its device time and tokens, capped at its quote;
  each client's ledger holds each ended job once and no others, with spent
  and held equal to the charges and the open quotes; unfinished jobs stay
  within each client's limit; and the scheduler's running list, state
  positions and queues match the jobs.
- **Stream resume equality.** `scx_resume` runs two identical servers with
  1 to 4 jobs in 100 random cases. One reader reads after every step
  without a break. The other drops at random, keeps a random part of what
  was in flight, and reconnects some steps later from its last event. Each
  read starts exactly after the cursor, and the events the interrupted
  reader collects equal the uninterrupted stream event for event. Some
  watched jobs are cancelled partway through.
- **Random clients.** `scx_random` builds 60 random servers (1 to 4
  clients, 1 to 4 slots, random weights, limits, retention and accounts
  small enough to refuse) and makes 80 random calls on each. The calls are
  fresh submissions, exact resubmissions, a key reused with another body,
  cancels, dropped connections, reads with random cursors, and steps. Each
  submission's result is predicted beforehand from the jobs alone
  (`scx_want`). A resubmission must leave every ledger unchanged. After
  the run, each finished job's events must equal those of the same request
  run alone, synchronously, on a fresh server.

<!-- source: psionic/lib/serve-client.coax check-random -->
```jai
#forall "resuming after any disconnect gives exactly the uninterrupted stream's suffix" seed:0..<100 #steps 400000000 {
    scx_resume(seed);
}
#forall "random clients: the table, the limits, one settlement per job, the stand-in's answer" seed:0..<60 #steps 800000000 {
    scx_random(seed);
}
```

## Refusals

A configuration out of bounds is `config`, and a server opened with one
refuses every call. A submission can be refused as `client`, `shape`,
`conflict`, `full`, `busy` or `budget`; a refused submission creates no job
and holds nothing. A read can be refused as `unknown`, `gone`, `cursor` or
`ahead`. A cancel can be refused as `unknown` or `over`. A run stops with
`steps` at its step limit.

<!-- source: psionic/lib/serve-client.coax check-refuse -->
```jai
#test "submissions the server refuses" {
    v:=[1]ScServer.[sc_open(scx_config())];
    for k:0..<9 {
        x:=scx_a();
        if k==0 {x.client= -1;}if k==1 {x.client=2;}if k==2 {x.key= -1;}if k==3 {x.prompt=0;}if k==4 {x.output=0;}
        if k==5 {x.output=SC_OUTPUT+1;}if k==6 {x.deadline=0;}if k==7 {x.prompt=62;}if k==8 {x.deadline=4000000000;}
        r:=sc_submit(v[..],x);
        want:=ifx k<2 then ScWhy.client else ifx k<8 then ScWhy.shape else ScWhy.budget;
        #assert r.why==want && r.id== -1 && !r.created && v[0].n==0;
    }
    // At every bound: 61 + 3 fills the 64-position state budget; 32 tokens.
    x:=scx_a();x.prompt=61;#assert sc_submit(v[..],x).why==ScWhy.none;
    x=scx_c();x.output=SC_OUTPUT;x.prompt=32;x.key=0;#assert sc_submit(v[..],x).why==ScWhy.none;
    // A key is per client: client 1 may use client 0's key 1.
    x.key=1;#assert sc_submit(v[..],x).why==ScWhy.busy;
    // Every field of a resubmission must match.
    for k:0..<5 {
        y:=scx_a();y.prompt=61;
        if k==0 {y.mode=ScMode.sync;}if k==1 {y.body=8;}if k==2 {y.prompt=60;}if k==3 {y.output=2;}if k==4 {y.deadline=21;}
        r:=sc_submit(v[..],y);#assert r.why==ScWhy.conflict && r.id==0;
    }
    // State that is not KV counts: 1 position more leaves room for a 60-token prompt, not 61.
    c:=scx_config();c.batch.state=1;w:=[1]ScServer.[sc_open(c)];
    x=scx_a();x.prompt=61;#assert sc_submit(w[..],x).why==ScWhy.shape;x.prompt=60;#assert sc_submit(w[..],x).why==ScWhy.none;
    // An account that cannot hold the quote.
    c=scx_config();c.budget[0]=29038;w[0]=sc_open(c);
    #assert sc_submit(w[..],scx_a()).why==ScWhy.budget && w[0].ledger[0].account.held==0 && w[0].active[0]==0;
    w[0].ledger[0].account.budget=29039;#assert sc_submit(w[..],scx_a()).why==ScWhy.none;
    // 64 jobs fill the table.
    c=scx_config();c.limit[0]=SC_JOBS;c.batch.kv=1024;c.budget[0]=100000000;w[0]=sc_open(c);
    for i:0..<64 {x=scx_a();x.key=i;#assert sc_submit(w[..],x).id==i;}
    x.key=64;r:=sc_submit(w[..],x);#assert r.why==ScWhy.full && r.id== -1 && w[0].n==64;
}
#test "reads the server refuses" {
    v:=[1]ScServer.[sc_open(scx_config())];scx_add(v[..],scx_a());scx_tick(v[..]);
    ev:[35]ScEvent;
    #assert sc_read(v[..],1,0,ev[..]).why==ScWhy.unknown && sc_read(v[..],-1,0,ev[..]).why==ScWhy.unknown && sc_poll(v[..],1).why==ScWhy.unknown && sc_poll(v[..],-1).why==ScWhy.unknown;
    #assert sc_read(v[..],0,-1,ev[..]).why==ScWhy.cursor && sc_read(v[..],0,4,ev[..]).why==ScWhy.ahead;
    // The cursor at the end: nothing new, not ended.
    g:=sc_read(v[..],0,3,ev[..]);#assert g.why==ScWhy.none && g.count==0 && g.last==3 && !g.end;
    // A short buffer takes what fits; the rest comes on the next read.
    g=sc_read(v[..],0,0,ev[0..<2]);#assert g.count==2 && g.last==2 && ev[1].kind==ScKind.started;
    scx_tick(v[..]);scx_tick(v[..]);
    g=sc_read(v[..],0,4,ev[0..<1]);#assert g.count==1 && g.last==5 && !g.end && ev[0].token==231;
    g=sc_read(v[..],0,5,ev[0..<1]);#assert g.count==1 && g.last==6 && g.end && sc_poll(v[..],0).why==ScWhy.none;
}
#test "configurations the server refuses" {
    #assert sc_open(scx_config()).why==ScWhy.none;
    for k:0..<11 {
        c:=scx_config();
        if k==0 {c.batch.slots=0;}if k==1 {c.batch.clients=5;c.limit=[4]int.[1,1,1,1];}if k==2 {c.retain=0;}if k==3 {c.price.base= -1;}
        if k==4 {c.evidence.backend="";}if k==5 {c.limit[0]=0;}if k==6 {c.limit[1]=SC_JOBS+1;}if k==7 {c.budget[1]= -1;}
        if k==8 {c.batch.clients=0;}if k==9 {c.batch.steps=0;}if k==10 {c.batch.weight[1]=0;}
        v:=[1]ScServer.[sc_open(c)];
        r:=sc_submit(v[..],scx_a());#assert v[0].why==ScWhy.config && r.why==ScWhy.config && r.id== -1 && sc_tick(v[..])==ScWhy.config;
    }
    c:=scx_config();c.batch.clients=4;c.limit=[4]int.[1,SC_JOBS,1,1];c.retain=1;c.budget[3]=7;c.batch.weight=[8]int.[1,1,1,1,0,0,0,0];
    v:=[1]ScServer.[sc_open(c)];
    #assert v[0].why==ScWhy.none && v[0].s.status==CbStatus.ok && v[0].ledger[1].account.budget==1000000 && v[0].ledger[3].account.budget==7;
    // A run stops at its step limit.
    c=scx_config();c.batch.steps=2;v[0]=sc_open(c);scx_add(v[..],scx_a());
    #assert sc_tick(v[..])==ScWhy.none && sc_tick(v[..])==ScWhy.none && sc_tick(v[..])==ScWhy.steps && v[0].why==ScWhy.steps && v[0].s.step==2;
}
```

[`serve-client-refuse.coax`](../examples/serve-client-refuse.coax) shows
each refusal. The program itself exits 0.

<!-- source: psionic/examples/serve-client-refuse.coax main -->
```jai
main :: () {
    v:=[1]ScServer.[sc_open(scx_config())];
    x:=scx_a();x.client=2;submit(v[..],"client 2 of 2",x);
    x=scx_a();x.output=0;submit(v[..],"0 output tokens",x);
    x=scx_a();x.output=SC_OUTPUT+1;submit(v[..],"33 output tokens",x);
    x=scx_a();x.prompt=62;submit(v[..],"62 + 3 positions of 64",x);
    x=scx_a();x.deadline=4000000000;submit(v[..],"a deadline past a day of device time",x);
    submit(v[..],"A",scx_a());
    x=scx_a();x.output=2;submit(v[..],"key 1 with 2 output tokens",x);
    submit(v[..],"B",scx_b());
    x=scx_b();x.key=3;submit(v[..],"a third unfinished job",x);
    c:=scx_config();c.budget[0]=29038;w:=[1]ScServer.[sc_open(c)];
    submit(w[..],"A against 29,038 msat",scx_a());

    scx_tick(v[..]);
    read(v[..],"job 2",2,0);
    read(v[..],"job 0 from -1",0,-1);
    read(v[..],"job 0 after event 4 of 3",0,4);
    print("cancel job 7: %\n",sc_why_name(sc_cancel(v[..],7)));
    for k:0..<3 {scx_tick(v[..]);}
    print("cancel job 0, done: %\n",sc_why_name(sc_cancel(v[..],0)));
    for k:0..<3 {scx_tick(v[..]);}
    read(v[..],"job 0, past its retention",0,0);

    c=scx_config();c.retain=0;print("open with 0 steps of retention: %\n",sc_why_name(sc_open(c).why));
    c=scx_config();c.limit[1]=0;print("open with a limit of 0 jobs: %\n",sc_why_name(sc_open(c).why));
    c=scx_config();c.batch.clients=5;print("open with 5 clients: %\n",sc_why_name(sc_open(c).why));
    c=scx_config();c.batch.steps=1;w[0]=sc_open(c);scx_tick(w[..]);
    print("a second step of a 1-step run: %\n",sc_why_name(sc_tick(w[..])));
}
```

```shell
coax/bin/coax run psionic/examples/serve-client-refuse.coax
```

```text
submit client 2 of 2: client; 0 jobs, 0 msat held
submit 0 output tokens: shape; 0 jobs, 0 msat held
submit 33 output tokens: shape; 0 jobs, 0 msat held
submit 62 + 3 positions of 64: shape; 0 jobs, 0 msat held
submit a deadline past a day of device time: budget; 0 jobs, 0 msat held
submit A: none, job 0; 1 jobs, 29039 msat held
submit key 1 with 2 output tokens: conflict, job 0; 1 jobs, 29039 msat held
submit B: none, job 1; 2 jobs, 58066 msat held
submit a third unfinished job: busy; 2 jobs, 58066 msat held
submit A against 29,038 msat: budget; 0 jobs, 0 msat held
read job 2: unknown, 0 events
read job 0 from -1: cursor, 0 events
read job 0 after event 4 of 3: ahead, 0 events
cancel job 7: unknown
cancel job 0, done: over
read job 0, past its retention: gone, 0 events
open with 0 steps of retention: config
open with a limit of 0 jobs: config
open with 5 clients: config
a second step of a 1-step run: steps
```

## What remains

This chapter builds the lifecycle and checks it with clients simulated in
one process. These parts of the plan do not run yet:

- **The live server.** Carrying these jobs and events over
  [chapter 79](ch079-serve-responses.md)'s HTTP listener, with real
  `Last-Event-ID` headers, a dropped socket as `sc_drop`, and polling
  routes. The events here are already server-sent-event text.
- **A real model.** The stand-in emits fixed token numbers. Chapter 173's
  engine and a qualified model would replace it.
- **Audio streams.** Audio in or out on the same connection, as in
  [chapter 154](ch154-transcribe-while-someone-speaks.md) and
  [chapter 155](ch155-speak-the-answer.md).
- **Session reuse.** One authenticated session across requests, with its
  overhead measured in
  [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s benchmark.
- **The Responses edge.** Translating Responses-shaped requests into System
  One requests, with a named error for each field that has no System One
  meaning.
- **Delivery.** Pushing a finished background job's result to the client
  instead of waiting for a poll.

## Run the checks

```shell
coax/bin/coax test psionic/lib/serve-client.coax
coax/bin/coax mutate psionic/lib/serve-client.coax --root-only
psionic/check.sh --only serve-client
```

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [Server-sent events, WHATWG HTML Living Standard](https://html.spec.whatwg.org/multipage/server-sent-events.html)
