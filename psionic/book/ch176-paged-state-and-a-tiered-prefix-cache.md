# 176. Paged state and a tiered prefix cache

> **Status: partly implemented.** Paged state, copy on write, the prefix
> trie, three tiers with eviction, and admission that charges pages (profile
> pg-pages-v1) run on the CPU against a stand-in model, checked by a hand
> trace, brute-force models and token-for-token equality with cold runs.
> The tiers are three regions of one array in one process: no GPU memory,
> host memory or disk file is involved, and no transfer is timed. Real model
> state, 128-token pages, layer-by-layer restore, storage that outlives a
> compute machine, retention per traffic type and chapter 172's report are
> not built; the last section lists them.

Requests that share a prefix repeat the same prefill. A system prompt, a
tool list or a long document at the front of many requests is computed
again for each one, and a request that waits for it pays the time to first
token. This chapter stores model state in fixed-size pages so that requests
can share the pages of a common prefix, keeps finished prompt blocks in a
prefix cache across requests, and lets that cache spill from device memory
to host memory to local disk instead of dropping what it holds.

Everything here lives in [`serve-pages.coax`](../lib/serve-pages.coax), an
`#authority` file: it decides which request is admitted, which cached block
is reused, and which one is moved or dropped. It builds on chapter 175's
scheduler and reuses its arrivals, its row planning and its fair order;
what changes is the charge. Chapter 175 charged each request its whole
state in positions. Here a request is charged the pages it will newly hold,
after the cached blocks it reuses.

## Pages, frames and tables

A page holds the state of `page` consecutive positions, `dim` values each.
Pages live in frames. Frames are numbered across three tiers: frames 0 to
15 are the device, 16 to 31 host memory, and 32 to 47 local disk, of which
`cap[t]` are in use in tier `t`. A request's page table lists the frame of
each of its blocks: position `p` is in block `p / page`, at offset
`p % page` within it.

<!-- source: psionic/lib/serve-pages.coax types -->
```jai
// page: positions per page. dim: values per position. hybrid: the model also
// carries a recurrent state of dim values, which is not KV and cannot be
// paged by position: each running request holds one more page for it, and
// each cached block's page keeps a snapshot of it after the block's last
// position. cap: frames in the device, host and disk tiers.
PgConfig :: struct {page:int;dim:int;hybrid:bool;cap:[3]int;}
// Reference counts, and a free stack per tier.
PgPool :: struct {ref:[48]int;stack:[48]int;nfree:[3]int;got:int;}
// One request's page table: the frame of each block, and the cache node
// that block is shared with (-1 while it is private).
PgTable :: struct {n:int;page:[16]int;node:[16]int;}
// One cached prompt block: its parent node (-1 - model for a first block),
// the chained hash of the prefix through it, its tokens, its frame (-1 when
// the slot is empty), and the clock reading of its last use.
PgNode :: struct {parent:int;hash:int;tok:[8]int;frame:int;stamp:int;keep:bool;}
```

A hybrid model, such as Elephant or Mammoth, also carries recurrent state:
a fixed number of values that every position updates, not one entry per
position. It cannot be split into pages by position, and it cannot be
rebuilt from the KV pages. The chapter handles it in two places. Each
running request of a hybrid model holds one extra page for its live
recurrent state, and each block's page has room for one more row of `dim`
values: a snapshot of the recurrent state after the block's last position.
A request that reuses a cached prefix restores its live state from the
snapshot in the last reused block.

Each frame has a reference count, and each tier has a free stack. Taking a
frame pops the stack; dropping the last reference pushes it back. The stack
starts with the lowest frame on top, so a fresh pool hands out frames 0, 1,
2 and so on, which keeps traces readable.

<!-- source: psionic/lib/serve-pages.coax pool -->
```jai
pg_tier :: (f:int)->int {return f/PG_TIER;}
// Every frame free; each tier's stack pops its lowest frame first.
pg_pool :: (c:PgConfig)->PgPool {
    p:PgPool;
    for t:0..<3 {for k:0..<c.cap[t] {p.stack[t*PG_TIER+k]=t*PG_TIER+c.cap[t]-1-k;}p.nfree[t]=c.cap[t];}
    return p;
}
// Take the top of tier t's free stack (got = -1 when it is empty).
pg_alloc :: (from:PgPool,t:int)->PgPool {
    p:=from;p.got=-1;
    if p.nfree[t]==0 {return p;}
    p.nfree[t]-=1;f:=p.stack[t*PG_TIER+p.nfree[t]];
    p.ref[f]=1;p.got=f;
    return p;
}
pg_retain :: (from:PgPool,f:int)->PgPool {p:=from;p.ref[f]+=1;return p;}
// Drop one reference; the last one returns the frame to its tier's stack.
pg_release :: (from:PgPool,f:int)->PgPool {
    p:=from;p.ref[f]-=1;
    if p.ref[f]==0 {t:=pg_tier(f);p.stack[t*PG_TIER+p.nfree[t]]=f;p.nfree[t]+=1;}
    return p;
}
pg_width :: (c:PgConfig)->int {return (c.page+1)*c.dim;}
pg_copy :: (c:PgConfig,mem:[]f64,from:int,to:int) {
    w:=pg_width(c);
    for j:0..<w {mem[to*w+j]=mem[from*w+j];}
}
```

## Copy on write

Two tables can share a frame. `pg_fork` gives a second table every page of
the first, adding one reference to each frame. A write to a frame that more
than one holder references copies the frame first, so the write never shows
through anyone else's table; that is copy on write. Parallel samples from
one prompt, or a beam that branches, share every page until they diverge,
and then copy only the page they write.

<!-- source: psionic/lib/serve-pages.coax cow -->
```jai
// Share every page of table a with table b.
pg_fork :: (from:PgStore,a:int,b:int)->PgStore {
    s:=from;s.table[b]=s.table[a];
    for k:0..<s.table[a].n {s.pool=pg_retain(s.pool,s.table[a].page[k]);}
    return s;
}
// Write one position's values. A frame that another holder also references
// is copied first, so the write never shows through anyone else's table.
pg_write :: (from:PgStore,mem:[]f64,i:int,pos:int,x:[]f64)->PgStore {
    s:=from;k:=pos/s.cfg.page;f:=s.table[i].page[k];
    if s.pool.ref[f]>1 {
        s=pg_take(s,mem);
        pg_copy(s.cfg,mem,f,s.pool.got);s.pool=pg_release(s.pool,f);
        s.table[i].page[k]=s.pool.got;f=s.pool.got;s.cow+=1;
    }
    w:=pg_width(s.cfg);
    for j:0..<s.cfg.dim {mem[f*w+pos%s.cfg.page*s.cfg.dim+j]=x[j];}
    return s;
}
pg_free_table :: (from:PgStore,i:int)->PgStore {
    s:=from;
    for k:0..<s.table[i].n {s.pool=pg_release(s.pool,s.table[i].page[k]);}
    s.table[i].n=0;
    if s.live[i]>=0 {s.pool=pg_release(s.pool,s.live[i]);s.live[i]=-1;}
    return s;
}
```

The serving path below never writes a shared frame: a request writes only
blocks it allocated itself. The checks assert that on every write.

## The prefix trie

Finished prompt blocks enter a trie. A node is one block. Its key is its
parent node and its exact tokens; a first block's parent is `-1 - model`,
so the model identity is the root and two models never share a node, even
for the same tokens. This is chapter 36's rule, prefix reuse by model
identity and exact token prefix, applied block by block.

Each node also stores a chained hash: a first block's hash starts from the
model identity, and each block continues from its parent's hash. The hash
narrows the search, but it never decides a hit. The parent and the tokens
decide it, so two prefixes whose hashes collide are still two prefixes.

<!-- source: psionic/lib/serve-pages.coax hash -->
```jai
// The chained hash: a first block starts from the model identity.
pg_hash :: (h:int,tok:[]int)->int {
    x:=h;
    for t:tok {x=(x*31+t+1)%PG_HASH;}
    return x;
}
// The node for this parent and these exact tokens, or -1. The hash narrows
// the search; the parent and the tokens decide it, so a collision is a miss.
pg_child :: (node:[]PgNode,parent:int,h:int,tok:[]int)->int {
    for n:0..<PG_NODES {
        d:=node[n];
        if d.frame>=0 && d.hash==h && d.parent==parent {
            same:=true;
            for j:0..<tok.count {if d.tok[j]!=tok[j] {same=false;}}
            if same {return n;}
        }
    }
    return -1;
}
// The cached blocks a prompt can reuse: its longest cached prefix of whole
// blocks, never the block that holds its last position, because that
// position must run to produce the first output token.
pg_match :: (from:PgStore,model:int,tok:[]int,prompt:int)->PgPath {
    s:=from;p:=s.cfg.page;r:PgPath;parent:=-1-model;h:=model;
    for k:0..<(prompt-1)/p {
        b:[8]int;for j:0..<p {b[j]=tok[k*p+j];}
        h=pg_hash(h,b[0..<p]);
        n:=pg_child(s.node[..],parent,h,b[0..<p]);
        if n<0 {return r;}
        r.node[k]=n;r.n=k+1;parent=n;
    }
    return r;
}
```

`pg_match` never reuses the block that holds the last prompt position. That
position must run through the model to produce the first output token, and
for a hybrid model the recurrent state has to be the one after that
position, which only a snapshot at a block boundary can restore. A prompt of
5 positions in pages of 2 may reuse 2 blocks and runs position 4; a prompt
of 4 may reuse only 1 block and runs positions 2 and 3, because block 1
holds position 3.

## Three tiers

A cached block's page can be on the device, in host memory or on disk.
When a tier needs a free frame, `pg_spill` moves one node down a tier. A
node can move when nothing else needs it where it is:

- it is not part of the admission in progress (`keep`);
- on the device, the cache holds its only reference, so no running request
  reads it;
- no child of it is in the same tier.

Among those, the least recently used node moves first: the one whose last
use was the earliest step, the lowest node on a tie. If the tier below is
full, it makes room first in the same way. A node on disk, or one whose
lower tiers are full of nodes that cannot move, is dropped together with
every node under it.

The last rule keeps an invariant: a node is never in a faster tier than its
parent. A lookup walks from the root, so a parent must be at least as easy
to reach as its child, and a cached prefix is never cut in the middle.
Restoring a prefix brings its nodes back to the device root first.

<!-- source: psionic/lib/serve-pages.coax tiers -->
```jai
// Make one frame free in tier t by moving its least recently used movable
// node down a tier. Movable: not kept for the admission in progress, no
// child in the same tier, and on the device referenced by the cache alone.
// A node with nowhere to go (on disk, or the tier below is full with
// nothing movable) is dropped with everything under it.
pg_spill :: (from:PgStore,mem:[]f64,t:int)->PgStore {
    s:=from;v:=-1;
    for n:0..<PG_NODES {
        d:=s.node[n];
        if d.frame<0 || pg_tier(d.frame)!=t || d.keep || s.pool.ref[d.frame]>1 || pg_child_in(s.node[..],n,t) {continue;}
        if v<0 || d.stamp<s.node[v].stamp {v=n;}
    }
    if v<0 {return s;}
    f:=s.node[v].frame;
    if t<2 && s.pool.nfree[t+1]==0 {s=pg_spill(s,mem,t+1);}
    if t<2 && s.pool.nfree[t+1]>0 {
        s.pool=pg_alloc(s.pool,t+1);pg_copy(s.cfg,mem,f,s.pool.got);s.pool=pg_release(s.pool,f);
        s.node[v].frame=s.pool.got;s.down[t]+=1;
        return s;
    }
    for n:0..<PG_NODES {
        if s.node[n].frame>=0 && pg_under(s.node[..],n,v) {s.pool=pg_release(s.pool,s.node[n].frame);s.node[n].frame=-1;s.drop+=1;}
    }
    return s;
}
pg_child_in :: (node:[]PgNode,n:int,t:int)->bool {
    for c:0..<PG_NODES {if node[c].frame>=0 && node[c].parent==n && pg_tier(node[c].frame)==t {return true;}}
    return false;
}
// Is node n the node v or under it?
pg_under :: (node:[]PgNode,n:int,v:int)->bool {
    a:=n;
    while a>=0 {if a==v {return true;}a=node[a].parent;}
    return false;
}
// A device frame for a request: free one first if none is.
pg_take :: (from:PgStore,mem:[]f64)->PgStore {
    s:=from;
    if s.pool.nfree[0]==0 {s=pg_spill(s,mem,0);}
    s.pool=pg_alloc(s.pool,0);
    return s;
}
```

A move copies the frame's bytes to the new frame and frees the old one. The
checks compare every cached frame, on whichever tier it sits, with the
bytes its tokens give, after every step.

## Admission charges pages

`pg_start` checks the page configuration (1 to 8 positions per page, 1 to
4 values per position, up to 16 frames per tier and at least one on the
device) and each request (up to 16 requests, prompts of up to 32 tokens, at
most 16 pages in a table, a model identity of 0 or more). It then starts
chapter 175's scheduler with its state budget set to the device tier in
positions, and a state charge of one page for a hybrid model. That lets
chapter 175 refuse at arrival any request that could never fit on the
device, by the same rule as before: when `cap[0] * page` is a whole number
of pages, a request fits in positions exactly when it fits in pages.

<!-- source: psionic/lib/serve-pages.coax start -->
```jai
// Refuse a page configuration or a request outside the bounds; the
// scheduler's state budget becomes the device tier in positions, and a
// hybrid model's recurrent state is charged one page per request.
pg_start :: (cfg:PgConfig,batch:CbConfig,reqs:[]CbRequest,model:[]int,tok:[]int)->PgState {
    s:PgState;s.cfg=cfg;s.status=PgStatus.pages;
    if cfg.page<1 || cfg.page>PG_PAGE || cfg.dim<1 || cfg.dim>PG_DIM || cfg.cap[0]<1 {return s;}
    for t:0..<3 {if cfg.cap[t]<0 || cfg.cap[t]>PG_TIER {return s;}}
    s.status=PgStatus.long;
    if reqs.count>PG_REQUESTS {return s;}
    for i:0..<reqs.count {
        r:=reqs[i];
        if r.prompt>PG_PROMPT || r.output>PG_BLOCKS*cfg.page-r.prompt || model[i]<0 {return s;}
        for p:0..<r.prompt {s.tok[i*PG_PROMPT+p]=tok[i*PG_PROMPT+p];}
        s.model[i]=model[i];
    }
    c:=batch;c.kv=cfg.cap[0]*cfg.page;c.state=ifx cfg.hybrid then cfg.page else 0;
    s.cb=cb_start(c,reqs);s.status=PgStatus.batch;
    if s.cb.status!=CbStatus.ok {return s;}
    s.st.cfg=cfg;s.st.pool=pg_pool(cfg);
    for n:0..<PG_NODES {s.st.node[n].frame=-1;}
    for i:0..<PG_REQUESTS {s.st.live[i]=-1;}
    s.status=PgStatus.ok;
    return s;
}
```

Admission keeps chapter 175's order: the backlogged client with the
smallest virtual time offers its oldest request, and admission stops when
that request does not fit. What fits is now counted in pages:

```text
charge = blocks - reused + reused blocks not on the device + (1 if hybrid)
```

`blocks` is the request's whole table, prompt and output. A reused block on
the device costs nothing; one on host or disk costs a device frame to bring
it back. The request is admitted when the charge is at most the free device
frames plus the device frames held only by cached blocks it does not reuse,
because those can always be moved down or dropped. Every frame is taken at
admission, so a running request never runs out of memory part way. Its
virtual time is charged `charge * 840 / weight`, so a request that reuses a
prefix also costs its client less of its share.

<!-- source: psionic/lib/serve-pages.coax admit -->
```jai
// Chapter 175's admission, charging pages. The head request's charge is its
// blocks, less the cached blocks it reuses, plus one page per reused block
// that must come back up from host or disk, plus its recurrent-state page.
// It is admitted when the charge fits in the free device frames and the
// frames that cached blocks it does not need could give back.
pg_admit :: (from:PgState,mem:[]f64)->PgState {
    s:=from;p:=s.cfg.page;
    while s.cb.nrun<s.cb.cfg.slots {
        c:=-1;
        for d:0..<s.cb.cfg.clients {if s.cb.queued[d]>0 && (c<0 || s.cb.vt[d]<s.cb.vt[c]) {c=d;}}
        if c<0 {return s;}
        h:=0;
        while s.cb.req[h].client!=c || s.cb.fate[h].status!=CbStatus.ok || s.cb.fate[h].admit>=0 {h+=1;}
        r:=s.cb.req[h];
        s.path=pg_match(s.st,s.model[h],s.tok[h*PG_PROMPT..<h*PG_PROMPT+PG_PROMPT],r.prompt);u:=s.path.n;
        blocks:=(r.prompt+r.output+p-1)/p;
        need:=blocks-u+s.cb.cfg.state/p;
        for k:0..<u {n:=s.path.node[k];s.st.node[n].keep=true;if s.st.node[n].frame>=PG_TIER {need+=1;}}
        spare:=s.st.pool.nfree[0];
        for n:0..<PG_NODES {d:=s.st.node[n];if d.frame>=0 && d.frame<PG_TIER && !d.keep && s.st.pool.ref[d.frame]==1 {spare+=1;}}
        if need>spare {for k:0..<u {s.st.node[s.path.node[k]].keep=false;}return s;}
        for k:0..<u {
            n:=s.path.node[k];f:=s.st.node[n].frame;t:=pg_tier(f);
            if t>0 {s.st=pg_take(s.st,mem);pg_copy(s.cfg,mem,f,s.st.pool.got);s.st.pool=pg_release(s.st.pool,f);s.st.node[n].frame=s.st.pool.got;}
            s.st.pool=pg_retain(s.st.pool,s.st.node[n].frame);
            s.hit[t]+=1;s.st.node[n].keep=false;s.st.table[h].page[k]=s.st.node[n].frame;
        }
        for k:u..<blocks {s.st=pg_take(s.st,mem);s.st.table[h].page[k]=s.st.pool.got;}
        s.st.table[h].n=blocks;s.miss+=(r.prompt+p-1)/p-u;s.reused[h]=u;
        if s.cfg.hybrid {
            s.st=pg_take(s.st,mem);l:=s.st.pool.got;s.st.live[h]=l;w:=pg_width(s.cfg);
            for j:0..<s.cfg.dim {mem[l*w+j]=0;if u>0 {mem[l*w+j]=mem[s.st.table[h].page[u-1]*w+p*s.cfg.dim+j];}}
        }
        s.cb.filled[h]=u*p;
        s.cb.fate[h].admit=s.cb.step;s.cb.fate[h].order=s.cb.admitted;s.cb.admitted+=1;
        s.cb.now=s.cb.vt[c];s.cb.vt[c]+=need*CB_SCALE/s.cb.cfg.weight[c];s.cb.queued[c]-=1;
        s.cb.run[s.cb.nrun]=h;s.cb.nrun+=1;
    }
    return s;
}
```

## One step

A step has two halves around the model. `pg_step` takes arrivals, admits,
and plans rows with chapter 175's `cb_tokens`; a reused prefix starts the
request's prompt at `reused * page`, so only the rest is prefilled. The
model then runs the rows, writing each position's keys into the frame its
table names. `pg_finish` puts every whole prompt block written so far into
the trie and retires finished requests, releasing their frames.

<!-- source: psionic/lib/serve-pages.coax step -->
```jai
// One iteration, in two halves around the model: pg_step plans it
// (arrivals, admission, rows); the model runs the rows, reading and writing
// frames through the page tables; pg_finish puts every whole prompt block
// written so far in the trie, marks its last use, and retires the finished
// requests.
pg_step :: (from:PgState,mem:[]f64)->PgState {
    s:=from;s.cb=cb_arrive(s.cb);s=pg_admit(s,mem);s.cb=cb_tokens(s.cb);
    return s;
}
pg_finish :: (from:PgState)->PgState {
    s:=from;
    for q:0..<s.cb.nrun {
        i:=s.cb.run[q];
        for k:0..<s.cb.filled[i]/s.cfg.page {s.st=pg_commit(s.st,i,k,s.model[i],s.tok[i*PG_PROMPT..<i*PG_PROMPT+PG_PROMPT],s.cb.step);}
    }
    s.cb.clock+=cb_step_ns(s.cb.used);
    m:=0;
    for q:0..<s.cb.nrun {
        i:=s.cb.run[q];
        if s.cb.fate[i].first==s.cb.step {s.cb.fate[i].first_ns=s.cb.clock;}
        if s.cb.emitted[i]==s.cb.req[i].output {s.cb.fate[i].finish=s.cb.step;s.cb.fate[i].finish_ns=s.cb.clock;s.cb.done+=1;s.st=pg_free_table(s.st,i);}
        else {s.cb.run[m]=i;m+=1;}
    }
    s.cb.nrun=m;s.cb.step+=1;
    return s;
}
// Put block k of request i in the trie. If the same block is already there,
// the request's frame and the cached one hold the same bytes: a cached frame
// on the device replaces the request's copy; one on host or disk is replaced
// by the request's device frame. A block already linked finds itself, and
// only its last use moves.
pg_commit :: (from:PgStore,i:int,k:int,model:int,tok:[]int,step:int)->PgStore {
    s:=from;p:=s.cfg.page;parent:=-1-model;h:=model;
    if k>0 {parent=s.table[i].node[k-1];h=s.node[parent].hash;}
    b:[8]int;for j:0..<p {b[j]=tok[k*p+j];}
    h=pg_hash(h,b[0..<p]);f:=s.table[i].page[k];
    n:=pg_child(s.node[..],parent,h,b[0..<p]);
    if n<0 {
        n=0;while s.node[n].frame>=0 {n+=1;}
        s.node[n]=PgNode.{parent=parent,hash=h,tok=b,frame=f};s.pool=pg_retain(s.pool,f);
    }
    else if s.node[n].frame<PG_TIER {s.pool=pg_release(s.pool,f);s.table[i].page[k]=s.node[n].frame;s.pool=pg_retain(s.pool,s.node[n].frame);}
    else {s.pool=pg_release(s.pool,s.node[n].frame);s.node[n].frame=f;s.pool=pg_retain(s.pool,f);}
    s.node[n].stamp=step;s.table[i].node[k]=n;
    return s;
}
// Without a model: every step's rows are taken as run. Page contents stay
// as they are; admission, sharing and tiers do not depend on them.
pg_run :: (cfg:PgConfig,batch:CbConfig,reqs:[]CbRequest,model:[]int,tok:[]int,mem:[]f64)->PgState {
    s:=pg_start(cfg,batch,reqs,model,tok);
    if s.status!=PgStatus.ok {return s;}
    while s.cb.done<s.cb.n && s.cb.step<s.cb.cfg.steps {s=pg_finish(pg_step(s,mem));}
    if s.cb.done<s.cb.n {s.cb.status=CbStatus.steps;}
    return s;
}
```

Two requests can compute the same block at once. The second to commit finds
the node the first created; the bytes are the same, because the model is
deterministic and batch-invariant (chapter 175). If the cached frame is on
the device, the request drops its copy and shares the cached one; if it is
on host or disk, the request's device frame replaces it. Committing every
step also records the step as the block's last use while a request holds
it.

## A trace by hand

Five requests from one client, one at a time (`slots = 1`), with pages of
2 positions, `dim = 1`, and tiers of 4, 1 and 1 frames: frames 0 to 3 on
the device, frame 16 in host memory and frame 32 on disk. Each request has
one output token, so each runs and finishes in its own step.

| Request | Model | Prompt | Blocks (prompt + output) | Reusable |
| --- | ---: | --- | ---: | ---: |
| A | 0 | 1 2 3 4 5 | 3 | up to 2 |
| B | 1 | 1 2 3 4 5 | 3 | up to 2 |
| C | 0 | 1 2 3 4 6 7 | 4 | up to 2 |
| D | 1 | 1 2 3 4 5 | 3 | up to 2 |
| E | 0 | 1 2 3 4 6 7 | 4 | up to 2 |

The hashes come from `x = (x * 31 + token + 1) mod 2^31-1`. Model 0's
first block [1, 2] starts at 0: 0 * 31 + 2 = 2, then 2 * 31 + 3 = 65. Its
second block [3, 4] continues: 65 * 31 + 4 = 2,019, then 2,019 * 31 + 5 =
62,594. Model 1's [1, 2] starts at 1: 33, then 1,026.

**Step 0, A.** Nothing is cached. A's charge is 3 and 4 frames are free;
it takes frames 0, 1 and 2. Its whole prompt runs, and blocks [1, 2] and
[3, 4] become nodes 0 and 1 (hashes 65 and 62,594), sharing frames 0 and 1.
A finishes: frames 0 and 1 keep the cache's reference, frame 2 is free.
Computed blocks: 3 (all of A's prompt).

**Step 1, B.** Model 1 has no root in the trie, so B reuses nothing,
although its tokens are A's. Charge 3; free frames 2 and 3, and node 1 can
move (node 0 cannot: its child is on the device). B takes frames 2 and 3;
for the third, node 1 moves to host frame 16 and B takes frame 1. B's
blocks become nodes 2 and 3 (hashes 1,026 and 986,115) on frames 2 and 3.
Computed: 6.

**Step 2, C.** Model 0's chain matches [1, 2] (node 0, device) and [3, 4]
(node 1, host). C's prompt has 6 positions; block 2 holds the last one, so
2 blocks are reusable and both match. Charge: 4 blocks - 2 reused + 1 to
bring node 1 up = 3. Free: frame 1; movable: nodes 2 and 3; 1 + 2 >= 3.
Node 1 comes up to frame 1 and host frame 16 is free again. For C's block
2, node 3 (the only movable one: node 2's child is on the device) goes to
host frame 16, and C takes frame 3. For block 3, node 2 is now movable but
host is full, so host makes room first: node 3 goes to disk frame 32, then
node 2 to host frame 16, and C takes frame 2. C's table is 0, 1, 3, 2; it
prefills positions 4 and 5 only, and its block 2 [6, 7] becomes node 4 on
frame 3. Computed: 7.

**Step 3, D.** B's prompt again: node 2 on host and node 3 on disk.
Charge 3 - 2 + 2 = 3; free frame 2; movable nodes 0, 1 and 4: enough.
Node 2 comes up to frame 2. Bringing node 3 up needs a device frame: node 4
(C's last block, the only movable one) goes to host frame 16, and node 3
comes up from disk to frame 3. D's own block needs one more: node 1 is now
movable, host is full, so node 4 moves on to disk and node 1 takes host
frame 16; D takes frame 1. Computed: 8.

**Step 4, E.** C's prompt again: node 0 on the device, node 1 on host.
Charge 4 - 2 + 1 = 3; free frame 1; movable nodes 2 and 3. Node 1 comes up
to frame 1. Node 3 goes to host frame 16 and E takes frame 3. For the last
frame, node 2 must move, host must make room by moving node 3 to disk, and
disk is full with node 4, which has nowhere to go: it is dropped. Node 3
goes to disk, node 2 to host, and E takes frame 2. E recomputes block 2,
and it becomes a new node 4. Computed: 9 of 15 prompt blocks; reused 6:
2 found on the device, 3 in host memory and 1 on disk.

The check runs the same five steps and asserts each claim above:

<!-- source: psionic/lib/serve-pages.coax check-hand -->
```jai
#test "the hand trace: sharing, model identity, spills down two tiers, a drop" {
    mem:[1728]f64;s:=pgx_hand_start();#assert s.status==PgStatus.ok;
    // Step 0, A (model 0): 3 pages, frames 0 to 2; blocks [1,2] and [3,4] enter the trie.
    s=pg_finish(pg_step(s,mem[..]));n:=s.st.node;f:=s.st.pool;
    #assert pgx_pages(s,0,0,1,2,-1) && s.miss==3 && n[0].hash==65 && n[1].hash==62594 && n[0].parent== -1 && n[1].parent==0;
    #assert f.ref[0]==1 && f.ref[1]==1 && f.ref[2]==0 && f.nfree[0]==2;
    // Step 1, B (model 1, the same tokens): no hit. A's block 1 goes to host for B's third page.
    s=pg_finish(pg_step(s,mem[..]));n=s.st.node;
    #assert pgx_pages(s,1,2,3,1,-1) && s.hit[0]==0 && s.miss==6 && s.st.down[0]==1 && n[1].frame==16;
    #assert n[2].parent== -2 && n[2].hash==1026 && n[3].parent==2 && n[3].hash==986115;
    // Step 2, C (model 0): A's blocks hit, one on the device and one brought up from host.
    // Making room sends B's block 1 to host, then B's block 0 to host and block 1 on to disk.
    s=pg_finish(pg_step(s,mem[..]));n=s.st.node;
    #assert pgx_pages(s,2,0,1,3,2) && s.hit[0]==1 && s.hit[1]==1 && s.miss==7 && s.st.down[0]==3 && s.st.down[1]==1;
    #assert n[2].frame==16 && n[3].frame==32 && n[4].frame==3 && n[4].parent==1 && n[4].hash==60153059;
    // Step 3, D (B's prompt again): one block from host, one from disk; C's block 2 goes to host, then on to disk.
    s=pg_finish(pg_step(s,mem[..]));n=s.st.node;f=s.st.pool;
    #assert pgx_pages(s,3,2,3,1,-1) && s.hit[1]==2 && s.hit[2]==1 && s.miss==8 && s.st.down[0]==5 && s.st.down[1]==2;
    #assert n[1].frame==16 && n[4].frame==32 && f.nfree[0]==1 && f.nfree[1]==0 && f.nfree[2]==0;
    // Step 4, E (C's prompt again): the chain of moves ends with C's block 2 dropped from disk.
    s=pg_finish(pg_step(s,mem[..]));n=s.st.node;f=s.st.pool;
    #assert pgx_pages(s,4,0,1,3,2) && s.hit[0]==2 && s.hit[1]==3 && s.miss==9 && s.st.down[0]==7 && s.st.down[1]==3 && s.st.drop==1;
    #assert n[2].frame==16 && n[3].frame==32 && n[4].frame==3 && f.ref[3]==1 && f.ref[2]==0 && s.cb.done==5;
    // Last use, by step: A's blocks at E's step 4, B's at D's step 3.
    #assert n[0].stamp==4 && n[1].stamp==4 && n[2].stamp==3 && n[3].stamp==3 && n[4].stamp==4;
    #assert s.reused[0]==0 && s.reused[1]==0 && s.reused[2]==2 && s.reused[3]==2 && s.reused[4]==2;
}
```

## Run it

```shell
coax/bin/coax run psionic/examples/serve-pages.coax
```

The example, [`serve-pages.coax`](../examples/serve-pages.coax), prints
the hand trace, then serves a shared-prefix workload
with four tier sizes and checks every request's tokens against a cold run
of the stand-in model.

<!-- source: psionic/examples/serve-pages.coax main -->
```jai
main :: () {
    mem:[1728]f64;s:=pgx_hand_start();
    for i:0..<5 {
        s=pg_step(s,mem[..]);r:=s.cb.run[0];t:=s.st.table[r];s=pg_finish(s);show(s,r,t);
        if i==1 || i==4 {trie(s);}
    }
    // 16 requests, one every 8 steps, served one at a time. Each starts with
    // one of 4 shared prefixes of 16 tokens, taken in turn, then has 8 tokens
    // of its own and 4 output tokens: 7 pages of 4 positions on a device of 12.
    q:[16]CbRequest;model:[16]int;tok:[512]int;
    for i:0..<16 {
        q[i]=CbRequest.{arrive=i*8,prompt=24,output=4};
        for p:0..<16 {tok[i*32+p]=i%4*100+p;}
        for p:16..<24 {tok[i*32+p]=i*100+50+p;}
    }
    b:=CbConfig.{slots=1,tokens=16,clients=1,steps=CB_STEPS};b.weight[0]=1;
    print("16 requests, 4 shared prefixes of 4 blocks in turn, 6 prompt blocks each (%):\n",PG_PROFILE);
    caps:=[12]int.[12,0,0,12,8,0,12,8,16,12,16,16];
    for c:0..<4 {
        cfg:=PgConfig.{page=4,dim=4};for t:0..<3 {cfg.cap[t]=caps[c*3+t];}
        r:=pg_start(cfg,b,q[..],model[..],tok[..]);out:[128]int;
        while r.cb.done<r.cb.n {r=pg_step(r,mem[..]);pgx_rows(r,mem[..],out[..]);r=pg_finish(r);}
        same:=0;
        for i:0..<16 {a:[8]int;pgx_alone(r,i,a[..]);ok:=true;for k:0..<8 {if out[i*8+k]!=a[k] {ok=false;}}if ok {same+=1;}}
        hits:=r.hit[0]+r.hit[1]+r.hit[2];
        hitfirst:[16]int;cold:[16]int;nh:=0;nc:=0;
        for i:0..<16 {
            f:=r.cb.fate[i];
            if r.reused[i]>0 {hitfirst[nh]=f.first_ns-f.start_ns;nh+=1;} else {cold[nc]=f.first_ns-f.start_ns;nc+=1;}
        }
        print("device %, host %, disk %: reused % of 96 prompt blocks (hit rate %.%; device %, host %, disk %), computed %\n",
            cfg.cap[0],cfg.cap[1],cfg.cap[2],hits,hits*100/96,hits*1000/96%10,r.hit[0],r.hit[1],r.hit[2],r.miss);
        print("  moved down % (device) and % (host), dropped %; % prompt positions run of 384\n",r.st.down[0],r.st.down[1],r.st.drop,384-4*hits);
        print("  first token P50: ");ms(cb_rank(hitfirst[0..<nh],50));print(" for % requests that reused blocks, ",nh);
        ms(cb_rank(cold[0..<nc],50));print(" for %; tokens equal to a cold run: % of 16\n",nc,same);
    }
}
```

```text
step 0: A (model 0) on frames 0 1 2; reused so far: device 0, host 0, disk 0; computed 3; moved down 0 and 0; dropped 0
step 1: B (model 1) on frames 2 3 1; reused so far: device 0, host 0, disk 0; computed 6; moved down 1 and 0; dropped 0
  node 0: model 0, first block, tokens [1,2], hash 65, frame 0 (device), last used at step 0
  node 1: model 0, under node 0, tokens [3,4], hash 62594, frame 16 (host), last used at step 0
  node 2: model 1, first block, tokens [1,2], hash 1026, frame 2 (device), last used at step 1
  node 3: model 1, under node 2, tokens [3,4], hash 986115, frame 3 (device), last used at step 1
step 2: C (model 0) on frames 0 1 3 2; reused so far: device 1, host 1, disk 0; computed 7; moved down 3 and 1; dropped 0
step 3: D (model 1) on frames 2 3 1; reused so far: device 1, host 2, disk 1; computed 8; moved down 5 and 2; dropped 0
step 4: E (model 0) on frames 0 1 3 2; reused so far: device 2, host 3, disk 1; computed 9; moved down 7 and 3; dropped 1
  node 0: model 0, first block, tokens [1,2], hash 65, frame 0 (device), last used at step 4
  node 1: model 0, under node 0, tokens [3,4], hash 62594, frame 1 (device), last used at step 4
  node 2: model 1, first block, tokens [1,2], hash 1026, frame 16 (host), last used at step 3
  node 3: model 1, under node 2, tokens [3,4], hash 986115, frame 32 (disk), last used at step 3
  node 4: model 0, under node 1, tokens [6,7], hash 60153059, frame 3 (device), last used at step 4
16 requests, 4 shared prefixes of 4 blocks in turn, 6 prompt blocks each (pg-pages-v1):
device 12, host 0, disk 0: reused 0 of 96 prompt blocks (hit rate 0.0; device 0, host 0, disk 0), computed 96
  moved down 0 (device) and 0 (host), dropped 85; 384 prompt positions run of 384
  first token P50: 0.0 ms for 0 requests that reused blocks, 67.6 ms for 16; tokens equal to a cold run: 16 of 16
device 12, host 8, disk 0: reused 12 of 96 prompt blocks (hit rate 12.5; device 0, host 12, disk 0), computed 84
  moved down 85 (device) and 0 (host), dropped 65; 336 prompt positions run of 384
  first token P50: 61.8 ms for 12 requests that reused blocks, 67.6 ms for 4; tokens equal to a cold run: 16 of 16
device 12, host 8, disk 16: reused 48 of 96 prompt blocks (hit rate 50.0; device 0, host 12, disk 36), computed 48
  moved down 85 (device) and 65 (host), dropped 13; 192 prompt positions run of 384
  first token P50: 28.0 ms for 12 requests that reused blocks, 67.6 ms for 4; tokens equal to a cold run: 16 of 16
device 12, host 16, disk 16: reused 48 of 96 prompt blocks (hit rate 50.0; device 0, host 48, disk 0), computed 48
  moved down 85 (device) and 21 (host), dropped 5; 192 prompt positions run of 384
  first token P50: 28.0 ms for 12 requests that reused blocks, 67.6 ms for 4; tokens equal to a cold run: 16 of 16
```

## A measured hit rate

The workload is 16 requests, one every 8 steps, served one at a time. Each
starts with one of four 16-token prefixes, taken in turn, then has 8
tokens of its own and 4 output tokens: 6 prompt blocks and 7 pages of 4
positions each. The device has 12 frames. A running request holds 7, so at
most 5 cached blocks stay on the device, and when a prefix comes back, the
3 requests since its last use have brought 18 newer blocks.

| Device, host, disk frames | Blocks reused of 96 | Found in host / disk | Moved down, device / host | Dropped | Prompt positions run | First token P50, reused / not |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 12, 0, 0 | 0 (0.0%) | 0 / 0 | 0 / 0 | 85 | 384 | none / 67.6 ms |
| 12, 8, 0 | 12 (12.5%) | 12 / 0 | 85 / 0 | 65 | 336 | 61.8 ms / 67.6 ms |
| 12, 8, 16 | 48 (50.0%) | 12 / 36 | 85 / 65 | 13 | 192 | 28.0 ms / 67.6 ms |
| 12, 16, 16 | 48 (50.0%) | 48 / 0 | 85 / 21 | 5 | 192 | 28.0 ms / 67.6 ms |

With the device alone, least-recently-used eviction under this round-robin
order removes each prefix before it returns, and nothing is reused. Host
memory of 8 frames keeps part of each prefix. With enough host and disk
together to hold the four prefixes and the blocks between their uses, every
request after the first four reuses all 4 of its shared blocks, which is
48 of 96, the most this workload allows: the first use of each prefix and
every request's own blocks are always computed. With 16 host frames the
same hits come from host memory instead of disk. In every configuration
all 16 requests produce the tokens of a cold run.

The first-token times come from chapter 175's timing model: a reused
prefix leaves 8 positions to prefill, one step of 8 rows, instead of 24
positions in two steps. They leave out what a real tier costs, the copy
from host memory or disk back to the device, because these tiers are
regions of one array. They show how much prefill a hit saves, not what a
hit costs on a machine.

## Refusals

A page configuration or request outside the bounds refuses the whole run.
A request whose pages can never fit the device is refused at arrival by
chapter 175's rule, and the others still run. A prompt under another model
identity, or a block whose hash collides with a cached one, is a miss.
[`serve-pages-refuse.coax`](../examples/serve-pages-refuse.coax) shows
each:

<!-- source: psionic/examples/serve-pages-refuse.coax refuse -->
```jai
main :: () {
    cfg:=PgConfig.{page=2,dim=1};cfg.cap[0]=4;cfg.cap[1]=1;cfg.cap[2]=1;
    b:=CbConfig.{slots=1,tokens=8,clients=1,steps=100};b.weight[0]=1;
    q:=[3]CbRequest.[CbRequest.{prompt=5,output=1},CbRequest.{prompt=5,output=1},CbRequest.{prompt=7,output=2}];
    m:=[3]int.[0,1,0];tok:[96]int;
    for i:0..<3 {for p:0..<7 {tok[i*32+p]=p+1;}}
    whole("the configuration of the hand trace",cfg,b,q[..],m[..],tok[..]);
    x:=cfg;x.page=9;whole("pages of 9 positions",x,b,q[..],m[..],tok[..]);
    x=cfg;x.cap[1]=17;whole("17 host frames",x,b,q[..],m[..],tok[..]);
    x=cfg;x.cap[0]=0;whole("no device frames",x,b,q[..],m[..],tok[..]);
    y:=q;y[2].output=26;whole("7 + 26 positions in 16 pages of 2",cfg,b,y[..],m[..],tok[..]);
    n:=m;n[1]=-1;whole("model identity -1",cfg,b,q[..],n[..],tok[..]);
    x=cfg;x.hybrid=true;x.cap[0]=1;whole("a hybrid model on 1 device frame",x,b,q[..],m[..],tok[..]);
    // At arrival: 7 + 2 positions need 5 pages, and the device has 4.
    mem:[1728]f64;s:=pg_run(cfg,b,q[..],m[..],tok[..],mem[..]);
    for i:0..<3 {
        f:=s.cb.fate[i];
        print("request % (model %, prompt %, output %): ",i,m[i],q[i].prompt,q[i].output);
        if f.status==CbStatus.ok {print("finished at step %, reused % blocks\n",f.finish,s.reused[i]);}
        else {print("refused (%)\n",cb_status_name(f.status));}
    }
    // Model 2 asks for a prompt cached under models 0 and 1; [1,0] asks where [0,31] is cached, with the same hash.
    t:=tok;
    print("model 0, its own prompt: reuses % blocks\n",pg_match(s.st,0,t[0..<32],5).n);
    print("model 2, the prompt models 0 and 1 cached: reuses % blocks\n",pg_match(s.st,2,t[0..<32],5).n);
    t[0]=0;t[1]=31;c:=[2]int.[1,0];
    s=pg_run(cfg,b,q[0..<1],m[..],t[..],mem[..]);
    print("[1,0] after [0,31] (hashes % and %): reuses % blocks\n",pg_hash(0,t[0..<2]),pg_hash(0,c[..]),pg_match(s.st,0,c[..],3).n);
}
```

```text
the configuration of the hand trace: ok
pages of 9 positions: page configuration out of bounds
17 host frames: page configuration out of bounds
no device frames: page configuration out of bounds
7 + 26 positions in 16 pages of 2: a request outside the page table, prompt or model bounds
model identity -1: a request outside the page table, prompt or model bounds
a hybrid model on 1 device frame: refused by the scheduler (configuration out of bounds)
request 0 (model 0, prompt 5, output 1): finished at step 0, reused 0 blocks
request 1 (model 1, prompt 5, output 1): finished at step 1, reused 0 blocks
request 2 (model 0, prompt 7, output 2): refused (state never fits the budget)
model 0, its own prompt: reuses 2 blocks
model 2, the prompt models 0 and 1 cached: reuses 0 blocks
[1,0] after [0,31] (hashes 63 and 63): reuses 0 blocks
```

## Independent checks

The checks in `serve-pages.coax` compare the library with models written
another way, in
[`serve-pages-checks.coax`](../tests/serve-pages-checks.coax):

- **Shared tables against private copies.** 100 random sequences of 30
  operations (new tables, forks, writes, frees) on 4 tables, each checked
  after every operation against a model in which every table is its own
  list of values: the values read through the frames, every reference
  count, the free stack, and the number of copies on write.
- **Paged attention equals contiguous attention.** After every operation,
  attention over each table, gathered page by page, equals attention over
  the table's private copy, bit for bit.
- **Cached prefixes give the cold run's tokens.** 60 random workloads with
  shared prefixes, models 0 and 1, hybrid and pure attention, and small
  host and disk tiers, run through the stand-in model reading and writing
  frames. Every request's tokens equal the same request run alone on a
  contiguous cache.
- **Invariants after every step.** In the same runs: each frame's count
  equals its holders, each free stack holds exactly the unreferenced frames
  of its tier, host and disk frames belong to the cache alone, a node's
  parent is never in a slower tier, no two nodes share a parent and tokens,
  every cached frame holds the bytes its tokens give (the round trip through
  the tiers), and every request's lookup equals a brute-force one that
  rebuilds each cached prefix and its model by walking to the root.
- **Nothing shared is chapter 175.** With pages of 1 position and prompts
  that share nothing, 50 of chapter 175's random workloads give the same
  admissions, first tokens, finishes, order, clock readings and virtual
  times as chapter 175's scheduler.
- **Hand-worked cases.** The trace above; hash values and collisions,
  including a token that differs by the hash's modulus; refusals at each
  bound; the least recently used node going first, and the lowest on a tie;
  and a hybrid hit restoring the recurrent snapshot.

`coax mutate psionic/lib/serve-pages.coax --root-only` kills 516 of 516
mutants.

## What remains

- **Real model state.** The pages hold a stand-in model's keys and
  recurrent state. Paging gpt-oss-20b's KV cache, or Elephant's and
  Mammoth's KV and recurrent state, on the RTX 4080, with chapter 41's tiled
  attention reading pages as its tiles and chapter 50's int8 KV inside the
  pages, is not built. Neither is the output check against a cold run of a
  real model.
- **Real tiers.** Device, host and disk are regions of one array in one
  process. Device frames in GPU memory, host frames in pinned memory, disk
  frames in files, their measured capacities, and the time each move takes
  are not built or measured, so the first-token times above leave out
  transfers.
- **Page size.** Pages hold at most 8 positions and tables 16 pages; the
  128-token pages of DeepSeek's design need larger bounds.
- **Layer-by-layer restore.** A restored block comes back whole before the
  request runs; restoring one layer at a time while compute proceeds is not
  built.
- **Storage independent of compute.** The cache lives in one process and
  ends with it.
- **Retention per traffic type.** One least-recently-used rule serves all
  traffic.
- **Chapter 172's report.** Hit rate, eviction counts and time to first
  token on hits and misses are printed by the example from a modeled clock,
  not reported by chapter 172's benchmark from a recorded workload.
- **Approximate reconstruction.** No approximate profile exists; every page
  identity is exact.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- Kwon et al., "Efficient memory management for large language model
  serving with PagedAttention", SOSP 2023.
- Zheng et al., "SGLang: efficient execution of structured language model
  programs", 2024 (RadixAttention: a prefix trie with least-recently-used
  eviction of leaves).
