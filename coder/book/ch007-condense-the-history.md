# 7. Condense the history

> **Status: partly implemented.** Condensation as a logged event, run on
> the CPU over chapter2's log and chapter3's requests (profile
> cd-condense-v1): a policy that decides when and what to condense while
> keeping the frozen prefix, pinned facts and the last turns; a begin event
> that names each replaced event by digest and a summary from a
> deterministic stand-in summarizer; a verifier that rebuilds every request
> from the log and admits a broken prefix only where a logged condensation
> removed history; and three schedules measured on Psionic chapter 176's cache with
> a hand-worked break-even. Still to come: a model as the summarizer, task
> success on held-out long sessions with and without condensation,
> rule-based pruning of tool results as a logged event, and time to first
> token measured on a named host rather than modeled. Microcoder already
> prunes by rule (see [In Coder](#in-coder)).

The [coverage matrix](CONTRACT.md#coverage-by-execution-path) separates the
logged experimental view below, Microcoder's currently unlogged state
pruning, and compaction owned by Claude Code or Codex.

A long session fills the model's context. When the next request would not
fit, the harness has to send less: it replaces older history with a short
summary. Doing that quietly would break two promises the earlier chapters
made. [Chapter2](ch002-model-visible-means-logged.md) promised that
every token the model sees comes from the log. [Chapter3](ch003-keep-the-prefix-stable.md)
promised that each request starts with every token the previous request
held before its cache boundary, and its checker refuses any request that
does not.

This chapter keeps both promises by making condensation an event. The
harness writes two events to the log before it sends a condensed request:
a begin event that names, by index and digest, exactly which events the
summary replaces, and then the summary itself. The model's view of the
history is computed from the log alone, so a verifier can rebuild every
request. A request may break the stable prefix only at the first event
that a logged condensation removed; any other change is refused at its
position.

Everything here lives in one `#authority` file,
[harness/condense.coax](../../psionic/lib/harness/condense.coax), because it decides
what a request may hold and which requests are admitted. Its checks leave
no mutant alive.

## The profile

This chapter's profile is `cd-condense-v1`:

- Requests are chapter3's (`px-prefix-v1`): integer tokens, at most 16
  sections and 64 tokens.
- The log is chapter2's, holding text events only: system, user and
  context. It holds at most 32 events, and the model's view of it at most
  64 tokens.
- One condensation names at most 8 events. Each named event's 32-byte
  digest is written as eight tokens of four bytes each, low byte first.
- The summarizer is a deterministic stand-in, not a model (see below).
- The cache measurement is chapter3's `px_measure`: Psionic chapter 176's cache
  with pages of 4 positions, and times from Psionic chapter 177's cost model in
  modeled microseconds.

## The view and the two events

The history a request holds is the **view**: the log's text events in log
order, with condensations applied. A condensation is two context events
whose first token is a mark that tokenizer output never holds:

| Event | Tokens |
| --- | --- |
| begin | `CD_BEGIN` (−206), the count `k`, then for each named event its index and its event digest as 8 words |
| summary | `CD_SUMMARY` (−207), then the rest of the summarizer's output |

A begin event of `k` names is `2 + 9k` tokens long. Applying it removes the
named events from the view and puts the summary event in the place of the
first one. The named events must be in the view, in view order, and named
with their own digests; none may be a pinned fact or redacted; and the
event right after the begin event must be exactly the summarizer's output
for them. A summary event anywhere else is refused, so a summary cannot be
slipped into the log on its own.

A **pinned fact** is a system event in the history: text the harness must
keep for the whole session, such as a constraint the user stated. The
policy never names one, and the verifier refuses a condensation that does.

The summarizer is a stand-in so that its output can be checked. It
returns the mark, the number of events named, the number of tokens they
held, and then the first token of each named user event, in order (an
empty user event adds nothing). It does not try to say what the history
meant. What matters here is that it is a function of the named events'
tokens alone, so the verifier recomputes it and refuses a summary that
differs. A model summarizer would have to be logged the same way, with
its output as the summary event; checking it would then mean replaying
the model, as chapter2's replay does for turns.

<!-- source: psionic/lib/harness/condense.coax types -->
```jai
CdStatus :: enum u8 {ok;shape;log;kind;names;pinned;open;summary;budget;source;prefix;}
// The condensed view of a log: the events the model sees, in order, and
// their tokens. gone[e] is the begin event of the condensation that removed
// event e, or 0 while e is in the view: a begin event names earlier events,
// so it is never event 0. A refusal names the event where it happened.
CdView :: struct {status:CdStatus;event:int;items:int;item:[32]int;gone:[32]int;n:int;tok:[64]int;}
// When to condense: the request's window in tokens (frozen prefix, view and
// tail), and the user turns kept whole at the end of the view.
CdPolicy :: struct {window:int;keep:int;}
// A plan: the request's length now and after, and the events to name.
CdPlan :: struct {status:CdStatus;condense:bool;length:int;after:int;names:int;name:[8]int;}
// The verifier's answer: the request, the log event or token position of a
// refusal, the tokens shared with the previous request, and for an admitted
// break the begin event that allowed it and the breaks counted so far.
CdVerdict :: struct {status:CdStatus;request:int;event:int;at:int;shared:int;by:int;breaks:int;}
```

`cd_view` verifies the log with chapter2's `lg_verify`, walks it once
and applies each begin event with `cd_apply`. A begin event as the last
event of the log means the harness stopped between the two events, for
example in a crash: the log still verifies and opens, and the view reports
the open condensation at the begin event's index instead of guessing.

<!-- source: psionic/lib/harness/condense.coax view -->
```jai
// Event e's digest as eight words of four little-endian bytes each.
cd_word :: (d:[32]u8,w:int)->int {
    x:=0;for b:0..<4 {x=256*x+cast(int)d[4*w+3-b];}
    return x;
}
// The stand-in summarizer: the mark, how many events and tokens it
// replaces, then the first token of each named user event, in order. It is
// not a model; it is deterministic, short, and a function of the named
// events' tokens only, so a verifier can recompute it.
cd_summarize :: (g:LgLog,names:[]int,out:[]int)->int {
    out[0]=CD_SUMMARY;out[1]=names.count;out[2]=0;n:=3;
    for x:names {
        at:=lg_offset(g,x);e:=g.events[x];out[2]+=e.count;
        if e.kind==LgKind.user && e.count>0 {out[n]=g.tokens[at];n+=1;}
    }
    return n;
}
cd_refuse :: (v:CdView,status:CdStatus,event:int)->CdView {r:=v;r.status=status;r.event=event;return r;}
// The view position of event x, or the view's length.
cd_find :: (v:CdView,x:int)->int {
    item:=v.item;for y,p:item[0..<v.items] {if y==x {return p;}}
    return v.items;
}
// The begin event at i and the summary after it, applied to the view. Each
// named event must be in the view, after the one named before it, not a
// pinned fact, unredacted, and named with its own digest; the next event
// must be the summarizer's output for them. The summary takes the first
// named event's place, and the others leave the view.
cd_apply :: (from:CdView,g:LgLog,i:int,start:int)->CdView {
    v:=from;at:=start;e:=g.events[i];k:=e.count/9;
    if (e.count-2)%9!=0 || k<1 || k>CD_NAMES || g.tokens[at+1]!=k {return cd_refuse(v,CdStatus.names,i);}
    names:[8]int;next:=0;
    for j:0..<k {
        x:=g.tokens[at+2+9*j];q:=cd_find(v,x);
        if q==v.items || q<next {return cd_refuse(v,CdStatus.names,i);}
        if g.events[x].kind==LgKind.system {return cd_refuse(v,CdStatus.pinned,i);}
        for w:0..<8 {if g.tokens[at+3+9*j+w]!=cd_word(g.events[x].digest,w) {return cd_refuse(v,CdStatus.names,i);}}
        if g.events[x].redacted {return cd_refuse(v,CdStatus.log,i);}
        names[j]=x;next=q+1;
    }
    if i+1==g.count {return cd_refuse(v,CdStatus.open,i);}
    s:[11]int;n:=cd_summarize(g,names[0..<k],s[..]);f:=g.events[i+1];at+=e.count;
    if f.kind!=LgKind.context || f.count!=n {return cd_refuse(v,CdStatus.summary,i+1);}
    for t:0..<n {if g.tokens[at+t]!=s[t] {return cd_refuse(v,CdStatus.summary,i+1);}}
    for x:names[0..<k] {v.gone[x]=i;}
    m:=0;
    item:=from.item;
    for x:item[0..<from.items] {
        if x==names[0] {v.item[m]=i+1;m+=1;}
        else if v.gone[x]==0 {v.item[m]=x;m+=1;}
    }
    v.items=m;return v;
}
// The condensed view of a verified log of text events. A context event that
// starts with CD_BEGIN is a condensation; one that starts with CD_SUMMARY
// must follow a begin event.
cd_view :: (g:LgLog)->CdView {
    v:CdView;
    if g.count>CD_EVENTS {return cd_refuse(v,CdStatus.shape,g.count);}
    c:=lg_verify(g);if c.status!=LgStatus.ok {return cd_refuse(v,CdStatus.log,c.index);}
    i:=0;at:=0;
    while i<g.count {
        e:=g.events[i];
        if !lg_text(e.kind) {return cd_refuse(v,CdStatus.kind,i);}
        marked:=e.kind==LgKind.context && e.count>0;
        if marked && g.tokens[at]==CD_SUMMARY {return cd_refuse(v,CdStatus.summary,i);}
        if marked && g.tokens[at]==CD_BEGIN {
            v=cd_apply(v,g,i,at);if v.status!=CdStatus.ok {return v;}
            at+=e.count+g.events[i+1].count;i+=2;
        }
        else {v.item[v.items]=i;v.items+=1;at+=e.count;i+=1;}
    }
    for x:v.item[0..<v.items] {
        e:=g.events[x];
        if e.redacted {return cd_refuse(v,CdStatus.log,x);}
        if e.count>PX_TOKENS-v.n {return cd_refuse(v,CdStatus.shape,x);}
        at=lg_offset(g,x);for t:0..<e.count {v.tok[v.n+t]=g.tokens[at+t];}
        v.n+=e.count;
    }
    return v;
}
// The log as it stood after its first n events.
cd_at :: (g:LgLog,n:int)->LgLog {return LgLog.{events=g.events,tokens=g.tokens,count=n,used=lg_offset(g,n)};}
```

## When and what to condense

The policy has two numbers: the window, the most tokens a request may
hold (frozen prefix, view and tail), and `keep`, how many user turns at
the end of the view stay whole. `cd_plan` does nothing while the next
request fits. When it would not, it finds the cut, the place in the view
of the `keep`-th user event from the end, and names the events before the
cut, oldest first, up to eight, skipping pinned facts. The system text and
tool schemas are not in the view at all; they are chapter3's frozen
prefix, so no plan can touch them. If the condensed request would still
not fit, or nothing can be named, the plan is refused with `budget` and
nothing is logged: the harness does not drop pinned facts or recent turns
to make room.

`cd_condense` logs the begin event and then the summary. `cd_begin` and
`cd_end` are separate so a check can stop between them; if the log fills
after the begin event, it is left open exactly as after a crash.

<!-- source: psionic/lib/harness/condense.coax condense -->
```jai
// Which events to name, under the policy, for a request with a tail of
// tail tokens. Nothing is named while the request fits the window. Then the
// cut is the view position of the keep-th user event from the end; the
// events before it, oldest first, up to eight, except pinned facts, are
// named. A plan that would still not fit is refused, and nothing is logged.
cd_plan :: (s:PxRequest,g:LgLog,tail:int,p:CdPolicy)->CdPlan {
    r:CdPlan;r.status=CdStatus.shape;
    if s.status!=PxStatus.ok || p.window<1 || p.keep<0 || tail<0 {return r;}
    v:=cd_view(g);if v.status!=CdStatus.ok {r.status=v.status;return r;}
    r.status=CdStatus.ok;r.length=s.n+v.n+tail;r.after=r.length;
    if r.length<=p.window {return r;}
    cut:=v.items;left:=p.keep;
    while left>0 && cut>0 {cut-=1;if g.events[v.item[cut]].kind==LgKind.user {left-=1;}}
    for x:v.item[0..<cut] {
        if g.events[x].kind!=LgKind.system && r.names<CD_NAMES {r.name[r.names]=x;r.names+=1;r.after-=g.events[x].count;}
    }
    out:[11]int;r.after+=cd_summarize(g,r.name[0..<r.names],out[..]);
    // With nothing named, after is longer than now, so the plan is refused.
    if r.after>p.window {r.status=CdStatus.budget;return r;}
    r.condense=true;return r;
}
// One to eight events of the log, by index.
cd_named :: (g:LgLog,names:[]int)->bool {
    if names.count<1 || names.count>CD_NAMES {return false;}
    for x:names {if x<0 || x>=g.count {return false;}}
    return true;
}
// The begin event: the mark, the count, then each named event's index and
// digest words.
cd_begin :: (l:[]LgLog,names:[]int)->CdStatus {
    g:=l[0];b:[74]int;b[0]=CD_BEGIN;b[1]=names.count;
    if !cd_named(g,names) {return CdStatus.shape;}
    for x,j:names {b[2+9*j]=x;for w:0..<8 {b[3+9*j+w]=cd_word(g.events[x].digest,w);}}
    if lg_say(l,LgKind.context,b[0..<2+9*names.count])!=LgStatus.ok {return CdStatus.shape;}
    return CdStatus.ok;
}
// The summary event for the same names, right after the begin event. If the
// log is full here, it holds an open condensation, as after a crash; the
// verifier refuses a summary that does not match its begin event.
cd_end :: (l:[]LgLog,names:[]int)->CdStatus {
    g:=l[0];s:[11]int;
    if !cd_named(g,names) {return CdStatus.shape;}
    if lg_say(l,LgKind.context,s[0..<cd_summarize(g,names,s[..])])!=LgStatus.ok {return CdStatus.shape;}
    return CdStatus.ok;
}
// Plan, and when the plan says so, log both events.
cd_condense :: (l:[]LgLog,s:PxRequest,tail:int,p:CdPolicy)->CdPlan {
    r:=cd_plan(s,l[0],tail,p);
    if r.condense {
        r.status=cd_begin(l,r.name[0..<r.names]);
        if r.status==CdStatus.ok {r.status=cd_end(l,r.name[0..<r.names]);}
    }
    return r;
}
```

`cd_request` is what the harness runs before each request: condense if
the policy says so, then assemble chapter3's request with the view as
its history section.

<!-- source: psionic/lib/harness/condense.coax assemble -->
```jai
// A request from the condensed view: chapter 202's frozen prefix, the view
// as its history section, then the tail.
cd_history :: (s:PxRequest,v:CdView)->PxRequest {
    r:=s;
    if r.status!=PxStatus.ok {return r;}
    if v.status!=CdStatus.ok {r.status=PxStatus.log;return r;}
    tok:=v.tok;return px_add(r,PxKind.history,0,tok[0..<v.n]);
}
cd_build :: (s:PxRequest,v:CdView,tail:[]int)->PxRequest {return px_add(cd_history(s,v),PxKind.tail,0,tail);}
cd_assemble :: (s:PxRequest,g:LgLog,tail:[]int)->PxRequest {return cd_build(s,cd_view(g),tail);}
// What the harness does before each request: condense if the policy says
// so, then assemble from the log.
cd_request :: (l:[]LgLog,s:PxRequest,tail:[]int,p:CdPolicy)->CdStep {
    r:CdStep;r.plan=cd_condense(l,s,tail.count,p);r.at=l[0].count;
    r.r.status=PxStatus.log;
    if r.plan.status==CdStatus.ok {r.r=cd_assemble(s,l[0],tail);}
    return r;
}
```

## The verifier and the exception

`cd_check` rebuilds a request from the log as it stood when the request
was assembled: the frozen prefix, the view, and the request's own tail.
Every token must match; a refusal names the first position that differs.
So every model-visible token is either a logged event's or a logged
summary's, and the summary is itself checked against the events it names.

`cd_admit` is chapter3's `px_admit` with one exception. It first checks
the next request with `cd_check`. If the next request keeps every token
of the previous one before its boundary, it is admitted as before. If not,
the first difference must be at or after the start of the first event of
the previous request's view that a logged condensation has since removed;
`by` names that condensation's begin event. A previous request that the
log does not explain breaks earlier, and is refused with `prefix` at the
difference. `cd_session` checks a whole session this way, which is also
its replay: every request is rebuilt from the log alone.

<!-- source: psionic/lib/harness/condense.coax verify -->
```jai
// Every token of r must be the frozen prefix, the view of the log's first
// at events, and r's own tail, in that order; a refusal names the first
// position that differs.
cd_match :: (s:PxRequest,v:CdView,r:PxRequest)->CdVerdict {
    if r.status!=PxStatus.ok {return CdVerdict.{status=CdStatus.shape};}
    tok:=r.tok;w:=cd_build(s,v,tok[px_boundary(r)..<r.n]);
    if w.status!=PxStatus.ok {return CdVerdict.{status=CdStatus.shape};}
    p:=0;while p<w.n && p<r.n && w.tok[p]==r.tok[p] {p+=1;}
    if p<w.n || p<r.n {return CdVerdict.{status=CdStatus.source,at=p};}
    return CdVerdict.{};
}
cd_check :: (s:PxRequest,g:LgLog,at:int,r:PxRequest)->CdVerdict {
    if at<0 || at>g.count {return CdVerdict.{status=CdStatus.shape};}
    v:=cd_view(cd_at(g,at));
    if v.status!=CdStatus.ok {return CdVerdict.{status=v.status,event=v.event};}
    return cd_match(s,v,r);
}
// The next request after prev, each with the log length it was assembled
// at. next must be what the log explains. If it does not start with every
// token of prev before prev's boundary, the first difference must be at or
// after the first event of prev's view that a logged condensation has since
// removed; by names that condensation's begin event.
cd_admit :: (s:PxRequest,g:LgLog,pat:int,prev:PxRequest,nat:int,next:PxRequest)->CdVerdict {
    if pat<0 || pat>nat || nat>g.count {return CdVerdict.{status=CdStatus.shape};}
    b:=cd_view(cd_at(g,nat));a:=cd_view(cd_at(g,pat));
    if b.status!=CdStatus.ok {return CdVerdict.{status=b.status,event=b.event};}
    c:=cd_match(s,b,next);if c.status!=CdStatus.ok {return c;}
    if a.status!=CdStatus.ok {return CdVerdict.{status=a.status,event=a.event};}
    v:=px_admit(prev,next);c.shared=v.shared;c.at=v.at;
    if v.status==PxStatus.ok {return c;}
    p:=s.n;
    for x:a.item[0..<a.items] {
        if b.gone[x]!=0 {
            if v.at<p {break;}
            c.by=b.gone[x];c.breaks=1;return c;
        }
        p+=g.events[x].count;
    }
    c.status=CdStatus.prefix;return c;
}
// A session's requests and the log lengths they were assembled at: the
// first refusal, with the request's index, or the last verdict with the
// number of admitted breaks. Every request is rebuilt from the log.
cd_session :: (s:PxRequest,g:LgLog,at:[]int,reqs:[]PxRequest)->CdVerdict {
    if reqs.count<1 || at.count!=reqs.count {return CdVerdict.{status=CdStatus.shape};}
    c:=cd_check(s,g,at[0],reqs[0]);n:=0;
    for i:1..<reqs.count {
        if c.status!=CdStatus.ok {return c;}
        c=cd_admit(s,g,at[i-1],reqs[i-1],at[i],reqs[i]);c.request=i;n+=c.breaks;c.breaks=n;
    }
    return c;
}
```

## A session by hand

The hand session has a frozen prefix of 4 tokens (system text `[1, 2]` and
one tool, id 7, with schema `[7, 8]`), then a pinned fact `[90, 91]` as
event 0. Each turn `t` logs the user's `[10t, 10t+1]`, sends a request with
the one-token tail `[1000 + t]`, then logs the reply as context
`[10t+2, 10t+3]`. The window is 20 tokens and `keep` is 1.

A request at turn `t` without condensation holds 4 + 2 + 4(t − 1) + 2 + 1
tokens: 9, 13, 17, then 21 at turn 4, which does not fit.

At turn 4 the view is the pinned fact (event 0), turns 1 to 3 (events 1 to
6) and turn 4's user event (event 7). The cut is at event 7, so events 1
to 6 are named: 12 tokens. The summary is the mark, 6 events, 12 tokens,
and the first tokens of the three user events, 10, 20 and 30: 6 tokens.
The request is 21 − 12 + 6 = 15 tokens. The begin event is event 8
(2 + 9 × 6 = 56 tokens), the summary event 9, and the view is events
0, 9, 7:

    90 91 | -207 6 12 10 20 30 | 40 41

Turn 5 adds turn 4's reply (event 10) and turn 5's user event (event 11):
15 + 4 = 19 tokens, which fits. Turn 6 adds two more events: 23 tokens.
The view is events 0, 9, 7, 10, 11, 12, 13, the cut is at event 13 (turn
6's user event), so the first summary (event 9) and turn 4 and 5's events
7, 10, 11 and 12 are named: 6 + 8 = 14 tokens. Note that the names are in
view order, not index order: event 9 comes before event 7. The new
summary is the mark, 5 events, 14 tokens, and the user events' first
tokens 40 and 50. The request is 23 − 14 + 5 = 14 tokens:

    90 91 | -207 5 14 40 50 | 60 61

Now the prefix. Request 3 is `1 2 7 8 90 91 10 11 …`; request 4 is
`1 2 7 8 90 91 -207 …`. They differ at position 6, which is exactly where
event 1, the first event the condensation at event 8 removed, started in
request 3 (4 tokens of prefix and 2 of the pinned fact before it). Chapter
202's `px_admit` refuses this; `cd_admit` admits it, with `by` = 8. At turn
6 the first event removed is the first summary, also at position 6, but
both summaries start with the mark −207, so the requests differ only at
position 7 (6 against 5). That is still at or after 6, so the break is
admitted, with `by` = 14. Every other pair shares all but the previous
request's one-token tail.

<!-- source: psionic/lib/harness/condense.coax check-hand -->
```jai
#test "the hand session: two condensations, by hand" #steps 4000000000 {
    ev:[32]LgEvent;tk:[512]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];
    q:[6]PxRequest;at:[6]int;p:[6]CdPlan;s:=cdc_open();
    #assert cdc_hand(l[..],CdPolicy.{window=20,keep=1},6,q[..],at[..],p[..])==6;
    // 9, 13 and 17 tokens fit the window of 20; 21 at turn 4 and 23 at turn 6 do not.
    before:=[6]int.[9,13,17,21,19,23];n:=[6]int.[9,13,17,15,19,14];
    for k:0..<6 {#assert p[k].status==CdStatus.ok && p[k].length==before[k] && p[k].after==n[k] && q[k].n==n[k] && p[k].condense==(k==3 || k==5);}
    // Turn 4: turns 1 to 3, events 1 to 6, go. Turn 6: the first summary (event 9) and turns 4 and 5 go.
    first:=[6]int.[1,2,3,4,5,6];second:=[5]int.[9,7,10,11,12];
    #assert p[3].names==6 && p[5].names==5;
    for j:0..<6 {#assert p[3].name[j]==first[j];}
    for j:0..<5 {#assert p[5].name[j]==second[j];}
    // The first summary: the mark, 6 events, 12 tokens, and turns 1 to 3's first tokens.
    v:=cd_view(cd_at(l[0],at[3]));
    #assert v.status==CdStatus.ok && v.items==3 && v.item[0]==0 && v.item[1]==9 && v.item[2]==7 && v.n==10;
    want:=[10]int.[90,91,CD_SUMMARY,6,12,10,20,30,40,41];for i:0..<10 {#assert v.tok[i]==want[i];}
    for e:1..<7 {#assert v.gone[e]==8;}
    #assert v.gone[0]==0 && v.gone[7]==0;
    v=cd_view(cd_at(l[0],at[5]));
    #assert v.status==CdStatus.ok && v.items==3 && v.item[1]==15 && v.item[2]==13 && v.n==9 && v.gone[9]==14 && v.gone[7]==14 && v.gone[1]==8;
    want2:=[9]int.[90,91,CD_SUMMARY,5,14,40,50,60,61];for i:0..<9 {#assert v.tok[i]==want2[i];}
    // The first condensation breaks the prefix at position 6, just after the pinned fact; the second
    // at 7, since both summaries start with the mark. Each break is admitted.
    for k:1..<6 {
        a:=cd_admit(s,l[0],at[k-1],q[k-1],at[k],q[k]);x:=px_admit(q[k-1],q[k]);
        if k==3 || k==5 {#assert a.status==CdStatus.ok && a.at==(ifx k==3 then 6 else 7) && a.by==(ifx k==3 then 8 else 14) && a.breaks==1 && x.status==PxStatus.prefix && x.at==a.at;}
        else {#assert a.status==CdStatus.ok && a.by==0 && a.breaks==0 && x.status==PxStatus.ok && a.shared==q[k-1].n-1;}
    }
    c:=cd_session(s,l[0],at[..],q[..]);#assert c.status==CdStatus.ok && c.request==5 && c.breaks==2;
}
```

Then the broken inputs. A second log holds the same three turns and turn
4's user event, but no condensation:

- A request that drops turns 1 to 3 anyway, `[90, 91, 40, 41]` after the
  prefix. The view still holds them, so `cd_check` refuses it with
  `source` at position 6, where turn 1 starts.
- A crash after the begin event. The log verifies, but the view is
  refused with `open` at event 8, and no request can be assembled. Once the
  summary is logged, the condensed request of 15 tokens is admitted, with
  the break at 6.
- One token of the summary changed in the request: `source` at position 9.
- Forgeries written straight into the log after event 7: a summary with no
  begin event (`summary`, event 8); a begin event that names the pinned
  fact (`pinned`); malformed begin events, with no names, nine names, an
  extra token, a wrong count, a wrong digest word, names out of view
  order, a name repeated, a name not yet logged, or only the mark (each
  `names`); a summary with a token changed, an extra token, no mark, or
  logged as user text (`summary`, event 9); and a redacted event named (`log`).

<!-- source: psionic/lib/harness/condense.coax check-refuse -->
```jai
#test "an unlogged rewrite, a crash between the events, and forged condensations" #steps 4000000000 {
    ev:[32]LgEvent;tk:[512]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];
    q:[6]PxRequest;at:[6]int;p:[6]CdPlan;s:=cdc_open();
    #assert cdc_hand(l[..],CdPolicy.{window=20,keep=1},3,q[..],at[..],p[..])==3;
    u:=[2]int.[40,41];#assert lg_say(l[..],LgKind.user,u[..])==LgStatus.ok;
    // Turns 1 to 3 dropped without a logged condensation: refused at position 6, where turn 1 was.
    h:=[4]int.[90,91,40,41];tail:=[1]int.[1004];x:=px_add(px_add(s,PxKind.history,0,h[..]),PxKind.tail,0,tail[..]);
    a:=cd_admit(s,l[0],at[2],q[2],l[0].count,x);#assert a.status==CdStatus.source && a.at==6;
    // A crash after the begin event: the log still verifies, and the view names the open condensation.
    g:=l[0];names:=[6]int.[1,2,3,4,5,6];#assert cd_begin(l[..],names[..])==CdStatus.ok;
    v:=cd_view(l[0]);#assert lg_verify(l[0]).status==LgStatus.ok && v.status==CdStatus.open && v.event==8;
    #assert cd_assemble(s,l[0],tail[..]).status==PxStatus.log && cd_check(s,l[0],l[0].count,x).status==CdStatus.open;
    // Finishing it admits exactly the condensed request.
    #assert cd_end(l[..],names[..])==CdStatus.ok;y:=cd_assemble(s,l[0],tail[..]);#assert y.status==PxStatus.ok && y.n==15;
    #assert y.sections==5 && y.section[3].kind==PxKind.history && y.section[3].key==0 && y.section[4].kind==PxKind.tail && y.section[4].key==0;
    a=cd_admit(s,l[0],at[2],q[2],l[0].count,y);#assert a.status==CdStatus.ok && a.by==8 && a.at==6 && a.breaks==1;
    // One token of the summary changed in the request: refused at its position.
    z:=y;z.tok[9]+=1;a=cd_admit(s,l[0],at[2],q[2],l[0].count,z);#assert a.status==CdStatus.source && a.at==9;
    // A previous request that the log does not explain is refused where it differs.
    o:=q[2];o.tok[5]=0;a=cd_admit(s,l[0],at[2],o,l[0].count,y);#assert a.status==CdStatus.prefix && a.at==5 && a.shared==5;
    o=q[2];o.tok[0]=0;a=cd_admit(s,l[0],at[2],o,at[2],q[2]);#assert a.status==CdStatus.prefix && a.at==0;
    // Forgeries written straight into the log after event 7, each refused at its event.
    f:=cdc_forge(g,0);#assert f.status==CdStatus.summary && f.event==8;
    f=cdc_forge(g,1);#assert f.status==CdStatus.pinned && f.event==8;
    f=cdc_forge(g,3);#assert f.status==CdStatus.open && f.event==8;
    for k:[8]int.[2,4,5,6,7,13,14,16] {f=cdc_forge(g,k);#assert f.status==CdStatus.names && f.event==8;}
    for k:[4]int.[8,9,10,17] {f=cdc_forge(g,k);#assert f.status==CdStatus.summary && f.event==9;}
    f=cdc_forge(g,11);#assert f.status==CdStatus.log && f.event==8;
    f=cdc_forge(g,12);#assert f.status==CdStatus.names && f.event==8;
    f=cdc_forge(g,15);#assert f.status==CdStatus.ok && f.items==3 && f.item[1]==9 && f.n==10;
}
```

## Measuring the cost

A condensation shortens every later request, but it breaks the cached
prefix once. Three schedules run the same six turns through Psionic chapter 176's
cache with `px_measure`, pages of 4 positions:

| Schedule | Tokens by request | Blocks reused | Prompt tokens | Cached | Recomputed |
| --- | --- | --- | --- | --- | --- |
| never condensed (window 64) | 9 13 17 21 25 29 | 0 2 3 4 5 6 | 114 | 80 | 34 |
| window 20, keep 1 turn | 9 13 17 15 19 14 | 0 2 3 1 3 1 | 87 | 40 | 47 |
| window 20, keep 2 turns | 9 13 17 18 17 17 | 0 2 3 1 1 2 | 91 | 36 | 55 |

The blocks follow from the shared prefixes. Without condensation each
request shares all but its tail with the one before, so request `k`
reuses one more block than request `k − 1`. With `keep` = 1, request 4
shares only positions 0 to 5 with request 3, so only block 0 is reused;
request 5 shares 14 positions with request 4, which cached 3 whole blocks;
request 6 breaks at position 7, inside block 1. With `keep` = 2 the
schedule condenses at turns 4, 5 and 6, in small steps: turns 1 and 2 into
5 tokens, then that summary and turn 3 into 4, then that summary and turn
4 into 4. Each step breaks the prefix again, at position 7 or 8, so it
recomputes the most.

**The break-even, by hand.** Take the condensation at turn 4 with
`keep` = 1. Without it, request 4 would be 21 tokens with 16 cached and 5
recomputed; with it, 15 tokens with 4 cached and 11 recomputed. So it
recomputes `X` = 6 more tokens and caches 12 fewer, and each later request
is `d` = 12 − 6 = 6 tokens shorter. Let a cached prompt token cost `c` and
a recomputed one `u`. The condensation costs 6u − 12c once and saves about
6c on each later request (block alignment moves this by a few tokens), so
it pays for itself after u/c − 2 later requests: at once if tokens are
counted alike (u = c), after 8 more requests if a cached token costs a
tenth of a recomputed one (u = 10c). This session ends two requests later,
and its totals agree: 80c + 34u never condensed against 40c + 47u with
`keep` = 1, which is cheaper only while u ≤ 40c/13, about 3c.

In Psionic chapter 177's time model a step's cost depends only on the rows it
computes, not on how long the context is, so in modeled time a
condensation is only a cost: the condensed schedules have the higher
prefill times below. What they buy is that every request fits the window
of 20, which the session without condensation leaves after turn 3. Large
steps (`keep` = 1) condense twice and recompute 47 tokens; small steps
(`keep` = 2) condense three times and recompute 55.

<!-- source: psionic/lib/harness/condense.coax check-cache -->
```jai
#test "three schedules on chapter 176's cache, block by block" #steps 4000000000 {
    mem:[1728]f64;m:[3]PxMeasure;win:=[3]int.[64,20,20];keep:=[3]int.[1,1,2];n:[18]int;
    for x:0..<3 {
        ev:[32]LgEvent;tk:[512]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];q:[6]PxRequest;at:[6]int;p:[6]CdPlan;
        #assert cdc_hand(l[..],CdPolicy.{window=win[x],keep=keep[x]},6,q[..],at[..],p[..])==6;
        #assert cd_session(cdc_open(),l[0],at[..],q[..]).status==CdStatus.ok;
        m[x]=px_measure(q[..],1,4,mem[..]);#assert m[x].status==PxStatus.ok;
        for i:0..<6 {n[6*x+i]=q[i].n;}
    }
    tokens:=[18]int.[9,13,17,21,25,29, 9,13,17,15,19,14, 9,13,17,18,17,17];
    hit:=[18]int.[0,2,3,4,5,6, 0,2,3,1,3,1, 0,2,3,1,1,2];
    for i:0..<18 {#assert n[i]==tokens[i] && m[i/6].reused[i%6]==hit[i];}
    #assert m[0].tokens==114 && m[0].cached==80 && m[1].tokens==87 && m[1].cached==40 && m[2].tokens==91 && m[2].cached==36;
    // The break-even at turn 4: 6 more tokens recomputed, 12 fewer cached, 6 fewer sent.
    #assert (15-4*m[1].reused[3])-(21-4*m[0].reused[3])==6 && 4*(m[0].reused[3]-m[1].reused[3])==12;
    // In modeled time a condensation only costs: the more it recomputes, the longer the prefill.
    #assert m[0].prefill==147610 && m[1].prefill==166579 && m[2].prefill==178251;
    #assert m[0].p50==23629 && m[1].p50==26547 && m[2].p95==36761;
}
```

## The program

The example, [condense-history.coax](../../psionic/examples/condense-history.coax),
runs the hand session, prints each request and its verdict, the two views
and the session's replay, then each broken input's refusal, and the three
schedules on the cache.

<!-- source: psionic/examples/condense-history.coax example -->
```jai
main :: () {
    // The hand session: a pinned fact, then six turns, a request in each, under a window of 20 tokens.
    ev:[32]LgEvent;tk:[512]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];
    q:[6]PxRequest;at:[6]int;p:[6]CdPlan;s:=cdc_open();
    turns:=cdc_hand(l[..],CdPolicy.{window=20,keep=1},6,q[..],at[..],p[..]);
    print("frozen prefix: % tokens; % turns; log: % events, % tokens\n",s.n,turns,l[0].count,l[0].used);
    for k:0..<6 {
        print("turn %: % tokens",k+1,p[k].length);
        if p[k].condense {print(", condensed % events into % tokens",p[k].names,p[k].after);}
        print("; request % tokens at log length %",q[k].n,at[k]);
        if k>0 {
            a:=cd_admit(s,l[0],at[k-1],q[k-1],at[k],q[k]);
            print("; %, shares % with the previous",a.status,a.shared);
            if a.by>0 {print(", break at % allowed by the condensation at event %",a.at,a.by);}
        }
        print("\n");
    }
    ex_view("view at turn 4",cd_view(cd_at(l[0],at[3])));
    ex_view("view at turn 6",cd_view(cd_at(l[0],at[5])));
    c:=cd_session(s,l[0],at[..],q[..]);print("the session rebuilt from the log: %, % breaks\n",c.status,c.breaks);
    // Broken inputs, each refused at its place.
    e2:[32]LgEvent;t2:[512]int;g:=[1]LgLog.[LgLog.{events=e2[..],tokens=t2[..]}];
    turns=cdc_hand(g[..],CdPolicy.{window=20,keep=1},3,q[..],at[..],p[..]);
    u:=[2]int.[40,41];print("a second log: % turns, then turn 4's user text: %\n",turns,lg_say(g[..],LgKind.user,u[..]));
    h:=[4]int.[90,91,40,41];tail:=[1]int.[1004];x:=px_add(px_add(s,PxKind.history,0,h[..]),PxKind.tail,0,tail[..]);
    a:=cd_admit(s,g[0],at[2],q[2],g[0].count,x);
    print("turns 1 to 3 dropped without a logged condensation: % (%), position %\n",a.status,cd_status_name(a.status),a.at);
    before:=g[0];names:=[6]int.[1,2,3,4,5,6];
    print("begin event: %; crash before the summary: log %; ",cd_begin(g[..],names[..]),lg_verify(g[0]).status);ex_refuse("view",cd_view(g[0]));
    print("summary event: %",cd_end(g[..],names[..]));y:=cd_assemble(s,g[0],tail[..]);a=cd_admit(s,g[0],at[2],q[2],g[0].count,y);
    print("; request % tokens, %, break at % allowed by the condensation at event %\n",y.n,a.status,a.at,a.by);
    labels:=[6]string.["a summary with no begin event","a condensation naming the pinned fact","a begin event with no summary","a wrong digest word","a summary token changed","a redacted event named"];
    cases:=[6]int.[0,1,3,6,8,11];
    for k,i:cases {ex_refuse(labels[i],cdc_forge(before,k));}
    // Three schedules of the same six turns on chapter 176's cache, pages of 4, 1 output token each.
    win:=[3]int.[64,20,20];keep:=[3]int.[1,1,2];sched:=[3]string.["never condensed","window 20, keep 1 turn","window 20, keep 2 turns"];mem:[1728]f64;
    for k:0..<3 {
        e3:[32]LgEvent;t3:[512]int;r:=[1]LgLog.[LgLog.{events=e3[..],tokens=t3[..]}];
        turns=cdc_hand(r[..],CdPolicy.{window=win[k],keep=keep[k]},6,q[..],at[..],p[..]);
        m:=px_measure(q[..],1,4,mem[..]);n:=0;for z:p {if z.condense {n+=1;}}
        print("%: % condensations, verifier %\n  tokens by request:",sched[k],n,cd_session(s,r[0],at[..],q[..]).status);
        for i:0..<6 {print(" %",q[i].n);}
        print("\n  blocks reused:");for i:0..<6 {print(" %",m.reused[i]);}
        print("\n  prompt tokens %, cached %, recomputed %; modeled prefill % us, P50 % us, P95 % us\n",m.tokens,m.cached,m.tokens-m.cached,m.prefill,m.p50,m.p95);
    }
}
```

```shell
coax/bin/coax run psionic/examples/condense-history.coax
```

```text
frozen prefix: 4 tokens; 6 turns; log: 17 events, 140 tokens
turn 1: 9 tokens; request 9 tokens at log length 2
turn 2: 13 tokens; request 13 tokens at log length 4; ok, shares 8 with the previous
turn 3: 17 tokens; request 17 tokens at log length 6; ok, shares 12 with the previous
turn 4: 21 tokens, condensed 6 events into 15 tokens; request 15 tokens at log length 10; ok, shares 6 with the previous, break at 6 allowed by the condensation at event 8
turn 5: 19 tokens; request 19 tokens at log length 12; ok, shares 14 with the previous
turn 6: 23 tokens, condensed 5 events into 14 tokens; request 14 tokens at log length 16; ok, shares 7 with the previous, break at 7 allowed by the condensation at event 14
view at turn 4: events 0 9 7; tokens 90 91 -207 6 12 10 20 30 40 41
view at turn 6: events 0 15 13; tokens 90 91 -207 5 14 40 50 60 61
the session rebuilt from the log: ok, 2 breaks
a second log: 3 turns, then turn 4's user text: ok
turns 1 to 3 dropped without a logged condensation: source (a request the log's condensed view does not explain), position 6
begin event: ok; crash before the summary: log ok; view: open (a condensation begun and not ended), event 8
summary event: ok; request 15 tokens, ok, break at 6 allowed by the condensation at event 8
a summary with no begin event: summary (a summary that is not the summarizer's output for the named events), event 8
a condensation naming the pinned fact: pinned (a condensation names a pinned fact), event 8
a begin event with no summary: open (a condensation begun and not ended), event 8
a wrong digest word: names (a begin event that does not name visible events by digest, in view order), event 8
a summary token changed: summary (a summary that is not the summarizer's output for the named events), event 9
a redacted event named: log (the log does not verify, or a redacted event is in the view or named), event 8
never condensed: 0 condensations, verifier ok
  tokens by request: 9 13 17 21 25 29
  blocks reused: 0 2 3 4 5 6
  prompt tokens 114, cached 80, recomputed 34; modeled prefill 147610 us, P50 23629 us, P95 29465 us
window 20, keep 1 turn: 2 condensations, verifier ok
  tokens by request: 9 13 17 15 19 14
  blocks reused: 0 2 3 1 3 1
  prompt tokens 87, cached 40, recomputed 47; modeled prefill 166579 us, P50 26547 us, P95 32384 us
window 20, keep 2 turns: 3 condensations, verifier ok
  tokens by request: 9 13 17 18 17 17
  blocks reused: 0 2 3 1 1 2
  prompt tokens 91, cached 36, recomputed 55; modeled prefill 178251 us, P50 29465 us, P95 36761 us
```

Every number matches the hand work above.

## What the checks cover

- **The hand session:** each turn's length before and after, which turns
  condense and which events they name, both views token for token, the
  `gone` marks, each break's position and condensation, and the session
  replayed from the log.
- **Broken inputs:** an unlogged rewrite refused at its position, a crash
  between the two events, a changed summary token, a previous request the
  log does not explain, and seventeen forged condensations, each with its
  status and event.
- **Views:** a tampered log, a redacted event in the view or named,
  chapter2's turn events, a name that an earlier condensation removed,
  marks in user text and an empty context event,
  and the bounds: 32 events and 33, 64 tokens and 65.
- **The summarizer and the two events:** empty user events, the digest
  words, one to eight names, names out of range, a full log at either
  event, and a summary whose names differ from its begin event's.
- **Plans:** the window reached exactly before and after condensing, `keep`
  of 0, 1, 2, 4 and 5, the cap of eight names, bad policies and tails, a
  refused prefix or log, and a plan refused or not needed logging nothing.
- **The verifier's bounds:** log lengths out of range or out of order,
  either one inside an open condensation, requests longer or shorter than
  the view, and a session that names the refused request.
- **The cache:** each schedule's reused blocks and totals, as in the table.
- **Every request after a condensation extends the condensed view
  exactly:** random sessions with chapter3's random prefixes, random
  windows and `keep` values, and up to 12 text events of 0 to 3 tokens
  (pinned facts among them), each followed by a request. An oracle keeps
  its own list of what the model sees, with its own policy (which finds the
  cut from a list of user entries rather than walking back) and its own
  summarizer and digest words. Each plan must name the oracle's events; a
  refused plan must log nothing; the begin event must hold exactly the
  oracle's names and digests; each request must be the oracle's prefix,
  view and tail, token for token; every pinned fact must be in the view
  exactly once; a request without a condensation must pass chapter3's
  `px_admit` unchanged, and one after a condensation may break only at or
  after the first named event, admitted with that condensation; one token
  changed before the boundary must be refused at exactly its position; and
  the whole session must replay from the log with the oracle's number of
  breaks.

Mutation of `harness/condense.coax` leaves no survivors.

## In Coder

Microcoder ([chapter 9](ch009-microcoder.md)) doesn't send a growing
transcript. Each step's prompt is built from its state
([coax/lib/microcoder/state.coax](../../coax/lib/microcoder/state.coax)),
which prunes tool results by fixed rules:

- Only the last 12 actions stay in the state (`MC_RECENT`). Older ones
  leave it.
- A command's output keeps its first 6,000 bytes and its last 6,000 bytes
  (`MC_OUTPUT_HEAD`, `MC_OUTPUT_TAIL`). An action's JSON is at most 16,384
  bytes (`MC_SLOT`).

The rules are fixed, so the same steps always give the same state. They
aren't logged as an event, though, so chapter 2's log doesn't say which
bytes a step's model call didn't see. Claude Code and Codex condense their
own transcripts, which Coder doesn't see or control.

## What remains

- A model as the summarizer, logged and replayed like any other model
  call, and task success on held-out long sessions with and without
  condensation.
- Rule-based pruning of tool results (keep the last N results of a tool,
  or truncate a large output to a stated size) as another logged event of
  the same shape. Microcoder prunes by these rules today without logging
  them.
- Time to first token measured on a named host and model rather than
  modeled from Psionic chapter 175's step cost, and sessions longer than chapter
  176's bounds of 16 requests of 32 tokens.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
