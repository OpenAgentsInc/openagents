# 77. Call a tool

[Previous: speculative decoding](ch075-let-the-small-model-guess.md) ·
[Contents](SUMMARY.md)

A Harmony handoff asks the host to do something. It grants no authority on its
own. This chapter constrains the argument body before sampling, checks the
recipient/channel/ending against a granted tool definition, returns a typed
tool message and resumes the assistant. A separate Coax display gate keeps
analysis and tool traffic out of end-user output.

The implemented schema profile is deliberately bounded: `gpt-oss-json-enum-v1`,
a root object whose allowed values form a finite JSON Schema `enum`. Each
value has one explicitly supplied canonical JSON serialization. The profile
admits 1–16 flat objects, each 2–512 bytes with at most 16 unique ASCII keys;
values may be JSON scalars. Nested containers and other schema forms refuse.
Equivalent whitespace or key-order serializations need not be offered by the
grammar. This is a sound subset of the declared enum schema, not a general
JSON-schema compiler. General properties, ranges, recursion and arbitrary
strings require another constrained-decoding profile.

For our owned calculator fixture, the schema is an object enum containing
`{"a":2,"b":2}`. The host explicitly selects `functions.add`. A forced named
tool choice tests the argument and handoff path; it does not establish that the
model autonomously selects the right tool.

## Offer only tokens that can finish an admitted argument

Parse every supplied enum object completely before using it. Reject malformed
JSON, duplicate keys, extra trailing values and unsupported containers. The
canonical object bytes then define a finite language.

For each ordinary vocabulary token, append its byte fragment to the current
argument prefix only if the concatenation is a prefix of an admitted object.
Fragments can end inside UTF-8 or a JSON escape: matching valid complete object
bytes preserves the unfinished sequence until later tokens complete it. Empty
fragments are excluded. All formatting IDs are masked except call, and call
is offered only once the whole canonical object matches. The mask is applied
to logits before `gs_distribution` chooses anything.

<!-- source: psionic/lib/gptoss/tool-schema.coax allowed -->
```jai
gt_allowed :: (prefix:string,fragment:string,ending:bool,alternatives:[]string)->bool #export_c {
    if ending && fragment.count!=0 {return false;}
    if !ending && fragment.count==0 {return false;}
    for text:alternatives {
        n:=prefix.count+fragment.count;if n>text.count || (ending && n!=text.count) {continue;}
        same:=true;for i:0..<prefix.count {if prefix[i]!=text[i] {same=false;break;}}
        if same {for i:0..<fragment.count {if fragment[i]!=text[prefix.count+i] {same=false;break;}}}if same {return true;}
    }return false;
}
```

`gt_mask` stages its byte output. It bounds the vocabulary to 201,088, token
pieces to 1,024 bytes and logical comparison work to 16,777,216 divided by the
schema's total byte length. An over-budget schema/vocabulary pair refuses;
it does not silently drop alternatives. The original token table is separately
authenticated, immutable and shared by both models.

A validator applied after generation cannot provide this contract: an invalid
token would already have been offered. Checks give the invalid token a much
larger logit than every legal token and verify it still receives zero mass.
Native checks exercise the original formatting-ID boundaries and nested
address aliases.

## A valid call still needs the host's grant

Only an assistant message on `commentary`, ending in call, addressed to the
exact allowed tool name can pass `gt_authorize`. Its complete argument body
must match one declared enum value. A valid message with no host grant refuses.
The gate returns the selected alternative; it does not execute arbitrary code,
open a socket, invoke a shell or infer permission from model text.

<!-- source: psionic/lib/gptoss/tools.coax authorize -->
```jai
gt_authorize :: (tool:GTool,record:HmRecord,bytes:string,granted:bool)->GCall #export_c {
    if !granted || record.role!=HmRole.assistant || record.channel!=HmChannel.commentary || record.ending!=HmEnd.call {return GCall.{status=GStatus.state};}
    if !gt_interval(record.recipient_begin,record.recipient_end,bytes.count) || !gt_interval(record.body_begin,record.body_end,bytes.count) {return GCall.{status=GStatus.capacity};}
    if !hm_ident(tool.name) || tool.name=="all" || bytes[record.recipient_begin..<record.recipient_end]!=tool.name {return GCall.{status=GStatus.token};}
    schema:=gt_schema(tool.arguments);if schema.status!=GStatus.ok {return GCall.{status=schema.status};}body:=bytes[record.body_begin..<record.body_end];
    for i:0..<tool.arguments.count {if body==tool.arguments[i] {return GCall.{choice=i};}}return GCall.{status=GStatus.token};
}
```

The example's calculator executes local checked Coax addition, bounded to
addends ±1,000,000. An actual external host tool needs its own grant and error,
resource and cancellation policy. Its result becomes a tool-role message
named `functions.add`, rendered by chapter 63 as ordinary body text. Formatting
markers in that result remain ordinary bytes. Re-render the supplied history
and resume with a fresh assistant completion prefix.

## Display is a separate decision

`gt_display` admits only an assistant final message ending the turn, with no
recipient and valid byte intervals. It copies only that body and preserves
unused capacity. Analysis, commentary, tool results and handoffs refuse, even
if they parse as valid Harmony. No caller should forward the entire decoded
completion buffer directly to a user.

<!-- source: psionic/lib/gptoss/tools.coax display -->
```jai
gt_display :: (record:HmRecord,bytes:string,out:[]u8,overlap:bool)->GResult #export_c {
    if record.role!=HmRole.assistant || record.channel!=HmChannel.final || record.ending!=HmEnd.turn || record.recipient_begin!=record.recipient_end {return GResult.{status=GStatus.state};}
    if !gt_interval(record.body_begin,record.body_end,bytes.count) {return GResult.{status=GStatus.capacity};}n:=record.body_end-record.body_begin;
    if out.count<n {return GResult.{status=GStatus.capacity};}if overlap {return GResult.{status=GStatus.alias};}
    for i:0..<n {out[i]=bytes[record.body_begin+i];}return GResult.{written=n};
}
```

[Run the owned example](../examples/call-a-tool.coax):

```shell
coax/bin/coax run psionic/examples/call-a-tool.coax
```

<!-- source: psionic/examples/call-a-tool.coax example -->
```jai
main :: () {
    alternatives:=[1]string.["{\"a\":2,\"b\":2}"];pieces:=[3]string.["{\"a\":","evil","{"];mask:[3]u8;mask_stage:[3]u8;
    masked:=gt_mask(alternatives[..],"",pieces[..],mask[..],mask_stage[..],false);#assert masked.status==GStatus.ok;
    logits:=[3]f32.[2,100,1];probabilities:[3]f64;stage:[3]f64;order:[3]int;temporary:[3]int;
    sampled:=gs_distribution(logits[..],mask[..],0,0,1,probabilities[..],stage[..],order[..],temporary[..],false);#assert sampled.status==GStatus.ok && probabilities[1]==0;
    print("invalid high-logit token: masked before sampling\n");
    bytes:="functions.add{\"a\":2,\"b\":2}";record:=HmRecord.{role=HmRole.assistant,channel=HmChannel.commentary,ending=HmEnd.call,recipient_begin=0,recipient_end=13,body_begin=13,body_end=bytes.count};
    tool:=GTool.{name="functions.add",arguments=alternatives[..]};call:=gt_authorize(tool,record,bytes,true);#assert call.status==GStatus.ok;
    sum:=gt_add(2,2);response:=gt_response(tool.name,"4");#assert sum.status==GStatus.ok && response.status==GStatus.ok;
    print("granted commentary call: %; result: %\n",tool.name,sum.value);
    out:[32]u8;record.channel=HmChannel.analysis;#assert gt_display(record,bytes,out[..],false).status==GStatus.state;
    print("analysis: excluded by the display gate\n");
    print("resume with tool role: %\n",response.message.name);
}
```

Expected output:

```jai
invalid high-logit token: masked before sampling
granted commentary call: functions.add; result: 4
analysis: excluded by the display gate
resume with tool role: functions.add
```

The opt-in original 20b round trip opens a forced commentary argument body,
masks the actual full vocabulary before every choice, samples through call,
checks the host grant, computes 4 in Coax, returns it as a tool message and
resumes to a complete final answer. The native harness displays final bodies
through `gt_display`; full token streams remain qualification artifacts.

```shell
psionic/tests/check_gptoss_original.sh tools
```

The [verification record](../../docs/verification/psionic-ch48-2026-10-29.md)
records the result and exact supported scope. Broad schema compilation,
autonomous tool selection, external tool effects and production serving are
separate gates. We now have the original two model engines, structured text,
seeded generation, a verified speculative core and a constrained tool round
trip on which those later applications can build.
