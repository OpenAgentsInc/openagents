# 188. Peers, transport and discovery

> **Status: partly implemented.** The in-process slice runs: encrypted frames, direct and relayed paths with measured round trips, invitation-only joins, group routing from beacon fields and separate lanes. Sockets, forward secrecy, signed invitations and the v2 beacon reader remain; see [What remains](#what-remains).

A model split across machines runs as stages on different peers. The peers
need four things before any activation moves:

- an **encrypted connection** to each other, whose path (direct or through a
  relay) and round trip are recorded;
- a way to keep a **private mesh** private: only invited peers join;
- a way to **find** a public model group: its head and stages, from
  [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md) beacons, never from a
  stale one;
- a **control lane** for the owner's commands that is separate from the
  inference lane.

Mesh-LLM does the first with QUIC and discovers public meshes through Nostr
([Inference across a mesh](../docs/serving/mesh.md)). This chapter builds
the same decisions in Coax, on the cryptography that `coax/lib` already has,
and runs them in one process over simulated wires. Two files hold it:

- [`peer/policy.coax`](../lib/peer/policy.coax), an `#authority` file: the
  lane rules, the join rule and the group route. It does no cryptography.
- [`peer/link.coax`](../lib/peer/link.coax): sessions, frames, wires, a
  relay, the ping that measures a round trip, and invitations.

The profile is `psionic-peer-loopback-v0`: in-process, one CPU core, every
clock simulated. The frame layout is that profile's, not a published
protocol. Nothing in this chapter opens a socket, so nothing in `proto/` or
`docs/protocol/` changes. A wire contract between real machines will start
there, as a new versioned schema.

## Run it

The program is [`peer-mesh.coax`](../examples/peer-mesh.coax):

```shell
coax/bin/coax run psionic/examples/peer-mesh.coax
```

Five peers take part. Their keys are BIP-340's signing vectors 0 to 3 and
the secret 7, so each public key can be checked against a published value:
the head is chapter 24's Pylon key `f9308a01…`, and the owner is chapter 24's
owner `5cbdf064…`.

<!-- source: psionic/examples/peer-mesh.coax example -->
```jai
main :: () {
    p:[5]ExPeer;
    for v:0..<4 {pp_vector_key(v,p[v].secret[..],p[v].pub[..]);}
    p[4].secret[31]=7;#assert pp_public(p[4].secret[..],p[4].pub[..]);
    owner:=p[4].pub;
    mine:[32]u8;#assert pp_public(p[EX_HEAD].secret[..],mine[..]) && pp_same(mine[..],p[EX_HEAD].pub[..]);
    hex:[5][8]u8;
    print("peers: head %, stage A %, stage B %, outsider %, owner %\n",ex_short(p[0].pub[..],hex[0][..]),ex_short(p[1].pub[..],hex[1][..]),ex_short(p[2].pub[..],hex[2][..]),ex_short(p[3].pub[..],hex[3][..]),ex_short(p[4].pub[..],hex[4][..]));

    // 1. Discovery: the head and two stages of group v41-a, read at EX_NOW.
    head:=PgBeacon.{key=cast(string)hex[0][..],owner=cast(string)hex[4][..],status=PbStatus.online,observed_at=EX_NOW-100,valid_until=EX_NOW+200,beacon_generation=3,
        id="v41-a",role=PgRole.head,generation=3,artifact="sha256:aa",readiness=PgReady.ready,free=1};
    st:[2]PgBeacon;st[0]=head;st[1]=head;
    st[0].key=cast(string)hex[2][..];st[0].role=PgRole.stage;st[0].free=0;st[0].from=30;st[0].to=61;
    st[1].key=cast(string)hex[1][..];st[1].role=PgRole.stage;st[1].free=0;st[1].from=0;st[1].to=30;
    order:[2]int;r:=pg_route(head,st[..],"sha256:aa",61,EX_NOW,order[..]);
    print("route to v41-a for sha256:aa at %: %, stages in order:",EX_NOW,r.code);
    for i:0..<r.stages {s:=st[order[i]];print(" % layers %..%",s.key,s.from,s.to);}
    print("\n");
    late:=st;late[0].valid_until=EX_NOW-10;r=pg_route(head,late[..],"sha256:aa",61,EX_NOW,order[..]);
    print("stage B's beacon expired 10 s ago: % (stage %)\n",r.code,r.bad);
    print("200 s later, past the head's valid_until: %\n",pg_route(head,st[..],"sha256:aa",61,EX_NOW+201,order[..]).code);
    print("a request for sha256:bb: %\n",pg_route(head,st[..],"sha256:bb",61,EX_NOW,order[..]).code);

    // 2. Connections from the head: stage A directly, stage B through a relay.
    ha:[1]PpSession;ah:[1]PpSession;hb:[1]PpSession;bh:[1]PpSession;ho:[1]PpSession;oh:[1]PpSession;ob:[1]PpSession;bo:[1]PpSession;
    ha[0]=pp_session(p[EX_HEAD].secret[..],p[EX_A].pub[..]);ah[0]=pp_session(p[EX_A].secret[..],p[EX_HEAD].pub[..]);
    hb[0]=pp_session(p[EX_HEAD].secret[..],p[EX_B].pub[..]);bh[0]=pp_session(p[EX_B].secret[..],p[EX_HEAD].pub[..]);
    ho[0]=pp_session(p[EX_HEAD].secret[..],p[EX_OUT].pub[..]);oh[0]=pp_session(p[EX_OUT].secret[..],p[EX_HEAD].pub[..]);
    ob[0]=pp_session(p[4].secret[..],p[EX_B].pub[..]);bo[0]=pp_session(p[EX_B].secret[..],owner[..]);
    #assert ha[0].ok && ah[0].ok && hb[0].ok && bh[0].ok && ho[0].ok && oh[0].ok && ob[0].ok && bo[0].ok;
    ra:[1]PpRoute;rb:[1]PpRoute;ro:[1]PpRoute;rw:[1]PpRoute;
    pp_route(ra[..],PpPath.direct,400,0);pp_route(rb[..],PpPath.relayed,9000,12000);
    pp_route(ro[..],PpPath.direct,1500,0);pp_route(rw[..],PpPath.direct,700,0);
    c:=pp_ping(ha[..],ah[..],ra[..],1000);print("head to stage A: %, round trip % us\n",c.path,c.rtt);
    c=pp_ping(hb[..],bh[..],rb[..],1000);print("head to stage B: %, round trip % us; the relay forwarded % frames, % bytes\n",c.path,c.rtt,rb[0].relayed,rb[0].relay_bytes);

    // 3. The private mesh "lab". The owner issued invitations under the mesh
    // key the head holds; each join travels on its sender's control lane.
    key:=[16]u8.[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16];
    expires:=EX_NOW+3600;mac_a:[32]u8;mac_b:[32]u8;text:[200]u8;plain:[PP_TEXT_MAX]u8;frame:[PP_FRAME_MAX]u8;
    #assert pp_invite_mac(key[..],"lab",p[EX_A].pub[..],expires,mac_a[..]) && pp_invite_mac(key[..],"lab",p[EX_B].pub[..],expires,mac_b[..]);
    n:=pp_join_text("lab",p[EX_A].pub[..],expires,mac_a[..],text[..]);
    print("join lab: stage A with its invitation: %\n",ex_join(ah[..],ha[..],ra[..],text[0..<n],key[..],EX_NOW));
    m:=pp_join_text("lab",p[EX_B].pub[..],expires,mac_b[..],text[100..<200]);
    print("join lab: stage B with its invitation, through the relay: %\n",ex_join(bh[..],hb[..],rb[..],text[100..<100+m],key[..],EX_NOW));
    none:=[1]u8.[cast(u8)PpKind.join];
    print("join lab: the outsider without one: %\n",ex_join(oh[..],ho[..],ro[..],none[0..<0],key[..],EX_NOW));
    print("join lab: the outsider with stage A's invitation: %\n",ex_join(oh[..],ho[..],ro[..],text[0..<n],key[..],EX_NOW));
    forged:[200]u8;for i:0..<n {forged[i]=text[i];}for i:0..<32 {forged[5+i]=p[EX_OUT].pub[i];}
    print("join lab: the outsider's own key in stage A's invitation: %\n",ex_join(oh[..],ho[..],ro[..],forged[0..<n],key[..],EX_NOW));
    print("join lab: stage A's invitation after it expired: %\n",ex_join(ah[..],ha[..],ra[..],text[0..<n],key[..],expires));

    // 4. Lanes. The head sends an activation, the stage answers with tokens,
    // the owner drains stage B; then three messages on the wrong terms.
    la:[1]ExLog;lh:[1]ExLog;lb:[1]ExLog;
    act:[17]u8;act[0]=cast(u8)PpKind.activation;for i:1..<17 {act[i]=cast(u8)(i*13);}
    tok:=[3]u8.[cast(u8)PpKind.tokens,42,7];drain:=[1]u8.[cast(u8)PpKind.drain];
    print("head -> stage A, activation (16 bytes) on inference: %\n",ex_send(ha[..],ah[..],ra[..],0,1,act[..],owner[..],true,la[..]));
    print("stage A -> head, tokens on inference: %\n",ex_send(ah[..],ha[..],ra[..],1,1,tok[..],owner[..],true,lh[..]));
    print("owner -> stage B, drain on control: %\n",ex_send(ob[..],bo[..],rw[..],0,0,drain[..],owner[..],true,lb[..]));
    print("head -> stage B, drain on control: %\n",ex_send(hb[..],bh[..],rb[..],0,0,drain[..],owner[..],true,lb[..]));
    print("owner -> stage B, drain on inference: %\n",ex_send(ob[..],bo[..],rw[..],0,1,drain[..],owner[..],true,lb[..]));
    print("head -> stage A, activation on control: %\n",ex_send(ha[..],ah[..],ra[..],0,0,act[..],owner[..],true,la[..]));
    print("outsider -> head, activation before joining: %\n",ex_send(oh[..],ho[..],ro[..],0,1,act[..],owner[..],false,lh[..]));
    print("head -> stage B, activation through the relay: %\n",ex_send(hb[..],bh[..],rb[..],0,1,act[..],owner[..],true,lb[..]));
    for who:0..<2 {
        l:=ifx who==0 then la[0] else lb[0];
        print("lanes at stage %:",ifx who==0 then "A" else "B");
        for lane:0..<2 {print(" % [",ex_lanes[lane]);for i:0..<l.count[lane] {print("%",ifx i>0 then ", " else "");print("%",ex_names[l.kinds[lane][i]]);}print("]");}
        print("\n");
    }
    seen:=rb[0].seen;
    print("the relay carried % frames, % bytes; the activation's bytes in its last frame: %\n",rb[0].relayed,rb[0].relay_bytes,pp_contains(seen[0..<rb[0].seen_n],act[1..<17]));

    // 5. Frames stage B refuses: cut short, claiming too much, altered in
    // transit, and the same frame twice.
    got:[PP_FRAME_MAX]u8;
    f:=pp_seal(hb[..],1,act[..],frame[..]);
    print("a frame cut by one byte: %\n",pp_open(bh[..],frame[0..<f-1],plain[..]).code);
    big:[PP_HEADER]u8;for i:0..<PP_HEADER {big[i]=frame[i];}big[10]=0x13;big[11]=0x88;
    print("a header claiming 5000 body bytes: %\n",pp_open(bh[..],big[..],plain[..]).code);
    #assert pp_send(rb[..],0,0,frame[0..<f]);
    rb[0].wires[0][0].slots[rb[0].wires[0][0].first].bytes[20]^=1;
    k:=pp_deliver(rb[..],0,got[..]);
    print("one bit of the ciphertext flipped on the way to the relay: %\n",pp_open(bh[..],got[0..<k.n],plain[..]).code);
    print("the untouched frame: %\n",pp_open(bh[..],frame[0..<f],plain[..]).code);
    print("the same frame again: %\n",pp_open(bh[..],frame[0..<f],plain[..]).code);
}
```

The transcript, the same in debug and release
([`peer-mesh.out`](../fixtures/peer-mesh.out)):

```text
peers: head f9308a01, stage A dff1d77f, stage B dd308afe, outsider 25d1dff9, owner 5cbdf064
route to v41-a for sha256:aa at 1791400100: ok, stages in order: dff1d77f layers 0..30 dd308afe layers 30..61
stage B's beacon expired 10 s ago: stage_stale (stage 0)
200 s later, past the head's valid_until: stale
a request for sha256:bb: model
head to stage A: direct, round trip 800 us
head to stage B: relayed, round trip 42000 us; the relay forwarded 2 frames, 74 bytes
join lab: stage A with its invitation: ok
join lab: stage B with its invitation, through the relay: ok
join lab: the outsider without one: invitation
join lab: the outsider with stage A's invitation: invitee
join lab: the outsider's own key in stage A's invitation: forged
join lab: stage A's invitation after it expired: expired
head -> stage A, activation (16 bytes) on inference: ok
stage A -> head, tokens on inference: ok
owner -> stage B, drain on control: ok
head -> stage B, drain on control: owner
owner -> stage B, drain on inference: lane
head -> stage A, activation on control: lane
outsider -> head, activation before joining: member
head -> stage B, activation through the relay: ok
lanes at stage A: control [] inference [activation]
lanes at stage B: control [drain] inference [activation]
the relay carried 5 frames, 253 bytes; the activation's bytes in its last frame: false
a frame cut by one byte: length
a header claiming 5000 body bytes: oversized
one bit of the ciphertext flipped on the way to the relay: forged
the untouched frame: ok
the same frame again: replay
```

The rest of the chapter works through each block of it.

## Find the group

A model too large for one machine runs as a NIP-PYLON *group*: one head and
one stage per machine, each stage holding a half-open range of layers. A
buyer routes to the head only when the head's beacon is fresh, online,
`ready` and has its one slot free, and only for the model it asked for.
Before the head can serve, it needs its stages in layer order, each fresh,
in the same group, on the same generation, with the same owner.

`pg_route` makes that decision from fields a reader has already taken out of
the beacons. Freshness is chapter 24's `pb_fresh`, called unchanged: a beacon
is stale after its `valid_until`, or more than 300 seconds after its
`observed_at`, and it is refused if its `observed_at` is more than 30 seconds
ahead of the reader's clock.

<!-- source: psionic/lib/peer/policy.coax group -->
```jai
// What a buyer read from a v2 group beacon (NIP-PYLON "Model groups"): the
// beacon's key, its NIP-OA owner, its clock fields and its group fields.
PgRole :: enum u8 {head;stage;}
PgReady :: enum u8 {starting;ready;recovering;}
PgBeacon :: struct {key:string;owner:string;status:PbStatus;observed_at:int;valid_until:int;beacon_generation:int;
    id:string;role:PgRole;generation:int;artifact:string;readiness:PgReady;free:int;from:int;to:int;}
PgCode :: enum u8 {ok;head;stale;offline;not_ready;busy;model;stage_stale;stage_group;gap;stages;}
PgRoute :: struct {code:PgCode;stages:int;bad:int=-1;}
PG_STAGES :: 16;

pg_fresh :: (b:PgBeacon,now:int)->bool {
    r:=PbRead.{observed_at=b.observed_at,valid_until=b.valid_until,generation=b.beacon_generation};
    return pb_fresh(r,now,PbHeld.{})==PbFresh.fresh;
}
// Whether a buyer may send this group a job for artifact now, and if so the
// order of its stages: order[i] is the index in stages of the stage that
// holds layers from order's running end. The stages must tile 0..layers
// exactly, each fresh, in this group, on the head's generation and owner.
pg_route :: (head:PgBeacon,stages:[]PgBeacon,artifact:string,layers:int,now:int,order:[]int)->PgRoute #export_c {
    if head.role!=PgRole.head {return PgRoute.{code=PgCode.head};}
    if !pg_fresh(head,now) {return PgRoute.{code=PgCode.stale};}
    if head.status!=PbStatus.online {return PgRoute.{code=PgCode.offline};}
    if head.readiness!=PgReady.ready {return PgRoute.{code=PgCode.not_ready};}
    if head.free<1 {return PgRoute.{code=PgCode.busy};}
    if head.artifact!=artifact {return PgRoute.{code=PgCode.model};}
    if stages.count<1 || stages.count>PG_STAGES || order.count<stages.count || layers<1 {return PgRoute.{code=PgCode.stages};}
    for i:0..<stages.count {
        s:=stages[i];
        if !pg_fresh(s,now) {return PgRoute.{code=PgCode.stage_stale,bad=i};}
        if s.role!=PgRole.stage || s.id!=head.id || s.artifact!=head.artifact || s.generation!=head.generation || s.owner!=head.owner {return PgRoute.{code=PgCode.stage_group,bad=i};}
    }
    // Walk the layers: at each running end exactly one stage must start there.
    at:=0;
    for n:0..<stages.count {
        hit:=-1;
        for i:0..<stages.count {if stages[i].from==at && stages[i].to>at {if hit>=0 {return PgRoute.{code=PgCode.gap,bad=i};}hit=i;}}
        if hit<0 {return PgRoute.{code=PgCode.gap,bad=n};}
        order[n]=hit;at=stages[hit].to;
    }
    if at!=layers {return PgRoute.{code=PgCode.gap};}
    return PgRoute.{code=PgCode.ok,stages=stages.count};
}
```

By hand, for the example: the head was observed at 1791400000 and is valid
until 1791400300; the reader's clock is 1791400100, so the head is fresh.
Stage B holds layers 30 to 61 and is listed first; stage A holds 0 to 30. The
walk starts at layer 0: only stage A starts there, so it comes first and
the running end moves to 30. Only stage B starts at 30, and the end moves to
61, which is the model's layer count. The route is `ok` with stages A, B.

The next three lines of the transcript are refusals. With stage B's beacon
valid only until 10 seconds ago, the route fails with `stage_stale` and
names stage 0, its index in the list. At 1791400301, one second past the
head's `valid_until`, the head itself is `stale`. A request for another
artifact is `model`. A stale or expired beacon never produces a route.

## Connect

Each connection has two ends. Both derive the same 32-byte conversation key
from their secp256k1 keys, using NIP-44's ECDH and HKDF-Extract
(`nip44_conversation_key_from_keys_checked`, from `coax/lib/nostr`). From it,
each direction gets its own key: HKDF-Expand with the info string
`psionic-peer-v0` followed by the sender's and the receiver's public keys.
The dialer's sending key is the other end's receiving key.

<!-- source: psionic/lib/peer/link.coax session -->
```jai
// The key for frames from `from` to `to`: HKDF-Expand of the conversation
// key with info "psionic-peer-v0" || from || to.
pp_direction :: (ck:[]u8,from:[]u8,to:[]u8,out:[]u8)->bool {
    info:[79]u8;tag:="psionic-peer-v0";
    for i:0..<15 {info[i]=tag[i];}
    for i:0..<32 {info[15+i]=from[i];info[47+i]=to[i];}
    return hkdf_expand_checked(ck,info[..],out,32,false)==SHA_OK;
}
// A session from its two public keys and their conversation key.
pp_keys :: (me:[]u8,peer:[]u8,ck:[]u8)->PpSession {
    s:PpSession;if me.count!=32 || peer.count!=32 {return s;}
    for i:0..<32 {s.me[i]=me[i];s.peer[i]=peer[i];}
    s.ok=pp_direction(ck,s.me[..],s.peer[..],s.send[..]) && pp_direction(ck,s.peer[..],s.me[..],s.recv[..]);
    return s;
}
pp_session :: (secret:[]u8,peer:[]u8)->PpSession {
    me:[32]u8;ck:[32]u8;work:[1]EcdhScratch;
    if !pp_public(secret,me[..]) || nip44_conversation_key_from_keys_checked(secret,peer,work[..],ck[..],false)!=NIP44_OK {return PpSession.{};}
    return pp_keys(me[..],peer,ck[..]);
}
// The nonce: the lane, 19 zero bytes and the sequence number. Each
// direction has its own key, so a (key, nonce) pair never repeats while
// sequence numbers only rise.
pp_nonce :: (lane:int,seq:int)->[24]u8 {
    out:[24]u8;out[0]=cast(u8)lane;stored:=cb_store_be32(out[..],20,cast(u32)seq);
    return out;
}
```

A frame is a 12-byte header and a sealed body. The body is the plaintext
encrypted with XChaCha20-Poly1305 (`coax/lib/crypto`) under the direction's
key, with the header as associated data, so a changed header fails the tag
check just as a changed ciphertext does.

<!-- source: psionic/lib/peer/link.coax frame -->
```jai
// A frame: "P" "M", version 0, the lane, a 32-bit sequence number and the
// 32-bit length of the sealed body, all big-endian; then the body, the
// ciphertext and its 16-byte Poly1305 tag.
PP_HEADER :: 12;
PP_TAG :: 16;
PP_TEXT_MAX :: 4096;                          // XChaCha20-Poly1305's limit here
PP_FRAME_MAX :: PP_HEADER+PP_TEXT_MAX+PP_TAG;  // 4124 bytes
PP_SEQ_MAX :: 4294967295;
PpOpenCode :: enum u8 {ok;short;magic;version;lane;oversized;length;replay;space;forged;}
PpOpened :: struct {code:PpOpenCode;lane:int;seq:int;n:int;}
// One end of a connection: its own and its peer's public key, a key for
// each direction and, per lane, the next sequence number to send and to
// accept.
PpSession :: struct {ok:bool;me:[32]u8;peer:[32]u8;send:[32]u8;recv:[32]u8;sent:[2]int;seen:[2]int;}
```

By hand: sealing the two bytes `hi` on lane 1 as the first frame of that
lane writes `50 4d` ("PM"), version `00`, lane `01`, sequence
`00 00 00 00` and body length `00 00 00 12` (18: two bytes of ciphertext and
the 16-byte tag). The frame is 12 + 2 + 16 = 30 bytes. The nonce is the lane
byte, 19 zero bytes and the sequence number. Because each direction has its
own key and sequence numbers only rise, a key and nonce pair is never used
twice.

<!-- source: psionic/lib/peer/link.coax seal -->
```jai
// Seal text on a lane into out; the frame's length, or -1. Text longer
// than PP_TEXT_MAX is refused by xcp_seal itself.
pp_seal :: (s:[]PpSession,lane:int,text:[]u8,out:[]u8)->int {
    if s.count!=1 || !s[0].ok || lane<0 || lane>1 {return -1;}
    total:=PP_HEADER+text.count+PP_TAG;seq:=s[0].sent[lane];
    if out.count<total || seq>=PP_SEQ_MAX {return -1;}
    out[0]='P';out[1]='M';out[2]=0;out[3]=cast(u8)lane;
    a:=cb_store_be32(out,4,cast(u32)seq);b:=cb_store_be32(out,8,cast(u32)(text.count+PP_TAG));
    nonce:=pp_nonce(lane,seq);key:=s[0].send;
    if !xcp_seal(key[..],nonce[..],out[0..<PP_HEADER],text,out[PP_HEADER..<total],false) {return -1;}
    s[0].sent[lane]=seq+1;return total;
}
// Open one frame into out. Every refusal leaves the session unchanged.
pp_open :: (s:[]PpSession,frame:[]u8,out:[]u8)->PpOpened {
    if frame.count<PP_HEADER {return PpOpened.{code=PpOpenCode.short};}
    if frame[0]!='P' || frame[1]!='M' {return PpOpened.{code=PpOpenCode.magic};}
    if frame[2]!=0 {return PpOpened.{code=PpOpenCode.version};}
    lane:=cast(int)frame[3];if lane>1 {return PpOpened.{code=PpOpenCode.lane};}
    seq:=cast(int)cb_load_be32(frame,4).value;body:=cast(int)cb_load_be32(frame,8).value;
    if body>PP_TEXT_MAX+PP_TAG {return PpOpened.{code=PpOpenCode.oversized,lane=lane,seq=seq};}
    if body<PP_TAG || frame.count!=PP_HEADER+body {return PpOpened.{code=PpOpenCode.length,lane=lane,seq=seq};}
    if s.count!=1 || !s[0].ok || seq!=s[0].seen[lane] {return PpOpened.{code=PpOpenCode.replay,lane=lane,seq=seq};}
    n:=body-PP_TAG;if out.count<n {return PpOpened.{code=PpOpenCode.space,lane=lane,seq=seq};}
    nonce:=pp_nonce(lane,seq);key:=s[0].recv;
    if !xcp_open(key[..],nonce[..],frame[0..<PP_HEADER],frame[PP_HEADER..<frame.count],out[0..<n],false) {return PpOpened.{code=PpOpenCode.forged,lane=lane,seq=seq};}
    s[0].seen[lane]=seq+1;
    return PpOpened.{code=PpOpenCode.ok,lane=lane,seq=seq,n=n};
}
```

`pp_open` checks the header in a fixed order and changes nothing unless the
frame opens: too short for a header (`short`), the wrong magic or version,
a lane other than 0 or 1, a body longer than 4,096 + 16 bytes
(`oversized`), a frame whose size does not match its header (`length`), a
sequence number other than the next one expected on that lane (`replay`),
too little room for the plaintext (`space`), and a tag that does not match
(`forged`).

## Direct and relayed paths

The wires are simulated: a frame arrives a fixed delay after it is sent, in
order, and a wire holds at most four frames. A direct route is one wire each
way. A relayed route is two wires each way with a relay between them. The
relay is a procedure that takes no key: it moves the bytes from one wire to
the next unchanged and keeps a copy of the last frame it saw.

<!-- source: psionic/lib/peer/link.coax wire -->
```jai
// A simulated wire: frames arrive delay microseconds after they are sent,
// in order. A route is one wire each way (direct) or two each way with a
// relay between them. The relay is a procedure that takes no key: it moves
// bytes from one wire to the next and keeps a copy of the last frame it saw.
PP_SLOTS :: 4;
PpPath :: enum u8 {direct;relayed;}
PpSlot :: struct {at:int;n:int;bytes:[PP_FRAME_MAX]u8;}
PpWire :: struct {delay:int;first:int;count:int;slots:[PP_SLOTS]PpSlot;}
PpRoute :: struct {path:PpPath;wires:[2][2]PpWire;relayed:int;relay_bytes:int;seen_n:int;seen:[PP_FRAME_MAX]u8;}
PpTaken :: struct {ok:bool;at:int;n:int;}

pp_put :: (w:[]PpWire,now:int,bytes:[]u8)->bool {
    if w.count!=1 || w[0].count>=PP_SLOTS || bytes.count>PP_FRAME_MAX {return false;}
    i:=(w[0].first+w[0].count)%PP_SLOTS;w[0].slots[i].at=now+w[0].delay;w[0].slots[i].n=bytes.count;
    for k:0..<bytes.count {w[0].slots[i].bytes[k]=bytes[k];}
    w[0].count+=1;return true;
}
pp_take :: (w:[]PpWire,out:[]u8)->PpTaken {
    if w.count!=1 || w[0].count==0 {return PpTaken.{};}
    i:=w[0].first;n:=w[0].slots[i].n;if out.count<n {return PpTaken.{};}
    for k:0..<n {out[k]=w[0].slots[i].bytes[k];}
    w[0].first=(i+1)%PP_SLOTS;w[0].count-=1;
    return PpTaken.{ok=true,at=w[0].slots[i].at,n=n};
}
// direct: hop is the one delay; relayed: hop and relay are the two legs.
pp_route :: (r:[]PpRoute,path:PpPath,hop:int,relay:int) {
    r[0]=PpRoute.{path=path};
    for d:0..<2 {r[0].wires[d][0].delay=hop;r[0].wires[d][1].delay=relay;}
}
pp_relay :: (r:[]PpRoute,from:[]PpWire,to:[]PpWire)->bool {
    t:=pp_take(from,r[0].seen[..]);if !t.ok {return false;}
    r[0].seen_n=t.n;r[0].relayed+=1;r[0].relay_bytes+=t.n;
    return pp_put(to,t.at,r[0].seen[0..<t.n]);
}
// Send on direction d (0 from the dialer, 1 back), then deliver.
pp_send :: (r:[]PpRoute,d:int,now:int,bytes:[]u8)->bool {return pp_put(r[0].wires[d][0..<1],now,bytes);}
pp_deliver :: (r:[]PpRoute,d:int,out:[]u8)->PpTaken {
    if r[0].path==PpPath.relayed && !pp_relay(r,r[0].wires[d][0..<1],r[0].wires[d][1..<2]) {return PpTaken.{};}
    leg:=ifx r[0].path==PpPath.relayed then 1 else 0;
    return pp_take(r[0].wires[d][leg..<leg+1],out);
}
```

A connection's round trip is measured, not configured: one end sends a ping
on the control lane with its clock, the other echoes those 8 bytes in a
pong, and the round trip is the pong's arrival time less the echoed clock.

<!-- source: psionic/lib/peer/link.coax ping -->
```jai
// Measure a connection: a pings b on the control lane with its clock, b
// echoes the 8 bytes in a pong, and the round trip is the pong's arrival
// time less the echoed clock.
PpConn :: struct {ok:bool;path:PpPath;rtt:int;}
PP_PING :: PP_HEADER+9+PP_TAG;      // 37 bytes
// b's side: open a ping and seal the pong into out; its length, or -1.
pp_echo :: (b:[]PpSession,got:[]u8,out:[]u8)->int {
    plain:[9]u8;o:=pp_open(b,got,plain[..]);
    if o.code!=PpOpenCode.ok || o.lane!=0 || o.n!=9 || plain[0]!=cast(u8)PpKind.ping {return -1;}
    plain[0]=cast(u8)PpKind.pong;return pp_seal(b,0,plain[..],out);
}
// a's side: open the pong that arrived at `at`; the round trip, or -1.
pp_rtt :: (a:[]PpSession,got:[]u8,at:int)->int {
    plain:[9]u8;o:=pp_open(a,got,plain[..]);
    if o.code!=PpOpenCode.ok || o.lane!=0 || o.n!=9 || plain[0]!=cast(u8)PpKind.pong {return -1;}
    return at-cast(int)cb_load_be64(plain[..],1).value;
}
pp_ping :: (a:[]PpSession,b:[]PpSession,r:[]PpRoute,now:int)->PpConn {
    text:[9]u8;frame:[PP_PING]u8;got:[PP_PING]u8;   // text[0] is 0, PpKind.ping
    s:=cb_store_be64(text[..],1,cast(u64)now);
    if pp_seal(a,0,text[..],frame[..])!=PP_PING || !pp_send(r,0,now,frame[..]) {return PpConn.{};}
    t:=pp_deliver(r,0,got[..]);
    if !t.ok || pp_echo(b,got[0..<t.n],frame[..])!=PP_PING || !pp_send(r,1,t.at,frame[..]) {return PpConn.{};}
    t=pp_deliver(r,1,got[..]);if !t.ok {return PpConn.{};}
    rtt:=pp_rtt(a,got[0..<t.n],t.at);if rtt<0 {return PpConn.{};}
    return PpConn.{ok=true,path=r[0].path,rtt=rtt};
}
```

By hand: the head reaches stage A directly with 400 µs each way, so the
round trip is 2 × 400 = 800 µs. It reaches stage B through a relay with legs
of 9,000 and 12,000 µs, so the round trip is 2 × (9,000 + 12,000) = 42,000
µs. A ping or pong frame carries 9 plaintext bytes: 12 + 9 + 16 = 37 bytes
on the wire, so the relay forwards 2 frames and 74 bytes. The transcript
agrees.

## Join a private mesh

The owner of mesh `lab` gives each invited peer an invitation: a MAC under
the mesh's admission key, which the head holds, over the mesh's name, the
invitee's public key and an expiry. The peer sends it as a `join` message on
its control lane. The head recomputes the MAC and compares it in constant
time, then applies the join rule:

<!-- source: psionic/lib/peer/link.coax invite -->
```jai
// An invitation to private mesh `mesh` for the peer with key invitee,
// valid before expires: HMAC-SHA256 under the mesh's admission key of
// "psionic-invite-v0" 0 mesh 0 invitee expires (8 bytes, big-endian).
// The join message carries kind, the mesh's length and name, the invitee,
// expires and the MAC; the admitter recomputes the MAC and compares it in
// constant time, then asks policy.coax's pp_join.
PP_MESH_MAX :: 64;
pp_invite_mac :: (key:[]u8,mesh:string,invitee:[]u8,expires:int,out:[]u8)->bool {
    msg:[17+1+PP_MESH_MAX+1+32+8]u8;tag:="psionic-invite-v0";
    if mesh.count<1 || mesh.count>PP_MESH_MAX || invitee.count!=32 || expires<0 {return false;}
    for i:0..<17 {msg[i]=tag[i];}
    at:=18;for i:0..<mesh.count {msg[at+i]=mesh[i];}at+=mesh.count+1;
    for i:0..<32 {msg[at+i]=invitee[i];}at+=32;
    s:=cb_store_be64(msg[..],at,cast(u64)expires);at+=8;
    return hmac_sha256_checked(key,msg[0..<at],out,false)==SHA_OK;
}
// The join message; its length, or -1.
pp_join_text :: (mesh:string,invitee:[]u8,expires:int,mac:[]u8,out:[]u8)->int {
    n:=2+mesh.count+32+8+32;
    if mesh.count<1 || mesh.count>PP_MESH_MAX || invitee.count!=32 || mac.count!=32 || expires<0 || out.count<n {return -1;}
    out[0]=cast(u8)PpKind.join;out[1]=cast(u8)mesh.count;
    for i:0..<mesh.count {out[2+i]=mesh[i];}
    at:=2+mesh.count;for i:0..<32 {out[at+i]=invitee[i];out[at+40+i]=mac[i];}
    s:=cb_store_be64(out,at+32,cast(u64)expires);return n;
}
// Admit a join: text is the opened plaintext (empty for no invitation),
// peer the session's authenticated key. Anything shorter than a join with a
// one-byte mesh name, or not exactly as long as its name says, is no
// invitation at all.
pp_check_join :: (private:bool,mesh:string,key:[]u8,peer:[]u8,text:[]u8,now:int)->PpJoin {
    if text.count<75 || text[0]!=cast(u8)PpKind.join || text.count!=74+cast(int)text[1] {return pp_join(private,mesh,cast(string)peer,PpInvite.{},now);}
    at:=2+cast(int)text[1];expires:=cast(int)cb_load_be64(text,at+32).value;want:[32]u8;
    ok:=pp_invite_mac(key,cast(string)text[2..<at],text[at..<at+32],expires,want[..]) && cb_ct_equal(want[..],text[at+40..<at+72])!=0;
    inv:=PpInvite.{present=true,mesh=cast(string)text[2..<at],invitee=cast(string)text[at..<at+32],expires=expires,mac_ok=ok};
    return pp_join(private,mesh,cast(string)peer,inv,now);
}
```

<!-- source: psionic/lib/peer/policy.coax join -->
```jai
// Joining a mesh. A public mesh admits any authenticated peer; a private
// one only a peer holding an invitation for this mesh, for this peer's key,
// unexpired, whose MAC the mesh's admitter recomputed and matched.
PpJoin :: enum u8 {ok;invitation;mesh;invitee;expired;forged;}
PpInvite :: struct {present:bool;mesh:string;invitee:string;expires:int;mac_ok:bool;}

pp_join :: (private:bool,mesh:string,peer:string,inv:PpInvite,now:int)->PpJoin #export_c {
    if !private {return PpJoin.ok;}
    if !inv.present {return PpJoin.invitation;}
    if inv.mesh!=mesh {return PpJoin.mesh;}
    if inv.invitee!=peer {return PpJoin.invitee;}
    if now>=inv.expires {return PpJoin.expired;}
    if !inv.mac_ok {return PpJoin.forged;}
    return PpJoin.ok;
}
```

The invitee in the join message is compared with the key the session
authenticated, so an invitation is useless to anyone else. In the
transcript, stages A and B join. The outsider is refused without an
invitation (`invitation`), with stage A's invitation (`invitee`), and with
stage A's invitation edited to carry its own key (`forged`: the MAC covers
the key). Stage A's own invitation, presented at its expiry time, is
`expired`. A public mesh skips the invitation: `pp_join` admits any
authenticated peer.

## Two lanes

Every message's first plaintext byte is its kind, and every kind belongs to
one lane. `ping`, `pong`, `join`, `welcome`, `drain` and `resume` are
control; `activation` and `tokens` are inference. `drain` and `resume`
change what a peer serves, so only the mesh owner's key may send them.
Inference traffic is accepted only from a peer that has joined.

<!-- source: psionic/lib/peer/policy.coax lanes -->
```jai
// Two lanes per connection. The owner's commands travel on control, model
// traffic on inference; a message whose kind belongs to the other lane is
// refused, never rerouted.
PpLane :: enum u8 {control;inference;}
PpKind :: enum u8 {ping;pong;join;welcome;drain;resume;activation;tokens;}
PP_KINDS :: 8;
PpVerdict :: enum u8 {ok;kind;lane;owner;member;}

pp_lane_of :: (kind:int)->int {if kind>=6 {return 1;}return 0;}
// Drain and resume change what a peer serves: only its owner sends them.
pp_owner_only :: (kind:int)->bool {return kind==4 || kind==5;}
// A message as the receiver sees it: the lane byte from the frame header,
// the kind byte from the opened plaintext, whether the sender's key is the
// mesh owner's and whether the sender has joined.
pp_admit :: (lane:int,kind:int,from_owner:bool,joined:bool)->PpVerdict #export_c {
    if kind<0 || kind>=PP_KINDS {return PpVerdict.kind;}
    if pp_lane_of(kind)!=lane {return PpVerdict.lane;}
    if pp_owner_only(kind) && !from_owner {return PpVerdict.owner;}
    if lane==1 && !joined {return PpVerdict.member;}
    return PpVerdict.ok;
}
```

The lane comes from the frame header, which the tag authenticates, and the
kind from the opened plaintext. In the transcript, the activation and the
tokens pass; the owner's `drain` passes on control. The head's `drain` is
refused with `owner`; the owner's `drain` on the inference lane and an
activation on the control lane are refused with `lane`; the outsider's
activation before joining is refused with `member`. Inspecting what each
stage accepted per lane shows the separation: stage A's inference lane holds
only the activation, and stage B's control lane holds only the owner's
drain.

The relay carried 5 frames and 253 bytes: the two ping frames (74 bytes),
stage B's join (77 plaintext bytes, a 105-byte frame), the head's refused
drain (29 bytes) and the activation (17 plaintext bytes, 45 bytes). The 16
activation bytes do not appear in the relay's copy of that frame: a relay
forwards ciphertext only.

## Refusals

The last five lines send stage B frames it must refuse:

- a frame cut by one byte: `length`;
- a header that claims 5,000 body bytes: `oversized`;
- one bit of the ciphertext flipped on the way to the relay: `forged`;
- the untouched frame then opens: `ok`;
- the same frame again: `replay`.

None of the refusals moved stage B's sequence numbers, which is why the
untouched frame still opens.

## How it is checked

The checks come before the code, in [`policy.coax`](../lib/peer/policy.coax)
and [`link.coax`](../lib/peer/link.coax), with fixtures in
[`peer-checks.coax`](../tests/peer-checks.coax) and
[`peer-link-checks.coax`](../tests/peer-link-checks.coax).

- **Public keys** against BIP-340's published vectors 0 to 3.
- **Session keys** against an oracle: NIP-44's conversation key, then one
  HMAC-SHA256 over the info string and the byte 1, which is what HKDF-Expand
  computes for 32 bytes. Both ends' keys mirror each other, and a session
  between two other peers cannot open the frame (`forged`).
- **A frame by hand**: the 12 header bytes above, and the body equal to
  `xcp_seal` of the same inputs.
- **Round trips** (`#forall`): frames of 0 to 37 bytes on both lanes, three
  in a row each way, opened in order; each frame sent again is a `replay`.
- **Refusals**: every header field, every length edge (4,112 body bytes
  allowed, 4,113 not), every body byte flipped, a frame moved to the other
  lane, a frame reflected to its sender, and a session left unchanged by all
  of them.
- **Wires and relay**: delivery times, the four-frame limit, a full wire on
  either leg.
- **Pings**: 800 µs direct and 42,000 µs relayed, as worked above; a pong
  answers only a 9-byte ping on the control lane, and a round trip is read
  only from a 9-byte pong on that lane.
- **Invitations**: the MAC against HMAC-SHA256 over the message laid out by
  hand, the join message's byte layout, and every join verdict.
- **Policy**: `pp_admit` against its whole table over lanes, kinds, owner
  and joined (`#forall`); `pp_join` over all 32 combinations of its five
  conditions; `pg_route` for every head refusal, every stage field, freshness
  at its edges, overlapping, missing, short and long layer ranges.

`coax mutate --root-only` changes each file's own code one site at a time
and reruns the checks. `policy.coax`, the `#authority` file: 82 of 82
mutants caught. `link.coax`: 414 of 414.

## What remains

- **Real sockets.** Frames travel over simulated wires in one process.
  There is no QUIC or TCP transport, no NAT traversal and no real relay
  service; the measured round trips are of the simulated clock.
- **Forward secrecy.** Session keys come from the two peers' long-term keys
  (static-static ECDH). A leaked secret key exposes past traffic. Mesh-LLM's
  QUIC handshake uses ephemeral keys; this transport does not yet.
- **Signed invitations.** An invitation is a MAC under the admitter's mesh
  key, so only a holder of that key can check it. A Schnorr-signed
  invitation that any member could check waits on the signer
  (`coax/lib/crypto/schnorr_sign.coax`) running in this toolchain.
- **Reading v2 group beacons.** `pg_route` takes the fields a reader
  extracted. Chapter 24's beacon reader refuses v2 bodies and the `group`
  field; a v2 reader, and fetching beacons from relays, are not built here.
- **A published wire contract.** The frame layout is
  `psionic-peer-loopback-v0`, an in-process profile. Peers on separate
  machines need a versioned schema in `proto/` and the protocol spec first.
- Encryption protects the link only. A peer that serves a request reads it;
  [chapter 194](ch194-who-can-read-the-prompt.md) covers that.

## Sources

- [Chapter plan](PLAN.md), Part XX.
- [Inference across a mesh](../docs/serving/mesh.md).
- [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md), "Model groups".
- [Chapter 24](ch024-a-pylon-answers-decisions.md): the Pylon's key and
  beacon freshness.
- [Chapter 25](ch025-the-api-fans-out.md): beacon expiry and generations.
- [Chapter 27](ch027-a-public-mesh.md): authenticated public access.
- [NIP-44](../../nostr/nips/official/44.md): the conversation key.
