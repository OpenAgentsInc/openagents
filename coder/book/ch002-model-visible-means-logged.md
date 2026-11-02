# 2. Model-visible means logged

> **Status: partly implemented.** A session log runs on the CPU over chapter
> 200's loop: an append-only, SHA-256 hash-chained list of typed events, in
> which every token a request holds is an event before the request is sent,
> and the request is read from the log's own token store. A verifier
> rebuilds every turn and step from the events and refuses a log or a
> request they do not explain, redaction keeps the chain, and a rebuilt turn
> replays through chapter1's `hl_replay`. Still to come: the event
> schemas in `proto/`, a byte format on disk with readers kept for older
> versions, and a write that is durable before its effect is reported.
> Coder's session logs recover from a cut at any byte offset, and each line
> is on storage before it is printed (see [In Coder](#in-coder)); this
> chapter's hash-chained log does neither yet.

The model sees only what the harness sends it. Chapter1's loop already
keeps that small: step `k`'s request is a prefix of one token array. This
chapter makes the array itself a record. The harness writes an append-only
log of events, each event's tokens are the only way anything enters the
array, and the array is what the model is sent. A request therefore never
holds a token the log does not, and every request can be rebuilt from the
log and checked.

Everything here lives in one `#authority` file,
[harness/log.coax](../../psionic/lib/harness/log.coax), because the log decides what
a model may be sent. Its checks leave no mutant alive.

## The profile

This chapter's profile is `sha256-harness-log-v1`:

- One log in process, in caller-owned storage: an array of events and an
  array of tokens. Nothing is written to disk.
- SHA-256 from `coax/lib/crypto`, over Psionic chapter 239's encoding: integers as
  little-endian 64-bit words (two's complement for negative numbers), text
  as its length then its bytes.
- Tokens, the model and the tool are chapter1's: the scripted model or
  Psionic chapter 126's engine, and the calculator.

## Events

An event has a kind, a token count, a seed and seven integer fields. Its
tokens are not stored in the event; they follow the earlier events' tokens
in the log's token store, so the store holds every event's tokens in order
and nothing else.

| Kind | Tokens | `seed` | Fields |
| --- | --- | --- | --- |
| `system`, `user`, `context` | the text | 0 | none |
| `start` | none | the turn's seed | steps, tokens, deadline, reply, tool, cancel |
| `request` | none | the step's seed | step, seen, limit |
| `reply` | the tokens kept | 0 | step, draws, stop, positions |
| `output` | the tool's output | 0 | step, value |
| `stop` | none | 0 | stop, steps, at, reply, tokens, ticks, tools |

`system`, `user` and `context` are text for the model. `context` is text the
harness injects, such as a retrieved document; it is logged like any other
text. The other kinds are one turn of chapter1's loop: the budget, each
request's place and seed, each reply and tool output, and the turn's
`HlTurn`.

Each event carries two digests. Its **content digest** covers its tokens:

```text
content = SHA-256( text("psionic.harness.log.content.v1") || u64(count) || u64(token) ... )
```

Its **event digest** covers everything else and the previous event's
digest, which chains the log:

```text
digest = SHA-256( text("psionic.harness.log.event.v1") || u64(sequence) || previous
                  || u8(kind) || u64(count) || content || u64(seed) || u64(field) x 7 )
```

The first event's `previous` is 32 zero bytes. Whether an event was redacted
is not in either digest.

<!-- source: psionic/lib/harness/log.coax encode -->
```jai
// Integers as two's-complement little-endian u64s; a content digest covers
// the token count and every token.
lg_word :: (x:int)->u64 {return ifx x<0 then (cast(u64)18446744073709551615)-cast(u64)(-1-x) else cast(u64)x;}
lg_content :: (tokens:[]int)->[32]u8 {
    s:[112]u8;b:[64]u8;out:[32]u8;#assert sha256_init(s[..])==SHA_OK;
    at:=su_u64(b[..],su_text(b[..],0,"psionic.harness.log.content.v1"),cast(u64)tokens.count);
    #assert sha256_update_checked(s[..],b[0..<at],false)==SHA_OK;
    for x:tokens {#assert sha256_update_checked(s[..],b[0..<su_u64(b[..],0,lg_word(x))],false)==SHA_OK;}
    #assert sha256_final_checked(s[..],out[..],false)==SHA_OK;return out;
}
// An event digest covers its sequence, the previous digest, its kind, its
// token count and content digest, its seed and its seven fields, and not
// whether it was redacted.
lg_digest :: (e:LgEvent)->[32]u8 {
    b:[256]u8;at:=su_u64(b[..],su_text(b[..],0,"psionic.harness.log.event.v1"),cast(u64)e.sequence);
    for i:0..<32 {b[at+i]=e.previous[i];}at+=32;b[at]=cast(u8)e.kind;at=su_u64(b[..],at+1,cast(u64)e.count);
    for i:0..<32 {b[at+i]=e.content[i];}at=su_u64(b[..],at+32,e.seed);
    for x:e.f {at=su_u64(b[..],at,lg_word(x));}
    return su_hash(b[0..<at]);
}
```

## Writing to the log

Only two functions add events. `lg_say` adds text, and only between turns:
inside a turn, the loop alone writes. `lg_put` appends an event over the
tokens already at the end of the store; the log's own functions call it
after checking that the event fits.

<!-- source: psionic/lib/harness/log.coax append -->
```jai
lg_text :: (k:LgKind)->bool {return k==LgKind.system || k==LgKind.user || k==LgKind.context;}
// Append the kind's event over the count tokens already at the store's end.
// Only the log's own functions call this; they check room first.
lg_put :: (l:[]LgLog,kind:LgKind,count:int,seed:u64,f:[7]int) {
    g:=l[0];e:=LgEvent.{sequence=g.count,kind=kind,count=count,seed=seed,f=f};
    if g.count>0 {e.previous=g.events[g.count-1].digest;}
    e.content=lg_content(g.tokens[g.used..<g.used+count]);e.digest=lg_digest(e);
    g.events[g.count]=e;l[0].count=g.count+1;l[0].used=g.used+count;
}
// Text the model will see before the next turn: system, user or injected
// context. It is refused inside an open turn, where only the loop appends.
lg_say :: (l:[]LgLog,kind:LgKind,text:[]int)->LgStatus {
    g:=l[0];
    if !lg_text(kind) {return LgStatus.shape;}
    if lg_fold_log(g,g.count).open {return LgStatus.order;}
    if g.count==g.events.count || text.count>g.tokens.count-g.used {return LgStatus.capacity;}
    for i:0..<text.count {g.tokens[g.used+i]=text[i];}
    lg_put(l,kind,text.count,0,[7]int.[0,0,0,0,0,0,0]);return LgStatus.ok;
}
```

`lg_run` is chapter1's turn with the log as its transcript. Its prompt
is every token logged so far. Before each request it logs a `request`
event, then sends exactly `tokens[0..<used]`. The model draws into the
store past `used`, where nothing is visible: the reply becomes part of
what a later request sees only when its `reply` event takes those tokens.
The tool's output is taken the same way. Every turn the log opens it also
closes with a `stop` event, so `lg_run` refuses before writing anything
when the event array could not hold the whole turn (a start, a stop and
three events a step), when the log does not verify, when a turn is
already open, or when any event is redacted: the model would be sent the
placeholder tokens.

<!-- source: psionic/lib/harness/log.coax run -->
```jai
// Chapter 200's turn with the log as its transcript. The prompt is every
// token logged so far. Each request is logged, then sent as exactly
// tokens[0..<used]; the model draws into the store past used, where nothing
// is visible until the reply event takes it; the tool's output is taken the
// same way. Every turn the log opens it also closes with a stop event, so a
// log is refused before the start when it could not hold the whole turn.
lg_run :: (m:HlModel,b:HlBudget,seed:u64,l:[]LgLog)->LgTurn {
    g:=l[0];context:=m.c.context;
    r:=LgTurn.{start=g.count,seed=seed,budget=b,prompt=g.used,turn=HlTurn.{at=g.used,reply=g.used}};
    if g.used<1 || g.tokens.count<context || b.steps<0 || b.reply<1 || b.tool<0 {r.status=LgStatus.shape;return r;}
    w:=lg_verify(g);if w.status!=LgStatus.ok {r.status=w.status;return r;}
    if lg_fold_log(g,g.count).open {r.status=LgStatus.order;return r;}
    for e:g.events[0..<g.count] {if e.redacted {r.status=LgStatus.redacted;return r;}}
    if g.events.count-g.count<2+3*b.steps {r.status=LgStatus.capacity;return r;}
    lg_put(l,LgKind.start,0,seed,[7]int.[b.steps,b.tokens,b.deadline,b.reply,b.tool,b.cancel,0]);
    t:=lg_loop(m,b,seed,l,r.prompt);
    lg_put(l,LgKind.stop,0,0,[7]int.[cast(int)cast(u8)t.stop,t.steps,t.at,t.reply,t.tokens,t.ticks,t.tools]);
    r.end=l[0].count-1;r.turn=t;return r;
}
lg_loop :: (m:HlModel,b:HlBudget,seed:u64,l:[]LgLog,prompt:int)->HlTurn {
    t:=HlTurn.{at=prompt,reply=prompt};context:=m.c.context;tokens:=l[0].tokens;
    while true {
        k:=t.steps;
        if b.cancel==2*k {t.stop=HlStop.cancelled;return t;}
        if k==b.steps {t.stop=HlStop.steps;return t;}
        left:=b.tokens-t.tokens;time:=b.deadline-t.ticks-t.at+1;room:=context-t.at;
        limit:=hl_least(hl_least(b.reply,left),hl_least(time,room));
        if limit<1 {t.stop=hl_binding(0,left,time,room);return t;}
        s:=cr_bits(seed,cast(u64)k);lg_put(l,LgKind.request,0,s,[7]int.[k,t.at,limit,0,0,0,0]);
        g:=hl_reply(m,k,tokens[0..<t.at],s,limit,tokens[t.at..<t.at+limit]);
        if g.status!=NeStatus.ok {t.stop=HlStop.model;return t;}
        lg_put(l,LgKind.reply,g.written,0,[7]int.[k,hl_draws(g),cast(int)cast(u8)g.stop,g.positions,0,0,0]);
        t.steps+=1;t.tokens+=hl_draws(g);t.ticks+=g.positions;t.reply=t.at;t.at+=g.written;
        if g.stop==NeStop.assistant_end {t.stop=HlStop.answer;return t;}
        if g.stop==NeStop.limit {t.stop=hl_binding(limit,left,time,room);return t;}
        if g.stop!=NeStop.tool {t.stop=HlStop.malformed;return t;}
        if b.cancel==2*k+1 {t.stop=HlStop.cancelled;return t;}
        if t.ticks+b.tool>b.deadline {t.stop=HlStop.deadline;return t;}
        u:=hl_tool(m.table,m.base,tokens[t.reply..<t.at],tokens[t.at..<context]);
        if u.stop!=HlStop.none {t.stop=u.stop;return t;}
        lg_put(l,LgKind.output,u.written,0,[7]int.[k,u.value,0,0,0,0,0]);
        t.at+=u.written;t.ticks+=b.tool;t.tools+=1;
    }
    return t;
}
```

The loop's control flow is chapter1's, line for line, so the checks
compare it with `hl_run` on every run rather than trust the copy.

## A log by hand

Chapter1's hand trace becomes eleven events: the prompt as one `user`
event, then the turn at seed 7 with the budget `steps 3, tokens 17,
deadline 68, reply 8, tool 3`, no cancel (`-1`).

| # | Kind | Tokens | Fields |
| --- | --- | --- | --- |
| 0 | `user` | 5: `<|bos|><|user_start|>?<|user_end|><|assistant_start|>` | |
| 1 | `start` | 0 | seed 7; 3 17 68 8 3 -1 |
| 2 | `request` | 0 | step 0, seen 5, limit 8 |
| 3 | `reply` | 7: `<|python_start|>37+48<|python_end|>` | step 0, draws 7, stop `tool` (5), positions 11 |
| 4 | `output` | 4: `<|output_start|>85<|output_end|>` | step 0, value 85 |
| 5 | `request` | 0 | step 1, seen 16, limit 8 |
| 6 | `reply` | 6: `<|python_start|>85*2<|python_end|>` | step 1, draws 6, stop `tool`, positions 21 |
| 7 | `output` | 5: `<|output_start|>170<|output_end|>` | step 1, value 170 |
| 8 | `request` | 0 | step 2, seen 27, limit 4 |
| 9 | `reply` | 3: `170` | step 2, draws 4, stop `assistant_end` (1), positions 30 |
| 10 | `stop` | 0 | `answer` (1); 3 30 27 17 68 2 |

Each number can be rebuilt from the events before it. A request's `seen` is
the count of tokens logged before it: `5`, then `5 + 7 + 4 = 16`, then
`16 + 6 + 5 = 27`. Its seed is Psionic chapter 53's counter word `cr_bits(7, k)`.
Each reply's draws follow chapter1's rule: the tokens kept, plus one when
the reply ended on `<|assistant_end|>` or `<|bos|>`, so `7`, `6` and
`3 + 1 = 4`. The stop's fields are sums over the steps: 3 replies; the
transcript ends at `27 + 3 = 30`; the last reply started at 27; draws
`7 + 6 + 4 = 17`; the clock is the positions read plus 3 ticks a tool
call, `11 + 3 + 21 + 3 + 30 = 68`; and 2 tool calls. These are exactly
chapter1's hand-traced numbers.

Event 0's content digest hashes 86 bytes: the 8-byte length 30, the 30
bytes of `psionic.harness.log.content.v1`, the count 5, and the five tokens
320, 321, 63, 322 and 323, each as 8 bytes. Its event digest hashes 181
bytes: 8 + 28 for the tag, 8 for sequence 0, 32 zero bytes, the kind byte 3,
8 for the count, the 32-byte content digest, 8 for the seed and 56 for the
seven fields. An independent reference,
[harness_log_oracle.py](../../psionic/tests/harness_log_oracle.py), writes the eleven
events from the numbers above, without running any Coax, and hashes them
with Python's `hashlib`:

```text
content(event 0) bb1b98884912c41772d47668c36585facff14c66f831db68633e330dcbf08dec
event 0  user    b4ebddca31f0f55e1ba0ff8883a0f41b43770344d986ab8a8759f1e58c73d42f
event 1  start   45049e473c7e17a058c163a6dd607518ca1ee4ca80f4549b537babbc66758ec5
event 2  request e52fb54919476951e5fa00ee1bc3ca5f9b7d0548b008033ee0852c9d94c2d142
...
event 10 stop    4433d94c10344be852085e41b0ca76456f839b121700c62f9a4858c9d9ef8a8c
```

The checks compare all eleven digests, and the content digests of event 0
and of an empty event, with the log `lg_run` writes.

## Verifying a log

The verifier walks the events in order and stops at the first one that
fails, naming the reason and the event. For each event it checks, in this
order:

1. `sequence`: its sequence number is its position.
2. `link`: its `previous` is the digest of the event before it.
3. `digest`: its digest is the digest of its own fields.
4. `content`: its tokens are in the store and, unless it was redacted, hash
   to its content digest.
5. The rules a turn must follow, which the next section folds.

At the end, every token in the store must belong to an event (`content`
otherwise). The first three checks catch any edit, reorder, drop or
insert made without rewriting the chain. The fifth catches what a forger
who rewrites the whole chain still cannot hide: a log whose events do not
explain its requests. A copy of the head digest, kept elsewhere, catches
even a consistent rewrite, because a rewritten chain has a new head.

<!-- source: psionic/lib/harness/log.coax verify -->
```jai
// The first event whose sequence, link, digest, tokens or place does not
// hold. A redacted event's tokens are not checked; its digest still is.
lg_verify :: (g:LgLog)->LgCheck {
    if g.count<0 || g.count>g.events.count || g.used<0 || g.used>g.tokens.count {return LgCheck.{status=LgStatus.shape};}
    v:=LgFold.{};c:=LgCheck.{};
    for e,i:g.events[0..<g.count] {
        c.index=i;
        if e.sequence!=i {c.status=LgStatus.sequence;return c;}
        if !su_same(e.previous,c.head) {c.status=LgStatus.link;return c;}
        if !su_same(e.digest,lg_digest(e)) {c.status=LgStatus.digest;return c;}
        if e.count<0 || e.count>g.used-v.at {c.status=LgStatus.content;return c;}
        if !e.redacted && !su_same(e.content,lg_content(g.tokens[v.at..<v.at+e.count])) {c.status=LgStatus.content;return c;}
        v=lg_fold(v,e);if v.status!=LgStatus.ok {c.status=v.status;return c;}
        c.head=e.digest;
    }
    c.index=g.count;if v.at!=g.used {c.status=LgStatus.content;}
    return c;
}
```

## What the events must explain

`lg_fold` applies one event to the verifier's state. Text comes only
between turns (`order` otherwise). A turn is `start`, then any number of
`request`, `reply` and, after a reply that ended in a call, `output`, then
`stop`. A request must carry the step number and its seed (`step`) and
must have seen exactly the tokens logged before it (`view`). A reply must
not be longer than its request's limit, and its draws must follow chapter
200's rule (`step`). The stop must equal the turn the steps rebuild
(`turn`). The stop's own kind, such as `answer` or `deadline`, is the
loop's decision; replay is what checks it against the model.

<!-- source: psionic/lib/harness/log.coax fold -->
```jai
// One event against the verifier's state. Text comes only between turns; a
// turn is start, then (request reply output?)*, then stop, where an output
// follows only a reply that ended in a call. Each request must have seen
// exactly the tokens before it and carry the step's seed; each reply's draws
// follow chapter 200's rule; the stop must equal the turn rebuilt from them.
// step is the step being rebuilt.
lg_fold :: (s:LgFold,e:LgEvent)->LgFold {
    v:=s;k:=v.r.turn.steps;f:=e.f;
    if lg_text(e.kind) {
        if v.open {v.status=LgStatus.order;return v;}
    }
    else if e.kind==LgKind.start {
        if v.open {v.status=LgStatus.order;return v;}
        v.open=true;v.r=LgTurn.{seed=e.seed,prompt=v.at,turn=HlTurn.{at=v.at,reply=v.at}};
        v.r.budget=HlBudget.{steps=f[0],tokens=f[1],deadline=f[2],reply=f[3],tool=f[4],cancel=f[5]};
    }
    else if e.kind==LgKind.request {
        if v.last!=LgKind.start && v.last!=LgKind.output {v.status=LgStatus.order;return v;}
        if f[0]!=k || e.seed!=cr_bits(v.r.seed,cast(u64)k) {v.status=LgStatus.step;return v;}
        if f[1]!=v.at {v.status=LgStatus.view;return v;}
        v.step=HlStep.{seen=f[1],seed=e.seed,limit=f[2]};
    }
    else if e.kind==LgKind.reply {
        if v.last!=LgKind.request {v.status=LgStatus.order;return v;}
        v.step.stop=cast(NeStop)cast(u8)f[2];
        if f[0]!=k || e.count>v.step.limit || f[1]!=hl_draws(NeReply.{stop=v.step.stop,written=e.count}) {v.status=LgStatus.step;return v;}
        v.step.written=e.count;v.step.draws=f[1];v.step.positions=f[3];
        v.r.turn.steps=k+1;v.r.turn.tokens+=f[1];v.r.turn.ticks+=f[3];v.r.turn.reply=v.at;v.r.turn.at=v.at+e.count;
    }
    else if e.kind==LgKind.output {
        if v.last!=LgKind.reply || v.step.stop!=NeStop.tool {v.status=LgStatus.order;return v;}
        if f[0]!=k-1 {v.status=LgStatus.step;return v;}
        v.step.value=f[1];v.step.output=e.count;
        v.r.turn.tools+=1;v.r.turn.ticks+=v.r.budget.tool;v.r.turn.at=v.at+e.count;
    }
    else if e.kind==LgKind.stop {
        if !v.open {v.status=LgStatus.order;return v;}
        t:=v.r.turn;
        if f[1]!=t.steps || f[2]!=t.at || f[3]!=t.reply || f[4]!=t.tokens || f[5]!=t.ticks || f[6]!=t.tools {v.status=LgStatus.turn;return v;}
        v.r.turn.stop=cast(HlStop)cast(u8)f[0];v.open=false;
    }
    else {v.status=LgStatus.order;return v;}
    v.at+=e.count;v.last=e.kind;return v;
}
// The state after events 0..<n, folded without checking digests.
lg_fold_log :: (g:LgLog,n:int)->LgFold {
    v:=LgFold.{};
    for e:g.events[0..<n] {v=lg_fold(v,e);}
    return v;
}
```

A log cut after any event still verifies. Its last turn is open: no text
and no new turn may be added until it is closed, and `lg_turn` reports it as
`open` rather than inventing a stop.

## Admitting a request

`lg_admit` is the gate for a request built somewhere other than the log,
such as chapter3's prompt assembly. Given a `request` event and the
tokens that were, or would be, sent, it refuses unless the tokens are
exactly the tokens of the events before it: the length must equal the
request's `seen` (`view`), and each event's slice of the request must hash
to that event's content digest (`content`, naming the event). Comparing
digests rather than tokens is what lets it check a request against a log
whose text has been redacted.

<!-- source: psionic/lib/harness/log.coax rebuild -->
```jai
// The turn whose start event is at start, rebuilt from a verified log: each
// step goes to steps once its reply, and then its output, is folded, while
// steps has room. A turn without a stop event is open.
lg_turn :: (g:LgLog,start:int,steps:[]HlStep)->LgTurn {
    c:=lg_verify(g);if c.status!=LgStatus.ok {return LgTurn.{status=c.status,start=start};}
    if lg_kind(g,start)!=LgKind.start {return LgTurn.{status=LgStatus.shape,start=start};}
    v:=LgFold.{at=lg_offset(g,start)};
    for e,i:g.events[start..<g.count] {
        v=lg_fold(v,e);k:=v.r.turn.steps;
        if (e.kind==LgKind.reply || e.kind==LgKind.output) && k<=steps.count {steps[k-1]=v.step;}
        if !v.open {v.r.start=start;v.r.end=start+i;return v.r;}
    }
    return LgTurn.{status=LgStatus.open,start=start};
}
// The request a step's model was sent, checked against the log: request
// must be exactly the tokens of the events before that step's request event,
// token for token, or for a redacted event, digest for digest.
lg_admit :: (g:LgLog,index:int,request:[]int)->LgCheck {
    c:=lg_verify(g);if c.status!=LgStatus.ok {return c;}
    c.index=index;
    if lg_kind(g,index)!=LgKind.request {c.status=LgStatus.shape;return c;}
    if request.count!=g.events[index].f[1] {c.status=LgStatus.view;return c;}
    at:=0;
    for e,i:g.events[0..<index] {
        if !su_same(e.content,lg_content(request[at..<at+e.count])) {c.status=LgStatus.content;c.index=i;return c;}
        at+=e.count;
    }
    return c;
}
```

## Redaction

Some text must not be kept, such as a secret a user pasted. `lg_redact`
overwrites one text, reply or output event's tokens with `-1` and marks it
redacted. Its content digest stays, so its event digest, every later
event's link and the head are unchanged, and the log still verifies.
Every request still admits exactly the tokens it was sent, compared by
digest. What redaction gives up is the tokens themselves: a turn that saw
a redacted event cannot be replayed (`redacted`), and a log holding one
runs no new turn.

<!-- source: psionic/lib/harness/log.coax redact -->
```jai
// Redaction overwrites a text event's tokens and keeps its content digest,
// so the chain, and every check that needs only digests, still holds.
lg_redact :: (l:[]LgLog,index:int)->LgStatus {
    g:=l[0];k:=lg_kind(g,index);
    if !lg_text(k) && k!=LgKind.reply && k!=LgKind.output {return LgStatus.shape;}
    at:=lg_offset(g,index);for i:at..<at+g.events[index].count {g.tokens[i]=LG_HIDDEN;}
    g.events[index].redacted=true;return LgStatus.ok;
}
// The kind of event index, or none past either end.
lg_kind :: (g:LgLog,index:int)->LgKind {if index<0 || index>=g.count {return LgKind.none;}return g.events[index].kind;}
lg_offset :: (g:LgLog,index:int)->int {at:=0;for e:g.events[0..<index] {at+=e.count;}return at;}
```

## Replay

`lg_replay` rebuilds a turn's `HlTurn` and `HlStep` records from the
events and hands them, with the log's own tokens as the transcript, to
chapter1's `hl_replay`. Replay from the log is therefore chapter1's
replay: every reply is redrawn from its logged seed and must match token
for token, and every tool output is recomputed.

<!-- source: psionic/lib/harness/log.coax replay -->
```jai
// A turn replays when its rebuilt record passes chapter 200's hl_replay over
// the log's own tokens. A turn whose text was redacted cannot be resent.
lg_replay :: (m:HlModel,g:LgLog,start:int,steps:[]HlStep,scratch:[]int)->LgStatus {
    r:=lg_turn(g,start,steps);if r.status!=LgStatus.ok {return r.status;}
    if steps.count<r.turn.steps {return LgStatus.shape;}
    for e:g.events[0..<r.end] {if e.redacted {return LgStatus.redacted;}}
    if !hl_replay(m,r.seed,r.prompt,g.tokens,r.turn,steps,scratch) {return LgStatus.replay;}
    return LgStatus.ok;
}
```

## Run it

The example, [harness-log.coax](../../psionic/examples/harness-log.coax), logs the
hand trace and prints every event, verifies the log, admits each request,
tampers with the log in eight ways, redacts the prompt, and runs a second
turn on Psionic chapter 126's engine with Psionic chapter 136's programmed bigram weights
on top of the first.

<!-- source: psionic/examples/harness-log.coax example -->
```jai
main :: () {
    tb:=NC_LEARNED;tr:=HL_TRACE;ts:=HL_TRACE_STARTS;m:=hl_scripted(tb[..],tr[..],ts[..],48);p:=HL_PROMPT;
    ev:[32]LgEvent;tk:[64]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];
    // Chapter 200's hand trace, logged: the prompt, then the turn.
    say:=lg_say(l[..],LgKind.user,p[..]);r:=lg_run(m,HL_TRACE_BUDGET,7,l[..]);
    print("prompt logged: %; turn: %, stop %, steps %, transcript % tokens\n",say,r.status,r.turn.stop,r.turn.steps,r.turn.at);
    lx_events(l[0],0);
    c:=lg_verify(l[0]);print("verify: % over % events, head ",c.status,c.index);lx_hex(c.head,32);print("\n");
    // Each request admits exactly what it was sent, and nothing else.
    g:=l[0];ids:=tk;
    for k:[3]int.[2,5,8] {n:=ev[k].f[1];print("request event % (% tokens): sent %",k,n,lg_admit(g,k,ids[0..<n]).status);
        print(", one token more %",lg_admit(g,k,ids[0..<n+1]).status);bad:=ids;bad[n-1]=64;c=lg_admit(g,k,bad[0..<n]);print(", its last token changed % at event %\n",c.status,c.index);}
    // Tampering, without rewriting the chain.
    t:=tk;t[8]=50;lx_check("token 8 edited",lg_verify(LgLog.{events=ev[..],tokens=t[..],count=11,used=30}));
    e:=ev;e[6].f[3]=20;lx_check("reply 1's positions edited",lg_verify(LgLog.{events=e[..],tokens=tk[..],count=11,used=30}));
    e=ev;e[4]=ev[5];e[5]=ev[4];lx_check("events 4 and 5 swapped",lg_verify(LgLog.{events=e[..],tokens=tk[..],count=11,used=30}));
    e=ev;for i:4..<10 {e[i]=ev[i+1];}lx_check("output 0 dropped",lg_verify(LgLog.{events=e[..],tokens=tk[..],count=10,used=30}));
    e=ev;for i:5..<12 {e[i]=ev[i-1];}lx_check("output 0 inserted twice",lg_verify(LgLog.{events=e[..],tokens=tk[..],count=12,used=30}));
    // Tampering by someone who rewrites the whole chain: what the events must explain still fails.
    e=ev;t=tk;e[1]=LgEvent.{kind=LgKind.context,count=1};for i:2..<12 {e[i]=ev[i-1];}t[5]=120;for i:6..<31 {t[i]=tk[i-1];}lc_rechain(e[..],12,t[..]);
    lx_check("context injected before the turn, chain rewritten",lg_verify(LgLog.{events=e[..],tokens=t[..],count=12,used=31}));
    e=ev;e[5]=LgEvent.{kind=LgKind.context};for i:6..<12 {e[i]=ev[i-1];}lc_rechain(e[..],12,tk[..]);
    lx_check("context injected inside the turn, chain rewritten",lg_verify(LgLog.{events=e[..],tokens=tk[..],count=12,used=30}));
    e=ev;e[10].f[4]=16;lc_rechain(e[..],11,tk[..]);lx_check("stop's token count rewritten",lg_verify(LgLog.{events=e[..],tokens=tk[..],count=11,used=30}));
    // Redaction: the prompt's tokens go, its digest stays.
    steps:[4]HlStep;scratch:[64]int;
    print("replay before redaction: %\n",lg_replay(m,l[0],1,steps[..],scratch[..]));
    e=ev;t=tk;h:=[1]LgLog.[LgLog.{events=e[..],tokens=t[..],count=11,used=30}];
    print("redact event 0: %; tokens 0..4 now % % % % %\n",lg_redact(h[..],0),t[0],t[1],t[2],t[3],t[4]);
    c=lg_verify(h[0]);print("verify: %, head unchanged: %\n",c.status,su_same(c.head,ev[10].digest));
    print("request event 8 admits what it was sent: %; replay: %; a new turn: %\n",lg_admit(h[0],8,ids[0..<27]).status,lg_replay(m,h[0],1,steps[..],scratch[..]),lg_run(m,HL_TRACE_BUDGET,7,h[..]).status);
    // A second turn on chapter 126's engine, on top of the first.
    cf:=SF_TEST;w:[29416]f64;cache:[8000]f64;work:[12000]f64;logits:[21056]f64;last:[329]f32;pr:[329]f64;st:[329]f64;or:[329]int;te:[329]int;
    ww:=ne_buffers(cf,64,cache[..],work[..],logits[..],last[..],pr[..],st[..],or[..],te[..]);pairs:=HL_PAIRS;sf_program(cf,pairs[..],w[..]);
    en:=HlModel.{engine=true,c=cf,base=320,table=tb[..],w=w[..],options=NxOptions.{temperature=0},work=ww};
    q:=[4]int.[321,63,322,323];eb:=HlBudget.{steps=4,tokens=40,deadline=1000,reply=16,tool=0};
    print("second prompt logged: %\n",lg_say(l[..],LgKind.user,q[..]));
    u:=lg_run(en,eb,5,l[..]);print("engine turn: %, stop %, steps %, transcript % tokens, events % to %\n",u.status,u.turn.stop,u.turn.steps,u.turn.at,u.start,u.end);
    lx_events(l[0],11);
    print("verify: %; replay turn 1 on the script: %, turn 2 on the engine: %, turn 2 on the script: %\n",lg_verify(l[0]).status,lg_replay(m,l[0],1,steps[..],scratch[..]),lg_replay(en,l[0],u.start,steps[..],scratch[..]),lg_replay(m,l[0],u.start,steps[..],scratch[..]));
}
```

```text
prompt logged: ok; turn: ok, stop answer, steps 3, transcript 30 tokens
  0 user tokens 5, seed 0, fields 0 0 0 0 0 0 0, digest b4ebddca31f0f55e
  1 start tokens 0, seed 7, fields 3 17 68 8 3 -1 0, digest 45049e473c7e17a0
  2 request tokens 0, seed 7191089600892374487, fields 0 5 8 0 0 0 0, digest e52fb54919476951
  3 reply tokens 7, seed 0, fields 0 7 5 11 0 0 0, digest 0002502c18e6fdd5
  4 output tokens 4, seed 0, fields 0 85 0 0 0 0 0, digest ed2b08e60de817ab
  5 request tokens 0, seed 309689372594955804, fields 1 16 8 0 0 0 0, digest c4c9224775dffee9
  6 reply tokens 6, seed 0, fields 1 6 5 21 0 0 0, digest 9227268f44a8fdea
  7 output tokens 5, seed 0, fields 1 170 0 0 0 0 0, digest c10cddbb9d9dec02
  8 request tokens 0, seed 16616101746815609346, fields 2 27 4 0 0 0 0, digest 434bc1d0a00a64a9
  9 reply tokens 3, seed 0, fields 2 4 1 30 0 0 0, digest 9f5c21d3656b21fe
  10 stop tokens 0, seed 0, fields 1 3 30 27 17 68 2, digest 4433d94c10344be8
verify: ok over 11 events, head 4433d94c10344be852085e41b0ca76456f839b121700c62f9a4858c9d9ef8a8c
request event 2 (5 tokens): sent ok, one token more view, its last token changed content at event 0
request event 5 (16 tokens): sent ok, one token more view, its last token changed content at event 4
request event 8 (27 tokens): sent ok, one token more view, its last token changed content at event 7
token 8 edited: content at event 3
reply 1's positions edited: digest at event 6
events 4 and 5 swapped: sequence at event 4
output 0 dropped: sequence at event 4
output 0 inserted twice: sequence at event 5
context injected before the turn, chain rewritten: view at event 3
context injected inside the turn, chain rewritten: order at event 5
stop's token count rewritten: turn at event 10
replay before redaction: ok
redact event 0: ok; tokens 0..4 now -1 -1 -1 -1 -1
verify: ok, head unchanged: true
request event 8 admits what it was sent: ok; replay: redacted; a new turn: redacted
second prompt logged: ok
engine turn: ok, stop answer, steps 2, transcript 43 tokens, events 12 to 18
  11 user tokens 4, seed 0, fields 0 0 0 0 0 0 0, digest 6b053f057a99a37c
  12 start tokens 0, seed 5, fields 4 40 1000 16 0 -1 0, digest c9200d68b5bfa0f3
  13 request tokens 0, seed 7134611160154358618, fields 0 34 16 0 0 0 0, digest a01ae49ea4fe74ad
  14 reply tokens 5, seed 0, fields 0 5 5 38 0 0 0, digest 6a25fd80fb5e5920
  15 output tokens 3, seed 0, fields 0 3 0 0 0 0 0, digest 4b99360c28deebf0
  16 request tokens 0, seed 13877614986023876344, fields 1 42 16 0 0 0 0, digest 025814b723ad89ee
  17 reply tokens 1, seed 0, fields 1 2 1 43 0 0 0, digest b4379d7bf50452a3
  18 stop tokens 0, seed 0, fields 1 2 43 42 7 81 1, digest 910ff536b36f1d69
verify: ok; replay turn 1 on the script: ok, turn 2 on the engine: ok, turn 2 on the script: replay
```

The [transcript](../../psionic/fixtures/harness-log.out) is exact for the named build
in debug and release. The first eleven events and the head are the
hand-worked values above. Each request admits the tokens it was sent;
one more token is refused as `view`, and a changed last token as `content`
at the event that holds it (the prompt, output 0, output 1). The tamper
lines are the deliberately broken inputs, each refused with its reason:
an edited token is `content` at the reply that holds it, an edited field
`digest`, a swap, a drop and a duplicate `sequence` where the order first
breaks. With the chain rewritten, an injected context token before the
turn leaves the first request explaining 5 of 6 tokens (`view` at event
3), context inside the turn is `order`, and a changed stop count is
`turn`. After the prompt is redacted the head is unchanged and the last
request still admits its 27 tokens, but the turn no longer replays and
no new turn runs. The engine's turn starts at event 12 with a 32-token
prompt, calls `1+2`, answers `3`, and replays on the engine but not on the
script.

## Checks

- **Hand-worked digests:** all eleven event digests of the hand trace, its
  content digests, the head, and the content digest of a 100-token event,
  against the values Python's `hashlib` gives in
  [harness_log_oracle.py](../../psionic/tests/harness_log_oracle.py), and the
  transcript equal to `hl_run`'s.
- **Chapter1's boundaries, logged:** every budget and window case
  chapter1 checks at its limit and one past it, each cancel boundary and
  a free tool, run through `hl_run` and `lg_run`: the same turn, transcript
  and steps, rebuilt from the log, and replay.
- **Tampering without rewriting the chain:** every one of the 30 tokens
  edited (`content` at its event), every field, seed, kind, content digest
  and digest of every event (`digest`), every link (`link`), every sequence
  number (`sequence`), a swap, a drop and an insert, and stores whose used
  length is one short or one long.
- **Tampering with the chain rewritten** by the forger in
  [harness-log-checks.coax](../../psionic/tests/harness-log-checks.coax): injected
  context before the turn (`view`, and a head equal to the Python
  reference's), text of each kind inside a turn, a dropped output, a second
  start, a stop outside a turn, a reply without its request, two requests
  in a row, an output after an answer or a cut reply (`order`), each step
  number, seed, draw count and reply length (`step`), a request's length
  (`view`), and each of the stop's six counts (`turn`).
- **Redaction** of each text, reply and output event: the chain and head
  unchanged, every request still admitted and a changed token still
  refused, the turn still rebuilt, replay refused as `redacted`, no new
  turn, and the digest still binding the redacted event; start, request
  and stop events cannot be redacted, nor can an index past the log's end.
- **The loop:** each refusal before anything is logged (empty log, invalid
  budget, a short token store, an event array one event too small, a broken
  log, an open turn), text rules, exact room, a model refusal logged as a
  request with no reply, and a second turn whose prompt is the whole log.
- **Every step's view is rebuilt from the log:** 200 random scripts and
  budgets from chapter1's brute-force oracle, with the prompt split into
  one to three text events of random kinds, run through `hl_run` and
  `lg_run`. The turns must be equal, the token store must equal `hl_run`'s
  transcript, the steps rebuilt from the log must equal `hl_run`'s field
  for field, each request event must admit exactly the prefix chapter
  200's step saw, and both `lg_replay` and `hl_replay` must accept the run.

Mutation of `harness/log.coax` leaves no survivors.

## In Coder

Coder's session log, `~/.openagents/coder/sessions/ID.jsonl`, is one JSON
event a line, appended by the session's own process
([session.coax](../../coder/tui/session.coax)). It isn't hash-chained
like this chapter's log, but it has the same problem at its end: a crash
during an append, or a reader that looks during one, finds a last line with
no newline.

- **Readers take complete lines.** `cd_session_file`, which `coder status`,
  `log`, `replay`, `fork`, `attach` and the Router all read through, stops
  at the last newline. The partial line is never parsed.
- **The cut is reported.** `coder status` shows "log ends in a partial line
  of N bytes, not read", and `--json` adds `"cut":N`. `coder log` says the
  same on stderr, and `coder log --json` prints only complete lines, so its
  output is always valid JSON lines.
- **A line after a crash starts clean.** Two lines are written by a process
  other than the session's: the watchdog's `lost` end when the session's
  process is gone (chapter 20), and `coder stop`'s `stopped` end after it
  kills the process. Both go through `cs_append_after`, which first moves a
  partial last line to `ID.jsonl.cut` and rewrites the log to its complete
  lines, so the new line is never glued to half of an old one.

The smoke runs a session whose log holds a two-byte character, cuts the
log at every byte offset from 0 to its length, and checks that `status`
reports the right cut, `log --json` returns exactly the complete prefix as
valid JSON, and a cut inside the first line reads as no session. It then
cuts the log inside its end line, leaves a dead process ID, and checks that
the watchdog's `lost` line follows the last complete line and the cut bytes
are in `ID.jsonl.cut`.

- **A line is on storage before anyone sees it.** Each session line is
  written with `fs_append_durable` ([fs.coax](../../coax/lib/sys/fs.coax)),
  which returns only after the bytes are on storage: `F_FULLFSYNC` on
  macOS, where plain `fsync` leaves data in the drive's cache, and `fsync`
  elsewhere, plus a sync of the folder when the file is new. Only then does
  `--json` print the line, so a line a reader saw survives a crash or power
  loss. On an Apple-silicon Mac this costs about 3 ms a line: a five-line
  `coder run` went from 26 ms to 42 ms.

What this does not give: Claude Code and Codex run their own tools and
report them afterward, so a tool line is durable before Coder reports it,
not before the tool's effect happens. Microcoder and `$ ` commands are
logged when their results come back for the same reason.

## What remains

- Event schemas in `proto/`, each a versioned message, and the log's
  content as protobuf rather than this chapter's fixed encoding.
- A byte format on disk, with a version, and readers kept for older
  versions so an old session still opens.
- Recovery of this chapter's hash-chained log truncated at any byte offset
  to its last complete event, with the truncation reported. Here a log can
  be cut only between events, where it verifies with its last turn open.
  Coder's session logs already do this (see [In Coder](#in-coder)).
- Durability for this chapter's log: each event written to storage before
  its effect is reported, so a crash leaves a log that ends at a known
  boundary. Coder's session log does the first half (see
  [In Coder](#in-coder)).
- Text added inside a turn (a user interjecting, or context injected
  between steps). Here text comes only between turns.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
