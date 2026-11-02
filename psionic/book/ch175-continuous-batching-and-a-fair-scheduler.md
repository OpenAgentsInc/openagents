# 175. Continuous batching and a fair scheduler

> **Status: partly implemented.** The iteration-level scheduler (profile
> cb-iteration-v1) runs on the CPU: admission per step under a state budget,
> chunked prefill after decode, and fair shares between clients by virtual
> time, with a brute-force simulator, starvation and share bounds, and
> token-for-token equality with solo runs on a stand-in model as its checks.
> Its steps also run on the original gpt-oss-20b on one RTX 4080 (profile
> cbd-gptoss20b-v1): each step's rows from several requests go through one
> batched pass, each request attending to its own KV cache, and every
> request's greedy IDs are checked against running it alone. Preemption,
> recurrent state sizes, and wiring into chapters 79, 173 and 180 are not
> built; the last section lists them.

One request at a time leaves most of a device idle. Chapter 71 measured
gpt-oss-20b on the RTX 4080 at 56.2 tokens per second for one request, and
most of each 17.8 ms decode step is spent reading the same weights that a
second or eighth request would also need. Continuous batching runs many
requests through each step. This chapter builds the part that decides which
requests run in each step: an iteration-level scheduler.

The scheduler is [`serve-batch.coax`](../lib/serve-batch.coax), an
`#authority` file because it decides admission. At every step it:

1. takes the requests that arrive at this step, refusing those that can
   never run;
2. admits waiting requests while a batch slot is free and the next request's
   whole state fits the memory budget;
3. gives every request that is decoding one position, then spends the rest
   of the step's token budget on prompt chunks;
4. retires every request whose last output token came out, freeing its slot
   and its state.

Which waiting request goes next is the fairness rule, and it is the subject
of the second half of the chapter.

## The configuration and the request

A configuration has six numbers and a weight per client. A request names its
client, the step it arrives at, its prompt length and the number of output
tokens it will produce. The schedule a step produces is a list of rows: for
each running request, the positions it processes in this step.

<!-- source: psionic/lib/serve-batch.coax types -->
```jai
// slots: most requests running at once. tokens: most positions processed in
// one step. kv: the state budget in positions. state: positions charged per
// request for state that is not KV (recurrent or convolution state).
// steps: the most steps a run may take; every arrival comes before it.
CbConfig :: struct {slots:int;tokens:int;kv:int;state:int;clients:int;steps:int;weight:[8]int;}
CbRequest :: struct {client:int;arrive:int;prompt:int;output:int;}
// Steps at which a request was admitted, produced its first token and
// finished; its place in the admission order; modeled clock readings.
CbFate :: struct {status:CbStatus;admit:int;first:int;finish:int;order:int;start_ns:int;first_ns:int;finish_ns:int;}
// One running request's work in one step: positions at..<at+count; emit
// when the last of them yields an output token.
CbRow :: struct {req:int;at:int;count:int;emit:bool;}
```

The state budget counts positions. A request needs its prompt plus its
output plus `state`, a fixed charge for state that is not a KV cache.
Elephant and Mammoth carry recurrent and convolution state per request; a
model like that sets `state` to that state's size in positions. The
scheduler reserves the whole need at admission, so a running request can
never run out of memory part way through.

`cb_start` checks the bounds once. Up to 16 slots, a token budget of at
least one per slot and at most 4,096, a budget of up to 2^20 positions with
`0 <= state < kv`, 1 to 8 clients with weights 1 to 8, a step limit of up to
8,192, and 1 to 64 requests whose arrival steps never go backwards and come
before the step limit.

<!-- source: psionic/lib/serve-batch.coax start -->
```jai
// Refuse a configuration or arrival list outside the bounds; otherwise an
// empty schedule at step 0.
cb_start :: (cfg:CbConfig,reqs:[]CbRequest)->CbState #export_c {
    s:CbState;s.cfg=cfg;s.status=CbStatus.config;
    if cfg.slots<1 || cfg.slots>CB_SLOTS || cfg.tokens<cfg.slots || cfg.tokens>CB_TOKENS || cfg.kv>CB_KV {return s;}
    if cfg.state<0 || cfg.state>=cfg.kv || cfg.clients<1 || cfg.clients>CB_CLIENTS || cfg.steps<1 || cfg.steps>CB_STEPS {return s;}
    for c:0..<cfg.clients {if cfg.weight[c]<1 || cfg.weight[c]>CB_WEIGHT {return s;}}
    s.status=CbStatus.order;
    if reqs.count<1 || reqs.count>CB_REQUESTS {return s;}
    for i:0..<reqs.count {
        a:=reqs[i].arrive;
        if a<0 || a>=cfg.steps || (i>0 && a<reqs[i-1].arrive) {return s;}
        s.req[i]=reqs[i];s.fate[i]=CbFate.{admit=-1,first=-1,finish=-1,order=-1};
    }
    s.n=reqs.count;s.status=CbStatus.ok;
    return s;
}
```

## One step

Arrivals come first. A request with an empty prompt or output, an unknown
client, or a need larger than the whole budget is refused with its reason;
it could never run, so it does not wait. The budget comparison is written
as `prompt > kv - state - output` so that it cannot overflow, whatever the
request says. A valid request joins its client's queue.

<!-- source: psionic/lib/serve-batch.coax arrive -->
```jai
// Arrivals at this step: refuse what can never run; queue the rest. The
// client's virtual time becomes the current one if it was behind it, so a
// client that idled earns no credit. (A client with requests queued is never
// behind: the current virtual time is the smallest queued client's.)
cb_arrive :: (from:CbState)->CbState {
    s:=from;
    for i:0..<s.n {
        r:=s.req[i];
        if r.arrive==s.step {
            s.fate[i].start_ns=s.clock;
            f:=CbStatus.ok;
            if r.prompt<1 || r.output<1 {f=CbStatus.shape;}
            else if r.client<0 || r.client>=s.cfg.clients {f=CbStatus.client;}
            else if r.prompt>s.cfg.kv-s.cfg.state-r.output {f=CbStatus.budget;}
            s.fate[i].status=f;
            if f!=CbStatus.ok {s.done+=1;}
            else {
                s.vt[r.client]=cb_max(s.vt[r.client],s.now);
                s.queued[r.client]+=1;
            }
        }
    }
    return s;
}
```

Admission takes the backlogged client with the smallest virtual time (the
next section explains it) and offers that client's oldest request. If its
need fits in the budget left, it is admitted and its client is charged. If
it does not fit, admission stops for this step, even when a smaller request
from another client would fit. Letting small requests pass a large one
would raise throughput and starve the large one; stopping keeps the order
fair and puts a bound on every wait, because the running requests always
finish and free their state.

<!-- source: psionic/lib/serve-batch.coax admit -->
```jai
// Admit while a slot is free: the backlogged client with the smallest
// virtual time (the lowest index on a tie) offers its oldest request; if that
// request's state does not fit, admission stops for this step.
cb_admit :: (from:CbState)->CbState {
    s:=from;
    while s.nrun<s.cfg.slots {
        c:=-1;
        for d:0..<s.cfg.clients {if s.queued[d]>0 && (c<0 || s.vt[d]<s.vt[c]) {c=d;}}
        if c<0 {return s;}
        h:=0;
        while s.req[h].client!=c || s.fate[h].status!=CbStatus.ok || s.fate[h].admit>=0 {h+=1;}
        need:=cb_need(s,h);
        if s.kv+need>s.cfg.kv {return s;}
        s.fate[h].admit=s.step;s.fate[h].order=s.admitted;s.admitted+=1;
        s.now=s.vt[c];s.vt[c]+=need*CB_SCALE/s.cfg.weight[c];s.queued[c]-=1;
        s.run[s.nrun]=h;s.nrun+=1;s.kv+=need;
    }
    return s;
}
```

Tokens go to decoding requests first: each gets exactly one position, its
last output token fed back in. What remains of the token budget goes to
prompt chunks in admission order. The chunk that completes a prompt also
yields the first output token. Because the budget is at least the number of
slots, every decoding request runs at every step: a long prompt arriving
never delays a running stream's next token.

<!-- source: psionic/lib/serve-batch.coax tokens -->
```jai
// Every decoding request gets one position; the rest of the step's budget
// goes to prompt chunks in admission order. The chunk that ends a prompt
// yields the first output token.
cb_tokens :: (from:CbState)->CbState {
    s:=from;
    budget:=s.cfg.tokens;s.nrows=0;
    for k:0..<s.nrun {
        i:=s.run[k];
        if s.filled[i]==s.req[i].prompt {
            s.rows[s.nrows]=CbRow.{req=i,at=s.req[i].prompt+s.emitted[i]-1,count=1,emit=true};
            s.nrows+=1;s.emitted[i]+=1;budget-=1;
        }
    }
    for k:0..<s.nrun {
        i:=s.run[k];left:=s.req[i].prompt-s.filled[i];
        if left>0 && budget>0 {
            n:=cb_min(left,budget);
            s.rows[s.nrows]=CbRow.{req=i,at=s.filled[i],count=n,emit=n==left};
            s.nrows+=1;s.filled[i]+=n;budget-=n;
            if n==left {s.emitted[i]=1;s.fate[i].first=s.step;}
        }
    }
    s.used=s.cfg.tokens-budget;
    return s;
}
```

<!-- source: psionic/lib/serve-batch.coax step -->
```jai
// One iteration: arrivals, admission, tokens, then retire every request
// whose last output token came out, freeing its slot and its state.
cb_step :: (from:CbState)->CbState #export_c {
    s:=cb_tokens(cb_admit(cb_arrive(from)));
    s.clock+=cb_step_ns(s.used);
    m:=0;
    for k:0..<s.nrun {
        i:=s.run[k];
        if s.fate[i].first==s.step {s.fate[i].first_ns=s.clock;}
        if s.emitted[i]==s.req[i].output {s.fate[i].finish=s.step;s.fate[i].finish_ns=s.clock;s.kv-=cb_need(s,i);s.done+=1;}
        else {s.run[m]=i;m+=1;}
    }
    s.nrun=m;s.step+=1;
    return s;
}
cb_run :: (cfg:CbConfig,reqs:[]CbRequest)->CbState {
    s:=cb_start(cfg,reqs);
    if s.status!=CbStatus.ok {return s;}
    while s.done<s.n && s.step<s.cfg.steps {s=cb_step(s);}
    if s.done<s.n {s.status=CbStatus.steps;}
    return s;
}
```

## Fair shares by virtual time

Each client c has a virtual time `vt[c]`. Admitting a request charges its
client `need * 840 / weight`; 840 is divisible by every weight from 1 to 8,
so every charge is an exact integer. The next client is the backlogged one
with the smallest virtual time, the lowest index on a tie. The current
virtual time `now` is the smallest backlogged client's time at its last
admission. A client whose time is behind `now` when a request arrives is
moved up to `now`, so a client that was idle cannot bank credit and then
take every slot. This is start-time fair queueing over admissions.

Two bounds follow, and the checks test both against every run.

- **Share.** At any moment, every backlogged client's virtual time lies
  between `now` and `now + C`, where C is the largest charge in the
  workload. So while every client stays backlogged, the service each has
  received per unit of weight differs from any other's by at most C.
- **Starvation.** From the moment a request heads its client's queue until
  it is admitted, another client d is admitted at most `floor(C / c_d) + 1`
  times, where `c_d` is d's smallest charge. Its virtual time cannot grow
  past `now + C`, and each admission of d adds at least `c_d`.

## A schedule by hand

Two slots, 4 positions per step, a budget of 14 positions with no extra
state, and two clients of weight 1. Five requests:

| Request | Client | Arrives | Prompt | Output | Need | Charge |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A | 0 | 0 | 5 | 2 | 7 | 5,880 |
| B | 0 | 0 | 2 | 2 | 4 | 3,360 |
| C | 1 | 0 | 3 | 1 | 4 | 3,360 |
| D | 1 | 1 | 20 | 1 | 21 | — |
| E | 1 | 2 | 2 | 3 | 5 | 4,200 |

**Step 0.** A, B and C arrive; both clients start at virtual time 0. The tie
goes to client 0, so A is admitted (memory 7 of 14, `vt[0]` = 7 × 840 =
5,880). Client 1's 0 is now the smallest, so C is admitted (memory 11,
`vt[1]` = 3,360). Both slots are full. No one is decoding, so A takes all 4
positions of its prompt, 0 to 3. C gets nothing this step.

**Step 1.** D arrives and needs 21 positions, more than the whole budget of
14: refused. A has 1 prompt position left; it takes position 4, which
completes its prompt and yields its first token. C takes its 3 prompt
positions, 0 to 2, and yields its first and only token, so C finishes and
frees 4 positions (memory 7).

**Step 2.** E arrives. Client 1 has nothing queued, and its 3,360 is ahead
of `now` = 0, so it keeps 3,360. Client 0's B waits at 5,880. Client 1 is
smaller, so E goes before B, although B arrived two steps earlier. E needs 5
(memory 12), `now` becomes 3,360 and `vt[1]` 3,360 + 4,200 = 7,560. A
decodes position 5 and yields its second and last token, then finishes
(memory 5). E's 2-position prompt takes 2 of the 3 positions left and
yields its first token.

**Step 3.** B is admitted at 5,880 (memory 9; `vt[0]` = 9,240). E decodes
position 2; B's 2-position prompt yields its first token.

**Step 4.** E decodes position 3, its third token; B decodes position 2, its
second. Both finish. Five steps, 8 output tokens.

The first check states these numbers step by step:

<!-- source: psionic/lib/serve-batch.coax check-hand -->
```jai
#test "the hand trace, step by step" {
    q:=cb_hand();s:=cb_start(cb_hand_config(),q[..]);#assert s.status==CbStatus.ok && s.n==5 && s.step==0;
    // Step 0: A, then C (client 1's virtual time 0 is below client 0's 5,880); A takes all 4 tokens.
    s=cb_step(s);#assert s.nrun==2 && s.run[0]==0 && s.run[1]==2 && s.kv==11 && s.nrows==1 && cb_row_is(s,0,0,0,4,false) && s.used==4;
    #assert s.vt[0]==5880 && s.vt[1]==3360 && s.now==0 && s.queued[0]==1 && s.queued[1]==0;
    // Step 1: D can never fit; A finishes its prompt, C prefills and finishes.
    s=cb_step(s);#assert s.fate[3].status==CbStatus.budget && s.done==2 && s.nrows==2 && cb_row_is(s,0,0,4,1,true) && cb_row_is(s,1,2,0,3,true);
    #assert s.fate[2].finish==1 && s.nrun==1 && s.kv==7 && s.used==4;
    // Step 2: E arrives at virtual time 3,360 < 5,880 and goes before B.
    s=cb_step(s);#assert s.fate[4].admit==2 && s.now==3360 && s.vt[1]==7560 && cb_row_is(s,0,0,5,1,true) && cb_row_is(s,1,4,0,2,true) && s.used==3;
    #assert s.fate[0].finish==2 && s.kv==5;
    s=cb_step(s);#assert s.fate[1].admit==3 && s.now==5880 && s.vt[0]==9240 && cb_row_is(s,0,4,2,1,true) && cb_row_is(s,1,1,0,2,true) && s.kv==9;
    s=cb_step(s);#assert s.nrows==2 && cb_row_is(s,0,4,3,1,true) && cb_row_is(s,1,1,2,1,true) && s.done==5 && s.nrun==0 && s.kv==0 && s.step==5;
    want:=[5]CbFate.[CbFate.{admit=0,first=1,finish=2,order=0},CbFate.{admit=3,first=3,finish=4,order=3},CbFate.{admit=0,first=1,finish=1,order=1},
        CbFate.{status=CbStatus.budget,admit=-1,first=-1,finish=-1,order=-1},CbFate.{admit=2,first=2,finish=4,order=2}];
    r:=cb_run(cb_hand_config(),q[..]);#assert r.status==CbStatus.ok && r.step==5;
    for i:0..<5 {f:=r.fate[i];w:=want[i];#assert f.status==w.status && f.admit==w.admit && f.first==w.first && f.finish==w.finish && f.order==w.order;}
}
```

The example, [`serve-batch.coax`](../examples/serve-batch.coax), prints the
same schedule. Each step lists the positions each
request processed and whether that row produced a token, then the memory in
use and the running requests after the step's retirements.

```shell
coax/bin/coax run psionic/examples/serve-batch.coax
```

<!-- source: psionic/examples/serve-batch.coax main -->
```jai
main :: () {
    q:=cb_hand();s:=cb_start(cb_hand_config(),q[..]);
    while s.done<s.n {s=cb_step(s);show_step(s);}
    for i:0..<s.n {
        f:=s.fate[i];
        if f.status==CbStatus.ok {print("%: admitted at step % (order %), first token %, finished %\n",NAMES[i..<i+1],f.admit,f.order,f.first,f.finish);}
        else {print("%: refused (%)\n",NAMES[i..<i+1],cb_status_name(f.status));}
    }
    print("virtual time: client 0 %, client 1 %\n",s.vt[0],s.vt[1]);
    // 16 requests from 4 clients in chapter 71's shape: 1,024 prompt
    // positions and 128 output tokens each, 512 positions per step.
    work:[16]CbRequest;
    for i:0..<16 {work[i]=CbRequest.{client=i%4,prompt=1024,output=128};}
    print("modeled (%), 16 requests of 1,024 + 128:\n",CB_PROFILE);
    for c:0..<3 {
        n:=ifx c==0 then 1 else 4*c;
        cfg:=CbConfig.{slots=n,tokens=512,kv=2048*n,clients=4,steps=CB_STEPS};
        for k:0..<4 {cfg.weight[k]=1;}
        p:=cb_report(cb_run(cfg,work[..]));
        tenths:=cast(int)(p.tok_s*10+0.5);
        print("concurrency %: % steps",n,p.steps);secs(", ",p.ns);print(", %.% tokens/s\n",tenths/10,tenths%10);
        secs("  latency P50 ",p.latency50);secs(", P95 ",p.latency95);
        secs("; first token P50 ",p.first50);secs(", P95 ",p.first95);print("\n");
    }
}
```

```text
step 0: A 0-3; then kv 11/14, 2 running
step 1: A 4 emits, C 0-2 emits; then kv 7/14, 1 running
step 2: A 5 emits, E 0-1 emits; then kv 5/14, 1 running
step 3: E 2 emits, B 0-1 emits; then kv 9/14, 2 running
step 4: E 3 emits, B 2 emits; then kv 0/14, 0 running
A: admitted at step 0 (order 0), first token 1, finished 2
B: admitted at step 3 (order 3), first token 3, finished 4
C: admitted at step 0 (order 1), first token 1, finished 1
D: refused (state never fits the budget)
E: admitted at step 2 (order 2), first token 2, finished 4
virtual time: client 0 9240, client 1 7560
modeled (cb-iteration-v1), 16 requests of 1,024 + 128:
concurrency 1: 2064 steps, 60.584 s, 33.8 tokens/s
  latency P50 30.292 s, P95 60.584 s; first token P50 28.032 s, P95 58.325 s
concurrency 4: 526 steps, 35.464 s, 57.7 tokens/s
  latency P50 22.255 s, P95 35.464 s; first token P50 14.984 s, P95 32.667 s
concurrency 8: 274 steps, 31.348 s, 65.3 tokens/s
  latency P50 26.186 s, P95 31.348 s; first token P50 12.314 s, P95 27.874 s
```

## Refusals

A configuration or arrival list outside the bounds refuses the whole run
before any step, as
[`serve-batch-refuse.coax`](../examples/serve-batch-refuse.coax) shows. A request that can never run is refused at arrival with its
reason, and the requests around it still run. A run that reaches its step
limit with requests unfinished reports `steps`.

<!-- source: psionic/examples/serve-batch-refuse.coax refuse -->
```jai
main :: () {
    c:=cb_hand_config();q:=cb_hand();
    whole("the hand-traced run",c,q[..]);
    x:=c;x.tokens=1;whole("1 token per step for 2 slots",x,q[..]);
    x=c;x.state=14;whole("state charge equal to the budget",x,q[..]);
    x=c;x.weight[1]=9;whole("a weight of 9",x,q[..]);
    y:=q;y[1].arrive=1;whole("arrivals out of order",c,y[..]);
    whole("no requests",c,q[0..<0]);
    x=c;x.steps=4;whole("a 4-step limit",x,q[..]);
    bad:=[4]CbRequest.[CbRequest.{client=0,prompt=0,output=4},CbRequest.{client=2,prompt=1,output=1},
        CbRequest.{client=1,prompt=7,output=8},CbRequest.{client=1,prompt=7,output=7}];
    r:=cb_run(c,bad[..]);
    for i:0..<4 {
        f:=r.fate[i];b:=bad[i];
        print("request % (client %, prompt %, output %): ",i,b.client,b.prompt,b.output);
        if f.status==CbStatus.ok {print("finished at step %\n",f.finish);}
        else {print("refused (%)\n",cb_status_name(f.status));}
    }
}
```

```text
the hand-traced run: ok
1 token per step for 2 slots: configuration out of bounds
state charge equal to the budget: configuration out of bounds
a weight of 9: configuration out of bounds
arrivals out of order: arrival list out of bounds or out of order
no requests: arrival list out of bounds or out of order
a 4-step limit: step limit reached
request 0 (client 0, prompt 0, output 4): refused (empty prompt or output)
request 1 (client 2, prompt 1, output 1): refused (unknown client)
request 2 (client 1, prompt 7, output 8): refused (state never fits the budget)
request 3 (client 1, prompt 7, output 7): finished at step 7
```

Request 2 needs 7 + 8 = 15 positions against a budget of 14. Request 3
needs exactly 14 and runs.

## Independent checks

Besides the hand trace and one check per boundary, five `#forall` checks run
over seeded random workloads of 1 to 12 requests from 1 to 3 clients, with
1 to 4 slots, budgets of 8 to 31 positions, state charges of 0 to 2, and
some requests that must be refused. Their helpers are in
[`serve-batch-checks.coax`](../tests/serve-batch-checks.coax).

- **A brute-force simulator.** `cbx_simulate` implements the same policy
  with no queues or running counters: at every step it recounts the running
  requests, the memory in use and each client's oldest waiting request from
  every request's record, and it resets a client's virtual time only when
  the client goes from nothing waiting to something waiting, the textbook
  rule. Every request's status, admission step, first-token step, finish
  step and admission order must equal the scheduler's, on 150 workloads.
- **Budgets every step.** After every step the memory in use equals the sum
  of the running requests' needs and is within the budget, at most `slots`
  requests run, the rows' positions add up to the step's use and stay
  within the token budget, and no request has two rows. Every finished
  request produced one token per step from its first token to its last.
- **Starvation.** For every admitted request, the admissions of each other
  client between the moment it headed its queue and its admission are
  counted and compared with `floor(C / c_d) + 1`.
- **Share.** Twelve requests, four per client, all at step 0: after every
  admission, until some client runs out, the service per weight of any two
  clients differs by at most C.
- **Batch invariance.** A stand-in model, a four-wide attention over each
  request's own cache with the next token chosen greedily from 16, runs the
  rows of every step, prompt positions in chunks mixed with other requests'
  decode positions. Each request's output tokens must equal the tokens it
  gets when its whole prompt runs in one pass and then one token at a time
  alone. A row with a wrong position or a missed chunk changes the tokens.

The stand-in model computes each row in a fixed order that does not depend
on which other rows share the step, which is what makes a batched run equal
a solo one bit for bit. A device kernel has to keep the same property: a
reduction whose order depends on the batch size would break the equality,
and chapter 41's tiled attention names its profile for that reason.

`coax mutate` on the scheduler kills all 305 mutants.

## What concurrency buys, in a model

The scheduler does not run on the device yet, so the numbers below come from
a timing model, not a measurement. One step's time is a straight line
through chapter 71's two receipts on the RTX 4080: one row (a decode token)
costs 17,792,472 ns, its decode time per token, and each further row costs
1,459,182 ns, which puts 1,024 rows at 1,510,535,658 ns, 352 ns from its
tensor-core prefill. The model assumes that a decode row in a batch costs
what a prefill row costs, and that attention over longer caches costs
nothing extra. Neither has been measured.

<!-- source: psionic/lib/serve-batch.coax timing -->
```jai
// Modeled step time: 17,792,472 ns for one row (chapter 71's decode, 2.277436 s
// for 128 tokens) plus 1,459,182 ns per further row (its tensor-core prefill,
// 1.510536 s for 1,024 rows, on the same line). A model, not a measurement.
CB_STEP_NS :: 17792472;
CB_ROW_NS :: 1459182;
```

`cb_report` turns a run into aggregate output tokens per modeled second and
nearest-rank P50 and P95 of each finished request's latency and time to
first token, measured from the start of its arrival step.

<!-- source: psionic/lib/serve-batch.coax report -->
```jai
// Aggregate output tokens per modeled second, and nearest-rank P50 and P95
// of each finished request's latency and time to first token.
cb_report :: (s:CbState)->CbReport {
    p:=CbReport.{status=s.status,steps=s.step,ns=s.clock};
    if s.status!=CbStatus.ok {return p;}
    lat:[64]int;first:[64]int;n:=0;
    for i:0..<s.n {
        f:=s.fate[i];
        if f.status==CbStatus.ok {lat[n]=f.finish_ns-f.start_ns;first[n]=f.first_ns-f.start_ns;p.tokens+=s.req[i].output;n+=1;}
    }
    p.latency50=cb_rank(lat[0..<n],50);p.latency95=cb_rank(lat[0..<n],95);
    p.first50=cb_rank(first[0..<n],50);p.first95=cb_rank(first[0..<n],95);
    if p.tokens>0 {p.tok_s=cast(f64)p.tokens*1000000000.0/cast(f64)s.clock;}
    return p;
}
```

The example runs 16 requests of chapter 71's shape (1,024 prompt positions,
128 output tokens) from four clients, all arriving at step 0, with 512
positions per step and 2,048 positions of budget per slot:

| Concurrency | Steps | Modeled time | Tokens/s | Latency P50 | Latency P95 | First token P50 | First token P95 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2064 | 60.584 s | 33.8 | 30.292 s | 60.584 s | 28.032 s | 58.325 s |
| 4 | 526 | 35.464 s | 57.7 | 22.255 s | 35.464 s | 14.984 s | 32.667 s |
| 8 | 274 | 31.348 s | 65.3 | 26.186 s | 31.348 s | 12.314 s | 27.874 s |

Under this model, concurrency 8 almost doubles aggregate throughput over one
request at a time, and the median time to first token falls by more than
half. The median latency is worse at 8 than at 4, because each step carries
both decode rows and prompt chunks, so every running request's steps are
slower. The next section measures the device instead.

## Steps on the RTX 4080

[`device-batch.coax`](../lib/gptoss/device-batch.coax) runs the scheduler's
steps on the original gpt-oss-20b. It is a true ragged batch, not one launch
per request: chapter 67's batched prefill already takes any number of rows
through every weight product (norms, QKV, output projection, router and the
grouped experts), and none of those products mixes rows. Only attention
needs to know which request a row belongs to. So each layer runs its
products once over all of the step's rows and runs rotation, scores,
softmax, context and the cache commit once per segment, a segment being one
request's rows in this step, on that request's own cache.

Each running request owns a region of upload slot 1: its whole KV cache, in
the layout of the session's own cache, then its eight control words.

<!-- source: psionic/lib/gptoss/device-batch.coax regions -->
```jai
// A region holds a request's whole cache (the layout of slot 16) and then its
// eight control words, so it is 8-byte aligned.
cbd_region_bytes :: (p:GDPlan)->int #export_c {return p.counts[16]*4+64;}
// Regions in upload slot 1. Refused when the plan already uses slot 1, when
// the tensor-core prefill is on (its products are not checked here), or for
// 0 or more than 8 regions.
cbd_with_regions :: (p:GDPlan,s:GShape,regions:int)->GDPlan #export_c {
    if p.status!=GStatus.ok {return GDPlan.{status=p.status};}
    if !g_shape(s) || p.mma!=0 || p.upload[1]!=0 || regions<1 || regions>CBD_REGIONS {return GDPlan.{status=GStatus.shape};}
    r:=p;r.upload[1]=regions*cbd_region_bytes(p);return r;
}
cbd_regions :: (p:GDPlan)->int #export_c {return p.upload[1]/cbd_region_bytes(p);}
cbd_cache :: (p:GDPlan,s:GShape,region:int,layer:int)->GDRef {
    return GDRef.{buffer=1,offset=region*cbd_region_bytes(p)+g_cache_base(s,layer)*4,count=2*s.kv_heads*64*g_layer_capacity(s,layer),elem=4};
}
cbd_control :: (p:GDPlan,region:int)->GDRef {return GDRef.{buffer=1,offset=region*cbd_region_bytes(p)+p.counts[16]*4,count=8,elem=8};}
// Byte offset of a region's control words, for the host's read.
cbd_control_offset :: (p:GDPlan,region:int)->int #export_c {return cbd_control(p,region).offset;}
```

For gpt-oss-20b at a context of 2,048, 12 sliding layers keep 128 positions
and 12 full layers keep 2,048, and each position holds 8 key and 8 value
heads of 64 floats, 1,024 floats in all. A region is
24 × 1,088 × 1,024 × 4 + 64 = 106,954,816 bytes, and eight regions take
855.6 MB. With them the device holds 15.63 GB of its 16.71 GB.

A step is admitted before any program is built:

<!-- source: psionic/lib/gptoss/device-batch.coax admit -->
```jai
// A step is admitted when its segments fit the plan: 1 to 8 segments in
// distinct regions, 1 to `batch` rows in all, every segment inside the
// context, and every ID to upload inside the vocabulary.
cbd_admit :: (s:GShape,p:GDPlan,segs:[]CbdSeg,ids:[]int)->CbdStep #export_c {
    st:=CbdStep.{status=GStatus.shape};
    if p.status!=GStatus.ok || !g_shape(s) || p.mma!=0 || cbd_regions(p)<1 {return st;}
    st.status=GStatus.state;
    if segs.count<1 {return st;}
    for j:0..<segs.count {
        g:=segs[j];
        if g.region<0 || g.region>=cbd_regions(p) || g.count<1 || g.at<0 || g.at>s.capacity-g.count || g.limit<1 || g.limit>s.capacity {return CbdStep.{status=GStatus.state};}
        for i:0..<j {if segs[i].region==g.region {return CbdStep.{status=GStatus.state};}}
        st.rows+=g.count;
    }
    if st.rows>p.batch || ids.count!=st.rows {return CbdStep.{status=GStatus.state};}
    for id:ids {if id<0 || id>=s.vocab {return CbdStep.{status=GStatus.token};}}
    st.status=GStatus.ok;st.segs=segs.count;return st;
}
```

Take step 1 of the hand trace. A has its last prompt position, 4, and C
its three prompt positions, 0 to 2. Region binding gives A region 0 (from
step 0) and C region 1, the lowest one no live request holds. The step
uploads four IDs: A's prompt ID 4 and C's prompt IDs 0, 1 and 2. The step's
rows are 0 for A and 1 to 3 for C, and both segments emit. C starts a
prompt, so the head clears region 1's control words before checking that C
is at position 0. In every layer the products run over 4 rows; A's
attention runs on row 0 at position 4 over region 0's cache, C's on rows 1
to 3 at positions 0 to 2 over region 1's. The tail reads out row 0 for A and
row 3 for C, each into its own region's control words. The checks
assert exactly these segments, IDs and regions.

<!-- source: psionic/lib/gptoss/device-batch.coax bind -->
```jai
// Regions for the requests in a step's rows: a request keeps the region it
// was given; a new one takes the lowest region that no live request holds.
// A request is live from its region's assignment until the step it finished
// in has run. Returns false when no region is free.
cbd_bind :: (s:CbState,region:[]int,regions:int)->bool #export_c {
    if region.count<s.n || regions<1 || regions>CBD_REGIONS {return false;}
    for k:0..<s.nrows {
        i:=s.rows[k].req;
        if region[i]<0 {
            used:[8]bool;
            for j:0..<s.n {done:=s.fate[j].finish>=0 && s.fate[j].finish<s.step-1;if region[j]>=0 && !done {used[region[j]]=true;}}
            r:=0;while r<regions && used[r] {r+=1;}
            if r==regions {return false;}
            region[i]=r;
        }
    }
    return true;
}
// The step's segments, in row order, and the IDs to upload: a prompt row
// feeds its prompt ID, a decode row the request's last chosen ID. `first`
// gives each request's first prompt ID in `prompts`; `last` its last ID.
cbd_segments :: (s:CbState,region:[]int,prompts:[]int,first:[]int,last:[]int,segs:[]CbdSeg,ids:[]int)->int #export_c {
    if region.count<s.n || first.count<s.n || last.count<s.n || segs.count<s.nrows {return -1;}
    n:=0;
    for k:0..<s.nrows {
        r:=s.rows[k];i:=r.req;p:=s.req[i].prompt;
        segs[k]=CbdSeg.{region=region[i],at=r.at,count=r.count,emit=r.emit,limit=s.req[i].output};
        for t:r.at..<r.at+r.count {
            if n>=ids.count {return -1;}
            if t<p {if first[i]+t<0 || first[i]+t>=prompts.count {return -1;}ids[n]=prompts[first[i]+t];}else {ids[n]=last[i];}
            n+=1;
        }
    }
    return n;
}
```

<!-- source: psionic/lib/gptoss/device-batch.coax head -->
```jai
// Before the layers: a segment that starts a prompt clears its region's
// control words; every segment checks that its position is the one its
// control words expect; then all rows' IDs are embedded at once.
cbd_head :: (s:GShape,p:GDPlan,params:[]GDParam,segs:[]CbdSeg,rows:int,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    w:=GDWriter.{plan=p,ops=ops,args=args};d:=s.width;
    for g:segs {
        control:=cbd_control(p,g.region);
        if g.at==0 {w=gd_emit(w,1,64,0,1,GDScalars.{},GDViews.{a=GDRef.{buffer=1,offset=control.offset,count=64}});}
        w=gd_emit(w,0,1,3,1,GDScalars.{a=0,b=g.at,c=s.vocab},GDViews.{a=control});
    }
    w=gd_emit(w,24,rows*d,2,4,GDScalars.{a=d,b=0},GDViews.{a=gd_all(p,26),b=gd_weight(params,0,2),c=gd_slice(p,28,0,rows*d),d=gd_all(p,37)});
    return gd_guard(w,rows*d).result;
}
```

<!-- source: psionic/lib/gptoss/device-batch.coax layer -->
```jai
// One layer. Every product runs once over all rows; rotation, scores,
// softmax, context and the cache commit run per segment, on that segment's
// rows and its own cache, with chapter 67's scalars for its position.
cbd_layer :: (s:GShape,p:GDPlan,params:[]GDParam,segs:[]CbdSeg,rows:int,layer:int,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    w:=GDWriter.{plan=p,ops=ops,args=args};d:=s.width;q:=s.heads*64;k:=s.kv_heads*64;h:=s.hidden;z:=s.selected;n:=rows;
    x:=gd_slice(p,28,0,n*d);normal:=gd_slice(p,29,0,n*d);qkv:=gd_slice(p,30,0,n*(q+2*k));projected:=gd_slice(p,32,0,n*d);router:=gd_slice(p,33,0,n*s.experts);flags:=gd_all(p,37);
    base:=3+15*layer;ids:=gd_slice(p,14,layer*z,z);weights:=gd_slice(p,15,layer*z,z);trace:=gd_slice(p,13,layer*2*d,d);cap:=g_layer_capacity(s,layer);window:=0;if cap<s.capacity {window=s.window;}
    w=gd_batch_norm(w,d,n,gd_weight(params,base,2),x,normal);
    w=gd_batch_bf(w,q+2*k,d,n,gd_weight(params,base+4,8),gd_weight(params,base+3,2),normal,qkv);
    row:=0;
    for g:segs {
        m:=g.count;at:=g.at;cache:=cbd_cache(p,s,g.region,layer);stride:=gs_window(at+m,cap);
        sq:=gd_slice(p,30,row*(q+2*k),m*(q+2*k));pending:=gd_slice(p,34,row*2*k,m*2*k);attention:=gd_slice(p,31,row*q,m*q);
        rotations:=m*(s.heads+s.kv_heads)*32;w=gd_emit(w,35,rotations,3,4,GDScalars.{a=s.heads,b=s.kv_heads,c=at},GDViews.{a=sq,b=gd_all(p,25),c=pending,d=flags});w=gd_guard(w,rotations);
        w=gd_emit(w,36,m*s.heads*stride,6,4,GDScalars.{a=s.heads,b=s.kv_heads,c=at,d=window,e=cap,f=stride},GDViews.{a=sq,b=cache,c=pending,d=gd_all(p,36)});
        w=gd_emit(w,37,m*s.heads,4,3,GDScalars.{a=s.heads,b=at,c=window,d=stride},GDViews.{a=gd_weight(params,base+5,2),b=gd_all(p,36),c=flags});w=gd_guard(w,m*s.heads);
        w=gd_emit(w,38,m*q*8,8,4,GDScalars.{a=s.heads,b=s.kv_heads,c=m,d=at,e=window,f=cap,g=stride,h=8},GDViews.{a=gd_all(p,36),b=cache,c=pending,d=gd_all(p,40)});
        w=gd_emit(w,28,m*q,3,4,GDScalars.{a=q,b=m,c=8},GDViews.{a=gd_all(p,40),b=GDRef.{elem=2},c=attention,d=flags});w=gd_guard(w,m*q);
        row+=m;
    }
    w=gd_batch_bf(w,d,q,n,gd_weight(params,base+2,8),gd_weight(params,base+1,2),gd_slice(p,31,0,n*q),projected);
    w=gd_emit(w,33,n*d,2,4,GDScalars.{a=d,b=n},GDViews.{a=x,b=projected,c=trace,d=flags});w=gd_guard(w,n*d);
    w=gd_batch_norm(w,d,n,gd_weight(params,base+10,2),x,normal);
    w=gd_batch_bf(w,s.experts,d,n,gd_weight(params,base+7,8),gd_weight(params,base+6,2),normal,router);
    w=gd_emit(w,29,n,3,6,GDScalars.{a=s.experts,b=z,c=n},GDViews.{a=router,b=gd_all(p,45),c=gd_all(p,46),d=ids,e=weights,f=flags});w=gd_guard(w,n);
    w=gd_emit(w,30,s.experts,3,3,GDScalars.{a=s.experts,b=z,c=n},GDViews.{a=gd_all(p,45),b=gd_all(p,38),c=gd_all(p,39)});
    w=gd_emit(w,31,1,1,1,GDScalars.{a=s.experts},GDViews.{a=gd_all(p,39)});
    w=gd_batch_mx(w,2*h,d,n,s.experts,z,0,gd_weight(params,base+11,8),gd_weight(params,base+12,1),gd_weight(params,base+8,2),normal,gd_all(p,41));
    w=gd_emit(w,10,n*z*h,1,3,GDScalars.{a=h},GDViews.{a=gd_all(p,41),b=gd_all(p,42),c=flags});w=gd_guard(w,n*z*h);
    w=gd_batch_mx(w,d,h,n,s.experts,z,1,gd_weight(params,base+13,8),gd_weight(params,base+14,1),gd_weight(params,base+9,2),gd_all(p,42),gd_all(p,43));
    w=gd_emit(w,34,n*d,3,5,GDScalars.{a=d,b=z,c=n},GDViews.{a=x,b=gd_all(p,43),c=gd_all(p,46),d=gd_slice(p,13,(2*layer+1)*d,d),e=flags});w=gd_guard(w,n*d);
    row=0;
    for g:segs {
        m:=g.count;kept:=gs_window(m,cap);
        w=gd_emit(w,40,kept*2*k,4,2,GDScalars.{a=s.kv_heads,b=g.at,c=m,d=cap},GDViews.{a=gd_slice(p,34,row*2*k,m*2*k),b=cbd_cache(p,s,g.region,layer)});
        row+=m;
    }
    return w.result;
}
```

<!-- source: psionic/lib/gptoss/device-batch.coax tail -->
```jai
// After the layers: every emitting segment's last row is normed and read out
// on its own, exactly as chapter 67's prefill reads its last row; the flags
// of the whole step reach the step's control words (slot 24); every segment
// advances its position, and an emitting one chooses its next ID greedily.
cbd_tail :: (s:GShape,p:GDPlan,params:[]GDParam,segs:[]CbdSeg,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    w:=GDWriter.{plan=p,ops=ops,args=args};d:=s.width;row:=0;
    for g:segs {
        row+=g.count;
        control:=cbd_control(p,g.region);
        if g.emit {
            w=gd_emit(w,1,p.flags,0,1,GDScalars.{},GDViews.{a=gd_all(p,22)});w.result.flags=0;
            w=gd_norm(w,d,gd_weight(params,1,2),gd_slice(p,28,(row-1)*d,d),gd_all(p,1));
            w=gd_bf(w,s.vocab,d,gd_weight(params,2,8),GDRef.{elem=2},gd_all(p,1),gd_all(p,17));
            summaries:=(w.result.flags+255)/256;w=gd_emit(w,18,summaries,0,2,GDScalars.{},GDViews.{a=gd_slice(p,22,0,w.result.flags),b=gd_slice(p,23,0,summaries)});
            w=gd_emit(w,19,1,0,2,GDScalars.{},GDViews.{a=gd_slice(p,23,0,summaries),b=gd_all(p,24)});
        }
        w=gd_emit(w,42,1,1,1,GDScalars.{a=g.count},GDViews.{a=control});
        if g.emit {
            w=gd_emit(w,22,p.counts[27],0,2,GDScalars.{},GDViews.{a=gd_all(p,17),b=gd_all(p,27)});
            w=gd_emit(w,23,1,1,3,GDScalars.{a=g.limit},GDViews.{a=gd_all(p,17),b=gd_all(p,27),c=control});
        }
    }
    return w.result;
}
```

The checks compare the programs, not just their shapes. A single segment's
layer must equal chapter 67's prefill layer op for op, scalar for scalar
and view for view, with the prefill's shadow cache replaced by the region;
in a step of three segments the shared products must equal those of one
six-row segment and each segment's attention must equal its own
single-segment layer moved to its rows; a single emitting segment's tail
must equal the prefill's readout of its last row. `coax mutate` on the unit
kills all of its mutants. The host
([`gptoss_batch.c`](../tests/gptoss_batch.c)) adds one call to the engine,
`pg_fetch`, which reads a region's control words; everything else it does
is upload IDs, run programs and read clocks.

### The run

```shell
COAX_CC=gcc psionic/tests/check_device_batch.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee \
  5 447e9a1f919e90b3 7c6705e4ab014dfd
```

The workload is eight requests from four clients, all arriving at step 0:
chapter 71's two fixture prompts ("Say hi." and "List the integers from 1
to 100, separated by commas.", rendered with Harmony, up to 256 IDs each),
and six prompts of chapter 71's IDs, 96 to 416 long, up to 64 IDs each.
Each request first runs alone on chapter 70's path (one prompt pass, then
one decode step per token). The two fixture prompts give 24 and 256 IDs
whose words equal the reference words in chapter 71; the eight requests give
664 output IDs. Each request's output length is then fixed to its length
alone, and the scheduler runs all eight with 128 positions per step and 1,
4 or 8 slots: one untimed warm-up and five timed runs at each concurrency.

**The independent check.** In every run at every concurrency, every
request's IDs equal its IDs alone, ID for ID and in number. Decode rows
here go through the batched kernels and alone they go through chapter 66's
decode kernels, so the equality also shows that the two paths choose the
same greedy IDs on this workload.

| Concurrency | Steps | Mean time (sd) | Tokens/s | Latency P50 | Latency P95 | First token P50 | First token P95 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 673 | 29.534 s (48.7 ms) | 22.5 | 16.163 s | 29.534 s | 13.824 s | 27.143 s |
| 4 | 257 | 20.348 s (25.4 ms) | 32.6 | 10.654 s | 20.348 s | 1.204 s | 11.623 s |
| 8 | 257 | 19.325 s (2.4 ms) | 34.4 | 11.909 s | 19.325 s | 1.498 s | 5.437 s |

Latency and time to first token are measured from the start of the run, when
all eight requests arrive; each is the mean over the five runs of that run's
nearest-rank P50 or P95 over the eight requests. The
[record](../../docs/verification/psionic-ch175-continuous-batching-2026-11-02.md)
has every run, the build words and the load average.

Batching helps the batched path: concurrency 4 is 1.45 times faster than
concurrency 1 and concurrency 8 is 1.53 times faster, and chapter 34's
comparison calls both faster beyond two standard errors. The median time to
first token falls from 13.8 s to 1.2 s.

**It does not beat one request at a time.** The same eight requests run
alone, one after another on chapter 70's path, took 15.285 s for 664 IDs,
43.4 tokens per second, in a single run. That is faster than this batched
path at any concurrency. Three costs explain it. A decode row in the
batched kernels costs more than in chapter 66's decode kernels, which are
tuned for one row: concurrency 1 takes 673 steps in 29.5 s, 43.9 ms a step.
The host waits for the device at the end of every step, to read the chosen
IDs, and builds 26 programs per step in that wait. And every step at
concurrency 4 and 8 is capped at 128 positions, so the long prompts take as
many steps at 8 as at 4 (257). None of these is measured separately yet.

Chapter 71's gate now admits concurrency 4 and 8 with this evidence (it
prints the reason `none` for each), but its own timed runs are still the
one-request workload.

## What remains

- **Faster batched decode.** The batched path is slower than one request at
  a time on chapter 70's path. A row-count-aware decode kernel, a recorded
  step without the per-step host wait (chapter 99), and the tensor-core
  prefill products (refused here for now) are the next steps; none is
  built.
- **Preemption and priority classes.** Requests are never preempted; a
  priority class is only a weight.
- **The server.** Chapter 79's Responses server, chapter 173's capability
  admission and chapter 180's metering do not call this scheduler yet, and
  chapter 172's load generator has not driven it.
- **Recurrent state.** `state` is a configured number; Elephant's and
  Mammoth's actual per-request state sizes are not plugged in.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- Goyal, Vin and Cheng, "Start-time fair queueing: a scheduling algorithm
  for integrated services packet switching networks", SIGCOMM 1996.
- Yu et al., "Orca: a distributed serving system for transformer-based
  generative models", OSDI 2022 (iteration-level scheduling).
