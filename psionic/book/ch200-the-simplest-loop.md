# 200. The simplest loop

> **Status: partly implemented.** The smallest agent loop runs on the CPU
> over chapter 126's engine and chapter 136's calculator tool: a model turn,
> a typed tool call, the result appended, and again, until the model answers
> or a budget stops the turn. Every turn ends with one typed stop, every
> request's input is a prefix of the transcript, and every step replays from
> its seed. Still to come: a model served through chapters 20, 22 and 79
> instead of one called in process, a deadline on the wall clock instead of
> a logical clock, and cancellation that interrupts a running model stream
> and running tools instead of acting between steps.

A harness turns a model into an agent. It sends the model a request, reads
the reply, runs the tool the reply asks for, appends the tool's output and
asks again. It also decides when to stop: when the model gives a final
answer, or when the turn has used up what it was allowed to spend. This
chapter builds the smallest loop that does that, and every later chapter in
Part XXI adds one thing to it.

Everything here lives in one `#authority` file,
[harness/loop.coax](../lib/harness/loop.coax), because the loop decides
what runs and when a turn ends. Its checks leave no mutant alive.

## The profile

This chapter's profile is `cpu-harness-loop-v1`:

- One model, called in process: chapter 126's `ne_generate`, or a scripted
  stand-in that draws a fixed reply one token at a time through the same
  stop rule. Nothing goes over a network.
- One tool: chapter 136's calculator, `a op b` with one to four decimal
  digits on each side and `op` one of `+`, `-`, `*`.
- Tokens are chapter 123's: 320 ordinary tokens and nanochat's nine
  specials from 320 (`<|bos|>` is 320, `<|assistant_end|>` 324,
  `<|python_start|>` 325, `<|python_end|>` 326, `<|output_start|>` 327,
  `<|output_end|>` 328). The digits and `+ - * ? /` are their own byte IDs.
- The clock is logical. A reply costs the number of positions the engine
  reads for it, and a tool call costs a fixed number of ticks. The same
  inputs always give the same clock, so a deadline is exact and testable.

## Turns, steps and stops

A **turn** is one task: a prompt, then steps until it stops. A **step** is
one model request plus the tool call its reply ends with, if any. The
transcript is one token array, `ids`. The prompt fills `ids[0..<prompt]`;
each reply and each tool output is appended after it. Step `k`'s request is
exactly `ids[0..<steps[k].seen]`: the model never sees anything the
transcript does not hold, and later steps only append. Chapters 201 and 202
build on that property.

A reply ends in one of four ways, the same ways chapter 126's engine ends
one: `<|assistant_end|>` (an answer; the stop token is not kept),
`<|python_end|>` closing a call (a tool call), `<|bos|>` (not kept), or the
draw limit. The loop turns those, and its own budgets, into exactly one
`HlStop`:

| Stop | When |
| --- | --- |
| `answer` | the reply ended with `<|assistant_end|>` |
| `steps` | the step budget is spent before a request |
| `tokens` | the drawn-token budget is spent |
| `deadline` | the clock would pass the deadline |
| `window` | the transcript, or a tool output, would not fit the model's window |
| `length` | a reply reached the per-reply cap without ending |
| `malformed` | a reply drew `<|bos|>`, or its call does not parse; nothing runs |
| `cancelled` | a cancel arrived at a boundary |
| `model` | the model refused the request |
| `shape` | the budget or buffers are invalid; nothing is asked |

## The budget

`HlBudget` names what a turn may spend: `steps` requests, `tokens` drawn
tokens (a stop token counts, because the model spent a draw on it), a
`deadline` on the clock, a `reply` cap on one reply's draws, the `tool`
ticks one call costs, and the boundary where a `cancel` arrives.

Before each request the loop computes what is left of each limit. A reply
of `limit` draws that starts at transcript length `at` reads at most
positions `0` to `at + limit - 1`, so the clock can reach
`ticks + at + limit - 1` and no further. The request's draw limit is the
least of four numbers:

```text
limit = least(reply, tokens - drawn, deadline - ticks - at + 1, window - at)
```

A request whose worst case would pass the deadline gets a smaller limit, so
no step ever passes the deadline. When the limit is below 1 the request is
not sent. When a reply stops at its limit, the stop names the limit that
bound it, in a fixed order: tokens, then the deadline, then the window,
then the per-reply cap.

<!-- source: psionic/lib/harness/loop.coax budget -->
```jai
// What ends the turn when a reply's draw limit is spent (n is that limit,
// the least of the four) or when one of them leaves nothing (n is 0):
// tokens first, then the deadline, then the window, else the per-reply cap.
hl_binding :: (n:int,tokens:int,time:int,room:int)->HlStop {
    if tokens<=n {return HlStop.tokens;}
    if time<=n {return HlStop.deadline;}
    if room<=n {return HlStop.window;}
    return HlStop.length;
}
hl_least :: (a:int,b:int)->int {return (a+b-cast(int)abs(f64(a-b)))/2;}
// A reply of limit draws reads at most positions 0..at+limit-1, so the
// clock may reach ticks+at+limit-1 and no further.
hl_draws :: (g:NeReply)->int {return ifx g.stop==NeStop.assistant_end || g.stop==NeStop.bos then g.written+1 else g.written;}
```

## A turn by hand

The checks' scripted turn has a 5-token prompt, `<|bos|><|user_start|>?
<|user_end|><|assistant_start|>`, a window of 48, and the budget
`steps 3, tokens 17, deadline 68, reply 8, tool 3`. The script has three
replies: the call `37+48`, the call `85*2`, and the answer `170`.

**Step 0.** `at = 5`, nothing drawn, clock 0. The limits left are 17
tokens, `68 - 0 - 5 + 1 = 64` ticks and `48 - 5 = 43` positions, so the
draw limit is the cap, 8. The reply draws 7 tokens,
`<|python_start|>3 7 + 4 8<|python_end|>`, and stops as a tool call. It
reads `5 + 7 - 1 = 11` positions, so the clock is 11 and the transcript 12.
The call parses as `37 + 48 = 85`, and the loop appends
`<|output_start|>8 5<|output_end|>`, 4 tokens: transcript 16, clock
`11 + 3 = 14`.

**Step 1.** `at = 16`, 7 drawn, clock 14. Left: 10 tokens,
`68 - 14 - 16 + 1 = 39` ticks, 32 positions; the limit is 8. The reply
`<|python_start|>8 5 * 2<|python_end|>` draws 6 and reads
`16 + 6 - 1 = 21` positions: clock 35, transcript 22. `85 * 2 = 170` appends
5 tokens: transcript 27, clock 38.

**Step 2.** `at = 27`, 13 drawn, clock 38. Left: 4 tokens,
`68 - 38 - 27 + 1 = 4` ticks, 21 positions; the limit is 4. The reply draws
`1 7 0<|assistant_end|>`, 4 draws, and keeps 3: a stop token drawn at the
limit still ends the reply, as chapter 53's sampler rules. It reads
`27 + 4 - 1 = 30` positions, so the clock reaches 68, exactly the
deadline.

The turn stops as `answer` after 3 steps, with a 30-token transcript, 17
tokens drawn, clock 68 and 2 tool calls. Every budget is exactly spent, so
lowering any one by 1 changes the stop:

- `steps 2`: the third request is never sent; stop `steps` at transcript 27.
- `tokens 16`: step 2's limit is 3, the reply is cut after `170`, stop
  `tokens`.
- `deadline 67`: step 2's limit is 3 again, but now the clock binds; stop
  `deadline` at clock 67.
- `deadline 38` runs the second call and refuses the third request;
  `deadline 37` refuses the second call itself, since `35 + 3 > 37`. No
  tool runs past the deadline.
- `reply 6`: the first call is cut after `37+48`, before `<|python_end|>`;
  stop `length`.
- window 30: step 2 has 3 positions; stop `window`. Window 26: the second
  output, 5 tokens at 22, does not fit; stop `window` and nothing is
  appended.

## The tool call

The call is the text between the reply's first `<|python_start|>` and its
closing `<|python_end|>`; text before the call is allowed. The tool decodes
it with the model's table, parses it, and only then computes. A call that
does not decode (any special token is outside the table) or does not parse
is `malformed`, and nothing runs and nothing is appended. Parsing is
separate from computing, so a malformed call cannot reach the arithmetic.

<!-- source: psionic/lib/harness/loop.coax call -->
```jai
// The calculator's call: "a op b", each number one to four decimal digits,
// op one of + - *. The grammar is chapter 136's; parsing runs nothing.
hl_digits :: (s:string,at:int)->int {e:=at;while e<s.count && s[e]>='0' && s[e]<='9' {e+=1;}return e-at;}
hl_number :: (s:string,at:int,n:int)->int {x:=0;for i:at..<at+n {x=x*10+cast(int)(s[i]-'0');}return x;}
hl_parse :: (s:string)->HlCall {
    da:=hl_digits(s,0);if da<1 || da>4 || da>=s.count {return HlCall.{};}
    op:=s[da];if op!='+' && op!='-' && op!='*' {return HlCall.{};}
    db:=hl_digits(s,da+1);if db<1 || db>4 || da+1+db!=s.count {return HlCall.{};}
    return HlCall.{ok=true,a=hl_number(s,0,da),op=op,b=hl_number(s,da+1,db)};
}
hl_apply :: (c:HlCall)->int {if c.op=='+' {return c.a+c.b;}if c.op=='-' {return c.a-c.b;}return c.a*c.b;}
```

<!-- source: psionic/lib/harness/loop.coax tool -->
```jai
// One tool call, from a reply that ended with <|python_end|>: the call is
// the text after the reply's first <|python_start|>. A call that does not
// decode (a special token is not in the table) or that the grammar refuses
// is malformed and runs nothing. The
// output goes to out between <|output_start|> and <|output_end|>, or, when
// out cannot hold it, nowhere.
hl_tool :: (table:[]u8,base:int,reply:[]int,out:[]int)->HlTool {
    s:=0;while s<reply.count && reply[s]!=base+NC_PYTHON_START {s+=1;}
    if s==reply.count {return HlTool.{stop=HlStop.malformed};}
    call:=reply[s+1..<reply.count-1];
    d:=bt_decode(table,call);if d.status!=PtStatus.ok {return HlTool.{stop=HlStop.malformed};}
    c:=hl_parse(cast(string)d.bytes[0..<d.written]);if !c.ok {return HlTool.{stop=HlStop.malformed};}
    v:=hl_apply(c);b:[24]u8;e:=bt_encode(table,cast(string)b[0..<hl_itoa(v,b[..])]);
    if e.written+2>out.count {return HlTool.{stop=HlStop.window,value=v};}
    out[0]=base+NC_OUTPUT_START;for i:0..<e.written {out[1+i]=e.ids[i];}out[e.written+1]=base+NC_OUTPUT_END;
    return HlTool.{value=v,written=e.written+2};
}
```

## The model behind the loop

Coax has no function values, so the model is a small record: either
chapter 126's engine over a configuration and weights, or a script. The
scripted model feeds each token through chapter 126's own `ne_after`, so it
stops, keeps tokens and counts positions exactly as the engine would. The
checks hold this to the engine's behavior directly: a 45-token prompt in a
48-token window stops as `context` after four tokens, as `ne_generate`
does.

<!-- source: psionic/lib/harness/loop.coax reply -->
```jai
// Reply k to the prompt, drawn under limit. The scripted model draws its
// script's tokens through chapter 126's own stop rule (ne_after), so it
// stops, keeps tokens and counts positions exactly as the engine does. A
// script reply that ends before it stops is the model's failure.
hl_reply :: (m:HlModel,k:int,prompt:[]int,seed:u64,limit:int,out:[]int)->NeReply {
    if m.engine {return ne_generate(m.c,m.w,m.base,prompt,m.options,seed,limit,out,m.work,false);}
    if k+1>=m.starts.count || m.starts[k]<0 || m.starts[k+1]>m.script.count {return NeReply.{status=NeStatus.shape};}
    r:=m.script[m.starts[k]..<m.script.count];python:=false;at:=prompt.count;
    for i:0..<m.starts[k+1]-m.starts[k] {
        s:=NxStop.running;
        if r[i]==m.base+NC_ASSISTANT_END || r[i]==m.base {s=NxStop.stop_token;}else if i+1==limit {s=NxStop.limit;}
        a:=ne_after(m.base,r[i],s,python,at+i,m.c.context);python=a.python;
        if a.emit {out[i]=r[i];}
        if a.stop!=NeStop.none {return NeReply.{stop=a.stop,written=ifx a.emit then i+1 else i,positions=at+i};}
    }
    return NeReply.{status=NeStatus.shape};
}
```

## The loop

Each request draws with its own seed, `cr_bits(seed, k)` from chapter 103's
counter generator, so a turn's seed names its whole transcript. Before each
request the loop checks the cancel, the step count and the limits. After a
reply it records the step, then decides: an answer stops, a limit stops
with its cause, `<|bos|>` is malformed, and a tool call passes two more
checks, the cancel and the clock, before the tool runs.

<!-- source: psionic/lib/harness/loop.coax loop -->
```jai
// One turn. ids[0..<prompt] holds the rendered prompt; the loop appends each
// reply and each tool output after it and records one HlStep per request.
// Before each request it checks the cancel, the step count, and what the
// tokens, the clock and the window leave; a request whose worst case would
// pass the deadline gets a smaller limit, and one with no room is refused.
hl_run :: (m:HlModel,b:HlBudget,seed:u64,prompt:int,ids:[]int,steps:[]HlStep)->HlTurn {
    t:=HlTurn.{at=prompt,reply=prompt};context:=m.c.context;
    if prompt<1 || ids.count<context || b.steps<0 || steps.count<b.steps || b.reply<1 || b.tool<0 {t.stop=HlStop.shape;return t;}
    while true {
        k:=t.steps;
        if b.cancel==2*k {t.stop=HlStop.cancelled;return t;}
        if k==b.steps {t.stop=HlStop.steps;return t;}
        tokens:=b.tokens-t.tokens;time:=b.deadline-t.ticks-t.at+1;room:=context-t.at;
        limit:=hl_least(hl_least(b.reply,tokens),hl_least(time,room));
        if limit<1 {t.stop=hl_binding(0,tokens,time,room);return t;}
        s:=HlStep.{seen=t.at,seed=cr_bits(seed,cast(u64)k),limit=limit};
        g:=hl_reply(m,k,ids[0..<t.at],s.seed,limit,ids[t.at..<t.at+limit]);
        if g.status!=NeStatus.ok {t.stop=HlStop.model;return t;}
        s.written=g.written;s.stop=g.stop;s.positions=g.positions;s.draws=hl_draws(g);
        steps[k]=s;t.steps+=1;t.tokens+=s.draws;t.ticks+=g.positions;t.reply=t.at;t.at+=g.written;
        if g.stop==NeStop.assistant_end {t.stop=HlStop.answer;return t;}
        if g.stop==NeStop.limit {t.stop=hl_binding(limit,tokens,time,room);return t;}
        if g.stop!=NeStop.tool {t.stop=HlStop.malformed;return t;}
        // Between the reply and its tool: the cancel, then the clock.
        if b.cancel==2*k+1 {t.stop=HlStop.cancelled;return t;}
        if t.ticks+b.tool>b.deadline {t.stop=HlStop.deadline;return t;}
        u:=hl_tool(m.table,m.base,ids[t.reply..<t.at],ids[t.at..<context]);
        if u.stop!=HlStop.none {t.stop=u.stop;return t;}
        steps[k].value=u.value;steps[k].output=u.written;t.at+=u.written;t.ticks+=b.tool;t.tools+=1;
    }
    return t;
}
```

Cancellation here is a boundary: a cancel at boundary `2k` stops before
step `k`'s request, and one at `2k + 1` stops after step `k`'s reply and
before its tool. Either way no tool output is appended after the cancel.

## Replay

The record is the transcript and one `HlStep` per request. Replay rebuilds
each request from `ids[0..<seen]` and the step's seed, asks the same model
again, and requires the same reply token for token, the same stop, draws
and positions, the same tool value and output, and steps that tile the
transcript with nothing between them. Any changed token, field, seed or
script fails it.

<!-- source: psionic/lib/harness/loop.coax replay -->
```jai
// The record replays: step k's request, rebuilt from ids[0..<seen] and its
// seed, gives the recorded reply token for token, the tool gives the
// recorded output, and the steps tile the transcript with nothing between.
hl_replay :: (m:HlModel,seed:u64,prompt:int,ids:[]int,t:HlTurn,steps:[]HlStep,scratch:[]int)->bool {
    at:=prompt;
    for k:0..<t.steps {
        s:=steps[k];if s.seen!=at || s.seed!=cr_bits(seed,cast(u64)k) || s.limit>scratch.count {return false;}
        g:=hl_reply(m,k,ids[0..<at],s.seed,s.limit,scratch[0..<s.limit]);
        if g.status!=NeStatus.ok || g.stop!=s.stop || g.written!=s.written || g.positions!=s.positions || hl_draws(g)!=s.draws {return false;}
        for i:0..<g.written {if scratch[i]!=ids[at+i] {return false;}}
        at+=g.written;
        if s.output>0 {
            u:=hl_tool(m.table,m.base,ids[at-g.written..<at],scratch);
            if u.value!=s.value || u.written!=s.output {return false;}
            for i:0..<u.written {if scratch[i]!=ids[at+i] {return false;}}
            at+=u.written;
        }
        else if s.value!=0 {return false;}
    }
    return at==t.at;
}
```

## Run it

The example, [harness-loop.coax](../examples/harness-loop.coax), runs the
hand trace, each budget at its limit and one past it,
cancellation at each boundary, and then chapter 126's engine in the loop.
The engine's weights are chapter 136's programmed bigram model: every block
passes its input through, and seven token pairs make the model call `1+2`
after `<|assistant_start|>` and answer `3` after `<|output_end|>`. Greedy
decoding gives that exchange; sampling at temperature 5 from the two
likeliest tokens sometimes strays, and each seed's transcript replays from
that seed and not from the next one. The example ends with refusals.

<!-- source: psionic/examples/harness-loop.coax example -->
```jai
main :: () {
    tb:=NC_LEARNED;tr:=HL_TRACE;ts:=HL_TRACE_STARTS;ids:[64]int;steps:[4]HlStep;scratch:[64]int;
    m:=hl_scripted(tb[..],tr[..],ts[..],48);
    // The hand trace: two calls and an answer.
    t:=hl_trace(tb[..],HL_TRACE_BUDGET,48,ids[..],steps[..]);
    hl_show_turn("hand trace",t);hl_show_steps(tb[..],ids[..],t,steps[..]);
    print("  the third request saw: ");hl_show_text(tb[..],ids[0..<steps[2].seen]);print("\n");
    print("  replays from seed 7: %; from seed 8: %\n",hl_replay(m,7,5,ids[..],t,steps[..],scratch[..]),hl_replay(m,8,5,ids[..],t,steps[..],scratch[..]));
    // Each budget at its limit, then one past it.
    b:=HL_TRACE_BUDGET;
    for v:[2]int.[3,2] {b.steps=v;hl_show_label("steps",v);hl_show_turn("",hl_trace(tb[..],b,48,ids[..],steps[..]));}
    b=HL_TRACE_BUDGET;for v:[2]int.[17,16] {b.tokens=v;hl_show_label("tokens",v);hl_show_turn("",hl_trace(tb[..],b,48,ids[..],steps[..]));}
    b=HL_TRACE_BUDGET;for v:[4]int.[68,67,38,37] {b.deadline=v;hl_show_label("deadline",v);hl_show_turn("",hl_trace(tb[..],b,48,ids[..],steps[..]));}
    b=HL_TRACE_BUDGET;for v:[2]int.[7,6] {b.reply=v;hl_show_label("reply cap",v);hl_show_turn("",hl_trace(tb[..],b,48,ids[..],steps[..]));}
    for v:[4]int.[31,30,27,26] {hl_show_label("window",v);hl_show_turn("",hl_trace(tb[..],HL_TRACE_BUDGET,v,ids[..],steps[..]));}
    b=HL_TRACE_BUDGET;for v:[4]int.[0,1,2,3] {b.cancel=v;hl_show_label("cancel at boundary",v);hl_show_turn("",hl_trace(tb[..],b,48,ids[..],steps[..]));}
    // Chapter 126's engine: chapter 136's programmed bigram weights.
    c:=SF_TEST;w:[29416]f64;cache:[8000]f64;work:[12000]f64;logits:[21056]f64;last:[329]f32;pr:[329]f64;st:[329]f64;or:[329]int;te:[329]int;
    ww:=ne_buffers(c,64,cache[..],work[..],logits[..],last[..],pr[..],st[..],or[..],te[..]);pairs:=HL_PAIRS;sf_program(c,pairs[..],w[..]);
    e:=HlModel.{engine=true,c=c,base=320,table=tb[..],w=w[..],options=NxOptions.{temperature=0},work=ww};
    n:=hl_ask(e,"What is 1+2?",ids[..]);eb:=HlBudget.{steps=4,tokens=40,deadline=1000,reply=16,tool=0};
    t=hl_run(e,eb,5,n,ids[..],steps[..]);hl_show_turn("engine, greedy",t);hl_show_steps(tb[..],ids[..],t,steps[..]);
    print("  transcript: ");hl_show_text(tb[..],ids[0..<t.at]);print("\n");
    e.options=NxOptions.{temperature=5,top_k=2};p:=ids;
    for seed:0..<4 {a:=p;u:=hl_run(e,eb,cast(u64)seed,n,a[..],steps[..]);hl_show_label("engine, sampled, seed",seed);hl_show_turn("",u);
        print("  replays from its seed: %, from the next: %\n",hl_replay(e,cast(u64)seed,n,a[..],u,steps[..],scratch[..]),hl_replay(e,cast(u64)seed+1,n,a[..],u,steps[..],scratch[..]));}
    // Refusals: each runs nothing it should not.
    bad:=tr;bad[3]=47;t=hl_trace_script(tb[..],bad[..],ts[..],ids[..],steps[..]);
    hl_show_turn("a call 37/48",t);print("  reply: ");hl_show_text(tb[..],ids[5..<t.at]);print(", tool value %, appended %\n",steps[0].value,steps[0].output);
    bad=tr;bad[4]=HL_OUT;t=hl_trace_script(tb[..],bad[..],ts[..],ids[..],steps[..]);hl_show_turn("a special token inside the call",t);
    bad=tr;bad[13]=HL_BOS;t=hl_trace_script(tb[..],bad[..],ts[..],ids[..],steps[..]);hl_show_turn("a reply that draws <|bos|>",t);
    short:=[3]int.[0,7,13];t=hl_trace_script(tb[..],tr[..],short[..],ids[..],steps[..]);hl_show_turn("a model with no third reply",t);
    b=HL_TRACE_BUDGET;b.steps= -1;hl_show_turn("a budget of -1 steps",hl_trace(tb[..],b,48,ids[..],steps[..]));
    b=HL_TRACE_BUDGET;b.reply=0;hl_show_turn("a reply cap of 0",hl_trace(tb[..],b,48,ids[..],steps[..]));
    b=HL_TRACE_BUDGET;b.steps=5;hl_show_turn("4 step records for a 5-step budget",hl_trace(tb[..],b,48,ids[..],steps[..]));
}
```

```shell
coax/bin/coax run psionic/examples/harness-loop.coax
psionic/check.sh --only harness/loop
```

```text
hand trace: stop answer, steps 3, transcript 30 tokens, drawn 17, clock 68, tool calls 2
  step 0: saw 5 tokens, limit 8, drew 7 (7 kept), stop tool, 11 positions read, tool value 85, 4 tokens appended
    reply: <|python_start|>37+48<|python_end|>
  step 1: saw 16 tokens, limit 8, drew 6 (6 kept), stop tool, 21 positions read, tool value 170, 5 tokens appended
    reply: <|python_start|>85*2<|python_end|>
  step 2: saw 27 tokens, limit 4, drew 4 (3 kept), stop assistant_end, 30 positions read
    reply: 170
  the third request saw: <|bos|><|user_start|>?<|user_end|><|assistant_start|><|python_start|>37+48<|python_end|><|output_start|>85<|output_end|><|python_start|>85*2<|python_end|><|output_start|>170<|output_end|>
  replays from seed 7: true; from seed 8: false
steps 3: stop answer, steps 3, transcript 30 tokens, drawn 17, clock 68, tool calls 2
steps 2: stop steps, steps 2, transcript 27 tokens, drawn 13, clock 38, tool calls 2
tokens 17: stop answer, steps 3, transcript 30 tokens, drawn 17, clock 68, tool calls 2
tokens 16: stop tokens, steps 3, transcript 30 tokens, drawn 16, clock 67, tool calls 2
deadline 68: stop answer, steps 3, transcript 30 tokens, drawn 17, clock 68, tool calls 2
deadline 67: stop deadline, steps 3, transcript 30 tokens, drawn 16, clock 67, tool calls 2
deadline 38: stop deadline, steps 2, transcript 27 tokens, drawn 13, clock 38, tool calls 2
deadline 37: stop deadline, steps 2, transcript 22 tokens, drawn 13, clock 35, tool calls 1
reply cap 7: stop answer, steps 3, transcript 30 tokens, drawn 17, clock 68, tool calls 2
reply cap 6: stop length, steps 1, transcript 11 tokens, drawn 6, clock 10, tool calls 0
window 31: stop answer, steps 3, transcript 30 tokens, drawn 17, clock 68, tool calls 2
window 30: stop window, steps 3, transcript 30 tokens, drawn 16, clock 67, tool calls 2
window 27: stop window, steps 2, transcript 27 tokens, drawn 13, clock 38, tool calls 2
window 26: stop window, steps 2, transcript 22 tokens, drawn 13, clock 35, tool calls 1
cancel at boundary 0: stop cancelled, steps 0, transcript 5 tokens, drawn 0, clock 0, tool calls 0
cancel at boundary 1: stop cancelled, steps 1, transcript 12 tokens, drawn 7, clock 11, tool calls 0
cancel at boundary 2: stop cancelled, steps 1, transcript 16 tokens, drawn 7, clock 14, tool calls 1
cancel at boundary 3: stop cancelled, steps 2, transcript 22 tokens, drawn 13, clock 35, tool calls 1
engine, greedy: stop answer, steps 2, transcript 24 tokens, drawn 7, clock 43, tool calls 1
  step 0: saw 15 tokens, limit 16, drew 5 (5 kept), stop tool, 19 positions read, tool value 3, 3 tokens appended
    reply: <|python_start|>1+2<|python_end|>
  step 1: saw 23 tokens, limit 16, drew 2 (1 kept), stop assistant_end, 24 positions read
    reply: 3
  transcript: <|bos|><|user_start|>What is 1+2?<|user_end|><|assistant_start|><|python_start|>1+2<|python_end|><|output_start|>3<|output_end|>3
engine, sampled, seed 0: stop answer, steps 2, transcript 24 tokens, drawn 7, clock 43, tool calls 1
  replays from its seed: true, from the next: false
engine, sampled, seed 1: stop length, steps 1, transcript 31 tokens, drawn 16, clock 30, tool calls 0
  replays from its seed: true, from the next: false
engine, sampled, seed 2: stop length, steps 1, transcript 31 tokens, drawn 16, clock 30, tool calls 0
  replays from its seed: true, from the next: false
engine, sampled, seed 3: stop answer, steps 2, transcript 24 tokens, drawn 7, clock 43, tool calls 1
  replays from its seed: true, from the next: false
a call 37/48: stop malformed, steps 1, transcript 12 tokens, drawn 7, clock 11, tool calls 0
  reply: <|python_start|>37/48<|python_end|>, tool value 0, appended 0
a special token inside the call: stop malformed, steps 1, transcript 12 tokens, drawn 7, clock 11, tool calls 0
a reply that draws <|bos|>: stop malformed, steps 3, transcript 27 tokens, drawn 14, clock 65, tool calls 2
a model with no third reply: stop model, steps 2, transcript 27 tokens, drawn 13, clock 38, tool calls 2
a budget of -1 steps: stop shape, steps 0, transcript 5 tokens, drawn 0, clock 0, tool calls 0
a reply cap of 0: stop shape, steps 0, transcript 5 tokens, drawn 0, clock 0, tool calls 0
4 step records for a 5-step budget: stop shape, steps 0, transcript 5 tokens, drawn 0, clock 0, tool calls 0
```

The [transcript](../fixtures/harness-loop.out) is exact for the named build
in debug and release, and its hand-trace lines match the worked numbers
above. With greedy decoding the engine's first request sees the 15-token
prompt, draws the 5-token call, reads 19 positions, gets 3 back between the
output markers, and answers `3` on the second request. At temperature 5,
seeds 1 and 2 never close their call and stop at the 16-token cap. The last
eight lines are deliberately broken inputs: a call `37/48` and a call
holding `<|output_start|>` are `malformed`, with nothing run and nothing
appended (`tool value 0, appended 0`); a reply that draws `<|bos|>` is
`malformed`; a script with no third reply is the model's refusal; and an
invalid budget, a zero reply cap, or too few step records is refused as
`shape` before any request.

The tuned model from chapter 136's short run is not used here: it makes no
exact calls on held-out questions, so a run with it would show only the
`length` stop. The programmed weights make every step's outcome known in
advance while the engine still does the real forward pass.

## Checks

- **The hand trace:** every field of every step, and the 30 transcript
  tokens, as worked above.
- **Budget boundaries:** each budget at its limit and one below it, the
  deadline at 68, 67, 38, 37, 35, 34, 5 and 4, the
  window at 31, 30, 27, 26 and 5, and the order in which limits that bind
  together are named.
- **Cancellation** at boundaries 0 to 3, and none at a boundary the turn
  never reaches.
- **Refusals:** malformed calls of every kind, `<|bos|>`, a script that
  runs out or is out of range, and every invalid budget and buffer.
- **Replay:** the trace replays from seed 7 and not from 8; every changed
  transcript token, step field, turn length or script fails it.
- **A brute-force model of the loop:** 400 random scripts, each a few
  replies of five kinds (answer, valid call, refused call, a ramble past any
  limit, `<|bos|>`), under 400 random budgets, are run through `hl_run` and
  through `ho_loop` in
  [harness-loop-oracle.coax](../tests/harness-loop-oracle.coax).
  The oracle reasons only about each reply's kind and length, never its
  tokens, and must agree on the stop, steps, transcript length, drawn
  tokens, clock and tool calls; each run must also replay.
- **The engine:** greedy and sampled runs of chapter 126's engine, with
  their exact stops and lengths, and replay from each seed.

Mutation of `harness/loop.coax` leaves no survivors.

## What remains

- A model served through chapter 20's API, chapter 22's live service and
  chapter 79's Responses server, and a run against a pinned resident model
  such as gpt-oss-20b with its identity and build recorded. Here the model
  is called in process.
- A deadline on the wall clock. The logical clock makes every boundary
  exact; a served model needs chapter 28's tickets, deadlines and fencing of
  late responses.
- Cancellation that stops a model stream mid-reply and every running tool
  together. Here a cancel takes effect only at a step boundary.
- More than one tool, typed schemas and constrained arguments (chapter 203),
  a durable session log (chapter 201) and a sandbox (chapter 204).

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
