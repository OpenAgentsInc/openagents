# 5. A sandbox and approvals

> **Status: partly implemented.** A sandbox runs on the CPU around chapter
> 203's tools. Its policy is data: an effect ceiling and the rooms of the
> world a session may ever touch, the effect, rooms and cost that run
> without asking, and limits on calls, call bytes, ticks per call and the
> session's clock. The policy decides before every run. A call between what
> runs freely and the ceiling pauses the session until a typed answer
> arrives: approve, deny or expire, for one call or as a pattern of tool
> and room. Approval cannot pass the ceiling or a limit. Every call, request
> and answer is an event in chapter2's log, and a replay of the log
> rebuilds the same events and the same world. Still to come: real
> processes and files (declared roots on disk, memory and process-count
> limits, stopping a process that overruns), the repeated-failure guard,
> approvals from a person through a client, and policies and answers as
> messages in `proto/`.

See [policy and actual confinement](CONTRACT.md#plugin-and-tool-trust) for
the live engine boundaries and defaults. The three-room experiment below
checks admission decisions; it does not confine native processes.

Chapter4's grant answers whether a tool may be called in a turn. It does
not say which parts of the world the tool may touch, how much a session may
spend, or what happens when a call is risky but not forbidden. This chapter
adds a sandbox around every call. No tool inherits authority by default:
the sandbox runs a call only when its policy allows it, or when the policy
allows it to be asked about and a person approves.

The world is still chapter4's: a light switch in each of three rooms,
`hall`, `den` and `study`, read by `get` and written by `set`. The rooms
stand in for the directories a process would see. A real process sandbox
needs operating-system confinement, which this chapter does not build; the
decision logic, the approval flow and the log are what run here.

Everything is in one `#authority` file,
[harness/sandbox.coax](../../psionic/lib/harness/sandbox.coax), because it decides
what runs. Its checks leave no mutant alive.

## The profile

This chapter's profile is `cpu-harness-sandbox-v1`:

- The tools are chapter4's registry: `add` and `div` (effect none),
  `get` (read) and `set` (write). `get` and `set` name a room in their first
  field. `add` and `div` touch no room.
- Time is a simulated clock in ticks. Each body takes a fixed number:
  `add` 1, `div` 2, `set` 3, `get` 1. The clock also moves forward when an
  answer arrives later than the session's current time.
- One session per log, and one request pending at a time. A session holds
  up to 4 pattern approvals; a fifth replaces the oldest.
- Session ids are given by the harness and are distinct.

## The policy

A policy has two layers. The ceiling is the most the session may ever do:
`ceiling`, the strongest effect, and `reach`, the rooms. Inside it, `allow`,
`open` and `cost` say what runs without asking. Rooms are masks: bit k is
room k, so `3` is hall and den. The rest are limits.

The chapter's policy, `SB_POLICY`:

| Field | Value | Meaning |
| --- | --- | --- |
| `ceiling` | write | No call with a stronger effect ever runs |
| `reach` | 3 | Only hall and den, ever |
| `allow` | read | Effects up to read run without asking |
| `open` | 1 | Only the hall runs without asking |
| `cost` | 1 | Calls that cost more than 1 ask |
| `calls` | 8 | Calls the session may run |
| `bytes` | 200 | Call text the session may run, in bytes |
| `ticks` | 30 | The session's clock may not pass 30 |
| `timeout` | 3 | No call may take more than 3 ticks |
| `wait` | 5 | A request stays open, and a pattern holds, for 5 ticks |

A policy is valid only when what runs freely is inside the ceiling: `allow`
no stronger than `ceiling`, `open` a subset of `reach`, and no limit
negative. An invalid policy refuses every call, and a session cannot be
opened with one.

## Deciding a call

The decision for a call that chapter4 has admitted goes in a fixed
order. The limits come first, then the ceiling. Nothing passes those two,
not even an approval. Then a pattern the session holds, or an approval, runs
the call. Otherwise the call asks when its effect is above `allow`, its
room is outside `open`, or its cost is above `cost`, in that order, and runs
when none of them holds.

<!-- source: psionic/lib/harness/sandbox.coax policy -->
```jai
// A policy is valid when what runs freely is inside the ceiling: allow no
// stronger than ceiling, open a subset of reach, reach a mask of the three
// rooms, and no limit negative.
sb_valid :: (p:SbPolicy)->bool {
    if cast(u8)p.allow>cast(u8)p.ceiling || p.reach<0 || p.reach>7 || (p.open|p.reach)!=p.reach {return false;}
    return p.cost>=0 && p.calls>=0 && p.bytes>=0 && p.ticks>=0 && p.timeout>=0 && p.wait>=0;
}
// Whether mask holds room; a call that names no room (-1) is in every scope.
sb_in :: (mask:int,room:int)->bool {return room<0 || ((mask>>room)&1)==1;}
// The room a call touches: set's and get's first field, else none.
sb_room :: (tools:[]HtTool,c:HtEvent)->int {
    b:=tools[c.tool].body;if b==HtBody.set || b==HtBody.get {return c.values[0];}
    return -1;
}
// Whether the session holds an unexpired pattern for this tool and room.
sb_held :: (s:SbSession,tool:int,room:int)->bool {
    for k:0..<nx_min(s.held,SB_HELD) {x:=s.patterns[k];if x.tool==tool && x.room==room && s.clock<=x.until {return true;}}
    return false;
}
// The decision for an admitted call of n bytes: the limits first, then the
// ceiling, which nothing passes; then a held pattern or an approval runs it;
// otherwise it asks when its effect, its room or its cost is past what runs
// freely.
sb_decide :: (p:SbPolicy,s:SbSession,tools:[]HtTool,tool:int,room:int,n:int,approved:bool)->SbEvent {
    t:=tools[tool];d:=SB_TICKS[cast(int)t.body];e:=SbEvent.{room=room};
    if !sb_valid(p) {e.why=SbWhy.policy;}
    else if s.calls>=p.calls {e.why=SbWhy.calls;}
    else if s.bytes+n>p.bytes {e.why=SbWhy.bytes;}
    else if d>p.timeout {e.why=SbWhy.timeout;}
    else if s.clock+d>p.ticks {e.why=SbWhy.time;}
    else if cast(u8)t.effect>cast(u8)p.ceiling {e.why=SbWhy.effect;}
    else if !sb_in(p.reach,room) {e.why=SbWhy.scope;}
    else if approved || sb_held(s,tool,room) {e.verdict=SbVerdict.run;}
    else {
        e.verdict=SbVerdict.ask;
        if cast(u8)t.effect>cast(u8)p.allow {e.why=SbWhy.effect;}
        else if !sb_in(p.open,room) {e.why=SbWhy.scope;}
        else if t.cost>p.cost {e.why=SbWhy.cost;}
        else {e.verdict=SbVerdict.run;}
    }
    return e;
}
```

Because the ceiling is checked before an approval is considered, there is
no path from a request to a run that the ceiling forbids. Escalation can
only reach what the policy already put inside the ceiling.

## By hand: one session

Session 1 runs under `SB_POLICY` with every tool granted 9 calls and a cost
budget of 99. The table works out each step from the rules above. The clock,
calls and bytes columns are the session's state after the step.

| Step | Input | Outcome | Clock | Calls | Bytes |
| --- | --- | --- | --- | --- | --- |
| 0 | `get{"room":"hall"}`, 18 bytes | read, hall is open, cost 1: run, result false | 1 | 1 | 18 |
| 1 | `get{"room":"den"}`, 17 bytes | den is in reach but not open: ask `scope`, request 0, deadline 1 + 5 = 6 | 1 | 1 | 18 |
| 2 | `add{"a":1,"b":2}` | a request is pending: refuse `paused` | 1 | 1 | 18 |
| 3 | answer request 0: approve, at 2 | clock moves to 2; limits hold (2 + 1 ≤ 30): run, result false | 3 | 2 | 35 |
| 4 | `set{"room":"den","on":true}`, 27 bytes | write is above `allow`: ask `effect`, request 1, deadline 8 | 3 | 2 | 35 |
| 5 | answer request 1: deny, at 4 | refuse `denied`; nothing runs | 4 | 2 | 35 |
| 6 | the same `set` | ask `effect`, request 2, deadline 4 + 5 = 9 | 4 | 2 | 35 |
| 7 | answer request 2: approve as a pattern, at 10 | 10 is past 9: refuse `expired`; no pattern is kept | 10 | 2 | 35 |
| 8 | the same `set` | ask `effect`, request 3, deadline 15 | 10 | 2 | 35 |
| 9 | answer request 3: approve as a pattern, at 11 | pattern (`set`, den) until 11 + 5 = 16; run, 3 ticks, den on | 14 | 3 | 62 |
| 10 | the same `set` | 14 ≤ 16, the pattern holds: run | 17 | 4 | 89 |
| 11 | `set{"room":"den","on":false}` | 17 > 16, the pattern has lapsed: ask `effect`, request 4, deadline 22 | 17 | 4 | 89 |
| 12 | answer for session 2, request 4 | refuse `session` | 17 | 4 | 89 |
| 13 | answer request 3, approve, at 18 | request 3 is not pending: refuse `stale` | 17 | 4 | 89 |
| 14 | answer request 4, approve, at 16 | 16 is before the clock: refuse `order` | 17 | 4 | 89 |
| 15 | answer request 4: expire, at 18 | refuse `expired`; den stays on | 18 | 4 | 89 |
| 16 | `set{"room":"study","on":true}` | study is outside `reach`: refuse `scope` | 18 | 4 | 89 |
| 17 | `div{"a":9,"b":0}`, 16 bytes | effect none, no room, cost 1: run, 2 ticks; the body refuses `run` (division by zero) | 20 | 5 | 105 |
| 18 | answer request 4, approve, at 19 | the request expired at step 15: refuse `stale` | 20 | 5 | 105 |

A request does not charge the session; only a run does. At the end the den
is on and the other rooms are off. Step 13 is an approval used a second
time, step 12 is an approval sent to the wrong session, and step 18 is an
approval sent after its request expired. Each is refused, and none changes
the world.

## Every outcome is logged

Each outcome is a `context` event in chapter2's log, so the same hash
chain that covers a model's requests covers the sandbox's decisions. The
first event holds the policy. A call's event holds the session, the
verdict, why, chapter4's tool, the room, chapter4's outcome and
value, the request, and then the call's text, one token per byte. An
answer's event holds the answer as given and then what took effect.

<!-- source: psionic/lib/harness/sandbox.coax log -->
```jai
// Room for one more event of n tokens, outside any open turn.
sb_fits :: (l:[]LgLog,n:int)->bool {
    g:=l[0];return g.count<g.events.count && n<=g.tokens.count-g.used && !lg_fold_log(g,g.count).open;
}
sb_say :: (l:[]LgLog,t:[]int) {#assert lg_say(l,LgKind.context,t)==LgStatus.ok;}
// Open a session on an empty log: the policy is its first event.
sb_open :: (id:int,p:SbPolicy,s:[]SbSession,l:[]LgLog)->SbWhy {
    if !sb_valid(p) {return SbWhy.policy;}
    if l[0].count!=0 || !sb_fits(l,SB_OPEN_SIZE) {return SbWhy.log;}
    s[0]=SbSession.{id=id,policy=p};
    t:=[SB_OPEN_SIZE]int.[SB_OPEN,id,cast(int)cast(u8)p.ceiling,p.reach,cast(int)cast(u8)p.allow,p.open,p.cost,p.calls,p.bytes,p.ticks,p.timeout,p.wait];
    sb_say(l,t[..]);return SbWhy.none;
}
// A call's event: the session, the outcome, chapter 203's verdict and value,
// the request, then the call's text, byte by byte.
sb_log_call :: (l:[]LgLog,id:int,e:SbEvent,text:string) {
    t:[SB_CALL_HEAD+HT_TEXT_MAX]int;
    head:=[SB_CALL_HEAD]int.[SB_CALL,id,cast(int)e.verdict,cast(int)e.why,e.call.tool,e.room,cast(int)e.call.why,e.call.value,e.request];
    for x,i:head {t[i]=x;}for c,i:text {t[SB_CALL_HEAD+i]=cast(int)c;}
    sb_say(l,t[0..<SB_CALL_HEAD+text.count]);
}
// An answer's event: the answer as given, then what took effect.
sb_log_answer :: (l:[]LgLog,a:SbAnswer,e:SbEvent) {
    t:=[SB_ANSWER_SIZE]int.[SB_ANSWER,a.session,a.request,cast(int)a.decision,ifx a.pattern then 1 else 0,a.at,
        cast(int)e.verdict,cast(int)e.why,cast(int)e.decision,cast(int)e.call.why,e.call.value];
    sb_say(l,t[..]);
}
```

By hand, from the table above, with the enums as their indices (verdicts
refuse 0, run 1, ask 2; `scope` is 11, `effect` 10, `denied` 17; decisions
approve 1, deny 2, expire 3):

| Event | Tokens |
| --- | --- |
| 0, the policy | `1 1 2 3 1 1 1 8 200 30 3 5` |
| 1, step 0 | `2 1 1 0 3 0 0 0 -1` and the 18 bytes of `get{"room":"hall"}` |
| 2, step 1 | `2 1 2 11 3 1 0 0 0` and 17 bytes |
| 4, step 3 | `3 1 0 1 0 2 1 0 1 0 0` |
| 6, step 5 | `3 1 1 2 0 4 0 17 2 0 0` |
| 10, step 9 | `3 1 3 1 1 11 1 0 1 0 1` |

A call that never enters the session is not logged. That covers three
cases: text longer than chapter4's 256 bytes, a world too small for the
registry's rooms, and a log with no room for the event. The sandbox checks
for room first, so an effect never happens without its event.

## A call

<!-- source: psionic/lib/harness/sandbox.coax call -->
```jai
// Run an admitted, decided call through chapter 203's pipeline, and charge
// it to the session: one call, its bytes, and its ticks on the clock.
sb_run :: (env:SbTools,s:[]SbSession,text:string,world:[]int)->HtEvent {
    u:=[1]HtUsage.[s[0].usage];c:=ht_call(env.tools,env.grant,u[..],text,world,env.work);
    s[0].usage=u[0];s[0].calls+=1;s[0].bytes+=text.count;s[0].clock+=SB_TICKS[cast(int)env.tools[c.tool].body];
    return c;
}
// One call. Refused before it enters the session (not logged) when the text
// is longer than a call may be, the world is too small or the log is full.
// Otherwise: a paused session refuses it; chapter 203 admits it; the policy
// decides; it runs, waits as a request, or is refused. Logged either way.
sb_call :: (env:SbTools,s:[]SbSession,text:string,world:[]int,l:[]LgLog)->SbEvent {
    e:=SbEvent.{};
    if text.count>HT_TEXT_MAX || world.count<HT_CHOICES_MAX {e.why=SbWhy.shape;return e;}
    if !sb_fits(l,SB_CALL_HEAD+text.count) {e.why=SbWhy.log;return e;}
    x:=s[0];
    if x.pending {e.why=SbWhy.paused;}
    else {
        c:=ht_admit(env.tools,env.grant,x.usage,text,env.work);e.call=c;
        if c.why!=HtWhy.none {e.why=SbWhy.tool;}
        else {
            d:=sb_decide(x.policy,x,env.tools,c.tool,sb_room(env.tools,c),text.count,false);e.verdict=d.verdict;e.why=d.why;e.room=d.room;
            if d.verdict==SbVerdict.run {e.call=sb_run(env,s,text,world);}
            if d.verdict==SbVerdict.ask {
                r:=SbRequest.{id=x.asked,tool=c.tool,room=d.room,deadline=x.clock+x.policy.wait,why=d.why,count=text.count};
                for b,i:text {r.text[i]=b;}
                s[0].request=r;s[0].pending=true;s[0].asked=x.asked+1;e.request=r.id;
            }
        }
    }
    sb_log_call(l,x.id,e,text);return e;
}
```

A call that runs goes through chapter4's `ht_call` unchanged, so its
budgets, its argument grammar and its result type still apply. The sandbox
charges the session after the run.

## An answer

An answer names its session and its request. Both must match the pending
request, and the answer's time may not be earlier than the session's clock.
An answer with no decision is refused as `answer`, and the request keeps
waiting. A valid answer closes the request. Deny and expire run nothing. An
approval runs the call that was paused, admitted again by chapter4 under
the grant passed in now, and decided again at the new clock. If the grant
no longer names the tool, or the wait used up the session's ticks, the
approved call is refused.

<!-- source: psionic/lib/harness/sandbox.coax answer -->
```jai
// An answer to the pending request. It must name this session and the
// request now pending, and come no earlier than the session's clock, which
// it moves to. Past the deadline, or an expire, it closes the request as
// expired; deny closes it as denied; neither runs anything. Approve runs
// the call, admitted and decided again at the new clock, with the approval
// standing in for the asking rules only: the limits and the ceiling still
// hold. A pattern approval also holds tool and room until at + wait.
sb_answer :: (env:SbTools,s:[]SbSession,a:SbAnswer,world:[]int,l:[]LgLog)->SbEvent {
    e:=SbEvent.{};
    if world.count<HT_CHOICES_MAX {e.why=SbWhy.shape;return e;}
    if !sb_fits(l,SB_ANSWER_SIZE) {e.why=SbWhy.log;return e;}
    x:=s[0];r:=x.request;
    if a.session!=x.id {e.why=SbWhy.session;}
    else if !x.pending || a.request!=r.id {e.why=SbWhy.stale;}
    else if a.decision==SbDecision.none {e.why=SbWhy.answer;}
    else if a.at<x.clock {e.why=SbWhy.order;}
    else {
        s[0].pending=false;s[0].clock=a.at;e.request=r.id;e.room=r.room;e.decision=a.decision;
        if a.decision==SbDecision.expire || a.at>r.deadline {e.decision=SbDecision.expire;e.why=SbWhy.expired;}
        else if a.decision==SbDecision.deny {e.why=SbWhy.denied;}
        else {
            if a.pattern {s[0].patterns[x.held%SB_HELD]=SbPattern.{tool=r.tool,room=r.room,until=a.at+x.policy.wait};s[0].held=x.held+1;}
            text:=cast(string)r.text[0..<r.count];
            c:=ht_admit(env.tools,env.grant,x.usage,text,env.work);e.call=c;
            if c.why!=HtWhy.none {e.why=SbWhy.tool;}
            else {
                d:=sb_decide(x.policy,s[0],env.tools,c.tool,sb_room(env.tools,c),r.count,true);e.verdict=d.verdict;e.why=d.why;
                if d.verdict==SbVerdict.run {e.call=sb_run(env,s,text,world);}
            }
        }
    }
    sb_log_answer(l,a,e);return e;
}
```

An approval is bound to one session and one request, and a request is
closed by its first valid answer, so an approval cannot be used twice or
carried to another session. A pattern lives in the session's own state and
lapses `wait` ticks after it was given.

## Replay

The log holds every input the session received: the policy, each call's
text and each answer. A replay opens a fresh session from the logged
policy, feeds it each logged call and answer, and requires every event the
fresh session logs to have the original's digest. Because each event's
digest covers the previous one, matching digests mean the two logs are the
same event for event. The world the replay leaves is the world the session
left. A log whose chain does not verify, an event that is not one of the
sandbox's, a redacted event, or an outcome the policy would not reach is
refused at the event where it happens.

<!-- source: psionic/lib/harness/sandbox.coax replay -->
```jai
// Replay a session's log: verify the chain, open a fresh session from its
// policy, feed it each logged call and answer, and require each event the
// fresh session logs to have the original's digest. The world the replay
// leaves is the world the session left.
sb_replay :: (env:SbTools,g:LgLog,s:[]SbSession,world:[]int,l:[]LgLog)->SbCheck {
    if lg_verify(g).status!=LgStatus.ok {return SbCheck.{status=SbReplay.log};}
    if l[0].count!=0 {return SbCheck.{status=SbReplay.log};}
    for e,i:g.events[0..<g.count] {
        k:=SbCheck.{status=SbReplay.event,index=i};
        if e.kind!=LgKind.context || e.redacted {return k;}
        t:=sb_tokens(g,i);
        if i==0 {
            if t.count!=SB_OPEN_SIZE || t[0]!=SB_OPEN || t[2]<0 || t[2]>2 || t[4]<0 || t[4]>2 {return k;}
            p:=SbPolicy.{ceiling=cast(HtEffect)cast(u8)t[2],reach=t[3],allow=cast(HtEffect)cast(u8)t[4],open=t[5],cost=t[6],calls=t[7],bytes=t[8],ticks=t[9],timeout=t[10],wait=t[11]};
            if sb_open(t[1],p,s,l)!=SbWhy.none {return k;}
        }
        else if t.count>=SB_CALL_HEAD && t.count<=SB_CALL_HEAD+HT_TEXT_MAX && t[0]==SB_CALL {
            b:[HT_TEXT_MAX]u8;n:=t.count-SB_CALL_HEAD;
            for x,j:t[SB_CALL_HEAD..<t.count] {if x<0 || x>255 {return k;}b[j]=cast(u8)x;}
            c:=sb_call(env,s,cast(string)b[0..<n],world,l);
        }
        else if t.count==SB_ANSWER_SIZE && t[0]==SB_ANSWER && t[3]>=0 && t[3]<=3 {
            c:=sb_answer(env,s,SbAnswer.{session=t[1],request=t[2],decision=cast(SbDecision)cast(u8)t[3],pattern=t[4]==1,at=t[5]},world,l);
        }
        else {return k;}
        if l[0].count!=i+1 || !su_same(l[0].events[i].digest,e.digest) {return SbCheck.{status=SbReplay.differs,index=i};}
    }
    return SbCheck.{index=g.count};
}
```

## The checks

- **Decisions against a table.** A separate file,
  [harness-sandbox-oracle.coax](../../psionic/tests/harness-sandbox-oracle.coax),
  writes the decision as a table of rules in order. It computes each rule's
  condition without the subject's code: room bits by repeated division,
  subsets room by room, and each tool's effect, cost and ticks from this
  chapter's tables. The first rule that holds decides. One `#test` compares
  every ceiling, `allow`, `reach`, `open`, room and tool: 27,648 cases. A
  `#forall` over 4,000 seeds compares random cases near every limit,
  including invalid policies, approvals and held patterns.
- **The hand trace**, every step's verdict, reason, decision, value,
  request, room, clock, calls and bytes, the event tokens above, and the
  replay's world and head digest.
- **Approvals.** Two sessions ask the same request id: session 1's approval
  is refused in session 2, then runs once in session 1 and is refused when
  sent again. A pattern holds in its session and not the other. A request
  answered at its deadline runs, one tick later it expires, and the late
  approval cannot be sent again. A denied pattern holds nothing. An
  approval that would pass the session's ticks, and an approval after the
  grant dropped the tool, are refused with nothing charged and the world
  unchanged. Five patterns replace the oldest.
- **Before the session.** Text of 257 bytes, a world of 7 rooms, and logs
  with one event or one token too few are refused and not logged; a log
  with exactly enough room takes the event. A log with an open chapter1
  turn takes nothing.
- **Replay refusals.** Thirty-two forged logs, each rechained so the hash chain
  verifies, each change one token of one event, drop tokens, or change its
  kind. Each is refused at the event it changed, as `event` or `differs`,
  or replays when the change does not alter what the session would log.
  An edit without rechaining, a redacted event, and a replay into a world
  or log too small are refused too.
- **Random sessions.** A `#forall` over 12 seeds draws a valid policy and
  16 steps of calls and answers. After every refused, paused or asked step
  the world, the calls, the bytes and chapter4's cost are unchanged.
  Every run is within the ceiling and the limits. An approval that ran is
  refused when sent again and when sent to another session, and the world
  does not change. The log replays to the same world and clock.
- **Mutation.** `coax mutate --root-only` leaves no survivor in
  `harness/sandbox.coax`.

## Run it

The example, [sandbox-approvals.coax](../../psionic/examples/sandbox-approvals.coax),
runs the hand trace's session step by step. It then verifies and replays
the log, sends session 1's approval to a second session and sends a used
approval again. Last, it shows two broken inputs: a policy whose `open`
mask is outside its `reach`, and a log whose denial was rewritten and
rechained.

<!-- source: psionic/examples/sandbox-approvals.coax example -->
```jai
main :: () {
    rm:=HT_ROOMS;fs:[5]HtField;r:=ht_registry(rm[..],fs[..]);e:[256]NxEdge;o:[128]int;n:[32768]int;a:[128]u8;lv:[128]u8;
    g:=[4]int.[0,1,2,3];c:=[4]int.[9,9,9,9];env:=sb_tools(r[..],g[..],c[..],99,ht_work(e[..],o[..],n[..],a[..],lv[..]));
    p:=SB_POLICY;
    print("policy: ceiling %, reach %, allow %, open %, cost %, calls %, bytes %, ticks %, timeout %, wait %\n",
        p.ceiling,p.reach,p.allow,p.open,p.cost,p.calls,p.bytes,p.ticks,p.timeout,p.wait);
    b:[1]SbLogs;l:=sb_logs(b[..]);s:[1]SbSession;w:[8]int;
    print("open session 1: %\n",sb_open(1,p,s[..],l[..]));
    for k:0..<19 {
        if SB_TRACE_TEXT[k].count>0 {print("% call %: ",k,SB_TRACE_TEXT[k]);}
        else {x:=SB_TRACE_ANSWER[k];print("% answer session %, request %, % %at %: ",k,x[0],x[1],cast(SbDecision)cast(u8)x[2],ifx x[3]==1 then "pattern " else "",x[4]);}
        sx_event(sb_step(env,s[..],k,w[..],l[..]));
        print("  [clock %, calls %, bytes %]\n",s[0].clock,s[0].calls,s[0].bytes);
    }
    print("world: hall %, den %, study %; requests %, patterns held %\n",w[0],w[1],w[2],s[0].asked,s[0].held);
    v:=lg_verify(l[0]);print("log: % events, % tokens, verify %\n",l[0].count,l[0].used,v.status);
    // Replay: the logged calls and answers, fed to a fresh session.
    b2:[1]SbLogs;l2:=sb_logs(b2[..]);s2:[1]SbSession;w2:[8]int;k:=sb_replay(env,l[0],s2[..],w2[..],l2[..]);
    print("replay: % through event %, same head %, world hall %, den %, study %, clock %\n",k.status,k.index,su_same(lg_verify(l2[0]).head,v.head),w2[0],w2[1],w2[2],s2[0].clock);
    // Approvals do not travel: session 2 asks the same thing, and session 1's answer is refused there.
    b3:[1]SbLogs;l3:=sb_logs(b3[..]);s3:[1]SbSession;w3:[8]int;x:=sb_open(2,p,s3[..],l3[..]);
    print("session 2: ");sx_event(sb_call(env,s3[..],SB_SET_DEN,w3[..],l3[..]));print("\n");
    print("session 1's answer to request 0, sent to session 2: ");sx_event(sb_answer(env,s3[..],sb_yes(1,0,1),w3[..],l3[..]));print("; den %\n",w3[1]);
    print("session 1's approval of request 3, sent again: ");sx_event(sb_answer(env,s[..],sb_yes(1,3,20),w[..],l[..]));print("; den %\n",w[1]);
    // Broken inputs: a policy that runs freely where it may never reach, and a forged log.
    bad:=p;bad.open=4;b4:[1]SbLogs;l4:=sb_logs(b4[..]);s4:[1]SbSession;
    print("a policy with open % outside reach %: %, % events logged\n",bad.open,bad.reach,sb_open(4,bad,s4[..],l4[..]),l4[0].count);
    f:[1]SbLogs;lf:=sb_logs(f[..]);sb_forge(l2[0],6,7,0,-1,LgKind.context,lf[..]);
    b5:[1]SbLogs;l5:=sb_logs(b5[..]);s5:[1]SbSession;w5:[8]int;k=sb_replay(env,lf[0],s5[..],w5[..],l5[..]);
    print("event 6's reason rewritten from denied to none, rechained: verify %, replay % at event %\n",lg_verify(lf[0]).status,k.status,k.index);
}
```

```shell
coax/bin/coax run psionic/examples/sandbox-approvals.coax
psionic/check.sh --only harness/sandbox
```

```text
policy: ceiling write, reach 3, allow read, open 1, cost 1, calls 8, bytes 200, ticks 30, timeout 3, wait 5
open session 1: none
0 call get{"room":"hall"}: run none, result 0 (none)  [clock 1, calls 1, bytes 18]
1 call get{"room":"den"}: ask scope, request 0  [clock 1, calls 1, bytes 18]
2 call add{"a":1,"b":2}: refuse paused  [clock 1, calls 1, bytes 18]
3 answer session 1, request 0, approve at 2: run none, decision approve, request 0, result 0 (none)  [clock 3, calls 2, bytes 35]
4 call set{"room":"den","on":true}: ask effect, request 1  [clock 3, calls 2, bytes 35]
5 answer session 1, request 1, deny at 4: refuse denied, decision deny, request 1  [clock 4, calls 2, bytes 35]
6 call set{"room":"den","on":true}: ask effect, request 2  [clock 4, calls 2, bytes 35]
7 answer session 1, request 2, approve pattern at 10: refuse expired, decision expire, request 2  [clock 10, calls 2, bytes 35]
8 call set{"room":"den","on":true}: ask effect, request 3  [clock 10, calls 2, bytes 35]
9 answer session 1, request 3, approve pattern at 11: run none, decision approve, request 3, result 1 (none)  [clock 14, calls 3, bytes 62]
10 call set{"room":"den","on":true}: run none, result 1 (none)  [clock 17, calls 4, bytes 89]
11 call set{"room":"den","on":false}: ask effect, request 4  [clock 17, calls 4, bytes 89]
12 answer session 2, request 4, approve at 17: refuse session  [clock 17, calls 4, bytes 89]
13 answer session 1, request 3, approve at 18: refuse stale  [clock 17, calls 4, bytes 89]
14 answer session 1, request 4, approve at 16: refuse order  [clock 17, calls 4, bytes 89]
15 answer session 1, request 4, expire at 18: refuse expired, decision expire, request 4  [clock 18, calls 4, bytes 89]
16 call set{"room":"study","on":true}: refuse scope  [clock 18, calls 4, bytes 89]
17 call div{"a":9,"b":0}: run none, result 0 (run)  [clock 20, calls 5, bytes 105]
18 answer session 1, request 4, approve at 19: refuse stale  [clock 20, calls 5, bytes 105]
world: hall 0, den 1, study 0; requests 5, patterns held 1
log: 20 events, 433 tokens, verify ok
replay: ok through event 20, same head true, world hall 0, den 1, study 0, clock 20
session 2: ask effect, request 0
session 1's answer to request 0, sent to session 2: refuse session; den 0
session 1's approval of request 3, sent again: refuse stale; den 1
a policy with open 4 outside reach 3: policy, 0 events logged
event 6's reason rewritten from denied to none, rechained: verify ok, replay differs at event 6
```

The forged log verifies, because its chain was rebuilt. Replay still
refuses it: the session, fed the same inputs, logs `denied` at event 6, and
that event's digest differs from the forged one.

## What remains

- Real processes and real files. Declared roots on disk, memory and
  process-count limits, and stopping a process that overruns a limit need
  operating-system confinement. Here the clock knows each body's ticks in
  advance, so the sandbox refuses before running instead of stopping a run.
- The repeated-failure guard: stopping the same failing call repeated with
  the same arguments.
- Approvals from a person through a client, and a timer that expires a
  request without being asked.
- Policies, requests and answers as versioned messages in `proto/`.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- [Chapter4](ch004-tools-as-typed-capabilities.md): the tool pipeline
  that the sandbox wraps.
- [Chapter2](ch002-model-visible-means-logged.md): the log that records
  policies, requests and answers.
