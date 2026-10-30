# 57. DeepSeek’s front door

A conversation has two kinds of material: ordinary text and boundaries the
recipe inserted. Keep that distinction through tokenization and decoding.
Searching a rendered string for `<think>` would let a user change the
conversation by typing it. Searching the growing generated answer on every
token would also make streaming quadratic.

`deepseek-front-v1` implements a bounded recipe and incremental protocol parser
in Coax. Its recipe follows the text subset of the pinned
[DeepSeek V4.1 renderer](https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe-encoding/src/v4/dsv41.rs).
Its parser follows typed reasoning boundaries and V4.1 DSML tool frames from
the pinned [output parser](https://github.com/deepseek-ai/deepseek-recipe/blob/935817c66f5d6e64838f419692545f06787e485f/deepseek-recipe/src/stream/output_parser.rs).
These sources are read-only references; their code is neither restored nor a
runtime dependency. The checks use owned protocol fixtures, chapter 56’s admitted original R1
tokenizer and the original V4.1 tokenizer through its separate text adapter.

The original R1 tokenizer has no V4.1 DSML token. We provide an explicitly
named R1 encoding/decoding adapter and refuse unmapped controls; this chapter
keeps that adapter separate from V4.1. The V4.1 text adapter below qualifies
its original tokenizer on CPU; the checkpoint, complete upstream recipe,
tool execution, schemas and constrained generation remain separate. Chapter 56’s existing
R1 history recipe remains its own versioned contract.

## Run the front door

```shell
coax/bin/coax run psionic/examples/deepseek-front.coax
coax/bin/coax run psionic/examples/deepseek-front.coax --release
psionic/tests/check_deepseek_front.sh
psionic/check.sh --only deepseek/front --jobs 8
```

The [example](../examples/deepseek-front.coax) feeds one typed unit at a time,
so every delimiter and UTF-8 scalar crosses calls. It prints each tool’s name
and assembled arguments, while the underlying interface emits their bytes
as soon as they are available. Reasoning and final text have distinct event
kinds and never share a display buffer by default.

<!-- source: psionic/examples/deepseek-front.coax example -->
```jai
main :: () {
    messages:=[2]DfMessage.[DfMessage.{role=DfRole.user,content="<think>"},DfMessage.{role=DfRole.tool,content="｜DSML｜"}];
    segments:[32]DfSegment;scratch:[32]DfSegment;
    recipe:=df_recipe(messages[..],false,segments[..],scratch[..],false);
    #assert recipe.status==DfStatus.ok;
    print("recipe: % segments; user/tool text stays ordinary\n",recipe.written);
    units:=DFF_UNITS;events:[128]DfEvent;stage:[128]DfEvent;s:=DfState.{};
    name:[64]u8;arguments:[256]u8;names:=0;args:=0;
    for i:0..<units.count {
        r:=df_feed(s,units[i..<i+1],events[..],stage[..],false);#assert r.status==DfStatus.ok;s=r.state;
        for j:0..<r.written {e:=events[j];
            if e.kind==DfKind.tool_begin {names=0;args=0;}
            if e.kind==DfKind.name {name[names]=e.byte;names+=1;}
            if e.kind==DfKind.arguments {arguments[args]=e.byte;args+=1;}
            if e.kind==DfKind.tool_end {print("tool %: %\n",cast(string)name[0..<names],cast(string)arguments[0..<args]);}
        }
    }
    #assert df_end(s,false).status==DfStatus.incomplete;
    #assert df_end(s,true).status==DfStatus.ok;
    print("EOS without finish: incomplete; EOS with finish: complete\n");
    table:=PT_TOY;tokens:[32]int;token_stage:[32]int;symbols:[32]int;
    encoded:=dvf_text(table[..],"é",tokens[..],token_stage[..],symbols[..],false);
    #assert encoded.status==PtStatus.ok && encoded.written==3;
    print("V4.1: decomposed accent stays % owned byte tokens; DSML ID %\n",encoded.written,dvf_control(DfSpecial.dsml));
}
```

The first call’s string contains a quote, a backslash, newline, tab, NUL and
an emoji. JSON string escaping happens in Coax: quotes and backslashes receive
a backslash, control bytes receive `\u00xx`, and validated UTF-8 stays UTF-8.
Non-string parameter values stream as raw fragments. Their JSON validity and
schema are separate checks a consumer must perform before granting a call.
A `tool_end` event closes a protocol frame; it grants no tool authority.

## Render boundaries, then encode the plain runs

`DfSegment` carries either a `DfSpecial` boundary or a borrowed ordinary string.
The recipe accepts an optional first system message, user and tool text,
and final-only assistant history. Consecutive user/tool messages share one
user boundary, separated by blank lines; tools get ordinary `<tool_result>`
wrappers. Assistant history gets assistant/end-thinking boundaries and EOS.
The generation prefix chooses thinking or end-thinking. Thinking mode writes
the pinned default effort text, 75, after a system boundary. Earlier reasoning
is absent from this API, rather than extracted from strings.

The subset requires a final user/tool message, rejects misplaced systems and
adjacent assistants, and accepts no schema, media, tool-declaration, assistant
prefix or strict-tool options. It is deliberately smaller than the upstream
conversation API. Supplied tool text is already ordered; this layer has no
call-ID sorting or execution policy.

Read [the recipe](../lib/deepseek/front-recipe.coax) and
[the R1 adapter](../lib/deepseek/front-encode.coax). `df_encode_r1` joins adjacent
plain fragments before tokenization. A UTF-8 scalar, NFC composition or BPE
merge may cross such a fragment boundary. Each real special boundary flushes
the run and inserts its mapped ID directly. Spelled markers in system, user,
assistant or tool strings always pass through ordinary byte BPE. The R1
adapter maps BOS, EOS, user, assistant and the two thinking controls; system
and DSML refuse. A recipe needing either requires another qualified adapter.

`df_r1_units` performs the inverse boundary-preserving operation: known control
IDs become typed units, and ordinary token bytes remain ordinary units,
including any marker spelling assembled across multiple tokens. Unknown
added-token controls refuse. Token fragments can end mid-scalar; UTF-8
validation belongs to the incremental parser, not the per-token decoder.

## Carry only a bounded suffix

The [stream parser](../lib/deepseek/front-stream.coax) receives integers 0–255
for ordinary bytes, or 256 plus a `DfSpecial` value for a genuine control.
These are parser units, **not vocabulary IDs**. A tokenizer adapter must
produce them from token identity. A transport must never manufacture them by
searching decoded text for spellings.

The initial state follows an already-inserted thinking prefix. Use
`DfState.{phase=1}` after an end-thinking prefix. Reasoning ends only on the
typed end-thinking unit; content can open a DSML call block. The supported
frames use ` calls`, ` invoke name="..."`, and
` parameter name="..." string="true|false"` with exact closing tags.
Only the DSML portion is a special: the angle brackets, tag names and
attributes are ordinary bytes. A plain string spelling the entire marker
never matches. The parser accepts whitespace between frames and preserves
reasoning/answer whitespace exactly.

Names and parameter keys use 1–64 ASCII letters, digits, underscore, dot or
hyphen. The profile accepts at most 64 calls and 64 parameters per call.
Malformed framing and unexpected controls refuse. Parameter strings escape
incrementally; raw values stay raw. This is a bounded protocol grammar,
not an XML parser or JSON validator.

The state holds a 40-unit delimiter window and at most four UTF-8 bytes,
regardless of answer length. Prefix comparisons and shifts inspect only this
fixed window. Each unit enters once and is either consumed in a delimiter
or emitted once: total parser work is linear in input size, including feeds
of one byte. It never re-decodes the accumulated answer. A scalar is emitted
only after its final byte passes overlong, surrogate and Unicode-range checks.

<!-- source: psionic/lib/deepseek/front-stream.coax feed -->
```jai
df_feed :: (state:DfState,units:[]int,output:[]DfEvent,stage:[]DfEvent,overlap:bool)->DfResult #export_c {
    if overlap {return DfResult.{status=DfStatus.alias,state=state};}
    if state.finished || state.phase<0 || state.phase>9 {return DfResult.{status=DfStatus.state,state=state};}
    if units.count>4096 || units.count>65536-state.total {return DfResult.{status=DfStatus.budget,state=state};}
    s:=state;w:=DfWriter.{events=stage};
    for c:units {r:=df_push(s,c,w);s=r.s;w=r.w;if w.status!=DfStatus.ok {return DfResult.{status=w.status,state=state};}}
    if w.written>output.count {return DfResult.{status=DfStatus.capacity,state=state};}
    for i:0..<w.written {output[i]=stage[i];}s.total+=units.count;return DfResult.{state=s,written=w.written};
}
df_end :: (state:DfState,finish:bool)->DfResult #export_c {
    if state.finished {return DfResult.{status=DfStatus.state,state=state};}
    if !finish || state.phase!=9 || state.count!=0 || state.left!=0 {return DfResult.{status=DfStatus.incomplete,state=state};}
    s:=state;s.finished=true;return DfResult.{state=s};
}
```

EOS closes syntax. `df_end(state, true)` represents a normal finish event and
succeeds only after EOS, with no pending delimiter or UTF-8 bytes. Transport
EOF uses `false` and is always incomplete, even after EOS. Truncation during
reasoning, a name, arguments or a scalar stays incomplete. Token limits and
cancellation retain chapter 56’s separate generation decisions; they must
not be relabeled as a normal finish here. Calls after a successful finish
refuse.

## Publish a chunk together

`df_feed` returns the next state by value and stages all events. On failure,
it returns the supplied state, writes no public events and reports zero
written; private stage memory may change. Retrying with more capacity is safe.
Successful earlier chunks remain committed. Recipe segments, encoded IDs and
decoded units use the same staged-publication rule and leave spare public
tails unchanged.

Bounds are 64 messages, 512 input segments, 65,536 content bytes, 16,384 encoded
IDs, a 16,777,216 BPE piece-work budget, and 4,096 units per parser feed or
token-fragment decode. A stream admits 65,536 units in total. Workspaces are
caller-owned. The native [adapter](../platform/deepseek-front-memory.c) only
observes address overlap, including borrowed nested strings. The caller must
supply valid typed values, allocations, truthful extents, disjoint writable
spans, immutable admitted tokenizer data and live borrowed strings, and use
only an initial or previously returned parser state. Commit returned state
and events together, with exclusive access.

The [owned oracle](../tests/deepseek_front_fixture.py) authors expected typed
units and events independently of the Coax state machine. Native checks compare
all **32,896 pairs of split points** for its 255-unit two-tool stream, every
truncation, a 65,536-unit content stream, literal-marker safety through the
original tokenizer, every byte split of independent chapter 56 BPE vectors,
publication tails and nested overlap refusal. Debug/release, ASan/UBSan,
exact book output and zero-survivor authority mutation are recorded in the
[verification record](../../docs/verification/psionic-ch57-2026-10-29.md).

Next, chapter 58 moves from the recipe to DeepSeek V2-Lite’s latent attention
and experts. Nothing here changes the numerical qualification of a model.

## The original V4.1 text tokenizer

Load [front-v41.coax](../lib/deepseek/front-v41.coax) for the original V4.1
adapter plus the bounded recipe and stream parser. Its profile is
`deepseek-front-v41-text-v1`; R1 retains its separate adapter and history API.
The original vocabulary has 129,280 entries and 127,741 ranked merges. The
[packed table](../fixtures/deepseek/front-v41.pst) is admitted against its
[recorded source and derived identities](../fixtures/deepseek/front-v41-source.json)
before use. The native gate performs that admission with Coax `pw_digest` and
rejects a changed byte.

V4.1 does no Unicode normalization. Decomposed accents stay decomposed. It
first isolates number runs in groups of up to three, then runs in the exact
Han/Hiragana/Katakana ranges from its source, then applies its word,
punctuation and whitespace split. Unmatched gaps are retained. Ranked byte BPE
runs inside each resulting piece. Unicode **16.0.0** properties are explicit
data: using the current regex library's newer Unicode database changed token
boundaries for newly assigned letters. The retained regressions include both
Unicode-16 and Unicode-17 additions.

<!-- source: psionic/lib/deepseek/front-v41-token.coax v41-text -->
```jai
dvf_text :: (table:[]u8,text:string,output:[]int,stage:[]int,symbols:[]int,overlap:bool)->PtResult #export_c {
    if overlap {return PtResult.{status=PtStatus.alias};}if !pt_text_count(text.count) {return PtResult.{status=PtStatus.size};}
    valid:=pt_utf8_validate(text);if valid!=PtStatus.ok {return PtResult.{status=valid};}t:=pt_layout(table);if t.status!=PtStatus.ok {return PtResult.{status=t.status};}
    at:=0;n:=0;work:=0;
    while at<text.count {partition:=dvf_partition(table,t,text,at);part:=text[at..<partition];pos:=0;
        while pos<part.count {end:=dvf_piece(table,t,part,pos);width:=end-pos;if !pt_piece_budget(work,width) {return PtResult.{status=PtStatus.budget};}work+=width*width;
            r:=pt_bpe(table,t,part[pos..<end],symbols);if r.status!=PtStatus.ok {return r;}if r.written>stage.count-n {return PtResult.{status=PtStatus.capacity};}
            for i:0..<r.written {stage[n+i]=symbols[i];}n+=r.written;if !pt_token_count(n) {return PtResult.{status=PtStatus.size};}pos=end;
        }at=partition;
    }
    if n>output.count {return PtResult.{status=PtStatus.capacity};}for i:0..<n {output[i]=stage[i];}return PtResult.{written=n,work=work};
}
```

`dvf_encode` accepts the recipe's typed segments. Adjacent plain fragments
rejoin before UTF-8 validation and BPE, even if a fragment ends inside a scalar.
Only real controls end a run. Their original IDs are:

| Boundary | ID |
| --- | --- |
| BOS / EOS | 0 / 1 |
| System / User / Assistant | 128799 / 128803 / 128804 |
| Think / end-think | 128821 / 128822 |
| DSML | 128825 |

`dvf_units` maps those controls back to the existing parser's typed units and
ordinary token IDs to their exact byte strings. It refuses every other added
ID. Literal marker spellings in ordinary text stay ordinary bytes. The adapter
stages the complete output before publication; capacity, malformed UTF-8,
unsupported IDs or overlap leave public output untouched. The exported
low-level span probes require a validated UTF-8 slice, a live scalar start and
a successful unmodified layout; the checked text entry points establish those
preconditions. Private work may
change. The library allocates nothing; callers own admitted immutable table
and text lifetimes, valid extents and exclusive disjoint write storage.

The default CPU gate needs no tokenizer package, network, weights or GPU:

```shell
psionic/tests/check_deepseek_front_v41.sh
psionic/check.sh --only deepseek/front-v41 --jobs 8
```

The [supplemental verification record](../../docs/verification/psionic-ch57-v41-2026-10-30.md)
checks all ordinary vocabulary bytes, every Unicode property lookup, independent
text/recipe token IDs, and reasoning/tool events through actual token split
pairs. The original tokenizer library separately checks every Unicode scalar
probe against the pinned-data split equations. Neither gate runs model inference.

This still uses the bounded `df_recipe` contract described above. Full upstream
tool declarations/schema formatting, namespaces, call-ID sorting, numeric effort
options, mid-conversation systems, assistant prefixes and media remain outside
that API. Tokenization parity does not establish model-output parity or authorize
any tool execution.
