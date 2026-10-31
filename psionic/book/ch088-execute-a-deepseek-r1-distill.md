# 88. Execute a DeepSeek R1 distill

DeepSeek’s first model in this book is a dense decoder, not V4.1 Flash.
We execute the original BF16 weights of `deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B`
at revision `ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562`. The checkpoint
uses Qwen2: 28 layers, width 1,536, intermediate width 8,960, twelve query
heads and two key/value heads, each 128 wide, and 151,936 vocabulary rows.
The published configuration disables sliding-window attention. The
[original configuration](https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/blob/ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562/config.json)
is the architecture contract; the
[model card](https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/tree/ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562)
provides the original artifacts and MIT terms.

This chapter provides a bounded CPU reference, greedy generation, and typed
reasoning/final intervals. It establishes no GPU throughput, serving,
statistical decision quality, tool execution, Qwen3.5 hybrid compatibility,
or V2/V4 architecture support. Chapter 89 extends the recipe and streaming
boundary separately. The supported numerical profile is
`deepseek-r1-qwen2-cpu-wide-v1`: original BF16 values, compensated binary64
products and reductions, float32 stored activations, epsilon `1e-6`, and
no FMA or reassociation. This is our CPU reference arithmetic, not a claim
of bitwise equality with an upstream BF16 runtime.

## Start with a complete small model

Run the owned two-layer fixture without downloading a model:

```shell
coax/bin/coax run psionic/examples/execute-deepseek.coax
coax/bin/coax run psionic/examples/execute-deepseek.coax --release
psionic/tests/check_deepseek.sh
psionic/check.sh --only deepseek --jobs 8
```

The [example](../examples/execute-deepseek.coax) uses shared library code,
not an implementation hidden in a transcript. Its tensors are genuine BF16
bytes in an owned Safetensors file. A second fixture has three layers and
two KV heads, so multiply/add indexing mistakes cannot hide behind a
two-by-two layout. An independent Python scalar program
recomputes the entire prefix without reading Psionic’s cache and supplies
all layer residuals and vocabulary logits for four consecutive positions.
Native checks require exact cached/replay equality, untouched unused tails,
and complete preservation of public outputs and every cache layer when the
last vocabulary projection fails. Plain, ASan and UBSan builds run in debug
and release. Python is fixture/oracle tooling only.

<!-- source: psionic/examples/execute-deepseek.coax example -->
```jai
main :: () {
    data:=DS_TINY;blocks:[2]DsBlock;m:=ds_tiny_model(data[..],blocks[..]);p:=ds_plan(m.shape);
    cache:[64]f32;floats:[148]f32;scores:[8]f64;logits:[9]f32;trace:[16]f32;
    for t:0..<4 {
        r:=ds_step(m,DS_TOKENS[t],t,cache[..],DsOutput.{logits=logits[..],trace=trace[..]},DsWork.{floats=floats[..],scores=scores[..]},false);
        #assert r.status==DsStatus.ok;
    }
    next:=ds_greedy(logits[..]);#assert next.status==DsStatus.ok;
    print("Qwen2: 2 layers, 4 tokens, full vocabulary\n");print("next token %\n",next.next);
    print("tokenizer BOS %, config BOS %\n",DS_BOS,151643);
    ids:=[5]int.[DS_THINK,1,DS_END_THINK,2,DS_EOS];parts:=ds_parts(ids[..]);
    #assert parts.complete && parts.reasoning_end==2 && parts.final_begin==3;
    print("reasoning and final have separate intervals\n");
}
```

## Authenticate once, retain original rows

The original `model.safetensors` is 3,554,214,621 bytes, with SHA-256
`58858233513d76b8703e72eed6ce16807b523328188e13329257fb9594462945`.
Its 339 tensors include separate embedding and LM-head matrices. The
[manifest](../fixtures/deepseek/original/manifest.json) pins the config,
tokenizer configuration, tokenizer bytes, original weights and derived
Unicode/BPE table. Generation settings are documented data; this chapter
explicitly selects greedy generation rather than inheriting sampling defaults.

`ds_bind_original` supplies the reviewed original profile to
[`ds_bind_profile`](../lib/deepseek/weights.coax). Complete SHA-256 checks
bind the exact original header to its reviewed tensor intervals. A typed
profile additionally checks complete payload coverage, unique names,
disjoint intervals and every expected BF16 shape before publishing any
layer. Caller-provided profiles require trustworthy pins and a correctly
chosen architecture; the generic binder does not authenticate their source.
Our small authenticated fixture exercises the same binder’s success and
refusal paths. Authentication is per session, not per token. Matrices stay
as borrowed BF16 rows; the engine never expands the complete model to F32.

## Qwen2 is its own architecture

Qwen3.5’s offset RMS and DeltaNet blocks cannot stand in for this decoder.
Each Qwen2 layer applies learned-scale RMS, biased Q/K/V projections,
split-half rotary positions over the full head at theta 10,000, causal
grouped-query attention, an unbiased output projection, a residual, another
learned RMS, and a gate/up SiLU product with a down projection and residual.
No Q/K normalization, attention gate, sink or expert router is present.

[`ds_step`](../lib/deepseek/model.coax) stages all activations, new KV rows,
intermediate diagnostics and the complete vocabulary readout. Only success
publishes the new cache entries and outputs. Position-major KV is allocated
separately per layer. A query reads positions zero through its own position;
the caller must supply sequential history for the same immutable model.
Rotary coefficients are computed in Coax, with explicit float32 storage
before rotation. Their numerical evidence belongs to the tested host/libm. Matrix products reuse chapter 11’s original BF16
projection; a vocabulary larger than its row bound is projected in chunks.

The pure [memory plan](../lib/deepseek/plan.coax) checks dimensions before
multiplication. Bounds are 1–28 layers, width derived from at most twelve
heads of even width 2–128, 1–2 KV heads dividing the query heads, intermediate
width 1–8,960, vocabulary 1–151,936, and context capacity 1–16,384. The original
profile fixes every dimension except capacity. At capacity 128 its KV cache
is 7,340,032 bytes. Planned workspace is distinct from mapped weight bytes,
public logits/diagnostics and operating-system resident memory. Admission
does not reserve physical RAM or establish a peak-RSS limit.

<!-- source: psionic/lib/deepseek/plan.coax plan -->
```jai
ds_shape :: (s:DsShape)->bool {
    if s.layers<1 || s.layers>28 || s.hidden<1 || s.hidden>8960 || s.heads<1 || s.heads>12 || s.kv_heads<1 || s.kv_heads>2 || s.head<2 || s.head/2>64 || s.head%2!=0 || s.vocab<1 || s.vocab>151936 || s.capacity<1 || s.capacity>16384 {return false;}
    return s.width==s.heads*s.head && s.heads%s.kv_heads==0;
}
ds_plan :: (s:DsShape)->DsPlan #export_c {
    if !ds_shape(s) {return DsPlan.{status=DsStatus.shape};}
    d:=s.width;h:=s.hidden;k:=s.kv_heads*s.head;mx:=s.vocab+h+d;
    sizes:=[17]int.[d,d,d,k,k,d,d,h,h,h,mx,mx,2*s.layers*k,2*s.layers*d,s.vocab,s.head,d];p:DsPlan;
    for i:0..<17 {p.begin[i+1]=p.begin[i]+sizes[i];}
    p.floats=p.begin[17];p.scores=s.capacity;p.cache=2*s.layers*s.capacity*k;return p;
}
```

## The tokenizer chooses BOS

The [pinned tokenizer configuration](../fixtures/deepseek/original/tokenizer_config.json)
uses BOS `151646`, spelled `<｜begin▁of▁sentence｜>`. The model config says
`151643`, which is the tokenizer’s EOS. Using that config value as BOS would
silently change every request. Our pinned text profile inserts tokenizer BOS
once, and EOS only after completed assistant history.

The original tokenizer has its own NFC-normalized byte-BPE vocabulary and
merge ranks. Its split expression has letter runs, without chapter 9’s
additional combining-mark class. We reuse chapter 9’s Unicode-17 table,
normalizer and ranked merge primitive, with a separate pinned table and split
function. A regex oracle and a heap-based merge oracle author independent
vectors, including spaces, contractions, digits, accents, Chinese, Japanese
and Korean. The oracle’s Python Unicode database is 15.1 for these selected,
long-established characters; it is not exhaustive Unicode-17 runtime parity.

The bounded conversation subset is an optional first system message, followed
by alternating user and assistant messages, ending in a user message. Other
layouts refuse rather than reinterpret the template. At most 64 messages,
65,536 combined supplied bytes and 16,384 final tokens are admitted. Each
BPE piece is at most 1,024 bytes, with a sum-of-squares work budget of
16,777,216. Ordinary strings never introduce added/control tokens, even if
they spell a marker. Markers are inserted by typed boundaries. The original
Jinja template independently checks the supported ordinary history path;
marker isolation is an intentional boundary-safety refinement for literal
user text, not byte-for-byte template encoding on malicious marker strings.

Prior assistant history keeps only text after its last `</think>`, exactly
as this template specifies. The generation prefix is assistant, `<think>`,
newline. The original “Hi” request is exactly:

`151646 151644 13048 151645 151648 198`

`ds_encode`, `ds_chat` and `ds_decode` stage public output; disposable
workspaces may change on refusal. The caller admits the packed table once,
then retains it immutable. Layout parsing is not identity admission. The tokenizer has 151,665 byte
entries beside the model’s 151,936 padded output rows. Decoding a model ID
with no tokenizer entry is a typed range refusal; no bytes are invented.

## Keep reasoning separate from the final answer

`ds_parts` returns token intervals. An unfinished reasoning segment has no
final interval. The first `</think>` opens the final segment; EOS must be
last and follow that boundary. Duplicate reasoning markers, conversation
markers, out-of-range IDs and trailing tokens after EOS refuse. A length
limit is incomplete even when final text has started. Byte decoding is
separate so each returned interval can be decoded independently; it does not
mix private reasoning into final display.

<!-- source: psionic/lib/deepseek/text.coax parts -->
```jai
ds_parts :: (ids:[]int)->DsParts #export_c {
    if ids.count>16384 {return DsParts.{status=DsStatus.budget};}p:=DsParts.{reasoning_end=ids.count,final_begin=ids.count,final_end=ids.count};closed:=false;
    for i:0..<ids.count {id:=ids[i];if id<0 || id>=151936 {return DsParts.{status=DsStatus.token};}
        if id==DS_THINK {if i!=0 {return DsParts.{status=DsStatus.format};}p.reasoning_begin=1;}
        else if id==DS_END_THINK {if closed {return DsParts.{status=DsStatus.format};}p.reasoning_end=i;p.final_begin=i+1;closed=true;}
        else if id==DS_EOS {if !closed || i!=ids.count-1 {return DsParts.{status=DsStatus.format};}p.final_end=i;p.complete=true;}
        else if id==DS_BOS || id==DS_USER || id==DS_ASSISTANT {return DsParts.{status=DsStatus.format};}
    }return p;
}
```

`ds_advance` owns greedy tie breaking and the distinct EOS, token limit,
context limit and between-step cancellation stops. No sampling claim is
made here. The C runner maps files, supplies storage, executes the Coax
schedule and records observations. It contains no model arithmetic.

## Original-weight evidence and its limits

The frozen [qualification policy](../fixtures/deepseek/original/qualification-policy.json)
predates the original comparison. It requires every logit and layer value
within `0.0002 + 0.00002 * abs(reference)`, plus identical greedy selection.
The independent full-prefix NumPy oracle uses original BF16 rows, float64
products, and the same explicitly specified float32 storage boundaries.
It imports no Transformers, Torch, or upstream model code. For the six-token
“Hi” request, every one of 151,936 logits and 86,016 intermediate values
matched exactly on the tested CPU host. Greedy selection was token `71486`
in both. This is one numerical reference request, not a held-out quality
corpus or an upstream runtime qualification.

The [verification record](../../docs/verification/psionic-ch56-2026-10-29.md)
binds the executed build, host, artifact hashes, retained arrays, generation
stop and exact limitations. To make a new capture, supply the pinned originals
in a local directory; this command downloads nothing:

```shell
psionic/tests/run_deepseek_original.sh ORIGINAL_DIRECTORY RECORD_DIRECTORY 64
python3 psionic/tests/deepseek_reference.py RECORD_DIRECTORY --original ORIGINAL_DIRECTORY
```

The second command needs NumPy as authoring tooling. Do not rerun the full
CPU model as a routine gate. Use the retained original reference and the
small changed-code checks instead. Device execution and performance get their
own profiles and evidence when they exist.

## Caller obligations

Views require valid aligned allocations, truthful extents, immutable live
weights/table/input bytes and exclusive writable storage. The native adapter
observes nested address overlaps; it does not prove pointer validity,
allocation ownership, sequential history, provenance or thread safety.
The checked plan bounds logical storage, not process availability. This
chapter does not give a model authority to call tools or act on its answer.

[Book contents](README.md) · [Plan](PLAN.md)
