# 3. Keep the prefix stable

> **Status: partly implemented.** Prompt assembly in a fixed section order,
> a checker that refuses a request which changes what the previous request
> held before its cache boundary, and a measurement on Psionic chapter 176's prefix
> cache priced with Psionic chapter 177's cost model (profile px-prefix-v1) run on
> the CPU over chapter2's log and scripted sessions. Times are modeled
> with Psionic chapter 175's step cost, not measured: time to first token on a
> named host and model, real tokenized system prompts and tool schemas, and
> a long session of more than 16 requests remain.

A server with a prefix cache reuses work only when a request starts with
the same tokens as an earlier one. [Psionic chapter 176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md)
keeps finished prompt blocks in a trie keyed by the model and the exact
tokens of each block, so a request reuses a block only if every token
before it, and in it, is the same. An agent sends many requests in one
session, each the previous one plus a little more history. If the harness
puts anything that changes per request near the front, such as the current
time, every later block differs and nothing is reused.

This chapter fixes the order in which the harness assembles a request so
that, within a session, each request starts with every token of the
previous one up to a stated boundary. A checker refuses an assembly that
breaks this and names the section and position where it breaks. The
chapter then runs scripted sessions through Psionic chapter 176's cache with three
assemblers and counts what each one reuses.

Everything here lives in one `#authority` file,
[harness/prefix.coax](../../psionic/lib/harness/prefix.coax), because it decides what
a request may hold and which requests are admitted. Its checks leave no
mutant alive.

## The profile

This chapter's profile is `px-prefix-v1`:

- Tokens are integers. A request holds at most 16 sections and 64 tokens;
  a session has at most 8 tools, each with a schema of at most 8 tokens.
- The history is chapter2's log, read from its token store.
- The cache measurement runs up to 16 requests of up to 32 tokens on one
  model through Psionic chapter 176's `pg_run`, one request at a time, with pages
  of 4 positions and 16 frames in each tier.
- Times are Psionic chapter 177's cost model over Psionic chapter 175's step cost, in
  modeled microseconds.

## Sections in a fixed order

A request is a list of tokens covered by typed sections. The kinds have
one order, and a request must hold them in it:

| Kind | What it holds | When it changes |
| --- | --- | --- |
| `system` | system text | never within a session |
| `tool` | one tool's schema; the key is the tool's id | never within a session |
| `header` | values fixed when the session opens, such as its date | never within a session |
| `history` | the tokens chapter2's log holds | only by appending |
| `tail` | values that change every request, such as a clock or a counter | every request |

The **cache boundary** is where the first `tail` section starts, or the
end of the request when it has none. Everything before the boundary only
grows during a session; everything after it may change. Anything that is
not in a section is not sent to the model. Identity headers and session
metadata that the model does not need travel beside the request; this
chapter has no type for them, because they never enter a request.

<!-- source: psionic/lib/harness/prefix.coax types -->
```jai
// Section kinds in the only order a request may hold them.
PxKind :: enum u8 {system;tool;header;history;tail;}
PxStatus :: enum u8 {ok;shape;log;order;tools;prefix;}
// A section: its kind, its key (a tool's id; 0 otherwise), and where its
// tokens start in the request and how many there are.
PxSection :: struct {kind:PxKind;key:int;at:int;count:int;}
// A tool's schema as tokens.
PxTool :: struct {id:int;count:int;tok:[8]int;}
// A request: its tokens and the sections that cover them, in order. A
// session's frozen prefix is a request with no history and no tail.
PxRequest :: struct {status:PxStatus;n:int;sections:int;section:[16]PxSection;tok:[64]int;}
// The checker's answer: the request it is about, the section and position
// of the first refusal, and the tokens shared with the previous request.
PxVerdict :: struct {status:PxStatus;request:int;section:int;kind:PxKind;at:int;shared:int;}
```

`px_open` lays out a session's frozen prefix once: the system text, every
tool's schema in increasing id order (whatever order the tools were
registered in), and the header. `px_assemble` adds the history and the
tail. The history comes from chapter2's log: either everything logged
so far, or exactly the tokens a logged request event saw. The log must
verify, and no event in the history may be redacted, the same rule
chapter2's `lg_run` applies before it sends anything. `px_naive` is the
assembler this chapter replaces: tools in registration order, and the
tail at the top when `top` is set.

<!-- source: psionic/lib/harness/prefix.coax assemble -->
```jai
// A section after the others; refused when either bound would be passed.
px_add :: (from:PxRequest,kind:PxKind,key:int,tok:[]int)->PxRequest {
    r:=from;
    if r.status!=PxStatus.ok {return r;}
    if r.sections==PX_SECTIONS || tok.count>PX_TOKENS-r.n {r.status=PxStatus.shape;return r;}
    r.section[r.sections]=PxSection.{kind=kind,key=key,at=r.n,count=tok.count};r.sections+=1;
    for i:0..<tok.count {r.tok[r.n+i]=tok[i];}
    r.n+=tok.count;return r;
}
// The session's frozen prefix, laid out once: the system text, every tool
// schema in increasing id order whatever order they were registered in, and
// the header's values. Two tools may not share an id.
px_open :: (system:[]int,tools:[]PxTool,header:[]int)->PxRequest {
    r:PxRequest;r.status=PxStatus.shape;
    if tools.count>PX_TOOLS {return r;}
    for t,i:tools {
        if t.id<0 || t.count<0 || t.count>PX_SCHEMA {return r;}
        for u:tools[0..<i] {if u.id==t.id {r.status=PxStatus.tools;return r;}}
    }
    // Ids are distinct, so each tool's place is the number of smaller ids.
    order:[8]int;
    for t,i:tools {k:=0;for u:tools {if u.id<t.id {k+=1;}}order[k]=i;}
    r.status=PxStatus.ok;r=px_add(r,PxKind.system,0,system);
    for b:order[0..<tools.count] {r=px_add(r,PxKind.tool,tools[b].id,tools[b].tok[0..<tools[b].count]);}
    return px_add(r,PxKind.header,0,header);
}
// The history a request holds: what request event index saw, or with index
// at the log's end, every token logged. The log must verify, and no event
// in the history may be redacted.
px_history :: (s:PxRequest,g:LgLog,index:int)->PxRequest {
    r:=s;
    if r.status!=PxStatus.ok {return r;}
    if lg_verify(g).status!=LgStatus.ok {r.status=PxStatus.log;return r;}
    n:=g.used;
    if index!=g.count {
        if lg_kind(g,index)!=LgKind.request {r.status=PxStatus.log;return r;}
        n=g.events[index].f[1];
    }
    for e:g.events[0..<index] {if e.redacted {r.status=PxStatus.log;return r;}}
    return px_add(r,PxKind.history,0,g.tokens[0..<n]);
}
// The request: the frozen prefix, the history, then the tail of values that
// change every request, after the cache boundary.
px_assemble :: (s:PxRequest,g:LgLog,index:int,tail:[]int)->PxRequest {
    return px_add(px_history(s,g,index),PxKind.tail,0,tail);
}
// The assembler this chapter replaces: tools in the order they were
// registered, and the tail at the top when top is set.
px_naive :: (system:[]int,tools:[]PxTool,header:[]int,g:LgLog,tail:[]int,top:bool)->PxRequest {
    r:PxRequest;
    if top {r=px_add(r,PxKind.tail,0,tail);}
    r=px_add(r,PxKind.system,0,system);
    for i:0..<tools.count {r=px_add(r,PxKind.tool,tools[i].id,tools[i].tok[0..<tools[i].count]);}
    r=px_history(px_add(r,PxKind.header,0,header),g,g.count);
    if !top {r=px_add(r,PxKind.tail,0,tail);}
    return r;
}
```

## The checker

`px_check` refuses a layout at the first section whose kind comes before
the previous section's kind (`order`), or at a tool section whose id is not
above the tool section before it (`tools`). `px_admit` checks the next
request after a previous one. The next request's layout must pass, and its
tokens must start with every token of the previous request before the
previous request's boundary. It computes the longest prefix the two share;
when that is shorter than the previous boundary, it refuses with `prefix`
and names the previous request's section that holds the first difference.
`px_session` runs the checker over a logged session's requests in order
and reports the first refusal with the request's index.

<!-- source: psionic/lib/harness/prefix.coax verdict -->
```jai
// The cache boundary: where the first tail section starts, or the end.
px_boundary :: (r:PxRequest)->int {
    for i:0..<r.sections {if r.section[i].kind==PxKind.tail {return r.section[i].at;}}
    return r.n;
}
px_at :: (r:PxRequest,i:int,status:PxStatus)->PxVerdict {return PxVerdict.{status=status,section=i,kind=r.section[i].kind,at=r.section[i].at};}
// A layout is refused at the first section whose kind comes before the
// previous section's, or at a tool whose id is not above the tool before it.
px_check :: (r:PxRequest)->PxVerdict {
    if r.status!=PxStatus.ok {return PxVerdict.{status=r.status};}
    for i:1..<r.sections {
        a:=r.section[i-1];x:=r.section[i];
        if cast(u8)x.kind<cast(u8)a.kind {return px_at(r,i,PxStatus.order);}
        if x.kind==PxKind.tool && a.kind==PxKind.tool && x.key<=a.key {return px_at(r,i,PxStatus.tools);}
    }
    return PxVerdict.{};
}
// The next request after prev: its layout must pass, and it must start with
// every token of prev before prev's boundary. shared is the longest common
// prefix; a refusal names prev's section that holds the first difference.
px_admit :: (prev:PxRequest,next:PxRequest)->PxVerdict {
    v:=px_check(next);
    if v.status!=PxStatus.ok {return v;}
    s:=0;
    while s<prev.n && s<next.n && prev.tok[s]==next.tok[s] {s+=1;}
    v.shared=s;v.at=s;
    if s<px_boundary(prev) {
        i:=0;while prev.section[i].at+prev.section[i].count<=s {i+=1;}
        v.status=PxStatus.prefix;v.section=i;v.kind=prev.section[i].kind;
    }
    return v;
}
// A logged session's requests in order: the first refusal, with the
// request's index, or the last request's verdict.
px_session :: (reqs:[]PxRequest)->PxVerdict {
    if reqs.count<1 {return PxVerdict.{status=PxStatus.shape};}
    v:=px_check(reqs[0]);
    for i:1..<reqs.count {
        if v.status!=PxStatus.ok {return v;}
        v=px_admit(reqs[i-1],reqs[i]);v.request=i;
    }
    return v;
}
```

## A session by hand

The hand session puts chapter1's hand trace, logged as in chapter2,
behind a frozen prefix:

- System text `[1, 2, 3, 4]`.
- Three tools registered in the order 30, 10, 20, each with a two-token
  schema `[id, id+1]`. `px_open` lays them out as 10, 20, 30.
- A header `[5, 6]`.

The frozen prefix is 12 tokens: system at positions 0 to 3, tool 10 at 4
and 5, tool 20 at 6 and 7, tool 30 at 8 and 9, the header at 10 and 11.

Chapter2's log holds 11 events and 30 tokens. Its three request events,
at events 2, 5 and 8, saw 5, 16 and 27 tokens; a request at the log's end
sees all 30. Request `k` (counting from 0) gets the tail
`[1000 + k, k]`. Its length is 12 + history + 2 and its boundary is 12 +
history:

| Request | Event | History | Tokens | Boundary | Shared with the previous |
| --- | --- | --- | --- | --- | --- |
| 0 | 2 | 5 | 19 | 17 | — |
| 1 | 5 | 16 | 30 | 28 | 17 |
| 2 | 8 | 27 | 41 | 39 | 28 |
| 3 | end | 30 | 44 | 42 | 39 |

The shared length is exactly the previous boundary. At that position the
previous request holds its tail, a number of 1000 or more, and the next
request holds the next history token, which is below 330. Each request
keeps everything before the previous boundary, so the checker admits all
four.

Now the broken assemblies, each checked against request 0:

- The header's first value changes from 5 to 7 mid-session. The first
  difference is position 10, in section 4 (`header`): `prefix`.
- Tool 20's schema changes its second token. Tool 20 sits at 6 and 7, so
  the difference is position 7, in section 2 (`tool`): `prefix`.
- A tool with id 15 is added mid-session. Sorted, it goes between 10 and
  20, so position 6 now holds 15 instead of 20: `prefix` at position 6,
  section 2.
- The clock at the top. The tail is section 0 and the system text section
  1, out of order: `order` at section 1, which starts at position 2. The
  checker refuses this layout before comparing any tokens.
- Tools in registration order (30, 10, 20). Section 2 holds tool 10 after
  tool 30: `tools` at section 2, position 6.
- Two tools with the same id cannot be opened: `tools`.
- With the reply at event 6 redacted, the request at event 5 still
  assembles (its history ends before event 6), but the request at event 8
  is refused with `log`.

The checks assert each of these numbers:

<!-- source: psionic/lib/harness/prefix.coax check-hand -->
```jai
#test "the hand session: chapter 201's trace behind a frozen prefix, shared lengths by hand" #steps 4000000000 {
    ev:[16]LgEvent;tk:[48]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];#assert pxc_trace(l[..]).status==LgStatus.ok;
    sy:=PXC_SYSTEM;hd:=PXC_HEADER;t:=pxc_tools();s:=px_open(sy[..],t[..],hd[..]);
    // system 0..3, tools 10, 20, 30 at 4..9 (sorted), header 10..11.
    #assert s.status==PxStatus.ok && s.n==12 && s.sections==5;
    keys:=[5]int.[0,10,20,30,0];at:=[5]int.[0,4,6,8,10];
    for i:0..<5 {#assert s.section[i].key==keys[i] && s.section[i].at==at[i];}
    #assert s.tok[4]==10 && s.tok[5]==11 && s.tok[6]==20 && s.tok[9]==31 && s.tok[11]==6;
    // The three requests saw 5, 16 and 27 tokens; the next one sees all 30.
    index:=[4]int.[2,5,8,11];n:=[4]int.[19,30,41,44];
    r:[4]PxRequest;
    for k:0..<4 {tail:=pxc_tail(k);r[k]=px_assemble(s,l[0],index[k],tail[..]);
        #assert r[k].status==PxStatus.ok && r[k].n==n[k] && px_boundary(r[k])==n[k]-2 && r[k].tok[12]==320 && r[k].tok[n[k]-2]==1000+k;}
    // Each request shares exactly the previous one's prefix up to its boundary: 17, 28, 39.
    for k:0..<4 {for i:0..<7 {#assert r[k].section[i].key==(ifx i>0 && i<4 then 10*i else 0);}}
    for k:1..<4 {v:=px_admit(r[k-1],r[k]);#assert v.status==PxStatus.ok && v.shared==n[k-1]-2 && v.at==n[k-1]-2;}
    v:=px_session(r[..]);#assert v.status==PxStatus.ok && v.request==3 && v.shared==39;
    // Its history passes chapter 201's gate: the tokens each request event saw.
    for k:0..<3 {#assert lg_admit(l[0],index[k],r[k].tok[12..<n[k]-2]).status==LgStatus.ok;}
}
```

<!-- source: psionic/lib/harness/prefix.coax check-refuse -->
```jai
#test "each broken assembly, with its reason, section and position" #steps 4000000000 {
    ev:[16]LgEvent;tk:[48]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];#assert pxc_trace(l[..]).status==LgStatus.ok;
    sy:=PXC_SYSTEM;hd:=PXC_HEADER;t:=pxc_tools();s:=px_open(sy[..],t[..],hd[..]);a:=pxc_tail(0);b:=pxc_tail(1);
    r0:=px_assemble(s,l[0],2,a[..]);r1:=px_assemble(s,l[0],5,b[..]);
    // A header value that changes mid-session: position 10, section 4.
    h:=[2]int.[7,6];x:=px_assemble(px_open(sy[..],t[..],h[..]),l[0],5,b[..]);
    v:=px_admit(r0,x);#assert v.status==PxStatus.prefix && v.at==10 && v.section==4 && v.kind==PxKind.header && v.shared==10;
    // A tool schema edited, and a tool added, mid-session: both inside the tool sections.
    u:=t;u[2].tok[1]=99;v=px_admit(r0,px_assemble(px_open(sy[..],u[..],hd[..]),l[0],5,b[..]));
    #assert v.status==PxStatus.prefix && v.at==7 && v.section==2 && v.kind==PxKind.tool;
    w:[4]PxTool;for i:0..<3 {w[i]=t[i];}w[3]=PxTool.{id=15,count=1};
    v=px_admit(r0,px_assemble(px_open(sy[..],w[..],hd[..]),l[0],5,b[..]));
    #assert v.status==PxStatus.prefix && v.at==6 && v.section==2 && v.kind==PxKind.tool;
    // History from another log: its second token differs, position 13, section 5.
    e2:[16]LgEvent;t2:[48]int;g:=[1]LgLog.[LgLog.{events=e2[..],tokens=t2[..]}];q:=[3]int.[320,63,5];#assert lg_say(g[..],LgKind.user,q[..])==LgStatus.ok;
    v=px_admit(r1,px_assemble(s,g[0],1,b[..]));#assert v.status==PxStatus.prefix && v.at==13 && v.section==5 && v.kind==PxKind.history && v.shared==13;
    // A request that drops the tail's place but keeps the history: fine; one shorter than the boundary is not.
    v=px_admit(r1,r0);#assert v.status==PxStatus.prefix && v.at==17 && v.section==5 && v.shared==17;
    // The clock at the top: refused for its place before anything is compared.
    c:=px_naive(sy[..],t[..],hd[..],l[0],a[..],true);
    v=px_admit(r0,c);#assert v.status==PxStatus.order && v.section==1 && v.at==2 && v.kind==PxKind.system;
    v=px_check(c);#assert v.status==PxStatus.order && v.section==1;
    // Tools in registration order: refused at the first tool out of order.
    d:=px_naive(sy[..],t[..],hd[..],l[0],a[..],false);
    v=px_check(d);#assert v.status==PxStatus.tools && v.section==2 && v.at==6 && v.kind==PxKind.tool;
    for i:0..<7 {#assert c.section[i].key==(ifx i==2 then 30 else ifx i==3 then 10 else ifx i==4 then 20 else 0) && d.section[i].key==(ifx i==1 then 30 else ifx i==2 then 10 else ifx i==3 then 20 else 0);}
    // The naive assembler does not refuse equal ids; the checker does.
    two:=[2]PxTool.[t[0],t[0]];v=px_check(px_naive(sy[..],two[..],hd[..],l[0],a[..],false));#assert v.status==PxStatus.tools && v.section==2;
    // A request that is a strict prefix of the previous one shares only its own length.
    p0:=[2]int.[1,0];p1:=[1]int.[1];v=px_admit(px_add(PxRequest.{},PxKind.history,0,p0[..]),px_add(PxRequest.{},PxKind.history,0,p1[..]));
    #assert v.status==PxStatus.prefix && v.at==1 && v.shared==1;
    // A duplicate tool id cannot be opened; a tool id 0 after the system section is fine.
    w[3].id=20;#assert px_open(sy[..],w[..],hd[..]).status==PxStatus.tools;
    w[3].id=0;z:=px_open(sy[..],w[..],hd[..]);#assert z.status==PxStatus.ok && z.section[1].key==0 && px_check(z).status==PxStatus.ok;
    // A session check names the request.
    rs:=[3]PxRequest.[r0,r1,x];v=px_session(rs[..]);#assert v.status==PxStatus.prefix && v.request==2 && v.at==10;
    rs[1]=c;v=px_session(rs[..]);#assert v.status==PxStatus.order && v.request==1;
    rs[0]=d;v=px_session(rs[..]);#assert v.status==PxStatus.tools && v.request==0;
    #assert px_session(rs[0..<0]).status==PxStatus.shape && px_session(rs[2..<3]).status==PxStatus.ok;
}
```

## Measuring the cache

`px_measure` runs a list of requests one at a time through Psionic chapter 176's
paged cache, and reads how many prompt blocks each one reused. Time to
first token for a request of `n` tokens with `c` of them cached is chapter
177's `rr_prefill`: one step of `n - c` rows. The total adds one step of
one row for each further output token, as Psionic chapter 177's `rr_cost` does.
Psionic chapter 177's file cannot be loaded beside chapter2's (a name in each
collides), so `px_prefill` and `px_cost` restate its two lines, and a
check compares them with Psionic chapter 177's own recorded values.

<!-- source: psionic/lib/harness/prefix.coax measure -->
```jai
// Chapter 177's cost model (rr_prefill and rr_cost in serve-route.coax), in
// modeled microseconds: the uncached prompt positions run as one step's
// rows, and each further output token is a step of one row.
px_prefill :: (prompt:int,cached:int)->int {return cb_step_ns(prompt-cached)/1000;}
px_cost :: (prompt:int,output:int,cached:int)->int {return (cb_step_ns(prompt-cached)+(output-1)*cb_step_ns(1))/1000;}
// The requests run one at a time, in order, on one model through chapter
// 176's paged cache with every tier at 16 frames of page positions. The
// scheduler's other settings take their largest values; with one request
// at a time, none of them changes what is reused. mem holds 1728 values.
px_measure :: (reqs:[]PxRequest,output:int,page:int,mem:[]f64)->PxMeasure {
    m:PxMeasure;m.status=PxStatus.shape;
    if reqs.count>PG_REQUESTS {return m;}
    q:[16]CbRequest;model:[16]int;tok:[512]int;
    for r,i:reqs {
        if r.n>PG_PROMPT {return m;}
        q[i]=CbRequest.{prompt=r.n,output=output};
        for p:0..<r.n {tok[i*PG_PROMPT+p]=r.tok[p];}
    }
    cfg:=PgConfig.{page=page,dim=PG_DIM};for t:0..<3 {cfg.cap[t]=PG_TIER;}
    b:=CbConfig.{slots=1,tokens=CB_TOKENS,clients=1,steps=CB_STEPS};b.weight[0]=CB_WEIGHT;
    s:=pg_run(cfg,b,q[0..<reqs.count],model[0..<reqs.count],tok[..],mem);
    if s.status!=PgStatus.ok {return m;}
    ttft:[16]int;
    for r,i:reqs {
        c:=s.reused[i]*page;m.reused[i]=s.reused[i];m.hit+=s.reused[i];m.blocks+=(r.n+page-1)/page;
        m.tokens+=r.n;m.cached+=c;ttft[i]=px_prefill(r.n,c);m.prefill+=ttft[i];m.cost+=px_cost(r.n,output,c);
    }
    m.status=PxStatus.ok;m.requests=reqs.count;
    // Nearest rank: P95 of at most 16 values is rank ceil(0.95 n) = n, the
    // largest, so it is taken as rank 100.
    m.p50=cb_rank(ttft[0..<reqs.count],50);m.p95=cb_rank(ttft[0..<reqs.count],100);
    return m;
}
```

The scripted workload is three sessions of four requests. Each session
uses the same system text, tools and header as the hand session, but
registers its tools in a different order: 30, 10, 20; then 20, 30, 10;
then 10, 20, 30. Before each request the session logs one 4-token text
event, so request `k` (1 to 4) of a session holds 12 + 4k history-and-prefix
tokens and a 2-token tail: 18, 22, 26 and 30 tokens.

Work out the stable assembler by hand, with pages of 4 positions:

- Psionic chapter 176 never reuses the block that holds a request's last position,
  so a request of `n` tokens can reuse at most `(n - 1) / 4` blocks
  (integer division). It caches every whole block it computed.
- Request `k` holds 14 + 4k tokens; its whole blocks are the first 3 + k,
  which cover positions 0 to 11 + 4k: the prefix and the history, exactly
  up to the boundary. The tail falls in the next, partial block.
- Request `k + 1` matches those 3 + k blocks, and may reuse up to
  (17 + 4k) / 4 = 4 + k. It reuses 3 + k: 4, 5 and 6 blocks for the
  second, third and fourth requests.
- The first session's first request reuses nothing. The second and third
  sessions' first requests share the 12-token prefix with the first
  session: 3 blocks. The fourth block holds their own history.
- Prompt blocks: request `k` has (14 + 4k + 3) / 4 = 4 + k, so 5 + 6 + 7 +
  8 = 26 a session and 78 in all. Reused: 0 + 4 + 5 + 6 = 15 for the
  first session, 3 + 4 + 5 + 6 = 18 for each of the others, 51 in all.
  That is 204 of 288 prompt tokens not computed again.

With tools in registration order, the second and third sessions' first
requests share only the system block with the first session, because
position 4 already differs (30, 20, 10 against each other): 1 block
instead of 3, so 47 in all. With the clock at the top, position 0 differs
in every request and nothing is reused.

Times, from Psionic chapter 175's step cost of 17,792,472 ns plus 1,459,182 ns per
row after the first:

- A cold 18-token request: (17,792,472 + 17 × 1,459,182) / 1000 = 42,598
  µs to its first token.
- A request with 6 uncached rows (every later request of the stable
  assembler, and the other sessions' first requests): (17,792,472 + 5 ×
  1,459,182) / 1000 = 25,088 µs.
- The stable assembler's modeled prefill: 42,598 + 11 × 25,088 = 318,566 µs.

<!-- source: psionic/lib/harness/prefix.coax check-cache -->
```jai
#test "the scripted sessions on chapter 176's cache, block by block" #steps 4000000000 {
    q:[12]PxRequest;mem:[1728]f64;
    // Stable: the second session's first request reuses the shared 12-token prefix, 3 blocks of 4.
    #assert pxc_sessions(0,q[..])==12;
    // Each session passes the checker; across sessions the history differs, so they are checked apart.
    for j:0..<3 {#assert px_session(q[4*j..<4*j+4]).status==PxStatus.ok;}
    v:=px_session(q[3..<5]);#assert v.status==PxStatus.prefix && v.at==12 && v.kind==PxKind.history;
    m:=px_measure(q[..],1,4,mem[..]);
    hit:=[12]int.[0,4,5,6,3,4,5,6,3,4,5,6];
    for i:0..<12 {#assert m.reused[i]==hit[i] && q[i].n==18+4*(i%4);}
    #assert m.status==PxStatus.ok && m.requests==12 && m.blocks==78 && m.hit==51 && m.tokens==288 && m.cached==204;
    // Tools in registration order: the other sessions share only the system block.
    #assert pxc_sessions(1,q[..])==12;v=px_session(q[..]);#assert v.status==PxStatus.tools && v.request==0;
    m=px_measure(q[..],1,4,mem[..]);hit=[12]int.[0,4,5,6,1,4,5,6,1,4,5,6];
    for i:0..<12 {#assert m.reused[i]==hit[i];}
    #assert m.blocks==78 && m.hit==47 && m.cached==188;
    // The clock at the top: nothing is ever reused.
    #assert pxc_sessions(2,q[..])==12 && px_session(q[..]).status==PxStatus.order;
    m=px_measure(q[..],1,4,mem[..]);#assert m.blocks==78 && m.hit==0 && m.cached==0;
    // With nothing cached, each request's time is its length's: 18, 22, 26, 30 tokens three times each.
    #assert m.p50==px_prefill(22,0) && m.p95==px_prefill(30,0);
    t:[12]int;for i:0..<12 {t[i]=px_prefill(q[i].n,0);}#assert cb_rank(t[..],95)==m.p95 && cb_rank(t[..],50)==m.p50;
}
```

## The program

The example, [prefix-stable.coax](../../psionic/examples/prefix-stable.coax), logs
chapter1's hand trace, assembles its four requests and checks them,
prints each broken assembly's verdict, and runs the three assemblers'
sessions through the cache.

<!-- source: psionic/examples/prefix-stable.coax example -->
```jai
main :: () {
    // Chapter 200's hand trace, logged as in chapter 201.
    ev:[16]LgEvent;tk:[48]int;l:=[1]LgLog.[LgLog.{events=ev[..],tokens=tk[..]}];
    print("hand trace logged: %, % events, % tokens\n",pxc_trace(l[..]).status,l[0].count,l[0].used);
    // The session opens once: tools registered as 30, 10, 20 are laid out by id.
    sy:=PXC_SYSTEM;hd:=PXC_HEADER;t:=pxc_tools();s:=px_open(sy[..],t[..],hd[..]);
    print("frozen prefix: %, % tokens\n",s.status,s.n);ex_sections(s);
    // Each request event, then the end of the log: history, then the tail.
    index:=[4]int.[2,5,8,11];r:[4]PxRequest;
    for k:0..<4 {
        tail:=pxc_tail(k);r[k]=px_assemble(s,l[0],index[k],tail[..]);
        print("request at event %: %, % tokens, boundary %",index[k],r[k].status,r[k].n,px_boundary(r[k]));
        if k>0 {v:=px_admit(r[k-1],r[k]);print(", shares % with the previous: %",v.shared,v.status);}
        print("\n");
    }
    ex_sections(r[3]);
    ex_verdict("the session",px_session(r[..]));
    // Broken assemblies, each against request 0.
    a:=pxc_tail(0);b:=pxc_tail(1);
    h:=[2]int.[7,6];ex_verdict("header value changed mid-session",px_admit(r[0],px_assemble(px_open(sy[..],t[..],h[..]),l[0],5,b[..])));
    u:=t;u[2].tok[1]=99;ex_verdict("tool 20's schema edited mid-session",px_admit(r[0],px_assemble(px_open(sy[..],u[..],hd[..]),l[0],5,b[..])));
    w:[4]PxTool;for i:0..<3 {w[i]=t[i];}w[3]=PxTool.{id=15,count=1};
    ex_verdict("tool 15 added mid-session",px_admit(r[0],px_assemble(px_open(sy[..],w[..],hd[..]),l[0],5,b[..])));
    ex_verdict("the clock at the top",px_admit(r[0],px_naive(sy[..],t[..],hd[..],l[0],a[..],true)));
    ex_verdict("tools in registration order",px_check(px_naive(sy[..],t[..],hd[..],l[0],a[..],false)));
    w[3].id=20;print("two tools with id 20: %\n",px_open(sy[..],w[..],hd[..]).status);
    e:=ev;k:=tk;g:=[1]LgLog.[LgLog.{events=e[..],tokens=k[..],count=11,used=30}];
    print("reply at event 6 redacted: %; request at event 5: %, at event 8: %\n",lg_redact(g[..],6),px_assemble(s,g[0],5,a[..]).status,px_assemble(s,g[0],8,a[..]).status);
    // Three sessions of four requests on chapter 176's cache, pages of 4, 3 output tokens each.
    names:=[3]string.["stable","registration order","clock at the top"];q:[12]PxRequest;mem:[1728]f64;
    for x:0..<3 {
        n:=pxc_sessions(x,q[..]);m:=px_measure(q[0..<n],3,4,mem[..]);
        print("%: checker % % %; blocks reused % of % (% per mille), prefill tokens saved % of %\n",names[x],px_session(q[0..<4]).status,px_session(q[4..<8]).status,px_session(q[8..<12]).status,m.hit,m.blocks,1000*m.hit/m.blocks,m.cached,m.tokens);
        print("  reused by request:");for i:0..<n {print(" %",m.reused[i]);}
        print("\n  modeled prefill % us, total % us, time to first token P50 % us, P95 % us\n",m.prefill,m.cost,m.p50,m.p95);
    }
}
```

```shell
coax/bin/coax run psionic/examples/prefix-stable.coax
```

```text
hand trace logged: ok, 11 events, 30 tokens
frozen prefix: ok, 12 tokens
  section 0 system key 0: tokens 0 to 3
  section 1 tool key 10: tokens 4 to 5
  section 2 tool key 20: tokens 6 to 7
  section 3 tool key 30: tokens 8 to 9
  section 4 header key 0: tokens 10 to 11
request at event 2: ok, 19 tokens, boundary 17
request at event 5: ok, 30 tokens, boundary 28, shares 17 with the previous: ok
request at event 8: ok, 41 tokens, boundary 39, shares 28 with the previous: ok
request at event 11: ok, 44 tokens, boundary 42, shares 39 with the previous: ok
  section 0 system key 0: tokens 0 to 3
  section 1 tool key 10: tokens 4 to 5
  section 2 tool key 20: tokens 6 to 7
  section 3 tool key 30: tokens 8 to 9
  section 4 header key 0: tokens 10 to 11
  section 5 history key 0: tokens 12 to 41
  section 6 tail key 0: tokens 42 to 43
the session: ok
header value changed mid-session: prefix (changes bytes before the previous request's boundary), section 4 (header), position 10
tool 20's schema edited mid-session: prefix (changes bytes before the previous request's boundary), section 2 (tool), position 7
tool 15 added mid-session: prefix (changes bytes before the previous request's boundary), section 2 (tool), position 6
the clock at the top: order (a section out of the fixed order), section 1 (system), position 2
tools in registration order: tools (tool schemas not sorted by id), section 2 (tool), position 6
two tools with id 20: tools
reply at event 6 redacted: ok; request at event 5: ok, at event 8: log
stable: checker ok ok ok; blocks reused 51 of 78 (653 per mille), prefill tokens saved 204 of 288
  reused by request: 0 4 5 6 3 4 5 6 3 4 5 6
  modeled prefill 318566 us, total 745586 us, time to first token P50 25088 us, P95 42598 us
registration order: checker tools tools ok; blocks reused 47 of 78 (602 per mille), prefill tokens saved 188 of 288
  reused by request: 0 4 5 6 1 4 5 6 1 4 5 6
  modeled prefill 341912 us, total 768932 us, time to first token P50 25088 us, P95 42598 us
clock at the top: checker order order order; blocks reused 0 of 78 (0 per mille), prefill tokens saved 0 of 288
  reused by request: 0 0 0 0 0 0 0 0 0 0 0 0
  modeled prefill 616239 us, total 1043256 us, time to first token P50 48435 us, P95 60108 us
```

Every number matches the hand work. The stable assembler reuses 51 of 78
prompt blocks (653 per mille) and saves 204 of 288 prompt tokens; its
modeled P50 time to first token is 25,088 µs against 48,435 µs with the
clock at the top. Tools in registration order lose only the cross-session
blocks here, 4 of them, because each session in this workload keeps one
order; the clock at the top loses everything.

## What the checks cover

- **The hand session:** the frozen prefix's layout, each request's length,
  boundary and shared length, the session verdict, and each history
  admitted by chapter2's `lg_admit` for the request event that saw it.
- **Each broken assembly** with its status, section, kind and position,
  and `px_session` naming the request.
- **The log:** an index that is not a request event or the log's end, a
  log that does not verify, a redacted event in the history and not after
  it, and a refused session staying refused.
- **Bounds:** 8 tools and 9, schema lengths 8, 9 and −1, a negative id, 64
  tokens and 65, 16 sections and 17, and the boundary with and without a
  tail.
- **The cache:** each request's reused blocks and the totals for the three
  assemblers, and that requests from different sessions are refused when
  checked as one session (their history differs at position 12).
- **Cost:** `px_cost` and `px_prefill` against Psionic chapter 177's recorded
  values for `rr_cost` and `rr_prefill`, the hand times above, P50 and
  P95, and the measurement's refusals (17 requests, a 33-token request, a
  page of 9, no requests).
- **Every assembled request extends the previous one exactly:** 80 random
  sessions (1 to 6 system tokens, 1 to 4 tools with distinct ids in random
  order, 0 to 3 header tokens, 1 to 6 logged text events, 0 to 3 tail
  tokens). Each request must equal a brute-force assembler's tokens, with
  its own sort; must pass the checker; and must share with the previous
  request exactly the brute-force longest common prefix, at least the
  previous boundary. Then one random token is changed: before the previous
  boundary it must be refused at exactly that position, in the section
  that holds it; after, admitted.
- **Swapped tools and a section out of place:** 100 random sessions with
  two adjacent tool sections swapped (refused as `tools` at the second) and
  a tool section added after the header (refused as `order`).

Mutation of `harness/prefix.coax` leaves no survivors.

## What remains

- Time to first token measured on a named host and model, with P50 and P95
  over a long session, rather than modeled from Psionic chapter 175's step cost.
- Real system prompts and tool schemas as tokens from a tokenizer, and
  sessions longer than Psionic chapter 176's bounds of 16 requests of 32 tokens.
- Condensation now runs in [chapter7](ch007-condense-the-history.md),
  whose `cd_admit` wraps `px_admit` with the one exception: a break at a
  logged condensation. Merging that exception into this chapter's
  assembler, so one checker covers both, remains.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
