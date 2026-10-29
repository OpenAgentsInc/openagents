# 5. Text becomes a model request

[Chapter 4](ch04-reading-original-weights.md) bound original bytes to an
expected model. Now we will turn state and typed questions into the exact
text and token sequence that a decision model sees.

We will build four shared Coax pieces: a frozen text renderer, Unicode NFC
normalization, the original Qwen3.5 byte-pair tokenizer, and a token-prefix
splitter. The result has a shared prefix, one suffix per question, answer
positions and a mapping from answer slots to the caller's ordered options.
Attention and numerical execution begin in chapter 6.

## Run the example

From the repository root, with Coax built:

```shell
coax/bin/coax run psionic/examples/text-request.coax
psionic/check.sh
```

The [example](../examples/text-request.coax) prints:

```jai
request: 2 slots, 380 UTF-8 bytes
prefix candidate: first 219 bytes
<|im_start|>system
Apply the question to the state. Choose exactly one of the listed options. Respond with only its uppercase letter, with no explanation or reasoning.<|im_end|>
<|im_start|>user
<state>
ready
</state>

Question: Which option applies to the state?

Options:
A. go: continue
B. stop

Answer with one letter: A, B.<|im_end|>
<|im_start|>assistant
<think>

</think>


teaching BPE: [256, 257, 262, 263]; work bound = 65
shared tokens: 2; suffix starts: [2, 7, 11]
refused: duplicate option label; published bytes unchanged
```

The prompt above uses the real frozen renderer. The merge demonstration uses
a small owned teaching vocabulary so we can inspect its steps. Those four
IDs belong to that teaching vocabulary. The independent native checks load
the full original-Qwen table and compare actual Qwen token IDs, complete
rendered requests and shared-prefix splits.

For this exact `ready` request, the pinned Qwen data gives 87 full
prompt tokens and 46 tokens for the separately encoded prefix candidate.
The illustrative three-branch vectors at the end of the program deliberately
have a different shared-prefix length, two, to demonstrate shrinking.

## Pin a renderer and tokenizer together

Our early targets are [Elephant and Mammoth](../docs/targets/model-targets.md).
Their pinned originals share the same `tokenizer.json` bytes: SHA-256
`a5cd9732badce41de57e6efce8302930ded1c1188c5f81feb2bd6c24c4a1941f`.
We read both originals separately. The serving contract is OneJev
[`81ce62f1597c91e46767d4d02d6ac2e18534fe94`](https://github.com/OmniJev/OneJev/tree/81ce62f1597c91e46767d4d02d6ac2e18534fe94),
plus the pinned model's
[serving patch](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/serve.patch)
and [chat template](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/chat_template.jinja).
Moving names are insufficient to identify a prompt contract.

The tokenizer data contains 248,044 byte-BPE vocabulary entries, 247,587
ranked merge rules and 33 added literals. Its IDs run through 248,076; the
model's larger 248,320-column vocabulary also reserves entries. The tokenizer
has NFC normalization, an explicit split expression, no automatic BOS token,
no added prefix space, no truncation and no padding.

[The retained data](../fixtures/text/ch05/README.md) includes the original
compressed tokenizer, deterministic lookup indexes, identities and notices.
Python is authoring/verification tooling. The runtime library receives immutable
data and caller storage through generated C interfaces. C supplies file and
memory effects; Coax performs text, token and admission work.

## A typed question has ordered options

The library uses typed values at its boundary. Existing
[System One codecs](../../coax/lib/systemone/README.md) remain the wire boundary;
this chapter introduces no protocol change. A caller keeps question IDs and
answer labels alongside each branch. Question IDs never enter the prompt.

<!-- source: psionic/lib/text/request.coax types -->
```jai
PtEntryKind :: enum u8 {none;text;json;}
PtEntry :: struct {kind:PtEntryKind;value:string;}
PtQuestionKind :: enum u8 {choice;noul;score;}
PtQuestion :: struct {kind:PtQuestionKind;instructions:PtEntry;}
PtOption :: struct {name:string;description:PtEntry;}
PtPrompt :: struct {status:PtStatus;written:int;prefix_bytes:int;slots:int;}
```

`PtEntry` represents null, text, or a JSON object/array. `text` preserves the
state string exactly, while instructions and descriptions use Python-compatible
Unicode `strip`. `json` renders structured content with two-space indentation,
insertion order, UTF-8 characters, standard string escapes and integer/boolean/null
values. Duplicate object keys are refused, including escaped spellings of
the same key. Decimal/exponent JSON numbers are explicitly unsupported in this
first request profile; they are not reformatted approximately. Integer spellings
up to 4,300 decimal digits remain exact, with `-0` rendered as `0`; longer
integers are refused. This matches the default integer-string limit in the
tested Python 3.15.1; configured limits may differ.

| Kind | Options / labels | Default instruction |
| --- | --- | --- |
| `choice` | 1–255, original option names in caller order. | Which option applies to the state? |
| `noul` | Two slots, `yes` then `no`; optional true/false descriptions. | Is the statement true, or is the answer to the question yes? |
| `score` | 1–10, labels `0` through `n-1`; display names `level 0`, etc. | Which level describes the state? |

The pinned schema actually accepts a one-level score. This corrects the
planning note's earlier 2–10 lower bound. The implementation preserves that
edge behavior. A `noul` request can supply zero description records to use
both defaults, or two ordered records. Null descriptions use its defaults;
an explicitly blank string remains blank after stripping.

A choice line has `A. name: description` when a description is nonempty, and
`A. name` otherwise. Names retain whitespace. Instructions and descriptions
can themselves be structured JSON. A score uses `Rate the state:`; other
kinds use `Question:`. More than 26 options switch the final instruction from
one *letter* to one *label*. Slot labels follow the frozen A–Z and two-letter
list, including its exclusions. All 256 labels are checked as single-token
round trips; choice requests retain the schema's 255-option maximum.

## Specialize the actual two-turn template

The user turn is `<state>`, rendered state, `</state>`, and the question suffix.
The system turn is fixed. The pinned engine passes `enable_thinking = false`,
so the assistant prefix still contains the empty thinking delimiters shown
in the transcript. Omitting them would move the answer position.

```jai
system turn + user turn:
    <state> ... </state>

 + question + options + answer instruction
assistant prefix:
    <|im_start|>assistant
<think>

</think>


```

[The renderer](../lib/text/request.coax) specializes this default tagged,
text-only two-turn contract. General Jinja execution, arbitrary chat histories,
tools and multimodal preprocessing are separate features. It stages the complete
prompt and copies it to the published byte buffer only after every check passes.

It also reports a prefix-candidate byte cut. The pinned engine finds the first
`QEV_PREFIX_MARK` occurrence when rendering a sentinel suffix. We preserve this
behavior, including the unusual case where the state itself contains that text:
the candidate cut moves earlier, and token matching still verifies the shared
prefix. No synthetic token boundary is trusted.

## Unicode normalization changes the bytes before BPE

UTF-8 is a byte encoding; NFC is a normalization operation on Unicode scalars.
A string containing `e` followed by combining acute accent and a string
containing `é` have different input bytes but the same NFC result. Canonical
ordering is stable for marks of the same combining class, and composition must
respect blocking by intervening classes. Hangul jamo compose as well.

Our `qwen35-text-v1` profile pins Unicode 17.0.0 properties and normalization
data. The [NFC unit](../lib/text/unicode.coax) validates UTF-8, expands complete
canonical decompositions into scalar workspace, orders combining runs, composes
eligible pairs and writes UTF-8 only after its final capacity check. The
[Unicode normalization specification](https://www.unicode.org/reports/tr15/tr15-57.html)
explains these operations and composition exclusions.

<!-- source: psionic/lib/text/unicode.coax normalize -->
```jai
pt_normalize_checked :: (table:[]u8,input:string,output:[]u8,scalars:[]int,overlap:bool)->PtResult #export_c {
    if !pt_text_count(input.count) {return PtResult.{status=PtStatus.size};}
    if overlap {return PtResult.{status=PtStatus.alias};}
    t:=pt_layout(table);if t.status!=PtStatus.ok {return PtResult.{status=t.status};}
    n:=0;at:=0;marks:=0;
    while at<input.count {
        r:=pt_rune(input,at);if r.status!=PtStatus.ok {return PtResult.{status=r.status};}at=r.next;
        d:=pt_decomposition(table,t,r.cp);width:=1;
        if d>=0 {width=pt_u32(table,pt_row(t,3,d)+4);if width<1 || width>4 {return PtResult.{status=PtStatus.artifact};}}
        if width>scalars.count-n {return PtResult.{status=PtStatus.capacity};}
        for k:0..<width {
            cp:=r.cp;if d>=0 {cp=pt_u32(table,pt_row(t,3,d)+8+k*4);}
            pos:=n;cc:=pt_prop(table,t,cp).ccc;
            if cc>0 {marks+=1;}else {marks=0;}
            if !pt_marks(marks) {return PtResult.{status=PtStatus.budget};}
            while pos>0 && cc>0 && pt_prop(table,t,scalars[pos-1]).ccc>cc {
                scalars[pos]=scalars[pos-1];pos-=1;
            }
            scalars[pos]=cp;n+=1;
        }
    }
    if n==0 {return PtResult.{};}
    size:=1;starter:=0;last:=pt_prop(table,t,scalars[0]).ccc;
    for i:1..<n {
        cp:=scalars[i];cc:=pt_prop(table,t,cp).ccc;composed:=pt_pair(table,t,4,scalars[starter],cp);
        if composed>=0 && (last<cc || last==0) {scalars[starter]=pt_u32(table,pt_row(t,4,composed)+8);}
        else {if cc==0 {starter=size;}scalars[size]=cp;size+=1;last=cc;}
    }
    bytes:=0;for i:0..<size {if !pt_scalar(scalars[i]) {return PtResult.{status=PtStatus.artifact};}bytes+=pt_utf8_width(scalars[i]);}
    if bytes>output.count {return PtResult.{status=PtStatus.capacity};}
    pos:=0;for i:0..<size {pos=pt_put_utf8(output,pos,scalars[i]);}
    return PtResult.{written=pos};
}
```

Normalization can dirty scalar workspace on failure; published bytes remain
unchanged. Combining runs are limited to 128 nonzero-class scalars to bound
insertion ordering. That is a resource refusal, with no dropped marks.
Caller scalar capacity must cover the expanded decomposition, not merely the
number of input characters. Four scalars per input byte is a conservative
bound for this pinned data; smaller actual capacities are admitted when sufficient.

## Split before merging

The pinned expression distinguishes contractions, optional punctuation/space
before letter-and-mark runs, individual numeric scalars, punctuation runs,
newlines and trailing whitespace. Numeric text `123` is split into individual
numeric scalars. Apostrophe contractions match prefixes: `'real` starts with
`'re`. Unicode case folding includes the long-s character in the `'s` case.

Two spaces before a word can split differently from one space. Whitespace
before a newline can group with that newline, and punctuation can include
following CR/LF bytes. These details can change later merges. We keep a separate
span oracle so a missing vocabulary merge cannot hide a wrong split.

<!-- source: psionic/lib/text/tokenizer.coax pieces -->
```jai
// The pinned split expression; numeric scalars are disjoint from word runs.
pt_piece :: (table:[]u8,t:PtLayout,s:string,at:int)->int {
    end:=pt_contraction(s,at);if end>at {return end;}
    r:=pt_rune(s,at);if pt_has(table,t,r.cp,4) {return r.next;}
    pos:=at;if r.cp!=13 && r.cp!=10 {pos=r.next;}
    end=pt_run(table,t,s,pos,3,true);
    if end>pos {return end;}
    end=pt_run(table,t,s,at,3,true);if end>at {return end;}
    pos=at;if r.cp==32 {pos=r.next;}
    end=pt_run(table,t,s,pos,15,false);
    if end>pos {
        while true {if end==s.count {return end;}c:=pt_rune(s,end);if c.cp!=13 && c.cp!=10 {return end;}end=c.next;}return end;
    }
    space:=at;last_newline:=at;last_space:=at;
    while true {
        if space==s.count {break;}c:=pt_rune(s,space);if !pt_has(table,t,c.cp,8) {break;}
        last_space=space;space=c.next;if c.cp==13 || c.cp==10 {last_newline=space;}
    }
    if last_newline>at {return last_newline;}
    if space==s.count {return space;}
    if last_space>at {return last_space;}return space;
}
```

The scanner operates on normalized UTF-8 with a pinned Unicode property table.
Its numeric branch is disjoint from word matching. Added literals are extracted
from the original input before normalization, longest first, matching their
`normalized = false` behavior. All 33 added literals participate, including
entries that the artifact does not mark `special`. Literal special-token text
therefore has the tokenizer's actual token semantics. Media feature generation
and model execution require their own later admission.

## Byte-pair encoding is ranked merging

The tokenizer first maps every UTF-8 byte through the original 256-byte alphabet
to its initial vocabulary ID. It repeatedly selects the lowest-rank adjacent
pair. Equal ranks choose the leftmost pair. Each merge replaces two symbols
with the original vocabulary ID for their concatenation.

For the owned teaching table, rank zero merges `a + b` into ID 256 and rank one
merges `ab + a` into 257:

```jai
ababa
[a, b, a, b, a]
[ab, a, b, a]       // rank 0, leftmost
[ab, ab, a]        // rank 0 again
[ab, aba]          // rank 1
[256, 257]
```

The original Qwen table supplies its own IDs and ranks. We do no longest-word
lookup or hash-based guessing. The [encoder](../lib/text/tokenizer.coax) stages
IDs from every normalized piece, validates budgets and capacities, then publishes
the complete sequence.

<!-- source: psionic/lib/text/tokenizer.coax encode -->
```jai
// Only output is published. Stage/normal/scalars/symbols are disposable workspaces.
pt_encode_checked :: (table:[]u8,input:string,output:[]int,stage:[]int,normal:[]u8,scalars:[]int,symbols:[]int,overlap:bool)->PtResult #export_c {
    if !pt_text_count(input.count) {return PtResult.{status=PtStatus.size};}
    if overlap {return PtResult.{status=PtStatus.alias};}
    t:=pt_layout(table);if t.status!=PtStatus.ok {return PtResult.{status=t.status};}
    at:=0;n:=0;used:=0;
    while at<input.count {
        added:=pt_added(table,t,input,at);if added.status!=PtStatus.ok {return PtResult.{status=added.status};}
        if added.rank!=-1 {
            if n>=stage.count {return PtResult.{status=PtStatus.capacity};}stage[n]=added.id;n+=1;at+=added.rank;
        }else {
            end:=at+1;
            while end<input.count {found:=pt_added(table,t,input,end);if found.status!=PtStatus.ok {return PtResult.{status=found.status};}if found.rank!=-1 {break;}end+=1;}
            norm:=pt_normalize_checked(table,input[at..<end],normal,scalars,false);if norm.status!=PtStatus.ok {return norm;}
            if !pt_text_count(norm.written) {return PtResult.{status=PtStatus.size};}
            text:=cast(string)normal[0..<norm.written];pos:=0;
            while pos<text.count {
                next:=pt_piece(table,t,text,pos);width:=next-pos;
                if !pt_piece_budget(used,width) {return PtResult.{status=PtStatus.budget};}used+=width*width;
                bpe:=pt_bpe(table,t,text[pos..<next],symbols);if bpe.status!=PtStatus.ok {return bpe;}
                if bpe.written>stage.count-n {return PtResult.{status=PtStatus.capacity};}
                for k:0..<bpe.written {stage[n+k]=symbols[k];}n+=bpe.written;pos=next;
            }
            at=end;
        }
        if !pt_token_count(n) {return PtResult.{status=PtStatus.size};}
    }
    if n>output.count {return PtResult.{status=PtStatus.capacity};}
    for i:0..<n {output[i]=stage[i];}return PtResult.{written=n,work=used};
}
```

`written` counts tokens. `work` is the admitted conservative BPE bound
`sum(piece_utf8_bytes²)`, with added literals contributing zero. It is a resource
accounting unit, not elapsed time or a count of actually executed comparisons.
Every encoding refusal reports zero tokens and zero work; stage, normal bytes,
scalar and symbol workspaces can be dirty. The published ID buffer and unused
capacity stay unchanged until success.

## A byte cut is only a candidate token cut

Encode each complete branch first. Separately encode the rendered prefix bytes.
Then find how much of that candidate token sequence agrees with *every* complete
branch. A merge can straddle a proposed byte boundary, so concatenating separately
encoded state and question IDs can change the model input.

[The splitter](../lib/text/prefix.coax) takes flattened full-branch IDs, exclusive
ends and candidate IDs. It validates all intervals and token IDs before publishing
metadata. It refuses empty suffixes and never shares tokens from one question's
suffix with its siblings.

<!-- source: psionic/lib/text/prefix.coax split -->
```jai
// `full` is the concatenation of complete branch encodings; ends are exclusive.
// Tokenize the prefix candidate separately, then shrink it against EVERY branch.
pt_split_checked :: (candidate:[]int,full:[]int,ends:[]int,output:[]PtBranch,overlap:bool)->PtSplit #export_c {
    if !pt_branch_count(ends.count) || !pt_token_count(candidate.count) {return PtSplit.{status=PtStatus.size};}
    if output.count<ends.count {return PtSplit.{status=PtStatus.capacity};}
    if overlap {return PtSplit.{status=PtStatus.alias};}
    for id:candidate {if !pt_id(id) {return PtSplit.{status=PtStatus.range};}}
    start:=0;n:=candidate.count;
    for end:ends {
        if !pt_branch_span(start,end,full.count) {return PtSplit.{status=PtStatus.range};}
        if !pt_token_count(end-start) {return PtSplit.{status=PtStatus.size};}
        for i:start..<end {if !pt_id(full[i]) {return PtSplit.{status=PtStatus.range};}}
        shared:=0;
        while shared<n && shared<end-start {if candidate[shared]!=full[start+shared] {break;}shared+=1;}
        n=shared;start=end;
    }
    if start!=full.count {return PtSplit.{status=PtStatus.range};}
    start=0;for end:ends {if end-start==n {return PtSplit.{status=PtStatus.range};}start=end;}
    start=0;
    for i:0..<ends.count {output[i]=PtBranch.{begin=start+n,end=ends[i],answer=ends[i]-start-1};start=ends[i];}
    return PtSplit.{written=ends.count,prefix=n};
}
```

A returned `begin`/`end` interval locates suffix tokens in the flattened full
encodings. `answer` is the final token's position in that branch's **complete**
sequence. A suffix-local answer index is `end - begin - 1`; absolute model
positions start at the shared prefix length. This chapter constructs the layout;
KV caches, attention masks and recurrent prefix snapshots come later.

## Native boundaries and explicit budgets

| Resource | This profile |
| --- | --- |
| Input text / rendered branch | At most 65,536 UTF-8 bytes. Each normalized ordinary run is also bounded to 65,536 bytes. |
| Encoded branch / candidate prefix | At most 16,384 tokens, with no truncation. |
| Sibling questions in a split | 1–32; each full branch is independently bounded. |
| Choice / `noul` / score | 1–255 / 2 / 1–10 slots. |
| Structured entries | JSON objects/arrays, depth at most 16, at most 256 members per container, integer/boolean/null/string values; integers have at most 4,300 digits. |
| Normalization | At most 128 consecutive nonzero combining classes. |
| One BPE piece | At most 1,024 normalized UTF-8 bytes. |
| BPE work bound | Sum of squared piece lengths at most 16,777,216. |

The lookup table has checked section counts and offsets. Production callers
first admit its exact compiled SHA-256 with `pt_admit`, then hold the table
immutable and alive for subsequent calls. Encoding reparses its bounded layout;
that does not rehash a large table on every question. Editing bytes invalidates
the identity admission. The owned teaching table is a separate test input and
cannot pass production admission.

[The native interface](../platform/text-memory.h) observes writable buffers
against one another and all read-only spans, including nested option names and
entry strings. C performs address observations; pointer validity, truthful
allocation extents, alignment, lifetime and exclusive access remain caller duties.
Public prompt bytes, IDs and split metadata change only on success. Workspaces
may change on refusal. These are synchronous publication contracts.

## The complete example

<!-- source: psionic/examples/text-request.coax example -->
```jai
main :: () {
    state:=PtEntry.{kind=PtEntryKind.text,value="ready"};
    question:=PtQuestion.{kind=PtQuestionKind.choice};
    options:=[2]PtOption.[
        PtOption.{name="go",description=PtEntry.{kind=PtEntryKind.text,value=" continue "}},
        PtOption.{name="stop"}];
    bytes:[2048]u8;scratch:[2048]u8;
    request:=pt_render_checked(state,question,options[..],bytes[..],scratch[..],false);
    #assert request.status==PtStatus.ok && request.slots==2;
    print("request: % slots, % UTF-8 bytes\n",request.slots,request.written);
    print("prefix candidate: first % bytes\n",request.prefix_bytes);
    print("%\n",cast(string)bytes[0..<request.written]);

    // A small owned vocabulary makes the merge steps visible; these are TOY IDs.
    table:=PT_TOY;ids:[32]int;stage:[32]int;normal:[128]u8;scalars:[128]int;symbols:[32]int;
    encoded:=pt_encode_checked(table[..],"ababa hello's",ids[..],stage[..],normal[..],scalars[..],symbols[..],false);
    #assert encoded.status==PtStatus.ok && encoded.written==4 && encoded.work==65;
    #assert ids[0]==256 && ids[1]==257 && ids[2]==262 && ids[3]==263;
    print("teaching BPE: [%, %, %, %]; work bound = %\n",ids[0],ids[1],ids[2],ids[3],encoded.work);

    // Illustrative complete token vectors: the candidate shrinks across branches.
    candidate:=[4]int.[10,11,12,13];full:=[12]int.[10,11,12,20,21,10,11,12,30,10,11,99];
    ends:=[3]int.[5,9,12];branches:[3]PtBranch;
    split:=pt_split_checked(candidate[..],full[..],ends[..],branches[..],false);
    #assert split.status==PtStatus.ok && split.prefix==2;
    print("shared tokens: %; suffix starts: [%, %, %]\n",split.prefix,branches[0].begin,branches[1].begin,branches[2].begin);

    before:=bytes;options[1].name="go";
    refused:=pt_render_checked(state,question,options[..],bytes[..],scratch[..],false);
    #assert refused.status==PtStatus.range && refused.written==0;
    for i:0..<bytes.count {#assert bytes[i]==before[i];}
    print("refused: duplicate option label; published bytes unchanged\n");
}
```

The original-table API is the same encoder used above. The native harness reads
its table file with ordinary C file effects, admits the identity and invokes
Coax. The command-line program uses embedded owned data because the current
native-free Coax driver does not expose a file-view import.

## Verification that can disagree with the candidate

[The fixture tooling](../tests/text_fixtures.py) verifies the original tokenizer
hash and deterministically rebuilds the lookup tables. Its default does not
rewrite artifacts. The independent [oracle](../tests/text_oracle.py) uses
Python's Unicode-17 NFC implementation, a standard-library Unicode regex, and
a priority queue over linked BPE symbols. Coax uses explicit scalar
ordering/composition, a manual split scanner and a linear scan for each
leftmost merge. No candidate output is used to generate the reference.

```shell
python3 -I psionic/tests/text_fixtures.py
python3 -I psionic/tests/text_oracle.py
coax/bin/coax test psionic/lib/text.coax
coax/bin/coax test psionic/lib/text.coax --release
coax/bin/coax mutate psionic/lib/text.coax
psionic/check.sh
```

The native corpus checks 177 original-Qwen tokenization vectors, all 33 added
literals both alone and in context, 34 exact rendered/tokenized requests,
256 single-token answer slots and 26,506 NFC
vectors, including every Hangul decomposition. The evaluator adds owned-table
encoding vectors, independent pretokenizer spans, typed request fixtures,
malformed tables/JSON/UTF-8, exact capacities, duplicate labels and resource
boundaries. Native tests use actual short allocations and aliases. The runner
also checks all earlier chapters and exact book source/transcript/status matches.

The [verification record](../../docs/verification/psionic-ch05-2026-10-29.md)
retains compiler/source identities, mutation results and plain/sanitized native
debug/release evidence. Tooling authoring requires Unicode database 17.0.0;
there is no Python dependency in the runtime library. The reference is an owned
standard-library oracle. The pinned upstream model runtime was read but not
executed; parity with an independently captured upstream runtime remains a
separate qualification gate. Whole-model numerical execution belongs to later
chapters.

## Try a change

Give the state leading whitespace and a structured nested value. Verify that
state whitespace survives while description whitespace is stripped. Request
27 choices and check the two-letter label transition. Then request 256 choices
and verify that the published prompt stays unchanged. Finally, compare encoding
a whole string with concatenating encodings at an arbitrary internal byte cut;
use complete-branch matching to recover a valid shared token prefix.

Next: attention without training. The tokenizer and layout built here provide
the exact IDs, positions and branch boundaries that the forward core consumes.
