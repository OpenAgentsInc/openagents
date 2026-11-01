# 173. Serve a model with an honest capability

> **Status: partly implemented.** The capability document is derived from
> qualified records, checked so it never claims more than they cover, served
> at `GET /v1/capability`, and enforced by admission before model work, on
> chapter 20's toy engine on the CPU. Schemas for the document in `proto/`
> and `docs/protocol/`, wiring the route into the live host, cancellation
> timed by chapter 172's harness, a protobuf projection of the document, and
> a qualified Elephant entry are not built yet.

[Chapter 20](ch020-serve-systemone.md)'s server answers any NIP-DEC request
that names its model and fits its buffers. A client cannot ask it what it
can actually do: which checkpoint answers, on which backend, under which
numerical profile, how large a request has been tested, and how long a
decision may take. This chapter adds that answer. The server publishes a
capability document, each entry in the document is backed by a recorded
qualified run, and admission refuses every request the document does not
cover.

The rule that makes the document honest is short: **an entry never claims
more than its record covered.** A record says that one checkpoint, on one
backend, under one profile, passed [chapter 17](ch017-qualify-honestly.md)'s
comparison on requests up to some size and within some deadline. The
operator's claim can ask for more; the published entry is the claim
narrowed to the record. An entry with no qualified record reads as
`unknown` and admits nothing.

The library is [`serve-capability.coax`](../lib/serve-capability.coax), an
`#authority` file because it decides admission. It builds on chapter 20's
request decoder, engine and slot machine, chapter 17's verdicts, and the
memory plans of chapters 22 and 33. It adds no model code.

```shell
coax/bin/coax run psionic/examples/serve-capability.coax
coax/bin/coax test psionic/lib/serve-capability.coax
```

## Records, claims and entries

Four values carry the whole idea:

<!-- source: psionic/lib/serve-capability.coax types -->
```jai
CpBackend :: enum u8 {none;cpu;cuda;metal;}
// The largest request an entry admits, and the decision deadline it keeps.
CpLimits :: struct {questions:int;options:int;state:int;deadline_ms:int;}
// One recorded run: which checkpoint ran where under which profile, its
// chapter-17 verdict, and the largest request and slowest deadline it covered.
CpEvidence :: struct {digest:string;backend:CpBackend;profile:string;record:string;verdict:QyVerdict;covered:CpLimits;}
// What the operator asks to publish, and the bytes its residency plan needs.
CpClaim :: struct {model:string;alias:string;digest:string;backend:CpBackend;profile:string;limits:CpLimits;memory:int;}
CpCause :: enum u8 {none;claim;memory;no_record;not_qualified;over;duplicate;}
CpEntry :: struct {claim:CpClaim;qualified:bool;cause:CpCause;record:string;limits:CpLimits;}
CpFault :: struct {cause:CpCause;entry:int;}
CpAdmit :: struct {code:SvCode;entry:int;}
```

- `CpEvidence` is one recorded run. `covered` holds the largest request it
  ran (questions, options in one question, bytes of state) and the deadline
  it needed. `verdict` is chapter 17's `QyVerdict`; only `qualified` counts.
- `CpClaim` is what the operator asks to publish, plus `memory`: the bytes
  the model needs resident, from chapter 22's `ls_memory` for a loaded
  engine or chapter 33's `rp_plan` for a model too large to load here.
- `CpEntry` is what the server publishes for one claim.
- `CpFault` names the first entry of a document that fails the honesty
  check, and why.

Every field has a bound. A name or record is 1 to 128 visible ASCII bytes,
a profile 1 to 64, a digest 64 lowercase hex characters, and the limits stay
inside NIP-DEC's own: 1 to 64 questions, 1 to 255 options, 1 to 131,072
bytes of state, and a deadline of 1 ms to one day.

<!-- source: psionic/lib/serve-capability.coax valid -->
```jai
cp_limits_valid :: (l:CpLimits)->bool {
    return l.questions>=1 && l.questions<=SV_QUESTIONS_MAX && l.options>=1 && l.options<=SV_OPTIONS_MAX
        && l.state>=1 && l.state<=sv_nip().state && l.deadline_ms>=1 && l.deadline_ms<=CP_DEADLINE_MOST;
}
cp_claim_valid :: (c:CpClaim)->bool {
    return cp_text(c.model,CP_NAME_MAX) && (c.alias.count==0 || cp_text(c.alias,CP_NAME_MAX)) && cp_hex64(c.digest)
        && c.backend!=CpBackend.none && cp_text(c.profile,64) && cp_limits_valid(c.limits) && c.memory>=0;
}
cp_evidence_valid :: (e:CpEvidence)->bool {
    return cp_hex64(e.digest) && e.backend!=CpBackend.none && cp_text(e.profile,64) && cp_text(e.record,CP_NAME_MAX) && cp_limits_valid(e.covered);
}
```

## Narrow the claim to the record

Sizes and deadlines narrow in opposite directions. A record that ran two
questions says nothing about three, so the question bound shrinks to the
smaller of claim and record. A record that needed 30,000 ms says nothing
about answering in 10,000 ms, so the deadline grows to the larger of the
two. A client that waits the published deadline is then waiting at least as
long as the qualified run needed.

<!-- source: psionic/lib/serve-capability.coax narrow -->
```jai
cp_matches :: (c:CpClaim,e:CpEvidence)->bool {return cp_evidence_valid(e) && e.digest==c.digest && e.backend==c.backend && e.profile==c.profile;}
// Sizes shrink to what the record covered; the deadline grows to what it needed.
cp_narrow :: (claim:CpLimits,covered:CpLimits)->CpLimits {
    return CpLimits.{questions=cp_min(claim.questions,covered.questions),options=cp_min(claim.options,covered.options),
        state=cp_min(claim.state,covered.state),deadline_ms=cp_max(claim.deadline_ms,covered.deadline_ms)};
}
cp_covers :: (published:CpLimits,covered:CpLimits)->bool {
    return cp_limits_valid(published) && published.questions<=covered.questions && published.options<=covered.options
        && published.state<=covered.state && published.deadline_ms>=covered.deadline_ms;
}
```

`cp_min` and `cp_max` are written without a branch, as chapter 33's
`rp_min` is, so mutation testing has no equal-input comparison to argue
about.

An entry is qualified by the first record that matches its digest, backend
and profile with a `qualified` verdict. A claim that fails validation, a
memory need above the host's budget, a record that only `agrees` or
`failed`, or no matching record at all leaves the entry `unknown` with its
cause and zero limits:

<!-- source: psionic/lib/serve-capability.coax entry -->
```jai
cp_entry :: (c:CpClaim,evidence:[]CpEvidence,budget:int)->CpEntry {
    e:=CpEntry.{claim=c,cause=CpCause.no_record};
    if !cp_claim_valid(c) {e.cause=CpCause.claim;return e;}
    if budget<c.memory {e.cause=CpCause.memory;return e;}
    for r:evidence {
        if cp_matches(c,r) {
            if r.verdict==QyVerdict.qualified {return CpEntry.{claim=c,qualified=true,record=r.record,limits=cp_narrow(c.limits,r.covered)};}
            e.cause=CpCause.not_qualified;
        }
    }
    return e;
}
```

### Work one entry by hand

The example's toy record is chapter 20's checked transcript,
[`serve-decision.out`](../fixtures/serve-decision.out): two questions, the
larger with two options, about the 22 bytes of `The kettle has boiled.`,
inside a 30,000 ms deadline. The claim asks for NIP-DEC's whole range and a
10,000 ms deadline:

| Limit | Claim | Record covered | Published |
| --- | --- | --- | --- |
| Questions | 64 | 2 | min(64, 2) = 2 |
| Options | 255 | 2 | min(255, 2) = 2 |
| State bytes | 131,072 | 22 | min(131072, 22) = 22 |
| Deadline | 10,000 ms | 30,000 ms | max(10000, 30000) = 30,000 ms |

Memory decides first. The host budget is 16 GiB, 17,179,869,184 bytes. The
toy needs 6,034,208 bytes, so it fits. Elephant at a 128-token capacity
needs 17,986,464,768 bytes by chapter 33's plan, which is more than the
budget, so its entry reads `unknown (memory)` before any record is looked
at. The same toy claim on CUDA matches no record, because the one record
names the CPU: `unknown (no_record)`.

Chapter 20's request has two questions (a noul and a two-option choice) and
22 bytes of state, all within the published limits, so it is admitted. The
slot's deadline is the arrival time plus the published deadline:
3 + 30,000 = 30,003 ms. A request with three questions exceeds 2 and is
refused with 422 `unsupported_input` before any model work.

The first check states these numbers, and the case without the record:

<!-- source: psionic/lib/serve-capability.coax check-entry -->
```jai
#test "an entry is qualified only by a matching qualified record, narrowed to it" {
    c:=cp_test_claim();r:=cp_test_record();
    one:=[1]CpEvidence.[r];e:=cp_entry(c,one[..],1000);
    #assert e.qualified && e.cause==CpCause.none && e.record=="r1";
    // 64 questions narrow to the record's 2, 255 options to 2, 131072 state
    // bytes to 64; the 10000 ms deadline widens to the 30000 ms the run needed.
    #assert e.limits.questions==2 && e.limits.options==2 && e.limits.state==64 && e.limits.deadline_ms==30000;
    // Remove the record and the same claim reads unknown.
    none:=one;e=cp_entry(c,none[0..<0],1000);
    #assert !e.qualified && e.cause==CpCause.no_record && e.record=="" && cp_zero(e.limits);
```

## Derive and publish the document

`cp_derive` turns up to 16 claims into entries. Two qualified entries that a
client could reach with one name (a model or an alias) are ambiguous; the
second becomes `unknown (duplicate)`. Unknown entries may share a name, so
the same model can list a qualified CPU entry and an unknown CUDA entry.

<!-- source: psionic/lib/serve-capability.coax derive -->
```jai
cp_derive :: (claims:[]CpClaim,evidence:[]CpEvidence,budget:int,out:[]CpEntry)->int {
    if claims.count>CP_ENTRIES_MAX || out.count<claims.count {return -1;}
    for i:0..<claims.count {
        e:=cp_entry(claims[i],evidence,budget);
        for k:0..<i {if e.qualified && out[k].qualified && cp_clash(out[k].claim,e.claim) {e=CpEntry.{claim=e.claim,cause=CpCause.duplicate};}}
        out[i]=e;
    }
    return claims.count;
}
```

`cp_render` writes the document as `psionic-capability-v1` JSON, and
`cp_response` wraps it in an HTTP 200 for `GET /v1/capability`. A qualified
entry publishes its record and limits. An unknown entry publishes only its
cause. An entry whose claim failed validation publishes none of the claim's
text. The identity string is the one chapter 20's answers carry in
`service.identity`: the name, `@sha256:`, and the checkpoint digest.

<!-- source: psionic/lib/serve-capability.coax render -->
```jai
cp_render :: (entries:[]CpEntry,out:[]u8)->int {
    at:=json_raw(out,0,"{\"schema\":\"");at=json_raw(out,at,CP_SCHEMA);at=json_raw(out,at,"\",\"door\":\"");at=json_raw(out,at,SV_DOOR);
    at=json_raw(out,at,"\",\"version\":\"");at=json_raw(out,at,SV_VERSION);at=json_raw(out,at,"\",\"entries\":[");
    for i:0..<entries.count {
        e:=entries[i];if i>0 {at=json_raw(out,at,",");}
        // A claim that failed validation publishes none of its text.
        if e.cause==CpCause.claim {at=json_raw(out,at,"{\"state\":\"unknown\",\"cause\":\"claim\"}");continue;}
        at=json_raw(out,at,"{\"model\":");at=json_write_string(out,at,e.claim.model);
        at=json_raw(out,at,",\"alias\":");at=json_write_string(out,at,e.claim.alias);
        // The identity is the escaped name, then @sha256: and the hex digest, in one string.
        at=json_raw(out,at,",\"identity\":");at=json_write_string(out,at,e.claim.model)-1;
        at=json_raw(out,at,"@sha256:");at=json_raw(out,at,e.claim.digest);at=json_raw(out,at,"\",\"backend\":\"");at=json_raw(out,at,cp_backend_name(e.claim.backend));
        at=json_raw(out,at,"\",\"profile\":");at=json_write_string(out,at,e.claim.profile);
        if e.qualified {
            at=json_raw(out,at,",\"state\":\"qualified\",\"record\":");at=json_write_string(out,at,e.record);
            at=cp_int(out,at,",\"limits\":{\"questions\":",e.limits.questions);at=cp_int(out,at,",\"options\":",e.limits.options);
            at=cp_int(out,at,",\"state_bytes\":",e.limits.state);at=cp_int(out,at,",\"deadline_ms\":",e.limits.deadline_ms);at=json_raw(out,at,"}}");
        } else {
            at=json_raw(out,at,",\"state\":\"unknown\",\"cause\":\"");at=json_raw(out,at,cp_cause_name(e.cause));at=json_raw(out,at,"\"}");
        }
    }
    return json_raw(out,at,"]}");
}
```

The document is in-process: the JSON shape is fixed by the exact-string
check in the library, not by a schema in `proto/` or a version of the
protocol spec. Publishing it as a protocol contract would start there, as a
new versioned message.

## Check the document against its records

A document can be edited after it is derived, or derived by a different
program. `cp_honest` checks any document against the records, independently
of how it was made. Each qualified entry needs a valid claim, a memory need
within the budget, a qualified record with the entry's digest, backend,
profile and record name, and limits that record covers. Each unknown entry
must publish no limits and no record. No two qualified entries may share a
name.

<!-- source: psionic/lib/serve-capability.coax honest -->
```jai
cp_honest :: (entries:[]CpEntry,evidence:[]CpEvidence,budget:int)->CpFault {
    if entries.count>CP_ENTRIES_MAX {return CpFault.{cause=CpCause.claim,entry=-1};}
    for i:0..<entries.count {
        e:=entries[i];
        if !e.qualified {
            if !cp_zero(e.limits) || e.record.count>0 {return CpFault.{cause=CpCause.over,entry=i};}
            continue;
        }
        if !cp_claim_valid(e.claim) {return CpFault.{cause=CpCause.claim,entry=i};}
        if budget<e.claim.memory {return CpFault.{cause=CpCause.memory,entry=i};}
        found:=-1;
        for k:0..<evidence.count {r:=evidence[k];if cp_matches(e.claim,r) && r.verdict==QyVerdict.qualified && r.record==e.record {found=k;break;}}
        if found<0 {return CpFault.{cause=CpCause.no_record,entry=i};}
        if !cp_covers(e.limits,evidence[found].covered) {return CpFault.{cause=CpCause.over,entry=i};}
        for k:0..<i {if entries[k].qualified && cp_clash(entries[k].claim,e.claim) {return CpFault.{cause=CpCause.duplicate,entry=i};}}
    }
    return CpFault.{entry=-1};
}
```

<!-- source: psionic/lib/serve-capability.coax check-honest -->
```jai
#test "a published document never claims more than its records" {
    c:=cp_test_claim();r:=cp_test_record();ev:=[1]CpEvidence.[r];doc:[2]CpEntry;
    cuda:=c;cuda.backend=CpBackend.cuda;claims:=[2]CpClaim.[c,cuda];
    #assert cp_derive(claims[..],ev[..],1000,doc[..])==2;
    f:=cp_honest(doc[..],ev[..],1000);#assert f.cause==CpCause.none && f.entry==-1;
    // A hand-edited entry that claims 64 questions where the record covered 2.
    forged:=doc;forged[0].limits.questions=64;
    f=cp_honest(forged[..],ev[..],1000);#assert f.cause==CpCause.over && f.entry==0;
```

## Admit against the document

Admission runs after chapter 20's decoder has turned the body into a typed
request, and before any tokenization or model work. It finds the qualified
entry by name or alias and compares the request with that entry's limits:

<!-- source: psionic/lib/serve-capability.coax admit -->
```jai
cp_find :: (entries:[]CpEntry,model:string)->int {
    for i:0..<entries.count {
        e:=entries[i];if e.qualified && (e.claim.model==model || (e.claim.alias.count>0 && e.claim.alias==model)) {return i;}
    }
    return -1;
}
cp_admit :: (entries:[]CpEntry,request:SvRequest,pools:SvPools)->CpAdmit {
    if request.code!=SvCode.none {return CpAdmit.{code=request.code,entry=-1};}
    i:=cp_find(entries,request.model);if i<0 {return CpAdmit.{code=SvCode.not_admitted,entry=-1};}
    l:=entries[i].limits;
    if request.questions>l.questions {return CpAdmit.{code=SvCode.unsupported_input,entry=i};}
    for q:0..<request.questions {if pools.questions[q].count>l.options {return CpAdmit.{code=SvCode.unsupported_input,entry=i};}}
    if request.state.value.count>l.state {return CpAdmit.{code=SvCode.limit_exceeded,entry=i};}
    return CpAdmit.{entry=i};
}
```

| Request | Status | Code |
| --- | --- | --- |
| Model or alias with no qualified entry | 403 | `not_admitted` |
| More questions than the entry admits | 422 | `unsupported_input` |
| A question with more options than the entry admits | 422 | `unsupported_input` |
| More bytes of state than the entry admits | 413 | `limit_exceeded` |
| Any refusal from chapter 20's decoder | as chapter 20 | unchanged |

`cp_bytes` puts this inside chapter 20's slot machine. It runs `sv_bytes`
first, so every chapter 20 refusal is unchanged, then refuses anything
outside the document and sets the slot's deadline from the entry. It also
refuses with `internal` when the loaded engine is not the checkpoint the
entry advertises: same name, alias and digest, sealed, and able to answer
as many options as the entry publishes. The answer's `service.identity` is
written from that engine, so it is the identity the document advertises.

<!-- source: psionic/lib/serve-capability.coax bytes -->
```jai
// The engine that answers must be the checkpoint the entry advertises, and
// able to answer as many options as the entry publishes.
cp_serves :: (e:CpEntry,engine:SvEngine)->bool {
    return e.qualified && engine.ok && engine.sealed && engine.name==e.claim.model && engine.alias==e.claim.alias
        && engine.digest==e.claim.digest && e.limits.options<=engine.options;
}
cp_bytes :: (slot:SvSlot,limits:SvLimits,engine:SvEngine,entries:[]CpEntry,pools:SvPools,text:string,now:int)->SvStep {
    step:=sv_bytes(slot,limits,engine,pools,text,now);
    if step.action!=SvAction.decide {return step;}
    a:=cp_admit(entries,step.request,pools);
    if a.code==SvCode.none && !cp_serves(entries[a.entry],engine) {a.code=SvCode.internal;}
    if a.code!=SvCode.none {return SvStep.{slot=sv_idle(step.slot),action=SvAction.refuse,code=a.code};}
    s:=step;s.slot.deadline=now+entries[a.entry].limits.deadline_ms;return s;
}
```

## Run it

The [example](../examples/serve-capability.coax) derives the document for three claims, serves it, admits
chapter 20's request through `cp_bytes`, runs the toy decision, and refuses
a three-question request:

<!-- source: psionic/examples/serve-capability.coax example -->
```jai
ELEPHANT_SHA :: "682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba";
PROFILE :: "serve-cpu-bf16-v1";
ASK :: "POST /v1/systemone HTTP/1.1\r\nContent-Length: 193\r\n\r\n{\"model\":\"psionic-toy\",\"state\":\"The kettle has boiled.\",\"questions\":{\"hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"},\"drink\":{\"type\":\"choice\",\"criteria\":{\"tea\":null,\"soup\":\"a meal\"}}}}";
THREE :: "POST /v1/systemone HTTP/1.1\r\nContent-Length: 130\r\n\r\n{\"model\":\"psionic-toy\",\"state\":\"The kettle has boiled.\",\"questions\":{\"a\":{\"type\":\"noul\"},\"b\":{\"type\":\"noul\"},\"c\":{\"type\":\"noul\"}}}";

main :: () {
    toy:[1]SvToy;engine:=sv_test_toy(toy[..],400);
    budget:=16*1073741824;
    // The record: chapter 20's checked transcript answered 2 questions of at
    // most 2 options about 22 bytes of state, inside a 30000 ms deadline.
    records:=[1]CpEvidence.[CpEvidence.{digest=SV_TOY_SHA,backend=CpBackend.cpu,profile=PROFILE,
        record="psionic/fixtures/serve-decision.out",verdict=QyVerdict.qualified,
        covered=CpLimits.{questions=2,options=2,state=22,deadline_ms=30000}}];
    // The claims ask for NIP-DEC's whole range; memory comes from each residency plan.
    asked:=CpLimits.{questions=64,options=255,state=131072,deadline_ms=10000};
    toy_bytes:=ls_memory(engine,budget).bytes;
    elephant_bytes:=rp_plan(rp_elephant(),RpRequest.{capacity=128,chunk=128},budget).need;
    claims:=[3]CpClaim.[
        CpClaim.{model="psionic-toy",digest=SV_TOY_SHA,backend=CpBackend.cpu,profile=PROFILE,limits=asked,memory=toy_bytes},
        CpClaim.{model="psionic-toy",digest=SV_TOY_SHA,backend=CpBackend.cuda,profile=PROFILE,limits=asked,memory=toy_bytes},
        CpClaim.{model="elephant-1",alias="baselabs/elephant-1",digest=ELEPHANT_SHA,backend=CpBackend.cpu,profile=PROFILE,limits=asked,memory=elephant_bytes}];
    print("memory: toy % bytes, elephant-1 % bytes, budget % bytes\n",toy_bytes,elephant_bytes,budget);
    doc:[3]CpEntry;n:=cp_derive(claims[..],records[..],budget,doc[..]);
    for i:0..<n {
        e:=doc[i];
        if e.qualified {print("% %: qualified by %\n",e.claim.model,cp_backend_name(e.claim.backend),e.record);}
        else {print("% %: unknown (%)\n",e.claim.model,cp_backend_name(e.claim.backend),cp_cause_name(e.cause));}
    }
    fault:=cp_honest(doc[..],records[..],budget);print("honest: %\n",cp_cause_name(fault.cause));
    out:[4096]u8;
    if cp_probe("GET /v1/capability HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n") {m:=cp_response(doc[..],out[..]);print("%\n",cast(string)out[0..<m]);}
    // Chapter 20's request is inside the qualified entry: admitted, with the published deadline.
    storage:[1]ExamplePools;pools:=example_pools(storage[..]);limits:=SvLimits.{read_ms=10000,decide_ms=30000};
    slot:=sv_accept(SvSlot.{},limits,0).slot;
    step:=cp_bytes(slot,limits,engine,doc[..],pools,ASK,3);
    print("ask: action %, deadline at % ms, serves entry: %\n",step.action,step.slot.deadline,cp_serves(doc[0],engine));
    prep:[1]PrScratch;work:=sv_scratch(prep[..],engine);answers:[16320]f64;
    code:=sv_answers(engine,step.request,pools,work,answers[..]);
    span:=sv_render(engine,step.request,pools,answers[..],0,out[..]);if code!=SvCode.none {print("decision: %\n",sv_code_name(code));}
    body:=cast(string)out[span.from..<span.to];
    print("answer: %\nidentity matches the entry: %\n",cast(string)out[sv_body_start(body)+span.from..<span.to],sv_index(body,"\"identity\":\"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573\"")>0);
    // Three questions are outside it: refused before any model work.
    over:=cp_bytes(slot,limits,engine,doc[..],pools,THREE,3);m:=sv_refusal(out[..],over.code);
    print("three questions: action %\n%\n",over.action,cast(string)out[0..<m]);
}
```

```text
memory: toy 6034208 bytes, elephant-1 17986464768 bytes, budget 17179869184 bytes
psionic-toy cpu: qualified by psionic/fixtures/serve-decision.out
psionic-toy cuda: unknown (no_record)
elephant-1 cpu: unknown (memory)
honest: none
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 858
Connection: close

{"schema":"psionic-capability-v1","door":"psionic","version":"psionic-serve-v1","entries":[{"model":"psionic-toy","alias":"","identity":"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573","backend":"cpu","profile":"serve-cpu-bf16-v1","state":"qualified","record":"psionic/fixtures/serve-decision.out","limits":{"questions":2,"options":2,"state_bytes":22,"deadline_ms":30000}},{"model":"psionic-toy","alias":"","identity":"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573","backend":"cuda","profile":"serve-cpu-bf16-v1","state":"unknown","cause":"no_record"},{"model":"elephant-1","alias":"baselabs/elephant-1","identity":"elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba","backend":"cpu","profile":"serve-cpu-bf16-v1","state":"unknown","cause":"memory"}]}
ask: action decide, deadline at 30003 ms, serves entry: true
answer: {"model":"psionic-toy","answers":{"hot":{"type":"noul","noul":0.437308645},"drink":{"type":"choice","choice":"soup","confidence":0.562514618,"probabilities":{"tea":0.437485382,"soup":0.562514618}}},"usage":null,"service":{"door":"psionic","version":"psionic-serve-v1","identity":"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573"},"latency_ms":0}
identity matches the entry: true
three questions: action refuse
HTTP/1.1 422 Unprocessable Content
Content-Type: application/json
Content-Length: 109
Connection: close

{"error":{"code":"unsupported_input","message":"the request is valid but outside this model's text profile"}}
```

The answer body is byte for byte the one in chapter 20's transcript: the
document changes what is admitted, not how an admitted request is answered.

## What it refuses

The [second example](../examples/serve-capability-refuse.coax) breaks the document and the requests on purpose. Each
line is the typed cause from `cp_honest`, or the status and code from
`cp_admit`:

<!-- source: psionic/examples/serve-capability-refuse.coax refuse -->
```jai
main :: () {
    record:=CpEvidence.{digest=SV_TOY_SHA,backend=CpBackend.cpu,profile="serve-cpu-bf16-v1",
        record="psionic/fixtures/serve-decision.out",verdict=QyVerdict.qualified,
        covered=CpLimits.{questions=2,options=2,state=22,deadline_ms=30000}};
    claim:=CpClaim.{model="psionic-toy",digest=SV_TOY_SHA,backend=CpBackend.cpu,profile="serve-cpu-bf16-v1",
        limits=CpLimits.{questions=64,options=255,state=131072,deadline_ms=10000},memory=6034208};
    records:=[1]CpEvidence.[record];claims:=[1]CpClaim.[claim];doc:[1]CpEntry;
    n:=cp_derive(claims[..],records[..],1<<30,doc[..]);
    say("derived",cp_honest(doc[..],records[..],1<<30));
    forged:=doc;forged[0].limits.questions=64;say("64 questions where the record covered 2",cp_honest(forged[..],records[..],1<<30));
    forged=doc;forged[0].limits.deadline_ms=10000;say("a 10000 ms deadline where the run needed 30000",cp_honest(forged[..],records[..],1<<30));
    forged=doc;forged[0].record="psionic/fixtures/other.out";say("a record name nobody holds",cp_honest(forged[..],records[..],1<<30));
    say("the same document on a 1 MiB budget",cp_honest(doc[..],records[..],1<<20));
    // The record removed, or only agreeing: the entry reads unknown and admits nothing.
    gone:[1]CpEntry;n=cp_derive(claims[..],records[0..<0],1<<30,gone[..]);
    print("record removed: qualified %, cause %\n",gone[0].qualified,cp_cause_name(gone[0].cause));
    weak:=records;weak[0].verdict=QyVerdict.agrees;n=cp_derive(claims[..],weak[..],1<<30,gone[..]);
    print("record only agrees: qualified %, cause %\n",gone[0].qualified,cp_cause_name(gone[0].cause));
    one:="{\"model\":\"psionic-toy\",\"state\":\"Water is wet.\",\"questions\":{\"wet\":{\"type\":\"noul\"}}}";
    admit("toy request, record removed",gone[..],one);
    admit("toy request, record present",doc[..],one);
    admit("23 bytes of state",doc[..],"{\"model\":\"psionic-toy\",\"state\":\"The kettle has boiled!!\",\"questions\":{\"q\":{\"type\":\"noul\"}}}");
    admit("a choice of 3 options",doc[..],"{\"model\":\"psionic-toy\",\"state\":\"s\",\"questions\":{\"q\":{\"type\":\"choice\",\"criteria\":{\"a\":null,\"b\":null,\"c\":null}}}}");
    admit("elephant-1",doc[..],"{\"model\":\"elephant-1\",\"state\":\"s\",\"questions\":{\"q\":{\"type\":\"noul\"}}}");
    admit("no questions",doc[..],"{\"model\":\"psionic-toy\",\"state\":\"s\",\"questions\":{}}");
}
```

```text
derived: none at entry -1
64 questions where the record covered 2: over at entry 0
a 10000 ms deadline where the run needed 30000: over at entry 0
a record name nobody holds: no_record at entry 0
the same document on a 1 MiB budget: memory at entry 0
record removed: qualified false, cause no_record
record only agrees: qualified false, cause not_qualified
toy request, record removed: 403 not_admitted
toy request, record present: 200 admitted
23 bytes of state: 413 limit_exceeded
a choice of 3 options: 422 unsupported_input
elephant-1: 403 not_admitted
no questions: 400 invalid_request
```

Both examples exit with status 0, and the book's runner compares each
transcript exactly in debug and release builds.

## The independent check

The checks above use hand-derived values. A `#forall` adds a brute-force
oracle. For each of 243 mixes of three records (each qualified, only
agreeing, or on the wrong backend, with different covered limits) and three
claim sizes, it reads the rules directly: scan the records from last to
first and keep the lowest-indexed qualified CPU record; with none, the
cause is `not_qualified` if some CPU record exists and `no_record`
otherwise. It then compares that reading with `cp_entry`, field by field,
and for every qualified result checks that:

- `cp_honest` accepts the derived entry, and refuses it with one more
  question than the record covered;
- a request exactly at every limit is admitted;
- one more question, one more option, or one more byte of state is refused
  with the code in the table above.

`coax mutate psionic/lib/serve-capability.coax --root-only` reports 267
mutants, 267 killed, 0 survived. `--root-only` mutates this file; the
chapter 20 and chapter 17 units it loads carry their own mutation gates.

## Limits

- The document is the in-process profile `psionic-capability-v1`. Its
  request and response messages are not yet in `proto/` or
  `docs/protocol/`, and there is no protobuf projection of it beside
  [chapter 21](ch021-the-protobuf-wire.md)'s.
- The live host ([`serve-host.c`](../platform/serve-host.c)) does not yet route
  `GET /v1/capability` or call `cp_bytes`; the examples drive the Coax side
  with clock values as inputs, as chapter 20's example does.
- Records are written in the example, not read from chapter 17 record
  files, and the toy's record is a teaching record: it points at chapter
  20's checked transcript, which is exact but covers two questions only.
- No Elephant entry is qualified. Chapter 20 served Elephant on CPU and
  CUDA, but one served answer is not a qualification record for a serving
  profile, and in the example the 16 GiB budget refuses it first.
- Token counts are still bounded after tokenization by chapter 20's
  `sv_fits`, against the engine's capacity, not by the document.
- Cancellation releasing its arena within a stated time, measured by
  [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s harness, is
  not built.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md)
