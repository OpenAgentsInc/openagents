# 8. Programs as tool calls

> **Status: partly implemented.** A model can write one small program
> instead of one tool call per request. The program binds call results to
> variables, branches on a boolean variable and repeats a block a fixed
> number of times, over chapter4's tools. A grammar built from the
> granted tools constrains decoding. Before anything runs, the program is
> parsed and checked against the grant, chapter5's policy and the log's
> room, using its worst case. An interpreter then sends each call through
> chapter5's sandbox with a step limit, and stops at the exact statement
> where a call asks, fails or is refused. The program, each call and its
> result, and the outcome are events in chapter2's log, and a replay
> rebuilds the same log and world. Still to come: a served model writing
> the programs, and the measurement of model requests, tokens and
> wall-clock time on a fixed task set against one call per request;
> resuming a program after an approval; host functions beyond chapter
> 203's four tools; and the uses in Psionic chapters 235 to 237.

Some tasks need many small tool calls. With one call per model request,
each call costs a round trip: the model writes the call, the harness runs
it, and the result goes back into the next request. This chapter lets the
model write one program instead. The harness checks the whole program,
runs it once, and returns one result.

A program is still only a sequence of tool calls. It has no ambient
authority: every call it makes goes through the same sandbox as a single
call, under the same policy, and is logged the same way. The program adds
variables, a condition and a bounded loop, so that one model request can
cover what would otherwise take several.

Everything is in one `#authority` file,
[harness/program.coax](../../psionic/lib/harness/program.coax), because it decides what
runs. Its checks leave no mutant alive.

## The profile

This chapter's profile is `cpu-harness-program-v1`:

- The tools are chapter4's four: `add` and `div` (integers of up to
  three digits, effect none), `get` (read a room's light) and `set` (write
  one). The world is chapter4's three rooms.
- A program has at most 12 statements and 384 bytes of text. It has eight
  variables, `x0` to `x7`.
- Blocks are one level deep. A loop runs 1 to 9 times.
- Time is chapter5's clock in ticks: `add` 1, `div` 2, `set` 3, `get` 1.

## The language

A program is a sequence of statements with no spaces:

- `x1=add{"a":1,"b":2};` calls a tool and binds its result to `x1`. The
  call is chapter4's call text, except that an integer or boolean
  argument may be a variable instead of a literal.
- `if x0{...}` runs the bind statements in the braces once if `x0` is true.
- `for 3{...}` runs them three times.

A block holds only bind statements. Choice arguments, such as a room, are
always literals, because no tool returns a room. That makes every call's
room known before the program runs.

The chapter's first program, `PG_ONE`:

```text
x1=add{"a":1,"b":2};for 3{x1=add{"a":x1,"b":x1};}x0=get{"room":"den"};if x0{x2=div{"a":x1,"b":4};}
```

It has six statements: a bind, a loop head, the bind inside the loop, a
bind, a condition, and the bind inside the condition.

## A grammar of programs

The program grammar is a byte automaton built with chapter4's builder
and compiled by Psionic chapter 53. Four states hold the shape of a program:
`top` before the first statement, `done` after a statement (the only
accepting state), `open` after a block opens, and `inner` after a statement
inside a block. A bind returns to `done` at the top level and to `inner`
inside a block, so the automaton has a copy of every call for each place.
`}` closes a block only from `inner`, so an empty block and a nested block
cannot be written.

<!-- source: psionic/lib/harness/program.coax grammar -->
```jai
// A value of type t after s, or a variable for an integer or a boolean:
// x and one digit.
pg_value :: (b:[]HtBuild,w:HtWork,s:int,t:HtType,ends:[]int)->int {
    n:=ht_value(b,w,s,t,ends);
    if t.kind!=HtKind.choice {d:=ht_state(b,w);ht_edge(b,w,ht_bytes(b,w,s,"x"),'0','7',d);ends[n]=d;n+=1;}
    return n;
}
// After s, a call of any offered tool, then ";" into the state to.
pg_calls :: (b:[]HtBuild,w:HtWork,s:int,tools:[]HtTool,offered:[]int,to:int) {
    for t:offered {
        ends:[HT_DIGITS_MAX+2]int;ends[0]=ht_bytes(b,w,s,tools[t].name);ne:=1;intro:="{\"";
        for f:tools[t].fields {
            x:=ht_join(b,w,ends[0..<ne],intro[0]);x=ht_bytes(b,w,ht_bytes(b,w,x,intro[1..<2]),f.name);
            ne=pg_value(b,w,ht_bytes(b,w,x,"\":"),f.type,ends[..]);intro=",\"";
        }
        if tools[t].fields.count==0 {ends[0]=ht_bytes(b,w,ends[0],"{");}
        ht_edge(b,w,ht_join(b,w,ends[0..<ne],'}'),';',';',to);
    }
}
// Programs over the offered tools. Four states hold the shape: before the
// first statement (top), after a statement (done, the only accepting one),
// after a block opens (open) and after a statement inside it (inner).
// "x0=" call ";" from top or done returns to done; from open or inner, to
// inner. "if x0{" and "for 3{" open a block; "}" closes it from inner.
pg_grammar :: (tools:[]HtTool,offered:[]int,w:HtWork)->HtGrammar {
    b:[1]HtBuild;top:=ht_state(b[..],w);done:=ht_state(b[..],w);open:=ht_state(b[..],w);inner:=ht_state(b[..],w);
    out:=[2]int.[top,done];inside:=[2]int.[open,inner];
    d:=ht_state(b[..],w);ht_edge(b[..],w,ht_join(b[..],w,out[..],'x'),'0','7',d);pg_calls(b[..],w,ht_bytes(b[..],w,d,"="),tools,offered,done);
    d=ht_state(b[..],w);ht_edge(b[..],w,ht_join(b[..],w,inside[..],'x'),'0','7',d);pg_calls(b[..],w,ht_bytes(b[..],w,d,"="),tools,offered,inner);
    d=ht_state(b[..],w);ht_edge(b[..],w,ht_bytes(b[..],w,ht_join(b[..],w,out[..],'i'),"f x"),'0','7',d);ht_edge(b[..],w,d,'{','{',open);
    d=ht_state(b[..],w);ht_edge(b[..],w,ht_bytes(b[..],w,ht_join(b[..],w,out[..],'f'),"or "),'1','9',d);ht_edge(b[..],w,d,'{','{',open);
    ht_edge(b[..],w,inner,'}','}',done);
    if b[0].states>w.owner.count || b[0].edges>w.edges.count {return HtGrammar.{why=HtWhy.capacity};}
    // As in chapter 203, state k is stored as last-k (top, state 0, becomes
    // last), so nearly every edge
    // points down and the liveness sweep settles in a few passes.
    last:=b[0].states-1;
    for i:0..<b[0].edges {x:=w.edges[i];w.edges[i]=NxEdge.{from=last-x.from,low=x.low,high=x.high,to=last-x.to};}
    final:=[1]int.[last-done];
    c:=nx_compile(b[0].states,last,w.edges[0..<b[0].edges],final[..],w.next,w.accept,w.live,false);
    if c.status!=NxStatus.ok {return HtGrammar.{why=HtWhy.capacity};}
    return HtGrammar.{grammar=c.grammar};
}
```

Over chapter4's four tools the grammar has 262 states and 317 edges.
Built from only the granted tools, it is the mask for decoding: the model
cannot write a call to a tool that is not granted, a variable in a room
argument, or a block inside a block.

## Decoding

The decoder is Psionic chapter 53's guided step over the program grammar. The
stand-in model here writes a fixed script, so the example is
deterministic. A served model would replace the score function; the mask
and the loop stay the same.

<!-- source: psionic/lib/harness/program.coax decode -->
```jai
// The stand-in model writes a script: a piece that continues the script
// scores its length, and every other piece 0. The end piece scores the
// script's length once the script is written, or once the model has
// written a piece the script did not have, after which no other piece
// scores. The grammar of the offered tools masks every draw, and the draw
// is greedy, so the lowest allowed piece wins a tie.
pg_decode :: (tools:[]HtTool,offered:[]int,script:string,pieces:[]string,end:int,limit:int,w:HtWork,sm:HtSampler)->PgDecode {
    c:=pg_grammar(tools,offered,w);if c.why!=HtWhy.none {return PgDecode.{status=NxStatus.grammar};}
    run:=NxRun.{limit=limit};state:=c.grammar.start;n:=0;on:=true;
    while run.stop==NxStop.running {
        for x,i:pieces {
            v:=0;
            if on && n+x.count<=script.count && x==script[n..<n+x.count] {v=x.count;}
            if i==end && (n==script.count || !on) {v=script.count;}
            sm.logits[nx_min(i,sm.logits.count-1)]=cast(f32)v;
        }
        d:=nx_guided(sm.logits,c.grammar,state,pieces,end,NxOptions.{temperature=0},run,false,sm.mask,sm.out,sm.work,false);
        if d.status!=NxStatus.ok {return PgDecode.{status=d.status};}
        if d.token!=end {
            x:=pieces[d.token];if n+x.count>sm.text.count {return PgDecode.{status=NxStatus.capacity};}
            on=on && sm.logits[d.token]>0;for y:x {sm.text[n]=y;n+=1;}
        }
        run=d.run;state=d.state;
    }
    return PgDecode.{tokens=run.emitted,count=n,done=run.stop==NxStop.stop_token};
}
```

Over every tool, the stand-in writes `PG_ONE` in 69 pieces: 68 pieces of
text, such as `add{`, `{"`, `":` and single bytes, and the end piece.
With only `add` and `get` granted, the script reaches byte 79, where it
has `div`. The mask allows only `a`, `g`, `add{` and `get{` there, so the
model goes off script, takes the lowest allowed piece at each step and ends
as soon as the program is complete.

## Parsing

The parser walks the program grammar over the registry's tools. It refuses
the first byte that cannot continue a program (`syntax`, with the byte),
text that stops before a program is complete (`incomplete`), and a
thirteenth statement (`statements`, with its first byte). Because the
grammar has accepted the text, the parser can read each statement where the
grammar put it.

<!-- source: psionic/lib/harness/program.coax parse -->
```jai
// A bind's statement from the byte after its "x": the variable, the tool by
// name, then each field's literal or variable. The grammar has already
// accepted the text, so each piece is where the grammar put it.
pg_bind :: (tools:[]HtTool,text:string,at:int)->PgStmt {
    st:=PgStmt.{var=cast(int)(text[at]-'0')};p:=at+2;e:=p;while text[e]!='{' {e+=1;}
    for x,i:tools {if x.name==text[p..<e] {st.tool=i;}}
    p=e+1;
    for f,i:tools[st.tool].fields {
        p+=f.name.count+3;
        if text[p]=='x' {st.refs[i]=cast(int)(text[p+1]-'0')+1;p+=2;}
        else if f.type.kind==HtKind.integer {while text[p]!=',' && text[p]!='}' {st.args[i]=st.args[i]*10+cast(int)(text[p]-'0');p+=1;}}
        else if f.type.kind==HtKind.boolean {st.args[i]=ifx text[p]=='t' then 1 else 0;p+=5-st.args[i];}
        else {q:=p;while text[q+1]!='"' {q+=1;}for x,k:f.type.choices {if x==text[p+1..<q+1] {st.args[i]=k;}}p=q+2;}
        p+=1;
    }
    return st;
}
// Parse a program: chapter 203's registry must be valid, then g, the
// program grammar over the registry's tools, accepts the text or names the
// byte where it stops being a program; then statements are read off in order,
// at most PG_STMTS of them. A block head records how many binds follow it.
pg_parse :: (tools:[]HtTool,g:NxGrammar,text:string)->PgParse {
    r:=PgParse.{};
    if text.count>PG_TEXT_MAX || ht_check(tools).why!=HtWhy.none {r.why=PgWhy.shape;return r;}
    s:=g.start;
    for x,i:text {n:=g.next[s*256+cast(int)x];if n<0 {r.why=PgWhy.syntax;r.at=i;return r;}s=n;}
    if g.accepting[s]!=1 {r.why=PgWhy.incomplete;r.at=text.count;return r;}
    p:=0;head:=-1;
    while p<text.count {
        if text[p]=='}' {p+=1;head= -1;continue;}
        k:=r.program.count;if k==PG_STMTS {r.why=PgWhy.statements;r.at=p;return r;}
        if text[p]=='x' {
            r.program.stmts[k]=pg_bind(tools,text,p+1);while text[p]!=';' {p+=1;}p+=1;
            if head>=0 {r.program.stmts[head].body+=1;}
        }
        else if text[p]=='i' {r.program.stmts[k]=PgStmt.{kind=PgKind.when,var=cast(int)(text[p+4]-'0')};head=k;p+=6;}
        else {r.program.stmts[k]=PgStmt.{kind=PgKind.repeat,count=cast(int)(text[p+4]-'0')};head=k;p+=6;}
        r.program.count=k+1;
    }
    return r;
}
```

For `PG_ONE` the statements are:

| Statement | Kind | Fields |
| --- | --- | --- |
| 0 | bind | `x1`, tool 0 (`add`), `a` = 1, `b` = 2 |
| 1 | repeat | 3 times, body of 1 statement |
| 2 | bind | `x1`, `add`, `a` = `x1`, `b` = `x1` |
| 3 | bind | `x0`, tool 3 (`get`), room 1 (`den`) |
| 4 | when | `x0`, body of 1 statement |
| 5 | bind | `x2`, tool 1 (`div`), `a` = `x1`, `b` = 4 |

## Checking before anything runs

The checker works from the parsed program and the session as it is now.
For each bind it checks the tool against the grant, the tool's effect
against the policy's ceiling, the tool's ticks against the policy's timeout,
and the call's room against the policy's reach. It also checks that each
variable the call reads is surely bound and has the field's type, and that
the bound variable keeps one type.

Then it adds the bind to the worst case: every `if` block taken and every
loop run in full. A variable's worst case is its type's largest value: 999
for a three-digit integer, `false` for a boolean (five bytes). After each
bind, the worst case so far must fit what is left of the policy's calls,
bytes and ticks, the grant's call and cost budgets, and the log's events
and tokens. The first statement where it does not fit is refused, with
the limit named.

A variable bound inside an `if` block is surely bound only inside that
block. One bound inside a loop is surely bound after the loop, because a
loop runs at least once.

<!-- source: psionic/lib/harness/program.coax check -->
```jai
// A variable of type got may fill a field of type want: the same kind, and
// an integer no wider. (The grammar has no variables in choice fields.)
pg_fits :: (want:HtType,got:HtType)->bool {return want.kind==got.kind && got.digits<=want.digits;}
pg_no :: (why:PgWhy,at:int,b:PgBound)->PgCheck {return PgCheck.{why=why,at=at,bound=b};}
// Check a parsed program of n bytes against the session before it runs.
// Each bind, in order: its tool granted, its effect and ticks inside the
// policy, its room in reach, each variable it reads surely bound and of the
// field's type, and its variable keeping one type. Then the worst case so
// far, with every block taken and every loop run in full, must fit what the
// session, the grant and the log have left. A variable bound inside an if
// block is surely bound only inside it; one bound in a loop, after it too.
pg_check :: (env:SbTools,s:SbSession,p:PgProgram,n:int,g:LgLog)->PgCheck {
    q:=s.policy;tools:=env.tools;gr:=env.grant;b:=PgBound.{events=2,tokens=PG_HEAD+n+PG_RESULT_SIZE};
    if !sb_valid(q) {return pg_no(PgWhy.policy,-1,b);}
    if s.pending {return pg_no(PgWhy.paused,-1,b);}
    if ht_grant(tools,gr).why!=HtWhy.none {return pg_no(PgWhy.grant,-1,b);}
    typed:[PG_VARS]int;worst:[PG_VARS]int;mask:=0;sure:int;times:=1;left:=0;keep:bool;
    for k:0..<p.count {
        st:=p.stmts[k];
        if st.kind!=PgKind.bind {
            if st.kind==PgKind.when {
                if ((mask>>st.var)&1)==0 {return pg_no(PgWhy.undefined,k,b);}
                if tools[typed[st.var]-1].result.kind!=HtKind.boolean {return pg_no(PgWhy.type,k,b);}
            }
            b.steps+=1;times=ifx st.kind==PgKind.repeat then st.count else 1;left=st.body;keep=st.kind==PgKind.repeat;sure=mask;
            continue;
        }
        t:=tools[st.tool];j:=-1;for x,i:gr.tools {if x==st.tool {j=i;}}
        if j<0 {return pg_no(PgWhy.ungranted,k,b);}
        if cast(u8)t.effect>cast(u8)q.ceiling {return pg_no(PgWhy.effect,k,b);}
        if SB_TICKS[cast(int)t.body]>q.timeout {return pg_no(PgWhy.timeout,k,b);}
        if !sb_in(q.reach,sb_room(tools,HtEvent.{tool=st.tool,values=st.args})) {return pg_no(PgWhy.scope,k,b);}
        for f,i:t.fields {
            v:=st.refs[i]-1;if v<0 {continue;}
            if ((mask>>v)&1)==0 {return pg_no(PgWhy.undefined,k,b);}
            if !pg_fits(f.type,tools[typed[v]-1].result) {return pg_no(PgWhy.type,k,b);}
        }
        if typed[st.var]!=0 && tools[typed[st.var]-1].result.kind!=t.result.kind {return pg_no(PgWhy.type,k,b);}
        if typed[st.var]!=0 && tools[typed[st.var]-1].result.digits!=t.result.digits {return pg_no(PgWhy.type,k,b);}
        out:[HT_TEXT_MAX]u8;most:=pg_render(t,st,worst,out[..]);
        typed[st.var]=st.tool+1;mask|=1<<st.var;worst[st.var]=0;for d:0..<t.result.digits {worst[st.var]=worst[st.var]*10+9;}
        b.calls+=times;b.steps+=times;b.uses[j]+=times;b.cost+=t.cost*times;b.ticks+=SB_TICKS[cast(int)t.body]*times;
        b.bytes+=most*times;b.events+=times;b.tokens+=(SB_CALL_HEAD+most)*times;
        if s.calls+b.calls>q.calls {return pg_no(PgWhy.calls,k,b);}
        if s.usage.calls[j]+b.uses[j]>gr.calls[j] {return pg_no(PgWhy.budget,k,b);}
        if s.usage.cost+b.cost>gr.cost {return pg_no(PgWhy.cost,k,b);}
        if s.bytes+b.bytes>q.bytes {return pg_no(PgWhy.bytes,k,b);}
        if s.clock+b.ticks>q.ticks {return pg_no(PgWhy.time,k,b);}
        if b.events>g.events.count-g.count || b.tokens>g.tokens.count-g.used {return pg_no(PgWhy.log,k,b);}
        left-=1;if left==0 {times=1;if !keep {mask=sure;}}
    }
    return pg_no(PgWhy.none,-1,b);
}
```

By hand, for `PG_ONE` in a new session under `PG_POLICY` (write at most,
hall and den only, reads there run freely, cost 1, 10 calls, 300 bytes, 40
ticks, 3 ticks a call):

| Statement | Times | Worst call text | Bytes | Ticks | Cost |
| --- | --- | --- | --- | --- | --- |
| 0 | 1 | `add{"a":1,"b":2}` | 16 | 1 | 1 |
| 2 | 3 | `add{"a":999,"b":999}` | 3 × 20 | 3 × 1 | 3 × 1 |
| 3 | 1 | `get{"room":"den"}` | 17 | 1 | 1 |
| 5 | 1 | `div{"a":999,"b":4}` | 18 | 2 | 1 |

The worst case is 6 calls, 111 bytes, 7 ticks and cost 6. Steps are one
per statement instance: 1 + 1 + 3 + 1 + 1 + 1 = 8. The log needs the
program event, 6 call events and the outcome event: 8 events. Each fits,
so the program is admitted.

## Running

The interpreter runs the statements in order. Each statement instance is
one step, and fuel limits the steps: when the steps reach the fuel, the
program stops before the next statement. A bind renders its call with the
variables' current values and sends it to chapter5's `sb_call`. A call
that runs binds its variable. A call that asks stops the program there and
leaves the request pending; a call whose body fails (division by zero, or a
result too wide for its type) stops it after the run; a call the sandbox
refuses stops it with the world unchanged.

<!-- source: psionic/lib/harness/program.coax render -->
```jai
// A call's text, as chapter 203 reads it, with each variable's value.
pg_render :: (t:HtTool,st:PgStmt,vals:[PG_VARS]int,out:[]u8)->int {
    n:=ht_put(out,ht_put(out,0,t.name),"{");
    for f,i:t.fields {
        if i>0 {n=ht_put(out,n,",");}
        n=ht_put(out,ht_put(out,ht_put(out,n,"\""),f.name),"\":");
        v:=st.args[i];if st.refs[i]>0 {v=vals[st.refs[i]-1];}
        if f.type.kind==HtKind.integer {n=ht_put_int(out,n,v);}
        else if f.type.kind==HtKind.boolean {n=ht_put(out,n,ifx v==1 then "true" else "false");}
        else {n=ht_put(out,ht_put(out,ht_put(out,n,"\""),f.type.choices[v]),"\"");}
    }
    return ht_put(out,n,"}");
}
```

<!-- source: psionic/lib/harness/program.coax run -->
```jai
// One statement instance: stop if the steps have reached fuel; a block head
// is a step and nothing else; a bind renders its call with the variables'
// values and sends it to the sandbox. A call that runs binds its variable;
// one that asks, fails in its body or is refused stops the program there.
pg_step :: (env:SbTools,s:[]SbSession,st:PgStmt,k:int,world:[]int,l:[]LgLog,fuel:int,r:PgRun)->PgRun {
    x:=r;
    if x.steps==fuel {x.stop=PgStop.fuel;x.at=k;return x;}
    x.steps+=1;if st.kind!=PgKind.bind {return x;}
    b:[HT_TEXT_MAX]u8;n:=pg_render(env.tools[st.tool],st,x.vars,b[..]);
    e:=sb_call(env,s,cast(string)b[0..<n],world,l);x.event=e;x.calls+=1;
    if e.verdict==SbVerdict.ask {x.stop=PgStop.asked;}
    else if e.verdict==SbVerdict.refuse {x.stop=PgStop.stopped;}
    else if e.call.why!=HtWhy.none {x.stop=PgStop.failed;}
    else {x.vars[st.var]=e.call.value;x.defined|=1<<st.var;return x;}
    x.at=k;return x;
}
// Run a program statement by statement. A when block's body runs once if
// its variable is true; a repeat block's body runs count times.
pg_exec :: (env:SbTools,s:[]SbSession,p:PgProgram,world:[]int,l:[]LgLog,fuel:int)->PgRun {
    r:=PgRun.{stop=PgStop.done};k:=0;
    while k<p.count && r.stop==PgStop.done {
        st:=p.stmts[k];r=pg_step(env,s,st,k,world,l,fuel,r);
        if st.kind==PgKind.bind {k+=1;continue;}
        times:=st.count;if st.kind==PgKind.when {times=ifx r.vars[st.var]!=0 then 1 else 0;}
        for i:0..<times {for j:k+1..<k+1+st.body {if r.stop==PgStop.done {r=pg_step(env,s,p.stmts[j],j,world,l,fuel,r);}}}
        k+=1+st.body;
    }
    return r;
}
// A program from its text. Refused before it enters the log when it is too
// long, fuel is negative, the world is too small or the log has no room for
// it. Otherwise it is logged with the reason it was refused, if any: its
// text, the fuel, and the byte or statement. An admitted program runs, and
// its outcome is logged after its calls: the stop, the sandbox's reason,
// the statement, the steps, the calls, which variables are bound and their
// values.
pg_program :: (env:SbTools,g:NxGrammar,s:[]SbSession,text:string,fuel:int,world:[]int,l:[]LgLog)->PgRun {
    r:=PgRun.{};
    if text.count>PG_TEXT_MAX || fuel<0 || world.count<HT_CHOICES_MAX {r.why=PgWhy.shape;return r;}
    if !sb_fits(l,PG_HEAD+text.count) {r.why=PgWhy.log;return r;}
    q:=pg_parse(env.tools,g,text);r.why=q.why;r.at=q.at;
    if q.why==PgWhy.none {c:=pg_check(env,s[0],q.program,text.count,l[0]);r.why=c.why;r.at=c.at;r.bound=c.bound;}
    t:[PG_HEAD+PG_TEXT_MAX]int;head:=[PG_HEAD]int.[PG_PROGRAM,s[0].id,fuel,cast(int)r.why,r.at];
    for x,i:head {t[i]=x;}for c,i:text {t[PG_HEAD+i]=cast(int)c;}
    sb_say(l,t[0..<PG_HEAD+text.count]);
    if r.why!=PgWhy.none {return r;}
    x:=pg_exec(env,s,q.program,world,l,fuel);x.bound=r.bound;
    o:[PG_RESULT_SIZE]int;head2:=[8]int.[PG_RESULT,s[0].id,cast(int)x.stop,cast(int)x.event.why,x.at,x.steps,x.calls,x.defined];
    for v,i:head2 {o[i]=v;}for v,i:x.vars {o[8+i]=v;}
    sb_say(l,o[..]);return x;
}
```

`pg_exec` can also run a program that was never checked. The sandbox still
decides every call, so the program stops at the first call it refuses; the
checks run one that names `study`, outside the policy's reach, and it stops
at that statement with the light unchanged.

## By hand: one session

Session 1 runs under `PG_POLICY` with every tool granted 9 calls and a cost
budget of 99, all lights off and fuel 20. The clock, calls and bytes are
the session's state after each step.

| Step | Statement | Call | Outcome | Clock | Calls | Bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | `add{"a":1,"b":2}` | run, `x1` = 3 | 1 | 1 | 16 |
| 2 | 1 | | loop head | 1 | 1 | 16 |
| 3 | 2 | `add{"a":3,"b":3}` | run, `x1` = 6 | 2 | 2 | 32 |
| 4 | 2 | `add{"a":6,"b":6}` | run, `x1` = 12 | 3 | 3 | 48 |
| 5 | 2 | `add{"a":12,"b":12}`, 18 bytes | run, `x1` = 24 | 4 | 4 | 66 |
| 6 | 3 | `get{"room":"den"}` | read, den is open: run, `x0` = 0 | 5 | 5 | 83 |
| 7 | 4 | | `x0` is false: skip the body | 5 | 5 | 83 |

The program is done after 7 steps and 5 calls, with `x0` = 0 and `x1` = 24
bound; `x2` never is. One model request did the work of five calls.

The second program, `PG_TWO`, is
`x4=get{"room":"hall"};x5=set{"room":"den","on":true};x6=get{"room":"den"};`.
Its worst case is 3 calls, 62 bytes and 5 ticks, which fit what is left (5
calls, 217 bytes, 35 ticks). `get` in the hall runs: clock 6, calls 6,
bytes 101. `set` is a write, above what runs freely, so the sandbox asks:
request 0, deadline 6 + 5 = 11. The program stops at statement 1 after 2
steps; `x6=get` is never sent.

While the request waits, a third program is refused as `paused` before it
runs. The approval at 8 runs the paused `set` through chapter5's
`sb_answer`: the clock goes to 8 + 3 = 11, calls to 7, bytes to 128, and the
den's light is on.

Then five programs are refused before they run. Each logs one event and
changes nothing:

| Program | Refused | At |
| --- | --- | --- |
| `x0=set{"room":"study","on":true};` | `scope`: study is outside reach | statement 0 |
| `x7=get{"room":"hall"};for 3{x3=add{"a":1,"b":1};}` | `calls`: 7 + 1 + 3 = 11 > 10 | statement 2 |
| `x0=get{"room":"hall"};x1=add{"a":x0,"b":1};` | `type`: a boolean in an integer field | statement 1 |
| `x0=get{"room":"hall"};if x0{x3=add{"a":1,"b":1};}x2=add{"a":x3,"b":1};` | `undefined`: `x3` is bound only inside the `if` | statement 3 |
| `x0=get{"room":"attic"};` | `syntax`: no room starts with `a` | byte 15 |

## Every step is logged

A program's event holds `4`, the session, the fuel, why it was refused (0
when it was not) and the byte or statement, then the program text, one
token per byte. Each call it sends is chapter5's call event, with the
call's text and its result. After the calls comes the outcome event: `5`,
the session, the stop, the sandbox's reason for the last call, the
statement, the steps, the calls, a mask of the bound variables, and the
eight variables. For `PG_ONE` the outcome is
`5 1 1 0 -1 7 5 3 0 24 0 0 0 0 0 0`; for `PG_TWO` it is
`5 1 3 10 1 2 2 16 0 0 0 0 0 0 0 0` (stop `asked`, reason `effect`).

Replay verifies the chain, opens a fresh session from the first event as
chapter5's replay does, then feeds it each logged program, with its
fuel, and each answer. Every event the fresh session logs must have the
original's digest, so a program's calls and outcome are compared as well
as the program.

<!-- source: psionic/lib/harness/program.coax replay -->
```jai
// Replay a session's log: verify the chain; open a fresh session from its
// first event, as chapter 204's replay does; then feed it each logged
// program, with its fuel, and each answer. Every event the fresh session
// logs must have the original's digest, so a program's calls and outcome
// are compared too. The world the replay leaves is the world the session
// left.
pg_replay :: (env:SbTools,gm:NxGrammar,g:LgLog,s:[]SbSession,world:[]int,l:[]LgLog)->SbCheck {
    if lg_verify(g).status!=LgStatus.ok || l[0].count!=0 {return SbCheck.{status=SbReplay.log};}
    i:=0;
    while i<g.count {
        e:=g.events[i];t:=sb_tokens(g,i);
        if e.kind!=LgKind.context || e.redacted {return SbCheck.{status=SbReplay.event,index=i};}
        if i==0 {
            one:=g;one.count=1;one.used=e.count;k:=sb_replay(env,one,s,world,l);
            if k.status!=SbReplay.ok {return k;}
        }
        else if t.count>=PG_HEAD && t.count<=PG_HEAD+PG_TEXT_MAX && t[0]==PG_PROGRAM {
            b:[PG_TEXT_MAX]u8;n:=t.count-PG_HEAD;
            for x,j:t[PG_HEAD..<t.count] {if x<0 || x>255 {return SbCheck.{status=SbReplay.event,index=i};}b[j]=cast(u8)x;}
            r:=pg_program(env,gm,s,cast(string)b[0..<n],t[2],world,l);
        }
        else if t.count==SB_ANSWER_SIZE && t[0]==SB_ANSWER && t[3]>=0 && t[3]<=3 {
            a:=sb_answer(env,s,SbAnswer.{session=t[1],request=t[2],decision=cast(SbDecision)cast(u8)t[3],pattern=t[4]==1,at=t[5]},world,l);
        }
        else {return SbCheck.{status=SbReplay.event,index=i};}
        if l[0].count<=i || l[0].count>g.count {return SbCheck.{status=SbReplay.differs,index=i};}
        for j:i..<l[0].count {if !su_same(l[0].events[j].digest,g.events[j].digest) {return SbCheck.{status=SbReplay.differs,index=j};}}
        i=l[0].count;
    }
    return SbCheck.{index=g.count};
}
```

## The checks

- **The interpreter against a small-step reference.** A separate file,
  [harness-program-oracle.coax](../../psionic/tests/harness-program-oracle.coax),
  generates random programs as statements and writes them as text with its
  own writer. Its reference interpreter is a machine of program counter,
  open block and iterations left, one action per step, with its own call
  writer. A `#forall` over 10 seeds parses each text back to the same
  statements, runs it both ways with random fuel and lights, and compares
  the stop, statement, steps, calls, variables and world, and each call
  event's tokens. The log then replays to the same world.
- **The checker never admits what the sandbox would refuse.** A `#forall`
  over 16 seeds draws a random valid policy, random grant budgets, a
  session that has already spent some of them, and a random program, some
  of them ill-typed or naming rooms outside reach. A refused program changes
  nothing and logs one event. An admitted one is never refused by the
  sandbox, and its calls, steps, bytes, ticks, cost, events and tokens stay
  within its worst case. A further check shows the checker is not vacuous:
  of 40 random programs, at least 10 are admitted and 10 refused.
- **The grammar.** Programs it accepts and 13 it refuses, the grammar of a
  subset of tools, buffers one state or one edge short, a tool with no
  fields, and a boolean field before another field.
- **The hand trace**, step for step: each call event's result, the
  outcome events' tokens, the ask, the paused program, the approval, each
  refusal's reason and statement, and the replay's world and head digest.
- **Every refusal.** Each reason the checker gives, with each limit tested
  one over and exactly at the limit; a program too long, negative fuel, a
  world too small, and a log with one token too few for the program event.
- **Fuel.** `PG_ONE` with fuel 0 to 8 stops at statements 0, 1, 2, 2, 2,
  3 and 4 after exactly that many steps, and is done from fuel 7. A loop
  stops inside its body.
- **Decoding.** The script is written exactly over every tool. With `add`
  and `get` granted the decoder cannot write `div`. A `#forall` over 4
  seeds decodes random programs under random tool subsets: whatever it
  writes parses and names only offered tools.
- **Replay refusals.** Twenty-one forged logs, each rechained, change one
  token, drop tokens or change an event's kind, and are refused at the
  event they changed or replay when the change does not alter what the
  session would log. An edit without rechaining, a redacted program, a
  stray call event, a log cut after a program's calls, a program event one
  token too long, and logs too small are refused too.
- **Mutation.** `coax mutate --root-only` leaves no survivor in
  `harness/program.coax`.

## Run it

The example, [program-tool-calls.coax](../../psionic/examples/program-tool-calls.coax),
decodes `PG_ONE` under two grants, runs the hand trace's session and prints
every call from the log, then verifies and replays the log. It shows fuel
6 and 7, an unchecked program stopped by the sandbox, and a forged log
whose first program's fuel was lowered to 1.

<!-- source: psionic/examples/program-tool-calls.coax example -->
```jai
main :: () {
    bx:[1]PgBox;env:=pg_box(bx[..],99);p:=PG_POLICY;all:=PG_ALL;two:=[2]int.[0,3];pc:=PG_PIECES;gw:=pg_gwork(bx[..]);
    print("policy: ceiling %, reach %, allow %, open %, cost %, calls %, bytes %, ticks %, timeout %, wait %\n",
        p.ceiling,p.reach,p.allow,p.open,p.cost,p.calls,p.bytes,p.ticks,p.timeout,p.wait);
    // The stand-in model writes the hand trace's program under the grant's grammar.
    lg:[59]f32;m:[59]u8;pr:[59]f64;st:[59]f64;od:[59]int;tm:[59]int;tx:[400]u8;sm:=ht_sampler(lg[..],m[..],pr[..],st[..],od[..],tm[..],tx[..]);
    d:=pg_decode(env.tools,all[..],PG_ONE,pc[..],PG_END,200,gw,sm);
    print("decoded with every tool granted: % pieces, % bytes, complete %, same as the script %\n",d.tokens,d.count,d.done,cast(string)tx[0..<d.count]==PG_ONE);
    d=pg_decode(env.tools,two[..],PG_ONE,pc[..],PG_END,200,gw,sm);
    print("decoded with add and get granted: byte 79 is '%' where the script has 'd' (div is not offered)\n",cast(string)tx[79..<80]);
    // The program grammar over every tool, which the parser walks.
    gm:=pg_grammar(env.tools,all[..],gw).grammar;print("program grammar: % states\n",gm.states);
    // The hand trace: one session, two programs, an approval, then refusals.
    b:[1]PgLogs;l:=pg_logs(b[..]);s:[1]SbSession;w:[8]int;
    print("open session 1: %\n",sb_open(1,p,s[..],l[..]));
    r:=ex_run(env,gm,s[..],PG_ONE,20,w[..],l[..]);
    r=ex_run(env,gm,s[..],PG_TWO,20,w[..],l[..]);
    r=ex_run(env,gm,s[..],"x0=get{\"room\":\"hall\"};",20,w[..],l[..]);
    x:=sb_answer(env,s[..],sb_yes(1,0,8),w[..],l[..]);
    print("answer request 0, approve at 8: % %, den %  [clock %, calls %, bytes %]\n",x.verdict,x.why,w[1],s[0].clock,s[0].calls,s[0].bytes);
    bad:=[5]string.["x0=set{\"room\":\"study\",\"on\":true};","x7=get{\"room\":\"hall\"};for 3{x3=add{\"a\":1,\"b\":1};}",
        "x0=get{\"room\":\"hall\"};x1=add{\"a\":x0,\"b\":1};","x0=get{\"room\":\"hall\"};if x0{x3=add{\"a\":1,\"b\":1};}x2=add{\"a\":x3,\"b\":1};","x0=get{\"room\":\"attic\"};"];
    for t:bad {r=ex_run(env,gm,s[..],t,20,w[..],l[..]);}
    print("world: hall %, den %, study %\n",w[0],w[1],w[2]);
    v:=lg_verify(l[0]);print("log: % events, % tokens, verify %\n",l[0].count,l[0].used,v.status);
    b2:[1]PgLogs;l2:=pg_logs(b2[..]);s2:[1]SbSession;w2:[8]int;k:=pg_replay(env,gm,l[0],s2[..],w2[..],l2[..]);
    print("replay: % through event %, same head %, world hall %, den %, study %, clock %\n",k.status,k.index,su_same(lg_verify(l2[0]).head,v.head),w2[0],w2[1],w2[2],s2[0].clock);
    // Fuel: the first program takes 7 steps.
    for fuel:6..<8 {
        b3:[1]PgLogs;l3:=pg_logs(b3[..]);s3:[1]SbSession;w3:[8]int;x2:=sb_open(2,p,s3[..],l3[..]);r=pg_program(env,gm,s3[..],PG_ONE,fuel,w3[..],l3[..]);
        print("fuel %: %",fuel,r.stop);if r.at>=0 {print(" at statement %",r.at);}print(" after % steps, % calls\n",r.steps,r.calls);
    }
    // Unchecked, a program reaches the sandbox, which stops it at the call it refuses.
    b4:[1]PgLogs;l4:=pg_logs(b4[..]);s4:[1]SbSession;w4:[8]int;x3:=sb_open(3,p,s4[..],l4[..]);
    q:=pg_parse(env.tools,gm,"x0=get{\"room\":\"hall\"};x1=set{\"room\":\"study\",\"on\":true};x2=get{\"room\":\"hall\"};");
    r=pg_exec(env,s4[..],q.program,w4[..],l4[..],20);
    print("unchecked: % at statement %, sandbox %, % calls sent, study %\n",r.stop,r.at,r.event.why,r.calls,w4[2]);
    // A forged log: the first program's fuel lowered to 1 and the chain rebuilt.
    f:[1]PgLogs;lf:=pg_logs(f[..]);sb_forge(l[0],1,2,1,-1,LgKind.context,lf[..]);
    b5:[1]PgLogs;l5:=pg_logs(b5[..]);s5:[1]SbSession;w5:[8]int;k=pg_replay(env,gm,lf[0],s5[..],w5[..],l5[..]);
    print("event 1's fuel rewritten from 20 to 1, rechained: verify %, replay % at event %\n",lg_verify(lf[0]).status,k.status,k.index);
}
```

```shell
coax/bin/coax run psionic/examples/program-tool-calls.coax
psionic/check.sh --only harness/program
```

```text
policy: ceiling write, reach 3, allow read, open 3, cost 1, calls 10, bytes 300, ticks 40, timeout 3, wait 5
decoded with every tool granted: 69 pieces, 98 bytes, complete true, same as the script true
decoded with add and get granted: byte 79 is 'a' where the script has 'd' (div is not offered)
program grammar: 262 states
open session 1: none
program x1=add{"a":1,"b":2};for 3{x1=add{"a":x1,"b":x1};}x0=get{"room":"den"};if x0{x2=div{"a":x1,"b":4};}
  admitted: at most 6 calls, 8 steps, 111 bytes, 7 ticks, cost 6
  call add{"a":1,"b":2}: run none, result 3
  call add{"a":3,"b":3}: run none, result 6
  call add{"a":6,"b":6}: run none, result 12
  call add{"a":12,"b":12}: run none, result 24
  call get{"room":"den"}: run none, result 0
  done after 7 steps, 5 calls; bound x0 0 x1 24  [clock 5, calls 5, bytes 83]
program x4=get{"room":"hall"};x5=set{"room":"den","on":true};x6=get{"room":"den"};
  admitted: at most 3 calls, 3 steps, 62 bytes, 5 ticks, cost 4
  call get{"room":"hall"}: run none, result 0
  call set{"room":"den","on":true}: ask effect, request 0
  asked at statement 1 after 2 steps, 2 calls; bound x4 0  [clock 6, calls 6, bytes 101]
program x0=get{"room":"hall"};
  refused paused at -1, logged as event 12
answer request 0, approve at 8: run none, den 1  [clock 11, calls 7, bytes 128]
program x0=set{"room":"study","on":true};
  refused scope at 0, logged as event 14
program x7=get{"room":"hall"};for 3{x3=add{"a":1,"b":1};}
  refused calls at 2, logged as event 15
program x0=get{"room":"hall"};x1=add{"a":x0,"b":1};
  refused type at 1, logged as event 16
program x0=get{"room":"hall"};if x0{x3=add{"a":1,"b":1};}x2=add{"a":x3,"b":1};
  refused undefined at 3, logged as event 17
program x0=get{"room":"attic"};
  refused syntax at 15, logged as event 18
world: hall 0, den 1, study 0
log: 19 events, 698 tokens, verify ok
replay: ok through event 19, same head true, world hall 0, den 1, study 0, clock 11
fuel 6: fuel at statement 4 after 6 steps, 5 calls
fuel 7: done after 7 steps, 5 calls
unchecked: stopped at statement 1, sandbox scope, 2 calls sent, study 0
event 1's fuel rewritten from 20 to 1, rechained: verify ok, replay differs at event 3
```

The forged log verifies, because its chain was rebuilt. Replay still
refuses it: with fuel 1 the first program stops after its first call, so
the fresh session logs the outcome at event 3, where the original has the
second call.

## What remains

- A served model writing programs, and the measurement this chapter's plan
  asks for: model requests, tokens and wall-clock time on a fixed task set,
  programs against one call per request. Here the stand-in model writes a
  fixed script, and the only measurement is that one request covered five
  calls.
- Resuming a program after an approval. Here a program that asks stops;
  the approval runs that one call, and the rest of the program does not run.
- Limits on a program's output bytes and memory as their own policy fields.
  Here the bounds are the statement, variable and text limits of the
  profile.
- Host functions beyond chapter4's four tools, and the programs of
  chapters [235](../../psionic/book/ch235-can-gradient-descent-learn-exact-execution.md),
  [236](../../psionic/book/ch236-a-frozen-exact-core-with-a-learned-interface.md) and
  [237](../../psionic/book/ch237-composable-weight-modules.md).

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- [Chapter5](ch005-a-sandbox-and-approvals.md): the sandbox every call
  goes through.
- [Chapter4](ch004-tools-as-typed-capabilities.md): the tools, their
  grammar builder and the grant.
- [Chapter2](ch002-model-visible-means-logged.md): the log.
- [Psionic chapter 53](../../psionic/book/ch053-choosing-the-next-token.md): the byte automaton and
  the guided step.
