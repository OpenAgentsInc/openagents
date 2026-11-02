# 191. Experts across machines and from disk

> **Status: partly implemented.** Expert parallelism and expert streaming
> run on the CPU (profiles expert-mesh-v1 and expert-stream-v1). Routed
> experts are split over peers simulated as separate expert stores in one
> process, tokens are dispatched and outputs returned with every byte
> counted, and the result equals [chapter 85](ch085-execute-mistral-small-7.md)'s
> one-machine layer bit for bit. A placement balances expected expert
> load from a recorded routing profile. An expert cache replays routing
> traces from chapter 85's router under three policies and turns misses
> into bytes, read time and tokens a second at measured disk and memory
> rates. Still to build: peers on real machines over
> [chapter 188](ch188-peers-transport-and-discovery.md)'s transport with
> measured per-hop latency, Mistral Small 4's real weights and real
> routing traces, reads from a real NVMe drive, and prefetch that predicts
> the next layer's experts.

Mixture-of-experts models keep most of their weights in experts that each
token uses rarely. In Mistral Small 4, 4,608 routed experts (36 layers of
128) take 115,964,153,856 bytes, 96.6% of the checkpoint, and a token
uses 4 experts a layer. This chapter moves those experts in two ways:
across machines, so that each machine holds a part, and onto disk, so
that memory holds only the experts in use.

Both change where an expert runs. Neither may change its result.

The libraries are [`expert-mesh.coax`](../lib/expert-mesh.coax), an
`#authority` file because it decides which peer holds which expert, and
[`expert-stream.coax`](../lib/expert-stream.coax). Both load chapter 85's
[`mistral-small.coax`](../lib/mistral-small.coax) and use its router and
its expert arithmetic unchanged.

```shell
coax/bin/coax run psionic/examples/expert-mesh.coax
coax/bin/coax run psionic/examples/expert-stream.coax
```

## How unevenly experts are used

A router does not spread tokens evenly. If one peer holds the experts
that most tokens choose, every layer waits for that peer. The first step
is to count. `xm_count` counts, for each expert, the (token, slot) pairs
that chose it. `xm_load` adds up the counts that each peer serves under a
placement. `xm_imbalance` divides the busiest peer's load by the mean, in
thousandths: 1.000 is perfect balance, and a layer takes that many times
longer than it would if the load were even.

<!-- source: psionic/lib/expert-mesh.coax load -->
```jai
// counts[e]: how many (token, slot) pairs of the first `tokens` routes
// chose expert e. A route that was refused, or names an expert outside
// counts, refuses.
xm_count :: (routes:[]M4Route,tokens:int,counts:[]int)->XmStatus {
    if tokens<0 || tokens>routes.count {return XmStatus.input;}
    for e:0..<counts.count {counts[e]=0;}
    for t:0..<tokens {
        r:=routes[t];
        if r.status!=M4Status.ok {return XmStatus.route;}
        for slot:0..<r.count {
            e:=r.ids[slot];
            if e<0 || e>=counts.count {return XmStatus.route;}
            counts[e]+=1;
        }
    }
    return XmStatus.ok;
}
// A placement's load: the uses each peer serves, their total, and the
// largest. owner[e] is the peer that holds expert e.
XmLoad :: struct {status:XmStatus;peers:int;load:[8]int;total:int;most:int;}
xm_load :: (counts:[]int,owner:[]int,peers:int)->XmLoad {
    l:=XmLoad.{peers=peers};
    if peers<1 || peers>XM_PEERS_MAX || owner.count!=counts.count {l.status=XmStatus.input;return l;}
    for e:0..<counts.count {
        p:=owner[e];
        if p<0 || p>=peers {l.status=XmStatus.owner;return l;}
        l.load[p]+=counts[e];l.total+=counts[e];
    }
    for p:0..<peers {l.most=rp_max(l.most,l.load[p]);}
    return l;
}
// The busiest peer's load over the mean, in thousandths, rounded down:
// 1000 is perfect balance, peers * 1000 is one peer doing everything. A
// layer waits for its busiest peer, so this is the slowdown imbalance
// causes. No uses at all count as balanced.
xm_imbalance :: (l:XmLoad)->int {
    if l.total==0 {return 1000;}
    return l.most*l.peers*1000/l.total;
}
```

A worked example. Six tokens choose two experts each from eight, so there
are 12 uses: expert 0 has 5, expert 1 has 3, expert 2 has 2, experts 4
and 6 have 1 each, and the rest have none. The usual split gives peer 0
experts 0 to 3 and peer 1 experts 4 to 7. Peer 0 serves 5 + 3 + 2 + 0 =
10 uses and peer 1 serves 2. The mean is 6, so the imbalance is 10 / 6 =
1.666, rounded down: the layer takes two thirds longer than it needs to.

## A placement that balances the load

`xm_place` takes expected counts, from a routing profile recorded on
earlier tokens, and the number of experts each peer has memory for. It
gives out experts most-used first, each to the least-loaded peer that
still has room. This is Graham's longest-processing-time rule. When every
peer has room to spare, the busiest peer's load is within
4/3 − 1/(3 × peers) of the best possible.

<!-- source: psionic/lib/expert-mesh.coax place -->
```jai
// The usual split: peer d holds a contiguous block of experts, cut by
// chapter 132's rule (sd_first), whatever their load.
xm_contiguous :: (experts:int,peers:int,owner:[]int)->XmStatus {
    if experts<1 || peers<1 || peers>XM_PEERS_MAX || owner.count!=experts {return XmStatus.input;}
    for d:0..<peers {for e:sd_first(experts,peers,d)..<sd_first(experts,peers,d+1) {owner[e]=d;}}
    return XmStatus.ok;
}
// A placement that balances expected load: counts[e] is expert e's
// expected uses (from a recorded routing profile), slots[p] the experts
// peer p has memory for. Experts go out most-used first (the lower id on a
// tie), each to the peer with room that has the least load so far (the
// lower peer on a tie): Graham's longest-processing-time rule. With room to
// spare everywhere its busiest peer is within 4/3 - 1/(3 * peers) of the
// best possible. owner is written only when every expert finds room.
XmPlace :: struct {status:XmStatus;load:XmLoad;lacking:int;}
xm_place :: (counts:[]int,slots:[]int,owner:[]int)->XmPlace {
    E:=counts.count;P:=slots.count;
    if E<1 || E>M4_EXPERTS_MAX || P<1 || P>XM_PEERS_MAX || owner.count!=E {return XmPlace.{status=XmStatus.input};}
    room:=0;
    for p:0..<P {if slots[p]<0 {return XmPlace.{status=XmStatus.input};}room+=slots[p];}
    for e:0..<E {if counts[e]<0 {return XmPlace.{status=XmStatus.input};}}
    // lacking: how many more expert slots the peers need between them.
    if room<E {return XmPlace.{status=XmStatus.memory,lacking=E-room};}
    placed:[256]bool;held:[8]int;load:[8]int;
    for n:0..<E {
        e:=-1;
        for i:0..<E {if !placed[i] && (e<0 || counts[i]>counts[e]) {e=i;}}
        p:=-1;
        for q:0..<P {if held[q]<slots[q] && (p<0 || load[q]<load[p]) {p=q;}}
        owner[e]=p;placed[e]=true;held[p]+=1;load[p]+=counts[e];
    }
    return XmPlace.{load=xm_load(counts,owner,P)};
}
```

On the example, with room for four experts on each peer: expert 0 goes
to peer 0 (load 5), expert 1 to peer 1 (3), expert 2 to peer 1 (5),
expert 4 to peer 0 on the tie (6), expert 6 to peer 1 (6), and the four
unused experts fill the remaining room. Both peers serve 6 uses, and the
imbalance is 1.000. If peer 0 had room for only one expert, it would take
expert 0 and peer 1 the other seven, for loads 5 and 7 and imbalance
1.166. If the peers had room for seven experts between them, the
placement refuses with `memory` and says one more slot is needed.

## Dispatch, compute, combine

A layer across peers runs in three phases. Each token's hidden row lives
on its home peer. The home normalizes and routes the token with chapter
85's `m4_route`. **Dispatch**: the token's normed row goes once to every
other peer that holds one of its chosen experts. **Compute**: each peer
runs its experts with chapter 85's `m4_swiglu` on the rows it received.
**Combine**: each expert's output returns to the home, which sums the
outputs in route-slot order, adds the shared expert and the residual.

The combine order is the point. Chapter 85's `m4_moe` sums a token's
routed outputs in binary64 in slot order and narrows once. Each expert's
output is computed from the same normed row with the same code, so it has
the same bits wherever it runs. The home sums the same values in the same
order. The result is therefore the one-machine result, bit for bit, for
any placement. A peer that summed its own experts first and sent one
row back would send fewer bytes but change the order of the additions,
and with it the last bits.

<!-- source: psionic/lib/expert-mesh.coax run -->
```jai
// One peer's store: the routed experts it holds, by id, each its gate_up
// codes (gate rows, then up rows), its down codes and their two scales.
XmExpert :: struct {id:int;up:[]u8;up_scale:f32;down:[]u8;down_scale:f32;}
XmPeer :: struct {experts:[]XmExpert;}
// Expert e of a whole layer's weights, as a peer would hold it.
xm_expert :: (s:M4Shape,w:M4Moe,e:int)->XmExpert {
    n:=s.expert_mlp*s.hidden;
    return XmExpert.{id=e,up=w.up[e*2*n..<(e+1)*2*n],up_scale=w.up_scale[e],down=w.down[e*n..<(e+1)*n],down_scale=w.down_scale[e]};
}
// Where expert id sits in a peer's store, or -1.
xm_find :: (p:XmPeer,id:int)->int {
    for i:0..<p.experts.count {if p.experts[i].id==id {return i;}}
    return -1;
}
// Work: every token's normed row, every (token, slot)'s returned output,
// one output row, one expert's inner values and each peer's received copy
// of every row. fwork is chapter 85's: the sums and one score an expert.
xm_work :: (s:M4Shape,tokens:int,peers:int)->int {
    return tokens*s.hidden*(1+peers+s.top_k)+s.hidden+2*rp_max(s.expert_mlp,s.shared_mlp);
}
// The layer across peers. The home holds the block norm, the router and
// the shared expert (w's routed experts are not read); peers[p] holds the
// experts owner[] gives it. Every token is normed and routed and every
// chosen expert found before any expert runs, so a route refusal, an
// expert missing from the peer that should hold it, or an expert whose
// arithmetic overflows leaves hidden unchanged; peer and expert name what
// was missing. Then each peer runs its experts on the rows it received,
// and each home sums its tokens' outputs in slot order, adds the shared
// expert and the residual, chapter 85's order exactly. A refusal while
// summing token t leaves tokens before it written, as m4_moe does.
XmResult :: struct {status:XmStatus;written:int;peer:int=-1;expert:int=-1;bytes:XmBytes;}
xm_moe :: (s:M4Shape,w:M4Moe,peers:[]XmPeer,owner:[]int,tokens:int,homes:[]int,hidden:[]f32,routes:[]M4Route,work:[]f32,fwork:[]f64)->XmResult {
    h:=s.hidden;E:=s.experts;I:=s.expert_mlp;k:=s.top_k;P:=peers.count;
    if !m4_valid(s) || w.norm.count!=h || w.gate.count!=E*h || owner.count!=E || P<1 || P>XM_PEERS_MAX {return XmResult.{status=XmStatus.input};}
    if !m4_matrix_ok(w.shared_gate,s.shared_mlp,h) || !m4_matrix_ok(w.shared_up,s.shared_mlp,h) || !m4_matrix_ok(w.shared_down,h,s.shared_mlp) {return XmResult.{status=XmStatus.input};}
    for e:0..<E {if owner[e]<0 || owner[e]>=P {return XmResult.{status=XmStatus.owner,expert=e};}}
    for p:0..<P {for x:peers[p].experts {
        if x.up.count!=2*I*h || x.down.count!=h*I || !fp8_f32_scale_ok(f32_bits(x.up_scale)) || !fp8_f32_scale_ok(f32_bits(x.down_scale)) {return XmResult.{status=XmStatus.input,peer=p,expert=x.id};}
    }}
    if tokens<1 || hidden.count!=tokens*h || routes.count<tokens || homes.count<tokens || work.count<xm_work(s,tokens,P) || fwork.count<m4_moe_fwork(s) {return XmResult.{status=XmStatus.capacity};}
    n:=tokens*h;wide:=2*rp_max(I,s.shared_mlp);
    a:=n*(1+k);b:=a+h;c:=b+wide;
    normed:=work[0..<n];outs:=work[n..<a];out:=work[a..<b];inner:=work[b..<c];inbox:=work[c..<work.count];sums:=fwork[0..<h];
    // Route every token; find every chosen expert where owner says it is.
    for t:0..<tokens {
        if !m4_norm(hidden[t*h..<(t+1)*h],w.norm,normed[t*h..<(t+1)*h]) {return XmResult.{status=XmStatus.arithmetic};}
        r:=m4_route(w.gate,normed[t*h..<(t+1)*h],k,fwork[h..<h+E]);routes[t]=r;
        if r.status!=M4Status.ok {return XmResult.{status=XmStatus.route};}
        for slot:0..<k {e:=r.ids[slot];if xm_find(peers[owner[e]],e)<0 {return XmResult.{status=XmStatus.missing,peer=owner[e],expert=e};}}
    }
    bytes:=xm_bytes(routes,tokens,homes,owner,P,h);
    if bytes.status!=XmStatus.ok {return XmResult.{status=bytes.status};}
    // Dispatch: a copy of the row in each peer's inbox that will use it.
    for t:0..<tokens {for slot:0..<k {p:=owner[routes[t].ids[slot]];
        for i:0..<h {inbox[(t*P+p)*h+i]=normed[t*h+i];}}}
    // Each peer runs its experts on the rows it received.
    for p:0..<P {for t:0..<tokens {for slot:0..<k {
        e:=routes[t].ids[slot];
        if owner[e]!=p {continue;}
        x:=peers[p].experts[xm_find(peers[p],e)];
        gate:=M4Matrix.{codes=x.up[0..<I*h],scale=x.up_scale};up:=M4Matrix.{codes=x.up[I*h..<2*I*h],scale=x.up_scale};
        down:=M4Matrix.{codes=x.down,scale=x.down_scale};
        if !m4_swiglu(gate,up,down,inbox[(t*P+p)*h..<(t*P+p+1)*h],inner[0..<2*I],outs[(t*k+slot)*h..<(t*k+slot+1)*h]) {return XmResult.{status=XmStatus.arithmetic,peer=p,expert=e};}
    }}}
    // Combine at each home, in slot order, then the shared expert.
    for t:0..<tokens {
        r:=routes[t];
        for i:0..<h {sums[i]=0;}
        for slot:0..<k {for i:0..<h {sums[i]+=f64(r.weights[slot])*f64(outs[(t*k+slot)*h+i]);}}
        if !m4_swiglu(w.shared_gate,w.shared_up,w.shared_down,normed[t*h..<(t+1)*h],inner[0..<2*s.shared_mlp],out) {return XmResult.{status=XmStatus.arithmetic,written=t,bytes=bytes};}
        for i:0..<h {
            v:=sums[i]+f64(out[i]);
            if !psionic_ops_fits_f32(v) {return XmResult.{status=XmStatus.arithmetic,written=t,bytes=bytes};}
            out[i]=f32(v);
        }
        if !m4_residual(hidden[t*h..<(t+1)*h],out) {return XmResult.{status=XmStatus.arithmetic,written=t,bytes=bytes};}
    }
    return XmResult.{written=tokens,bytes=bytes};
}
```

Before any expert runs, `xm_moe` routes every token and looks up every
chosen expert on the peer that `owner` names. An expert that peer does
not hold is a `missing` refusal that names the peer and the expert, with
nothing written. A peer never substitutes another expert.

## Counting the bytes

`xm_bytes` counts what one layer's exchange moves between each ordered
pair of peers. A row that goes to another peer carries `hidden` f32
values, a 4-byte token index, and a 1-byte expert id and 1-byte slot for
each of its experts on that peer. Each remote expert's output returns as
`hidden` f32 values. An expert on the token's home moves nothing.

<!-- source: psionic/lib/expert-mesh.coax bytes -->
```jai
// The bytes one layer's expert exchange moves, for tokens whose rows live
// on peer homes[t]. Dispatch: the token's normed row (hidden f32 values)
// goes once to every other peer that holds one of its chosen experts, with
// a 4-byte token index and, for each of its experts there, a 1-byte expert
// id and a 1-byte route slot. Return: each such expert's output, hidden
// f32 values. Nothing moves for an expert on the token's home.
// send[a][b] and back[a][b] are the bytes from peer a to peer b.
XmBytes :: struct {status:XmStatus;send:[8][8]int;back:[8][8]int;dispatched:int;returned:int;total:int;}
XM_INDEX :: 4;XM_TAG :: 2;
xm_bytes :: (routes:[]M4Route,tokens:int,homes:[]int,owner:[]int,peers:int,hidden:int)->XmBytes {
    b:XmBytes;
    if peers<1 || peers>XM_PEERS_MAX || hidden<1 || tokens<0 || tokens>routes.count || tokens>homes.count {b.status=XmStatus.input;return b;}
    row:=4*hidden;
    for t:0..<tokens {
        r:=routes[t];a:=homes[t];
        if r.status!=M4Status.ok {b.status=XmStatus.route;return b;}
        if a<0 || a>=peers {b.status=XmStatus.owner;return b;}
        sent:[8]bool;
        for slot:0..<r.count {
            e:=r.ids[slot];
            if e<0 || e>=owner.count {b.status=XmStatus.route;return b;}
            p:=owner[e];
            if p<0 || p>=peers {b.status=XmStatus.owner;return b;}
            if p==a {continue;}
            if !sent[p] {sent[p]=true;b.send[a][p]+=row+XM_INDEX;b.dispatched+=1;}
            b.send[a][p]+=XM_TAG;b.back[p][a]+=row;b.returned+=1;
        }
    }
    for a:0..<peers {for p:0..<peers {b.total+=b.send[a][p]+b.back[a][p];}}
    return b;
}
```

A worked example with hidden width 4 (16-byte rows), experts 0 and 1 on
peer 0 and experts 2 and 3 on peer 1:

| Token | Home | Experts | Sent | Returned |
| --- | --- | --- | --- | --- |
| 0 | 0 | 1, 2 | 16 + 4 + 2 = 22 to peer 1 | 16 from peer 1 |
| 1 | 1 | 0, 3 | 22 to peer 0 | 16 from peer 0 |
| 2 | 0 | 3, 2 | 16 + 4 + 2 + 2 = 24 to peer 1 | 32 from peer 1 |

Peer 0 sends 46 bytes to peer 1 and receives 48 back; peer 1 sends 22
and receives 16. In all, 3 rows go out, 4 outputs come back, and 132
bytes move.

When every token needs every peer, dispatch is an all-gather of the token
rows: each of the P peers receives the P − 1 rows it lacks. That is
chapter 133's `sd_cost` for moving a row-sharded tensor to a replicated
one, at 4 bytes a value instead of 8, and a check holds the two equal.

## The time an exchange takes

`xm_link` prices one link the way chapter 189's `plc_send` does: the
latency plus the bytes over the bandwidth, rounded up to a nanosecond.
`xm_exchange` assumes every ordered pair of peers has its own link, so
the dispatch phase lasts as long as its slowest link, and so does the
return.

<!-- source: psionic/lib/expert-mesh.coax time -->
```jai
// Nanoseconds to move b bytes over a link of one-way latency lat ns and
// bw bytes a microsecond (MB/s): lat + ceil(b * 1000 / bw), chapter 189's
// plc_send. Nothing to move takes no time.
xm_link :: (lat:int,bw:int,b:int)->int {
    if b==0 {return 0;}
    return lat+(b*1000+bw-1)/bw;
}
// One layer's exchange when every ordered pair of peers has its own link:
// the dispatch phase lasts as long as its slowest link, then the experts
// run, then the return phase lasts as long as its slowest link. -1 when a
// link that carries bytes has no bandwidth.
xm_exchange :: (b:XmBytes,peers:int,lat:[8][8]int,bw:[8][8]int)->int {
    out:=0;back:=0;
    for a:0..<peers {for p:0..<peers {
        if (b.send[a][p]>0 || b.back[a][p]>0) && bw[a][p]<1 {return -1;}
        out=rp_max(out,xm_link(lat[a][p],bw[a][p],b.send[a][p]));
        back=rp_max(back,xm_link(lat[a][p],bw[a][p],b.back[a][p]));
    }}
    return out+back;
}
```

In the worked example, with 0.5 ms of latency and 125 MB/s each way, the
dispatch waits on peer 0's 46 bytes (500,000 + 368 ns) and the return on
peer 1's 48 (500,000 + 384 ns): 1,000,752 ns. Latency dominates small
messages. For Mistral Small 4 a remote expert's row is 16,384 bytes, and
on a link of 50 µs and 1,250 MB/s one row takes 63,112 ns to send.

## Experts from disk

When the routed experts do not fit in memory, a fixed number of expert
slots holds some of them, and the rest are read from disk when a token
chooses them. Whether this is usable depends on how often a token's
experts are already resident. That depends on the routing, so the chapter
measures it on routing traces.

The traces come from chapter 85's `m4_route` on a synthetic router: one
gate matrix per layer and inputs from a stated distribution. Each prompt
wanders among four topics, and a token's input is its topic's center plus
noise. Two prompt numbers share a router and a distribution, so a profile
recorded on one prompt can be used to serve another.

<!-- source: psionic/lib/expert-stream.coax trace -->
```jai
// The synthetic router and its inputs. Width 16; every value comes from
// one generator, xs_unit, uniform on [-1, 1) in steps of 1/1000. The model
// seed fixes each layer's gate rows and XS_TOPICS topic centers; the
// prompt number then shifts the generator, so two prompts share a router
// and a distribution but not their tokens. A prompt starts on topic 0 and
// before each token moves, with probability 1/8, to a topic the generator
// picks (it may pick the same one). A token's input is its topic's center
// plus noise uniform on [-1/2, 1/2) in each value. Every layer routes the
// same input with m4_route and its own gate; the trace records key
// layer * experts + expert for each slot.
XS_WIDTH :: 16;XS_TOPICS :: 4;
xs_next :: (x:int)->int {return (x*48271+11)%2147483647;}
xs_unit :: (x:int)->f32 {return f32(x%2000-1000)/1000;}
XsTrace :: struct {status:XsStatus;tokens:int;layers:int;experts:int;top_k:int;accesses:int;}
// seed and prompt from 0 to 1,000,000; gate holds layers * experts *
// XS_WIDTH values, fwork experts.
xs_trace :: (seed:int,prompt:int,tokens:int,layers:int,experts:int,k:int,keys:[]int,gate:[]f32,fwork:[]f64)->XsTrace {
    t:=XsTrace.{tokens=tokens,layers=layers,experts=experts,top_k=k,accesses=tokens*layers*k};
    if seed<0 || seed>1000000 || prompt<0 || prompt>1000000 || tokens<1 || layers<1 || experts<2 || experts>M4_EXPERTS_MAX || k<1 || k>M4_TOP_MAX || k>=experts {t.status=XsStatus.input;return t;}
    if keys.count<t.accesses || gate.count<layers*experts*XS_WIDTH || fwork.count<experts {t.status=XsStatus.capacity;return t;}
    x:=seed+1;
    for i:0..<layers*experts*XS_WIDTH {x=xs_next(x);gate[i]=xs_unit(x);}
    centers:[64]f32;
    for i:0..<XS_TOPICS*XS_WIDTH {x=xs_next(x);centers[i]=xs_unit(x);}
    topic:=0;input:[16]f32;n:=0;x=xs_next(x+prompt);
    for token:0..<tokens {
        x=xs_next(x);
        if x%8==0 {x=xs_next(x);topic=x%XS_TOPICS;}
        for i:0..<XS_WIDTH {x=xs_next(x);input[i]=centers[topic*XS_WIDTH+i]+xs_unit(x)/2;}
        for l:0..<layers {
            r:=m4_route(gate[l*experts*XS_WIDTH..<(l+1)*experts*XS_WIDTH],input[..],k,fwork[0..<experts]);
            for slot:0..<k {keys[n]=l*experts+r.ids[slot];n+=1;}
        }
    }
    return t;
}
```

A check regenerates every input from this statement and ranks the
experts with chapter 85's binary64 oracle, which shares no code with the
router. The trace must choose the same experts.

`xs_replay` replays a trace through a cache of `capacity` slots. A key is
a layer and an expert. Three policies decide which resident to evict:
least recently used, least frequently used, and chapter 96's decayed
heat, which replaces the coldest resident only when the incoming expert
is more than 1.25 times as hot plus one use, and otherwise reads the
expert without caching it. Heat can start from a recorded profile, as in
chapter 96.

<!-- source: psionic/lib/expert-stream.coax cache -->
```jai
// One replay. Accesses before `warm` fill the cache but are not counted.
// A miss reads the expert from disk; it is then cached (an empty slot
// first, the lowest one) or, under heat, passed through when it is not hot
// enough to replace the coldest resident.
//   lru:  evict the resident used longest ago, counted in tokens (the
//         lowest slot on a tie).
//   lfu:  evict the resident with the fewest uses so far, then the one used
//         longest ago, then the lowest slot.
//   heat: chapter 96's rule: 1,024 heat a use (profile[key] uses to start,
//         when a profile is given), every 256 tokens all heat times 9/10
//         rounded down; replace the coldest resident (the lowest slot on a
//         tie) only when 4 * incoming > 5 * coldest + 4,096.
XS_HEAT :: 1024;
XsCache :: struct {status:XsStatus;accesses:int;hits:int;misses:int;passed:int;evicted:int;}
xs_work :: (capacity:int,keys:int)->int {return capacity+3*keys;}
xs_replay :: (policy:XsPolicy,trace:[]int,per_token:int,warm:int,capacity:int,keys:int,profile:[]int,work:[]int)->XsCache {
    c:XsCache;
    if per_token<1 || warm<0 || warm>trace.count || capacity<1 || keys<1 || keys>XS_KEYS_MAX || (profile.count!=0 && profile.count!=keys) {c.status=XsStatus.input;return c;}
    if work.count<xs_work(capacity,keys) {c.status=XsStatus.capacity;return c;}
    for key:trace {if key<0 || key>=keys {c.status=XsStatus.key;return c;}}
    for u:profile {if u<0 || u>1000000 {c.status=XsStatus.input;return c;}}
    a:=capacity+keys;b:=a+keys;
    slot:=work[0..<capacity];where:=work[capacity..<a];last:=work[a..<b];score:=work[b..<work.count];
    for i:0..<capacity {slot[i]=-1;}
    // A key's last use is written before it can be resident, so it is not cleared.
    for key:0..<keys {where[key]=-1;score[key]=0;}
    if policy==XsPolicy.heat {for key:0..<profile.count {score[key]=XS_HEAT*profile[key];}}
    decay:=256*per_token;
    for n:0..<trace.count {
        key:=trace[n];counted:=n>=warm;token:=n/per_token;
        if policy==XsPolicy.heat && n==decay {for k:0..<keys {score[k]=score[k]*9/10;}decay+=256*per_token;}
        // Every policy counts uses in heat units; only heat decays them.
        score[key]+=XS_HEAT;
        if counted {c.accesses+=1;}
        if where[key]>=0 {last[key]=token;if counted {c.hits+=1;}continue;}
        if counted {c.misses+=1;}
        // The victim: the lowest empty slot, else the policy's choice.
        v:=0;
        for i:0..<capacity {
            if slot[i]<0 {v=i;break;}
            x:=slot[i];y:=slot[v];
            if policy==XsPolicy.lru {if last[x]<last[y] {v=i;}}
            else if score[x]<score[y] || (policy==XsPolicy.lfu && score[x]==score[y] && last[x]<last[y]) {v=i;}
        }
        last[key]=token;
        if slot[v]>=0 {
            if policy==XsPolicy.heat && 4*score[key]<=5*score[slot[v]]+4*XS_HEAT {if counted {c.passed+=1;}continue;}
            where[slot[v]]=-1;if counted {c.evicted+=1;}
        }
        slot[v]=key;where[key]=v;
    }
    return c;
}
```

A worked example with two slots and keys 0 to 3, trace 0 1 0 2 0 1 3 0:

| Access | 0 | 1 | 0 | 2 | 0 | 1 | 3 | 0 | Hits |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lru | miss | miss | hit | miss, evicts 1 | hit | miss, evicts 2 | miss, evicts 0 | miss, evicts 1 | 2 |
| lfu | miss | miss | hit | miss, evicts 1 | hit | miss, evicts 2 | miss, evicts 1 | hit | 3 |
| heat | miss | miss | hit | miss, passes | hit | hit | miss, passes | hit | 4 |

Under heat, expert 2 arrives with 1,024 heat against expert 1's 1,024:
4 × 1,024 = 4,096 is not above 5 × 1,024 + 4,096 = 9,216, so 2 is read
and not cached, and 1 is still resident when it is used again. Four hits
is Belady's optimum for this trace: the best any cache of two slots can
do when it knows the future.

## What a miss costs

`xs_cost` turns misses into time. Each miss reads one expert from disk at
a measured rate. Each token also reads its resident weights from memory.
Two times are reported: serial, the sum, and overlapped, the larger of the
two, which is the bound if every read were prefetched while the previous
layer computes.

<!-- source: psionic/lib/expert-stream.coax cost -->
```jai
// What a replay costs a model: every miss reads one expert's bytes at the
// disk's measured rate, and every token reads its resident weights at the
// memory's measured rate (rates in bytes a microsecond, MB/s). serial adds
// the two; overlap is the bound if every read were prefetched under the
// computation, the larger of the two. Tokens a second times 1,000, rounded
// down, for each.
XsCost :: struct {status:XsStatus;read_bytes:int;read_us:int;compute_us:int;serial_us:int;overlap_us:int;serial_milli:int;overlap_milli:int;}
xs_us :: (bytes:int,rate:int)->int {return (bytes+rate-1)/rate;}
xs_cost :: (misses:int,tokens:int,expert_bytes:int,disk_rate:int,token_bytes:int,memory_rate:int)->XsCost {
    if misses<0 || tokens<1 || expert_bytes<1 || disk_rate<1 || token_bytes<1 || memory_rate<1 {return XsCost.{status=XsStatus.input};}
    c:=XsCost.{read_bytes=misses*expert_bytes};
    c.read_us=xs_us(c.read_bytes,disk_rate);c.compute_us=xs_us(tokens*token_bytes,memory_rate);
    c.serial_us=c.read_us+c.compute_us;c.overlap_us=rp_max(c.read_us,c.compute_us);
    c.serial_milli=tokens*1000000000/c.serial_us;c.overlap_milli=tokens*1000000000/c.overlap_us;
    return c;
}
```

The rates are measured on coderos-4080: 984 bytes a microsecond for a
cold read through a mapping ([chapter 178](ch178-start-fast.md)'s
receipt) and 23,361 for one thread reading memory
([chapter 33](ch033-measure-the-work-and-plan-the-memory.md)'s roofline
receipt). A Mistral Small 4 expert is 25,165,824 FP8 bytes plus four BF16
scales: 25,165,832 bytes. A token reads 6,651,117,568 resident bytes:
chapter 85's `m4_active` values, with the BF16 router and readout at two
bytes each. Ten misses over two tokens cost:

- read: 10 × 25,165,832 = 251,658,320 bytes at 984 B/µs = 255,751 µs,
  rounded up;
- compute: 2 × 6,651,117,568 = 13,302,235,136 bytes at 23,361 B/µs =
  569,421 µs;
- serial: 825,172 µs, or 2.423 tokens a second; overlapped: 569,421 µs, or
  3.512 tokens a second.

If every expert came from disk, a token would read 144 experts,
3,623,879,808 bytes, which takes 3.68 s at this disk's rate.

## Running it

The first example, [`expert-mesh.coax`](../examples/expert-mesh.coax), runs the checks' small layer on three peers and
compares it with chapter 85's layer. Then it routes 256 tokens of Mistral
Small 4's shape through the synthetic router and places each layer's 128
experts on four peers of 32: contiguous, and balanced on the first 128
tokens' counts. It measures both placements on the next 128 tokens and
counts a decode token's exchange bytes.

<!-- source: psionic/examples/expert-mesh.coax example -->
```jai
MESH_TOKENS :: 256;MESH_LAYERS :: 36;MESH_EXPERTS :: 128;MESH_K :: 4;MESH_PEERS :: 4;
main :: () {
    // 1. The checks' small layer (5 experts, 2 a token) on three peers.
    s:=m4o_shape();a:=m4o_moe_weights(1);w:=m4_moe_view(a.f[..],a.c[..]);
    hidden:[56]f32;m4o_values(6,hidden[..],0);one:=hidden;spread:=hidden;
    routes:[7]M4Route;mw:[22]f32;fwork:[13]f64;fine:=m4_moe(s,w,7,one[..],routes[..],mw[..],fwork[..]).written==7;
    owner:[5]int;fine=fine && xm_contiguous(5,3,owner[..])==XmStatus.ok;
    store:[15]XmExpert;peers:[3]XmPeer;
    for p:0..<3 {n:=0;for e:0..<5 {if owner[e]==p {store[p*5+n]=xm_expert(s,w,e);n+=1;}}peers[p]=XmPeer.{experts=store[p*5..<p*5+n]};}
    homes:[7]int;got:[7]M4Route;work:[7*8*6+14]f32;
    r:=xm_moe(s,w,peers[..],owner[..],7,homes[..],spread[..],got[..],work[..],fwork[..]);
    same:=true;for i:0..<56 {if f32_bits(one[i])!=f32_bits(spread[i]) {same=false;}}
    print("small layer, 7 tokens on peer 0, experts 0-1 | 2-3 | 4 on 3 peers: %, % rows sent, % outputs returned, % bytes\n",xm_status_name(r.status),r.bytes.dispatched,r.bytes.returned,r.bytes.total);
    print("  equal to chapter 85's layer on one machine, bit for bit: %\n",same);

    // 2. Mistral Small 4's 36 mixture layers on four peers, routed by the
    // synthetic router: profile on the first 128 tokens, measure on the rest.
    keys:[MESH_TOKENS*MESH_LAYERS*MESH_K]int;gate:[MESH_LAYERS*MESH_EXPERTS*XS_WIDTH]f32;f:[MESH_EXPERTS]f64;
    t:=xs_trace(1,0,MESH_TOKENS,MESH_LAYERS,MESH_EXPERTS,MESH_K,keys[..],gate[..],f[..]);
    half:=MESH_TOKENS/2;slots:=[4]int.[32,32,32,32];
    contiguous:[MESH_EXPERTS]int;fine=fine && t.status==XsStatus.ok && xm_contiguous(MESH_EXPERTS,MESH_PEERS,contiguous[..])==XmStatus.ok;
    sum_c:=0;sum_b:=0;worst_c:=0;worst_b:=0;bytes:=0;dispatched:=0;ns:=0;
    lat:[8][8]int;bw:[8][8]int;for i:0..<8 {for j:0..<8 {lat[i][j]=50000;bw[i][j]=1250;}}
    for l:0..<MESH_LAYERS {
        // Routes of this layer for every token.
        lr:[MESH_TOKENS]M4Route;
        for tok:0..<MESH_TOKENS {lr[tok].count=MESH_K;for k:0..<MESH_K {lr[tok].ids[k]=keys[(tok*MESH_LAYERS+l)*MESH_K+k]-l*MESH_EXPERTS;}}
        profile:[MESH_EXPERTS]int;fine=fine && xm_count(lr[0..<half],half,profile[..])==XmStatus.ok;
        balanced:[MESH_EXPERTS]int;fine=fine && xm_place(profile[..],slots[..],balanced[..]).status==XmStatus.ok;
        later:[MESH_EXPERTS]int;fine=fine && xm_count(lr[half..<MESH_TOKENS],half,later[..])==XmStatus.ok;
        ic:=xm_imbalance(xm_load(later[..],contiguous[..],MESH_PEERS));ib:=xm_imbalance(xm_load(later[..],balanced[..],MESH_PEERS));
        sum_c+=ic;sum_b+=ib;worst_c=rp_max(worst_c,ic);worst_b=rp_max(worst_b,ib);
        // One decode token at a time, its row on peer 0.
        zero:[1]int;
        for tok:half..<MESH_TOKENS {
            b:=xm_bytes(lr[tok..<tok+1],1,zero[..],balanced[..],MESH_PEERS,4096);
            bytes+=b.total;dispatched+=b.dispatched;ns+=xm_exchange(b,MESH_PEERS,lat,bw);
        }
    }
    print("Mistral Small 4, % layers of % experts, top %, on % peers of 32 experts a layer: % traced tokens\n",MESH_LAYERS,MESH_EXPERTS,MESH_K,MESH_PEERS,t.tokens);
    print("  busiest peer over the mean, tokens 128-255: contiguous ");mesh_ratio(sum_c/MESH_LAYERS);print(" (worst layer ");mesh_ratio(worst_c);
    print("), balanced on tokens 0-127 ");mesh_ratio(sum_b/MESH_LAYERS);print(" (worst layer ");mesh_ratio(worst_b);print(")\n");
    print("  a decode token on peer 0: % bytes over % layers, % rows sent to other peers\n",bytes/half,MESH_LAYERS,dispatched/half);
    print("  on links of 50 us and 1,250 MB/s (assumed, not measured): % us of expert exchange a token\n",(ns/half+999)/1000);
    print("  every step ok: %\n",fine);
    print("  each peer holds 32 x 36 experts: % bytes\n",32*36*(m4_weights(m4_small4()).expert+8));
}
```

```text
small layer, 7 tokens on peer 0, experts 0-1 | 2-3 | 4 on 3 peers: ok, 8 rows sent, 13 outputs returned, 730 bytes
  equal to chapter 85's layer on one machine, bit for bit: true
Mistral Small 4, 36 layers of 128 experts, top 4, on 4 peers of 32 experts a layer: 256 traced tokens
  busiest peer over the mean, tokens 128-255: contiguous 1.375 (worst layer 1.984), balanced on tokens 0-127 1.257 (worst layer 1.570)
  a decode token on peer 0: 2954750 bytes over 36 layers, 75 rows sent to other peers
  on links of 50 us and 1,250 MB/s (assumed, not measured): 4903 us of expert exchange a token
  every step ok: true
  each peer holds 32 x 36 experts: 28991038464 bytes
```

The balanced placement lowers the average imbalance from 1.375 to 1.257
and the worst layer's from 1.984 to 1.570, measured on tokens it did not
see. It cannot reach 1.000: a peer holds whole experts, and the counts on new
tokens differ from the profile's. A decode token sends about two rows a layer to
other peers. The link figures are assumptions, not measurements; with
them, expert exchange costs about 4.9 ms a token, 36 round trips of
latency.

The second example, [`expert-stream.coax`](../examples/expert-stream.coax), replays 384 tokens through caches of 8, 16 and 32 GiB,
counting the last 256.

<!-- source: psionic/examples/expert-stream.coax example -->
```jai
// Mistral Small 4 with its routed experts on disk: 36 layers of 128, top 4.
// Rates measured on coderos-4080: reading a file through a mapping from
// disk, 984 bytes a microsecond (chapter 178's receipt); one thread reading
// memory, 23,361 (chapter 33's roofline receipt).
ST_TOKENS :: 384;ST_WARM :: 128;ST_LAYERS :: 36;ST_EXPERTS :: 128;ST_K :: 4;
ST_DISK :: 984;ST_MEMORY :: 23361;
main :: () {
    s:=m4_small4();
    expert:=m4_weights(s).expert+8;
    // Resident bytes a token reads: m4_active's values, the BF16 router and readout at two bytes.
    token:=m4_active(s)+s.layers*s.experts*s.hidden+s.vocab*s.hidden;
    keys:=ST_LAYERS*ST_EXPERTS;per:=ST_LAYERS*ST_K;
    print("expert % bytes, % keys, % expert reads a token, % resident bytes a token\n",expert,keys,per,token);
    trace:[ST_TOKENS*ST_LAYERS*ST_K]int;recorded:[ST_TOKENS*ST_LAYERS*ST_K]int;gate:[ST_LAYERS*ST_EXPERTS*XS_WIDTH]f32;f:[ST_EXPERTS]f64;
    a:=xs_trace(1,2,ST_TOKENS,ST_LAYERS,ST_EXPERTS,ST_K,trace[..],gate[..],f[..]);
    // A profile recorded on another prompt through the same router.
    b:=xs_trace(1,1,ST_TOKENS,ST_LAYERS,ST_EXPERTS,ST_K,recorded[..],gate[..],f[..]);
    profile:[ST_LAYERS*ST_EXPERTS]int;for k:recorded {profile[k]+=1;}
    used:[ST_LAYERS*ST_EXPERTS]bool;distinct:=0;for n:ST_WARM*per..<trace.count {if !used[trace[n]] {used[trace[n]]=true;distinct+=1;}}
    print("traces %, %: tokens %-% measured after % warm, % distinct experts used\n",a.status,b.status,ST_WARM,ST_TOKENS-1,ST_WARM,distinct);
    none:[1]int;work:[1400+3*ST_LAYERS*ST_EXPERTS]int;
    budgets:=[3]int.[8,16,32];
    for g:budgets {
        slots:=g*1073741824/expert;
        print("cache % GiB = % experts, % percent of %:\n",g,slots,slots*100/keys,keys);
        for which:0..<4 {
            policy:=ifx which==3 then XsPolicy.heat else cast(XsPolicy)which;
            p:=ifx which==3 then profile[..] else none[0..<0];
            c:=xs_replay(policy,trace[..],per,ST_WARM*per,slots,keys,p,work[..]);
            tokens:=ST_TOKENS-ST_WARM;cost:=xs_cost(c.misses,tokens,expert,ST_DISK,token,ST_MEMORY);
            label:=ifx which==3 then "heat, profiled" else xs_policy_name(policy);
            print("  %: hits % of %, % reads a token, % MB read a token, tokens/s ",label,c.hits,c.accesses,c.misses/tokens,cost.read_bytes/tokens/1000000);
            stream_milli(cost.serial_milli);print(" serial, ");stream_milli(cost.overlap_milli);print(" if reads overlapped\n");
        }
    }
    all:=xs_cost(0,1,expert,ST_DISK,token,ST_MEMORY);
    print("all in memory: tokens/s ");stream_milli(all.serial_milli);print("; every expert from disk: ");
    stream_milli(xs_cost(per,1,expert,ST_DISK,token,ST_MEMORY).serial_milli);print("\n");
}
```

```text
expert 25165832 bytes, 4608 keys, 144 expert reads a token, 6651117568 resident bytes a token
traces ok, ok: tokens 128-383 measured after 128 warm, 2861 distinct experts used
cache 8 GiB = 341 experts, 7 percent of 4608:
  lru: hits 22661 of 36864, 55 reads a token, 1396 MB read a token, tokens/s 0.586 serial, 0.704 if reads overlapped
  lfu: hits 15722 of 36864, 82 reads a token, 2078 MB read a token, tokens/s 0.417 serial, 0.473 if reads overlapped
  heat: hits 14621 of 36864, 86 reads a token, 2186 MB read a token, tokens/s 0.398 serial, 0.450 if reads overlapped
  heat, profiled: hits 15517 of 36864, 83 reads a token, 2098 MB read a token, tokens/s 0.413 serial, 0.468 if reads overlapped
cache 16 GiB = 682 experts, 14 percent of 4608:
  lru: hits 27685 of 36864, 35 reads a token, 902 MB read a token, tokens/s 0.832 serial, 1.090 if reads overlapped
  lfu: hits 24102 of 36864, 49 reads a token, 1254 MB read a token, tokens/s 0.641 serial, 0.784 if reads overlapped
  heat: hits 23215 of 36864, 53 reads a token, 1341 MB read a token, tokens/s 0.606 serial, 0.733 if reads overlapped
  heat, profiled: hits 23768 of 36864, 51 reads a token, 1287 MB read a token, tokens/s 0.627 serial, 0.764 if reads overlapped
cache 32 GiB = 1365 experts, 29 percent of 4608:
  lru: hits 31482 of 36864, 21 reads a token, 529 MB read a token, tokens/s 1.215 serial, 1.859 if reads overlapped
  lfu: hits 31965 of 36864, 19 reads a token, 481 MB read a token, tokens/s 1.291 serial, 2.043 if reads overlapped
  heat: hits 31600 of 36864, 20 reads a token, 517 MB read a token, tokens/s 1.233 serial, 1.901 if reads overlapped
  heat, profiled: hits 31998 of 36864, 19 reads a token, 478 MB read a token, tokens/s 1.297 serial, 2.057 if reads overlapped
all in memory: tokens/s 3.512; every expert from disk: 0.252
```

What the numbers show, for this synthetic distribution only:

- **Locality helps a lot.** A cache of 14% of the experts hits 75% of
  reads under LRU, because a prompt stays on a topic for several tokens.
- **No policy wins everywhere.** LRU is best at 8 and 16 GiB. At 32 GiB,
  the other three pass it, profiled heat most. Heat's hysteresis keeps it from chasing
  a topic change, which costs it at small sizes.
- **The disk is the limit.** At 32 GiB, a token still reads about 500 MB
  from disk, and the serial rate is a third of the all-in-memory rate of
  3.512 tokens a second.

Real routing has different locality. The policy to use for Mistral Small 4
must be chosen on its real traces, which this host cannot produce.

## Refusals

[`expert-mesh-refuse.coax`](../examples/expert-mesh-refuse.coax) breaks
one input at a time.

<!-- source: psionic/examples/expert-mesh-refuse.coax example -->
```jai
// Four things the chapter refuses, each with its typed status: an expert
// missing from the peer that should hold it, a placement with too little
// room, a link that carries bytes with no bandwidth, and a trace key
// outside the cache's keys.
main :: () {
    s:=m4o_shape();a:=m4o_moe_weights(2);w:=m4_moe_view(a.f[..],a.c[..]);
    hidden:[56]f32;m4o_values(4,hidden[..],0);h:=hidden;owner:=[5]int.[0,0,1,1,1];
    // Peer 1 should hold experts 2, 3 and 4, but its store lost 3.
    store:=[4]XmExpert.[xm_expert(s,w,0),xm_expert(s,w,1),xm_expert(s,w,2),xm_expert(s,w,4)];
    peers:=[2]XmPeer.[XmPeer.{experts=store[0..<2]},XmPeer.{experts=store[2..<4]}];
    homes:[7]int;routes:[7]M4Route;work:[7*8*5+14]f32;fwork:[13]f64;
    r:=xm_moe(s,w,peers[..],owner[..],7,homes[..],h[..],routes[..],work[..],fwork[..]);
    same:=true;for i:0..<56 {if f32_bits(h[i])!=f32_bits(hidden[i]) {same=false;}}
    print("refused: %, peer % does not hold expert %; % tokens written, hidden unchanged %\n",xm_status_name(r.status),r.peer,r.expert,r.written,same);
    // 128 experts, 4 peers with room for 31 each.
    counts:[128]int;room:=[4]int.[31,31,31,31];placed:[128]int;
    p:=xm_place(counts[..],room[..],placed[..]);
    print("refused: %, the peers need % more expert slots\n",xm_status_name(p.status),p.lacking);
    // One row from peer 0 to peer 1 on a link with no bandwidth.
    b:XmBytes;b.send[0][1]=4100;lat:[8][8]int;bw:[8][8]int;
    print("refused: exchange time % for a link with no bandwidth\n",xm_exchange(b,2,lat,bw));
    // A trace key 4608 when 36 layers of 128 experts have keys 0 to 4607.
    trace:=[3]int.[0,4607,4608];none:[1]int;cache:[2+3*4608]int;
    c:=xs_replay(XsPolicy.lru,trace[..],1,0,2,4608,none[0..<0],cache[..]);
    print("refused: replay status %, % accesses replayed\n",c.status,c.accesses);
}
```

```text
refused: missing, peer 1 does not hold expert 3; 0 tokens written, hidden unchanged true
refused: memory, the peers need 4 more expert slots
refused: exchange time -1 for a link with no bandwidth
refused: replay status key, 0 accesses replayed
```

The peer that should hold expert 3 does not, and the layer refuses before
any expert runs, so no token's hidden row changes. The placement needs
128 slots and has 124. A link that would carry bytes but has no bandwidth
gives no time. A trace key outside the cache's keys refuses the whole
replay before any access is counted.

## Checks

<!-- source: psionic/lib/expert-mesh.coax checks-run -->
```jai
#forall "the layer across peers equals chapter 85's layer on one machine, bit for bit" seed:0..<12,peers:1..<5,part:0..<3 #steps 400000000 {
    s:=m4o_shape();a:=m4o_moe_weights(seed);w:=m4_moe_view(a.f[..],a.c[..]);
    hidden:[56]f32;m4o_values(seed+5,hidden[..],0);
    // Three partitions: random, contiguous, balanced on this prompt's uses.
    owner:[5]int;xmc_owner(seed*3+part,peers,owner[..]);
    local:=hidden;routes:[7]M4Route;mw:[22]f32;fwork:[13]f64;
    #assert m4_moe(s,w,7,local[..],routes[..],mw[..],fwork[..]).written==7;
    if part==1 {#assert xm_contiguous(5,peers,owner[..])==XmStatus.ok;}
    if part==2 {
        counts:[5]int;#assert xm_count(routes[..],7,counts[..])==XmStatus.ok;
        slots:[4]int;for p:0..<peers {slots[p]=5;}
        #assert xm_place(counts[..],slots[0..<peers],owner[..]).status==XmStatus.ok;
    }
    store:[20]XmExpert;held:[4]XmPeer;xmc_peers(s,w,owner[..],store[..],held[0..<peers]);
    homes:[7]int;for t:0..<7 {homes[t]=(t+seed)%peers;}
    spread:=hidden;got:[7]M4Route;work:[406]f32;
    r:=xm_moe(s,w,held[0..<peers],owner[..],7,homes[..],spread[..],got[..],work[..],fwork[..]);
    #assert r.status==XmStatus.ok && r.written==7 && xmc_same(spread[..],local[..]);
    for t:0..<7 {for slot:0..<2 {#assert got[t].ids[slot]==routes[t].ids[slot] && got[t].weights[slot]==routes[t].weights[slot];}}
    c:=xmc_bytes(got[..],7,homes[..],owner[..],peers,8);#assert r.bytes.total==c.send+c.back && r.bytes.dispatched==c.rows;
}
```

The independent checks:

- **Distributed equals local.** Over 12 weight seeds, 1 to 4 peers and
  three partitions (random, contiguous and balanced on the prompt's own
  counts), `xm_moe` equals chapter 85's `m4_moe` bit for bit, with the
  same routes, and its bytes equal a count written per peer.
- **Placement against brute force.** Over every small case of up to 6
  experts and 3 peers, the balanced placement is never better than the
  best of every assignment, which is a sanity bound, and with room to
  spare it is within Graham's bound. With tight room it never puts more
  experts on a peer than it has room for, and it refuses exactly when the
  room is short.
- **Bytes against chapter 133.** When every token needs every peer, the
  rows sent equal `sd_cost`'s all-gather.
- **Cache against two oracles.** LRU replays equal hits computed by stack
  distance, and no policy hits more often than Belady's optimum.

<!-- source: psionic/lib/expert-stream.coax checks-oracle -->
```jai
#forall "replays agree with stack-distance LRU and never beat Belady's optimum" seed:0..<40,keys:1..<7,capacity:1..<5 #steps 400000000 {
    trace:[40]int;xsc_trace(seed,keys,trace[..]);work:[25]int;none:[1]int;warm:=seed%5;
    best:=xsc_min(trace[..],capacity);distinct:=xsc_distinct(trace[..]);
    for policy:[3]XsPolicy.[XsPolicy.lru,XsPolicy.lfu,XsPolicy.heat] {
        c:=xs_replay(policy,trace[..],1+seed%3,0,capacity,keys,none[0..<0],work[..]);
        #assert c.status==XsStatus.ok && c.accesses==40 && c.hits+c.misses==40 && c.hits<=best;
        // Each miss is either cached, evicting a resident once the cache is full, or passed through.
        #assert c.misses==c.passed+c.evicted+rp_min(capacity,c.misses-c.passed-c.evicted) && c.evicted+rp_min(capacity,distinct)==c.misses-c.passed;
        if capacity>=distinct {#assert c.misses==distinct && c.evicted==0;}
        if policy!=XsPolicy.heat {#assert c.passed==0;}
    }
    c:=xs_replay(XsPolicy.lru,trace[..],1,warm,capacity,keys,none[0..<0],work[..]);
    #assert c.hits==xsc_lru(trace[..],capacity,warm) && c.accesses==40-warm;
}
```

```shell
coax/bin/coax test psionic/lib/expert-mesh.coax
coax/bin/coax test psionic/lib/expert-stream.coax
coax/bin/coax mutate psionic/lib/expert-mesh.coax --root-only
coax/bin/coax mutate psionic/lib/expert-stream.coax --root-only
```

Both libraries have no surviving mutants in their own code.

## What remains

- **Real machines.** Peers here are expert stores in one process, and the
  exchange is copies between buffers. Running the dispatch over chapter
  188's transport, and measuring each hop's latency against
  `xm_exchange`'s estimate, needs peers on separate machines.
- **Real weights and traces.** The equality check runs on the checks'
  small synthetic layer. Mistral Small 4's checkpoint (about 120 GB) is
  not on this host, so neither its expert outputs nor its real routing
  traces are here. The placement and cache numbers above are for the
  synthetic router.
- **A real disk.** `xs_cost` uses the measured read rate but no expert is
  read from disk. A harness that streams experts from NVMe and times each
  read is not built.
- **Prefetch.** The overlapped time is a bound. Predicting the next
  layer's experts before its router runs, and measuring how often the
  prediction is right, is not built.

## Dependencies

- [Chapter 61](ch061-a-few-experts-then-a-router.md): the router.
- [Chapter 85](ch085-execute-mistral-small-7.md): Mistral Small 4's router,
  expert arithmetic, mixture layer and weight counts.
- [Chapter 96](ch096-hot-experts-in-fast-memory.md): the decayed-heat
  replacement rule.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md): `sd_first` and
  `sd_cost`.
- [Chapter 178](ch178-start-fast.md) and
  [chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the measured
  disk and memory rates.
- [Chapter 183](ch183-split-a-model-across-devices.md): splitting a model
  inside one machine.
- [Chapter 188](ch188-peers-transport-and-discovery.md) and
  [chapter 189](ch189-place-the-model.md): peers, links and their prices.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Mistral target](../docs/targets/mistral.md)
