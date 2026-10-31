# 20. Serve `/v1/systemone`

Chapter 14 answered one decision from a C harness. Callers do not link a
harness; they send `POST /v1/systemone` with the decision body that
[NIP-DEC](../../nips/openagents/NIP-DEC.md) defines, the same body a decision
job carries over Nostr. This chapter puts the chapter-14 path behind that
route.

The division of work is strict. A small C host opens a loopback socket, reads
and writes bounded bytes, reads a monotonic clock, maps model files and runs
each decision in a child process. Coax decides everything else: whether the
bytes are a complete request, every refusal and its HTTP status, the deadline,
what a disconnect means, whether the server is busy, and the answer body.

Start with the [example](../examples/serve-decision.coax). It runs the Coax side of one request without a socket,
on an owned toy model, so it needs no model weights:

```shell
coax/bin/coax run psionic/examples/serve-decision.coax
```

```jai
served: psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573
bytes: action decide, 2 questions, deadline at 30003 ms
second connection: HTTP/1.1 503 Service Unavailable
decision: answered
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 381
Connection: close

{"model":"psionic-toy","answers":{"hot":{"type":"noul","noul":0.437308645},"drink":{"type":"choice","choice":"soup","confidence":0.562514618,"probabilities":{"tea":0.437485382,"soup":0.562514618}}},"usage":null,"service":{"door":"psionic","version":"psionic-serve-v1","identity":"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573"},"latency_ms":0}
done: action answer, slot idle
```

<!-- source: psionic/examples/serve-decision.coax example -->
```jai
ask :: "POST /v1/systemone HTTP/1.1\r\nContent-Length: 193\r\n\r\n{\"model\":\"psionic-toy\",\"state\":\"The kettle has boiled.\",\"questions\":{\"hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"},\"drink\":{\"type\":\"choice\",\"criteria\":{\"tea\":null,\"soup\":\"a meal\"}}}}";

main :: () {
    toy:[1]SvToy;engine:=sv_test_toy(toy[..],400);
    print("served: %@sha256:%\n",engine.name,engine.digest);
    storage:[1]ExamplePools;pools:=example_pools(storage[..]);
    limits:=SvLimits.{read_ms=10000,decide_ms=30000};
    // The host accepted a connection at t=0 ms and has read every byte by t=3 ms.
    slot:=sv_accept(SvSlot.{},limits,0).slot;
    step:=sv_bytes(slot,limits,engine,pools,ask,3);
    print("bytes: action %, % questions, deadline at % ms\n",step.action,step.request.questions,step.slot.deadline);
    busy:=sv_accept(step.slot,limits,5);out:[4096]u8;
    n:=sv_refusal(out[..],busy.code);print("second connection: %\n",cast(string)out[0..<n][0..<32]);
    // The child process runs the decision and writes the response.
    prep:[1]PrScratch;work:=sv_scratch(prep[..],engine);answers:[16320]f64;
    code:=sv_answers(engine,step.request,pools,work,answers[..]);
    span:=SvSpan.{to=sv_refusal(out[..],code)};
    if code==SvCode.none {span=sv_render(engine,step.request,pools,answers[..],0,out[..]);}
    print("decision: %\n%\n",ifx code==SvCode.none then "answered" else sv_code_name(code),cast(string)out[span.from..<span.to]);
    // The parent hands the framed response back to the slot.
    frame:[4096]u8;head:=sv_frame_put(frame[..],code,span.to-span.from);
    for i:span.from..<span.to {frame[head+i-span.from]=out[i];}
    done:=sv_done(step.slot,step.slot.epoch,true,cast(string)frame[0..<head+span.to-span.from],40);
    print("done: action %, slot %\n",done.action,done.slot.phase);
}
```

The clock values are inputs. The example passes `0`, `3`, `5` and `40`
milliseconds; the real host passes its monotonic clock. The latency in the
body is `0` for the same reason.

## The toy model

The toy is teaching data, not a trained model. It has Elephant's layer
schedule, three linear layers and then one full-attention layer, at width 2
with a 64-dimensional rotary head, and the 268-entry toy vocabulary from
chapter 9. Its BF16 weights are small deterministic values written by
[a fixture script](../tests/serve_fixture.py). Its probabilities are near
uniform and mean nothing about kettles.

It still takes the real path. The host copies the fixture bytes into its own
buffers, and the chapter-14 session authenticates them against their pinned
SHA-256, inventories all 56 tensors and binds the layers. Requests are
rendered and tokenized as in chapter 9, and executed as in chapters 12 to 14.
Only the pinned slot IDs below the vocabulary can be read, so the toy answers
at most 26 options; Elephant answers up to 255.

## The request

A request is HTTP/1.1 with a `Content-Length`. The head may be at most 8 KiB
and the body at most 256 KiB, NIP-DEC's payload bound. `Transfer-Encoding` is
refused rather than decoded, and `Expect: 100-continue` is answered once
before the body arrives. Every connection carries one request and is then
closed.

The body is decoded into chapter 9's typed entries. Object and array entries
keep their original JSON bytes; strings are unescaped into a caller-owned
arena:

<!-- source: psionic/lib/serve-request.coax request -->
```jai
SvQuestion :: struct {id:string;kind:PtQuestionKind;instructions:PtEntry;first:int;count:int;}
SvPools :: struct {questions:[]SvQuestion;options:[]PtOption;raw:[]string;arena:[]u8;}
SvRequest :: struct {code:SvCode;model:string;state:PtEntry;questions:int;options:int;}
```

Decoding checks each NIP-DEC bound before a model is consulted: a model name
of 1 to 128 bytes, a string or object state of at most 128 KiB, 1 to 64
questions with IDs of 1 to 64 bytes, at most 64 KiB for one question, entries
nested at most 8 levels, 1 to 255 choice options with IDs of 1 to 128 bytes,
and 2 to 10 score levels. Fields a request carries beyond `model`, `state` and
`questions`, and question fields beyond `type`, `instructions` and `criteria`,
are read past and ignored. The legacy string form of a `noul` question's
criteria is accepted.

Some valid NIP-DEC requests are outside chapter 9's text profile: a state
nested deeper than 15 levels (the JSON reader's limit), a fractional number
inside an entry, or more options than the model has answer slots. Those get
`unsupported_input`, a code this server defines, so a caller can tell them
from a malformed request.

## Refusals

Every refusal is a JSON body, `{"error":{"code","message"}}`, with a fixed
message that never repeats request bytes:

<!-- source: psionic/lib/serve-request.coax codes -->
```jai
SvCode :: enum u8 {none;malformed;invalid_request;too_many_questions;too_many_options;not_admitted;not_found;method_not_allowed;limit_exceeded;unsupported_input;request_timeout;busy;timeout;internal;}
```

| Status | Code | When |
| --- | --- | --- |
| 400 | `malformed` | Not HTTP/1.1 with a declared length, a chunked body, or a body that is not JSON |
| 400 | `invalid_request` | The body is JSON but not a NIP-DEC decision request, or exceeds one of its bounds |
| 400 | `too_many_questions` | More than 64 questions |
| 400 | `too_many_options` | More than 255 choice options or 10 score levels |
| 403 | `not_admitted` | The `model` is not the served model's name or alias |
| 404 | `not_found` | Any route but `/v1/systemone` |
| 405 | `method_not_allowed` | A method other than `POST` (server-defined) |
| 408 | `request_timeout` | The request did not arrive within the read deadline (server-defined) |
| 413 | `limit_exceeded` | A head over 8 KiB, a body over 256 KiB, or a prompt beyond the token capacity |
| 422 | `unsupported_input` | Valid NIP-DEC outside this text profile (server-defined) |
| 500 | `internal` | The decision failed |
| 503 | `busy` | Another decision is running; `Retry-After: 1` |
| 524 | `timeout` | The decision did not finish before its deadline |

The statuses of NIP-DEC's codes are the table in its
["Refusals and HTTP status"](../../nips/openagents/NIP-DEC.md#refusals-and-http-status)
section. A code only this server defines keeps its own status, as NIP-DEC
allows for gateway codes.

## The served model

An engine is one authenticated chapter-14 session and what it answers as:

<!-- source: psionic/lib/serve-engine.coax engine -->
```jai
SvEngine :: struct {ok:bool;sealed:bool;session:PsSession;name:string;alias:string;digest:string;temperature:f32;capacity:int;options:int;}
SvWork :: struct {prep:PrWorkspace;tokens:[]int;state:[]f32;work:[]f32;prob:[]f64;}
```

Every answer names the served identity, `name@sha256:<digest>`, in
`service.identity`, and `response.model` is the name before `@`, the form
[NIP-PYLON](../../nips/openagents/NIP-PYLON.md#decision-services) uses. The
digest is the SHA-256 of the single checkpoint file that the session
authenticated before the server accepted any connection. It is the loaded
file's digest, not a digest computed over tensor payloads alone, so a file that
differs only in its header metadata would have a different identity.

[Chapter 19](ch019-keep-the-model-resident.md) now keeps one session resident
and admits reserved execution storage from a memory plan. Its session enters here:

<!-- source: psionic/lib/serve-engine.coax seam -->
```jai
sv_engine :: (session:PsSession,name:string,alias:string,digest:string,temperature:f32,capacity:int)->SvEngine #export_c {
    if session.status!=PsOpenStatus.ok || name.count<1 || !sv_hex64(digest) || !(temperature>0) {return SvEngine.{};}
    if capacity<1 || capacity>session.model.shape.state.capacity {return SvEngine.{};}
    return SvEngine.{ok=true,session=session,name=name,alias=alias,digest=digest,temperature=temperature,capacity=capacity,options=sv_max_options(session.model.vocab)};
}
```

The host allocates one fixed workspace at start from `sv_plan`,
for a request of the full capacity. The host checks once that every buffer it
allocated is disjoint from the others and from the session's borrowed
storage, and passes the result to `sv_seal`; an engine that is not sealed
answers nothing. Each answer is produced at temperature 1, with no
calibration, as in chapter 14.

The reference sequencing API runs each question through chapter 14's
preparation and answer. [Chapter 23](ch023-make-the-numbers-move.md) now uses
checked per-request prefix snapshots in the CPU host and removes redundant
CPU arenas on device hosts:

<!-- source: psionic/lib/serve-run.coax answers -->
```jai
sv_answers :: (engine:SvEngine,request:SvRequest,pools:SvPools,work:SvWork,p:[]f64)->SvCode #export_c {
    admitted:=sv_admit(engine,request,pools);if admitted!=SvCode.none {return admitted;}
    if !engine.sealed || p.count<SV_ANSWER_POOL {return SvCode.internal;}
    for i:0..<request.questions {
        q:=pools.questions[i];options:=pools.options[q.first..<q.first+q.count];
        question:=PtQuestion.{kind=q.kind,instructions=q.instructions};
        prepared:=ps_prepare(engine.session.table,request.state,question,options,work.tokens,work.prep,false);
        fits:=sv_fits(engine,prepared,work.work.count);if fits!=SvCode.none {return fits;}
        out:=p[i*SV_OPTIONS_MAX..<(i+1)*SV_OPTIONS_MAX];
        answer:=ps_answer(engine.session,prepared,work.tokens[0..<prepared.written],options,engine.temperature,work.state,out,work.prob,work.work,false);
        code:=sv_answer_code(answer.status);if code!=SvCode.none {return code;}
    }
    return SvCode.none;
}
```

The answer body follows NIP-DEC. A `choice` names a most probable option,
with ties broken by the lexicographically smallest option ID, and its
probability as `confidence`. A `score` reports the probability-weighted level,
the largest level probability as `confidence`, that level's index as
`selected` (the lowest index on a tie), and a `legend` that repeats each
requested level entry's original JSON. Probabilities are written with nine decimals.
`usage` remains `null`. [Chapter 22](ch022-run-it-live.md) adds separate
private operational usage records, without request or answer content.

## One decision at a time

The server holds one slot. A connection that arrives while the slot is in use
is refused at once with `busy`; nothing waits in a queue. The host reports
events and performs the action Coax returns:

<!-- source: psionic/lib/serve-engine.coax slot -->
```jai
SvPhase :: enum u8 {idle;reading;deciding;}
SvSlot :: struct {phase:SvPhase;epoch:u32;deadline:int;continued:bool;}
SvLimits :: struct {read_ms:int;decide_ms:int;}
SvAction :: enum u8 {none;read;send_continue;refuse;decide;answer;cancel_refuse;cancel_close;close;busy;}
SvStep :: struct {slot:SvSlot;action:SvAction;code:SvCode;request:SvRequest;from:int;to:int;}
```

Received bytes go through `sv_bytes`. It refuses a request that arrives after
the read deadline, waits for more bytes, decodes and admits the body, and only
then starts a decision with its own deadline and a new epoch:

<!-- source: psionic/lib/serve-engine.coax bytes -->
```jai
sv_bytes :: (slot:SvSlot,limits:SvLimits,engine:SvEngine,pools:SvPools,text:string,now:int)->SvStep #export_c {
    if slot.phase!=SvPhase.reading {return SvStep.{slot=slot};}
    if now>=slot.deadline {return sv_refused(slot,SvCode.request_timeout);}
    h:=sv_head(text);
    if h.status==SvHeadStatus.refused {return sv_refused(slot,h.code);}
    if h.status==SvHeadStatus.more {return SvStep.{slot=slot,action=SvAction.read};}
    if text.count<h.head+h.body {
        if h.expect && !slot.continued && text.count==h.head {s:=slot;s.continued=true;return SvStep.{slot=s,action=SvAction.send_continue};}
        return SvStep.{slot=slot,action=SvAction.read};
    }
    request:=sv_decode(text[h.head..<h.head+h.body],pools);
    code:=sv_admit(engine,request,pools);if code!=SvCode.none {return sv_refused(slot,code);}
    if slot.epoch==4294967295 {return sv_refused(slot,SvCode.internal);}
    next:=SvSlot.{phase=SvPhase.deciding,epoch=slot.epoch+1,deadline=now+limits.decide_ms};
    return SvStep.{slot=next,action=SvAction.decide,request=request};
}
```

A decision runs in a child process, because a CPU decision has no point at
which it could stop itself. The child inherits the authenticated session and
the decoded request, runs `sv_answers` and `sv_render`, writes a framed
response to a pipe and exits. The parent keeps watching the client and the
clock. When the deadline passes, `sv_tick` returns `cancel_refuse` and the
host kills the child and answers `524 timeout`. When the client closes its
connection, `sv_closed` returns `cancel_close` and the host kills the child
without writing. A child result that arrives for an older epoch, after the
deadline, or without a complete frame is never relayed as an answer, as in
the request machine of [Coax chapter 15](../../coax/book/ch15-asynchronous-work.md).

The server logs one line per request: the status, the code and the time it
took. It logs nothing from a request or an answer.

## Run the server

The server binds 127.0.0.1 only. Build and start it on the toy:

```shell
psionic/platform/serve.sh --toy --port 8790
```

```shell
curl -s http://127.0.0.1:8790/v1/systemone -d '{"model":"psionic-toy","state":"The kettle has boiled.","questions":{"hot":{"type":"noul","instructions":"Is the water hot?"}}}'
```

The answer has the same form as the example's, with a measured
`latency_ms`. The options are `--read-ms` (10,000 by default), the time a
request may take to arrive; `--deadline-ms` (30,000), the time a decision may
take; and `--capacity`, the most tokens one question may use (2,048 for the
toy, 512 for Elephant).

## Serve Elephant

With the pinned original files from chapter 14 in a directory, the same
server serves Elephant. It authenticates all 18.8 GB before it accepts a
connection, and one decision takes minutes on a CPU:

```shell
psionic/platform/serve.sh --elephant ~/models/elephant-1-b3f8a934 --capacity 128 --deadline-ms 7200000 --warmup-ms 7200000
```

We sent chapter 14's sample as a NIP-DEC body, recorded in
[elephant-request.json](../fixtures/models/serve/elephant-request.json):

```jai
{"model":"elephant-1","state":"Water is wet.","questions":{"wet":{"type":"choice","instructions":"Is the water wet?","criteria":{"yes":null,"no":null}}}}
```

The server authenticated the checkpoint in 84 seconds and answered with the
body below, recorded in
[elephant-response.http](../fixtures/models/serve/elephant-response.http):

```jai
{"model":"elephant-1","answers":{"wet":{"type":"choice","choice":"yes","confidence":0.757196285,"probabilities":{"yes":0.757196285,"no":0.242803715}}},"usage":null,"service":{"door":"psionic","version":"psionic-serve-v1","identity":"elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba"},"latency_ms":2816952}
```

The probabilities are chapter 14's to every printed digit: the HTTP path
rendered the same 86 tokens and ran the same calculation. The decision took
47 minutes on a 20-core Intel i7-14700K that other jobs were also using; the
reference CPU path uses one core. That figure is one observation, not a
latency claim.

## Serve Elephant on the GPU

With an NVIDIA device, the same server answers on chapter 44's device path:

```shell
psionic/platform/serve.sh --elephant-gpu ~/models/elephant-1-b3f8a934
```

Only the engine changes. Admission, the NIP-DEC bounds, the refusal codes
and the answer body are the ones above: `sv_stage` in
[serve-run.coax](../lib/serve-run.coax) prepares and fits each question
exactly as `sv_answers` does, the host runs the device plan, and `sv_read`
applies chapter 14's readout to the last token's logits. The plan and its
residency come from [plan.coax](../lib/device/plan.coax), opened once for the
largest admitted request (`--capacity`, 128 tokens by default and at most
512, `DP_MAX_TOKENS`). The C in
[elephant-gpu.c](../platform/elephant-gpu.c) loads modules, places buffers,
launches and waits, and nothing else.

A CUDA context does not survive `fork`, so device decisions run in the
server process, one at a time. Between layers the host asks whether the
deadline has passed or the client has gone (`dp_stop_point` marks the 33
places). A stopped decision returns before anything is rendered, and the
slot machine then refuses with `timeout` or closes, as it does when it kills
a CPU child.

On the RTX 4080 with the card otherwise idle, the server was ready 76 seconds
after it started: 0.3 seconds to compile the 19 generated kernels, 72.7
seconds to authenticate the checkpoint on the CPU, and 1.3 seconds to place
13.85 GiB on the device and pin 0.94 GiB of feed-forward matrices for
streaming. Twelve warm JSON requests of chapter 14's sample, each over its
own loopback connection and timed by `curl`, took 146.9 to 155.4 ms end to
end, with a median of 147.8 ms. Three protobuf round trips (chapter 21) took
146.6 to 149.2 ms. Every answer was:

```jai
{"model":"elephant-1","answers":{"wet":{"type":"choice","choice":"yes","confidence":0.757202773,"probabilities":{"yes":0.757202773,"no":0.242797227}}},"usage":null,"service":{"door":"psionic","version":"psionic-serve-v1","identity":"elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba"},"latency_ms":147}
```

That is p_yes 0.757202773 against chapter 14's 0.757196285, a difference of
6.5e-6, inside chapter 44's frozen 0.0005. A client that hung up after 50 or
80 ms was cancelled 3 to 4 ms later, and the next request answered the same.
With `--deadline-ms 60` both requests were refused 524 `timeout` at 64 ms.

A decision request that carries real context needs more than 128 tokens:
the Mac app's Lab (chapter 22's first caller) asks five questions about a
message, and each rendered question is 200 to 300 tokens. With
`--capacity 512` the larger workspace leaves less room for weights, so the
plan streams more feed-forward matrices: 13.29 GiB resident and 1.50 GiB
pinned for streaming. Chapter 14's sample still answers p_yes 0.757202773,
bit for bit the 128-token result, in 147 to 175 ms. The Lab's five questions
take about 1.8 seconds together. Prompts longer than 128 tokens use the same
kernels; they have not been compared against the CPU reference.

## What the checks establish

The Coax checks cover HTTP framing, every NIP-DEC bound at and past its limit,
the refusal table, admission, answer rendering, the toy session and the slot
machine, in debug and release; `coax mutate` leaves no survivors in
`serve-request.coax` and `serve-engine.coax`. A complete toy decision is too
large for the compile-time evaluator, so the loop in `serve-run.coax` is
checked natively. [The native checks](../tests/serve.c) start real servers
and send complete, partial, slow, abandoned, malformed and oversized requests
over loopback, in debug and release, plain and under ASan and UBSan. They
compare the served probabilities with direct calls to the chapter-14 C
adapters, check busy refusals, cancellation on disconnect and deadline
expiry, and check that the logs hold no request content:

```shell
psionic/tests/serve.sh
```

These checks cover one host. They do not cover a hostile network: there is no
TLS, the server answers one connection at a time, and a client that sends
slowly holds the slot until the read deadline. `Idempotency-Key` and
`X-Attempt` are not read, and nothing is metered. Running the server live is
[chapter 22](PLAN.md). See the
[verification record](../../docs/verification/psionic-ch16-2026-10-29.md) for
the measured results.

## Serving the original Mammoth checkpoint

The same server now accepts `--mammoth` and `--mammoth-gpu`. The launcher
selects CUDA for the latter and supplies the pinned Qwen tokenizer table:

```shell
psionic/platform/serve.sh --mammoth-gpu ~/models/mammoth-1-61cec44c \
  --capacity 512 --device-mib 90000
```

The directory contains the original configuration, tokenizer, readout,
index and both safetensors shards from chapter 15. Session admission still
authenticates every file and both shards. The model's served identity is
`mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019`:
the digest of its complete pinned inventory, which includes both shard
digests. It does not pass off the first shard's digest as the whole model.

[`serve-original.coax`](../lib/serve-original.coax) owns the model factory,
exact allocation counts and the frozen warmup rule as a small independent
authority root. The C host maps one or two files, allocates those counts,
places the same checked model on the device, and seals the spans. A checked
generated inventory header avoids adding another build-time generator;
[`mammoth-serving.test.ts`](../tests/mammoth-serving.test.ts) checks every
row against the original manifest and its frozen SHA-256.

Warmup uses chapter 15's original wet/no-wet decision. Its maximum absolute
probability error remains `0.0005`; normalization must be within `1e-8`.
The error predicate is checked exactly on either boundary as well as just
outside it. The new authority root kills all **38** of its mutants.

```shell
coax/bin/coax mutate psionic/lib/serve-original.coax --root-only
bun test psionic/tests/mammoth-serving.test.ts
```

For the authenticated public edge and its private native fleet services,
continue with [chapter 27](ch027-a-public-mesh.md).
