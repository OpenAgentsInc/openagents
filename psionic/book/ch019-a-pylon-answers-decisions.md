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

One piece is a stand-in until other work lands on main. The chapter says
so wherever it appears.

- **The key.** The Pylon's key is the SHA-256 of a label, made for tests.
  `--key-file` is accepted as configuration (the owner's choice: a `0600`
  file in `~/work/.secrets/`) and refused until the secret-handling audit
  (#4828) admits real keys.

Relay frames were a stand-in too. They are read by `rl_read` in
`coax/lib/nostr/relay_read.coax` now: `message.coax` (#6015) reads the
relay's EVENT, OK, EOSE, CLOSED and NOTICE frames, an EVENT's id and
signature are checked with the fast verifier, and NIP-42's `auth-required:`
is mapped onto the relay machine's input before the machine sees it. An
AUTH challenge (`rl_read`'s kind 0) goes to `nip42_input`, and
`nostr_message_write_req`, `_close` and `_event` write what the Pylon sends.

Signing and NIP-44 were stand-ins in the first version of this chapter and
are Coax now. Every event is signed by `nostr_event_sign_step` in
`coax/lib/nostr/event_sign.coax` (`7bef91d26c`): BIP-340 over
`coax/lib/crypto/schnorr_sign.coax`, with 32 fresh bytes from the OS as the
auxiliary randomness of each signature. The NIP-44 conversation key comes
from the Pylon's key and the buyer's x-only key through
`coax/lib/nostr/nip44-key.coax` (`a7589e2c90`: x-only ECDH, then
HKDF-extract). Before that, a `@noble/curves` process did both over a pipe.

Everything else is the real code: the relay connection
(`coax/lib/net/relay_conn.coax`: `ws://` or `wss://`, the WebSocket from
`ws.coax`, TLS from `coax/platform/tls-native.c`), the relay client machine
(`coax/lib/nostr/relay.coax`: reconnect, backoff, resubscription), NIP-42
(`nip42.coax`), the event check (`event.coax`, a full BIP-340 verification
of every job), NIP-44, the beacon, the job rules and the engine.

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
hands over the arguments, the wall clock, a sleep for the reconnect
backoff and a stop request (SIGTERM, SIGINT or a stop file). Sockets are
`coax/platform/socket-native.c`, TLS `tls-native.c` (OpenSSL from Nix,
linked even for `ws://`), random bytes `entropy-foreign.c`, and the
signer's process `process-native.c`.

At start it signs nothing until it knows what it serves. It asks the
engine one fixed question and takes the identity from the answer's
`service.identity`; the beacon names that identity, and every answer is
checked against it. Then it connects (`--relay ws://HOST:PORT` or
`wss://HOST:PORT`, a host name rather than an address, which `relay_uri`
requires), answers the relay's NIP-42 challenge with a signed kind 22242,
and publishes its relay list (by default `wss://relay.openagents.com` and
the relay it connected to) and an online beacon. It subscribes to kind
25910 with a `#p` filter on its own key.

The connection is not opened once and trusted. `relay.coax`'s machine
owns it: each connection is a generation, a drop backs off (0.5 s doubling
to 30 s, jittered) and reconnects, and the subscription goes again with
`since` at the newest job delivered. Every published event waits in one
of the machine's publish slots, its signed bytes kept beside it, until the
relay says `OK`; on a new connection the ones without an `OK` go again. A
new connection is a new NIP-42 challenge, so what the Pylon may do on it is
`queue.coax`'s decision:

<!-- source: psionic/lib/pylon/queue.coax wire -->
```jai
// The step on an open connection. challenged: the relay's NIP-42 challenge
// for this connection is unanswered; authed: our AUTH for it was accepted;
// announced: this connection's beacon is queued; waited_ms: how long the
// connection has been open. Nothing is written (no REQ, no EVENT) before
// AUTH is accepted, because the relay refuses unauthenticated writes and a
// refused answer would be lost; a relay that never challenges or refuses the
// AUTH is dropped at limit_ms and the reconnect backs off.
pq_wire :: (challenged:bool,authed:bool,announced:bool,waited_ms:int,limit_ms:int)->PqWire #export_c {
    if authed {return ifx announced then PqWire.flow else PqWire.announce;}
    if waited_ms>=limit_ms {return PqWire.drop;}
    if challenged {return PqWire.auth;}
    return PqWire.wait;
}
```

Nothing is written before the relay has accepted this connection's AUTH,
because our relay refuses unauthenticated writes and a refused answer
would be lost. After AUTH comes a fresh beacon (the one that was waiting is
dropped for it), then the REQ and the queued answers. An answer the
engine finished while the relay was away is one of those queued answers.
A job the relay sends again after the resubscription (its `since`
boundary is inclusive) is dropped by the machine's id window and then by
`pq_seen`, so it is not answered twice.

Each job goes through the same steps:

<!-- source: psionic/lib/pylon/run.coax event -->
```jai
// ev is verified (py_take) and relay.coax delivered it once.
py_event :: (p:[]Py,event:NostrEvent) {
    ev:=event;
    if !pq_seen(p[0].q[..],ev.id[..]) {p[0].duplicates+=1;py_again(p,ev.id[..]);return;}
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
waiting. A drop needs nothing here: the next read reconnects.

<!-- source: psionic/lib/pylon/run.coax loop -->
```jai
    while true {
        // Frames first, so a job that arrived while the engine ran is held
        // before the stop request is weighed. A dropped connection comes
        // back here: py_step reconnects, re-authenticates, republishes the
        // beacon and resubscribes, and queued answers go out after AUTH.
        py_pump(p,200);
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

Signing is one call, `nostr_event_sign_step`, run until it writes the
event:

<!-- source: psionic/lib/pylon/run.coax sign -->
```jai
// One event signed in Coax: event_sign.coax's stages until it writes the
// event's JSON into out, with 32 fresh bytes from the OS as BIP-340's aux.
// The length, or -1 refused.
py_sign :: (p:[]Py,created_at:int,kind:int,tags:string,content:string,scratch:[]u8,out:[]u8)->int {
    aux:[32]u8;if !py_draw(32,aux[..]) {return -1;}
    n:=EVENT_MORE;
    while n==EVENT_MORE {n=nostr_event_sign_step(p[0].key[..],aux[..],created_at,kind,tags,content,p[0].signer[..],scratch,out,false);}
    for i:0..<32 {aux[i]=0;}
    return n;
}
```

It works in stages (the key, the id, the signature, the JSON) and returns
`EVENT_MORE` until the last one. Before it returns the event, `event.coax`
reads the JSON back and recomputes the id. The Pylon learns its own public
key the same way: it signs an event it never sends and reads `pubkey` back
(`py_public`).

## Run it

The end-to-end check builds our own Coax relay and the Pylon with ASan and
UBSan, starts the relay on `127.0.0.1` with `--store`, an engine, the
Pylon, and a buyer (`psionic/tests/pylon/buyer.ts`) that checks everything
from outside with independent code: `@noble/curves` for signatures and
`nostr-effect`'s NIP-44. It never connects to a public relay.

```shell
python3 -I psionic/tests/pylon/pylon_e2e.py build/pylon-e2e
```

Since [chapter 159](ch159-a-pylon-checks-itself-before-serving.md), the
Pylon checks its engine against a shipped reference set before it
publishes anything. It takes `--references` and `--identity-state`. The
harness passes `psionic/fixtures/pylon/MODEL.selfcheck` and a fresh state
file, and checks that the self-check passed and the identity was recorded.

Midway, while job r-7 is with the engine, the harness stops the relay,
keeps it down for 4 s and starts it again on the same port and store. The
buyer reconnects as soon as it can and sends r-8 while the Pylon is still
backing off. Jobs and answers are ephemeral (25910, 26910 and 27010 are in
NIP-01's 20000 to 29999 range), so the relay stores neither. It forwards
each one only to the subscribers it has at that moment, so r-8 can reach
no Pylon, and r-7's answer, which the Pylon sends again on its new
connection, can go out before the buyer has resubscribed. So the buyer
does what any buyer without an answer does: once the Pylon's beacon is
back, it sends again the same signed jobs that have no answer yet. A job
the Pylon has not seen is decided. For a job it has already decided, it
sends the kept answer again (`pq_again`), the same event with the same id.
At the end the buyer checks that every job has exactly one answer event.

With chapter 16's toy engine (`psionic-serve --toy`, holding each decision
2.5 s so that jobs wait), the whole output:

```jai
ok   the Coax relay listens on 127.0.0.1:58301 (NIP-42 URL ws://localhost:58301, --store)
ok   psionic-serve --toy listens on 127.0.0.1:33483
ok   the Pylon authenticated, published its list and beacon, and subscribed
     relay stopped at 13.7 s; down for 4 s
ok   the relay restarted on the same port and store
ok   the relay sent a NIP-42 challenge
ok   the buyer authenticated (NIP-42) with its test key
ok   the Pylon's 30200 beacon is on the relay
ok   the beacon's id and BIP-340 signature verify
ok   the beacon says online, v1
ok   the beacon names the served identity psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573
ok   the service is this key's pylon/decision, lane cj-decision
ok   the x tag is the content's SHA-256
ok   the Pylon's NIP-65 relay list is on the relay
ok   the list names wss://relay.openagents.com and ws://localhost:58301
ok   job r-1 (25910, NIP-44 to the Pylon) accepted by the relay
ok   r-1 answered with a 26910
     r-1 answer after 3275 ms: {"wet":{"type":"choice","choice":"no","confidence":0.562052749,"probabilities":{"yes":0.437947251,"no":0.562052749}}}
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
ok   job r-7 accepted by the relay
ok   the harness restarted the relay
ok   the buyer reconnected and re-authenticated to the restarted relay
ok   job r-8 accepted by the restarted relay
ok   the Pylon republished its beacon to the restarted relay
     r-7: answer arrived without a resend
     r-8: no answer yet; the same signed job sent again after the Pylon's beacon came back
ok   r-7, in flight during the restart, answered after it
ok   r-8, sent while the Pylon was away, answered after it
ok   jobs r-5 and r-6 accepted by the relay
ok   r-5 and r-6 both answered after the stop
ok   the beacon went offline
     beacon statuses seen: online -> draining -> offline
ok   draining before offline
ok   every job got exactly one answer event
ok   the offline beacon verifies and offers no free slot
buyer: ok
ok   the buyer's checks held
--- pylon log ---
pylon: TEST-ONLY key, signed in Coax; pubkey 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b
pylon: engine psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 answered the probe in 2569 ms
pylon: relay list of 2
pylon: connection 1 open to ws://localhost:58301
pylon: authenticated to ws://localhost:58301 as 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b (connection 1)
pylon: beacon online generation 1 free 4 of 4 (connection 1)
pylon: REQ s0.1 since 1793341392 (connection 1)
pylon: caught up on s0.1 (connection 1)
pylon: online, waiting for jobs
pylon: event d255822e3df5 kind 25910 -> queue (none)
pylon: job d255822e3df5 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2570 ms
pylon: event bcc4842d3664 kind 25910 -> refuse (unsupported_version)
pylon: job bcc4842d3664 -> 27010 error unsupported_version
pylon: event 2053778e60a2 kind 25910 -> queue (none)
pylon: event 133f87bc249e kind 25910 -> queue (none)
pylon: job 2053778e60a2 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2542 ms
pylon: event dd57e68c1985 kind 25910 -> cancel (none)
pylon: cancel dd57e68c1985 drops waiting job 133f87bc249e
pylon: job 133f87bc249e -> 27010 error cancelled
pylon: event c95e533f1e06 kind 25910 -> queue (none)
pylon: beacon online generation 1 free 3 of 4 (connection 1)
pylon: job c95e533f1e06 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2570 ms
pylon: connection 1 ended (send failed); reconnecting in 518 ms
pylon: connection 2 ended (no WebSocket); reconnecting in 1237 ms
pylon: connection 3 open to ws://localhost:58301
pylon: authenticated to ws://localhost:58301 as 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b (connection 3)
pylon: beacon online generation 1 free 4 of 4 (connection 3)
pylon: REQ s0.1 since 1793341522 (connection 3), resumed from the newest job delivered
pylon: caught up on s0.1 (connection 3)
pylon: event 4554ec8d63b4 kind 25910 -> queue (none)
pylon: job 4554ec8d63b4 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2563 ms
pylon: event 653db1443573 kind 25910 -> queue (none)
pylon: event 10f8423bb162 kind 25910 -> queue (none)
pylon: beacon draining generation 1 free 0 of 4 (connection 3)
pylon: job 653db1443573 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2568 ms
pylon: job 10f8423bb162 -> 26910 answered by psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 in 2603 ms
pylon: beacon offline generation 1 free 0 of 4 (connection 3)
pylon: offline. sent 16, accepted 13, refused 0, answered 6, refusals 2, cancelled 1, forged 0, duplicates 0, ignored 0, beacons 5, connections 3, drops 2, auths 2, catch-ups 2
--- end ---
ok   the Pylon stopped cleanly after the offline beacon
ok   the relay accepted every event the Pylon published; no forged input
ok   the Pylon saw the drop and reconnected (2 connections opened)
ok   it re-authenticated on each connection, once per new challenge (2 AUTHs)
ok   it resubscribed with since moved to the newest job delivered (1793341392 -> 1793341522)
ok   it republished its beacon on the new connection
ok   no sanitizer report from the Pylon
ok   no sanitizer report from the relay
pylon e2e: ok
```

The relay went away while r-7 (`c95e533f1e06`) was with the engine. Its
answer was made with the relay gone, and the first try to send it failed
(`send failed`). One reconnect was refused while the relay was down. The
next connection was a new generation with a new NIP-42 challenge. The
Pylon signed a new 22242 for it, published a fresh beacon, resubscribed
with `since` at the newest job it had taken, and sent the waiting r-7
answer from its outbox; the buyer, already back, got it. r-8 had been sent
while no Pylon was subscribed, so it reached nobody. The buyer sent it
again once the beacon was back, and the Pylon decided it on that
connection (`4554ec8d63b4`). The cancel case and the shutdown order are
unchanged. Every event the Pylon published, each one signed in Coax, was
accepted by the relay after its own BIP-340 check, and neither process
reported to the sanitizers.

When an answer goes out before the buyer is back, the buyer's resend is a
job id the Pylon has seen, and `pq_again` sends the kept answer again. An
earlier run showed it: `job 435184d06b04 seen again; its answer
09e47cd93bc2 goes again`.

Over `wss://` the Pylon reaches the same relay through a TLS proxy in the
harness, with a CA and a `localhost` certificate generated for the run.
The Pylon is built with `--test-trust` and trusts only that CA:

```shell
python3 -I psionic/tests/pylon/pylon_e2e.py --wss build/pylon-e2e
```

```jai
ok   the Coax relay listens on 127.0.0.1:36865 (NIP-42 URL wss://localhost:45481, --store)
ok   job r-7 accepted by the relay
ok   job r-8 accepted by the restarted relay
     r-7: answer arrived without a resend
     r-8: no answer yet; the same signed job sent again after the Pylon's beacon came back
ok   r-7, in flight during the restart, answered after it
ok   r-8, sent while the Pylon was away, answered after it
ok   every job got exactly one answer event
pylon: connection 1 open to wss://localhost:45481
pylon: authenticated to wss://localhost:45481 as 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b (connection 1)
pylon: beacon online generation 1 free 4 of 4 (connection 1)
pylon: REQ s0.1 since 1793341431 (connection 1)
pylon: caught up on s0.1 (connection 1)
pylon: connection 1 ended (closed by the relay); reconnecting in 503 ms
pylon: beacon online generation 1 free 3 of 4 (connection 1)
pylon: connection 2 ended (no WebSocket); reconnecting in 1427 ms
pylon: connection 3 open to wss://localhost:45481
pylon: authenticated to wss://localhost:45481 as 1a2f3c48c45139da6541f87126d63da040222dde2219b93b405ec7570813345b (connection 3)
pylon: beacon online generation 1 free 4 of 4 (connection 3)
pylon: REQ s0.1 since 1793341564 (connection 3), resumed from the newest job delivered
pylon: caught up on s0.1 (connection 3)
pylon: beacon draining generation 1 free 0 of 4 (connection 3)
pylon: beacon offline generation 1 free 0 of 4 (connection 3)
ok   the Pylon spoke TLS to the proxy (3 TLS handshakes)
pylon e2e (wss): ok
```

The same run against Elephant, `elephant-1` on a Google Cloud L4, reached
through an IAP tunnel at `127.0.0.1:18797` (no cancel case: it needs a job
that waits, which a 0.3 s engine does not make):

```shell
ENGINE=http://127.0.0.1:18797 python3 -I psionic/tests/pylon/pylon_e2e.py build/pylon-e2e
```

```jai
ok   the beacon names the served identity elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
ok   r-1 answered with a 26910
     r-1 answer after 1464 ms: {"wet":{"type":"choice","choice":"yes","confidence":0.757202773,"probabilities":{"yes":0.757202773,"no":0.242797227}}}
ok   job r-7 accepted by the relay
ok   job r-8 accepted by the restarted relay
     r-7: answer arrived without a resend
     r-8: no answer yet; the same signed job sent again after the Pylon's beacon came back
ok   r-7, in flight during the restart, answered after it
ok   r-8, sent while the Pylon was away, answered after it
ok   every job got exactly one answer event
pylon: engine elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba answered the probe in 658 ms
pylon: connection 1 open to ws://localhost:36607
pylon: connection 1 ended (closed by the relay); reconnecting in 696 ms
pylon: connection 2 ended (no WebSocket); reconnecting in 1033 ms
pylon: connection 3 ended (no WebSocket); reconnecting in 2522 ms
pylon: connection 4 open to ws://localhost:36607
pylon: offline. sent 13, accepted 11, refused 0, answered 5, refusals 1, cancelled 0, forged 0, duplicates 0, ignored 0, beacons 4, connections 4, drops 3, auths 2, catch-ups 2
pylon e2e: ok
```

The answer is chapter 10's sample, `yes` with p 0.757202773, and the
identity in the beacon and the answer is Elephant's own,
`elephant-1@sha256:682f0234…`. This time the relay stayed down through two
refused reconnects (backoff 0.7, 1.0 and 2.5 s) before the fourth
connection opened.

The [Apple TLS extension](../../docs/verification/2026-10-30-apple-tls.md)
now qualifies this Pylon over `wss://` on macOS, including relay restart,
re-authentication, queued cancellation and encrypted answers. Native iOS
compilation is checked; iOS runtime remains unqualified.

## What this does not do yet

- **Real keys.** The key is a test key. Nothing here may be pointed at
  `relay.openagents.com` until the owner's key file is wired, after the
  secret-handling audit (#4828).
- **One relay.** The Pylon holds one connection. Its relay list names
  more, and a buyer may use any of them, but this host listens on one.
- **Replay beyond the windows.** Jobs are deduplicated by id in two
  64-id windows and `since` keeps reconnects to the newest job delivered.
  A relay that resent more than 64 older jobs at once could get a second
  answer for one of them; idempotency storage across restarts of the Pylon
  itself is not here either.
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
