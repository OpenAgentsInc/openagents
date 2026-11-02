# 78. Device generation calls a tool

> **Status: partly implemented.** The tool-call contract of chapter 77 runs on
> chapter 66's resident CUDA engine with the original gpt-oss-20b on one RTX
> 4080 (profile gpt-oss-cuda-tools-v1), and every decision runs on the CPU
> without a GPU. The grammar mask is applied on the host to logits the
> device publishes, after one explicit, counted transfer per masked choice.
> A mask kernel on the device, the same path on chapter 72's Metal engine,
> and tool calls inside chapter 79's streamed server are not built.

[Previous: call a tool](ch077-call-a-tool.md) · [Contents](SUMMARY.md)

Chapter 77 ran a schema-constrained tool call on the CPU reference engine.
This chapter moves the same call onto the resident device engine and keeps
its guarantees there:

- The grammar mask is applied to the model's logits before any token is
  chosen.
- A call runs only if the host declared and granted that tool.
- A request can be cancelled at any point. A cancelled request never leaves
  half a tool effect, and the device is reset before the next request.
- There is no CPU fallback. If no device can serve, the request is refused
  with a typed reason.

The decisions live in [`device-tools.coax`](../lib/device-tools.coax), an
`#authority` file: it decides where a request may run, which tools may run
and when device state must be released. The harness that drives the original
model is [`gptoss_device_tools.c`](../tests/gptoss_device_tools.c), run by
[`check_device_tools.sh`](../tests/check_device_tools.sh).

## Where the mask runs

Chapter 66's engine chooses greedy tokens on the device: `gk_choose` reads
the published logits and writes the winning ID into a control word. A grammar
mask has to act before that choice, so a tool call cannot use the device's
own choice for its argument tokens. There are two honest options: build a
mask kernel, or copy the logits to the host and mask them there. This
chapter takes the second, and makes the copy explicit.

After each device step, the engine publishes the next-token logits in buffer
slot 18 as `vocab` f32 values. For gpt-oss-20b, `vocab` is 201,088, so one
transfer is

```text
201,088 logits x 4 bytes = 804,352 bytes per masked choice
```

The route admits this only on a device that is present and holds the
weights, and only if the per-choice transfer fits a fixed limit of 1,048,576
bytes. There is no third outcome: the function returns a device route or a
reason.

<!-- source: psionic/lib/device-tools.coax route -->
```jai
dx_route :: (d:DxDevice)->DxRoute #export_c {
    if !d.present {return DxRoute.{why=DxWhy.device};}
    if !d.resident {return DxRoute.{why=DxWhy.resident};}
    if d.vocab<1 || d.vocab>DX_VOCAB || d.limit<0 {return DxRoute.{why=DxWhy.shape};}
    bytes:=d.vocab*4;if bytes>d.limit {return DxRoute.{why=DxWhy.transfer};}
    return DxRoute.{transfer=bytes};
}
```

Only the choice runs on the host. Every model layer, the cache and the output
head still run on the device. The harness binary is built without any C copy
of the model's kernels (`gk_*`) and without the test-only CPU emulation
(`pg_open_cpu`); `check_device_tools.sh` checks the binary's symbols for
them. If a device step could not run, the request fails; nothing else could
compute it.

## Mask, then choose

`dx_choose` takes the downloaded logits and chapter 77's finite enum schema,
builds the mask with `gt_mask`, and only then calls chapter 53's
`gg_emit`. The two are one call, so a caller cannot sample without the mask.

<!-- source: psionic/lib/device-tools.coax choose -->
```jai
dx_choose :: (alternatives:[]string,prefix:string,pieces:[]string,logits:[]f32,mask:[]u8,stage:[]u8,options:GSampling,state:GGeneration,cancel:bool,probabilities:[]f64,work:GSampleWork)->GEmission #export_c {
    if logits.count!=pieces.count {return GEmission.{status=GStatus.capacity};}
    masked:=gt_mask(alternatives,prefix,pieces,mask,stage,false);if masked.status!=GStatus.ok {return GEmission.{status=masked.status};}
    return gg_emit(logits,mask[0..<logits.count],options,state,cancel,probabilities,work,false);
}
```

Trace it by hand on a small vocabulary. The schema admits one argument,
`{"a":2,"b":2}`. The pieces are `{"`, `a`, `":`, `2`, `,"`, `b`, `}` and
`evil`, all with logit 1 except `evil`, which has logit 50.

1. The argument so far is empty. Only `{"` is a prefix of the admitted
   object, so the mask allows one piece of 8. `evil` gets mass 0 although its
   logit is 49 higher than any other. Greedy picks `{"`.
2. The argument is `{"`. Only `a` continues it. Then `":`, `2`, `,"`, `b`,
   `":`, `2` and `}`, one allowed piece each time.
3. After nine choices the argument is `{"a":2,"b":2}`.

On the original vocabulary the same rule also lets the call token
(`<|call|>`, ID 200012) through only when the argument is complete, so the
model ends the handoff exactly there.

If no piece can continue the argument, the choice refuses with
`GStatus.token`. It never widens the grammar.

## Check the grant before anything runs

A complete call names its tool in the recipient field. `dx_grant` looks that
name up among the declared tools, refuses an undeclared name or an ungranted
tool, and then applies chapter 77's `gt_authorize` to the record: assistant
role, commentary channel, a call ending, and an argument body equal to one of
the declared values.

<!-- source: psionic/lib/device-tools.coax grant -->
```jai
dx_grant :: (tools:[]GTool,granted:[]bool,record:HmRecord,bytes:string)->DxGrant #export_c {
    if tools.count<1 || tools.count>DX_TOOLS_MAX || granted.count!=tools.count {return DxGrant.{why=DxWhy.shape};}
    if !gt_interval(record.recipient_begin,record.recipient_end,bytes.count) {return DxGrant.{why=DxWhy.call,status=GStatus.capacity};}
    name:=bytes[record.recipient_begin..<record.recipient_end];
    for i:0..<tools.count {if tools[i].name==name {
        if !granted[i] {return DxGrant.{why=DxWhy.ungranted,tool=i};}
        c:=gt_authorize(tools[i],record,bytes,true);if c.status!=GStatus.ok {return DxGrant.{why=DxWhy.call,status=c.status,tool=i};}
        return DxGrant.{tool=i,choice=c.choice};
    }}
    return DxGrant.{why=DxWhy.unknown};
}
```

## One request, one effect, and cancellation

A request moves through phases: `argument` while the masked argument is
generated, `called` once the call token arrives, `granted` after the grant,
`ran` after the tool runs, `answer` while the model writes its final answer,
and `done` at the end of the turn. `dx_next` is the only way to move between
them, and it answers three questions for the harness at every step: is this
event allowed now, does the tool run (`effect`), and must the device state
be released (`release`).

<!-- source: psionic/lib/device-tools.coax next -->
```jai
dx_next :: (phase:DxPhase,event:DxEvent)->DxStep #export_c {
    live:=phase!=DxPhase.idle && phase!=DxPhase.done && phase!=DxPhase.cancelled && phase!=DxPhase.refused;
    if event==DxEvent.cancel && live {return DxStep.{phase=DxPhase.cancelled,release=true};}
    if event==DxEvent.release && phase!=DxPhase.idle && !live {return DxStep.{phase=DxPhase.idle};}
    if phase==DxPhase.idle && event==DxEvent.begin {return DxStep.{phase=DxPhase.argument};}
    if (phase==DxPhase.argument || phase==DxPhase.answer) && event==DxEvent.token {return DxStep.{phase=phase};}
    if phase==DxPhase.argument && event==DxEvent.call {return DxStep.{phase=DxPhase.called};}
    if phase==DxPhase.called && event==DxEvent.grant {return DxStep.{phase=DxPhase.granted};}
    if phase==DxPhase.called && event==DxEvent.refuse {return DxStep.{phase=DxPhase.refused,release=true};}
    if phase==DxPhase.granted && event==DxEvent.run {return DxStep.{phase=DxPhase.ran,effect=true};}
    if phase==DxPhase.ran && event==DxEvent.resume {return DxStep.{phase=DxPhase.answer};}
    if phase==DxPhase.answer && event==DxEvent.turn {return DxStep.{phase=DxPhase.done,release=true};}
    return DxStep.{why=DxWhy.state,phase=phase};
}
```

Exactly one transition has `effect` set: `run` from `granted`. So the tool
runs at most once per request, and only after a grant. A cancel that arrives
before `ran` leaves no effect at all. A cancel that arrives after `ran`
discards the answer but not the tool's completed effect; the tool is never
interrupted halfway. Every ending phase (`done`, `cancelled`, `refused`) sets
`release`, and only `release` leads back to `idle`, so the harness must reset
the device's cache and control words (`pg_reset`) before the next request
can begin.

The checks compare `dx_next` with a hand-written table of all 90 (phase,
event) pairs, and require `effect` exactly at (`granted`, `run`) and
`release` exactly where the next phase is an ending.

## Audit the receipt

Each request keeps a receipt: device prefills, device decode steps, masked
choices and bytes transferred. A masked choice needs fresh logits from a
device prefill or step, and each needs exactly one transfer:

<!-- source: psionic/lib/device-tools.coax audit -->
```jai
dx_audit :: (r:DxReceipt,route:DxRoute)->DxWhy #export_c {
    if route.why!=DxWhy.none {return route.why;}
    if r.prefills<0 || r.steps<0 || r.masked<0 || r.transferred<0 {return DxWhy.shape;}
    if r.masked>r.prefills+r.steps {return DxWhy.state;}
    if r.transferred!=r.masked*route.transfer {return DxWhy.transfer;}
    return DxWhy.none;
}
```

For chapter 77's fixture, work out the receipt before running it. The call
prompt is 54 IDs, prefilled once on the device. The argument is 9 tokens
plus the call token: 10 masked choices, so 10 transfers, and 9 device steps
(the call token ends the handoff without a step). The answer prompt is 72
IDs, prefilled once with the device's own greedy choice. The answer is 5 IDs,
the first from that prefill and 4 from device steps, with no mask and no
transfer. That gives 2 prefills, 13 device steps, 10 masked choices, and
10 x 804,352 = 8,043,520 bytes transferred.

## Run the decisions without a GPU

[The example](../examples/device-tool-call.coax) runs every decision of this
chapter on the CPU: the route, the masked choices traced above, the grant,
the phases with a cancel at each live phase, and the audit of the receipt
just derived.

```shell
coax/bin/coax run psionic/examples/device-tool-call.coax
```

<!-- source: psionic/examples/device-tool-call.coax example -->
```jai
main :: () {
    route:=dx_route(DxDevice.{present=true,resident=true,vocab=201088,limit=dx_limit()});
    print("route: device, % logits bytes per masked choice; no CPU route\n",route.transfer);

    // The argument body, one masked choice at a time. "evil" has the highest logit.
    a:=[1]string.["{\"a\":2,\"b\":2}"];
    pieces:=[8]string.["{\"","a","\":","2",",\"","b","}","evil"];logits:=[8]f32.[1,1,1,1,1,1,1,50];
    mask:[8]u8;stage:[8]u8;p:[8]f64;s:[8]f64;o:[8]int;t:[8]int;w:=GSampleWork.{stage=s[..],order=o[..],temporary=t[..]};
    body:[32]u8;n:=0;g:=GGeneration.{remaining=16};
    while n<a[0].count {
        e:=dx_choose(a[..],cast(string)body[0..<n],pieces[..],logits[..],mask[..],stage[..],GSampling.{temperature=0},g,false,p[..],w);
        #assert e.status==GStatus.ok;g=e.state;allowed:=0;for m:mask {allowed+=cast(int)m;}
        for c:pieces[e.token] {body[n]=c;n+=1;}
        print("choice %: % allowed of 8, chose % (evil: mass %)\n",g.random.counter,allowed,pieces[e.token],p[7]);
    }
    print("argument body: %\n",cast(string)body[0..<n]);

    // The grant, checked on the parsed call before anything runs.
    tools:=[1]GTool.[GTool.{name="functions.add",arguments=a[..]}];granted:=[1]bool.[true];
    text:="functions.add{\"a\":2,\"b\":2}";
    r:=HmRecord.{role=HmRole.assistant,channel=HmChannel.commentary,ending=HmEnd.call,recipient_begin=0,recipient_end=13,body_begin=13,body_end=text.count};
    grant:=dx_grant(tools[..],granted[..],r,text);print("grant: % (tool %, choice %)\n",dx_why_name(grant.why),grant.tool,grant.choice);

    // The whole request, then a cancellation arriving at each phase.
    script:=[9]DxEvent.[DxEvent.begin,DxEvent.token,DxEvent.call,DxEvent.grant,DxEvent.run,DxEvent.resume,DxEvent.token,DxEvent.turn,DxEvent.release];
    phase:=DxPhase.idle;effects:=0;
    for e:script {step:=dx_next(phase,e);#assert step.why==DxWhy.none;phase=step.phase;if step.effect {effects+=1;}if step.release {print("% -> release device state\n",PHASES[cast(int)phase]);}}
    print("round trip: tool effects %, back to %\n",effects,PHASES[cast(int)phase]);
    points:=[5]int.[1,3,4,5,6];
    for at:points {phase=DxPhase.idle;effects=0;
        for i:0..<at {step:=dx_next(phase,script[i]);phase=step.phase;if step.effect {effects+=1;}}
        cancelled:=dx_next(phase,DxEvent.cancel);
        print("cancel in %: tool effects %, release %\n",PHASES[cast(int)phase],effects,cancelled.release);
    }

    // The recorded original run: 2 prefills, 13 device steps, 10 masked choices.
    receipt:=DxReceipt.{prefills=2,steps=13,masked=10,transferred=10*route.transfer};
    print("receipt: % bytes transferred, audit %\n",receipt.transferred,dx_why_name(dx_audit(receipt,route)));
}
```

Expected output:

```text
route: device, 804352 logits bytes per masked choice; no CPU route
choice 1: 1 allowed of 8, chose {" (evil: mass 0.0)
choice 2: 1 allowed of 8, chose a (evil: mass 0.0)
choice 3: 1 allowed of 8, chose ": (evil: mass 0.0)
choice 4: 1 allowed of 8, chose 2 (evil: mass 0.0)
choice 5: 1 allowed of 8, chose ," (evil: mass 0.0)
choice 6: 1 allowed of 8, chose b (evil: mass 0.0)
choice 7: 1 allowed of 8, chose ": (evil: mass 0.0)
choice 8: 1 allowed of 8, chose 2 (evil: mass 0.0)
choice 9: 1 allowed of 8, chose } (evil: mass 0.0)
argument body: {"a":2,"b":2}
grant: none (tool 0, choice 0)
done -> release device state
round trip: tool effects 1, back to idle
cancel in argument: tool effects 0, release true
cancel in called: tool effects 0, release true
cancel in granted: tool effects 0, release true
cancel in ran: tool effects 1, release true
cancel in answer: tool effects 1, release true
receipt: 8043520 bytes transferred, audit none
```

[The refusal example](../examples/device-tool-call-refuse.coax) shows each
typed refusal:

<!-- source: psionic/examples/device-tool-call-refuse.coax refuse -->
```jai
main :: () {
    d:=DxDevice.{present=true,resident=true,vocab=201088,limit=dx_limit()};
    x:=d;x.present=false;say("no CUDA device",dx_route(x).why);
    x=d;x.resident=false;say("weights not resident",dx_route(x).why);
    x=d;x.vocab=201089;say("a vocabulary of 201,089",dx_route(x).why);
    x=d;x.limit=804351;say("a 804,351-byte transfer limit",dx_route(x).why);

    a:=[1]string.["{\"a\":2,\"b\":2}"];m:=[1]string.["{\"a\":3}"];
    tools:=[2]GTool.[GTool.{name="functions.add",arguments=a[..]},GTool.{name="functions.mul",arguments=m[..]}];
    text:="functions.add{\"a\":2,\"b\":2}";
    r:=HmRecord.{role=HmRole.assistant,channel=HmChannel.commentary,ending=HmEnd.call,recipient_begin=0,recipient_end=13,body_begin=13,body_end=text.count};
    no:=[2]bool.[false,true];say("functions.add not granted",dx_grant(tools[..],no[..],r,text).why);
    yes:=[1]bool.[true];say("only functions.mul declared",dx_grant(tools[1..<2],yes[..],r,text).why);
    r.channel=HmChannel.analysis;both:=[2]bool.[true,true];say("the call on the analysis channel",dx_grant(tools[..],both[..],r,text).why);

    say("run before the grant",dx_next(DxPhase.called,DxEvent.run).why);
    say("a new request before release",dx_next(DxPhase.cancelled,DxEvent.begin).why);
    say("cancel with nothing running",dx_next(DxPhase.idle,DxEvent.cancel).why);

    route:=dx_route(d);
    say("one extra transferred byte",dx_audit(DxReceipt.{prefills=2,steps=13,masked=10,transferred=10*route.transfer+1},route));
    say("more masked choices than device logits",dx_audit(DxReceipt.{prefills=1,steps=0,masked=2,transferred=2*route.transfer},route));
}
```

```text
no CUDA device: device: no CUDA device; no CPU fallback
weights not resident: resident: the weights are not resident on the device
a vocabulary of 201,089: shape: a count or size outside its bounds
a 804,351-byte transfer limit: transfer: logits bytes over the limit, or not one download per masked choice
functions.add not granted: ungranted: the host did not grant this tool
only functions.mul declared: unknown: the recipient is not a declared tool
the call on the analysis channel: call: the handoff is not a well-formed call
run before the grant: state: out of order for this phase or these device steps
a new request before release: state: out of order for this phase or these device steps
cancel with nothing running: state: out of order for this phase or these device steps
one extra transferred byte: transfer: logits bytes over the limit, or not one download per masked choice
more masked choices than device logits: state: out of order for this phase or these device steps
```

The same refusal comes from the harness itself when no device is visible. It
exits with status 5 and runs nothing:

```shell
CUDA_VISIBLE_DEVICES= gptoss_device_tools device-kernels.cu ORIGINAL_DIR psionic/fixtures/gptoss/qualification/tools
```

```text
refused: device: no CUDA device; no CPU fallback
```

`check_device_tools.sh` builds the harness, runs it this way, and checks the
output and the exit status.

## The original model on the device

With the original weights and a CUDA device, the harness runs chapter 77's
forced `functions.add` request on the resident engine and compares the
complete ID sequences, prompt and generated, with chapter 77's recorded CPU
reference ([`step0.call.i64`](../fixtures/gptoss/qualification/tools/step0.call.i64)
and [`step0.resumed.i64`](../fixtures/gptoss/qualification/tools/step0.resumed.i64)).
The reference is not recomputed: the CPU run of the original model was
recorded once in chapter 77 and is compared, not repeated.

```shell
COAX_CC=gcc psionic/tests/check_device_tools.sh ORIGINAL_DIR 8
```

The recorded run ([`rtx4080-original.log`](../fixtures/device-tools/rtx4080-original.log)),
on one RTX 4080 at commit `121b2f1149` plus this chapter's files, built with
the cached `coax0 0.46.0` compiler:

```text
load average before: 22.50 27.04 27.75
device: NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
original 20b authenticated and resident in 41.762 s
route: device; logits transfer 804352 bytes per masked choice; no CPU route exists
call: functions.add arguments={"a":2,"b":2} granted=yes result=4 effects=1
call: 54 prompt + 10 generated IDs equal the CPU reference
answer: 72 prompt + 5 generated IDs equal the CPU reference
final: 4
receipt: prefills=2 device_steps=13 masked=10 transferred=8043520 bytes transfer_ns=930054 round_trip_ns=636350980 audit=pass
ungranted: refused (ungranted: the host did not grant this tool); effects=1
undeclared: refused (unknown: the recipient is not a declared tool); effects=1
seed=1 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=2 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=3 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=4 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=5 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=6 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=7 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
seed=8 temperature=1 argument_ids=10 arguments={"a":2,"b":2} granted=yes final=4
cancel at prefill: cancelled in phase argument, tool effects 0; device state released; next request identical
cancel at argument token 3: cancelled in phase argument, tool effects 0; device state released; next request identical
cancel at call parsed, before the grant: cancelled in phase called, tool effects 0; device state released; next request identical
cancel at answer token 2: cancelled in phase answer, tool effects 1; device state released; next request identical
device bytes in use=14783086592 of 16711352320
device tool round trips pass
load average after: 13.61 23.96 26.66
```

The receipt matches the hand-derived one: 2 prefills, 13 device steps, 10
masked choices and 8,043,520 bytes. The ten transfers took 0.93 ms in total,
against 636 ms for the whole round trip.

The independent checks in that run are:

- **Device against the CPU reference.** The call (prompt, 9 argument
  tokens, call token) and the resumed answer equal the recorded CPU IDs
  token for token, under greedy choice. The tool result is 4 and the
  displayed final answer is `4`, through chapter 77's `gt_display`.
- **Every sampled call parses and is granted.** Eight further requests sample
  the argument at temperature 1 with seeds 1 to 8. Each call parses as
  Harmony, passes `dx_grant`, and its arguments are the one admitted value.
- **Refusals before any effect.** The same call with the grant withdrawn is
  refused as `ungranted`, and with only `functions.mul` declared as
  `unknown`. The tool effect count does not change.
- **Cancellation leaves the session reusable.** A cancel arrives during the
  prompt prefill, after the third argument token, after the call parses but
  before the grant, and after the second answer token. Each request ends in
  `cancelled`, the device is reset, and the next greedy request repeats the
  reference IDs exactly. Only the last cancel, which came after the tool ran,
  counts one tool effect.

## What is not built

- **A mask kernel on the device.** The mask runs on the host after a counted
  transfer of 804,352 bytes per argument token. A device kernel would avoid
  the transfer; it is not written.
- **Metal.** Chapter 72's engine publishes the same logits slot, but this
  harness runs only on CUDA.
- **Streaming and serving.** Chapter 79's Responses server handles external
  function calls, but not this masked, granted in-engine path, and the
  throughput gate of chapter 71 does not measure requests with tool calls.
- **General schemas.** The grammar is still chapter 77's bounded
  `gpt-oss-json-enum-v1` profile, and the tool choice is forced by the host.
