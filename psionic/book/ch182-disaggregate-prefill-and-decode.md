# 182. Disaggregate prefill and decode

> **Status: partly implemented.** The state handoff, the placement rule
> and the simulation (profile dg-split-v1) run on the CPU in one process.
> A request's pages move from a prefill worker's memory to a decode
> worker's memory as a checked snapshot, and the decode worker produces
> the same tokens as one worker would. Workers, links and transfer times
> are simulated with chapter 175's step model and a modeled link. No
> transfer between real devices or machines runs; the last section lists
> what is not built.

A prompt is processed in one step of many rows (prefill). After that,
each output token is a step of one row per running request (decode). The
two share a worker badly: when a long prompt joins a step, every decoding
request on that worker waits for the whole step. Chapter 175's scheduler
already gives decoding requests their row first, but the step still takes
as long as all its rows.

Disaggregation gives the two phases separate workers. Prefill workers run
prompts only. When a prompt finishes, its state (the KV pages, and the
recurrent state of a hybrid model) moves to a decode worker, which produces
the remaining tokens in short steps. The cost is a transfer per request
and fewer workers for each phase.

This chapter builds the pieces in an `#authority` file,
[`serve-split.coax`](../lib/serve-split.coax):

- a snapshot of a request's pages, with a header and a checksum, and a
  receiver that accepts it only whole and unchanged;
- a placement rule that splits a fixed number of workers between the two
  pools;
- a discrete-event simulation that runs the same traffic on co-located
  workers and on every split, and reports time to first token (TTFT), the
  gaps between tokens and throughput.

It builds on chapter 175 (the step model: 17,792,472 ns for one row,
1,459,182 ns per further row), chapter 176 (pages, frames and the
stand-in paged model), and chapter 172 (synthetic traffic and
nearest-rank percentiles).

## The snapshot

A snapshot is an array of 64-bit words. Nine header words come first:
version, model, request, positions, page size, values per position,
whether the model is hybrid, frame count and checksum. Then each frame
follows as it sits in memory, `(page + 1) * dim` values as their bit
patterns: first the frames of the prompt's pages, then the recurrent-state
frame of a hybrid model. The size is exact: `8 * (9 + frames * width)`
bytes, nothing more.

<!-- source: psionic/lib/serve-split.coax types -->
```jai
// Why the decode side refused a snapshot: fewer words than it declares (or
// than a header), another version, another model, a page geometry this
// worker does not use, another request or prompt length, a frame count that
// does not match the positions, more words than declared, a checksum that
// does not match, a slot already holding a request, or too few free frames.
DgRefusal :: enum u8 {none;partial;version;model;geometry;request;frames;excess;checksum;slot;room;}
// What the decode side was told to expect when the request was placed.
DgExpect :: struct {model:int;request:int;positions:int;}
DgReceived :: struct {refusal:DgRefusal;st:PgStore;frames:int;bytes:int;}
```

The checksum is chapter 176's chained hash, run over the high and low
halves of every word except word 8, which holds it.

<!-- source: psionic/lib/serve-split.coax wire -->
```jai
dg_frames :: (c:PgConfig,positions:int)->int {return (positions+c.page-1)/c.page+ifx c.hybrid then 1 else 0;}
// The checksum: chapter 176's chained hash over both halves of every word
// but word 8, which holds it.
dg_sum :: (w:[]u64)->int {
    h:=0;
    for k:0..<w.count {
        if k==8 {continue;}
        h=(h*31+cast(int)(w[k]>>32)+1)%PG_HASH;
        h=(h*31+cast(int)(w[k]&cast(u64)4294967295)+1)%PG_HASH;
    }
    return h;
}
// Pack the first `positions` of request i's table, and its recurrent state
// when the model is hybrid, frame by frame as the frames hold them. Returns
// the words written, or -1 when there are no positions, the table does not
// cover them, or out is too small.
dg_pack :: (st:PgStore,mem:[]f64,i:int,model:int,request:int,positions:int,out:[]u64)->int {
    c:=st.cfg;w:=pg_width(c);pages:=(positions+c.page-1)/c.page;frames:=dg_frames(c,positions);n:=DG_HEAD+frames*w;
    if positions<1 || pages>st.table[i].n || n>out.count {return -1;}
    head:=[8]int.[DG_VERSION,model,request,positions,c.page,c.dim,ifx c.hybrid then 1 else 0,frames];
    for k:0..<8 {out[k]=cast(u64)head[k];}
    for k:0..<frames {
        f:=ifx k<pages then st.table[i].page[k] else st.live[i];
        for j:0..<w {out[DG_HEAD+k*w+j]=f64_bits(mem[f*w+j]);}
    }
    out[8]=cast(u64)dg_sum(out[0..<n]);
    return n;
}
```

### A snapshot by hand

Take pages of 2 positions and 1 value per position. A frame is
`(2 + 1) * 1 = 3` words wide. Three positions with values 1.0, 2.0 and 3.0
fill one page and a half, so the snapshot has 2 frames: `9 + 2 * 3 = 15`
words, 120 bytes. Words 9 to 11 are the first frame (1.0, 2.0 and an unused
slot of 0); words 12 to 14 the second (3.0 and two zeros).

The checksum of the two words 0 and 1, high half then low half of each:

- 0 * 31 + 0 + 1 = 1
- 1 * 31 + 0 + 1 = 32
- 32 * 31 + 0 + 1 = 993
- 993 * 31 + 1 + 1 = 30,785

A one-bit change in any word changes one half by 2^b with b at most 31.
Modulo the prime 2^31 - 1 that is never 0, and multiplying by 31 keeps it
nonzero, so every single-bit change moves the checksum.

## Receiving it

The receiver checks the snapshot against what it was told to expect when
the request was placed (model, request and prompt length) and against its
own page geometry, in a fixed order: a whole header, then the version,
the model, the geometry, the request, the frame count, the exact size and
the checksum. Then it checks its own state: the slot must be empty and
enough device frames free. Only then does it take one frame per snapshot
frame and write the values back bit for bit. A refused snapshot leaves the
receiver as it was: nothing allocated, nothing written.

<!-- source: psionic/lib/serve-split.coax receive -->
```jai
// The count rule, shared with the simulation: a message is whole when
// exactly the declared size arrived.
dg_count :: (want:int,got:int)->DgRefusal {
    if got<want {return DgRefusal.partial;}
    if got>want {return DgRefusal.excess;}
    return DgRefusal.none;
}
// Check a snapshot against what this worker expects, in order: a whole
// header; the version, the model, the geometry; the request and a prompt of
// at least one position; the frame count; exactly the size those frames
// take; the checksum.
dg_check :: (c:PgConfig,want:DgExpect,wire:[]u64)->DgRefusal {
    if wire.count<DG_HEAD {return DgRefusal.partial;}
    h:[9]int;for k:0..<DG_HEAD {h[k]=cast(int)wire[k];}
    if h[0]!=DG_VERSION {return DgRefusal.version;}
    if h[1]!=want.model {return DgRefusal.model;}
    if h[4]!=c.page || h[5]!=c.dim || h[6]!=(ifx c.hybrid then 1 else 0) {return DgRefusal.geometry;}
    if h[2]!=want.request || h[3]!=want.positions || want.positions<1 {return DgRefusal.request;}
    frames:=dg_frames(c,want.positions);
    if h[7]!=frames {return DgRefusal.frames;}
    r:=dg_count(DG_HEAD+frames*pg_width(c),wire.count);
    if r!=DgRefusal.none {return r;}
    if h[8]!=dg_sum(wire) {return DgRefusal.checksum;}
    return DgRefusal.none;
}
// Receive into slot i: refuse, or take one device frame per page (and one
// for the recurrent state) and write the values back bit for bit. Nothing
// is held or written on a refusal.
dg_receive :: (dst:PgStore,mem:[]f64,i:int,want:DgExpect,wire:[]u64)->DgReceived {
    r:=DgReceived.{refusal=dg_check(dst.cfg,want,wire),st=dst};
    if r.refusal!=DgRefusal.none {return r;}
    s:=dst;c:=s.cfg;w:=pg_width(c);frames:=dg_frames(c,want.positions);pages:=(want.positions+c.page-1)/c.page;
    if s.table[i].n>0 || s.live[i]>=0 {r.refusal=DgRefusal.slot;return r;}
    if s.pool.nfree[0]<frames {r.refusal=DgRefusal.room;return r;}
    for k:0..<frames {
        s.pool=pg_alloc(s.pool,0);f:=s.pool.got;
        for j:0..<w {mem[f*w+j]=f64_from_bits(wire[DG_HEAD+k*w+j]);}
        if k<pages {s.table[i].page[k]=f;} else {s.live[i]=f;}
    }
    s.table[i].n=pages;
    return DgReceived.{st=s,frames=frames,bytes=8*wire.count};
}
```

The decode worker therefore never sees part of a prefix: either every
frame arrived and matched, or the request goes back to be prefilled.

## Same tokens after a handoff

The first check runs chapter 176's stand-in model three ways on the same
prompt: on one worker with paged memory, handed off between two workers,
and on chapter 176's contiguous reference with no pages at all. The
handoff prefills in one memory, packs the snapshot, frees the sender's
frames and overwrites that memory, receives into slot 2 of a second store
whose first three frames are already taken (so every frame number
differs), and decodes there. Prompts of 1, 5, 8 and 13 positions, with and
without the recurrent state, give the same six tokens all three ways.

<!-- source: psionic/lib/serve-split.coax check-same -->
```jai
#test "a handed-off request decodes token for token as it does on one worker" {
    for hybrid:0..<2 {
        for prompt:[4]int.[1,5,8,13] {
            c:=PgConfig.{page=4,dim=3,hybrid=hybrid==1};c.cap[0]=12;
            one:=dgx_colocated(c,prompt,6);two:=dgx_handed(c,prompt,6);ref:=dgx_alone(c,prompt,6);
            for t:0..<6 {#assert one.tok[t]==ref[t] && two.tok[t]==ref[t];}
            #assert two.refusal==DgRefusal.none && two.frames==(prompt+3)/4+hybrid && two.bytes==8*(9+two.frames*15);
        }
    }
}
```

## What a transfer costs

The simulation does not move real gpt-oss-20b state, but it charges the
bytes that state would take. gpt-oss-20b has 24 layers. Half of them
attend to every position, half to a window of 128. Each layer stores a key
and a value for 8 heads of 64. This chapter assumes 2 bytes per value
(bf16) and pages of 16 positions; both are parameters.

One page of one layer holds K and V:
`2 * 8 * 64 * 2 * 16 = 32,768` bytes.

For a 1,024-position prompt:

- full layers: 1,024 / 16 = 64 pages each, 12 * 64 = 768 pages;
- windowed layers: min(1,024, 128) / 16 = 8 pages each, 12 * 8 = 96 pages;
- 864 pages * 32,768 = 28,311,552 bytes, plus the 72-byte header:
  28,311,624 bytes.

On a 10 Gbit/s link (10 bits a nanosecond) with 50 µs of latency:
28,311,624 * 8 = 226,492,992 bits, / 10 = 22,649,299.2 ns, rounded up to
22,649,300, plus 50,000: **22,699,300 ns**, about 22.7 ms. That is about
one and a quarter decode steps of one row (17.8 ms), and 1.5% of the
1,510.5 ms the same prompt takes to prefill.

For 64 positions: 4 pages in every layer, 96 pages, 3,145,800 bytes,
2,566,640 ns.

<!-- source: psionic/lib/serve-split.coax cost -->
```jai
// gpt-oss-20b (psionic/docs/targets/gpt-oss.md): 24 layers alternating full
// and 128-position windowed attention, 8 KV heads of 64. Two bytes a value
// and pages of 16 positions are this chapter's assumptions.
dg_gptoss :: ()->DgShape {return DgShape.{full=12,windowed=12,window=128,kv_heads=8,head_dim=64,value_bytes=2,page=16};}
// The bytes a snapshot of `positions` moves: the header, then whole pages
// of K and V in every layer, a windowed layer holding at most its window.
dg_bytes :: (s:DgShape,positions:int)->int {
    page:=2*s.kv_heads*s.head_dim*s.value_bytes*s.page;
    full:=(positions+s.page-1)/s.page;win:=(cb_min(positions,s.window)+s.page-1)/s.page;
    return 8*DG_HEAD+page*(s.full*full+s.windowed*win);
}
// Nanoseconds on the link: the latency, then the bits at gbit bits a
// nanosecond, rounded up.
dg_transfer_ns :: (l:DgLink,bytes:int)->int {return l.lat_ns+(bytes*8+l.gbit-1)/l.gbit;}
```

<!-- source: psionic/lib/serve-split.coax check-bytes -->
```jai
#test "transfer bytes and time for gpt-oss-20b, by hand" {
    g:=dg_gptoss();l:=DgLink.{gbit=10,lat_ns=50000};
    // A page of one layer: K and V, 8 heads of 64, 2 bytes, 16 positions = 32,768 bytes.
    // 1,024 positions: 12 layers of 64 pages and 12 of 8 pages (the 128-position window).
    #assert dg_bytes(g,1024)==72+32768*(12*64+12*8) && dg_bytes(g,1024)==28311624;
    // 226,492,992 bits at 10 Gbit/s: 22,649,299.2 ns, rounded up, and 50 us.
    #assert dg_transfer_ns(l,28311624)==22699300;
    // 100 positions: 7 pages in every layer. 17 positions: 2 pages. 16: 1.
    #assert dg_bytes(g,100)==72+32768*24*7 && dg_bytes(g,17)==72+32768*48 && dg_bytes(g,16)==72+32768*24;
    #assert dg_bytes(g,128)==72+32768*24*8 && dg_bytes(g,129)==72+32768*(12*9+12*8);
    #assert dg_transfer_ns(DgLink.{gbit=3},1)==3 && dg_transfer_ns(DgLink.{gbit=8},1)==1 && dg_transfer_ns(DgLink.{gbit=8,lat_ns=5},2)==7;
}
```

## Placing the workers

Given a window of traffic and a number of workers, the placement rule
predicts each pool's busy time and splits the workers so the busier pool,
per worker, is as idle as possible.

- **Prefill work `a`:** each prompt as one step of its rows,
  `cb_step_ns(prompt)`.
- **Decode work `b`:** each token after the first as a `slots`-th share of
  a full decode step, `(output - 1) * cb_step_ns(slots) / slots`.

With `p` prefill workers, the larger share per worker is
`max(a / p, b / (workers - p))`. The first term falls as `p` grows and the
second rises, so the best whole `p` is the floor or the ceiling of
`workers * a / (a + b)`. At the floor `lo` the prefill share `a / lo` is
the larger one; at the ceiling `hi = lo + 1` the decode share
`b / (workers - hi)` is. So `hi` wins when `b * lo < a * (workers - hi)`,
compared without dividing. A floor of 0 always loses and a ceiling of
`workers` never wins, so both pools keep a worker. A tie goes to fewer
prefill workers. With no decode work at all (every output one token),
every worker but one runs prompts.

By hand, two requests of 512 positions and 33 tokens on 4 workers with 16
slots:

- `a = 2 * cb_step_ns(512) = 2 * 763,434,474 = 1,526,868,948` ns;
- `cb_step_ns(16) = 17,792,472 + 15 * 1,459,182 = 39,680,202`, so
  `b = 64 * 39,680,202 / 16 = 158,720,808` ns;
- `4 * a / (a + b) = 3.6`: the candidates are 3 and 4, and 4 leaves no
  decode worker. Three prefill workers carry 509 ms each and one decode
  worker 159 ms.

With 1,000-token outputs, `b` becomes `1,998 * 39,680,202 / 16 =
4,955,065,224` ns, `4 * a / (a + b) = 0.94`, and the candidates are 1 and
2: one prefill worker carries `max(1,527 / 1, 4,955 / 3)` = 1,652 ms, two
carry `max(763, 2,478)` = 2,478 ms. One prefill worker and three decode.

A tie: one request with a 1-position prompt and 3 tokens on one slot gives
`b = 2a`. One prefill worker carries `max(3a, 2a) / 3 = a`, two carry
`max(2a, 4a) / 4 = a`; the rule keeps one.

<!-- source: psionic/lib/serve-split.coax place -->
```jai
// Predicted busy time for a window of traffic: each prompt as one step of
// its rows (chapter 175's cb_step_ns), each token after the first as a
// slots-th share of a full decode step. Split the workers to minimize the
// larger of the two pools' time per worker, a / p against b / (workers - p);
// the best p is the floor or the ceiling of workers * a / (a + b). With no
// decode work, every worker but one runs prompts. A tie goes to fewer
// prefill workers. Traffic outside the bounds, or fewer than 2 workers,
// gets no split.
dg_place :: (workers:int,slots:int,reqs:[]DgReq)->DgPlace {
    d:DgPlace;
    if workers<2 || workers>DG_WORKERS || slots<1 || slots>DG_SLOTS || reqs.count<1 || reqs.count>DG_REQUESTS {return d;}
    for r:reqs {if r.prompt<1 || r.prompt>DG_PROMPT || r.output<1 || r.output>DG_PLACE_OUTPUT {return d;}}
    for r:reqs {d.prefill_ns+=cb_step_ns(r.prompt);d.decode_ns+=r.output-1;}
    d.decode_ns=d.decode_ns*cb_step_ns(slots)/slots;
    a:=d.prefill_ns;b:=d.decode_ns;
    // lo is at most workers * a / (a + b), so its larger share is prefill's, a / lo;
    // hi is above it, so its larger share is decode's, b / (workers - hi). hi wins
    // when b / (workers - hi) < a / lo, compared without dividing: lo = 0 always
    // loses and hi = workers never wins.
    lo:=workers*a/(a+b);hi:=lo+1;
    d.prefill=lo;
    if b*lo<a*(workers-hi) {d.prefill=hi;}
    if b==0 {d.prefill=workers-1;}
    d.decode=workers-d.prefill;
    return d;
}
```

<!-- source: psionic/lib/serve-split.coax check-place -->
```jai
#test "the placement by hand" {
    // Two prompts of 512 and outputs of 33: prefill 2 * cb_step_ns(512) = 1,526,868,948 ns;
    // decode 64 tokens at a sixteenth of a 16-row step each, 64 * 39,680,202 / 16 = 158,720,808 ns.
    q:=[2]DgReq.[DgReq.{prompt=512,output=33},DgReq.{prompt=512,output=33}];
    p:=dg_place(4,16,q[..]);#assert p.prefill_ns==1526868948 && p.decode_ns==158720808;
    // 4 * 1,526,868,948 / 1,685,589,756 = 3.6: 3 prefill workers (509 ms each) against 1 decode (159 ms).
    #assert p.prefill==3 && p.decode==1;
    // Outputs of 1,000 tokens: decode 1,998 * 39,680,202 / 16 = 4,955,065,224 ns. 4 * a / (a + b) = 0.94,
    // so p is 1 or 2: a / 1 or b / 3 against a / 2 or b / 2; 1 prefill worker, 3 decode.
    q[0].output=1000;q[1].output=1000;p=dg_place(4,16,q[..]);#assert p.prefill==1 && p.decode==3 && p.decode_ns==4955065224;
    // One slot, prompts of 1 and outputs of 64: 4 * a / (a + b) = 4 / 64, floor 0, kept to 1.
    o:=[2]DgReq.[DgReq.{prompt=1,output=64},DgReq.{prompt=1,output=64}];p=dg_place(4,1,o[..]);
    #assert p.prefill==1 && p.prefill_ns==2*17792472 && p.decode_ns==126*17792472;
    // A tie: prompt 1 and 3 tokens on one slot, b = 2a. One prefill worker: max(3a, 2a) / 3 = a;
    // two: max(2a, 4a) / 4 = a. Fewer prefill workers win.
    t:=[1]DgReq.[DgReq.{prompt=1,output=3}];p=dg_place(4,1,t[..]);#assert p.prefill==1 && p.decode==3;
    t[0].output=4;#assert dg_place(4,1,t[..]).prefill==1;t[0].output=2;#assert dg_place(4,1,t[..]).prefill==2;
    // Fewer than 2 workers, more than 8, or traffic outside the bounds: no split.
    #assert dg_place(1,16,q[..]).prefill==0 && dg_place(2,16,q[..]).prefill==1 && dg_place(8,16,q[..]).prefill==2 && dg_place(9,16,q[..]).decode==0;
    #assert dg_place(4,16,q[0..<0]).prefill==0 && dg_place(4,0,q[..]).prefill==0 && dg_place(4,17,q[..]).prefill==0 && dg_place(4,1,q[..]).prefill==1;
    many:[65]DgReq;for i:0..<65 {many[i]=t[0];}#assert dg_place(4,1,many[0..<64]).prefill>0 && dg_place(4,1,many[..]).prefill==0;
    b:=t;b[0].prompt=0;#assert dg_place(4,1,b[..]).decode==0;b[0].prompt=4097;#assert dg_place(4,1,b[..]).decode==0;b[0].prompt=4096;#assert dg_place(4,1,b[..]).decode>0;
    b=t;b[0].output=0;#assert dg_place(4,1,b[..]).decode==0;b[0].output=65537;#assert dg_place(4,1,b[..]).decode==0;b[0].output=65536;#assert dg_place(4,1,b[..]).decode>0;
    b[0].output=1;#assert dg_place(4,1,b[..]).prefill==3;
}
```

## The simulation

The simulation runs workers 0 to `workers - 1`; the first `prefill` of
them are prefill workers. With `prefill` 0 every worker does both phases,
which is the co-located baseline, scheduled by the same rule as chapter
175: one row for each decoding request first, then prompt chunks in
admission order within the step's `tokens`. A prefill worker sends each
finished prompt down its own link, one message after another; the request
leaves its slot then. A message that arrives short (the `loss` setting
cuts every loss-th one in half) is refused by the same count rule the
receiver uses, and the request is prefilled again from the start. A
prompt no longer than `local` skips the prefill pool and runs on a decode
worker.

<!-- source: psionic/lib/serve-split.coax config -->
```jai
// The model a timed transfer carries: layers holding every position, layers
// holding a window of positions, KV heads, their width, bytes per value,
// and positions per page.
DgShape :: struct {full:int;windowed:int;window:int;kv_heads:int;head_dim:int;value_bytes:int;page:int;}
// A link: gigabits per second and a fixed latency per message.
DgLink :: struct {gbit:int;lat_ns:int;}
// workers: in all. prefill: how many of them run prompts only (0: every
// worker does both, co-located). slots: requests one worker runs at once.
// tokens: positions one step processes. local: prompts this long or shorter
// run on a decode worker. loss: every loss-th transfer arrives cut in half
// (0: none). events: the most event times a run may take.
DgConfig :: struct {workers:int;prefill:int;slots:int;tokens:int;local:int;shape:DgShape;link:DgLink;loss:int;events:int;}
DgReq :: struct {arrive:int;prompt:int;output:int;}
// One request: when its first token came out and when its last did, when
// its state arrived whole at the decode side (-1 if it never moved), when a
// decode worker took it, transfers refused, bytes that arrived and link
// time spent.
DgFate :: struct {first:int;finish:int;arrived:int;decode:int;resent:int;bytes:int;link:int;}
DgSim :: struct {
    status:DgStatus;n:int;fate:[64]DgFate;
    gaps:[4032]int;ngaps:int;tokens:int;rows:int;
    transfers:int;refused:int;bytes:int;end:int;events:int;
}
DgReport :: struct {ttft:BnStats;itl:BnStats;tokens:int;span:int;transfers:int;refused:int;bytes:int;link:int;rows:int;}
// The split: prefill and decode workers, and each pool's predicted busy
// nanoseconds for the traffic (0 workers: refused).
DgPlace :: struct {prefill:int;decode:int;prefill_ns:int;decode_ns:int;}
```

<!-- source: psionic/lib/serve-split.coax sim -->
```jai
// A discrete-event simulation over workers 0 to workers - 1, the first
// `prefill` of them prefill workers. At each event time, in this order:
// steps end (a prefill worker sends each finished prompt down its link, one
// message after another; a request's last token retires it); transfers end
// (accepted: the request waits for a decode worker; refused: it waits to
// be prefilled again); arrivals queue (a prompt longer than `local` for the
// prefill workers when there are any); then idle workers take requests from
// their queue, oldest first, one each in turn while they have free slots;
// and every idle worker with requests starts a step: one row for each
// decoding request, then prompt chunks in admission order within `tokens`.
dg_sim :: (cfg:DgConfig,reqs:[]DgReq)->DgSim {
    s:DgSim;s.status=dg_start(cfg,reqs);
    if s.status!=DgStatus.ok {return s;}
    s.n=reqs.count;
    filled:[64]int;emitted:[64]int;last:[64]int;plan:[64]int;dec:[64]bool;ready:[64]int;cut:[64]bool;moving:[64]bool;
    busy:[8]int;stepping:[8]bool;nrun:[8]int;slot:[128]int;link:[8]int;queue:[128]int;nq:[2]int;
    for i:0..<s.n {s.fate[i]=DgFate.{arrived=-1,decode=-1};}
    a:=0;
    while true {
        // The next event: an arrival, a step's end or a transfer's end.
        t:int;any:=a<s.n;
        if any {t=reqs[a].arrive;}
        for k:0..<cfg.workers {if stepping[k] {t=ifx any then cb_min(t,busy[k]) else busy[k];any=true;}}
        for i:0..<s.n {if moving[i] {t=ifx any then cb_min(t,ready[i]) else ready[i];any=true;}}
        if !any {break;}
        s.events+=1;
        if s.events>cfg.events {s.status=DgStatus.events;return s;}
        s.end=t;
        for k:0..<cfg.workers {
            if !stepping[k] || busy[k]!=t {continue;}
            stepping[k]=false;m:=0;
            for j:0..<nrun[k] {
                i:=slot[k*16+j];r:=reqs[i];
                if dec[i] {emitted[i]+=1;s.gaps[s.ngaps]=t-last[i];s.ngaps+=1;last[i]=t;}
                else {
                    filled[i]+=plan[i];
                    if filled[i]==r.prompt && emitted[i]==0 {s.fate[i].first=t;last[i]=t;emitted[i]=1;}
                }
                if emitted[i]==r.output {s.fate[i].finish=t;s.tokens+=r.output;continue;}
                if k<cfg.prefill && filled[i]==r.prompt {
                    b:=dg_bytes(cfg.shape,r.prompt);go:=cb_max(t,link[k]);link[k]=go+dg_transfer_ns(cfg.link,b);ready[i]=link[k];moving[i]=true;
                    s.transfers+=1;s.fate[i].link+=link[k]-go;
                    cut[i]=cfg.loss!=0 && s.transfers%cfg.loss==0;
                    continue;
                }
                slot[k*16+m]=i;m+=1;
            }
            nrun[k]=m;
        }
        for i:0..<s.n {
            if !moving[i] || ready[i]!=t {continue;}
            moving[i]=false;b:=dg_bytes(cfg.shape,reqs[i].prompt);got:=b;
            if cut[i] {got=b/2;}
            s.bytes+=got;s.fate[i].bytes+=got;w:=1;
            if dg_count(b,got)==DgRefusal.none {s.fate[i].arrived=t;}
            else {s.fate[i].resent+=1;s.refused+=1;filled[i]=0;w=0;}
            queue[w*64+nq[w]]=i;nq[w]+=1;
        }
        while a<s.n && reqs[a].arrive==t {
            w:=1;
            if cfg.prefill>0 && reqs[a].prompt>cfg.local {w=0;}
            queue[w*64+nq[w]]=a;nq[w]+=1;a+=1;
        }
        // Idle workers take one request each in turn, until slots or queues run out.
        took:=true;
        while took {
            took=false;
            for k:0..<cfg.workers {
                w:=ifx k<cfg.prefill then 0 else 1;
                if stepping[k] || nrun[k]==cfg.slots || nq[w]==0 {continue;}
                i:=queue[w*64];
                for j:1..<nq[w] {queue[w*64+j-1]=queue[w*64+j];}
                nq[w]-=1;slot[k*16+nrun[k]]=i;nrun[k]+=1;took=true;
                if w==1 {s.fate[i].decode=t;}
            }
        }
        for k:0..<cfg.workers {
            if stepping[k] || nrun[k]==0 {continue;}
            left:=cfg.tokens;
            for j:0..<nrun[k] {i:=slot[k*16+j];dec[i]=filled[i]==reqs[i].prompt;if dec[i] {left-=1;}}
            for j:0..<nrun[k] {
                i:=slot[k*16+j];
                if !dec[i] {plan[i]=cb_min(reqs[i].prompt-filled[i],left);left-=plan[i];s.rows+=plan[i];}
            }
            busy[k]=t+cb_step_ns(cfg.tokens-left);stepping[k]=true;
        }
    }
    return s;
}
```

The report gives nearest-rank TTFT and gap percentiles in nanoseconds
(chapter 172's `bn_stats`), the output tokens, and the span from the first
arrival to the last token; the example divides the two for tokens per
second. Every gap after the first token counts, so the first gap after a
handoff includes the transfer and any wait for a decode slot.

<!-- source: psionic/lib/serve-split.coax report -->
```jai
// Nearest-rank time to first token and gaps between tokens in
// nanoseconds (chapter 172's bn_stats: P50, P90, P95, P99), output tokens,
// and the span from the first arrival to the last token.
dg_report :: (s:DgSim,reqs:[]DgReq)->DgReport {
    p:=DgReport.{tokens=s.tokens,span=s.end-reqs[0].arrive,transfers=s.transfers,refused=s.refused,bytes=s.bytes,rows=s.rows};
    first:[64]int;
    for i:0..<s.n {first[i]=s.fate[i].first-reqs[i].arrive;p.link+=s.fate[i].link;}
    gaps:=s.gaps;p.ttft=bn_stats(first[0..<s.n]);p.itl=bn_stats(gaps[0..<s.ngaps]);
    return p;
}
```

## A trace by hand

Four requests arrive at 0, each with a 64-position prompt and 3 output
tokens, on two workers with 2 slots and steps of 64 positions. A 64-row
step takes `17,792,472 + 63 * 1,459,182 = 109,720,938` ns; a 2-row step
19,251,654 ns; a 1-row step 17,792,472 ns.

**Co-located.** Requests are handed out one each in turn: worker 0 takes A
and C, worker 1 takes B and D. On worker 0:

| Step | Rows | Ends at (ns) | What comes out |
| --- | --- | --- | --- |
| 1 | A's 64 prompt rows | 109,720,938 | A's first token |
| 2 | A's decode row and C's first 63 prompt rows | 219,441,876 | A's second token |
| 3 | A's decode row and C's last prompt row | 238,693,530 | A's last token, C's first |
| 4 | C's decode row | 256,486,002 | C's second token |
| 5 | C's decode row | 274,278,474 | C's last token |

A waits 109,720,938 ns for its second token: C's prompt rows ride in the
same step. That is the interference this chapter is about.

**Split, one prefill worker and one decode worker.** Worker 0 runs one
whole prompt per step, so the first tokens come at 1, 2, 3 and 4 times
109,720,938 ns. Each state is 3,145,800 bytes and takes 2,566,640 ns on
the link; A's arrives at 112,287,578 ns. Worker 1 decodes it in one-row
steps: A's tokens come at 130,080,050 and 147,872,522 ns. Every request's
gaps are 20,359,112 ns (a transfer and a step) and 17,792,472 ns.

The split removes the 109.7 ms gap and costs time to first token: D's
first token comes at 438.9 ms instead of 238.7 ms, because only one
worker runs prompts.

<!-- source: psionic/lib/serve-split.coax check-hand -->
```jai
#test "the hand trace: two workers co-located, then one prefill and one decode" {
    h:=dgx_hand();s:=dg_sim(dgx_hand_fleet(0),h[..]);
    // Each worker: A's 64 rows (109,720,938 ns); A's second token with C's first 63 rows
    // (the same); A's last token with C's last row (19,251,654); C's two tokens alone.
    #assert s.fate[0].first==109720938 && s.fate[0].finish==238693530 && s.fate[2].first==238693530 && s.fate[2].finish==274278474;
    #assert s.fate[1].first==109720938 && s.fate[1].finish==238693530 && s.fate[3].finish==274278474 && s.fate[3].decode==0;
    #assert s.gaps[0]==109720938 && s.gaps[2]==19251654 && s.gaps[4]==17792472 && s.ngaps==8 && s.transfers==0 && s.end==274278474;
    r:=dg_report(s,h[..]);
    #assert r.ttft.n==4 && r.ttft.p[0]==109720938 && r.ttft.p[3]==238693530 && r.itl.n==8 && r.itl.p[0]==17792472 && r.itl.p[1]==109720938;
    #assert r.tokens==12 && r.span==274278474 && r.transfers==0 && r.bytes==0 && r.link==0 && r.rows==256;
    // Split: one prompt a step on worker 0; each 3,145,800-byte state takes 2,566,640 ns;
    // worker 1 decodes it in one-row steps of 17,792,472 ns.
    s=dg_sim(dgx_hand_fleet(1),h[..]);
    for i:0..<4 {f:=s.fate[i];#assert f.first==109720938*(i+1) && f.arrived==f.first+2566640 && f.decode==f.arrived && f.finish==f.arrived+2*17792472 && f.bytes==3145800 && f.link==2566640;}
    for g:0..<8 {#assert s.gaps[g]==ifx g%2==0 then 20359112 else 17792472;}
    #assert s.transfers==4 && s.bytes==4*3145800 && s.end==477035336 && s.rows==256 && s.tokens==12;
    r=dg_report(s,h[..]);#assert r.ttft.p[0]==219441876 && r.itl.p[0]==17792472 && r.itl.p[2]==20359112 && r.link==4*2566640 && r.bytes==4*3145800;
    // Everything 1,000 ns later: the same run, shifted, in the same 17 events.
    d:=h;for i:0..<4 {d[i].arrive=1000;}
    x:=dg_sim(dgx_hand_fleet(1),d[..]);r=dg_report(x,d[..]);
    #assert x.end==477036336 && x.events==17 && r.span==477035336 && r.ttft.p[0]==219441876 && x.fate[0].decode==112288578;
    // Every second transfer cut in half: B's (the 2nd) is refused and B is prefilled again;
    // D's (the 4th, then the 6th) twice.
    c:=dgx_hand_fleet(1);c.loss=2;s=dg_sim(c,h[..]);
    #assert s.fate[1].resent==1 && s.fate[3].resent==2 && s.fate[0].resent==0 && s.fate[2].resent==0 && s.refused==3 && s.transfers==7 && s.rows==448;
    // A cut message brings half its bytes: 4 whole and 3 halves arrive.
    #assert s.bytes==4*3145800+3*1572900 && s.fate[3].bytes==3145800+2*1572900 && s.fate[3].link==3*2566640;
    #assert s.fate[1].first==219441876 && s.fate[1].arrived==551171330 && s.fate[1].decode==551171330 && s.fate[3].arrived==773179846 && s.end==808764790;
    // A prompt of 64 or shorter runs on the decode worker when local is 64, not when it is 63.
    c=dgx_hand_fleet(1);c.local=64;s=dg_sim(c,h[..]);#assert s.transfers==0 && s.fate[0].decode==0 && s.fate[0].arrived== -1;
    c.local=63;s=dg_sim(c,h[..]);#assert s.transfers==4;
    // One output token: done at the prefill worker, nothing moves.
    o:=h;for i:0..<4 {o[i].output=1;}s=dg_sim(dgx_hand_fleet(1),o[..]);
    #assert s.transfers==0 && s.fate[3].finish==438883752 && s.fate[3].decode== -1 && s.ngaps==0 && s.tokens==4;
}
```

## Run it

```shell
coax/bin/coax run psionic/examples/serve-split.coax
```

[`serve-split.coax`](../examples/serve-split.coax) prints the transfer
arithmetic, the handoff on the stand-in model, the hand trace, and then
runs 48 synthetic requests on 4 workers at two loads. The traffic comes
from chapter 172's load schedule: prompts of 128, 512, 1,024 or 2,048
positions (weights 3, 2, 2, 1), outputs of 16, 32 or 64 tokens (weights 1,
2, 1), a mean gap of 400 or 300 ms with 50% jitter. Each worker has 16
slots and steps of 256 positions; the link is 10 Gbit/s with 50 µs of
latency.

<!-- source: psionic/examples/serve-split.coax main -->
```jai
main :: () {
    g:=dg_gptoss();l:=DgLink.{gbit=10,lat_ns=50000};
    print("gpt-oss-20b state (%): 64 positions % bytes, % ns at 10 Gbit/s; 1024 positions % bytes, % ns\n",
        DG_PROFILE,dg_bytes(g,64),dg_transfer_ns(l,dg_bytes(g,64)),dg_bytes(g,1024),dg_transfer_ns(l,dg_bytes(g,1024)));

    for hybrid:0..<2 {
        c:=PgConfig.{page=4,dim=3,hybrid=hybrid==1};c.cap[0]=12;
        one:=dgx_colocated(c,13,6);two:=dgx_handed(c,13,6);
        print("stand-in model, hybrid %, 13-position prompt: % frames, % bytes, %\n",hybrid==1,two.frames,two.bytes,dg_refusal_name(two.refusal));
        print("  one worker:");for t:0..<6 {print(" %",one.tok[t]);}
        print("; handed off:");for t:0..<6 {print(" %",two.tok[t]);}print("\n");
    }

    trace("hand trace, 2 workers co-located",dgx_hand_fleet(0));
    trace("hand trace, 1 prefill and 1 decode",dgx_hand_fleet(1));

    q:[48]DgReq;
    for gap:[2]int.[400000,300000] {
        if dgx_traffic(gap,q[..])!=BnStatus.ok {print("traffic refused\n");return;}
        pl:=dg_place(4,16,q[..]);
        print("48 requests, mean gap % ms, 4 workers: predicted prefill % ms, decode % ms; placed % prefill, % decode\n",
            gap/1000,pl.prefill_ns/1000000,pl.decode_ns/1000000,pl.prefill,pl.decode);
        line("co-located",dgx_fleet(0),q[..]);
        names:=[3]string.["1 prefill, 3 decode","2 prefill, 2 decode","3 prefill, 1 decode"];
        for p:1..<4 {line(names[p-1],dgx_fleet(p),q[..]);}
    }
    c:=dgx_fleet(3);c.local=128;line("3:1, prompts of 128 or fewer on the decode worker",c,q[..]);
    c=dgx_fleet(3);c.loss=4;line("3:1, every fourth transfer cut",c,q[..]);
    c=dgx_fleet(3);c.link.gbit=1;line("3:1 on a 1 Gbit/s link",c,q[..]);
}
```

<!-- transcript: psionic/fixtures/serve-split/serve-split.out -->
```text
gpt-oss-20b state (dg-split-v1): 64 positions 3145800 bytes, 2566640 ns at 10 Gbit/s; 1024 positions 28311624 bytes, 22699300 ns
stand-in model, hybrid false, 13-position prompt: 4 frames, 552 bytes, accepted
  one worker: 11 11 4 11 11 4; handed off: 11 11 4 11 11 4
stand-in model, hybrid true, 13-position prompt: 5 frames, 672 bytes, accepted
  one worker: 11 11 4 11 11 4; handed off: 11 11 4 11 11 4
hand trace, 2 workers co-located:
  A first token 109720938 ns, last 238693530 ns
  B first token 109720938 ns, last 238693530 ns
  C first token 238693530 ns, last 274278474 ns
  D first token 238693530 ns, last 274278474 ns
  gaps (ns): 109720938 109720938 19251654 19251654 17792472 17792472 17792472 17792472
hand trace, 1 prefill and 1 decode:
  A first token 109720938 ns, last 147872522 ns, state arrived 112287578 ns
  B first token 219441876 ns, last 257593460 ns, state arrived 222008516 ns
  C first token 329162814 ns, last 367314398 ns, state arrived 331729454 ns
  D first token 438883752 ns, last 477035336 ns, state arrived 441450392 ns
  gaps (ns): 20359112 17792472 20359112 17792472 20359112 17792472 20359112 17792472
48 requests, mean gap 400 ms, 4 workers: predicted prefill 43742 ms, decode 3928 ms; placed 3 prefill, 1 decode
  co-located: first token P50 779.7 P95 1600.7 ms; gaps P50 17.7 P95 20.7 P99 389.8 ms; 1632 tokens in 19940.1 ms, 81 tokens/s
  1 prefill, 3 decode: first token P50 10250.0 P95 26217.9 ms; gaps P50 17.7 P95 17.7 P99 40.4 ms; 1632 tokens in 45634.8 ms, 35 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 702.0 ms, 29440 prompt positions run
  2 prefill, 2 decode: first token P50 2061.1 P95 10432.8 ms; gaps P50 17.7 P95 20.7 P99 40.4 ms; 1632 tokens in 30493.9 ms, 53 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 702.0 ms, 29440 prompt positions run
  3 prefill, 1 decode: first token P50 779.7 P95 2800.4 ms; gaps P50 19.2 P95 22.1 P99 42.8 ms; 1632 tokens in 20931.3 ms, 77 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 702.0 ms, 29440 prompt positions run
48 requests, mean gap 300 ms, 4 workers: predicted prefill 43742 ms, decode 3928 ms; placed 3 prefill, 1 decode
  co-located: first token P50 812.5 P95 3366.1 ms; gaps P50 20.7 P95 389.8 P99 389.8 ms; 1632 tokens in 17693.2 ms, 92 tokens/s
  1 prefill, 3 decode: first token P50 11499.4 P95 29914.0 ms; gaps P50 17.7 P95 17.7 P99 40.4 ms; 1632 tokens in 45426.8 ms, 35 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 702.0 ms, 29440 prompt positions run
  2 prefill, 2 decode: first token P50 2523.8 P95 10966.5 ms; gaps P50 17.7 P95 20.7 P99 40.4 ms; 1632 tokens in 26385.9 ms, 61 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 702.0 ms, 29440 prompt positions run
  3 prefill, 1 decode: first token P50 1559.5 P95 5906.1 ms; gaps P50 19.2 P95 25.0 P99 49.5 ms; 1632 tokens in 20310.5 ms, 80 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 702.0 ms, 29440 prompt positions run
  3:1, prompts of 128 or fewer on the decode worker: first token P50 779.7 P95 3608.6 ms; gaps P50 20.7 P95 23.6 P99 207.4 ms; 1632 tokens in 18242.3 ms, 89 tokens/s
    30 transfers (0 refused), 761268336 bytes arrived, link busy 610.5 ms, 29440 prompt positions run
  3:1, every fourth transfer cut: first token P50 1725.2 P95 7900.0 ms; gaps P50 19.2 P95 22.1 P99 48.6 ms; 1632 tokens in 25688.5 ms, 63 tokens/s
    63 transfers (15 refused), 1023938460 bytes arrived, link busy 941.8 ms, 39680 prompt positions run
  3:1 on a 1 Gbit/s link: first token P50 1559.5 P95 5906.1 ms; gaps P50 19.2 P95 25.0 P99 244.3 ms; 1632 tokens in 20547.0 ms, 79 tokens/s
    48 transfers (0 refused), 874515840 bytes arrived, link busy 6998.5 ms, 29440 prompt positions run
```

What the runs show, all from the step model:

- **Interference.** At a 300 ms gap the co-located workers' gap P95 and
  P99 are both 389.8 ms: a full step of 256 positions, decode rows and a
  prompt chunk together (`cb_step_ns(256)` = 389,883,882 ns). Three prefill workers and
  one decode worker bring the P95 to 25.0 ms and the P99 to 49.5 ms, the
  P99 lower by 340.3 ms.
- **The price.** The same split doubles the median time to first token
  (812.5 ms to 1,559.5 ms), raises its P95 from 3,366.1 ms to 5,906.1 ms,
  and lowers throughput from 92 to 80 tokens a second: one worker fewer
  runs prompts, and the step model gives batched prompts no discount.
- **The placement.** Prompts are about 92% of the predicted work
  (43,742 ms against 3,928 ms), and the rule picks 3 prefill and 1 decode,
  the best of the three splits on time to first token and throughput. Two
  and two has a lower gap P95 (20.7 ms) but a P95 time to first token of
  11.0 s; one prefill worker is overwhelmed, with a median time to first
  token over 10 s.
- **At 400 ms.** Interference shows only in the P99 (389.8 ms co-located
  against 42.8 ms split); the P95 is already low co-located.
- **Local prefill.** Letting prompts of 128 or fewer run on the decode
  worker saves 18 transfers and brings the median time to first token
  back to 779.7 ms, but those prompts interfere again: the gap P99 is
  207.4 ms.
- **Lost transfers.** Cutting every fourth transfer refuses 15 of 63 and
  prefills those prompts again: 10,240 more prompt positions, a P95 time
  to first token of 7,900.0 ms and 63 tokens a second. A cut message
  counts the half of its bytes that arrived.
- **A slow link.** At 1 Gbit/s each transfer takes ten times as long
  (6,998.5 ms of link time in all): a 1,024-position state takes 227 ms to
  move and a 2,048-position one 428 ms. The gap P99 rises to 244.3 ms; the
  P50 and P95 do not move.

The `#test`s freeze these numbers, so a change that moves any of them
fails until the chapter is updated.

<!-- source: psionic/lib/serve-split.coax check-frozen -->
```jai
#test "frozen: 48 requests 300 ms apart, co-located on 4 workers" #steps 400000000 {
    q:=dgx_q(300000);p:=dg_report(dg_sim(dgx_fleet(0),q[..]),q[..]);
    #assert dgx_frozen(p,812539072,3366199406,20710836,389883882,389883882,17693296578) && p.transfers==0 && p.rows==29440;
}
#test "frozen: 300 ms apart, 1 prefill and 3 decode" #steps 400000000 {
    q:=dgx_q(300000);#assert dgx_frozen(dg_report(dg_sim(dgx_fleet(1),q[..]),q[..]),11499452690,29914032020,17792472,17792472,40491772,45426807989);
}
#test "frozen: 300 ms apart, 2 prefill and 2 decode" #steps 400000000 {
    q:=dgx_q(300000);#assert dgx_frozen(dg_report(dg_sim(dgx_fleet(2),q[..]),q[..]),2523842702,10966546982,17792472,20710836,40491772,26385918498);
}
#test "frozen: 300 ms apart, 3 prefill and 1 decode, the placement's split" #steps 400000000 {
    q:=dgx_q(300000);p:=dg_report(dg_sim(dgx_fleet(3),q[..]),q[..]);
    #assert dgx_frozen(p,1559535528,5906186870,19251654,25088382,49563444,20310536734);
    #assert p.transfers==48 && p.refused==0 && p.bytes==874515840 && p.link==702012702 && p.rows==29440;
    #assert dg_place(4,16,q[..]).prefill==3;
}
#test "frozen: 300 ms apart, 3:1, prompts of 128 or fewer run on the decode worker" #steps 400000000 {
    q:=dgx_q(300000);c:=dgx_fleet(3);c.local=128;p:=dg_report(dg_sim(c,q[..]),q[..]);
    #assert dgx_frozen(p,779767764,3608655820,20710836,23629200,207486132,18242399044) && p.transfers==30 && p.bytes==761268336;
}
#test "frozen: 300 ms apart, 3:1, every fourth transfer cut" #steps 400000000 {
    q:=dgx_q(300000);c:=dgx_fleet(3);c.loss=4;p:=dg_report(dg_sim(c,q[..]),q[..]);
    #assert dgx_frozen(p,1725285410,7900007046,19251654,22170018,48630443,25688517392);
    #assert p.transfers==63 && p.refused==15 && p.rows==39680 && p.bytes==1023938460 && p.link==941838904;
}
#test "frozen: 300 ms apart, 3:1 on a 1 Gbit/s link" #steps 400000000 {
    q:=dgx_q(300000);c:=dgx_fleet(3);c.link.gbit=1;p:=dg_report(dg_sim(c,q[..]),q[..]);
    #assert dgx_frozen(p,1559535528,5906186870,19251654,25088382,244335464,20547047006) && p.link==6998526720;
}
#test "frozen: 400 ms apart, co-located and 3:1" #steps 400000000 {
    q:=dgx_q(400000);
    #assert dgx_frozen(dg_report(dg_sim(dgx_fleet(0),q[..]),q[..]),779767764,1600792644,17792472,20710836,389883882,19940198730);
    #assert dgx_frozen(dg_report(dg_sim(dgx_fleet(3),q[..]),q[..]),779767764,2800424348,19251654,22170018,42800804,20931336612);
}
```

## Refusals

[`serve-split-refuse.coax`](../examples/serve-split-refuse.coax) breaks one
snapshot ten ways, each drawing its own refusal, then cuts every second
transfer in the hand trace, and asks the simulation and the placement to
run configurations they refuse.

```shell
coax/bin/coax run psionic/examples/serve-split-refuse.coax
```

<!-- source: psionic/examples/serve-split-refuse.coax main -->
```jai
main :: () {
    what:=[11]string.["as sent","one word short","version 2","expected model 0","receiver dim 2","expected request 3",
        "frame count 5","one word extra","one payload bit flipped","slot already holding a page","3 free frames"];
    for k:0..<11 {
        e:=dgx_case(k);
        print("%: %, % frames, % bytes, % free frames left\n",what[k],dg_refusal_name(e.got.refusal),e.got.frames,e.got.bytes,e.got.st.pool.nfree[0]);
    }

    h:=dgx_hand();c:=dgx_hand_fleet(1);c.loss=2;s:=dg_sim(c,h[..]);
    print("hand trace, every second transfer cut in half: % transfers, % refused, % prompt positions run\n",s.transfers,s.refused,s.rows);
    for i:0..<4 {
        f:=s.fate[i];
        print("  request %: first token % ns, refused % times, state arrived % ns, last token % ns\n",i,f.first,f.resent,f.arrived,f.finish);
    }

    x:=dgx_hand_fleet(2);print("2 prefill workers of 2: %\n",dg_status_name(dg_sim(x,h[..]).status));
    x=dgx_hand_fleet(1);x.loss=1;print("every transfer cut: %\n",dg_status_name(dg_sim(x,h[..]).status));
    x=dgx_hand_fleet(1);x.events=16;print("16 events for a run that needs 17: %\n",dg_status_name(dg_sim(x,h[..]).status));
    b:=h;b[2].arrive=6;print("an arrival before the one ahead of it: %\n",dg_status_name(dg_sim(dgx_hand_fleet(1),b[..]).status));
    p:=dg_place(1,16,h[..]);print("placement on 1 worker: % prefill, % decode\n",p.prefill,p.decode);
}
```

<!-- transcript: psionic/fixtures/serve-split/serve-split-refuse.out -->
```text
as sent: accepted, 4 frames, 552 bytes, 8 free frames left
one word short: fewer bytes than declared, 0 frames, 0 bytes, 12 free frames left
version 2: another snapshot version, 0 frames, 0 bytes, 12 free frames left
expected model 0: another model, 0 frames, 0 bytes, 12 free frames left
receiver dim 2: another page geometry, 0 frames, 0 bytes, 12 free frames left
expected request 3: another request or prompt length, 0 frames, 0 bytes, 12 free frames left
frame count 5: frame count does not match the positions, 0 frames, 0 bytes, 12 free frames left
one word extra: more bytes than declared, 0 frames, 0 bytes, 12 free frames left
one payload bit flipped: checksum does not match, 0 frames, 0 bytes, 12 free frames left
slot already holding a page: slot already in use, 0 frames, 0 bytes, 11 free frames left
3 free frames: too few free frames, 0 frames, 0 bytes, 3 free frames left
hand trace, every second transfer cut in half: 7 transfers, 3 refused, 448 prompt positions run
  request 0: first token 109720938 ns, refused 0 times, state arrived 112287578 ns, last token 147872522 ns
  request 1: first token 219441876 ns, refused 1 times, state arrived 551171330 ns, last token 586756274 ns
  request 2: first token 329162814 ns, refused 0 times, state arrived 331729454 ns, last token 367314398 ns
  request 3: first token 438883752 ns, refused 2 times, state arrived 773179846 ns, last token 808764790 ns
2 prefill workers of 2: configuration or traffic out of bounds
every transfer cut: configuration or traffic out of bounds
16 events for a run that needs 17: event limit reached
an arrival before the one ahead of it: arrivals out of order
placement on 1 worker: 0 prefill, 0 decode
```

A header alone (9 words) is checked for its own faults before it is
refused as short, so a header with version 2 draws `version`; the
`#test` below also refuses a whole, correctly summed header for a prompt
of no positions as `request`.

Every refused snapshot leaves the receiver's 12 free frames as they were
(11 where the test took one to fill the slot, 3 where it took nine). In
the lossy trace, B's transfer (the second) and D's (the fourth, then the
sixth) are refused; each is prefilled again, 3 * 64 = 192 more prompt
positions, and its state arrives whole before its decode starts. The
example exits with status 0.

<!-- source: psionic/lib/serve-split.coax check-refuse -->
```jai
#test "every refusal, and the one snapshot accepted" {
    for k:0..<11 {
        e:=dgx_case(k);
        #assert e.got.refusal==cast(DgRefusal)cast(u8)k;
        if k==0 {#assert e.got.frames==4 && e.got.bytes==8*(9+4*15) && e.got.st.pool.nfree[0]==e.free-4;}
        else {#assert e.got.frames==0 && e.got.bytes==0 && e.got.st.pool.nfree[0]==e.free && e.got.st.table[2].n==e.held;}
    }
    // A crafted header for a prompt of no positions, whole and summed: refused as a request.
    c:=PgConfig.{page=4,dim=3};c.cap[0]=12;d:=dgx_store(c);mem:[64]f64;
    w:=[9]u64.[1,1,2,0,4,3,0,0,0];w[8]=cast(u64)dg_sum(w[..]);
    #assert dg_receive(d,mem[..],2,DgExpect.{model=1,request=2,positions=0},w[..]).refusal==DgRefusal.request;
    // The header alone, 9 words: its own faults first; with none, too short. 8 words: too short.
    st:=dgx_store(PgConfig.{page=4,dim=3});st.cfg.cap[0]=12;hw:=[9]u64.[2,1,2,13,4,3,0,4,0];
    #assert dg_check(st.cfg,DgExpect.{model=1,request=2,positions=13},hw[..])==DgRefusal.version;
    #assert dg_check(st.cfg,DgExpect.{model=1,request=2,positions=13},hw[0..<8])==DgRefusal.partial;
    hw[0]=1;#assert dg_check(st.cfg,DgExpect.{model=1,request=2,positions=13},hw[..])==DgRefusal.partial;
    // The count rule alone: fewer, exactly, more.
    #assert dg_count(10,9)==DgRefusal.partial && dg_count(10,10)==DgRefusal.none && dg_count(10,11)==DgRefusal.excess;
}
```

## Independent checks

- **Token for token.** The handed-off request equals the same request on
  one worker and on chapter 176's contiguous reference, for four prompt
  lengths, with and without recurrent state.
- **Transfers against a known corruption.** For 400 random snapshots
  (pages of 1 to 8, 1 to 4 values, hybrid or not, up to 16 frames in a
  scrambled order, values including -0), each seed applies one known
  corruption, or none: a cut at any length, another version, model,
  request or prompt length, another page size, width or hybrid flag, a
  wrong frame count, extra words, one flipped bit anywhere in the payload
  or the checksum, a busy slot, too few frames. The refusal must be the
  one that corruption names. When accepted, every frame holds the
  sender's words bit for bit, each is held once, the free count fell by
  exactly the frame count, the byte count is `8 * (9 + frames * width)`,
  and no other memory was written.
- **A brute-force placement.** For 2,000 random traffic windows and worker
  counts, the closed-form split equals the best of every split tried, with
  the step model's constants written out by hand.
- **Chapter 175's scheduler.** One co-located worker with every request at
  0 is chapter 175's scheduler: for 60 random workloads, every first and
  last token time and the end time equal `cb_run`'s, to the nanosecond.
- **Simulation invariants.** For 40 random fleets and traffic (2 to 6
  workers, any split, local thresholds, lost transfers, links of 1 to
  100 Gbit/s): every request finishes with all its tokens; a request moves
  if and only if it is split, longer than `local` and wants more than one
  token; its state arrives at least one transfer after its first token,
  and its decode starts only after that; bytes and link time are its
  transfers times the formula; prompt positions run are each prompt times
  one plus its refusals; and the gaps add up to each request's first to
  last token.

`coax mutate psionic/lib/serve-split.coax --root-only` kills 581 of 581
mutants.

## What remains

- **Real transfers.** The snapshot moves between two arrays in one
  process. Moving it between GPUs or machines (PCIe, NVLink, RDMA or a
  network socket through chapter 188's peer link), measuring the transfer
  time per request on real links, and checking the measured times against
  this model are not built.
- **A message in `proto/`.** The header is nine words here; a published,
  versioned protobuf message for the snapshot, and chapter 12's
  identity-bound claim (a model digest rather than a small integer) in its
  header, are not built.
- **Placement over time.** The rule splits the workers once for a window
  of traffic. Re-splitting as traffic changes, scaling each pool with
  chapter 177's autoscaler (prefill on unserved prompt tokens, decode on
  requests in flight, as DeepSeek describes), and a separate vision
  encoder role are not built.
- **Memory on each side.** A prefill worker frees its slot when the
  prompt ends, as if the link buffered the pages; holding the pages until
  the decode side acknowledges them, and the decode pool's page budget,
  are not modeled.
- **A measured need.** The interference here comes from chapter 175's
  straight-line step model, which gives batched rows no discount. Whether
  real prefill interference is the bottleneck on owner-approved machines
  is for chapter 172's benchmark to measure first.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- [gpt-oss target](../docs/targets/gpt-oss.md) (layers, KV heads, window)
