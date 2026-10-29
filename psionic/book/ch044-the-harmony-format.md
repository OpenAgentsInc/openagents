# 44. The harmony format

An attention kernel takes vectors. A chat model takes a conversation, including
who spoke, whether an assistant message is an answer or a tool handoff, and
where a turn ends. This chapter builds that boundary in Coax for gpt-oss.

We will tokenize with the original byte ranks, render typed messages and parse
complete model replies. We can check all of it without loading model weights.
Chapters [5](ch005-text-becomes-a-model-request.md) and
[43](ch043-attention-with-sinks-and-a-window.md) provide the earlier tokenizer
and attention context; the [plan](PLAN.md) puts complete gpt-oss generation next.

```shell
coax/build.sh
coax/bin/coax run psionic/examples/harmony.coax
psionic/tests/check_harmony.sh
```

```jai
ordinary markers: preserved as body text
completion: 2 messages; analysis then final; stop=return
truncated turn: refused without output
handoff: functions.weather; stop=call
reasoning: medium
```

The [example](../examples/harmony.coax) uses a small invented ordinary
vocabulary, with the real control IDs. The native gate uses the full original
ranks and requires exact token/byte matches. Both call the same
[Coax library](../lib/harmony.coax). This is the bounded
`gpt-oss-harmony-text-v1` profile, with Unicode 17 properties; it does not run a
model or execute tools.

## Message boundaries are tokens

A simple exchange looks like this when control IDs are displayed as text:

```jai
<|start|>user<|message|>Hello.<|end|>
<|start|>assistant<|channel|>final<|message|>Hi.<|return|>
```

There are no implicit newlines between messages; the lines above separate them
for reading. Role names and channel names are ordinary text. The boundary
markers each occupy one dedicated token ID.

| Token | ID | Meaning in this profile |
| --- | --- | --- |
| `<\|start\|>` | 200006 | Begin a message header. |
| `<\|channel\|>` | 200005 | The following ordinary header text names a channel. |
| `<\|message\|>` | 200008 | End the header and begin its body. |
| `<\|end\|>` | 200007 | End a message; the assistant turn may continue. |
| `<\|return\|>` | 200002 | Finish an assistant turn. |
| `<\|call\|>` | 200012 | Yield an assistant message to its named recipient. |

The pinned [Harmony renderer](https://github.com/openai/harmony/blob/abd677f7ac962629c808197caa1feb9e3e95d2b0/src/encoding.rs)
encodes text fields with `encode_ordinary`, inserting formatting IDs separately.
Our typed renderer follows that division. If the user writes the literal
characters `<|start|>` inside a body, those characters become ordinary tokens.
They cannot manufacture a new message boundary. Parsing therefore branches on
control **IDs**, never a search for marker spellings in decoded text.

## Preserve bytes, then apply the model's split pattern

Chapter 5's Qwen tokenizer normalizes to NFC. The gpt-oss tokenizer does not:
`é` and `e` followed by a combining acute accent retain their different bytes.
Its split expression also distinguishes uppercase and lowercase word runs,
attaches contractions, groups one through three numeric scalars, and permits
slashes after punctuation/newline runs. Reusing Qwen's split expression would
change token IDs even with the correct vocabulary.

The [gpt-oss tokenizer source](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/tokenizer.py)
uses o200k_base's pattern and ranks. The [pinned Harmony encoding source](https://github.com/openai/harmony/blob/abd677f7ac962629c808197caa1feb9e3e95d2b0/src/tiktoken_ext/public_encodings.rs)
provides the pattern and expected SHA-256 of the original rank data. We retain
those original bytes, then deterministically index them for Coax. The
[fixture provenance](../fixtures/harmony/README.md) and
[manifest](../fixtures/harmony/manifest.json) record the exact identities.

The original ranks contain 199,998 ordinary byte strings. The gpt-oss special
range is 199,998–201,087, giving 201,088 total entries. Generic Harmony at the
pinned revision also registers one more reserved ID, 201,088. That extra ID is
outside this model profile and refuses when encountered before a terminal
stop. Reserved IDs inside the model range can be displayed by the raw decoder;
the structured text parser refuses unsupported controls and extensions.

Our `HST7` table has an independent identity from chapter 5's `PST5` table. Its
Unicode-17 properties include the letter categories needed by the new pattern.
A byte-to-ID index initializes symbols, a sorted byte-string index supplies the
whole-piece fast path, and 446,189 pair records represent every concatenation
that appears in the original ranks. We include pairs whose component IDs are
newer than the combined ID; a guessed historical training merge list would
lose valid rank lookups.

When a whole piece is already a token, the tokenizer returns its ID directly.
Otherwise it calls the shared `pt_bpe` loop from chapter 5: select the smallest
available rank, merge the leftmost tied pair, and repeat. The independent oracle
uses a heap over actual byte-string concatenations rather than the packed pair
index.

<!-- source: psionic/lib/harmony/tokenizer.coax encode -->
```jai
hs_encode_checked :: (table:[]u8,input:string,out:[]int,stage:[]int,symbols:[]int,overlap:bool)->PtResult #export_c {
    if overlap {return PtResult.{status=PtStatus.alias};}t:=hs_layout(table);if t.status!=PtStatus.ok {return PtResult.{status=t.status};}
    r:=hs_append(table,t,input,stage,symbols,0);if r.status!=PtStatus.ok {return r;}
    if out.count<r.written {return PtResult.{status=PtStatus.capacity};}
    for i:0..<r.written {out[i]=stage[i];}return r;
}
```

`hs_append` validates UTF-8, finds pieces without normalization, applies BPE and
stages all IDs. Only a successful `hs_encode_checked` publishes output. The raw
`hs_decode_checked` returns concatenated **bytes**, since a single byte token
need not be valid UTF-8 by itself. The structured parser validates the complete
body after joining its token bytes; this handles multibyte characters split
across token boundaries.

Before using the original table in a model session, admit its complete hash
once and keep the bytes immutable and alive:

<!-- source: psionic/lib/harmony/admission.coax admit -->
```jai
hs_admit :: (table:[]u8)->bool #export_c {return pw_digest(table,HS_TABLE_SHA);}
```

Layout parsing on later calls is not a new identity admission. The toy table
used by compiler checks and the example has different ordinary IDs and must
not be supplied to a real gpt-oss model. The native gate authenticates
`psionic/fixtures/harmony/gptoss.hst` and checks changed and truncated bytes
against the runtime admission function.

## Render a typed message

`HmMessage` separates role, channel, ending, tool name, recipient and body.
System, developer, user and assistant roles render their ordinary role names.
A tool response renders its tool name as the role, such as `functions.weather`.
A recipient renders as ordinary ` to=functions.weather` before the channel.

For example:

```jai
<|start|>assistant to=functions.weather<|channel|>commentary<|message|>{"city":"Tokyo"}<|call|>
<|start|>functions.weather to=assistant<|channel|>commentary<|message|>{"temperature":20}<|end|>
```

A call requests a handoff; parsing it grants no authority to run that tool.
The host decides whether and how to execute a request. Tool schemas and
constrained-format metadata remain later work in chapter 48.

The three supported channel names are `analysis`, `commentary` and `final`.
An absent channel remains absent. The parser exposes the channel explicitly;
applications can route final text to the user while keeping analysis separate.
This library preserves the supplied message history. It does not apply the
upstream conversation renderer's optional analysis-history filtering.

An assistant can finish a normal message, end its turn, or yield to a named
recipient. Other roles finish messages. Empty recipient means broadcast;
explicit `all`, malformed identifiers and contradictory endings refuse.
Identifiers use ASCII letters or underscore first, then letters, digits,
underscore, dot or dash, through 64 bytes. The text-only profile does not admit
user nicknames, arbitrary channels, content-type extensions or multimodal data.

<!-- source: psionic/lib/harmony/messages.coax metadata -->
```jai
hm_metadata :: (role:u8,channel:u8,ending:u8,name:string,recipient:string)->bool {
    if role>4 || channel>3 || ending>2 {return false;}
    if role==4 {if !hm_ident(name) || hm_role(name)!=4 {return false;}}else if name.count!=0 {return false;}
    if recipient.count!=0 {if !hm_ident(recipient) || recipient=="all" {return false;}}
    if ending==2 {return role==3 && recipient.count>0;}
    if ending==1 {return role==3 && recipient.count==0;}
    return role!=3 || recipient.count==0;
}
```

`hm_preflight` checks all metadata and counts logical decoded text bytes before
encoding. `hm_render_checked` then stages the complete token stream, including
all structural IDs, and copies it to public output on success. Its `written`
field counts tokens, `bytes` counts decoded text/header bytes excluding the
structural marker spellings, and `work` reports the accumulated piece budget.
Underlying tokenizer failures use `status=tokenizer` and retain the precise
`PtStatus` in `cause`.

Passing `completion=true` appends exactly `<|start|>assistant`, leaving the
header open for generation. It appends neither a channel nor `<|message|>`.
The model can continue with a channel and recipient before beginning its body.

## Reasoning effort is system content

Low, medium and high effort are ordinary text inside the system body. They are
not new special tokens and do not alter sampling parameters by themselves.
`hm_system_checked` reproduces the pinned default system content with the
chosen effort, including its model identity, training cutoff text and valid
channel declaration. The cutoff is part of this frozen source template.

<!-- source: psionic/lib/harmony/messages.coax system -->
```jai
hm_system_checked :: (effort:int,out:[]u8,work:[]u8,overlap:bool)->PtResult #export_c {
    if effort<0 || effort>2 {return PtResult.{status=PtStatus.range};}if overlap {return PtResult.{status=PtStatus.alias};}
    efforts:=[3]string.["low","medium","high"];parts:=[3]string.[HM_SYSTEM_HEAD,efforts[effort],HM_SYSTEM_TAIL];n:=HM_SYSTEM_HEAD.count+efforts[effort].count+HM_SYSTEM_TAIL.count;
    if out.count<n || work.count<n {return PtResult.{status=PtStatus.capacity};}p:=0;
    for part:parts {for c:part {work[p]=c;p+=1;}}
    for i:0..<n {out[i]=work[i];}return PtResult.{written=n};
}
```

Callers can supply their own system body through `HmMessage`. The provided
helper covers the pinned default template with no tools or current-date field.
The oracle independently builds the low-effort conversation and compares its
text with the unchanged upstream fixture, then freezes token IDs for all three
effort settings. No upstream renderer is executed.

## Parse a complete reply, including its stop

The parser accepts full transcripts with `completion=false`. With
`completion=true`, its first header may continue the open assistant prefix,
without repeating `<|start|>assistant`. Later messages require an explicit
start and role. It accepts recipients before or after the channel, since a
generated header can choose either order, and refuses duplicate recipients or
channel markers.

An analysis message can end with `<|end|>`, followed by another assistant
message in the final channel. Stopping on that first end marker would lose the
answer. In completion mode the parser stops only when the assistant ends its
turn or yields:

<!-- source: psionic/lib/harmony/messages.coax stop -->
```jai
        if completion && record.ending!=HmEnd.message {stop=record.ending;break;}
```

The stop token itself is consumed. `consumed` counts the used input tokens;
trailing tokens remain for the caller and are not inspected after that stop.
A completion ending only in `<|end|>` is incomplete. A missing header separator,
body ending or terminal stop returns `incomplete` when more tokens could finish
it. Unsupported controls, bad IDs, malformed headers, invalid UTF-8 and illegal
role/ending combinations refuse with their corresponding status.

This is an atomic parser for complete bounded inputs. A streaming host can
retain a token buffer and retry it; partial parses publish no records or bytes.
It does not implement an incremental stream decoder or repair malformed output.

`HmRecord` stores role/channel/ending and byte intervals for tool name,
recipient and body. Those intervals refer to the caller's decoded-byte output,
so publication does not leave pointers into temporary scratch. Whole records
and all decoded bytes publish together:

<!-- source: psionic/lib/harmony/messages.coax publish -->
```jai
    if out.count<count || bytes.count<n {return HmResult.{status=HmStatus.capacity};}
    for i:0..<n {bytes[i]=work[i];}for i:0..<count {out[i]=draft[i];}
    return HmResult.{written=count,bytes=n,consumed=at,stop=stop};
```

The parse result's `written` counts records, `bytes` counts decoded bytes, and
`stop` describes a completion's terminal ending. Failed results have zero
counts; scratch and draft records may change. Spare public capacity is preserved.
All native wrappers observe writable/read-only and writable/writable overlaps;
C performs only address observation and file I/O in the test harness. Grammar,
tokenization, admission and publication stay in Coax.

## Bounds make the format an executable contract

| Resource | Bound |
| --- | --- |
| Ordinary input or decoded conversation text | 65,536 bytes; render preflight includes headers and an optional open assistant role. |
| Token stream | 16,384 IDs, including structural IDs. |
| Complete messages | 64. |
| Tool name or recipient | 1–64 ASCII bytes when present. |
| Decoded header | At most 256 bytes. |
| Ordinary regex piece | At most 1,024 bytes. |
| BPE work | Sum of squared piece byte lengths at most 16,777,216. |

At the maximum bounds, encode/render can use 16,384 integer slots each for
output and stage, plus 1,024 symbol slots. Parsing can use 64 records each for
output and draft, plus 65,536 bytes each for decoded output and scratch.
Smaller buffers are valid when the actual result fits.

The caller supplies valid allocations, truthful extents, alignment, immutable
input/table bytes and exclusive writable access. Raw Coax calls pass the
observed overlap flag; the C adapter obtains it from addresses, including
nested message strings. Hash admission and those storage obligations precede
model use. These budgets refuse work; they never silently truncate a request.

## Run the whole path

<!-- source: psionic/examples/harmony.coax example -->
```jai
main :: () {
    table:=HS_TOY;tokens:[512]int;stage:[512]int;symbols:[1024]int;records:[4]HmRecord;draft:[4]HmRecord;bytes:[1024]u8;work:[1024]u8;
    user:=[1]HmMessage.[HmMessage.{role=HmRole.user,body="<|start|> stays text."}];
    rendered:=hm_render_checked(table[..],user[..],false,tokens[..],stage[..],symbols[..],false);
    parsed:=hm_parse_checked(table[..],tokens[0..<rendered.written],false,records[..],draft[..],bytes[..],work[..],false);
    #assert rendered.status==HmStatus.ok && parsed.status==HmStatus.ok;
    #assert cast(string)bytes[records[0].body_begin..<records[0].body_end]==user[0].body;
    print("ordinary markers: preserved as body text\n");
    reply:=[2]HmMessage.[HmMessage.{role=HmRole.assistant,channel=HmChannel.analysis,body="Check the arithmetic."},HmMessage.{role=HmRole.assistant,channel=HmChannel.final,ending=HmEnd.turn,body="4."}];
    rendered=hm_render_checked(table[..],reply[..],false,tokens[..],stage[..],symbols[..],false);
    parsed=hm_parse_checked(table[..],tokens[0..<rendered.written],true,records[..],draft[..],bytes[..],work[..],false);
    #assert parsed.status==HmStatus.ok && parsed.written==2 && parsed.stop==HmEnd.turn;
    #assert records[0].channel==HmChannel.analysis && records[1].channel==HmChannel.final;
    print("completion: % messages; analysis then final; stop=return\n",parsed.written);
    bytes[0]=7;parsed=hm_parse_checked(table[..],tokens[0..<rendered.written-1],true,records[..],draft[..],bytes[..],work[..],false);
    #assert parsed.status==HmStatus.incomplete && bytes[0]==7;
    print("truncated turn: refused without output\n");
    call:=[1]HmMessage.[HmMessage.{role=HmRole.assistant,channel=HmChannel.commentary,ending=HmEnd.call,recipient="functions.weather",body="{}"}];
    rendered=hm_render_checked(table[..],call[..],false,tokens[..],stage[..],symbols[..],false);
    parsed=hm_parse_checked(table[..],tokens[0..<rendered.written],true,records[..],draft[..],bytes[..],work[..],false);
    #assert parsed.status==HmStatus.ok && parsed.stop==HmEnd.call;
    print("handoff: %; stop=call\n",cast(string)bytes[records[0].recipient_begin..<records[0].recipient_end]);
    system:=hm_system_checked(1,bytes[..],work[..],false);#assert system.status==PtStatus.ok;
    print("reasoning: medium\n");
}
```

The [owned oracle](../tests/harmony_oracle.py) compares exact bytes and IDs,
with no numerical tolerance. It covers original-rank text, Unicode boundaries,
all supported roles, the three efforts, literal marker text, canonical and
alternate headers, analysis followed by final, and tool handoffs. Native checks
also exercise all special IDs, UTF-8 fragments, proper-prefix truncation,
malformed streams, real aliases and unchanged public state on refusal. The
focused gate runs debug/release, ASan/UBSan, source-fixture drift checks, example
transcripts and zero-survivor authority mutation. The
[verification record](../../docs/verification/psionic-ch44-2026-10-29.md)
records the actual results and scope.

This chapter qualifies a named Unicode-17 text profile against owned
independent references and pinned source data. It does not claim complete
upstream-library parity for every Unicode/runtime version or optional Harmony
extension. Chapter **45. Execute gpt-oss-20b** must now join original-weight
execution, routing, attention, this token stream and sampling, with independent
intermediate and logit references. Correct framing is one of those gates.
