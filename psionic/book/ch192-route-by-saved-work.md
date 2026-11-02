# 192. Route by saved work

> **Status: partly implemented.** The saved-work router (profile
> sw-saved-v1) runs on the CPU against a simulation of a mesh of peers in
> one process. Each peer holds chapter 176's prefix trie and advertises it
> as a bounded, short-lived list of salted block hashes; the router refuses
> advertisements for another epoch, replayed, from the future, expired,
> over the bound or over the peer's frames, and with repeated entries. For
> each request it prices reusing claimed blocks in place, moving them from
> another peer over a link, or computing the prompt cold, and takes the
> smallest predicted finish. A claim that turns out to be missing falls
> back to computing, and every request records its predicted and realized
> saving. It is checked by hand-worked prices and routes, a brute-force
> router over every option and block count, an audit of every simulated
> request, and frozen runs against prefix affinity, a cache-blind router
> and round robin. No real peer runs: advertisements over chapter 188's
> links, state moved between real processes, a real model's output,
> recorded traffic and trust between peers are not built; the last section
> lists them.

A mesh can save prefill by sending a request to a peer that already holds
its prefix. On one cluster, [chapter 177](ch177-route-queue-and-scale-replicas.md)'s
router reads each replica's prefix trie directly. Across a mesh it cannot:
a peer only says what it holds, and by the time a request arrives the
blocks it claimed may be gone. This chapter routes by that saved work, and
records whether the saving was real.

It builds on four earlier chapters. [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md)'s
trie is what a peer holds: prompt blocks keyed by the model, the parent
block and the block's exact tokens, each with a chained hash.
[Chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md)'s step
model prices computing a prompt. [Chapter 182](ch182-disaggregate-prefill-and-decode.md)'s
link model prices moving state: a latency, then the bits at the link's
rate, rounded up to a nanosecond. And [chapter 188](ch188-peers-transport-and-discovery.md)
gives peers and the links between them; here they are numbers in a table,
not sockets.

The rules decide where work goes and what the mesh believes about its
peers, so they are pure Coax procedures in an `#authority` file,
[`serve-saved.coax`](../lib/serve-saved.coax), checked with zero surviving
mutants.

## Peers, links and advertisements

A mesh names its peers, their page geometry (the device tier only, as in
chapter 177), and for every pair of peers a one-way latency in nanoseconds
and a rate in bits a nanosecond, which is gigabits a second; a rate of 0
means no link. It also says how many bytes of state one block holds, the
epoch and salt that advertisements use, how often peers advertise, and the
longest an advertisement may live.

<!-- source: psionic/lib/serve-saved.coax types -->
```jai
// The mesh: peers, their page geometry (device tier only, as chapter 177),
// one-way latency in nanoseconds and bandwidth in bits a nanosecond (Gbit/s)
// from peer to peer (0: no link), bytes of state one block holds, the
// advertisement epoch and its salt, how often peers advertise, and how long
// an advertisement may live.
SwMesh :: struct {peers:int;pages:PgConfig;lat:[8][8]int;gbit:[8][8]int;block_bytes:int;epoch:int;salt:int;period:int;ttl:int;}
// An advertisement: who sent it, for which epoch, its sequence number, when
// it was issued and for how long it holds, and n salted block hashes.
SwAd :: struct {peer:int;epoch:int;seq:int;issued:int;ttl:int;n:int;entry:[16]int;}
// Why an advertisement was refused: no such peer, another epoch, not newer
// than the one held, issued in the future, a lifetime out of bounds,
// already expired, more entries than an advertisement carries, more blocks
// than the peer has frames, or an entry listed twice.
SwAdRefusal :: enum u8 {none;peer;epoch;seq;early;ttl;stale;count;over;dup;}
// The router's view: the last advertisement it accepted from each peer.
SwView :: struct {held:[8]bool;ad:[8]SwAd;}
SwAccepted :: struct {refusal:SwAdRefusal;view:SwView;}
// saved: reuse, move or compute, whichever finishes first. reuse: reuse in
// place or compute (chapter 177's prefix affinity, from advertisements).
// blind: the least predicted finish, ignoring caches. round: the next peer.
SwPolicy :: enum u8 {saved;reuse;blind;round;}
SwRequest :: struct {arrive:int;prompt:int;output:int;}
SwPeer :: struct {busy:int;st:PgStore;}
// The router's choice: the peer that runs the request, the peer whose
// blocks it uses (-1: none), how many it counts on, and the predicted
// nanoseconds from now to the last token.
SwPick :: struct {refused:bool;dst:int;src:int;blocks:int;est:int;turn:int;}
```

An advertisement lists hashes, not tokens. Each entry is a block's chained
hash from chapter 176, which covers the model and every token up to the end
of that block, mixed with the epoch's salt:

<!-- source: psionic/lib/serve-saved.coax advertise -->
```jai
// The salt: a block's chained hash (chapter 176) mixed with the epoch's
// salt, so an advertisement lists neither tokens nor the plain hashes an
// outsider could compute from guessed tokens.
sw_mix :: (m:SwMesh,h:int)->int {return (h*SW_MIX+m.salt)%PG_HASH;}
// What a peer holds, newest first (the lower node on a tie), at most
// SW_ADS entries.
sw_advertise :: (st:PgStore,m:SwMesh,peer:int,seq:int,now:int)->SwAd {
    a:=SwAd.{peer=peer,epoch=m.epoch,seq=seq,issued=now,ttl=m.ttl};
    taken:[48]bool;
    while a.n<SW_ADS {
        v:=-1;
        for n:0..<PG_NODES {
            d:=st.node[n];
            if d.frame>=0 && !taken[n] && (v<0 || d.stamp>st.node[v].stamp) {v=n;}
        }
        if v<0 {return a;}
        taken[v]=true;a.entry[a.n]=sw_mix(m,st.node[v].hash);a.n+=1;
    }
    return a;
}
// Accept an advertisement into the view, or refuse it and keep the view.
sw_accept :: (from:SwView,m:SwMesh,a:SwAd,now:int)->SwAccepted {
    g:=SwAccepted.{refusal=sw_refuse(from,m,a,now),view=from};
    if g.refusal==SwAdRefusal.none {g.view.held[a.peer]=true;g.view.ad[a.peer]=a;}
    return g;
}
sw_refuse :: (v:SwView,m:SwMesh,a:SwAd,now:int)->SwAdRefusal {
    if a.peer<0 || a.peer>=m.peers {return SwAdRefusal.peer;}
    if a.epoch!=m.epoch {return SwAdRefusal.epoch;}
    if v.held[a.peer] && a.seq<=v.ad[a.peer].seq {return SwAdRefusal.seq;}
    if a.issued>now {return SwAdRefusal.early;}
    if a.ttl<1 || a.ttl>m.ttl {return SwAdRefusal.ttl;}
    if now>=a.issued+a.ttl {return SwAdRefusal.stale;}
    if a.n<0 || a.n>SW_ADS {return SwAdRefusal.count;}
    if a.n>m.pages.cap[0] {return SwAdRefusal.over;}
    for i:0..<a.n {for j:i+1..<a.n {if a.entry[i]==a.entry[j] {return SwAdRefusal.dup;}}}
    return SwAdRefusal.none;
}
// The blocks peer k claims for a prompt: the longest run of its whole
// blocks, never the one holding the last position, whose salted chained
// hashes are all listed in k's live advertisement.
sw_claim :: (v:SwView,m:SwMesh,k:int,tok:[]int,prompt:int,now:int)->int {
    a:=v.ad[k];
    if !v.held[k] || now>=a.issued+a.ttl {return 0;}
    p:=m.pages.page;h:=0;most:=(prompt-1)/p;
    for b:0..<most {
        h=pg_hash(h,tok[b*p..<b*p+p]);e:=sw_mix(m,h);found:=false;
        for j:0..<a.n {if a.entry[j]==e {found=true;}}
        if !found {return b;}
    }
    return most;
}
```

`sw_advertise` lists what a peer holds, most recently used first, and stops
at 16 entries, so an advertisement has a fixed size however much the peer
caches. The salt keeps the entries from matching hashes that an outsider
computes from guessed tokens, and from matching another epoch's entries.
It is a keyed mix, not a cryptographic MAC: peers in the mesh know the
salt, and an advertisement's authenticity comes from the link it arrives
on, not from its contents.

`sw_accept` is the router's admission rule. It refuses an advertisement
from a peer the mesh does not have, for another epoch, whose sequence
number is not newer than the one it holds (a replay or a reordering), issued
after the moment it arrives, claiming a lifetime outside 1 ns to the mesh's
limit, already expired, carrying more than 16 entries, claiming more blocks
than the peer has frames, or listing an entry twice. A refusal leaves the
view as it was.

`sw_claim` reads a claim from the view. It walks the prompt's whole blocks,
never the one that holds the last position (that position must run to
produce the first token, as in chapter 176), and counts the leading blocks
whose salted chained hashes are listed. Because each hash covers the whole
prefix, a changed early token breaks the chain even when a later block's
own tokens match. An advertisement past its lifetime claims nothing.

## The price of each way

There are three ways to get a prompt's state on the peer that will run it:
reuse blocks it already holds, move blocks from another peer, or compute
them. Each is priced in nanoseconds from now to the last output token:

<!-- source: psionic/lib/serve-saved.coax cost -->
```jai
// Chapter 175's step model, as chapter 177's rr_cost in nanoseconds: the
// prompt's rows after `cached` positions in one step, then one step a token.
sw_cost :: (prompt:int,output:int,cached:int)->int {return cb_step_ns(prompt-cached)+(output-1)*cb_step_ns(1);}
// Chapter 182's link model per block: bits at gbit bits a nanosecond,
// rounded up.
sw_block_ns :: (m:SwMesh,a:int,b:int)->int {return (m.block_bytes*8+m.gbit[a][b]-1)/m.gbit[a][b];}
// Moving `blocks` blocks from peer a to peer b: nothing on one peer;
// otherwise the latency, even for none (a refusal comes back), then each
// block.
sw_link :: (m:SwMesh,a:int,b:int,blocks:int)->int {
    if a==b {return 0;}
    return m.lat[a][b]+blocks*sw_block_ns(m,a,b);
}
```

A request on peer `k` first waits for `k` to be free. Then, for `c` blocks
of `p` positions taken from peer `j`, it pays the link from `j` to `k` (0
when `j` is `k`) and the step model for the rows left to compute. Reuse in
place is the case `j = k`; computing cold is `c = 0`.

Work the hand mesh by hand: three peers, pages of 4 positions, 8 frames
each, every link 2 ms and 10 Gbit/s. A block is 4 positions of gpt-oss-20b's
key and value state: 24 layers, 8 KV heads of 64 values, 2 bytes a value, K
and V, so 2 × 8 × 64 × 2 × 24 × 4 = 196,608 bytes. At 10 bits a nanosecond
that is 1,572,864 / 10 = 157,286.4, so 157,287 ns a block.

Chapter 175's step costs 17,792,472 ns for the first row and 1,459,182 ns
for each further row. A prompt of 13 tokens with 2 output tokens costs,
cold, 17,792,472 + 12 × 1,459,182 = 35,302,656 ns of prefill plus one
decode step of 17,792,472, so 53,095,128 ns. Each cached block of 4
positions removes 4 × 1,459,182 = 5,836,728 ns: with 2 blocks it is
41,421,672 ns, with 3 it is 35,584,944 ns.

**When does moving pay?** Moving `b` blocks over a link costs its latency
plus `b` × 157,287 ns and saves `b` × 5,836,728 ns, so it pays when
`b` × 5,679,441 ns exceeds the latency. Over a 20 ms link, 3 blocks save
17,038,323 ns and lose 2,961,677 ns against computing them; 4 blocks save
22,717,764 ns and win by 2,717,764 ns. The checks hold both numbers:

<!-- source: psionic/lib/serve-saved.coax check-cost -->
```jai
#test "the hand-worked prices and the break-even" {
    // Chapter 175's steps: 13 rows, 17,792,472 + 12 * 1,459,182 ns; one row per further token.
    #assert sw_cost(13,2,0)==53095128 && sw_cost(13,2,8)==41421672 && sw_cost(13,2,12)==35584944;
    // gpt-oss-20b, 4 positions a block: 196,608 bytes, 157,287 ns a block over 10 Gbit/s.
    m:=swx_hand_mesh();
    #assert sw_block_ns(m,0,1)==157287 && sw_link(m,0,1,3)==2471861 && sw_link(m,0,0,3)==0 && sw_link(m,1,2,0)==2000000;
    // Over 20 ms, a block saves 5,836,728 - 157,287 = 5,679,441 ns: 3 blocks lose, 4 win.
    m.lat[0][1]=20000000;
    #assert sw_link(m,0,1,3)+sw_cost(13,2,12)>sw_cost(13,2,0);
    #assert sw_link(m,0,1,4)+sw_cost(17,2,16)<sw_cost(17,2,0);
    #assert sw_cost(17,2,0)-sw_cost(17,2,16)-sw_link(m,0,1,4)==2717764 && sw_cost(13,2,0)-sw_cost(13,2,12)-sw_link(m,0,1,3)== -2961677;
}
```

Because a move's price is a constant plus a straight line in `b`, the best
number of blocks to move from one peer is either none or all it claims:
moving fewer than all only pays the same latency for less saving. The
router therefore prices each source at its full claim, and the
brute-force check below confirms that no smaller count is ever cheaper.

## The router

<!-- source: psionic/lib/serve-saved.coax route -->
```jai
// Refuse a request outside the bounds; otherwise price every peer as the
// one to run it: its wait, then computing cold, reusing its own claimed
// blocks, or (saved work only) moving another peer's claimed blocks over a
// link. The smallest predicted finish wins; on a tie the lower peer, then
// computing cold before reuse, then the lower source. Round robin takes the
// next peer in turn and computes cold.
sw_route :: (peer:[]SwPeer,m:SwMesh,v:SwView,pol:SwPolicy,r:SwRequest,tok:[]int,now:int,turn:int)->SwPick {
    pick:=SwPick.{refused=true,dst=-1,src=-1,turn=turn};
    if r.prompt<1 || r.prompt>PG_PROMPT || r.output<1 || r.output>SW_OUTPUT {return pick;}
    pick.refused=false;
    if pol==SwPolicy.round {
        pick.dst=turn;pick.est=sw_wait(peer,turn,now)+sw_cost(r.prompt,r.output,0);pick.turn=(turn+1)%peer.count;
        return pick;
    }
    c:[8]int;
    if pol!=SwPolicy.blind {for j:0..<peer.count {c[j]=sw_claim(v,m,j,tok,r.prompt,now);}}
    for k:0..<peer.count {
        w:=sw_wait(peer,k,now);
        e:=w+sw_cost(r.prompt,r.output,0);
        if pick.dst<0 || e<pick.est {pick=SwPick.{dst=k,src=-1,est=e,turn=turn};}
        for j:0..<peer.count {
            if c[j]==0 || (j!=k && (pol!=SwPolicy.saved || m.gbit[j][k]==0)) {continue;}
            e=w+sw_link(m,j,k,c[j])+sw_cost(r.prompt,r.output,c[j]*m.pages.page);
            if e<pick.est {pick=SwPick.{dst=k,src=j,blocks=c[j],est=e,turn=turn};}
        }
    }
    return pick;
}
sw_wait :: (peer:[]SwPeer,k:int,now:int)->int {return cb_max(0,peer[k].busy-now);}
```

For every peer that could run the request, the router prices computing
cold, then reusing that peer's own claim, then (under the saved-work
policy) moving each other peer's claim over its link. The smallest
predicted finish wins. Ties go to the lower peer, then to computing cold,
then to the lower source, so the choice is a function of the inputs. Three
other policies use the same prices for comparison: reuse in place (chapter
177's prefix affinity, read from advertisements), cache-blind (the least
predicted finish with every claim ignored, chapter 177's least work), and
round robin.

### A route, traced by hand

Peer 0 holds prompt A's 3 blocks (tokens 1 to 12) and is busy until 30 ms.
Peer 1 is empty and idle. Peer 2 holds two prompts that share A's first 2
blocks and is busy until 3 ms. Every peer's advertisement is in the view,
so the claims for A are 3, 0 and 2 blocks. At time 0:

| Run on | State from | Wait | Link | Compute | Predicted finish |
| --- | --- | --- | --- | --- | --- |
| peer 0 | its own 3 blocks | 30,000,000 | 0 | 35,584,944 | 65,584,944 |
| peer 1 | computed cold | 0 | 0 | 53,095,128 | 53,095,128 |
| peer 1 | 3 blocks from peer 0 | 0 | 2,471,861 | 35,584,944 | **38,056,805** |
| peer 1 | 2 blocks from peer 2 | 0 | 2,314,574 | 41,421,672 | 43,736,246 |
| peer 2 | its own 2 blocks | 3,000,000 | 0 | 41,421,672 | 44,421,672 |
| peer 2 | 3 blocks from peer 0 | 3,000,000 | 2,471,861 | 35,584,944 | 41,056,805 |

The saved-work router moves peer 0's 3 blocks to idle peer 1. Prefix
affinity can only reuse in place, and takes peer 2's 2 blocks at
44,421,672 ns. The cache-blind router computes on peer 1 at 53,095,128 ns.
With a 20 ms link to peer 1, moving there costs 56,056,805 ns and the
router moves the blocks to peer 2 instead, at 41,056,805 ns.

<!-- source: psionic/lib/serve-saved.coax check-route -->
```jai
#test "the hand-traced routes" {
    f:=swx_hand();m:=swx_hand_mesh();v:=swx_hand_view(f);tok:=swx_hand_tok();r:=SwRequest.{prompt=13,output=2};
    // Peer 0 holds 3 blocks and is busy 30 ms; peer 1 is idle and empty; peer 2 holds 2 and is busy 3 ms.
    p:=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0);
    #assert !p.refused && p.dst==1 && p.src==0 && p.blocks==3 && p.est==38056805 && p.turn==0;
    p=sw_route(f[..],m,v,SwPolicy.reuse,r,tok[..],0,0);#assert p.dst==2 && p.src==2 && p.blocks==2 && p.est==44421672;
    p=sw_route(f[..],m,v,SwPolicy.blind,r,tok[..],0,0);#assert p.dst==1 && p.src== -1 && p.blocks==0 && p.est==53095128;
    p=sw_route(f[..],m,v,SwPolicy.round,r,tok[..],0,2);#assert p.dst==2 && p.src== -1 && p.est==56095128 && p.turn==0;
    p=sw_route(f[..],m,v,SwPolicy.round,r,tok[..],0,0);#assert p.dst==0 && p.est==83095128 && p.turn==1;
    // 10 ms later peer 2 is free: moving peer 0's 3 blocks there costs 41,056,805 - 3,000,000 = 38,056,805, a tie that goes to the lower peer.
    p=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],10000000,0);#assert p.dst==1 && p.src==0 && p.est==38056805;
    // At 20 ms the link to peer 1 is slower than reuse on peer 2 for 3 blocks.
    m.lat[0][1]=20000000;p=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0);#assert p.dst==2 && p.src==0 && p.blocks==3 && p.est==41056805;
    // No link from peer 0: peer 2's 2 blocks move to idle peer 1, 43,736,246 against 44,421,672 in place.
    m.gbit[0][1]=0;m.gbit[0][2]=0;p=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0);#assert p.dst==1 && p.src==2 && p.blocks==2 && p.est==43736246;
    m.gbit[2][1]=0;p=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0);#assert p.dst==2 && p.src==2 && p.est==44421672;
    // Equal predictions. A link of 17,038,323 ns makes moving 3 blocks to peer 1 cost what computing them does: cold wins the tie.
    m=swx_hand_mesh();m.lat[0][1]=17038323;m.gbit[2][1]=0;g:=f;g[2].busy=30000000;
    p=sw_route(g[..],m,v,SwPolicy.saved,r,tok[..],0,0);#assert p.dst==1 && p.src== -1 && p.est==53095128;
    m.lat[0][1]=17038322;p=sw_route(g[..],m,v,SwPolicy.saved,r,tok[..],0,0);#assert p.dst==1 && p.src==0 && p.est==53095127;
    // A 9-token prompt: peers 0 and 2 both claim 2 blocks over equal links; the lower source wins.
    m=swx_hand_mesh();q:=r;q.prompt=9;p=sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0);
    #assert p.dst==1 && p.src==0 && p.blocks==2 && p.est==2314574+sw_cost(9,2,8);
    // A request outside the bounds is refused.
    q=r;q.prompt=0;#assert sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0).refused;q.prompt=33;#assert sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0).refused;
    q.prompt=32;#assert !sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0).refused;
    q=r;q.output=0;#assert sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0).refused;q.output=65;#assert sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0).refused;
    q.output=64;#assert !sw_route(f[..],m,v,SwPolicy.saved,q,tok[..],0,0).refused;
}
```

## Claims are hints

A claim was true when the peer advertised it. When the request runs, the
peer may have dropped those blocks to make room for others. `sw_run` runs
the pick against what the peers really hold: it moves the blocks the source
still has, up to the count the pick relied on, uses the destination's own
blocks when it holds more, and computes the rest. A move that finds nothing
still pays the link's latency, because the refusal has to come back.

The model here is a stand-in: its state after a prompt is the prompt's
chained hash. For the blocks it used, the run reads the hash stored in the
trie that supplied them; for the rest it hashes the tokens. So a run that
trusts a missing block reads the wrong node and gives a different output,
and a correct fallback gives the cold output exactly.

<!-- source: psionic/lib/serve-saved.coax run -->
```jai
// Run the pick against what the peers really hold. Moved blocks are those
// the source still has, up to the count the pick relied on; the
// destination also has its own. The rest of the prompt is computed. The
// stand-in model's output is the chained hash of the whole prompt, read
// from the trie for the blocks used and computed for the rest, so a wrong
// block changes it.
sw_run :: (peer:[]SwPeer,m:SwMesh,p:SwPick,r:SwRequest,tok:[]int)->SwDone {
    d:SwDone;src:=ifx p.src<0 then p.dst else p.src;
    d.used=pg_match(peer[p.dst].st,0,tok,r.prompt).n;
    have:=pg_match(peer[src].st,0,tok,r.prompt).n;
    d.missing=cb_max(0,p.blocks-have);
    if src!=p.dst {
        d.moved=cb_min(p.blocks,have);d.link=sw_link(m,src,p.dst,d.moved);
        d.used=cb_max(d.used,d.moved);
    }
    d.cost=d.link+sw_cost(r.prompt,r.output,d.used*m.pages.page);
    d.out=sw_output(peer[p.dst].st,peer[src].st,tok,d.used,r.prompt);
    return d;
}
// The state after `used` blocks: the destination's block when it holds
// that many, else the moved one.
sw_output :: (dst:PgStore,src:PgStore,tok:[]int,used:int,prompt:int)->int {
    h:=0;a:=pg_match(dst,0,tok,prompt);b:=pg_match(src,0,tok,prompt);
    if used>0 {h=ifx a.n>=used then dst.node[a.node[used-1]].hash else src.node[b.node[used-1]].hash;}
    return pg_hash(h,tok[used*dst.cfg.page..<prompt]);
}
```

If peer 0 lost A's third block after advertising it, the routed request
moves 2 blocks and computes the other 13 − 8 = 5 rows: 2,000,000 + 2 ×
157,287 + 41,421,672 = 43,736,246 ns, not the 38,056,805 predicted. If peer 0 lost
everything, the request pays the 2,000,000 ns latency and computes cold:
55,095,128 ns. In both cases the output is the cold run's.

<!-- source: psionic/lib/serve-saved.coax check-missing -->
```jai
#test "a claimed hit that is gone falls back and gives the cold output" {
    f:=swx_hand();m:=swx_hand_mesh();v:=swx_hand_view(f);tok:=swx_hand_tok();r:=SwRequest.{prompt=13,output=2};
    cold:=pg_hash(0,tok[0..<13]);
    p:=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0);
    d:=sw_run(f[..],m,p,r,tok[..]);
    #assert d.used==3 && d.moved==3 && d.missing==0 && d.link==2471861 && d.cost==38056805 && d.out==cold;
    // Peer 0 lost A's third block after advertising it: 2 move, 1 is computed.
    g:=f;g[0].st=swx_forget(g[0].st,2);
    d=sw_run(g[..],m,p,r,tok[..]);#assert d.used==2 && d.moved==2 && d.missing==1 && d.link==2314574 && d.cost==43736246 && d.out==cold;
    // It lost everything: the refusal still costs the link's latency.
    g[0].st=sw_store(m.pages);d=sw_run(g[..],m,p,r,tok[..]);#assert d.used==0 && d.missing==3 && d.link==2000000 && d.cost==55095128 && d.out==cold;
    // Reuse in place that finds 1 of 2 blocks.
    p=sw_route(f[..],m,v,SwPolicy.reuse,r,tok[..],0,0);g=f;g[2].st=swx_forget(g[2].st,1);
    d=sw_run(g[..],m,p,r,tok[..]);#assert d.used==1 && d.moved==0 && d.missing==1 && d.link==0 && d.cost==sw_cost(13,2,4) && d.out==cold;
    // Computing cold on a peer that holds blocks after all uses them: a hit nobody claimed.
    p=SwPick.{dst=0,src=-1};d=sw_run(f[..],m,p,r,tok[..]);#assert d.used==3 && d.missing==0 && d.cost==sw_cost(13,2,12) && d.out==cold;
    // Blocks moved to a peer that holds more of its own: the more it has is used.
    p=SwPick.{dst=0,src=2,blocks=2};d=sw_run(f[..],m,p,r,tok[..]);#assert d.used==3 && d.moved==2 && d.link==m.lat[2][0]+2*sw_block_ns(m,2,0) && d.out==cold;
}
```

## The simulation

<!-- source: psionic/lib/serve-saved.coax sim -->
```jai
// Requests in arrival order. When a request arrives in a new advertising
// round, every peer advertises what it holds, stamped with the round's
// start, and the router accepts or refuses each at the request's arrival.
// The request is routed, run against the real caches, and waits for its
// peer; its prompt is cached on that peer at once.
sw_sim :: (m:SwMesh,pol:SwPolicy,reqs:[]SwRequest,tok:[]int)->SwSim {
    s:SwSim;s.status=sw_check(m,reqs,tok);
    if s.status!=SwStatus.ok {return s;}
    s.n=reqs.count;mem:[1728]f64;v:SwView;round:=-1;turn:=0;
    for k:0..<m.peers {s.peer[k].st=sw_store(m.pages);}
    for i:0..<s.n {
        r:=reqs[i];now:=r.arrive;t:=tok[i*PG_PROMPT..<i*PG_PROMPT+PG_PROMPT];
        if now/m.period>round {
            round=now/m.period;
            for k:0..<m.peers {
                g:=sw_accept(v,m,sw_advertise(s.peer[k].st,m,k,round,round*m.period),now);v=g.view;
                if g.refusal==SwAdRefusal.none {s.ads+=1;} else {s.stale+=1;}
            }
        }
        p:=sw_route(s.peer[0..<m.peers],m,v,pol,r,t,now,turn);
        s.fate[i]=SwFate.{refused=p.refused,dst=p.dst,src=p.src,claimed=p.blocks};
        if p.refused {s.refused+=1;continue;}
        turn=p.turn;d:=sw_run(s.peer[0..<m.peers],m,p,r,t);k:=p.dst;
        f:=SwFate.{dst=k,src=p.src,claimed=p.blocks,used=d.used,moved=d.moved,missing=d.missing,cost=d.cost,out=d.out};
        f.start=now+sw_wait(s.peer[..],k,now);f.pred=p.est-(f.start-now);f.cold=sw_cost(r.prompt,r.output,0);f.finish=f.start+d.cost;
        s.fate[i]=f;s.peer[k].busy=f.finish;s.peer[k].st=sw_cache(s.peer[k].st,mem[..],t,r.prompt,i);
        s.served+=1;s.missing+=d.missing;s.pred_saved+=f.cold-f.pred;s.real_saved+=f.cold-d.cost;s.latency+=f.finish-now;s.end=cb_max(s.end,f.finish);
        if d.moved>0 {s.moves+=1;s.moved+=d.moved;} else {s.local+=d.used;}
    }
    return s;
}
```

Requests are taken in arrival order. When one arrives in a new advertising
round, every peer advertises what it holds at that moment, stamped with the
round's start, and the router accepts or refuses each one at the request's
arrival. The request is routed, run against the real caches, and waits for
its peer; its prompt's blocks are then cached on that peer, with chapter
176's least recently used blocks making room. Each fate records the claim,
the blocks used, moved and missing, the predicted and realized cost, and
the cold cost on the same peer. Two simplifications are stated in the
profile: a transfer does not occupy its source, and a prompt is cached when
it is routed rather than when it finishes.

## Running it

The example, [`serve-saved.coax`](../examples/serve-saved.coax), prints the
hand trace, the fallback, then four routers on the same shared-prefix
traffic and the saved-work router at three advertising periods and with
short-lived advertisements.

<!-- source: psionic/examples/serve-saved.coax main -->
```jai
main :: () {
    f:=swx_hand();m:=swx_hand_mesh();v:=swx_hand_view(f);tok:=swx_hand_tok();r:=SwRequest.{prompt=13,output=2};
    print("hand trace (%): claims ",SW_PROFILE);
    for k:0..<3 {print("peer % % blocks; ",k,sw_claim(v,m,k,tok[..],13,0));}
    print("one block over 10 Gbit/s: % ns\n",sw_block_ns(m,0,1));
    for k:0..<4 {pol:=cast(SwPolicy)cast(u8)k;show(sw_policy_name(pol),sw_route(f[..],m,v,pol,r,tok[..],0,0));}
    slow:=m;slow.lat[0][1]=20000000;show("saved work, 20 ms to peer 1",sw_route(f[..],slow,v,SwPolicy.saved,r,tok[..],0,0));

    p:=sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0);g:=f;g[0].st=swx_forget(g[0].st,2);
    d:=sw_run(g[..],m,p,r,tok[..]);
    print("peer 0 lost its third block: % moved, % missing, cost % ns against % predicted; output % (cold %)\n",d.moved,d.missing,d.cost,p.est,d.out,pg_hash(0,tok[0..<13]));

    w:=swx_traffic();reqs:[64]SwRequest;toks:[2048]int;sw_traffic(w,reqs[..],toks[..]);
    mesh:=swx_mesh();
    print("shared-prefix traffic: % requests % ms apart, % families of % tokens, prompts of %, % output tokens; % peers, advertising every % ms\n",w.n,w.gap/1000000,w.families,w.prefix,w.prompt,w.output,mesh.peers,mesh.period/1000000);
    for k:0..<4 {pol:=cast(SwPolicy)cast(u8)k;line(sw_policy_name(pol),sw_sim(mesh,pol,reqs[..],toks[..]));}
    for period:[3]int.[20000000,100000000,400000000] {
        x:=mesh;x.period=period;x.ttl=period+period/2;s:=sw_sim(x,SwPolicy.saved,reqs[..],toks[..]);
        print("saved work, advertising every % ms (% accepted):\n",period/1000000,s.ads);
        line("saved work",s);
    }
    x:=mesh;x.ttl=10000000;
    print("saved work, advertisements living 10 ms:\n");line("saved work",sw_sim(x,SwPolicy.saved,reqs[..],toks[..]));
}
```

```shell
coax/bin/coax run psionic/examples/serve-saved.coax
```

```text
hand trace (sw-saved-v1): claims peer 0 3 blocks; peer 1 0 blocks; peer 2 2 blocks; one block over 10 Gbit/s: 157287 ns
  saved work: peer 1, 3 blocks moved from peer 0, predicted finish 38056805 ns
  reuse in place: peer 2, 2 blocks reused in place, predicted finish 44421672 ns
  cache-blind: peer 1, computed cold, predicted finish 53095128 ns
  round robin: peer 0, computed cold, predicted finish 83095128 ns
  saved work, 20 ms to peer 1: peer 2, 3 blocks moved from peer 0, predicted finish 41056805 ns
peer 0 lost its third block: 2 moved, 1 missing, cost 43736246 ns against 38056805 predicted; output 1122935492 (cold 1122935492)
shared-prefix traffic: 64 requests 20 ms apart, 6 families of 24 tokens, prompts of 28, 4 output tokens; 4 peers, advertising every 100 ms
  saved work: 24 transfers moving 69 blocks, 60 blocks reused in place, 61 claimed blocks missing
    saved against cold: predicted 1029.3 ms, realized 711.2 ms; mean latency 249.5 ms; last token at 1661.9 ms
  reuse in place: 0 transfers moving 0 blocks, 128 blocks reused in place, 16 claimed blocks missing
    saved against cold: predicted 811.3 ms, realized 747.1 ms; mean latency 252.4 ms; last token at 1656.9 ms
  cache-blind: 0 transfers moving 0 blocks, 53 blocks reused in place, 0 claimed blocks missing
    saved against cold: predicted 0.0 ms, realized 309.3 ms; mean latency 296.6 ms; last token at 1768.2 ms
  round robin: 0 transfers moving 0 blocks, 64 blocks reused in place, 0 claimed blocks missing
    saved against cold: predicted 0.0 ms, realized 373.5 ms; mean latency 292.9 ms; last token at 1770.7 ms
saved work, advertising every 20 ms (256 accepted):
  saved work: 34 transfers moving 154 blocks, 57 blocks reused in place, 0 claimed blocks missing
    saved against cold: predicted 1139.3 ms, realized 1139.3 ms; mean latency 191.7 ms; last token at 1552.0 ms
saved work, advertising every 100 ms (52 accepted):
  saved work: 24 transfers moving 69 blocks, 60 blocks reused in place, 61 claimed blocks missing
    saved against cold: predicted 1029.3 ms, realized 711.2 ms; mean latency 249.5 ms; last token at 1661.9 ms
saved work, advertising every 400 ms (16 accepted):
  saved work: 13 transfers moving 33 blocks, 46 blocks reused in place, 115 claimed blocks missing
    saved against cold: predicted 855.5 ms, realized 411.9 ms; mean latency 289.4 ms; last token at 1722.7 ms
saved work, advertisements living 10 ms:
  saved work: 6 transfers moving 21 blocks, 64 blocks reused in place, 0 claimed blocks missing
    saved against cold: predicted 107.2 ms, realized 480.8 ms; mean latency 269.5 ms; last token at 1723.0 ms
```

The traffic is 64 requests 20 ms apart. Each prompt has 28 tokens, and its
first 24 are one of 6 families; each peer has room for 8 blocks, a little
more than one prompt. The load is higher than four peers can serve cold, so
queues build and where a request goes matters.

With advertisements every 100 ms, moving blocks to free peers gives the
lowest mean latency, 249.5 ms against 252.4 ms for prefix affinity and
296.6 ms for the cache-blind router. But its claims go stale fastest: it
predicted 1,029.3 ms of saving and realized 711.2 ms, because 61 claimed
blocks were gone when their requests ran. Prefix affinity realized more,
747.1 ms, with only 16 missing blocks. The advertising period decides this:
every 20 ms, no claim is missing, every predicted nanosecond is realized,
1,139.3 ms, and the mean latency falls to 191.7 ms; every 400 ms, 115 claimed
blocks are missing and less than half the predicted saving is realized.
Advertisements that live 10 ms expire before most requests of a round
arrive, so the router mostly ignores the caches; most of what is still
saved comes from blocks the destination happened to hold.

These numbers are frozen in a check:

<!-- source: psionic/lib/serve-saved.coax check-frozen -->
```jai
#test "frozen: four routers on shared-prefix traffic, and the advertising period" {
    m:=swx_mesh();reqs:[64]SwRequest;tok:[2048]int;sw_traffic(swx_traffic(),reqs[..],tok[..]);
    s:=sw_sim(m,SwPolicy.saved,reqs[..],tok[..]);
    #assert s.status==SwStatus.ok && s.served==64 && s.ads==52 && s.stale==0 && s.moves==24 && s.moved==69 && s.local==60 && s.missing==61;
    #assert s.pred_saved==1029317445 && s.real_saved==711268749 && s.latency==15970160055 && s.end==1661982342;
    s=sw_sim(m,SwPolicy.reuse,reqs[..],tok[..]);
    #assert s.moves==0 && s.local==128 && s.missing==16 && s.pred_saved==811305192 && s.real_saved==747101184 && s.latency==16154479018;
    s=sw_sim(m,SwPolicy.blind,reqs[..],tok[..]);
    #assert s.moves==0 && s.local==53 && s.pred_saved==0 && s.real_saved==309346584 && s.latency==18982802282;
    s=sw_sim(m,SwPolicy.round,reqs[..],tok[..]);
    #assert s.moves==0 && s.local==64 && s.real_saved==373550592 && s.latency==18748806096;
    // Advertising every 20 ms: no claim is missing, and every predicted saving is realized.
    m.period=20000000;m.ttl=30000000;s=sw_sim(m,SwPolicy.saved,reqs[..],tok[..]);
    #assert s.ads==256 && s.missing==0 && s.moves==34 && s.pred_saved==1139327410 && s.real_saved==1139327410 && s.latency==12268903814;
}
```

## Independent checks

Two `#forall` checks compare the code against separate implementations in
[`serve-saved-checks.coax`](../tests/serve-saved-checks.coax).

The first builds 120 random meshes of 1 to 5 peers, with random links (some
missing), caches filled from three families of prompts, advertisements that
may be absent, expired, or taken before more was cached, random queues and
a random request. The brute-force router recomputes every claim with its
own lookup (each block's hash taken over the whole prefix from the start,
every entry scanned) and its own copy of the price, and tries every peer,
every source and every block count from 1 to the claim. Each policy's
choice must have the brute-force minimum price.

<!-- source: psionic/lib/serve-saved.coax check-oracle -->
```jai
#forall "every policy's choice is the brute-force cheapest of its options" seed:0..<120 #steps 200000000 {
    x:=swx_random(seed);
    for k:0..<4 {
        pol:=cast(SwPolicy)cast(u8)k;
        p:=sw_route(x.peer[0..<x.mesh.peers],x.mesh,x.view,pol,x.req,x.tok[..],x.now,x.turn);
        o:=swx_oracle(x,pol);
        #assert !p.refused && p.est==o.est && swx_price(x,pol,p.dst,p.src,p.blocks)==p.est;
        if pol==SwPolicy.round {#assert p.dst==o.dst;}
    }
}
```

The second runs 12 random simulations of 2 to 4 peers under all four
policies and recomputes each record from its fates: every output is the
cold run's, each peer serves in arrival order starting when it is free,
every cost is the link plus the step model for the rows left, a fate with
nothing missing and nothing extra realizes its prediction, the policy's
limits hold (cache-blind and round robin never claim, prefix affinity never
moves), the counters and savings are the sums of the fates, the accepted and
refused advertisements are those each round's first arrival implies, and no
final advertisement lists any token of the workload or any unsalted hash.
A last check confirms that these random runs reach transfers, missing
claims, stale advertisements and reuse in place.

## Refusals

The second example,
[`serve-saved-refuse.coax`](../examples/serve-saved-refuse.coax), sends
the router advertisements it must refuse, one per
reason, then a forged one: peer 1 advertises blocks it does not hold. The
router believes it, as it must believe any well-formed hint, routes the
request to peer 1, finds the blocks missing, computes them, and gives the
cold output. Last come a prompt over the bound and a mesh whose peers cannot
hold one prompt.

<!-- source: psionic/examples/serve-saved-refuse.coax main -->
```jai
main :: () {
    f:=swx_hand();m:=swx_hand_mesh();empty:SwView;
    a:=sw_advertise(f[0].st,m,0,5,1000);
    offer("peer 0's advertisement",empty,m,a,1000);
    held:=sw_accept(empty,m,a,1000).view;
    x:=a;x.peer=3;offer("from peer 3 of 3",empty,m,x,1000);
    x=a;x.epoch=6;offer("epoch 6, the mesh is at 7",empty,m,x,1000);
    offer("the same advertisement again",held,m,a,1000);
    x=a;x.issued=1001;offer("issued at 1,001 ns, received at 1,000",empty,m,x,1000);
    x=a;x.ttl=m.ttl+1;offer("living 1 ns longer than the mesh allows",empty,m,x,1000);
    x=a;x.ttl=5000;offer("issued at 1,000 ns for 5,000 ns, received at 6,000",empty,m,x,6000);
    x=a;x.n=17;offer("17 entries",empty,m,x,1000);
    for k:0..<16 {x.entry[k]=k+1;}
    x.n=9;offer("9 distinct entries from a peer of 8 frames",empty,m,x,1000);
    x.n=8;x.entry[7]=1;offer("8 entries, one listed twice",empty,m,x,1000);
    // A forged claim: peer 1 holds nothing, but advertises A's blocks.
    tok:=swx_hand_tok();forged:=sw_advertise(f[0].st,m,1,1,0);v:=held;v=sw_accept(v,m,forged,1000).view;
    r:=SwRequest.{prompt=13,output=2};g:=f;
    p:=sw_route(g[..],m,v,SwPolicy.saved,r,tok[..],1000,0);d:=sw_run(g[..],m,p,r,tok[..]);
    print("peer 1 claims % blocks it does not hold; routed to peer % from peer %; % missing, output equals cold: %\n",sw_claim(v,m,1,tok[..],13,1000),p.dst,p.src,d.missing,d.out==pg_hash(0,tok[0..<13]));
    r.prompt=33;print("a prompt of 33 tokens: refused %\n",sw_route(f[..],m,v,SwPolicy.saved,r,tok[..],0,0).refused);
    q:=[1]SwRequest.[SwRequest.{prompt=4,output=1}];t:[32]int;bad:=m;bad.pages.cap[0]=7;
    print("a mesh whose peers hold 7 pages of 4: %\n",sw_status_name(sw_sim(bad,SwPolicy.saved,q[..],t[..]).status));
}
```

```shell
coax/bin/coax run psionic/examples/serve-saved-refuse.coax
```

```text
peer 0's advertisement: accepted
from peer 3 of 3: no such peer
epoch 6, the mesh is at 7: another epoch
the same advertisement again: not newer than the one held
issued at 1,001 ns, received at 1,000: issued in the future
living 1 ns longer than the mesh allows: lifetime out of bounds
issued at 1,000 ns for 5,000 ns, received at 6,000: expired
17 entries: more entries than an advertisement carries
9 distinct entries from a peer of 8 frames: more blocks than the peer has frames
8 entries, one listed twice: an entry listed twice
peer 1 claims 3 blocks it does not hold; routed to peer 1 from peer 1; 3 missing, output equals cold: true
a prompt of 33 tokens: refused true
a mesh whose peers hold 7 pages of 4: configuration or traffic out of bounds
```

Both examples exit with status 0; the transcripts are compared in debug and
release builds.

## What remains

This chapter builds the router, the advertisement rules and the fallback,
and checks them in simulation. These parts of the plan do not run yet:

- **Real advertisements.** Peers sending advertisements over
  [chapter 188](ch188-peers-transport-and-discovery.md)'s encrypted links,
  with the link authenticating the sender.
- **Real transfers.** Moving blocks between real processes with chapter
  182's snapshot format (`dg_pack`, `dg_check`), whose refusal would replace
  the latency-only refusal here, and charging the source for the time it
  spends sending.
- **A real model.** The output here is a stand-in hash; the same fallback
  against a real model's tokens is not checked.
- **Recorded traffic.** The savings are reported on synthetic traffic, not
  on a recorded workload.
- **Trust.** A peer that lies in its advertisements costs the mesh time
  but cannot change an output; penalizing such a peer, and routing only to
  peers trusted with a prompt, belong to later chapters.

## Run the checks

```shell
coax/bin/coax test psionic/lib/serve-saved.coax
coax/bin/coax mutate psionic/lib/serve-saved.coax --root-only
psionic/check.sh --only serve-saved
```

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
