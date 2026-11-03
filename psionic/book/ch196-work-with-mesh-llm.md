# 196. Work with Mesh-LLM

> **Status: partly implemented.** A Psionic buyer works with a simulated Mesh-LLM peer in one process: it maps the peer's gossip to an honest capability claim and a one-stage route, sends System One requests through a byte-exact translation, re-runs each answer and compares it, meters the reported usage, prices it by Mesh-LLM's own bill rules and decides payment in chapter 238's ledger. Live interop with real Mesh-LLM nodes remains: decoding protobuf gossip, QUIC transport, the plugin endpoint, the OpenAI-compatible path and the payment exchange itself; see [What remains](#what-remains).

[Mesh-LLM](https://github.com/Mesh-LLM/mesh-llm) already runs public
meshes. Its nodes gossip which models they serve, route whole requests to a
peer that holds a model, and sell inference for Lightning payments. This
chapter is about working with those nodes without adopting their engine.
Mesh-LLM computes with a patched llama.cpp runtime. Psionic's arithmetic
stays in Coax, and a Mesh-LLM peer is treated like any other peer: what it
advertises is a claim, not evidence; what it answers is checked; what it
bills is metered and paid only after the check.

Mesh-LLM was read at revision
[`f2a7f62`](https://github.com/Mesh-LLM/mesh-llm/tree/f2a7f62c755e283c389a28cbf8d1e8f926677a4e)
as untrusted source. The parts this chapter relies on are:

- the gossip schema, `mesh/crates/mesh-llm-protocol/proto/node.proto`:
  `PeerAnnouncement`, `ServedModelDescriptor`, `ModelRuntimeDescriptor`,
  `CapabilityLevel`, `InferenceAdmissionState` and `LightningOffer`;
- the System One API page, `mesh/website/src/docs/pages/system-one-api.md`:
  `POST /systemone` with `{model, state, questions}` in and
  `{model, answers, usage}` out;
- the metered payment rules, `mesh/docs/specs/lightning-payments.md` and
  `mesh/crates/mesh-llm-payments-types/src/{pricing,metering}.rs`;
- the wire-byte commitments and their published test vectors,
  `mesh/docs/plugins/openai-wire-byte-transcripts.md`.

None of its code runs here. The parts below are reimplemented in Coax from
those documents, each checked against values the documents themselves
publish.

One `#authority` file holds every decision:
[`peer/meshllm.coax`](../lib/peer/meshllm.coax). It loads chapter 238's
[`peer/pay.coax`](../lib/peer/pay.coax) (and through it chapter 195's
checker and chapter 180's prices), chapter 188's
[`peer/policy.coax`](../lib/peer/policy.coax) for routes, chapter 173's
[`serve-capability.coax`](../lib/serve-capability.coax) for claims and
evidence, and chapter 180's [`serve-meter.coax`](../lib/serve-meter.coax)
for usage records. It holds no key, opens no socket and calls no wallet.

```shell
coax/bin/coax run psionic/examples/meshllm-loopback.coax
coax/bin/coax test psionic/lib/peer/meshllm.coax
coax/bin/coax mutate psionic/lib/peer/meshllm.coax --root-only
```

## What a peer advertises

A buyer reads a peer's gossip and keeps a small part of it. The types name
the protobuf fields they come from. The decoded form is this chapter's
in-process profile: decoding the protobuf bytes of a live `GossipFrame` is
not built.

<!-- source: psionic/lib/peer/meshllm.coax types -->
```jai
ML_REVISION :: "f2a7f62c755e283c389a28cbf8d1e8f926677a4e";
// The numerical profile a Mesh-LLM peer is named under. Its arithmetic is a
// patched llama.cpp at this revision, never one of Psionic's own profiles.
ML_PROFILE :: "mesh-llm-f2a7f62";
// node.proto's NodeRole, InferenceAdmissionState and CapabilityLevel.
MlRole :: enum u8 {unspecified;worker;host;client;}
MlAdmission :: enum u8 {unspecified;accepting;deprioritized;remote_paused;all_paused;}
MlLevel :: enum u8 {unspecified;none;likely;supported;}
// One served model as the buyer decoded it from a peer's gossip:
// ServedModelDescriptor's identity.model_name, identity.identity_hash,
// capabilities.system_one and metadata.layer_count, and the
// ModelRuntimeDescriptor's identity_hash and ready for the same model.
MlModel :: struct {name:string;identity:string;system_one:MlLevel;layers:int;runtime_identity:string;ready:bool;}
// The peer: its 32-byte endpoint_id in hex, role, mesh_id, the GossipFrame's
// gen, inference_admission_state, how many entries hardware.gpus lists, and
// the one model this mapping is about.
MlPeer :: struct {endpoint:string;role:MlRole;mesh:string;gen:int;admission:MlAdmission;gpus:int;model:MlModel;}
// A LightningOffer: msat per million input and output tokens, the cached
// input discount and the payment protocol (3 is metered v3).
MlOffer :: struct {model:string;input:int;output:int;cache:int;protocol:int;}
MlRefuse :: enum u8 {none;endpoint;role;admission;name;identity;runtime;ready;system_one;backend;mesh;layers;offer;protocol;rate;discount;usage;deposit;request;status;length;json;model;answers;}
MlClaim :: struct {refuse:MlRefuse;claim:CpClaim;}
```

Every Mesh-LLM peer is named under one numerical profile,
`mesh-llm-f2a7f62`. The gossip does not say how a model's arithmetic was
done, and what Mesh-LLM runs is llama.cpp at that revision, never one of
Psionic's own profiles.

## From advertisement to claim

Chapter 173 publishes a capability entry only when a qualified record names
the same checkpoint digest, backend and profile, and narrows the entry to
what the record covered. A Mesh-LLM peer gets the same treatment. The
adapter turns an advertisement into a claim, and refuses an advertisement
that cannot support one:

<!-- source: psionic/lib/peer/meshllm.coax claim -->
```jai
// The claim a peer's advertisement supports, or the first reason it supports
// none. Only a worker or host that is accepting work, serving a model whose
// name is printable, whose identity hash is a SHA-256 the loaded runtime
// repeats, that is ready, that states System One support (likely is a guess)
// and that lists no GPU, maps. A node with GPUs does not say whether this
// model runs on one, so its backend is unknown and it is refused.
ml_asked :: ()->CpLimits {return CpLimits.{questions=SV_QUESTIONS_MAX,options=SV_OPTIONS_MAX,state=sv_nip().state,deadline_ms=1};}
ml_claim :: (p:MlPeer)->MlClaim #export_c {
    r:=MlClaim.{refuse=MlRefuse.endpoint};
    if !cp_hex64(p.endpoint) {return r;}
    r.refuse=MlRefuse.role;
    if p.role!=MlRole.worker && p.role!=MlRole.host {return r;}
    r.refuse=MlRefuse.admission;
    if p.admission!=MlAdmission.accepting && p.admission!=MlAdmission.deprioritized {return r;}
    m:=p.model;
    r.refuse=MlRefuse.name;
    if !cp_text(m.name,CP_NAME_MAX) {return r;}
    r.refuse=MlRefuse.identity;
    if !cp_hex64(m.identity) {return r;}
    r.refuse=MlRefuse.runtime;
    if m.runtime_identity!=m.identity {return r;}
    r.refuse=MlRefuse.ready;
    if !m.ready {return r;}
    r.refuse=MlRefuse.system_one;
    if m.system_one!=MlLevel.supported {return r;}
    r.refuse=MlRefuse.backend;
    if p.gpus!=0 {return r;}
    return MlClaim.{claim=CpClaim.{model=m.name,digest=m.identity,backend=CpBackend.cpu,profile=ML_PROFILE,limits=ml_asked()}};
}
```

The checks run in a fixed order and the first that fails names the
refusal. Three of them are about honesty rather than form:

- **System One support must be stated.** `CapabilityLevel` has
  `likely`. A model that is likely to support `/systemone` is a guess, and
  Mesh-LLM's own docs say a client should call `/systemone` only after the
  model advertises `system_one`. Only `supported` maps.
- **The loaded file must be the advertised one.** The runtime descriptor's
  `identity_hash` must repeat the served model's. The docs read do not
  define the hash's format, so the adapter accepts only 64 lowercase hex
  digits and treats them as the SHA-256 of the checkpoint. A peer that
  states anything else, or nothing, is refused rather than guessed at.
- **The backend must be knowable.** The gossip lists a node's GPUs but not
  which device a given model runs on; Laya, for example, runs on the CPU by
  default even on a GPU node. A node that lists no GPU runs on its CPU, so
  it maps to `cpu`. A node with GPUs is refused with `backend`.

The claim asks for NIP-DEC's whole range and a 1 ms deadline, because the
advertisement states no limits. That is safe only because a claim is not
an entry: without a record, chapter 173 publishes it as `unknown`, and
admission refuses every request for it.

<!-- source: psionic/lib/peer/meshllm.coax check-claim -->
```jai
#test "a peer maps to a claim, or is refused for the first thing wrong" {
    c:=ml_claim(mlt_peer());
    #assert c.refuse==MlRefuse.none && c.claim.model=="psionic-toy" && c.claim.digest==SV_TOY_SHA && c.claim.backend==CpBackend.cpu;
    #assert c.claim.profile==ML_PROFILE && c.claim.alias=="" && c.claim.memory==0 && cp_claim_valid(c.claim);
    // The peer states no limits: the claim asks for NIP-DEC's whole range and
    // the shortest deadline, and only a record can narrow it.
    #assert c.claim.limits.questions==64 && c.claim.limits.options==255 && c.claim.limits.state==131072 && c.claim.limits.deadline_ms==1;
    p:=mlt_peer();p.endpoint="xyz";#assert ml_claim(p).refuse==MlRefuse.endpoint;
    p=mlt_peer();p.role=MlRole.client;#assert ml_claim(p).refuse==MlRefuse.role;
    p=mlt_peer();p.admission=MlAdmission.remote_paused;#assert ml_claim(p).refuse==MlRefuse.admission;
    p=mlt_peer();p.model.name="";#assert ml_claim(p).refuse==MlRefuse.name;
    p=mlt_peer();p.model.identity="";p.model.runtime_identity="";#assert ml_claim(p).refuse==MlRefuse.identity;   // no hash advertised
    p=mlt_peer();p.model.runtime_identity=CP_D1;#assert ml_claim(p).refuse==MlRefuse.runtime;                       // loaded another file
    p=mlt_peer();p.model.ready=false;#assert ml_claim(p).refuse==MlRefuse.ready;
    p=mlt_peer();p.model.system_one=MlLevel.likely;#assert ml_claim(p).refuse==MlRefuse.system_one;                 // a guess is not support
    p=mlt_peer();p.gpus=1;#assert ml_claim(p).refuse==MlRefuse.backend;                                              // CPU or GPU: not stated
}
```

## From advertisement to route

Chapter 188 routes a job to a group only when a fresh head beacon and its
stage beacons tile the model's layers. Mesh-LLM's gossip names no stage
boundaries; a split inside a Mesh-LLM peer is its own business, and its
boundary is not one Psionic has checked (chapter 186's first lesson). So
the adapter maps a peer to a group of one stage holding every layer, keyed
and owned by the peer's endpoint, named by its mesh, and versioned by the
gossip generation. The buyer stamps when it read the gossip and how long it
trusts it, at most 300 s, chapter 24's window.

<!-- source: psionic/lib/peer/meshllm.coax route -->
```jai
// The peer as chapter 188's group: Mesh-LLM gossip names no stage
// boundaries, so a peer is one stage holding every layer, under the mesh's
// name, keyed and owned by its endpoint, its generation the gossip's. The
// buyer stamps when it read the gossip and how long it trusts it.
MlRoute :: struct {refuse:MlRefuse;route:PgRoute;}
ml_beacon :: (p:MlPeer,observed:int,ttl:int,role:PgRole)->PgBeacon {
    return PgBeacon.{key=p.endpoint,owner=p.endpoint,status=PbStatus.online,observed_at=observed,valid_until=observed+ttl,beacon_generation=p.gen,
        id=p.mesh,role=role,generation=p.gen,artifact=p.model.identity,readiness=PgReady.ready,free=1,from=0,to=p.model.layers};
}
ml_route :: (p:MlPeer,now:int,observed:int,ttl:int,order:[]int)->MlRoute #export_c {
    r:=MlRoute.{refuse=ml_claim(p).refuse,route=PgRoute.{code=PgCode.stages}};
    if r.refuse!=MlRefuse.none {return r;}
    r.refuse=MlRefuse.mesh;
    if !cp_text(p.mesh,CP_NAME_MAX) {return r;}
    r.refuse=MlRefuse.layers;
    if p.model.layers<1 {return r;}
    r.refuse=MlRefuse.request;
    if ttl<1 || ttl>ML_TTL_MOST || p.gen<0 {return r;}
    stage:=[1]PgBeacon.[ml_beacon(p,observed,ttl,PgRole.stage)];
    return MlRoute.{route=pg_route(ml_beacon(p,observed,ttl,PgRole.head),stage[..],p.model.identity,p.model.layers,now,order)};
}
```

`pg_route` then applies chapter 188's rules unchanged. A peer read 61 s ago
under a 60 s lifetime is stale and has no route:

<!-- source: psionic/lib/peer/meshllm.coax check-route -->
```jai
#test "a whole-model peer is a one-stage group" {
    order:[1]int;r:=ml_route(mlt_peer(),PG_NOW,PG_NOW,60,order[..]);
    #assert r.refuse==MlRefuse.none && r.route.code==PgCode.ok && r.route.stages==1 && order[0]==0;
    // Seen 61 s ago with a 60 s lifetime: stale, so no route.
    r=ml_route(mlt_peer(),PG_NOW,PG_NOW-61,60,order[..]);#assert r.refuse==MlRefuse.none && r.route.code==PgCode.stale;
    r=ml_route(mlt_peer(),PG_NOW,PG_NOW-60,60,order[..]);#assert r.route.code==PgCode.ok;
```

## What a peer charges

A `LightningOffer` names msat per million input and output tokens, a cached
input discount and a payment protocol. Mesh-LLM bills reported usage, not a
quote: input is uncached tokens plus cached tokens at the discounted rate,
rounded up once to the msat; output is rounded up on its own. A cold buyer
first pays a deposit (1 sat by default) that is kept even when it exceeds
the bill; the final invoice is the bill less the deposit, exact to the
msat, with a positive remainder under 1 sat raised to 1 sat. Offers on a
payment protocol other than 3 (metered v3) are refused, as Mesh-LLM's own
buyers refuse them.

<!-- source: psionic/lib/peer/meshllm.coax bill -->
```jai
// Mesh-LLM's bill at revision f2a7f62 (pricing.rs, metering.rs): input is
// uncached tokens plus cached tokens at the discounted rate, rounded up once
// to the msat; output is rounded up on its own. The final invoice is the
// bill less the deposit, exact to the msat, with a positive remainder below
// the floor raised to it. A paid offer needs protocol 3 and positive rates.
MlBill :: struct {refuse:MlRefuse;msat:int;}
ml_offer :: (o:MlOffer,model:string)->MlRefuse {
    if o.model!=model {return MlRefuse.offer;}
    if o.protocol!=3 {return MlRefuse.protocol;}
    if !bl_in(o.input-1,ML_RATE_MOST-1) || !bl_in(o.output-1,ML_RATE_MOST-1) {return MlRefuse.rate;}
    if !bl_in(o.cache,100) {return MlRefuse.discount;}
    return MlRefuse.none;
}
ml_bill :: (o:MlOffer,model:string,prompt:int,cached:int,completion:int)->MlBill #export_c {
    r:=MlBill.{refuse=ml_offer(o,model)};
    if r.refuse!=MlRefuse.none {return r;}
    r.refuse=MlRefuse.usage;
    if !bl_in(prompt,BL_TOKENS_MOST) || !bl_in(cached,prompt) || !bl_in(completion,BL_TOKENS_MOST) {return r;}
    weighted:=(prompt-cached)*100+cached*(100-o.cache);
    return MlBill.{msat=bl_ceil(o.input*weighted,100000000)+bl_ceil(o.output*completion,1000000)};
}
// -1 for a floor outside 1..1000000 msat, a deposit above 1000000 msat or a
// bill past 2^53 msat, the largest integer a wire report carries here.
ml_final :: (bill:int,deposit:int,least:int)->int #export_c {
    if !bl_in(least-1,ML_FLOOR_MOST-1) || !bl_in(bill,JSON_INT_MOST) || !bl_in(deposit,ML_FLOOR_MOST) {return -1;}
    rest:=bill-deposit;
    if rest<=0 {return 0;}
    return cp_max(rest,least);
}
// What a buyer pays for one request: the deposit (0 for a peer that already
// trusts it), which is kept even when it exceeds the bill, plus the final
// invoice at the default 1 sat floor.
MlCost :: struct {refuse:MlRefuse;bill:int;deposit:int;final:int;msat:int;}
ml_cost :: (o:MlOffer,model:string,prompt:int,completion:int,deposit:int)->MlCost #export_c {
    b:=ml_bill(o,model,prompt,0,completion);
    if b.refuse!=MlRefuse.none {return MlCost.{refuse=b.refuse};}
    f:=ml_final(b.msat,deposit,ML_FLOOR);
    if f<0 {return MlCost.{refuse=MlRefuse.deposit};}
    return MlCost.{bill=b.msat,deposit=deposit,final=f,msat=deposit+f};
}
```

The check below is Mesh-LLM's own: the values in its `pricing.rs` and
`metering.rs` tests, rerun against this reimplementation. It is the
independent check for the bill.

<!-- source: psionic/lib/peer/meshllm.coax check-bill -->
```jai
#test "Mesh-LLM's bill, against its own published checks" {
    // pricing.rs: 1000000 and 2000000 msat per million, 75 % off cached input.
    o:=MlOffer.{model="m",input=1000000,output=2000000,cache=75,protocol=3};
    #assert ml_bill(o,"m",10,8,0).msat==4 && ml_bill(o,"m",0,0,10).msat==20 && ml_bill(o,"m",1,1,0).msat==1;
    #assert ml_bill(o,"m",1,2,0).refuse==MlRefuse.usage;
    o.cache=100;#assert ml_bill(o,"m",10,10,0).msat==0;
    o.cache=0;#assert ml_bill(o,"m",10,8,0).msat==10;
    o.cache=101;#assert ml_bill(o,"m",10,8,0).refuse==MlRefuse.discount;
    // Fractional rates round once, up, to the msat.
    f:=MlOffer.{model="m",input=500,output=1500,protocol=3};
    #assert ml_bill(f,"m",1000,0,0).msat==1 && ml_bill(f,"m",0,0,1000).msat==2 && ml_bill(f,"m",0,0,0).msat==0 && ml_bill(f,"m",1000000,0,0).msat==500;
    f.input=1000001;#assert ml_bill(f,"m",1000,0,0).msat==1001;
    // metering.rs: the final invoice after the deposit, and its 1 sat floor.
    // Each row: bill, deposit applied, floor, final invoice.
    want:=[60]int.[0,0,1000,0, 1,0,1000,1000, 1001,1000,1000,1000, 1000,1000,1000,0, 499,0,1000,1000, 500,0,1000,1000,
        999,0,1000,1000, 1000,0,1000,1000, 1499,0,1000,1499, 4045,0,1000,4045, 1499,1000,1000,1000, 1500,1000,1000,1000,
        1640,1000,1000,1000, 2640,1000,1000,1640, 500,1000,1000,0];
    for i:0..<15 {#assert ml_final(want[4*i],want[4*i+1],want[4*i+2])==want[4*i+3];}
    #assert ml_final(1,0,2000)==2000 && ml_final(2500,1000,2000)==2000 && ml_final(4000,2000,2000)==2000 && ml_final(1500,0,1)==1500;
}
```

Integers read from a wire report are bounded by 2^53, so a bill past
2^53 msat (about 90,000 bitcoin) is refused.

## Work it by hand

The example's peer offers 2,000,000 msat per million input tokens (2 msat a
token) and 8,000,000 per million output tokens, with no cache discount. Its
System One answers report one input token per state byte and no output
tokens.

- **The quote.** The buyer caps a request at 4096 input tokens. The bill
  for that is ceil(2,000,000 x 4096 x 100 / 100,000,000) = **8192 msat**. A
  1000 msat deposit leaves a final invoice of 7192, so the worst a request
  costs is 1000 + 7192 = **8192 msat**. Each job holds that much in
  chapter 238's ledger.
- **Job 1.** The state is "The kettle is cold.", 19 bytes, so 19 tokens:
  a bill of ceil(2,000,000 x 1900 / 100,000,000) = **38 msat**. The buyer is
  cold, so it pays the 1000 msat deposit; 38 - 1000 is negative, so there
  is no final invoice. Job 1 costs **1000 msat**.
- **Job 2.** "The tea is ready." is 17 bytes: a bill of **34 msat**. The
  buyer is now warm and pays no deposit. 34 msat is positive and under the
  floor, so the final invoice is raised to **1000 msat**.

So a buyer who sends small System One requests pays about 1 sat each,
whatever the rate: Lightning's 1 sat floor dominates.

The buyer re-runs job 1 and gets the same answers bytes: the peer is paid
1000 msat. Job 2's peer changes one digit of a probability: the bytes
differ, the checker says `disagree`, and the 1000 msat is withheld. The
ledger ends with nothing in escrow, 1000 msat owed in one intent and 1000
msat withheld.

<!-- source: psionic/lib/peer/meshllm.coax check-cost -->
```jai
#test "what a cold buyer pays: the deposit, then the rest" {
    o:=mlt_offer();
    // 22 input tokens at 2 msat each is 44 msat. The 1000 msat deposit covers
    // it and is kept, so the buyer pays 1000 msat; a warm buyer pays 44 msat
    // raised to the 1 sat floor.
    c:=ml_cost(o,"psionic-toy",22,0,1000);#assert c.refuse==MlRefuse.none && c.bill==44 && c.deposit==1000 && c.final==0 && c.msat==1000;
    c=ml_cost(o,"psionic-toy",22,0,0);#assert c.bill==44 && c.final==1000 && c.msat==1000;
    c=ml_cost(o,"psionic-toy",4096,0,1000);#assert c.bill==8192 && c.final==7192 && c.msat==8192;
    c=ml_cost(o,"psionic-toy",600,0,1000);#assert c.bill==1200 && c.final==1000 && c.msat==2000;   // 200 left, raised to the floor
}
```

## The wire

Mesh-LLM commits to request and response bytes with SHA-256 over the HTTP
entity, excluding headers and transfer framing. Its docs publish two
vectors: a 35-byte request and an SSE transcript. The adapter's commitment
reproduces both, and the buyer keys each job in chapter 238's ledger by its
request's commitment.

<!-- source: psionic/lib/peer/meshllm.coax check-wire -->
```jai
#test "Mesh-LLM's published wire-byte vectors" {
    hex:[64]u8;
    #assert ml_commit("{ \"model\": \"tiny\", \"stream\":true }\n",hex[..])=="61b54b4afd933a3dad0f1854e1ee723d92e10e0289a84d14079fefe34b99921e";
    #assert ml_commit("data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n",hex[..])=="2774745745fc204699d02f6f2d032c205fd17caf18070316bcfbbb16956f66fd";
}
```

A request the buyer admitted goes out on Mesh-LLM's path. Psionic serves
System One at `/v1/systemone`; Mesh-LLM serves it at `/systemone`, with the
same `{model, state, questions}` body. The body's bytes go out unchanged, so
both sides compute the same commitment:

<!-- source: psionic/lib/peer/meshllm.coax wire -->
```jai
// Mesh-LLM's wire_bytes commitment: SHA-256 over the HTTP entity bytes, in
// lowercase hex. The buyer keys a job by its request's commitment.
ml_commit :: (entity:string,out:[]u8)->string {
    sha:[112]u8;d:[32]u8;chunk:[64]u8;#assert sha256_init(sha[..])==SHA_OK;
    at:=0;
    while at<entity.count {
        n:=0;
        while n<64 && at+n<entity.count {chunk[n]=entity[at+n];n+=1;}
        #assert sha256_update_checked(sha[..],chunk[0..<n],false)==SHA_OK;at+=n;
    }
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    digits:="0123456789abcdef";
    for i:0..<32 {out[2*i]=digits[cast(int)(d[i]>>4)];out[2*i+1]=digits[cast(int)(d[i]&15)];}
    return cast(string)out[0..<64];
}
// A System One body the buyer admitted, for the model its entry names, sent
// on Mesh-LLM's path. The body's bytes go out unchanged, so the request
// commitment both sides compute is the same.
ML_HEAD :: "POST /systemone HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: ";
ml_request :: (body:string,model:string,pools:SvPools,out:[]u8)->int #export_c {
    r:=sv_decode(body,pools);
    if r.code!=SvCode.none || r.model!=model {return -1;}
    n:=json_raw(out,0,ML_HEAD);n=json_write_int(out,n,body.count);n=json_raw(out,n,"\r\n\r\n");
    return json_raw(out,n,body);
}
```

The answer comes back as `{model, answers, usage}`. The adapter reads it
strictly:

<!-- source: psionic/lib/peer/meshllm.coax answer -->
```jai
// A peer's HTTP response read as System One's {model, answers, usage}. The
// status must be 200, the body exactly its Content-Length, the JSON one
// object with each of the three keys once; model must be the model asked;
// answers an object; usage two integers inside chapter 180's token bound.
// Missing or unreadable usage is unknown, never zero.
MlAnswer :: struct {refuse:MlRefuse;status:int;answers:string;tokens_in:int;tokens_out:int;}
ml_status :: (text:string)->int {
    if text.count<13 || text[0..<9]!="HTTP/1.1 " || text[12]!=' ' {return -1;}
    v:=0;
    for i:9..<12 {c:=text[i];if c<'0' || c>'9' {return -1;}v=v*10+cast(int)(c-'0');}
    return v;
}
// A header's value, or "!" when the head has no such line.
// A header's value from the lines that end by stop, or "!" when none is it.
ml_header :: (text:string,stop:int,name:string)->string {
    at:=0;
    while true {
        end:=sv_find(text,at,"\r\n");
        if end<0 || end>stop {return "!";}
        line:=text[at..<end];
        if line.count>name.count && line[name.count]==':' && sv_same(line[0..<name.count],name) {return sv_trim(line[name.count+1..<line.count]);}
        at=end+2;
    }
    return "!";
}
ml_usage :: (j:[]Json,a:[]MlAnswer)->bool {
    seen:=[2]bool.[false,false];
    while true {
        t:=json_next(j);if t==JSON_OBJECT_END {break;}
        key:=json_text(j);t=json_next(j);
        k:=-1;if json_is(key,"input_tokens") {k=0;} else if json_is(key,"output_tokens") {k=1;}
        if k<0 {ok:=json_skip(j);continue;}
        if seen[k] || t!=JSON_NUMBER {return false;}
        v:=json_int(json_text(j));
        if !v.ok || !bl_in(v.value,BL_TOKENS_MOST) {return false;}
        seen[k]=true;
        if k==0 {a[0].tokens_in=v.value;} else {a[0].tokens_out=v.value;}
    }
    return seen[0] && seen[1];
}
ml_answer :: (text:string,model:string)->MlAnswer #export_c {
    a:=[1]MlAnswer.[MlAnswer.{refuse=MlRefuse.status,status=ml_status(text)}];
    if a[0].status!=200 {return a[0];}
    a[0].refuse=MlRefuse.length;
    // No blank line: head is -1, no header ends by it, and the length is refused.
    head:=sv_index(text,"\r\n\r\n");
    body:=text[head+4..<text.count];
    if sv_length(ml_header(text,head,"content-length"))!=body.count {return a[0];}
    a[0].refuse=MlRefuse.json;
    if !json_valid(body) {return a[0];}
    j:[1]Json;json_start(j[..],body);
    if json_next(j[..])!=JSON_OBJECT {return a[0];}
    named:=false;answered:=false;used:=false;
    while true {
        t:=json_next(j[..]);if t==JSON_OBJECT_END {break;}
        key:=json_text(j[..]);t=json_next(j[..]);from:=j[0].from;
        if json_is(key,"model") {
            if named {return a[0];}
            named=t==JSON_STRING && json_is(json_text(j[..]),model);
            if !named {a[0].refuse=MlRefuse.model;return a[0];}
        } else if json_is(key,"answers") {
            if answered {return a[0];}
            answered=t==JSON_OBJECT;ok:=json_skip(j[..]);
            a[0].answers=body[from..<j[0].at];
        } else if json_is(key,"usage") {
            if used {return a[0];}
            used=t==JSON_OBJECT && ml_usage(j[..],a[..]);
            if !used {a[0].refuse=MlRefuse.usage;return a[0];}
        } else {ok:=json_skip(j[..]);}
    }
    a[0].refuse=MlRefuse.model;
    if !named {return a[0];}
    a[0].refuse=MlRefuse.answers;
    if !answered {return a[0];}
    a[0].refuse=MlRefuse.usage;
    if !used {return a[0];}
    a[0].refuse=MlRefuse.none;
    return a[0];
}
```

As in Mesh-LLM's own metering, usage that is missing or malformed is
unknown, never a zero-token bill. An answer that names another model is
not this model's answer.

<!-- source: psionic/lib/peer/meshllm.coax check-answer -->
```jai
#test "a peer's answer is read, or refused for the first thing wrong" {
    o:[1024]u8;
    a:=mlt_answer(o[..],MLT_BODY);
    #assert a.refuse==MlRefuse.none && a.status==200 && a.answers==MLT_ANSWERS && a.tokens_in==22 && a.tokens_out==0;
    // Another model's answer is not this model's.
    #assert mlt_answer(o[..],"{\"model\":\"laya\",\"answers\":{},\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}").refuse==MlRefuse.model;
    // No usage, or usage a meter cannot read, is unknown, never zero.
    #assert mlt_answer(o[..],"{\"model\":\"psionic-toy\",\"answers\":{},\"usage\":null}").refuse==MlRefuse.usage;
    #assert mlt_answer(o[..],"{\"model\":\"psionic-toy\",\"answers\":{},\"usage\":{\"input_tokens\":1.5,\"output_tokens\":0}}").refuse==MlRefuse.usage;
    #assert mlt_answer(o[..],"{\"model\":\"psionic-toy\",\"answers\":{}}").refuse==MlRefuse.usage;
    #assert mlt_answer(o[..],"{\"model\":\"psionic-toy\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}").refuse==MlRefuse.answers;
```

The Psionic client gets the peer's answers and usage unchanged, with a
service block naming the peer, its profile and the checkpoint identity, so
an answer never carries a Psionic engine's identity it did not come from.

## Check the peer

Chapter 195's checker re-runs a sampled job and compares. Here the buyer
re-runs the System One request on its own engine and compares the answers'
bytes. That is a verdict only under an exact profile the checker lists for
Mesh-LLM peers; comparing probabilities within a tolerance would need them
parsed back to numbers, which is not built.

<!-- source: psionic/lib/peer/meshllm.coax verify -->
```jai
// Chapter 195's comparison for a Mesh-LLM peer. The checker re-runs the
// request on its own engine and compares the answers' bytes. Only a profile
// the checker lists for Mesh-LLM peers, and only an exact one, can give a
// verdict: equal bytes agree, any other bytes disagree. Comparing within a
// tolerance would need the probabilities parsed, which is not built.
ml_check :: (profiles:[]PkProfile,answers:string,reference:string,ref_ok:bool)->PkCompare #export_c {
    k:=-1;
    for i:0..<profiles.count {if profiles[i].name==ML_PROFILE {k=i;}}
    r:=PkCompare.{verdict=PkVerdict.unverifiable,reason=PkReason.profile};
    if k<0 || !profiles[k].exact {return r;}
    r.reason=PkReason.reference;
    if !ref_ok {return r;}
    if answers==reference {return PkCompare.{verdict=PkVerdict.agree,reason=PkReason.exact};}
    return PkCompare.{verdict=PkVerdict.disagree,reason=PkReason.bits};
}
// A probe's outcome as chapter 173's evidence: qualified only when the
// checker agreed, covering exactly the probe's size: its questions, the
// most options any one lists (at least one), its state bytes and the
// deadline it ran under.
ml_evidence :: (c:CpClaim,cmp:PkCompare,record:string,covered:CpLimits)->CpEvidence #export_c {
    v:=QyVerdict.failed;
    if cmp.verdict==PkVerdict.agree {v=QyVerdict.qualified;}
    return CpEvidence.{digest=c.digest,backend=c.backend,profile=c.profile,record=record,verdict=v,covered=covered};
}
ml_size :: (r:SvRequest,pools:SvPools,deadline_ms:int)->CpLimits {
    most:=1;
    for q:pools.questions[0..<r.questions] {most=cp_max(most,q.count);}
    return CpLimits.{questions=r.questions,options=most,state=r.state.value.count,deadline_ms=deadline_ms};
}
```

The same comparison makes the evidence a claim needs. Before any work, the
buyer sends the peer a probe, chapter 173's two-question request, and
re-runs it. If the bytes match, the probe becomes a record covering exactly
the probe's size: 2 questions, 2 options, 22 state bytes, 30000 ms. If they
don't, the record is `failed` and the entry stays `unknown`.

<!-- source: psionic/lib/peer/meshllm.coax check-evidence -->
```jai
#test "a probe the peer matched is a record; one it missed is not" {
    c:=ml_claim(mlt_peer()).claim;size:=CpLimits.{questions=2,options=2,state=22,deadline_ms=30000};
    agree:=PkCompare.{verdict=PkVerdict.agree,reason=PkReason.exact};
    e:=ml_evidence(c,agree,"probe-1",size);
    #assert e.verdict==QyVerdict.qualified && e.digest==SV_TOY_SHA && e.backend==CpBackend.cpu && e.profile==ML_PROFILE && e.record=="probe-1" && e.covered.state==22;
    ev:=[1]CpEvidence.[e];en:=cp_entry(c,ev[..],0);
    #assert en.qualified && en.limits.questions==2 && en.limits.options==2 && en.limits.state==22 && en.limits.deadline_ms==30000;
    for v:0..<4 {
        if v==1 {continue;}
        e=ml_evidence(c,PkCompare.{verdict=cast(PkVerdict)v},"probe-1",size);#assert e.verdict==QyVerdict.failed;
        ev[0]=e;#assert !cp_entry(c,ev[..],0).qualified;
    }
}
```

## Run it

The [example](../examples/meshllm-loopback.coax) runs both sides in one
process. The simulated peer is not llama.cpp: it is chapter 20's toy engine
behind Mesh-LLM's wire shapes, advertising the toy's checkpoint. That is why
its answers can match the buyer's bit for bit. A real Mesh-LLM node serving
the same weights through llama.cpp would almost certainly not match, and
then the probe gives no record and nothing is routed to it, which is the
behavior this chapter wants. Usage records come from chapter 180's
`um_observe`; the peer reports no device time, so none is recorded.

<!-- source: psionic/examples/meshllm-loopback.coax example -->
```jai
PEER :: "8f0e3c2a5b6d4e1f9a7c3b2d1e0f4a5b6c7d8e9f0a1b2c3d4e5f60718293a4b5";
PROBE :: "{\"model\":\"psionic-toy\",\"state\":\"The kettle has boiled.\",\"questions\":{\"hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"},\"drink\":{\"type\":\"choice\",\"criteria\":{\"tea\":null,\"soup\":\"a meal\"}}}}";
JOB1 :: "{\"model\":\"psionic-toy\",\"state\":\"The kettle is cold.\",\"questions\":{\"hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"}}}";
JOB2 :: "{\"model\":\"psionic-toy\",\"state\":\"The tea is ready.\",\"questions\":{\"hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"}}}";
JOB3 :: "{\"model\":\"psionic-toy\",\"state\":\"s\",\"questions\":{\"a\":{\"type\":\"noul\"},\"b\":{\"type\":\"noul\"},\"c\":{\"type\":\"noul\"}}}";

main :: () {
    toy:[1]SvToy;engine:=sv_test_toy(toy[..],400);
    ex:[1]Ex;ex[0].engine=engine;ex[0].work=sv_scratch(ex[0].prep[..],engine);
    print("Mesh-LLM % in process; the peer is simulated: chapter 20's toy engine behind Mesh-LLM's wire\n",ML_REVISION[0..<7]);
    // Four advertisements, as decoded from node.proto's PeerAnnouncement.
    toy_model:=MlModel.{name="psionic-toy",identity=SV_TOY_SHA,system_one=MlLevel.supported,layers=4,runtime_identity=SV_TOY_SHA,ready=true};
    peer:=MlPeer.{endpoint=PEER,role=MlRole.host,mesh="lab",gen=7,admission=MlAdmission.accepting,model=toy_model};
    laya:=peer;laya.model.name="meshllm/laya-multilingual-F16-GGUF";laya.model.system_one=MlLevel.likely;
    gpu:=peer;gpu.gpus=1;paused:=peer;paused.admission=MlAdmission.all_paused;
    ads:=[4]MlPeer.[peer,laya,gpu,paused];names:=[4]string.["toy, CPU only","Laya, system_one likely","toy, one GPU","toy, all paused"];
    print("advertisements:\n");
    for i:0..<4 {
        c:=ml_claim(ads[i]);
        if c.refuse==MlRefuse.none {print("  %: claim % % %\n",names[i],c.claim.model,cp_backend_name(c.claim.backend),c.claim.profile);}
        else {print("  %: refused, %\n",names[i],ml_refuse_name(c.refuse));}
    }
    order:[1]int;r:=ml_route(peer,PG_NOW,PG_NOW,60,order[..]);
    print("route: %, % stage, layers 0..%\n",r.route.code,r.route.stages,peer.model.layers);
    offer:=MlOffer.{model="psionic-toy",input=2000000,output=8000000,protocol=3};
    print("offer: % msat per million in, % out, protocol %: %\n",offer.input,offer.output,offer.protocol,ifx ml_offer(offer,"psionic-toy")==MlRefuse.none then "accepted" else "refused");
    claims:=[1]CpClaim.[ml_claim(peer).claim];doc:[1]CpEntry;
    none:[1]CpEvidence;n:=cp_derive(claims[..],none[0..<0],0,doc[..]);
    print("before a probe: % %: unknown (%)\n",doc[0].claim.model,cp_backend_name(doc[0].claim.backend),cp_cause_name(doc[0].cause));

    // The probe: chapter 173's request, sent through the translation and
    // re-run by the buyer. The buyer's checker requires Mesh-LLM peers to
    // match its own answers byte for byte.
    exact:=[1]PkProfile.[PkProfile.{name=ML_PROFILE,exact=true}];
    keys:[256]u8;key:=ml_commit(PROBE,keys[0..<64]);
    req:[1024]u8;m:=ml_request(PROBE,"psionic-toy",ex_pools(ex[..]),req[..]);
    print("probe request, commitment %:\n%\n",key,cast(string)req[0..<m]);
    resp:[2048]u8;k:=ex_peer(ex[..],cast(string)req[0..<m],Mode.honest,resp[..]);
    print("peer:\n%\n",cast(string)resp[0..<k]);
    a:=ml_answer(cast(string)resp[0..<k],"psionic-toy");
    ref:[1024]u8;own:=ex_reference(ex[..],PROBE,ref[..]);
    cmp:=ml_check(exact[..],a.answers,own,true);
    size:=ml_size(sv_decode(PROBE,ex_pools(ex[..])),ex_pools(ex[..]),30000);
    record:="probe:8f0e3c2a5b6d:1";
    evidence:=[1]CpEvidence.[ml_evidence(claims[0],cmp,record,size)];
    print("probe: % %; record % covers % questions, % options, % state bytes, % ms\n",cmp.verdict,cmp.reason,record,size.questions,size.options,size.state,size.deadline_ms);
    n=cp_derive(claims[..],evidence[..],0,doc[..]);
    body:[2048]u8;b:=cp_render(doc[..],body[..]);print("capability: %\n",cast(string)body[0..<b]);
    print("honest: %\n",cp_cause_name(cp_honest(doc[..],evidence[..],0).cause));

    // Work: a ledger with a 20000 msat cap. Each job holds the cost of the
    // buyer's per-request cap, 4096 input tokens, with a 1 sat deposit.
    quote:=ml_cost(offer,"psionic-toy",4096,0,1000).msat;
    l:=VpLedger.{account=BlAccount.{budget=20000}};policy:=VpPolicy.{trust_unsampled=false,payout_cap=5000};
    metrics:=UmMetrics.{};deposit:=1000;
    print("quote per job: % msat (4096 tokens in, 1000 msat deposit)\n",quote);
    jobs:=[2]string.[JOB1,JOB2];modes:=[2]Mode.[Mode.honest,Mode.tamper];
    for j:0..<2 {
        text:=jobs[j];key=ml_commit(text,keys[64*j+64..<64*j+128]);
        rq:=sv_decode(text,ex_pools(ex[..]));ad:=cp_admit(doc[..],rq,ex_pools(ex[..]));
        h:=vp_hold(l,key,PEER,quote,"msat");l=h.ledger;
        m=ml_request(text,doc[ad.entry].claim.model,ex_pools(ex[..]),req[..]);
        k=ex_peer(ex[..],cast(string)req[0..<m],modes[j],resp[..]);
        a=ml_answer(cast(string)resp[0..<k],"psionic-toy");
        own=ex_reference(ex[..],text,ref[..]);
        cmp=ml_check(exact[..],a.answers,own,true);
        cost:=ml_cost(offer,"psionic-toy",a.tokens_in,a.tokens_out,deposit);deposit=0;
        um:=um_observe(metrics,ml_record(j+1,doc[ad.entry],a,0,quote,cost.msat));metrics=um.metrics;
        d:=vp_decide(l,policy,key,cost.msat,true,cmp);l=d.ledger;
        print("job %, commitment %...: admission %, hold %\n",j+1,key[0..<12],ifx ad.code==SvCode.none then "ok" else sv_code_name(ad.code),vp_state_name(h.state));
        print("  answers %\n  usage % in, % out; bill % msat, deposit %, final invoice %, cost % msat\n",a.answers,a.tokens_in,a.tokens_out,cost.bill,cost.deposit,cost.final,cost.msat);
        print("  checked: % %; meter: %; ledger: %\n",cmp.verdict,cmp.reason,um_cause_name(um.cause),vp_state_name(d.state));
        if j==0 {out:[1024]u8;o:=ml_reply(a,doc[0].claim,PEER,out[..]);print("  reply: %\n",cast(string)out[0..<o]);}
    }
    rq:=sv_decode(JOB3,ex_pools(ex[..]));ad:=cp_admit(doc[..],rq,ex_pools(ex[..]));
    print("job 3, three questions: %, nothing sent\n",sv_code_name(ad.code));
    print("meter: % requests, % tokens in, % msat quoted, % msat billed by the peer\n",metrics.requests,metrics.tokens_in,metrics.quoted,metrics.charged);
    print("ledger: escrow % msat, owed % msat, withheld % msat\n",l.account.held,l.account.spent,l.withheld);
    for i:0..<l.m {
        kb:[VP_TEXT]u8;pb:[VP_TEXT]u8;id:=kept_view(l.jobs[l.intents[i].at].receipt,kb[..]);
        v:=vp_invoice(l,id,"lightning");payee:=kept_view(v.payee,pb[..]);
        print("  intent %... % msat to %...: lightning invoice for % msat (not paid)\n",id[0..<12],v.msat,payee[0..<12],v.msat);
    }

    print("refusals:\n");
    m=ml_request(PROBE,"psionic-toy",ex_pools(ex[..]),req[..]);
    wrong:=[3]Mode.[Mode.model,Mode.usage,Mode.down];what:=[3]string.["answers for another model","sends \"usage\":null","answers 503"];
    for i:0..<3 {k=ex_peer(ex[..],cast(string)req[0..<m],wrong[i],resp[..]);a=ml_answer(cast(string)resp[0..<k],"psionic-toy");print("  a peer that %: %\n",what[i],ml_refuse_name(a.refuse));}
    old:=offer;old.protocol=2;print("  an offer on payment protocol 2: %\n",ml_refuse_name(ml_offer(old,"psionic-toy")));
    print("  a request for a model the entry doesn't name: %\n",ifx ml_request(PROBE,"meshllm/laya-multilingual-F16-GGUF",ex_pools(ex[..]),req[..])<0 then "not sent" else "sent");
}
```

```shell
coax/bin/coax run psionic/examples/meshllm-loopback.coax
```

```text
Mesh-LLM f2a7f62 in process; the peer is simulated: chapter 20's toy engine behind Mesh-LLM's wire
advertisements:
  toy, CPU only: claim psionic-toy cpu mesh-llm-f2a7f62
  Laya, system_one likely: refused, system_one
  toy, one GPU: refused, backend
  toy, all paused: refused, admission
route: ok, 1 stage, layers 0..4
offer: 2000000 msat per million in, 8000000 out, protocol 3: accepted
before a probe: psionic-toy cpu: unknown (no_record)
probe request, commitment fc3f77ef3b316ac0733a2ec6d425da0936d61b0f3eb84fe3b3a572a6f2419e52:
POST /systemone HTTP/1.1
Content-Type: application/json
Content-Length: 193

{"model":"psionic-toy","state":"The kettle has boiled.","questions":{"hot":{"type":"noul","instructions":"Is the water hot?"},"drink":{"type":"choice","criteria":{"tea":null,"soup":"a meal"}}}}
peer:
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 244

{"model":"psionic-toy","answers":{"hot":{"type":"noul","noul":0.437308645},"drink":{"type":"choice","choice":"soup","confidence":0.562514618,"probabilities":{"tea":0.437485382,"soup":0.562514618}}},"usage":{"input_tokens":22,"output_tokens":0}}
probe: agree exact; record probe:8f0e3c2a5b6d:1 covers 2 questions, 2 options, 22 state bytes, 30000 ms
capability: {"schema":"psionic-capability-v1","door":"psionic","version":"psionic-serve-v1","entries":[{"model":"psionic-toy","alias":"","identity":"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573","backend":"cpu","profile":"mesh-llm-f2a7f62","state":"qualified","record":"probe:8f0e3c2a5b6d:1","limits":{"questions":2,"options":2,"state_bytes":22,"deadline_ms":30000}}]}
honest: none
quote per job: 8192 msat (4096 tokens in, 1000 msat deposit)
job 1, commitment f27525ac8096...: admission ok, hold held
  answers {"hot":{"type":"noul","noul":0.437483439}}
  usage 19 in, 0 out; bill 38 msat, deposit 1000, final invoice 0, cost 1000 msat
  checked: agree exact; meter: none; ledger: paid
  reply: {"model":"psionic-toy","answers":{"hot":{"type":"noul","noul":0.437483439}},"usage":{"input_tokens":19,"output_tokens":0},"service":{"door":"mesh-llm","revision":"f2a7f62","profile":"mesh-llm-f2a7f62","identity":"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573","peer":"8f0e3c2a5b6d4e1f9a7c3b2d1e0f4a5b6c7d8e9f0a1b2c3d4e5f60718293a4b5"}}
job 2, commitment d8b1c8dcf41c...: admission ok, hold held
  answers {"hot":{"type":"noul","noul":0.537392432}}
  usage 17 in, 0 out; bill 34 msat, deposit 0, final invoice 1000, cost 1000 msat
  checked: disagree bits; meter: none; ledger: withheld
job 3, three questions: unsupported_input, nothing sent
meter: 2 requests, 36 tokens in, 16384 msat quoted, 2000 msat billed by the peer
ledger: escrow 0 msat, owed 1000 msat, withheld 1000 msat
  intent f27525ac8096... 1000 msat to 8f0e3c2a5b6d...: lightning invoice for 1000 msat (not paid)
refusals:
  a peer that answers for another model: model
  a peer that sends "usage":null: usage
  a peer that answers 503: status
  an offer on payment protocol 2: protocol
  a request for a model the entry doesn't name: not sent
```

The probe's commitment `fc3f77ef...` is the SHA-256 of its 193-byte body,
which an independent Python `hashlib` run reproduces. The transcript matches
the hand trace: 8192 msat quoted per job, 38 and 34 msat billed, 1000 msat
each paid, one paid and one withheld. It is compared exactly in debug and
release builds.

## What it refuses

The last lines of the transcript are the broken inputs: a peer that answers
for another model is refused with `model`, one that sends `"usage":null`
with `usage`, one that answers 503 with `status`, an offer on payment
protocol 2 with `protocol`, and a request for a model the entry does not
name is not sent. Job 3, three questions against an entry qualified for
two, is refused by chapter 173's admission with `unsupported_input` before
any byte leaves. Further checks pin every claim refusal in order, every
role and admission state, every capability level, the HTTP status, length
and header rules, duplicate keys, and the offer and token bounds.

## Checks

`peer/meshllm.coax` is an `#authority` file. `coax mutate --root-only`
changes each of its 371 mutation sites in turn, and the checks kill all
371. Run it with the other Psionic units:

```shell
psionic/check.sh --only peer/meshllm
```

## What remains

- **Live interop.** No real Mesh-LLM node was run, joined or contacted.
  Decoding protobuf `GossipFrame` bytes, the signed node records, QUIC
  transport and Nostr discovery are not built; the advertisement here is
  the decoded subset, written by hand from `node.proto`.
- **The other direction.** The draft's goal of a Psionic server registered
  as a Mesh-LLM backend through the plugin endpoint interface
  (`EndpointManifest`, `openai_http(...)`) is not built.
- **The OpenAI-compatible path.** Only System One is translated. Chat
  completions, SSE streams and the `/v1/decisions` adapter are not.
- **The payment exchange.** The bill, the deposit and the final invoice
  follow Mesh-LLM's rules, but the `/mesh/payment/v3` messages (admission
  terms, `Settlement`, strikes after three unpaid invoices) are not spoken,
  and nothing is paid. Whether the exchange bills `/systemone` usage at this
  revision is not stated in the documents read. Paying a peer and joining a
  public mesh are owner decisions recorded in `NEEDS_OWNER.md`.
- **Checking real peers.** A byte comparison qualifies only a peer that
  reproduces the buyer's engine exactly. A tolerance profile for llama.cpp
  answers, and checking a peer's reported token counts against the buyer's
  own tokenizer, are not built.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Chapter 173](ch173-serve-a-model-with-an-honest-capability.md),
  [chapter 180](ch180-observe-meter-and-price.md),
  [chapter 188](ch188-peers-transport-and-discovery.md),
  [chapter 195](ch195-check-a-peers-work.md) and
  [chapter 238](ch238-pay-for-verified-work.md).
- Mesh-LLM at
  [`f2a7f62`](https://github.com/Mesh-LLM/mesh-llm/tree/f2a7f62c755e283c389a28cbf8d1e8f926677a4e),
  Apache-2.0, read as untrusted source.
