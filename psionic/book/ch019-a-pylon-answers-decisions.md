# 19. A Pylon answers decisions

[Chapter 16](ch016-serve-systemone.md) answers a typed decision over HTTP on
one machine. This chapter puts the same engine behind a Nostr key. A
Pylon holds one relay connection, says who it is and what it serves in a
signed beacon, takes decision jobs that buyers encrypt to its key, and
answers each one, encrypted back, with the identity of the model that
decided. Nobody has to know its address. They have to know its key and a
relay.

Four kinds of event carry all of it:

| Kind | From | What |
| --- | --- | --- |
| `10002` | Pylon | NIP-65 relay list: where to send jobs |
| `30200` | Pylon | the beacon: online, draining or offline; the served model identity; free slots |
| `25910` | buyer | a job: chapter 16's request body plus a request id, attempt and deadline, NIP-44 encrypted to the Pylon |
| `26910`, `27010` | Pylon | the result, or a status (a refusal, busy, cancelled), NIP-44 encrypted to the buyer |

The protocol units came first and are covered by their own
[record](../../docs/verification/psionic-ch19-protocol-2026-10-29.md):
`template.coax`, `beacon.coax`, `relays.coax` and `decision.coax` in
`psionic/lib/pylon/`. They say what a beacon contains, what a job is and
what its answer looks like. This chapter adds the program that runs them:
`queue.coax`, which decides what the running Pylon does with each event,
and `run.coax`, which wires the units to a relay, a signer and an engine.

## What is test-only here

Three pieces are stand-ins until other work lands on main. The chapter says
so wherever they appear.

- **Signing.** Every event the Pylon publishes is signed by
  `psionic/tests/pylon/sign_double.ts`, a small `@noble/curves` process the
  Pylon talks to over a pipe. It has the same shape as
  `nostr_event_sign_step` in `coax/lib/nostr/event_sign.coax`, which is
  still in its mutation gate. The BIP-340 signer underneath it,
  `coax/lib/crypto/schnorr_sign.coax`, landed while this chapter was
  written (`0cc86015e7`).
- **The NIP-44 conversation key.** NIP-44 itself is
  `coax/lib/nostr/nip44.coax`, for real: padding, keys, MAC and
  ChaCha20. Only the shared x of the two keys comes from the double, until
  NIP-44's conversation key from two keys lands.
- **The key.** The Pylon's key is the SHA-256 of a label, made for tests.
  `--key-file` is accepted as configuration (the owner's choice: a `0600`
  file in `~/work/.secrets/`) and refused while the signer is the double.
- **Relay frames.** `py_frame` reads the relay's EVENT, OK, EOSE, CLOSED
  and NOTICE frames until `coax/lib/nostr/message.coax` lands.

Everything else is the real code: the WebSocket (`ws.coax`), NIP-42
(`nip42.coax`), the event check (`event.coax`, a full BIP-340
verification of every job), NIP-44, the beacon, the job rules and the
engine.

## One job on paper

Before any socket, here is one job through the units, as
[`psionic/examples/pylon-job.coax`](../examples/pylon-job.coax):

<!-- source: psionic/examples/pylon-job.coax example -->
```jai
// A decrypted 25910 from the chapter's fixtures, at the clock the job was made.
CLOCK :: 1791400000;

main :: () {
    body:[4096]u8;jb:=pd_admit(pd_env(PD_CALL),PD_WORKER,CLOCK,body[..]);
    print("admit: %, request %, attempt %, % ms left\n",jb.code,jb.request,jb.attempt,pd_left_ms(jb,CLOCK));
    print("server body: %\n",cast(string)body[0..<jb.body]);
    q:[1]PqQueue;
    route:=pq_route(true,25910,true,cast(int)jb.code,cast(int)jb.type,jb.answerable,false,pq_free(q[..]));
    slot:=pq_put(q[..]);print("route: %, slot %, % of % free\n",route,slot,pq_free(q[..]),PQ_SLOTS);
    budget:=pq_budget(pd_left_ms(jb,CLOCK),30000);print("budget: % ms, set by the deadline: %\n",budget.ms,budget.deadline);
    // The engine answered 200 with chapter 16's body.
    out:[4096]u8;receipt:="{\"attempt_id\":\"r-1#1\",\"schema\":\"openagents.receipt.execution.v1\",\"transport\":\"relay\"}";
    r:=pd_result(out[..],jb,PD_ANSWER,"studio-gpu",PD_WORKER,"psionic-pylon-v1",PB_IDENTITY,receipt);
    a:=pq_answer(true,false,200,cast(int)r.code,PQ_NONE);
    print("answer: 26910 %, %\n",a.result,cast(string)out[0..<r.written]);
    // The same body from an engine serving other weights.
    other:=pq_answer(true,false,200,cast(int)pd_result(out[..],jb,PD_ANSWER,"studio-gpu",PD_WORKER,"psionic-pylon-v1","elephant-1@sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",receipt).code,PQ_NONE);
    print("other identity: 26910 %, code %\n",other.result,pd_code_name(cast(PdCode)other.code));
    done:=pq_done(q[..],slot);
    // A version the pylon does not speak: refused with a 27010 the caller can match.
    old:=pd_admit(pd_env(pd_swap(out[..],PD_CALL,"systemone.v1","systemone.v0")),PD_WORKER,CLOCK,body[..]);
    route=pq_route(true,25910,true,cast(int)old.code,cast(int)old.type,old.answerable,false,pq_free(q[..]));
    n:=pd_status(body[..],old,PdStatus.error,old.code,-1);
    print("v0: %, %\n",route,cast(string)body[0..<n]);
    // Two jobs wait; a cancel names the second.
    first:=pq_put(q[..]);second:=pq_put(q[..]);running:=pq_next(q[..]);
    print("running slot %, cancel: % then %\n",running,pq_cancel(q[..],second),pq_cancel(q[..],running));
    print("stop with % held: %; with none: %\n",pq_held(q[..]),pq_life(true,pq_held(q[..])),pq_life(true,0));
}
```

```shell
coax/bin/coax run psionic/examples/pylon-job.coax
```

```jai
admit: none, request r-1, attempt 1, 60000 ms left
server body: {"model":"elephant-1","state":{"ticket":"I was billed twice"},"questions":{"refund":{"type":"noul","instructions":"Does the customer ask for money back?"}}}
route: queue, slot 0, 3 of 4 free
budget: 30000 ms, set by the deadline: false
answer: 26910 true, {"attempt":1,"outcome":"answered","receipt":{"attempt_id":"r-1#1","schema":"openagents.receipt.execution.v1","transport":"relay"},"request":"r-1","requires":[],"response":{"answers":{"refund":{"type":"noul","noul":0.93}},"latency_ms":412,"model":"elephant-1","service":{"door":"pylon:studio-gpu","identity":"elephant-1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","provider":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","version":"psionic-pylon-v1"},"usage":null},"type":"result","v":"openagents.systemone.v1"}
other identity: 26910 false, code identity_mismatch
v0: refuse, {"attempt":1,"code":"unsupported_version","message":"this pylon reads openagents.systemone.v1 only","request":"r-1","requires":[],"status":"error","type":"status","v":"openagents.systemone.v1"}
running slot 0, cancel: 1 then 2
stop with 1 held: drain; with none: off
```

The 25910's plaintext becomes exactly the body chapter 16's server reads.
The answer comes back as a 26910 that names this Pylon (`door`
`pylon:<slug>`, `provider` its key) and the identity the engine reported.
If the engine reports any other identity, the Pylon does not send the
answer: it sends a 27010 `identity_mismatch`. A buyer that chose the Pylon
by its beacon gets the model the beacon named, or a refusal.

## What the running Pylon decides

`queue.coax` is `#authority` and loads nothing. It takes the other units'
verdicts as integers: whether the signature verified, whether NIP-44
opened the content, and `pd_admit`'s code and type. `run.coax` ties those
integers to `PdCode` with top-level `#assert`s, so a renumbering fails the
build. Its mutation run therefore tests its own decisions and nothing
else: 173 mutants, 173 killed.

The first decision is whether an event gets an answer at all:

<!-- source: psionic/lib/pylon/queue.coax route -->
```jai
// What a received event gets. verified: its id and BIP-340 signature checked;
// decrypted: NIP-44 opened its content; code, type and answerable are
// pd_admit's; free is pq_free.
pq_route :: (verified:bool,kind:int,decrypted:bool,code:int,type:int,answerable:bool,draining:bool,free:int)->PqRoute #export_c {
    if !verified || kind!=PQ_REQUEST || !decrypted {return PqRoute.ignore;}
    if code!=PQ_NONE {return ifx answerable then PqRoute.refuse else PqRoute.ignore;}
    if type==PQ_CANCEL {return PqRoute.cancel;}
    if type!=PQ_CALL || !answerable {return PqRoute.ignore;}
    if draining || free<1 {return PqRoute.busy;}
    return PqRoute.queue;
}
```

An event that does not verify, is not a request, or cannot be decrypted
is dropped without an answer: there is nobody trustworthy to answer, and
an answer to a forged event would be a way to make the Pylon talk. A
refusal goes back only when `pd_admit` could read who asked and which
request it was (`answerable`). A cancel is always weighed, even when the
queue is full. A call while draining, or with all four slots held, is
`busy` with a `retry_after_ms`.

The second is what goes back after the engine replied:

<!-- source: psionic/lib/pylon/queue.coax answer -->
```jai
// The answer after the engine call. reached: a whole HTTP response came back;
// timed_out: the budget ran out first; status: its HTTP status; written: the
// code pd_result gave for a 200 body (none when the 26910 was written);
// refused: the PdCode the server's refusal body named (0 when none was read).
pq_answer :: (reached:bool,timed_out:bool,status:int,written:int,refused:int)->PqAnswer #export_c {
    if !reached {return PqAnswer.{code=ifx timed_out then PQ_TIMEOUT else PQ_INTERNAL};}
    if status==200 {
        if written==PQ_NONE {return PqAnswer.{result=true};}
        return PqAnswer.{code=written};
    }
    if refused<=PQ_NONE || refused>=PQ_CODES {return PqAnswer.{code=PQ_INTERNAL};}
    return PqAnswer.{code=refused};
}
```

`pd_result` has already checked the 200 body against the beacon's
identity. A refusal from the engine keeps its own code (`busy`,
`timeout`, `unsupported_input`, and so on) when the body names one this
protocol knows, and becomes `internal` otherwise. No answer at all is
`timeout` when the job's budget ran out, `internal` when the connection
failed.

The rest of the unit is small: a 64-id window against relay resends
(`pq_seen`), four slots run oldest first (`pq_put`, `pq_next`), a cancel
that removes a waiting job (`pq_cancel`), the engine's budget as the
smaller of the job's deadline and the host's limit (`pq_budget`), and the
shutdown order, run then draining then offline (`pq_life`,
`pq_beacon_now`).

## The program

`run.coax` is the Pylon. Its host, `psionic/platform/pylon-host.c`, only
hands over the arguments, the wall clock and a stop request (SIGTERM,
SIGINT or a stop file). Sockets are `coax/platform/socket-native.c`,
random bytes `entropy-foreign.c`, and the signer's process
`process-native.c`.

At start it signs nothing until it knows what it serves. It asks the
engine one fixed question and takes the identity from the answer's
`service.identity`; the beacon names that identity, and every answer is
checked against it. Then it opens the WebSocket, answers the relay's
NIP-42 challenge with a signed kind 22242, and publishes its relay list
(by default `wss://relay.openagents.com` and the relay it connected to)
and an online beacon. It subscribes to kind 25910 with a `#p` filter on
its own key.

Each job goes through the same steps:

<!-- source: psionic/lib/pylon/run.coax event -->
```jai
py_event :: (p:[]Py,text:string) {
    scratch:[262144]u8;
    ev:=nostr_event_verify_vartime(text,scratch[..],false);
    verified:=ev.error==NE_OK && ev.verified;
    if !verified {p[0].forged+=1;print("pylon: event refused by verification (error %)\n",ev.error);return;}
    if !pq_seen(p[0].q[..],ev.id[..]) {p[0].duplicates+=1;return;}
    idh:[64]u8;pkh:[64]u8;id:=py_hex(ev.id[..],idh[..]);caller:=py_hex(ev.public_key[..],pkh[..]);
    raw:=ev.original[ev.content_from..<ev.content_to];payload:[PY_OUT]u8;k:=json_unescape(raw,payload[..]);
    plain:[PY_OUT]u8;opened:=Nip44Result.{error=NIP44_PAYLOAD};
    if ev.kind==PD_REQUEST && k>0 {opened=py_open_payload(p,caller,cast(string)payload[0..<k],plain[..]);}
    decrypted:=opened.error==NIP44_OK;
    plaintext:=ifx decrypted then cast(string)plain[0..<opened.written] else "";
    env:=PdEnvelope.{kind=ev.kind,id=id,pubkey=caller,created_at=ev.created_at,tags=ev.original[ev.tags_from..<ev.tags_to],plaintext=plaintext};
    body:[16384]u8;now:=pyh_now_s();jb:=pd_admit(env,py_me(p),now,body[..]);
    draining:=p[0].life.status!=PbStatus.online;
    route:=pq_route(true,ev.kind,decrypted,cast(int)jb.code,cast(int)jb.type,jb.answerable,draining,pq_free(p[0].q[..]));
    print("pylon: event % kind % -> % (%)\n",py_short(id),ev.kind,route,jb.code);
    if route==PqRoute.ignore {p[0].ignored+=1;return;}
    if route==PqRoute.refuse {ok:=py_status(p,jb,jb.code);return;}
    if route==PqRoute.busy {ok:=py_status(p,jb,PdCode.busy);return;}
    if route==PqRoute.cancel {
        for i:0..<PQ_SLOTS {
            s:=p[0].q[0].slots[i].state;if s==PqState.free {continue;}
            c:=pd_cancel(p[0].slots[i].held,s==PqState.running,jb);
            if c==PdCancel.drop && pq_cancel(p[0].q[..],i)==1 {
                p[0].cancelled+=1;print("pylon: cancel % drops waiting job %\n",py_short(id),py_short(p[0].slots[i].held.id));
                ok:=py_status(p,p[0].slots[i].held,PdCode.cancelled);
            } else if c==PdCancel.stop {
                // This host runs one decision at a time and reads frames only
                // between decisions, so a running job is never seen here.
                print("pylon: cancel % names the running job %; it finishes\n",py_short(id),py_short(p[0].slots[i].held.id));
            }
        }
        return;
    }
    slot:=pq_put(p[0].q[..]);
    if slot<0 {ok:=py_status(p,jb,PdCode.busy);return;}
    py_keep(p,slot,jb,cast(string)body[0..<jb.body]);
}
```

The loop reads every frame that is ready before it runs the next job, so
a cancel that arrived while the engine was busy finds its job still
waiting:

<!-- source: psionic/lib/pylon/run.coax loop -->
```jai
    while true {
        // Frames first, so a job that arrived while the engine ran is held
        // before the stop request is weighed.
        if !py_pump(p,200) {return 6;}
        now=pyh_now_s();
        l:=pq_life(pyh_stop()==1,pq_held(p[0].q[..]));
        if l==PqLife.off {break;}
        was:=p[0].life.status;
        if l==PqLife.drain {p[0].life=pb_drain(p[0].life);}
        snap:=pb_snap(p[0].life,pq_free(p[0].q[..]),1);
        if pq_beacon_now(pb_due(p[0].life,snap,c.refresh,now),was!=p[0].life.status,l!=PqLife.run) {if !py_beacon(p,now) {return 5;}}
        slot:=pq_next(p[0].q[..]);
        if slot>=0 {py_run(p,slot);}
    }
```

This host runs one decision at a time and calls the engine over HTTP
(`py_http`, a `POST /v1/systemone` with the job's budget as its time
limit). That is how the same program reaches chapter 16's toy engine on
this machine and Elephant on the cloud L4 through a tunnel. A cancel for
the job that is running cannot be seen until it has finished, so it is
logged and the answer goes out; only waiting jobs are cancelled.

The signer is the one boundary that is test-only:

<!-- source: psionic/lib/pylon/run.coax sign -->
```jai
// TEST-ONLY, shaped as nostr_event_sign_step: the signed event's JSON into
// out and its length, 0 for "call again" (never, here), -1 refused.
py_sign_step :: (key:[]u8,aux:[]u8,created_at:int,kind:int,tags:string,content:string,work:[]PySignWork,scratch:[]u8,out:[]u8,overlap:bool)->int {
    if overlap || key.count!=32 || aux.count!=32 || !json_valid(tags) {return -1;}
    hk:[64]u8;ha:[64]u8;
    n:=json_raw(scratch,0,"{\"op\":\"sign\",\"key\":\"");n=json_raw(scratch,n,py_hex(key,hk[..]));
    n=json_raw(scratch,n,"\",\"aux\":\"");n=json_raw(scratch,n,py_hex(aux,ha[..]));
    n=json_raw(scratch,n,"\",\"created_at\":");n=json_write_int(scratch,n,created_at);
    n=json_raw(scratch,n,",\"kind\":");n=json_write_int(scratch,n,kind);
    n=json_raw(scratch,n,",\"tags\":");n=json_raw(scratch,n,tags);
    n=json_raw(scratch,n,",\"content\":");n=json_write_string(scratch,n,content);n=json_raw(scratch,n,"}");
    for i:0..<hk.count {hk[i]=0;}
    if n<0 {return -1;}
    m:=py_double(work,cast(string)scratch[0..<n],out);
    for i:0..<n {scratch[i]=0;}
    return m;
}
```

It has `nostr_event_sign_step`'s arguments and its return values (the
signed event's length, `0` to call again, `-1` refused). When
`event_sign.coax` lands, the call sites change from `py_sign_step` to
`nostr_event_sign_step` and `PySignWork` to `NostrSignWork`, and nothing
else moves.

## Run it

The end-to-end check builds our own Coax relay and the Pylon with ASan and
UBSan, starts the relay on `127.0.0.1`, an engine, the Pylon, and a buyer
(`psionic/tests/pylon/buyer.ts`) that checks everything from outside with
independent code: `@noble/curves` for signatures and `nostr-effect`'s
NIP-44. It never connects to a public relay.

```shell
python3 -I psionic/tests/pylon/pylon_e2e.py build/pylon-e2e
```

With chapter 16's toy engine (`psionic-serve --toy`, holding each decision
2.5 s so that jobs wait), the buyer's checks and the Pylon's log:

```jai
ok   the Coax relay listens on 127.0.0.1:47293
ok   psionic-serve --toy listens on 127.0.0.1:37341
ok   the Pylon authenticated, published its list and beacon, and subscribed
ok   the relay sent a NIP-42 challenge
ok   the buyer authenticated (NIP-42) with its test key
ok   the Pylon's 30200 beacon is on the relay
ok   the beacon's id and BIP-340 signature verify
ok   the beacon says online, v1
ok   the beacon names the served identity psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573
ok   the service is this key's pylon/decision, lane cj-decision
ok   the x tag is the content's SHA-256
ok   the Pylon's NIP-65 relay list is on the relay
ok   the list names wss://relay.openagents.com and ws://127.0.0.1:47293
ok   job r-1 (25910, NIP-44 to the Pylon) accepted by the relay
ok   r-1 answered with a 26910
     r-1 answer after 4755 ms: {"wet":{"type":"choice","choice":"no","confidence":0.562052749,"probabilities":{"yes":0.437947251,"no":0.562052749}}}
ok   the 26910 is signed by the Pylon, to the buyer, naming the job
ok   outcome answered for request r-1, attempt 1
ok   the answer names the beacon's identity psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573
ok   service door pylon:ch19-local, provider the Pylon's key
ok   the choice's probabilities sum to 1
ok   job r-2 (v0) accepted by the relay
ok   r-2 refused: 27010 error unsupported_version
ok   jobs r-3 and r-4 accepted by the relay
ok   the cancel for r-4 accepted by the relay
ok   r-3, already running, answered
ok   r-4, still waiting, cancelled: 27010 error cancelled
ok   no 26910 for r-4
ok   jobs r-5 and r-6 accepted by the relay
ok   r-5 and r-6 both answered after the stop
ok   the beacon went offline
     beacon statuses seen: online -> draining -> offline
ok   draining before offline
ok   the offline beacon verifies and offers no free slot
buyer: ok
ok   the buyer's checks held
--- pylon log ---
pylon: TEST-ONLY key and signing double; pubkey 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b
pylon: engine psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 answered the probe in 2758 ms
pylon: authenticated to ws://127.0.0.1:47293 as 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b
pylon: relay list of 2
pylon: beacon online generation 1 free 4 of 4
pylon: online, waiting for jobs
pylon: caught up on pylon-jobs
pylon: event 150f8547903a kind 25910 -> queue (none)
pylon: job 150f8547903a -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2723 ms
pylon: event f5eec94bc750 kind 25910 -> refuse (unsupported_version)
pylon: job f5eec94bc750 -> 27010 error unsupported_version
pylon: event 90cc85046c2e kind 25910 -> queue (none)
pylon: event 8510c534fb33 kind 25910 -> queue (none)
pylon: event 1c267a7c6492 kind 25910 -> cancel (none)
pylon: cancel 1c267a7c6492 drops waiting job 8510c534fb33
pylon: job 8510c534fb33 -> 27010 error cancelled
pylon: beacon online generation 1 free 3 of 4
pylon: job 90cc85046c2e -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2673 ms
pylon: event 4de19ab85d55 kind 25910 -> queue (none)
pylon: event 57c80e304a13 kind 25910 -> queue (none)
pylon: beacon draining generation 1 free 0 of 4
pylon: job 4de19ab85d55 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2632 ms
pylon: job 57c80e304a13 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2711 ms
pylon: beacon offline generation 1 free 0 of 4
pylon: offline. sent 12, accepted 12, refused 0, answered 4, refusals 2, cancelled 1, forged 0, duplicates 0, ignored 0, beacons 4
--- end ---
ok   the Pylon stopped cleanly after the offline beacon
ok   the relay accepted every event the Pylon published; no forged input
ok   no sanitizer report from the Pylon
ok   no sanitizer report from the relay
pylon e2e: ok
```

The cancelled job `8510c534fb33` was waiting behind `90cc85046c2e`; it got
a 27010 `cancelled` and never a 26910. The stop arrived while
`4de19ab85d55` ran and `57c80e304a13` waited, so the beacon went
draining, both were answered, and then it went offline with no free
slot. Every event the Pylon published was accepted by the relay after its
own BIP-340 check, and neither process reported to the sanitizers.

The same run against Elephant, `elephant-1` on a Google Cloud L4, reached
through an IAP tunnel at `127.0.0.1:18797`:

```shell
ENGINE=http://127.0.0.1:18797 python3 -I psionic/tests/pylon/pylon_e2e.py build/pylon-e2e
```

```jai
ok   the Coax relay listens on 127.0.0.1:60865
ok   the Pylon authenticated, published its list and beacon, and subscribed
ok   the relay sent a NIP-42 challenge
ok   the buyer authenticated (NIP-42) with its test key
ok   the Pylon's 30200 beacon is on the relay
ok   the beacon's id and BIP-340 signature verify
ok   the beacon says online, v1
ok   the beacon names the served identity elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
ok   the service is this key's pylon/decision, lane cj-decision
ok   the x tag is the content's SHA-256
ok   the Pylon's NIP-65 relay list is on the relay
ok   the list names wss://relay.openagents.com and ws://127.0.0.1:60865
ok   job r-1 (25910, NIP-44 to the Pylon) accepted by the relay
ok   r-1 answered with a 26910
     r-1 answer after 3208 ms: {"wet":{"type":"choice","choice":"yes","confidence":0.757202773,"probabilities":{"yes":0.757202773,"no":0.242797227}}}
ok   the 26910 is signed by the Pylon, to the buyer, naming the job
ok   outcome answered for request r-1, attempt 1
ok   the answer names the beacon's identity elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
ok   service door pylon:ch19-local, provider the Pylon's key
ok   the choice's probabilities sum to 1
ok   job r-2 (v0) accepted by the relay
ok   r-2 refused: 27010 error unsupported_version
ok   jobs r-5 and r-6 accepted by the relay
ok   r-5 and r-6 both answered after the stop
ok   the beacon went offline
     beacon statuses seen: online -> draining -> offline
ok   the offline beacon verifies and offers no free slot
buyer: ok
ok   the buyer's checks held
--- pylon log ---
pylon: TEST-ONLY key and signing double; pubkey 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b
pylon: engine elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba answered the probe in 361 ms
pylon: authenticated to ws://127.0.0.1:60865 as 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b
pylon: relay list of 2
pylon: beacon online generation 1 free 4 of 4
pylon: online, waiting for jobs
pylon: caught up on pylon-jobs
pylon: event 29cbd3406652 kind 25910 -> queue (none)
pylon: job 29cbd3406652 -> 26910 answered by elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba in 326 ms
pylon: event 8b147ffceee1 kind 25910 -> refuse (unsupported_version)
pylon: job 8b147ffceee1 -> 27010 error unsupported_version
pylon: event fe2a2f1a149d kind 25910 -> queue (none)
pylon: event 473c5d9b674a kind 25910 -> queue (none)
pylon: beacon draining generation 1 free 0 of 4
pylon: job fe2a2f1a149d -> 26910 answered by elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba in 355 ms
pylon: job 473c5d9b674a -> 26910 answered by elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba in 707 ms
pylon: beacon offline generation 1 free 0 of 4
pylon: offline. sent 9, accepted 9, refused 0, answered 3, refusals 1, cancelled 0, forged 0, duplicates 0, ignored 0, beacons 3
--- end ---
ok   the Pylon stopped cleanly after the offline beacon
ok   the relay accepted every event the Pylon published; no forged input
ok   no sanitizer report from the Pylon
ok   no sanitizer report from the relay
pylon e2e: ok
```

The answer is chapter 10's sample, `yes` with p 0.757202773, and the
identity in the beacon and the answer is Elephant's own,
`elephant-1@sha256:682f0234…`. The engine took 326 ms. The buyer waited
about 3.2 s from publishing to reading the answer: two BIP-340
verifications at the relay and one at the Pylon, each under ASan, are
most of that. The cancel case needs a job that waits, which the toy
engine's delay arranges and a 0.35 s engine does not, so it runs with the
toy engine only.

## What this does not do yet

- **Real keys.** Signing and the conversation key are the double above.
  Nothing here may be pointed at `relay.openagents.com` until they are
  Coax and the owner's key file is wired.
- **Reconnects.** The connection is opened once; if the relay drops it the
  Pylon stops. `coax/lib/nostr/relay.coax` has the reconnect and catch-up
  machine; moving to it comes with `message.coax`.
- **`wss://`.** The Pylon speaks `ws://` only. `coax/platform/tls-native.c`
  exists; the Pylon does not use it yet.
- **One decision at a time, no stop for a running one.** A running job is
  never cancelled mid-decision.
- **The receipt.** The 26910 carries a receipt object with `attempt_id`,
  `schema` and `transport`, not yet the request digest the deployed reader
  checks.
- **The owner's `auth` tag** on the beacon, and many Pylons together:
  [chapter 20](PLAN.md) fans the API out across them, and
  [chapter 21](PLAN.md) adds receipts and checks.

See the [verification record](../../docs/verification/psionic-ch19-2026-10-29.md)
for what ran and the exact swaps.
