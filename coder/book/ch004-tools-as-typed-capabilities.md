# 4. Tools as typed capabilities

> **Status: partly implemented.** Tools run on the CPU as typed
> capabilities: each one declares a name, typed arguments, a typed result,
> an effect class and a cost. A grant for the turn names the tools, a call
> budget for each, a cost budget and an effect ceiling. The declarations
> compile to Psionic chapter 53's byte automaton, so masked sampling cannot produce
> an ungranted tool or arguments of the wrong type. Every call goes through
> before, run and after stages, and every refusal is a typed error that
> names the reason, the tool, the field and the byte. Still to come:
> declarations as versioned messages in `proto/`; filesystem and process
> tools (the shared world here is three light switches); these calls
> inside chapter1's token loop on Psionic chapter 126's engine (the turns here
> come from a stand-in model and contain only calls); and logging each
> event through chapter2.

Chapter1's loop has one tool, the calculator, and its grammar is written
into the loop. A harness needs more than one tool, and it needs to answer
three questions about each call before anything runs. Is the model allowed
to call this tool in this turn? Are the arguments the right types? Can the
turn still pay for the call? This chapter makes a tool a declaration
instead of a name. The declaration answers the second question. A grant
answers the first and third.

The declaration also controls generation. Every declaration compiles to a
byte automaton, and Psionic chapter 53's mask lets the model draw only pieces that
keep the call parseable. The harness still checks every call before it
runs, because calls can also come from places the mask does not cover,
such as a client or a script.

Everything is in one `#authority` file,
[harness/tools.coax](../../psionic/lib/harness/tools.coax), because it decides what
runs. Its checks leave no mutant alive.

## The profile

This chapter's profile is `cpu-harness-tools-v1`:

- A call is a tool name followed by a canonical JSON object: no
  whitespace, fields in declared order, every field required. For example,
  `add{"a":37,"b":48}`.
- An argument or result type is one of three kinds. An integer has 1 to 6
  decimal digits, no sign and no leading zero. A boolean is `true` or
  `false`. A choice is one of 1 to 8 words.
- A registry has 1 to 8 tools, and a tool has at most 4 fields. Names,
  field names and words are 1 to 16 bytes of `a`–`z` and `_`. A tool's
  cost is 0 to 1,000, and a call is at most 256 bytes.
- Effects are ordered: `none`, `read`, `write`. A grant's ceiling admits
  every tool at or below it.
- The model in this chapter's turns is a stand-in. It gives seeded random
  logits over 52 pieces of at most 8 bytes each. That is the worst case for
  a grammar, because the model has no preference for valid text.

## The registry

The chapter's registry has four tools that share one world, a switch per
room:

| Tool | Arguments | Result | Effect | Cost |
| --- | --- | --- | --- | --- |
| `add` | `a`, `b`: integer, 3 digits | integer, 3 digits | none | 1 |
| `div` | `a`, `b`: integer, 3 digits | integer, 3 digits | none | 1 |
| `set` | `room`: `hall`, `den` or `study`; `on`: boolean | boolean | write | 2 |
| `get` | `room`: `hall`, `den` or `study` | boolean | read | 1 |

The registry is checked before anything else uses it. Names must be well
formed and distinct, fields must be distinct within their tool, every type
must be in the profile, and costs must be in range. A refusal names the
tool and, for a field, the field. The grant is checked the same way: each
entry names a distinct tool of the registry, no budget is negative, and no
granted tool's effect is above the ceiling.

<!-- source: psionic/lib/harness/tools.coax check -->
```jai
// A name or a word: 1 to 16 bytes of a-z and _.
ht_word :: (s:string)->bool {
    if s.count<1 || s.count>HT_WORD_MAX {return false;}
    for c:s {if !((c>='a' && c<='z') || c=='_') {return false;}}
    return true;
}
ht_type :: (t:HtType)->bool {
    if t.kind==HtKind.integer {return t.digits>=1 && t.digits<=HT_DIGITS_MAX;}
    if t.kind==HtKind.boolean {return true;}
    if t.choices.count<1 || t.choices.count>HT_CHOICES_MAX {return false;}
    for x,i:t.choices {if !ht_word(x) {return false;}for k:0..<i {if t.choices[k]==x {return false;}}}
    return true;
}
// The registry: names distinct and well formed, fields distinct and typed,
// results typed, costs bounded. field -1 is the tool's own declaration.
ht_check :: (tools:[]HtTool)->HtEvent {
    if tools.count<1 || tools.count>HT_TOOLS_MAX {return HtEvent.{why=HtWhy.shape};}
    for t,i:tools {
        bad:=!ht_word(t.name) || !ht_type(t.result) || t.cost<0 || t.cost>HT_COST_MAX;
        for k:0..<i {if tools[k].name==t.name {bad=true;}}
        if bad || t.fields.count>HT_FIELDS_MAX {return HtEvent.{why=HtWhy.schema,tool=i};}
        for f,j:t.fields {
            same:=false;for k:0..<j {if t.fields[k].name==f.name {same=true;}}
            if same || !ht_word(f.name) || !ht_type(f.type) {return HtEvent.{why=HtWhy.schema,tool=i,field=j};}
        }
    }
    return HtEvent.{};
}
// The grant: each entry a distinct tool of the registry, budgets not
// negative, and no granted tool's effect above the ceiling.
ht_grant :: (tools:[]HtTool,g:HtGrant)->HtEvent {
    if g.tools.count!=g.calls.count || g.tools.count>HT_TOOLS_MAX {return HtEvent.{why=HtWhy.shape};}
    if g.cost<0 {return HtEvent.{why=HtWhy.grant};}
    for t,j:g.tools {
        twice:=false;for k:0..<j {if g.tools[k]==t {twice=true;}}
        if t<0 || t>=tools.count || twice || g.calls[j]<0 {return HtEvent.{why=HtWhy.grant,tool=t};}
        if cast(u8)tools[t].effect>cast(u8)g.effects {return HtEvent.{why=HtWhy.effect,tool=t};}
    }
    return HtEvent.{};
}
```

## A declaration becomes a grammar

The builder creates states in order and adds one edge for each byte a call
can contain. A name or a word is spelled one byte at a time with
`ht_follow`. That function reuses an edge that already exists, so `add`
and `ad` share their first two states. A fixed piece of text, such as
`{"room":`, is spelled the same way. An integer of `d` digits has `d + 1`
end states: `0` ends at once, and `1`–`9` starts a run of up to `d`
digits. That is why `01` cannot be spelled. A boolean has two end states.
A choice ends at one shared closing quote. Each state records which field
it belongs to: `-1` while the name is spelled, then `i` from the separator
before field `i`.

**By hand: `get`.** The builder makes these states, in this order:

| States | Bytes that lead there |
| --- | --- |
| 0 | the start |
| 1–3 | `g`, `e`, `t` |
| 4–5 | `{`, `"` (field 0 starts at 4) |
| 6–9 | `r`, `o`, `o`, `m` |
| 10–12 | `"`, `:`, `"` |
| 13–16 | `h`, `a`, `l`, `l` |
| 17–19 | `d`, `e`, `n` |
| 20–24 | `s`, `t`, `u`, `d`, `y` |
| 25 | the closing `"`, from 16, 19 and 24 |
| 26 | `}`, the only accepting state |

That is 27 states. Every state except the start has one incoming edge,
except state 25, which has three, so the grammar has 28 edges. The builder
counts states and edges that do not fit its buffers instead of keeping
them, and the count refuses the grammar as `capacity`. The checks build
`get` with exactly 28 edge slots and 27 state slots, then with one fewer
of each.

The table stores state `k` as `26 - k`. Every edge then points to a lower
state. Psionic chapter 53's liveness sweep runs upward, so it finishes in one pass
instead of one pass per state. The start is therefore state 26 in the
table and the accepting state is 0.

For a grant, the grammar is the union of the calls of every tool offered:
the names share a trie from the start, and each name continues into its
own arguments.

<!-- source: psionic/lib/harness/tools.coax compile -->
```jai
// Building the automaton: states are numbered as they are made, and every
// edge goes from an older state to a newer one, so the language is finite.
// Each state records the field whose text it is inside (-1 before the first).
// States and edges past the buffers are counted, not kept; the count refuses them.
ht_state :: (b:[]HtBuild,w:HtWork)->int {
    s:=b[0].states;if s<w.owner.count {w.owner[s]=b[0].field;}
    b[0].states+=1;return s;
}
ht_edge :: (b:[]HtBuild,w:HtWork,from:int,low:u8,high:u8,to:int) {
    if b[0].edges<w.edges.count {w.edges[b[0].edges]=NxEdge.{from=from,low=low,high=high,to=to};}
    b[0].edges+=1;
}
// One byte from every state in ends to one new state.
ht_join :: (b:[]HtBuild,w:HtWork,ends:[]int,c:u8)->int {
    s:=ht_state(b,w);for e:ends {ht_edge(b,w,e,c,c,s);}return s;
}
// One byte from s: the existing edge if a shared prefix made it, else a new state.
ht_follow :: (b:[]HtBuild,w:HtWork,s:int,c:u8)->int {
    for e:w.edges[0..<nx_min(b[0].edges,w.edges.count)] {if e.from==s && e.low==c {return e.to;}}
    one:=[1]int.[s];return ht_join(b,w,one[..],c);
}
ht_bytes :: (b:[]HtBuild,w:HtWork,s:int,text:string)->int {x:=s;for c:text {x=ht_follow(b,w,x,c);}return x;}
// A value of type t after state s; its end states go to ends, and the count is returned.
ht_value :: (b:[]HtBuild,w:HtWork,s:int,t:HtType,ends:[]int)->int {
    if t.kind==HtKind.integer {
        ends[0]=ht_bytes(b,w,s,"0");d:=ht_state(b,w);ht_edge(b,w,s,'1','9',d);ends[1]=d;
        for k:1..<t.digits {x:=ht_state(b,w);ht_edge(b,w,d,'0','9',x);d=x;ends[k+1]=d;}
        return t.digits+1;
    }
    if t.kind==HtKind.boolean {ends[0]=ht_bytes(b,w,s,"true");ends[1]=ht_bytes(b,w,s,"false");return 2;}
    q:=ht_bytes(b,w,s,"\"");words:[HT_CHOICES_MAX]int;
    for x,i:t.choices {words[i]=ht_bytes(b,w,q,x);}
    ends[0]=ht_join(b,w,words[0..<t.choices.count],'"');return 1;
}
// The calls of the offered tools: name{"field":value,...}, fields in order.
ht_compile :: (tools:[]HtTool,offered:[]int,w:HtWork)->HtGrammar {
    b:[1]HtBuild;start:=ht_state(b[..],w);
    closing:[HT_TOOLS_MAX*(HT_DIGITS_MAX+1)]int;nc:=0;
    for t:offered {
        b[0].field= -1;ends:[HT_DIGITS_MAX+1]int;ends[0]=ht_bytes(b[..],w,start,tools[t].name);ne:=1;
        intro:="{\"";
        for f,i:tools[t].fields {
            b[0].field=i;s:=ht_join(b[..],w,ends[0..<ne],intro[0]);
            s=ht_bytes(b[..],w,ht_bytes(b[..],w,s,intro[1..<2]),f.name);s=ht_bytes(b[..],w,s,"\":");
            ne=ht_value(b[..],w,s,f.type,ends[..]);intro=",\"";
        }
        if tools[t].fields.count==0 {ends[0]=ht_bytes(b[..],w,ends[0],"{");}
        for e:ends[0..<ne] {closing[nc]=e;nc+=1;}
    }
    accept:=ht_join(b[..],w,closing[0..<nc],'}');
    if b[0].states>w.owner.count || b[0].edges>w.edges.count {return HtGrammar.{why=HtWhy.capacity};}
    // The table stores state k as last-k, so every edge points down and the
    // liveness sweep, which runs upward, finishes in one pass.
    last:=b[0].states-1;
    for i:0..<b[0].edges {x:=w.edges[i];w.edges[i]=NxEdge.{from=last-x.from,low=x.low,high=x.high,to=last-x.to};}
    for i:0..<b[0].states/2 {x:=w.owner[i];w.owner[i]=w.owner[last-i];w.owner[last-i]=x;}
    final:=[1]int.[last-accept];
    c:=nx_compile(b[0].states,last,w.edges[0..<b[0].edges],final[..],w.next,w.accept,w.live,false);
    if c.status!=NxStatus.ok {return HtGrammar.{why=HtWhy.capacity};}
    return HtGrammar.{grammar=c.grammar};
}
```

The mask follows from the grammar. From the start of the grant
`add, div, get`, the 52 pieces include four that keep a call possible:
`a`, `d`, `g` and the longer piece `add{`. After `add{"a":`, only the ten
digits are offered. After `add{"a":4`, the comma is offered as well. After
a complete `get{"room":"den"}`, only the end piece is offered.

## Before: admission

`ht_admit` runs the checks in a fixed order. First it checks the registry,
the grant and the call's length. Then it reads the name, the text before
the first `{`. A call with no `{` ended before its arguments, so it is
`incomplete`. Otherwise it looks up the name, then the grant entry, then the call
budget, then the cost budget. Last, it walks the arguments byte by byte
through that tool's own grammar. When a byte has no edge, the refusal names
the byte and the field whose state refused it. A call that ends before the
accepting state is `incomplete`. A spent budget refuses the call before
the arguments are read. An admitted call is decoded into one value per
field: the integer, `0` or `1` for a boolean, or the word's index.

**By hand: the refusal table.** The grant is `add` twice, `div` once and
`get` once, with cost 3 and ceiling `read`. In `add{"a":01,"b":2}`, bytes
0–2 are `add`, byte 3 is `{` and bytes 4–7 are `"a":`. Byte 8, `0`, ends
the integer, so byte 9, `1`, has no edge. In `add{"b":1,"a":2}`, byte 5 is
`b` where the grammar expects `a`. In `get{"room":"attic"}`, byte 12 is
`a`, and no room starts with `a`.

| Call | Reason | Tool | Field | Byte |
| --- | --- | --- | --- | --- |
| `mul{"a":1,"b":2}` | `unknown` | | | 0 |
| `set{"room":"den","on":true}` | `ungranted` | `set` | | 0 |
| `add{"a":01,"b":2}` | `arguments` | `add` | `a` | 9 |
| `add{"a":1000,"b":2}` | `arguments` | `add` | `a` | 11 |
| `add{"b":1,"a":2}` | `arguments` | `add` | `a` | 5 |
| `add{"a":1}` | `arguments` | `add` | `a` | 9 |
| `add{"a": 1,"b":2}` | `arguments` | `add` | `a` | 8 |
| `add{"a":1,"b":2}x` | `arguments` | `add` | `b` | 16 |
| `get{"room":"attic"}` | `arguments` | `get` | `room` | 12 |
| `get{"room":"den","on":true}` | `arguments` | `get` | `room` | 16 |
| `add{"a":1,` | `incomplete` | `add` | `b` | 10 |
| `add` | `incomplete` | `add` | | 3 |
| a third `add` | `calls` | `add` | | |
| a `get` after cost 3 is spent | `cost` | `get` | | |

The checks assert every row, and that each refused call decodes and
charges nothing.

<!-- source: psionic/lib/harness/tools.coax admit -->
```jai
// Before: the registry, the grant, the name, the budgets, then the
// arguments, byte by byte through the tool's own grammar. A refusal names
// why, the tool, the field and the byte; nothing runs.
ht_admit :: (tools:[]HtTool,g:HtGrant,u:HtUsage,text:string,w:HtWork)->HtEvent {
    e:=ht_check(tools);if e.why!=HtWhy.none {return e;}
    e=ht_grant(tools,g);if e.why!=HtWhy.none {return e;}
    if text.count>HT_TEXT_MAX {return HtEvent.{why=HtWhy.shape};}
    p:=0;while p<text.count && text[p]!='{' {p+=1;}
    t:=-1;for x,i:tools {if x.name==text[0..<p] {t=i;}}
    if p==text.count {return HtEvent.{why=HtWhy.incomplete,tool=t,at=p};}
    if t<0 {return HtEvent.{why=HtWhy.unknown,at=0};}
    j:=-1;for x,i:g.tools {if x==t {j=i;}}
    if j<0 {return HtEvent.{why=HtWhy.ungranted,tool=t,at=0};}
    if u.calls[j]>=g.calls[j] {return HtEvent.{why=HtWhy.calls,tool=t};}
    if u.cost+tools[t].cost>g.cost {return HtEvent.{why=HtWhy.cost,tool=t};}
    one:=[1]int.[t];c:=ht_compile(tools,one[..],w);if c.why!=HtWhy.none {return HtEvent.{why=c.why,tool=t};}
    s:=c.grammar.start;
    for x,i:text {n:=w.next[s*256+cast(int)x];if n<0 {return HtEvent.{why=HtWhy.arguments,tool=t,field=w.owner[s],at=i};}s=n;}
    if w.accept[s]!=1 {return HtEvent.{why=HtWhy.incomplete,tool=t,field=w.owner[s],at=text.count};}
    return HtEvent.{tool=t,values=ht_decode(tools[t],text)};
}
// The values of an admitted call: integers, booleans as 0 and 1, choices as indices.
ht_decode :: (t:HtTool,text:string)->[HT_FIELDS_MAX]int {
    v:[HT_FIELDS_MAX]int;at:=t.name.count+1;
    for f,i:t.fields {
        at+=f.name.count+3;
        if f.type.kind==HtKind.integer {while text[at]!=',' && text[at]!='}' {v[i]=v[i]*10+cast(int)(text[at]-'0');at+=1;}}
        else if f.type.kind==HtKind.boolean {v[i]=ifx text[at]=='t' then 1 else 0;at+=5-v[i];}
        else {e:=at;while text[e+1]!='"' {e+=1;}for x,k:f.type.choices {if x==text[at+1..<e+1] {v[i]=k;}}at=e+2;}
        at+=1;
    }
    return v;
}
```

## Run, then after

An admitted call is charged one call and its cost before its body runs.
The charge stays even if the run or after stage refuses the call, because
the tool did the work. The bodies are the chapter's four. `div` refuses a
zero divisor as `run`, and names field `b`. `set` writes the session's
world, and `get` reads it. A second session has its own world, so a switch
set in one session is not visible in another.

The after stage checks the result against its declared type. `add` of
`999` and `1` computes `1000`, which does not fit three digits. The call is
charged and its result is withheld. The model sees a `result` error
instead of a number that breaks the declaration.

<!-- source: psionic/lib/harness/tools.coax pipeline -->
```jai
// Run: the four bodies. The world is the session's state, one switch per room.
ht_body :: (body:HtBody,v:[HT_FIELDS_MAX]int,world:[]int)->HtEvent {
    if body==HtBody.add {return HtEvent.{value=v[0]+v[1]};}
    if body==HtBody.divide {if v[1]==0 {return HtEvent.{why=HtWhy.run,field=1};}return HtEvent.{value=v[0]/v[1]};}
    if body==HtBody.set {world[v[0]]=v[1];}
    return HtEvent.{value=world[v[0]]};
}
ht_power :: (n:int)->int {x:=1;for i:0..<n {x*=10;}return x;}
// After: a result is shown only when it fits its declared type.
ht_fits :: (t:HtType,v:int)->bool {
    if t.kind==HtKind.integer {return v>=0 && v<ht_power(t.digits);}
    if t.kind==HtKind.boolean {return v==0 || v==1;}
    return v>=0 && v<t.choices.count;
}
// One call through before, run and after. A call that is admitted is
// charged one call and its cost, whatever the run and after stages say.
ht_call :: (tools:[]HtTool,g:HtGrant,u:[]HtUsage,text:string,world:[]int,w:HtWork)->HtEvent {
    if u.count<1 || world.count<HT_CHOICES_MAX {return HtEvent.{why=HtWhy.shape};}
    e:=ht_admit(tools,g,u[0],text,w);if e.why!=HtWhy.none {return e;}
    t:=tools[e.tool];j:=0;while g.tools[j]!=e.tool {j+=1;}
    u[0].calls[j]+=1;u[0].cost+=t.cost;e.cost=t.cost;e.ran=true;
    r:=ht_body(t.body,e.values,world);e.why=r.why;e.field=r.field;
    if r.why==HtWhy.none && !ht_fits(t.result,r.value) {e.why=HtWhy.result;}
    if e.why==HtWhy.none {e.value=r.value;}
    return e;
}
```

## The tool-role message

Each outcome is one `HtEvent`. The tool-role message is that event written
as JSON. A result is written as its declared type: digits, `true` or
`false`, or the word. A refusal is written with the reason, followed by the
tool, the field and the byte when they apply:
`{"error":"arguments","tool":"add","field":"b","at":16}`. The longest
message in this profile fits in 96 bytes. When the output buffer is too
small, nothing is written.

<!-- source: psionic/lib/harness/tools.coax render -->
```jai
// The tool-role message: the result as its declared type, or the refusal
// with whatever of tool, field and byte applies. 0 when out is too small.
HT_WHY :: [14]string.["none","shape","capacity","schema","grant","effect","unknown","ungranted","calls","cost","arguments","incomplete","run","result"];
ht_put :: (out:[]u8,n:int,s:string)->int {for c,i:s {out[n+i]=c;}return n+s.count;}
ht_put_int :: (out:[]u8,n:int,v:int)->int {b:[20]u8;k:=0;x:=v;while true {b[19-k]='0'+cast(u8)(x%10);k+=1;x/=10;if x==0 {break;}}return ht_put(out,n,cast(string)b[20-k..<20]);}
ht_render :: (tools:[]HtTool,e:HtEvent,out:[]u8)->int {
    m:[96]u8;n:=0;
    if e.why==HtWhy.none {
        r:=tools[e.tool].result;n=ht_put(m[..],ht_put(m[..],n,"{\"tool\":\""),tools[e.tool].name);n=ht_put(m[..],n,"\",\"result\":");
        if r.kind==HtKind.integer {n=ht_put_int(m[..],n,e.value);}
        else if r.kind==HtKind.boolean {n=ht_put(m[..],n,ifx e.value==1 then "true" else "false");}
        else {n=ht_put(m[..],ht_put(m[..],ht_put(m[..],n,"\""),r.choices[e.value]),"\"");}
    } else {
        n=ht_put(m[..],ht_put(m[..],ht_put(m[..],n,"{\"error\":\""),HT_WHY[cast(int)e.why]),"\"");
        if e.tool>=0 {n=ht_put(m[..],ht_put(m[..],ht_put(m[..],n,",\"tool\":\""),tools[e.tool].name),"\"");}
        if e.field>=0 {n=ht_put(m[..],ht_put(m[..],ht_put(m[..],n,",\"field\":\""),tools[e.tool].fields[e.field].name),"\"");}
        if e.at>=0 {n=ht_put_int(m[..],ht_put(m[..],n,",\"at\":"),e.at);}
    }
    n=ht_put(m[..],n,"}");
    if n>out.count {return 0;}
    for i:0..<n {out[i]=m[i];}return n;
}
```

## A turn of constrained calls

Before each request, `ht_turn` offers only the granted tools whose call
budget and cost budget can still pay for them. It compiles their calls into
the mask and samples a call one piece at a time with Psionic chapter 53's
`nx_guided`. Then the call goes through `ht_call`, the same path as any
other call. A tool whose budget is spent is no longer offered. When no
tool is offered, the turn stops as `spent`. When the step budget is
reached, it stops as `steps`. A call cut off by the piece limit is
`incomplete` and does not run.

<!-- source: psionic/lib/harness/tools.coax turn -->
```jai
// The stand-in model's logits for draw i of step k: seeded noise in [0, 8).
ht_noise :: (seed:u64,k:int,i:int,logits:[]f32) {
    for x,j:logits {logits[j]=f32(cast(int)(cr_bits(seed,cast(u64)(k*1048576+i*1024+j))%4096))/512.0;}
}
// One turn of tool calls. Before each request the turn offers only the
// granted tools whose call and cost budgets can still pay for them, and
// compiles their calls into the mask; then the call goes through ht_call
// like any other. Each step's event is events[k].
ht_turn :: (tools:[]HtTool,g:HtGrant,m:HtModel,seed:u64,steps:int,world:[]int,w:HtWork,sm:HtSampler,events:[]HtEvent)->HtTurn {
    t:=HtTurn.{stop=HtStop.invalid};
    e:=ht_check(tools);if e.why==HtWhy.none {e=ht_grant(tools,g);}
    if e.why!=HtWhy.none {t.why=e.why;return t;}
    long:=false;for x:m.pieces {if x.count>HT_PIECE_MAX {long=true;}}
    if long || steps<0 || events.count<steps || world.count<HT_CHOICES_MAX || m.limit<1 || m.limit*HT_PIECE_MAX>sm.text.count || sm.logits.count!=m.pieces.count {t.why=HtWhy.shape;return t;}
    u:[1]HtUsage;
    while true {
        if t.steps==steps {t.stop=HtStop.steps;return t;}
        offer:[HT_TOOLS_MAX]int;no:=0;
        for x,j:g.tools {if u[0].calls[j]<g.calls[j] && u[0].cost+tools[x].cost<=g.cost {offer[no]=x;no+=1;}}
        if no==0 {t.stop=HtStop.spent;return t;}
        c:=ht_compile(tools,offer[0..<no],w);if c.why!=HtWhy.none {t.why=c.why;return t;}
        run:=NxRun.{limit=m.limit,random=CrSeed.{seed=cr_bits(seed,cast(u64)t.steps)}};state:=c.grammar.start;n:=0;
        while run.stop==NxStop.running {
            ht_noise(seed,t.steps,run.emitted,sm.logits);
            d:=nx_guided(sm.logits,c.grammar,state,m.pieces,m.end,m.options,run,false,sm.mask,sm.out,sm.work,false);
            if d.status!=NxStatus.ok {t.stop=HtStop.model;return t;}
            if d.token!=m.end {for x:m.pieces[d.token] {sm.text[n]=x;n+=1;}}
            run=d.run;state=d.state;
        }
        v:=ht_call(tools,g,u[..],cast(string)sm.text[0..<n],world,w);events[t.steps]=v;t.steps+=1;
        if v.ran {t.ran+=1;}else {t.refused+=1;}
        t.cost=u[0].cost;
    }
    return t;
}
```

## The checks

- **The grammar against enumeration.** A `#forall` over 100 seeds builds a
  random registry of up to three tools. Their names are `f`, `fg` and `g`,
  so two names share a prefix. Each tool has up to two fields of a one-digit
  integer, a boolean, or a choice among words such as `a`, `ab` and `abc`,
  which are prefixes of each other. A separate file,
  [harness-tools-oracle.coax](../../psionic/tests/harness-tools-oracle.coax), writes
  out every call the schemas allow, up to 200 per registry. Each of them
  must be accepted. The oracle also checks that every edge of the compiled
  table points to a lower state, then counts the accepted strings path by
  path. That count must equal the number written out. Every written-out
  call is accepted and the counts are equal, so the grammar accepts exactly
  the calls the schemas allow. Then 40 one-byte edits per registry are
  checked: an edit is accepted only when it is itself a call. Admission
  decodes the values the enumerator wrote.
- **Sampling never produces a refused call.** Twenty seeded turns with
  every tool granted ran 160 calls. Each one passed admission, and only
  `run` and `result` refused any of them. The example runs 1,000 turns.
- **The hand traces and the table above**, each asserted exactly,
  including the buffer sizes at their limits.
- **Mutation.** `coax mutate --root-only` leaves no survivor in
  `harness/tools.coax`.

## Run it

The example, [tool-capabilities.coax](../../psionic/examples/tool-capabilities.coax),
prints the registry and `get`'s grammar, and refuses a read-only grant that
names `set`. It prints what the mask offers at four points, runs the
refusal table through the full pipeline with the usage after each call,
and shows that the world belongs to one session. Then it runs one turn
and 1,000 turns of constrained calls.

<!-- source: psionic/examples/tool-capabilities.coax example -->
```jai
main :: () {
    rm:=HT_ROOMS;fs:[5]HtField;r:=ht_registry(rm[..],fs[..]);
    e:[512]NxEdge;o:[256]int;n:[65536]int;a:[256]u8;l:[256]u8;w:=ht_work(e[..],o[..],n[..],a[..],l[..]);
    print("registry: % tools, check %\n",r.count,ht_check(r[..]).why);
    for t:r {print("  % fields %, result %, effect %, cost %\n",t.name,t.fields.count,t.result.kind,t.effect,t.cost);}
    // The hand trace: get's grammar, built state by state.
    get:=[1]int.[3];c:=ht_compile(r[..],get[..],w);
    print("get's grammar: % states, start %, accepts get{\"room\":\"den\"}: %, get{\"room\":\"attic\"}: %\n",c.grammar.states,c.grammar.start,
        nx_accepts(c.grammar,"get{\"room\":\"den\"}"),nx_accepts(c.grammar,"get{\"room\":\"attic\"}"));
    // The grant: add twice, div once, get once, cost 3, nothing that writes.
    g:=[3]int.[0,1,3];calls:=[3]int.[2,1,1];gr:=HtGrant.{tools=g[..],calls=calls[..],cost=3,effects=HtEffect.read};
    print("grant: %\n",ht_grant(r[..],gr).why);
    wide:=[4]int.[0,1,2,3];four:=[4]int.[1,1,1,1];bad:=HtGrant.{tools=wide[..],calls=four[..],cost=3,effects=HtEffect.read};
    print("a read-only grant that names set: ");tc_show(r[..],ht_grant(r[..],bad));print("\n");
    // What the mask offers at three points of a call.
    pc:=ht_pieces();u:=ht_compile(r[..],g[..],w).grammar;
    tc_offered("at the start",u,u.start,pc[..]);
    tc_offered("after add{\"a\":",u,nx_walk(u,u.start,"add{\"a\":"),pc[..]);
    tc_offered("after add{\"a\":4",u,nx_walk(u,u.start,"add{\"a\":4"),pc[..]);
    tc_offered("after get{\"room\":\"den\"}",u,nx_walk(u,u.start,"get{\"room\":\"den\"}"),pc[..]);
    // The pipeline: each call, its event, and the session's usage after it.
    texts:=[14]string.["add{\"a\":37,\"b\":48}","mul{\"a\":1,\"b\":2}","set{\"room\":\"den\",\"on\":true}","add{\"a\":01,\"b\":2}","add{\"b\":1,\"a\":2}",
        "add{\"a\": 1,\"b\":2}","get{\"room\":\"attic\"}","add{\"a\":1,","div{\"a\":7,\"b\":0}","div{\"a\":7,\"b\":2}","add{\"a\":999,\"b\":1}","add{\"a\":1,\"b\":1}","get{\"room\":\"den\"}","add"];
    us:[1]HtUsage;world:[8]int;
    for x:texts {v:=ht_call(r[..],gr,us[..],x,world[..],w);print("  % -> ",x);tc_show(r[..],v);print("  (calls % % %, cost %)\n",us[0].calls[0],us[0].calls[1],us[0].calls[2],us[0].cost);}
    // Effects stay in the session's world.
    all:=[4]int.[0,1,2,3];some:=[4]int.[1,1,1,1];rw:=HtGrant.{tools=all[..],calls=some[..],cost=10,effects=HtEffect.write};us[0]=HtUsage.{};other:[8]int;
    print("set den on: ");tc_show(r[..],ht_call(r[..],rw,us[..],"set{\"room\":\"den\",\"on\":true}",world[..],w));
    print("; get den: ");tc_show(r[..],ht_call(r[..],rw,us[..],"get{\"room\":\"den\"}",world[..],w));us[0]=HtUsage.{};
    print("; get den in another session: ");tc_show(r[..],ht_call(r[..],rw,us[..],"get{\"room\":\"den\"}",other[..],w));print("\n");
    // Constrained turns: a stand-in model of seeded noise, masked by the grant's grammar.
    lg:[52]f32;mk:[52]u8;p:[52]f64;s:[52]f64;od:[52]int;tm:[52]int;tx:[512]u8;sm:=ht_sampler(lg[..],mk[..],p[..],s[..],od[..],tm[..],tx[..]);
    m:=HtModel.{pieces=pc[..],end=HT_END,options=NxOptions.{temperature=1},limit=64};ev:[12]HtEvent;tw:[8]int;
    t:=ht_turn(r[..],gr,m,0,8,tw[..],w,sm,ev[..]);
    print("turn, seed 0: stop %, steps %, ran %, refused %, cost %\n",t.stop,t.steps,t.ran,t.refused,t.cost);
    for k:0..<t.steps {print("  ");tc_show(r[..],ev[k]);print("\n");}
    c4:=[4]int.[3,3,3,3];wide4:=HtGrant.{tools=all[..],calls=c4[..],cost=12,effects=HtEffect.write};
    ran:=0;refused:=0;runs:=0;results:=0;cost:=0;stops:[4]int;
    for seed:0..<1000 {wd:[8]int;q:=ht_turn(r[..],wide4,m,cast(u64)seed,12,wd[..],w,sm,ev[..]);stops[cast(int)q.stop]+=1;ran+=q.ran;refused+=q.refused;cost+=q.cost;
        for k:0..<q.steps {if ev[k].why==HtWhy.run {runs+=1;}if ev[k].why==HtWhy.result {results+=1;}}}
    print("1000 turns of up to 12 steps, every tool granted 3 calls, cost 12: % stopped at the step limit, % spent; % calls ran (% refused by run, % by result), % refused before running, cost %\n",
        stops[0],stops[1],ran,runs,results,refused,cost);
    m.limit=3;t=ht_turn(r[..],gr,m,0,2,tw[..],w,sm,ev[..]);print("a 3-piece limit: ran %, refused %: ",t.ran,t.refused);tc_show(r[..],ev[0]);print("\n");
}
```

```shell
coax/bin/coax run psionic/examples/tool-capabilities.coax
psionic/check.sh --only harness/tools
```

```text
registry: 4 tools, check none
  add fields 2, result integer, effect none, cost 1
  div fields 2, result integer, effect none, cost 1
  set fields 2, result boolean, effect write, cost 2
  get fields 1, result boolean, effect read, cost 1
get's grammar: 27 states, start 26, accepts get{"room":"den"}: true, get{"room":"attic"}: false
grant: none
a read-only grant that names set: {"error":"effect","tool":"set"}
at the start: 4 pieces: a d g add{
after add{"a":: 10 pieces: 0 1 2 3 4 5 6 7 8 9
after add{"a":4: 11 pieces: , 0 1 2 3 4 5 6 7 8 9
after get{"room":"den"}: 1 pieces: <end>
  add{"a":37,"b":48} -> {"tool":"add","result":85}  (calls 1 0 0, cost 1)
  mul{"a":1,"b":2} -> {"error":"unknown","at":0}  (calls 1 0 0, cost 1)
  set{"room":"den","on":true} -> {"error":"ungranted","tool":"set","at":0}  (calls 1 0 0, cost 1)
  add{"a":01,"b":2} -> {"error":"arguments","tool":"add","field":"a","at":9}  (calls 1 0 0, cost 1)
  add{"b":1,"a":2} -> {"error":"arguments","tool":"add","field":"a","at":5}  (calls 1 0 0, cost 1)
  add{"a": 1,"b":2} -> {"error":"arguments","tool":"add","field":"a","at":8}  (calls 1 0 0, cost 1)
  get{"room":"attic"} -> {"error":"arguments","tool":"get","field":"room","at":12}  (calls 1 0 0, cost 1)
  add{"a":1, -> {"error":"incomplete","tool":"add","field":"b","at":10}  (calls 1 0 0, cost 1)
  div{"a":7,"b":0} -> {"error":"run","tool":"div","field":"b"}  (calls 1 1 0, cost 2)
  div{"a":7,"b":2} -> {"error":"calls","tool":"div"}  (calls 1 1 0, cost 2)
  add{"a":999,"b":1} -> {"error":"result","tool":"add"}  (calls 2 1 0, cost 3)
  add{"a":1,"b":1} -> {"error":"calls","tool":"add"}  (calls 2 1 0, cost 3)
  get{"room":"den"} -> {"error":"cost","tool":"get"}  (calls 2 1 0, cost 3)
  add -> {"error":"incomplete","tool":"add","at":3}  (calls 2 1 0, cost 3)
set den on: {"tool":"set","result":true}; get den: {"tool":"get","result":true}; get den in another session: {"tool":"get","result":false}
turn, seed 0: stop spent, steps 3, ran 3, refused 0, cost 3
  {"tool":"add","result":617}
  {"tool":"div","result":0}
  {"tool":"get","result":false}
1000 turns of up to 12 steps, every tool granted 3 calls, cost 12: 0 stopped at the step limit, 1000 spent; 9629 calls ran (201 refused by run, 967 by result), 0 refused before running, cost 11837
a 3-piece limit: ran 0, refused 2: {"error":"incomplete","tool":"add","field":"a","at":6}
```

In the 1,000 turns, every tool has 3 calls and the turn has a cost budget
of 12. 9,629 calls ran. `run` refused 201 of them, all of them division by
zero, and `result` refused 967, all of them sums above 999. None was
refused before it ran: masked sampling did not produce an ungranted tool,
an ill-typed argument or a call over budget. Every turn stopped as `spent`
before it reached 12 steps. The last line limits a call to 3 pieces, so
`add{"a` is cut off and refused as `incomplete` before anything runs.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- [Psionic chapter 53](../../psionic/book/ch053-choosing-the-next-token.md): the automaton, the mask
  and guided sampling.
- [Psionic chapter 77](../../psionic/book/ch077-call-a-tool.md): granted tools and a finite schema on
  gpt-oss.
- [Chapter1](ch001-the-simplest-loop.md): the loop these calls will
  run inside.
