# 83. A coding agent

> **Status: partly implemented.** The pinned checkpoint's inventory (from its
> headers alone) against params.json, the memory plan on this host, Mistral's
> function-calling format byte for byte and token for token, tool calls
> constrained by a grammar so every sampled call parses, and a fixed
> coding-task set with hidden tests and pass@k all run. Devstral itself does
> not: the measurements here come from a seeded stand-in policy. See
> [What this does not do yet](#what-this-does-not-do-yet).

Devstral Small 2 (`mistralai/Devstral-Small-2-24B-Instruct-2512`, Apache-2.0)
is Mistral's coding and tool-calling model. Its text decoder is the same
dense grouped-query decoder as [chapter 81](ch081-execute-ministral-4.md)'s
Ministral 3, only larger, and its tokenizer file is byte-for-byte the
Tekken file chapter 81 pins. Its weights are FP8 (E4M3) with one BF16 scale
per matrix.

A coding agent is a loop: the model reads a task, calls tools, reads their
results and submits an answer that hidden tests check. This chapter builds
every part of that loop that does not need the model's weights: it checks the
checkpoint, plans its memory, writes and reads the exact text the model sees,
compiles the tools into a grammar so the model can only write calls that
parse, and measures a fixed task set with pass@k.

```shell
coax/bin/coax run psionic/examples/devstral.coax
coax/bin/coax run psionic/examples/devstral-refuse.coax
coax/bin/coax test psionic/lib/devstral.coax
python3 -I psionic/tests/devstral_manifest.py --fetch DIR
python3 -I psionic/tests/devstral_manifest.py DIR
```

The [library](../lib/devstral.coax), its
[check oracle](../tests/devstral-oracle.coax), the
[example](../examples/devstral.coax), the
[refusal example](../examples/devstral-refuse.coax) and the
[manifest tool](../tests/devstral_manifest.py) are the implementation. It
reuses chapter 81's shape rules, inventory names and Tekken encoder, chapter
53's grammar compiler and guided sampler, the Coder book's
[chapter 4](../../coder/book/ch004-tools-as-typed-capabilities.md) grammar
builder and stand-in noise, chapter 33's residency planner, and chapter
212's pass@k estimator and Wilson interval.

## The pinned checkpoint, read by its headers

The checkpoint is pinned at revision
`55c5b41e98c2dbd21b0c8afffc540dcfc9eb5128`. Its consolidated form is two
safetensors shards, 25,793,035,888 bytes. That is more than this chapter
downloads, so the manifest tool fetches `params.json`, `config.json`, the
index, `chat_template.jinja` and `tekken.json`, and reads each shard's header
with an HTTP range request. It checks every file against its sha256 pin,
checks that each header's tensors tile its shard exactly to the pinned size,
and writes one line per text-decoder tensor into
[a fixture](../fixtures/devstral-manifest.coax). The 222 image-path tensors
are counted, not listed.

The shape comes from `params.json`: 40 layers of width 5120, 32 query heads
and 8 key-value heads of 128, a SwiGLU block of 32768 and 131,072 vocabulary
entries. Unlike Ministral 3 3B, the readout `output.weight` is its own matrix.

<!-- source: psionic/lib/devstral.coax shape -->
```jai
// Devstral Small 2's text decoder in chapter 81's terms: 40 layers of
// width 5120, 32 query heads and 8 key-value heads of 128, a SwiGLU block
// of 32768 and Tekken's 131,072 entries. Unlike Ministral 3 3B, the readout
// (output.weight) is its own matrix.
DV_SMALL2 :: MsShape.{width=5120,layers=40,heads=32,kv_heads=8,head_dim=128,hidden=32768,vocab=131072};
// The params.json fields this chapter does not change: untied readout,
// rotary base, norm epsilon, context, and the FP8 format with one
// activation scale per tensor.
DV_FIXED_PARAMS :: "tied_embeddings false\nrope_theta 100000000.0\nnorm_eps 1e-05\nmax_position_embeddings 393216\nqformat_weight fp8_e4m3\nqscheme_act TENSOR\n";
DV_PARAM_NAMES :: [7]string.["dim","n_layers","head_dim","hidden_dim","n_heads","n_kv_heads","vocab_size"];
// The shape as params.json lines; cause is the first byte that differs
// from params, or -1 when they match.
dv_params :: (s:MsShape,params:string)->DvResult {
    if !ms_shape_valid(s) {return DvResult.{status=DvStatus.shape};}
    buf:[512]u8;n:=0;v:=[7]int.[s.width,s.layers,s.head_dim,s.hidden,s.heads,s.kv_heads,s.vocab];
    for x,i:DV_PARAM_NAMES {n=ms_put(buf[..],n,x);n=ms_put(buf[..],n," ");n=ms_put_int(buf[..],n,v[i]);n=ms_put(buf[..],n,"\n");}
    n=ms_put(buf[..],n,DV_FIXED_PARAMS);
    for i:0..<n {if i==params.count || buf[i]!=params[i] {return DvResult.{status=DvStatus.params,cause=i};}}
    if params.count!=n {return DvResult.{status=DvStatus.params,cause=n};}
    return DvResult.{written=n,cause=-1};
}
```

Each layer holds 23 tensors: seven matrices (wq, wk, wv, wo, w1, w2, w3), each
an FP8 weight with two BF16 scalars (the weight scale and the activation
scale), and two BF16 norm vectors. With the final norm, the readout and the
embedding, that is 3 + 40 × 23 = 923 tensors. By hand, one layer's matrices
hold

- attention: 2 × 4096 × 5120 + 2 × 1024 × 5120 = 52,428,800 values,
- the SwiGLU block: 3 × 32768 × 5120 = 503,316,480 values,

so 555,745,280 FP8 bytes. Forty layers make 22,229,811,200 bytes. The
embedding and readout add 2 × 131072 × 5120 × 2 = 2,684,354,560 BF16 bytes,
the norms 40 × 2 × 5120 × 2 + 5120 × 2 = 829,440 bytes and the scales
40 × 14 × 2 = 1,120 bytes: 24,914,996,320 bytes in all. The headers say the
same.

<!-- source: psionic/lib/devstral.coax inventory -->
```jai
// The text decoder's tensors. Each of a layer's seven matrices is three
// tensors: its FP8 (E4M3) weight and two BF16 scalars, the weight scale and
// the activation scale. The two norms are BF16 vectors. Then the final
// norm, the readout and the embedding, BF16. part j of a layer is matrix k
// of chapter 81's nine names and its kind: 0 activation scale, 1 weight
// scale, 2 weight, 3 norm; parts follow the names' sorted order.
DvTensor :: struct {name:string;layer:int;kind:int;dtype:string;rank:int;rows:int;cols:int;}
DV_PARTS :: 23;
DV_KIND_NAMES :: [4]string.[".qscale_act",".qscale_weight",".weight",".weight"];
dv_part :: (j:int)->[2]int {
    at:=0;
    for k:0..<9 {
        n:=ifx k==4 || k==8 then 1 else 3;
        if j<at+n {return [2]int.[k,ifx n==1 then 3 else j-at];}
        at+=n;
    }
    return [2]int.[-1,-1];
}
dv_expected_count :: (s:MsShape)->int {return 3+DV_PARTS*s.layers;}
dv_expected :: (s:MsShape,e:int)->DvTensor {
    if e==0 {return DvTensor.{name="norm.weight",layer=-1,dtype="BF16",rank=1,rows=s.width};}
    if e==1 {return DvTensor.{name="output.weight",layer=-1,dtype="BF16",rank=2,rows=s.vocab,cols=s.width};}
    if e==2 {return DvTensor.{name="tok_embeddings.weight",layer=-1,dtype="BF16",rank=2,rows=s.vocab,cols=s.width};}
    layer:=(e-3)/DV_PARTS;p:=dv_part((e-3)%DV_PARTS);
    m:=ms_expected(s,2+9*layer+p[0]);
    t:=DvTensor.{name=m.name[0..<m.name.count-7],layer=layer,kind=p[1],dtype="BF16",rank=m.rank,rows=m.rows,cols=m.cols};
    if p[1]<2 {t.rank=0;}
    if p[1]==2 {t.dtype="F8_E4M3";}
    return t;
}
// "layers.L.name DTYPE rows[,cols]", or "scalar" for a 0-dimensional tensor.
dv_line :: (t:DvTensor,out:[]u8)->int {
    n:=0;if t.layer>=0 {n=ms_put(out,n,"layers.");n=ms_put_int(out,n,t.layer);}
    n=ms_put(out,n,t.name);if t.layer>=0 {n=ms_put(out,n,DV_KIND_NAMES[t.kind]);}
    n=ms_put(out,n," ");n=ms_put(out,n,t.dtype);n=ms_put(out,n," ");
    if t.rank==0 {return ms_put(out,n,"scalar");}
    n=ms_put_int(out,n,t.rows);
    if t.rank==2 {n=ms_put(out,n,",");n=ms_put_int(out,n,t.cols);}
    return n;
}
dv_bytes :: (t:DvTensor)->int {
    if t.rank==0 {return 2;}
    size:=t.rows;if t.rank==2 {size*=t.cols;}
    return ifx t.kind==2 then size else 2*size;
}
// Which expected tensor a line names, by the text before its first space,
// or -1. The whole line must then equal that tensor's line.
DV_LAYERS_MAX :: 44;
dv_starts :: (s:string,prefix:string)->bool {return s.count>=prefix.count && s[0..<prefix.count]==prefix;}
dv_name :: (line:string)->int {k:=0;while k<line.count && line[k]!=' ' {k+=1;}return k;}
DV_GLOBAL_NAMES :: [3]string.["norm.weight","output.weight","tok_embeddings.weight"];
dv_index :: (s:MsShape,line:string)->int {
    k:=dv_name(line);name:=line[0..<k];
    for g,e:DV_GLOBAL_NAMES {if name==g {return e;}}
    if !dv_starts(name,"layers.") {return -1;}
    p:=7;layer:=0;while p<k && name[p]>='0' && name[p]<='9' {layer=layer*10+cast(int)(name[p]-'0');p+=1;}
    if layer>=s.layers {return -1;}
    for j:0..<DV_PARTS {
        part:=dv_part(j);base:=MS_LAYER_NAMES[part[0]];base=base[0..<base.count-7];kind:=DV_KIND_NAMES[part[1]];
        if k-p==base.count+kind.count && name[p..<p+base.count]==base && name[p+base.count..<k]==kind {return 3+layer*DV_PARTS+j;}
    }
    return -1;
}
// The manifest (one line a tensor, each ending in a newline) holds s's text
// decoder when every expected line is there exactly once and nothing else
// is. cause names the first expected tensor missing or repeated, or is
// -1 - k for line k that is no expected tensor. written counts parameters,
// bytes the stored bytes.
dv_inventory :: (s:MsShape,manifest:string)->DvResult {
    if !ms_shape_valid(s) || s.layers>DV_LAYERS_MAX {return DvResult.{status=DvStatus.shape};}
    seen:[3+DV_PARTS*DV_LAYERS_MAX]u8;buf:[96]u8;from:=0;line:=0;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        text:=manifest[from..<i];e:=dv_index(s,text);
        if e<0 {return DvResult.{status=DvStatus.checkpoint,cause=-1-line};}
        n:=dv_line(dv_expected(s,e),buf[..]);
        if text.count!=n || cast(string)buf[0..<n]!=text {return DvResult.{status=DvStatus.checkpoint,cause=-1-line};}
        if seen[e]==1 {return DvResult.{status=DvStatus.checkpoint,cause=e};}
        seen[e]=1;line+=1;from=i+1;
    }}
    r:=DvResult.{};
    for e:0..<dv_expected_count(s) {
        if seen[e]==0 {return DvResult.{status=DvStatus.checkpoint,cause=e};}
        t:=dv_expected(s,e);r.bytes+=dv_bytes(t);
        if t.rank==0 {r.written+=1;}else if t.rank==1 {r.written+=t.rows;}else {r.written+=t.rows*t.cols;}
    }
    return r;
}
```

## The memory plan on this host

This host has an RTX 4080 with 17,171,480,576 bytes and 134,828,441,600
bytes of RAM. Chapter 33's `rp_admit` decides. For a 4096-token request in
chunks of 256:

- the f32 keys and values take 4 × 40 × 4096 × 2 × 8 × 128 = 1,342,177,280
  bytes;
- scratch takes 4 × (256 × (2 × 5120 + 2 × 32768) + 32 × 4096 + 131072) =
  78,643,200 bytes.

Weights, state and scratch together are 26,335,816,800 bytes: the model is
resident in RAM. On the GPU it fits only by streaming. The vectors (830,560
bytes), the state, the scratch and two stream slots the size of the readout
(2 × 1,342,177,280 bytes) need 4,106,005,600 bytes, so 13,066,305,536 bytes of
weights stay resident and 11,848,690,784 bytes stream for every token.

<!-- source: psionic/lib/devstral.coax plan -->
```jai
// Chapter 33's accounting: weights as stored (FP8 matrices, BF16 vectors,
// scales, embedding and readout); f32 keys and values at every position
// of every layer; f32 scratch for a chunk: the residual and its normed
// copy, the widest intermediate (w1 and w3 together, or q k v), one score
// row a head, and the logits. The readout is the largest unit that streams.
DV_HOST_GPU :: 17171480576;
DV_HOST_RAM :: 134828441600;
dv_weights :: (s:MsShape)->RpWeights {
    total:=0;vectors:=0;
    for e:0..<dv_expected_count(s) {t:=dv_expected(s,e);total+=dv_bytes(t);if t.rank<2 {vectors+=dv_bytes(t);}}
    return RpWeights.{total=total,vectors=vectors,slot=2*s.vocab*s.width,embedding=2*s.vocab*s.width};
}
dv_state :: (s:MsShape,capacity:int)->int {return 4*s.layers*capacity*2*s.kv_heads*s.head_dim;}
dv_scratch :: (s:MsShape,chunk:int,capacity:int)->int {
    widest:=rp_max(2*s.hidden,(s.heads+2*s.kv_heads)*s.head_dim);
    return 4*(chunk*(2*s.width+widest)+s.heads*capacity+s.vocab);
}
dv_plan :: (s:MsShape,r:RpRequest,budget:int)->RpPlan {
    if !ms_shape_valid(s) {return RpPlan.{status=RpStatus.model};}
    if r.capacity<1 || r.capacity>393216 || r.chunk<1 || r.chunk>r.capacity {return RpPlan.{status=RpStatus.request};}
    return rp_admit(dv_weights(s),dv_state(s,r.capacity),dv_scratch(s,r.chunk,r.capacity),r.stream,budget);
}
```

## Mistral's function-calling format

The pinned `chat_template.jinja` decides the exact text. A conversation is
`<s>`, the system prompt in `[SYSTEM_PROMPT]...[/SYSTEM_PROMPT]`, the tools as
JSON in `[AVAILABLE_TOOLS]...[/AVAILABLE_TOOLS]` before the first user turn,
the user's turn in `[INST]...[/INST]`, then each assistant call as
`[TOOL_CALLS]name[ARGS]{...}</s>` and each tool's result in
`[TOOL_RESULTS]...[/TOOL_RESULTS]`. The template prints the tools and the
arguments with `tojson`, which under Hugging Face's renderer is
`json.dumps` with its default `", "` and `": "` separators and keys in the
order written.

This chapter's agent has two tools. `run(code, x)` runs a program on one
input and returns its value; `submit(code)` hands a program to the hidden
tests. The text that never comes from text is the twelve control tokens:
`<s>` 1, `</s>` 2, `[INST]` 3, `[/INST]` 4, `[AVAILABLE_TOOLS]` 5,
`[/AVAILABLE_TOOLS]` 6, `[TOOL_RESULTS]` 7, `[/TOOL_RESULTS]` 8,
`[TOOL_CALLS]` 9, `[SYSTEM_PROMPT]` 17, `[/SYSTEM_PROMPT]` 18 and `[ARGS]` 32,
read from `tekken.json`. The renderer refuses any text field that would spell
one, and the encoder maps each to its id and the text between them through
chapter 81's Tekken.

<!-- source: psionic/lib/devstral.coax format -->
```jai
// A tool: its name and description, and whether it takes the input x
// besides the program. A call: the tool, the program and x.
DvTool :: struct {name:string;description:string;x:bool;}
DvCall :: struct {tool:int;code:string;x:int;}
// One assistant turn: a call, then the tool's result text (empty: none).
DvTurn :: struct {call:DvCall;result:string;}
DV_TOOLS :: [2]DvTool.[
    DvTool.{name="run",description="Run a Stack program on one input.",x=true},
    DvTool.{name="submit",description="Submit a Stack program for the hidden tests.",x=false}];
// Control tokens: their text in the rendered conversation and their ids.
DV_CONTROLS :: [12]string.["<s>","</s>","[INST]","[/INST]","[AVAILABLE_TOOLS]","[/AVAILABLE_TOOLS]","[TOOL_RESULTS]","[/TOOL_RESULTS]","[TOOL_CALLS]","[SYSTEM_PROMPT]","[/SYSTEM_PROMPT]","[ARGS]"];
DV_CONTROL_IDS :: [12]int.[1,2,3,4,5,6,7,8,9,17,18,32];
DV_CODE_SCHEMA :: "{\"type\": \"string\", \"pattern\": \"^[x0-9*+-]{1,11}$\"}";
DV_TEXT_MAX :: 4096;
// The tools as the chat template's tojson prints them: json.dumps with
// ", " and ": " between items, keys in the order written.
dv_tools_json :: (tools:[]DvTool,out:[]u8)->int {
    n:=ms_put(out,0,"[");
    for t,i:tools {
        if i>0 {n=ms_put(out,n,", ");}
        n=ms_put(out,n,"{\"type\": \"function\", \"function\": {\"name\": \"");n=ms_put(out,n,t.name);
        n=ms_put(out,n,"\", \"description\": \"");n=ms_put(out,n,t.description);
        n=ms_put(out,n,"\", \"parameters\": {\"type\": \"object\", \"properties\": {\"code\": ");n=ms_put(out,n,DV_CODE_SCHEMA);
        if t.x {n=ms_put(out,n,", \"x\": {\"type\": \"integer\", \"minimum\": 0, \"maximum\": 999}}, \"required\": [\"code\", \"x\"]}}}");}
        else {n=ms_put(out,n,"}, \"required\": [\"code\"]}}}");}
    }
    return ms_put(out,n,"]");
}
// One call as the model writes it: [TOOL_CALLS]name[ARGS]{"code": "...", "x": N}.
dv_call :: (tools:[]DvTool,c:DvCall,out:[]u8,at:int)->int {
    n:=ms_put(out,at,"[TOOL_CALLS]");n=ms_put(out,n,tools[c.tool].name);n=ms_put(out,n,"[ARGS]{\"code\": \"");n=ms_put(out,n,c.code);
    if tools[c.tool].x {n=ms_put(out,n,"\", \"x\": ");n=ms_put_int(out,n,c.x);return ms_put(out,n,"}");}
    return ms_put(out,n,"\"}");
}
dv_control_at :: (text:string,at:int)->int {
    for c,k:DV_CONTROLS {if at+c.count<=text.count && cast(string)text[at..<at+c.count]==c {return k;}}
    return -1;
}
dv_plain :: (text:string)->bool {for i:0..<text.count {if dv_control_at(text,i)>=0 {return false;}}return true;}
// The conversation: <s>, the system prompt, the tools, the user's turn,
// then each call, closed by </s>, and its result. Text that spells a
// control token is refused (status control, cause the turn, -1 the system
// prompt, -2 the user's turn); the renderer never lets text become one.
dv_render :: (tools:[]DvTool,system:string,user:string,turns:[]DvTurn,out:[]u8)->DvResult {
    if !dv_plain(system) {return DvResult.{status=DvStatus.control,cause=-1};}
    if !dv_plain(user) {return DvResult.{status=DvStatus.control,cause=-2};}
    for t,i:turns {if !dv_plain(t.call.code) || !dv_plain(t.result) {return DvResult.{status=DvStatus.control,cause=i};}}
    if out.count<DV_TEXT_MAX {return DvResult.{status=DvStatus.capacity};}
    n:=ms_put(out,0,"<s>[SYSTEM_PROMPT]");n=ms_put(out,n,system);n=ms_put(out,n,"[/SYSTEM_PROMPT][AVAILABLE_TOOLS]");
    n=dv_tools_json(tools,out[n..<out.count])+n;n=ms_put(out,n,"[/AVAILABLE_TOOLS][INST]");n=ms_put(out,n,user);n=ms_put(out,n,"[/INST]");
    for t:turns {
        n=dv_call(tools,t.call,out,n);n=ms_put(out,n,"</s>");
        if t.result.count>0 {n=ms_put(out,n,"[TOOL_RESULTS]");n=ms_put(out,n,t.result);n=ms_put(out,n,"[/TOOL_RESULTS]");}
    }
    return DvResult.{written=n};
}
// Rendered text to ids: each control token is its id, and the text between
// them is chapter 81's Tekken encoding. cause is the failing byte.
dv_ids :: (t:MsTable,text:string,out:[]int)->DvResult {
    at:=0;n:=0;
    while at<text.count {
        k:=dv_control_at(text,at);
        if k>=0 {if n==out.count {return DvResult.{status=DvStatus.capacity,cause=at};}out[n]=DV_CONTROL_IDS[k];n+=1;at+=DV_CONTROLS[k].count;continue;}
        end:=at;while end<text.count && dv_control_at(text,end)<0 {end+=1;}
        r:=ms_encode(t,text[at..<end],out[n..<out.count]);
        if r.status!=MsStatus.ok {return DvResult.{status=DvStatus.tokens,cause=at+r.cause};}
        n+=r.written;at=end;
    }
    return DvResult.{written=n};
}
```

The check renders one conversation (a `run` call, its result `7`, then a
`submit` call) and compares its 960 bytes with the text the manifest tool
writes from `json.dumps`, and its 283 token ids with tiktoken's rule run in
Python over the full Tekken table.

## Stack, the language the agent writes

The tasks are small functions in Stack, a postfix language: `x` pushes the
input, a digit pushes itself, and `+`, `-` and `*` pop b, then a, and push
a + b, a - b or a × b. A program is 1 to 11 bytes, its stack never holds more
than four values, and it ends holding one. Trace `x2*1+` at x = 3:

| byte | stack after |
| --- | --- |
| `x` | 3 |
| `2` | 3 2 |
| `*` | 6 |
| `1` | 6 1 |
| `+` | 7 |

It ends with one value, 7, so f(3) = 7, which is 2x + 1. Because each value
needs one byte and each operator removes one value, a valid program has an
odd length, so the limit is 11 bytes: at most six values and five operators.

<!-- source: psionic/lib/devstral.coax stack -->
```jai
// Run a program on x. Programs outside the grammar are refused with the
// byte: a byte that is no instruction, an operator with fewer than two
// values, a fifth value, or an end with other than one value.
DvStop :: enum u8 {ok;byte;underflow;overflow;left;}
DvValue :: struct {stop:DvStop;value:int;at:int=-1;}
dv_eval :: (code:string,x:int)->DvValue {
    st:[DV_DEPTH]int;d:=0;
    for c,i:code {
        if c=='x' || (c>='0' && c<='9') {
            if d==DV_DEPTH {return DvValue.{stop=DvStop.overflow,at=i};}
            st[d]=ifx c=='x' then x else cast(int)(c-'0');d+=1;continue;
        }
        if c!='+' && c!='-' && c!='*' {return DvValue.{stop=DvStop.byte,at=i};}
        if d<2 {return DvValue.{stop=DvStop.underflow,at=i};}
        a:=st[d-2];b:=st[d-1];d-=1;
        st[d-1]=ifx c=='+' then a+b else ifx c=='-' then a-b else a*b;
    }
    if d!=1 {return DvValue.{stop=DvStop.left,at=code.count};}
    return DvValue.{value=st[0]};
}
// A task: what f is, the example input shown to the agent, four hidden
// inputs and their outputs, and one reference program.
DvTask :: struct {spec:string;shown:int;inputs:[4]int;outputs:[4]int;reference:string;}
DV_TASKS :: [8]DvTask.[
    DvTask.{spec="x + 3",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[3,8,43,1002],reference="x3+"},
    DvTask.{spec="2x",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[0,10,80,1998],reference="x2*"},
    DvTask.{spec="x squared",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[0,25,1600,998001],reference="xx*"},
    DvTask.{spec="2x + 1",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[1,11,81,1999],reference="x2*1+"},
    DvTask.{spec="x squared minus x",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[0,20,1560,997002],reference="xx*x-"},
    DvTask.{spec="x cubed",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[0,125,64000,997002999],reference="xx*x*"},
    DvTask.{spec="3x - 5",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[-5,10,115,2992],reference="x3*5-"},
    DvTask.{spec="(x + 1)(x - 1)",shown=3,inputs=[4]int.[0,5,40,999],outputs=[4]int.[-1,24,1599,998000],reference="x1+x1-*"}];
// The user's turn for a task, with f at the shown input.
dv_task_text :: (t:DvTask,out:[]u8)->int {
    n:=ms_put(out,0,"Write f(x) = ");n=ms_put(out,n,t.spec);
    n=ms_put(out,n," in Stack. x pushes the input, a digit pushes itself, and + - * pop two values and push the result. Example: f(");
    n=ms_put_int(out,n,t.shown);n=ms_put(out,n,") = ");n=ms_put_int(out,n,dv_eval(t.reference,t.shown).value);return ms_put(out,n,".");
}
// The hidden tests: the program passes when it runs and matches every output.
dv_passes :: (t:DvTask,code:string)->bool {
    for x,i:t.inputs {v:=dv_eval(code,x);if v.stop!=DvStop.ok || v.value!=t.outputs[i] {return false;}}
    return true;
}
```

The task set is fixed: eight functions, each shown to the agent at x = 3 and
checked on four hidden inputs, 0, 5, 40 and 999. A program passes when it
runs and matches all four outputs, so `xx+` passes "2x" just as the
reference `x2*` does.

## Calls under a grammar

The grammar for the offered tools is chapter 53's byte automaton, built with
chapter 4's builder. Inside the program it tracks two numbers: bytes so far
and stack depth. A push is allowed below depth 4, an operator at depth 2 or
more, and the closing quote only at depth 1. For `x2*1+` the walk is (bytes,
depth) = (0, 0), (1, 1), (2, 2), (3, 1), (4, 2), (5, 1), and at (5, 1) the
quote is allowed. For `x+`, the walk reaches (1, 1) and has no operator edge:
the call is refused at that byte. Every edge goes to a newer state, so the
language is finite and chapter 53's liveness sweep runs in one pass; a state
from which no complete call can be reached is never offered.

<!-- source: psionic/lib/devstral.coax grammar -->
```jai
// Stack: x pushes the input, a digit pushes itself, and + - * pop b then a
// and push a+b, a-b, a*b. The grammar tracks (bytes so far, stack depth):
// a push needs depth below 4, an operator depth 2 or more, and the program
// may end at depth 1. Every edge goes to a newer state, so the language is
// finite and chapter 53's liveness sweep finishes in one pass.
// Values and operators alternate in count, so a program's length is odd:
// six values and five operators, 11 bytes, is the longest.
DV_CODE_MAX :: 11;
DV_DEPTH :: 4;
DV_STATES :: 512;
DV_EDGES :: 1024;
dv_code :: (b:[]HtBuild,w:HtWork,s:int,ends:[]int)->int {
    cur:[DV_DEPTH+1]int;cur[0]=s;have:=[DV_DEPTH+1]bool.[true,false,false,false,false];n:=0;
    for p:0..<DV_CODE_MAX {
        next:[DV_DEPTH+1]int;made:[DV_DEPTH+1]bool;
        for d:0..<DV_DEPTH+1 {if have[d] {
            if d<DV_DEPTH {
                if !made[d+1] {next[d+1]=ht_state(b,w);made[d+1]=true;}
                ht_edge(b,w,cur[d],'x','x',next[d+1]);ht_edge(b,w,cur[d],'0','9',next[d+1]);
            }
            if d>=2 {
                if !made[d-1] {next[d-1]=ht_state(b,w);made[d-1]=true;}
                ht_edge(b,w,cur[d],'*','+',next[d-1]);ht_edge(b,w,cur[d],'-','-',next[d-1]);
            }
        }}
        if made[1] {ends[n]=next[1];n+=1;}
        cur=next;have=made;
    }
    return n;
}
// The offered tools' calls. State owners name the field: -1 the frame, 0
// the program, 1 x.
dv_grammar :: (tools:[]DvTool,offered:[]int,w:HtWork)->HtGrammar {
    b:[1]HtBuild;start:=ht_state(b[..],w);head:=ht_bytes(b[..],w,start,"[TOOL_CALLS]");
    closing:[8]int;nc:=0;
    for t:offered {
        b[0].field= -1;s:=ht_bytes(b[..],w,head,tools[t].name);s=ht_bytes(b[..],w,s,"[ARGS]{\"code\": \"");
        b[0].field=0;ends:[DV_CODE_MAX]int;ne:=dv_code(b[..],w,s,ends[..]);
        b[0].field= -1;q:=ht_join(b[..],w,ends[0..<ne],'"');
        if tools[t].x {
            b[0].field=1;s=ht_bytes(b[..],w,q,", \"x\": ");
            values:[HT_DIGITS_MAX+1]int;nv:=ht_value(b[..],w,s,ht_int(3),values[..]);
            for v:values[0..<nv] {closing[nc]=v;nc+=1;}
        } else {closing[nc]=q;nc+=1;}
    }
    b[0].field= -1;accept:=ht_join(b[..],w,closing[0..<nc],'}');
    if b[0].states>w.owner.count || b[0].edges>w.edges.count {return HtGrammar.{why=HtWhy.capacity};}
    // The Coder book's chapter 4 renumbering: state k becomes last-k, so
    // edges point down. Owners keep the build's numbering: state s's owner
    // is owner[last-s].
    last:=b[0].states-1;
    for i:0..<b[0].edges {x:=w.edges[i];w.edges[i]=NxEdge.{from=last-x.from,low=x.low,high=x.high,to=last-x.to};}
    final:=[1]int.[last-accept];
    c:=nx_compile(b[0].states,last,w.edges[0..<b[0].edges],final[..],w.next,w.accept,w.live,false);
    if c.status!=NxStatus.ok {return HtGrammar.{why=HtWhy.capacity};}
    return HtGrammar.{grammar=c.grammar};
}
```

Admission walks a call's text through the same table. A refusal names the
byte and the field (-1 the frame, 0 the program, 1 x), and an admitted call
is decoded into its tool, program and input.

<!-- source: psionic/lib/devstral.coax admit -->
```jai
// A call's text, admitted byte by byte through the grammar of the offered
// tools. A refusal names the byte and the field it falls in; an admitted
// call is decoded into its tool, program and x.
DvWhy :: enum u8 {none;syntax;incomplete;capacity;}
DvAdmit :: struct {why:DvWhy;at:int=-1;field:int=-1;call:DvCall;}
dv_admit :: (tools:[]DvTool,offered:[]int,text:string,w:HtWork)->DvAdmit {
    g:=dv_grammar(tools,offered,w);if g.why!=HtWhy.none {return DvAdmit.{why=DvWhy.capacity};}
    return dv_read(tools,g.grammar,text,w);
}
// The same, through a grammar already compiled into w.
dv_read :: (tools:[]DvTool,g:NxGrammar,text:string,w:HtWork)->DvAdmit {
    s:=g.start;
    for x,i:text {n:=w.next[s*256+cast(int)x];if n<0 {return DvAdmit.{why=DvWhy.syntax,at=i,field=w.owner[g.start-s]};}s=n;}
    if w.accept[s]!=1 {return DvAdmit.{why=DvWhy.incomplete,at=text.count,field=w.owner[g.start-s]};}
    // [TOOL_CALLS] is 12 bytes; the name runs to [ARGS]; the program starts
    // 10 bytes after it and runs to its closing quote.
    head:=DV_CONTROLS[8].count;p:=head;while text[p]!='[' {p+=1;}
    c:=DvCall.{};for t,i:tools {if t.name==text[head..<p] {c.tool=i;}}
    from:=p+16;e:=from;while text[e]!='"' {e+=1;}
    c.code=text[from..<e];
    if tools[c.tool].x {for k:e+8..<text.count-1 {c.x=c.x*10+cast(int)(text[k]-'0');}}
    return DvAdmit.{call=c};
}
```

The checks compare the grammar with two other definitions. Over every string
of 1 to 5 bytes from `x`, `9`, `*`, `+`, `-` and `a` (9,330 strings), the
grammar accepts a `submit` call exactly when the
[oracle](../tests/devstral-oracle.coax), written from the definition by
counting pushes and operators, says the program is valid, and exactly when
Stack runs it without a refusal. By hand, the valid ones are 2 of length 1,
2 × 2 × 3 = 12 of length 3 and 2 × 72 = 144 of length 5: 158.

## A stand-in policy and the measurement

The 24-billion-parameter model does not run here, so the measurement uses a
seeded stand-in. Its logits are chapter 4's counter noise in [0, 8), plus a
fixed skill on every piece that continues its script: a `run` call on the
shown input, then a `submit` call with the task's reference program. Its
vocabulary is the frame pieces, both names, the program bytes, three pieces
no call contains (`a`, a space and `Q`) and the end piece `</s>`. With the
grammar, chapter 53's guided step masks every piece that cannot lead to a
complete call; without it, the same logits are sampled unmasked. An attempt
makes up to three calls and passes when its submitted program passes the
hidden tests.

<!-- source: psionic/lib/devstral.coax policy -->
```jai
// The stand-in model's vocabulary: the call's frame, both names, the
// program's bytes, three pieces no call contains, and the end piece last
// (</s>). Its logits for draw i of call k: the Coder book's chapter 4 noise in
// [0, 8), plus skill on each piece that continues its script, the
// reference call for that step: run on the shown input first, then submit.
DV_PIECES :: [28]string.["[TOOL_CALLS]","run","submit","[ARGS]","{\"code\": \"","\"","\", \"x\": ",", \"x\": ","}","\"}",
    "x","0","1","2","3","4","5","6","7","8","9","*","+","-","a"," ","Q",""];
DV_END :: 27;
DV_CALLS :: 3;
DV_LIMIT :: 32;
DvPolicy :: struct {seed:u64;skill:f64;constrained:bool;}
// One attempt: up to three calls. It passes when a submitted program
// passes the hidden tests. parsed counts calls the grammar admits.
DvAttempt :: struct {passed:bool;submitted:bool;calls:int;parsed:int;pieces:int;}
dv_script :: (t:DvTask,k:int,out:[]u8)->int {
    c:=DvCall.{tool=ifx k==0 then 0 else 1,code=t.reference,x=t.shown};tools:=DV_TOOLS;
    return dv_call(tools[..],c,out,0);
}
dv_logits :: (p:DvPolicy,script:string,made:string,k:int,i:int,logits:[]f32) {
    ht_noise(p.seed,k,i,logits);
    pieces:=DV_PIECES;
    for x,j:pieces {
        follows:=ifx j==DV_END then made.count==script.count else made.count+x.count<=script.count && cast(string)script[made.count..<made.count+x.count]==x;
        if follows {logits[j]+=f32(p.skill);}
    }
}
dv_attempt :: (p:DvPolicy,t:DvTask,w:HtWork,sm:HtSampler)->DvAttempt {
    tools:=DV_TOOLS;pieces:=DV_PIECES;offered:=[2]int.[0,1];a:=DvAttempt.{};
    g:=dv_grammar(tools[..],offered[..],w);all:[28]u8;for j:0..<28 {all[j]=1;}
    sbuf:[160]u8;
    for k:0..<DV_CALLS {
        script:=cast(string)sbuf[0..<dv_script(t,k,sbuf[..])];
        run:=NxRun.{limit=DV_LIMIT,random=CrSeed.{seed=cr_bits(p.seed,cast(u64)k)}};state:=g.grammar.start;n:=0;
        while run.stop==NxStop.running {
            dv_logits(p,script,cast(string)sm.text[0..<n],k,run.emitted,sm.logits);
            token:int;
            if p.constrained {
                d:=nx_guided(sm.logits,g.grammar,state,pieces[..],DV_END,NxOptions.{},run,false,sm.mask,sm.out,sm.work,false);
                token=d.token;run=d.run;state=d.state;
            } else {
                stops:=[1]int.[DV_END];e:=nx_step(sm.logits,all[..],NxOptions.{},run,stops[..],false,sm.out,sm.work,false);
                token=e.token;run=e.run;
            }
            for x:pieces[token] {sm.text[n]=x;n+=1;}
        }
        a.calls+=1;a.pieces+=run.emitted;
        c:=dv_read(tools[..],g.grammar,cast(string)sm.text[0..<n],w);
        if c.why!=DvWhy.none {return a;}
        a.parsed+=1;
        if c.call.tool==1 {a.submitted=true;a.passed=dv_passes(t,c.call.code);return a;}
    }
    return a;
}
```

Each task gets eight attempts. pass@k is the Coder book's chapter 22 unbiased estimator,
averaged over the eight tasks: for a task with c of n = 8 attempts right,
pass@k = 1 - C(n - c, k) / C(n, k). For the third task, with c = 2,
pass@4 = 1 - C(6, 4) / C(8, 4) = 1 - 15/70 = 0.786.

<!-- source: psionic/lib/devstral.coax measure -->
```jai
// n attempts at each task, attempt q seeded seed + 64 r + q. right[r] counts
// task r's passes; pass@k is the Coder book's chapter 22 estimator averaged over
// the tasks, and the Wilson interval is over every attempt.
DvTally :: struct {status:DvStatus;attempts:int;passed:int;submitted:int;calls:int;parsed:int;pieces:int;right:[8]int;}
dv_tally :: (p:DvPolicy,n:int,w:HtWork,sm:HtSampler)->DvTally {
    if n<1 {return DvTally.{status=DvStatus.shape};}
    tasks:=DV_TASKS;y:=DvTally.{};
    for t,r:tasks {for q:0..<n {
        a:=dv_attempt(DvPolicy.{seed=p.seed+cast(u64)(64*r+q),skill=p.skill,constrained=p.constrained},t,w,sm);
        y.attempts+=1;y.calls+=a.calls;y.parsed+=a.parsed;y.pieces+=a.pieces;
        if a.submitted {y.submitted+=1;}
        if a.passed {y.passed+=1;y.right[r]+=1;}
    }}
    return y;
}
dv_pass_at :: (y:DvTally,n:int,k:int)->CdValue {
    s:=0.0;for c:y.right {v:=cd_pass_at(n,c,k);if v.status!=CdStatus.ok {return v;}s+=v.value;}
    return CdValue.{value=s/8};
}
```

## The program

<!-- source: psionic/examples/devstral.coax example -->
```jai
// The pinned checkpoint against params.json, its memory plan on this host,
// one conversation in Mistral's format, and the task set under the seeded
// stand-in, with and without the grammar.
main :: () {
    s:=DV_SMALL2;
    c:=dv_params(s,DV_PINNED_PARAMS);i:=dv_inventory(s,DV_PINNED_MANIFEST);
    print("params.json: %, % bytes match the shape\n",c.status,c.written);
    print("inventory: %, % text tensors, % parameters, % bytes as stored\n",i.status,dv_expected_count(s),i.written,i.bytes);
    r:=RpRequest.{capacity=4096,chunk=256,stream=true};
    gpu:=dv_plan(s,r,DV_HOST_GPU);ram:=dv_plan(s,r,DV_HOST_RAM);
    print("plan, 4096 tokens: RAM % (% bytes), GPU % (% resident, % streamed)\n",rp_mode_name(ram.mode),ram.weights+ram.state+ram.scratch,rp_mode_name(gpu.mode),gpu.resident,gpu.streamed);

    tools:=DV_TOOLS;tasks:=DV_TASKS;ub:[256]u8;buf:[DV_TEXT_MAX]u8;ids:[512]int;
    tb:=DV_TEKKEN_BYTES;tr:=DV_TEKKEN_RANKS;t:=MsTable.{bytes=tb[..],ranks=tr[..]};
    turns:=[2]DvTurn.[DvTurn.{call=DvCall.{tool=0,code="x2*1+",x=3},result="7"},DvTurn.{call=DvCall.{tool=1,code="x2*1+"}}];
    v:=dv_render(tools[..],DV_SYSTEM,cast(string)ub[0..<dv_task_text(tasks[3],ub[..])],turns[..],buf[..]);
    k:=dv_ids(t,cast(string)buf[0..<v.written],ids[..]);
    print("conversation: % bytes, % tokens, ends %\n",v.written,k.written,cast(string)buf[v.written-45..<v.written]);

    box:[1]DvBox;w:=dv_work(box[..]);sm:=dv_sampler(box[..]);
    for constrained:[2]bool.[true,false] {
        y:=dv_tally(DvPolicy.{seed=83,skill=8,constrained=constrained},8,w,sm);
        print("% : calls %, parsed %, submitted %, passed % of %, pieces %\n",ifx constrained then "constrained  " else "unconstrained",y.calls,y.parsed,y.submitted,y.passed,y.attempts,y.pieces);
        print("  passes per task:");for x:y.right {print(" %",x);}print("\n");
        print("  pass@1 %, pass@2 %, pass@4 %, pass@8 %\n",dv_pass_at(y,8,1).value,dv_pass_at(y,8,2).value,dv_pass_at(y,8,4).value,dv_pass_at(y,8,8).value);
        wi:=cd_wilson(y.passed,y.attempts);print("  Wilson 95%% interval for pass@1: [%, %]\n",wi.low,wi.high);
    }
}
```

```text
params.json: ok, 227 bytes match the shape
inventory: ok, 923 text tensors, 23572403760 parameters, 24914996320 bytes as stored
plan, 4096 tokens: RAM resident (26335816800 bytes), GPU streamed (13066305536 resident, 11848690784 streamed)
conversation: 960 bytes, 283 tokens, ends [TOOL_CALLS]submit[ARGS]{"code": "x2*1+"}</s>
constrained   : calls 128, parsed 128, submitted 64, passed 30 of 64, pieces 1794
  passes per task: 4 6 2 4 2 4 5 3
  pass@1 0.46875, pass@2 0.7232142857142856, pass@4 0.9321428571428572, pass@8 1.0
  Wilson 95% interval for pass@1: [0.3517578764404486, 0.5892812499952487]
unconstrained : calls 69, parsed 6, submitted 1, passed 1 of 64, pieces 1335
  passes per task: 0 1 0 0 0 0 0 0
  pass@1 0.015625, pass@2 0.03125, pass@4 0.0625, pass@8 0.125
  Wilson 95% interval for pass@1: [0.0027634707504014047, 0.08334298900290629]
```

With skill 8, every one of the 128 constrained calls parses, and 30 of 64
attempts pass: pass@1 is 0.469, with a Wilson 95% interval of 0.352 to
0.589, and pass@4 is 0.932. Without the grammar the same policy writes 69
calls of which 6 parse, and 1 attempt of 64 passes. The numbers measure the
stand-in, not Devstral; they show the measurement and the grammar's effect,
and they are fixed by the seed, so the transcript above is exact.

## A call that does not parse

A `submit` call whose program pops two values from a stack of one:

<!-- source: psionic/examples/devstral-refuse.coax example -->
```jai
// A call whose program pops two values when the stack holds one. The
// grammar refuses it at that byte, inside the program field, and nothing
// runs; Stack itself would have stopped at the same byte.
main :: () {
    tools:=DV_TOOLS;offered:=[2]int.[0,1];box:[1]DvBox;w:=dv_work(box[..]);
    text:="[TOOL_CALLS]submit[ARGS]{\"code\": \"x+\"}";
    a:=dv_admit(tools[..],offered[..],text,w);
    print("refused: %, byte % ('%'), field %\n",a.why,a.at,cast(string)text[a.at..<a.at+1],a.field);
    v:=dv_eval("x+",5);print("Stack: %, byte %\n",v.stop,v.at);
}
```

```text
refused: syntax, byte 35 ('+'), field 0
Stack: underflow, byte 1
```

The grammar refuses at byte 35, the `+`, inside the program field (0), before
anything runs. Stack, run on the same program, stops at the same byte of the
program.

## What this does not do yet

- Devstral does not run. Its FP8 weights (24.9 GB of text decoder) are not
  downloaded, and no token or logit is compared with a reference. Running it
  needs chapter 82's FP8 decoding on chapter 81's decoder at width 5120, YaRN
  with factor 48 over theta 10^8, and a separate readout.
- The pass rates, tokens and times on the task set are the stand-in's. Real
  Devstral numbers on a named host remain.
- Long-context requests through chapter 41's tiled attention and chapter
  176's prefix cache are not wired in.
- The format follows the pinned chat template under Hugging Face's `tojson`;
  mistral-common's own encoder is not compared. Text is ASCII only, as in
  chapter 81.
- `run` and `submit` evaluate Stack directly, with its own bounds; they do
  not go through the Coder book's [chapter 5](../../coder/book/ch005-a-sandbox-and-approvals.md)
  sandbox and approvals.
- The task set is eight toy functions, not a real coding benchmark.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
- The pinned `params.json`, `chat_template.jinja` and `tekken.json` at
  `mistralai/Devstral-Small-2-24B-Instruct-2512`, revision
  `55c5b41e98c2dbd21b0c8afffc540dcfc9eb5128`.
