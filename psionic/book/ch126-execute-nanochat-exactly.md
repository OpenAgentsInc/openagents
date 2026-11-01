# 126. Execute nanochat exactly

> **Status: partly implemented.** Text to text runs on the CPU in binary64
> through the pinned tokenizer format, conversation rules, forward pass and
> cached decoding, and matches an independent oracle token for token. A real
> upstream nanochat checkpoint and its 32,768-token tokenizer, with recorded
> upstream ids and replies, and `engine.py`'s calculator tool remain.

[Chapter 123](ch123-learn-a-tokenizer.md) renders a nanochat conversation
into token ids. [Chapter 125](ch125-execute-the-pinned-nanochat-gpt.md) runs
the pinned `gpt.py` forward pass, decodes one cached token at a time and
admits an inert checkpoint export. [Chapter 53](ch053-choosing-the-next-token.md)
picks a token from logits with a named, seeded generator. This chapter joins
the three into the loop that nanochat's `engine.py` (revision `92d63d4e`, read
and never executed) runs for a chat reply: text in, text out.

## What the engine does

`Engine.generate` takes a rendered prompt and repeats four steps:

1. Run the model and take the logits of the last position.
2. Pick the next token from them: the largest logit at temperature 0,
   otherwise a draw from the softmax after temperature and top-k.
3. Stop if the token is `<|assistant_end|>` or `<|bos|>`. Neither becomes
   part of the reply.
4. Otherwise append the token, feed it through the KV cache and go back to 1.

The prompt comes from `render_for_completion`: the conversation must end with
an assistant message, that message is dropped, the rest is rendered as in
training, and `<|assistant_start|>` is appended so the model writes the
assistant's turn.

This chapter's profile is `nanochat-engine-cpu-f64-v1`:

- The model is chapter 125's binary64 forward pass. The prompt is prefilled
  with the full pass; every later token is one cached step.
- The logits reach the sampler as binary32, as `engine.py` receives them
  after the model's `.float()`.
- Every pick, greedy or not, is chapter 53's `nx_step`: one counter of the
  `splitmix64-top53-v1` generator per step, starting at counter 0 of the
  given seed. A seed therefore names one reply. This is not
  `torch.multinomial`'s random stream, so seeded replies are not claimed to
  equal upstream ones; greedy replies do not depend on the generator.
- The reply stops for one of five stated reasons: `assistant_end`, `bos`,
  `limit` (the requested number of tokens), `context` (the KV cache is full)
  and `tool`.
- `engine.py` watches for `<|python_start|>` ... `<|python_end|>`, evaluates
  the expression with its calculator and forces the result into the reply
  between `<|output_start|>` and `<|output_end|>`. That calculator is not
  implemented here. A reply that closes a python block stops as `tool` with
  the call in its tokens, rather than continuing as if no tool existed.

## Work one reply by hand

The conversation is one user turn, `Hello, world!`, and an empty assistant
turn. `render_for_completion` drops the assistant turn. Chapter 123's table
has 320 ordinary tokens, so the nine specials are 320 to 328 in nanochat's
order. The prompt is:

| position | id | meaning |
| --- | --- | --- |
| 0 | 320 | `<|bos|>` |
| 1 | 321 | `<|user_start|>` |
| 2–12 | 72, 101, 315, 111, 44, 263, 111, 114, 108, 100, 33 | `H`, `e`, `ll`, `o`, `,`, ` w`, `o`, `r`, `l`, `d`, `!` |
| 13 | 322 | `<|user_end|>` |
| 14 | 323 | `<|assistant_start|>` |

The pretokenizer splits the text into `Hello`, `,`, ` world` and `!`. Within
those pieces the 64 learned merges join only `ll` (ID 315) and ` w` (ID 263);
the rest stay single bytes.

That is 15 positions. The prefill fills positions 0 to 14 of the cache and
gives the first pick. With a limit of 12 tokens, picks 1 to 11 are each fed
back as one cached step, at positions 15 to 25. The twelfth pick reaches the
limit and is never fed. The cache then holds 15 + 11 = 26 positions.

The same count gives the `context` stop. The test checkpoint's context is 64
positions. A prompt of 34 positions leaves room for 30 cached steps
(positions 34 to 63); the 31st pick fills the reply's last slot and stops as
`context`, because there is no position left to feed it into.

The checkpoint's weights must also be exactly the exported ones. nanochat
stores `wte` and the value embeddings as BF16, which torch rounds to nearest
with ties to even. A BF16 number keeps 7 fraction bits, so between 1 and 2 its
step is `2^-7`. The value `1 + 3·2^-8` lies halfway between `1 + 2^-7` (odd
last bit) and `1 + 2^-6` (even), so it rounds up to `1 + 2^-6 = 1.015625`;
`1 + 2^-8` lies halfway between 1 and `1 + 2^-7` and rounds down to 1.
Truncation would give `1 + 2^-7` for the first, which is why the stand-in
weights round explicitly:

<!-- source: psionic/lib/nanochat/engine.coax weights -->
```jai
// Round a binary32 to BF16, nearest with ties to even, as torch stores
// nanochat's wte and value embeddings.
ne_bf16 :: (x:f64)->f64 {b:=f32_bits(cast(f32)x);b=(b+0x7fff+((b>>16)&1))&0xffff0000;return cast(f64)f32_from_bits(b);}
// The committed export's values: chapter 125's formula weights, stored F32
// except wte and the value embeddings, which are BF16.
ne_checkpoint_weights :: (c:NcConfig,p:[]f64) {
    nc_formula(c,0.5,p);inv:[NC_TENSORS_MAX]NcTensor;n:=nc_inventory(c,inv[..]);
    for k:0..<n {t:=inv[k];bf:=t.kind==NcKind.wte || t.kind==NcKind.value;
        for e:0..<nc_elements(t) {x:=p[t.offset+e];if bf {p[t.offset+e]=ne_bf16(x);}else {p[t.offset+e]=cast(f64)cast(f32)x;}}}
}
```

## Render for completion

<!-- source: psionic/lib/nanochat/engine.coax prompt -->
```jai
// render_for_completion: the conversation must end with an assistant
// message; it is dropped, the rest is rendered whole (never truncated), and
// <|assistant_start|> opens the reply.
ne_prompt :: (table:[]u8,specials:[]int,vocab:int,messages:[]NcMessage,parts:[]NcPart,ids:[]int,overlap:bool)->NePrompt {
    if messages.count<2 || messages[messages.count-1].role!=NcRole.assistant {return NePrompt.{status=NeStatus.prompt};}
    if overlap {return NePrompt.{status=NeStatus.alias};}
    staged:[NE_PROMPT_MAX]int;mask:[NE_PROMPT_MAX]u8;
    r:=nc_render(table,specials,vocab,messages[0..<messages.count-1],parts,NE_PROMPT_MAX,staged[..],mask[..],false);
    if r.status==NcChatStatus.artifact {return NePrompt.{status=NeStatus.artifact,chat=r.status};}
    if r.status!=NcChatStatus.ok {return NePrompt.{status=NeStatus.prompt,chat=r.status};}
    if r.total+1>ids.count || r.total+1>NE_PROMPT_MAX {return NePrompt.{status=NeStatus.capacity};}
    for i:0..<r.total {ids[i]=staged[i];}ids[r.total]=specials[0]+chat.NC_ASSISTANT_START;
    return NePrompt.{written=r.total+1};
}
```

The renderer is chapter 123's `nc_render`, unchanged; its own refusals (a
wrong special-token layout, a broken role order, invalid UTF-8) pass through
with its status in `chat`. A prompt is never truncated: one that needs more
than the caller's room, or more than 2,048 tokens, refuses as `capacity`
before any ID is written.

## Generate

What one pick does is a small pure function, so its order of rules is visible
and checked on its own:

<!-- source: psionic/lib/nanochat/engine.coax after -->
```jai
// The sampler already stopped on <|assistant_end|> or <|bos|>; neither is
// part of the reply. A python_end that closes an open block stops as tool:
// engine.py would run its calculator here, which this chapter does not.
ne_after :: (base:int,token:int,sampler:NxStop,python:bool,filled:int,context:int)->NeState {
    if sampler==NxStop.stop_token {return NeState.{stop=ifx token==base then NeStop.bos else NeStop.assistant_end};}
    s:=NeState.{emit=true,python=python || token==base+chat.NC_PYTHON_START};
    if python && token==base+chat.NC_PYTHON_END {s.stop=NeStop.tool;}
    else if sampler==NxStop.limit {s.stop=NeStop.limit;}
    else if filled==context {s.stop=NeStop.context;}
    return s;
}
```

<!-- source: psionic/lib/nanochat/engine.coax generate -->
```jai
// Engine.generate for one row. Prefill with the full pass, then one cached
// step per emitted token. base is <|bos|>'s ID; the eight others follow it.
ne_generate :: (c:NcConfig,p:[]f64,base:int,prompt:[]int,options:NxOptions,seed:u64,limit:int,out:[]int,w:NeWork,overlap:bool)->NeReply {
    if base<0 || base+chat.NC_SPECIAL_COUNT>c.vocab {return NeReply.{status=NeStatus.artifact};}
    if limit<1 || !nx_options(options,c.vocab) {return NeReply.{status=NeStatus.shape};}
    if out.count<limit || w.last.count<c.vocab || w.probabilities.count<c.vocab || w.stage.count<c.vocab || w.order.count<c.vocab || w.temporary.count<c.vocab {return NeReply.{status=NeStatus.capacity};}
    if overlap {return NeReply.{status=NeStatus.alias};}
    none:[1]f64;
    f:=nc_forward(c,p,prompt,w.cache,w.work,w.logits,none[0..<0],false);
    if f.status!=NcStatus.ok {return NeReply.{status=NeStatus.model,model=f.status};}
    row:=w.logits[(prompt.count-1)*c.vocab..<prompt.count*c.vocab];filled:=prompt.count;
    run:=NxRun.{random=CrSeed.{seed=seed},limit=limit};stops:=[2]int.[base+chat.NC_ASSISTANT_END,base];
    next:=nx_work(w.stage,w.order,w.temporary);python:=false;written:=0;
    while true {
        for v:0..<c.vocab {w.last[v]=cast(f32)row[v];}
        nomask:[1]u8;e:=nx_step(w.last[0..<c.vocab],nomask[0..<0],options,run,stops[..],false,w.probabilities,next,false);#assert e.status==NxStatus.ok;
        run=e.run;a:=ne_after(base,e.token,run.stop,python,filled,c.context);python=a.python;
        if a.emit {out[written]=e.token;written+=1;}
        if a.stop!=NeStop.none {return NeReply.{stop=a.stop,written=written,positions=filled};}
        d:=nc_decode(c,p,w.cache,filled,e.token,w.work,w.logits,false);#assert d.status==NcStatus.ok;
        row=w.logits[0..<c.vocab];filled+=1;
    }
    return NeReply.{};
}
```

Every refusal comes before the first write: a special-token range that does
not fit the vocabulary (`artifact`), a limit below 1 or sampler options
chapter 53 would refuse (`shape`), too little room for the reply or the
sampler (`capacity`), an alias, or a model refusal from chapter 125 (`model`,
with chapter 125's own status, for example a prompt longer than the context).
After admission neither the model nor the sampler can fail, which the loop
states with assertions.

The reply's bytes come from the table for ordinary tokens and from the
special names for specials, as tiktoken decodes them. The size is computed
first, so a refusal writes nothing:

<!-- source: psionic/lib/nanochat/engine.coax text -->
```jai
// The reply's bytes: ordinary tokens from the table, specials by name, as
// tiktoken decodes them. Sized first; a refusal writes nothing.
ne_text :: (table:[]u8,ids:[]int,out:[]u8)->NeText {
    t:=pt_layout(table);if !chat.nc_ok(t) {return NeText.{status=NeStatus.artifact};}base:=t.counts[0];names:=chat.NC_SPECIAL_NAMES;need:=0;
    for id:ids {if id<0 || id>=base+chat.NC_SPECIAL_COUNT {return NeText.{status=NeStatus.shape};}
        if id<base {s:=pt_span(table,t,id);need+=s.end-s.begin;}else {need+=names[id-base].count;}}
    if need>out.count {return NeText.{status=NeStatus.capacity};}
    at:=0;
    for id:ids {
        if id<base {s:=pt_span(table,t,id);for i:s.begin..<s.end {out[at]=table[i];at+=1;}}
        else {name:=names[id-base];for i:0..<name.count {out[at]=name[i];at+=1;}}
    }
    return NeText.{written=at};
}
```

## Run it

[The example](../examples/nanochat-reply.coax) renders the hand-worked conversation, generates a greedy reply
and a seeded top-k 40 reply from the stand-in checkpoint, decodes both, and
shows two refusals:

<!-- source: psionic/examples/nanochat-reply.coax example -->
```jai
main :: () {
    table:=NC_LEARNED;sp:[9]int;ne_specials(sp[..]);none:[1]NcPart;
    hello:=[2]NcMessage.[NcMessage.{role=NcRole.user,text="Hello, world!"},NcMessage.{role=NcRole.assistant,text=""}];
    prompt:[64]int;r:=ne_prompt(table[..],sp[..],329,hello[..],none[0..<0],prompt[..],false);#assert r.status==NeStatus.ok;
    show_ids("prompt",prompt[0..<r.written]);

    c:=NE_CHECKPOINT;p:[53290]f64;ne_checkpoint_weights(c,p[..]);
    cache:[4128]f64;work:[64*6*32+6*32+64]f64;logits:[64*329]f64;last:[329]f32;pr:[329]f64;st:[329]f64;or:[329]int;te:[329]int;
    w:=ne_buffers(c,r.written,cache[..],work[..],logits[..],last[..],pr[..],st[..],or[..],te[..]);
    settings:=[2]NxOptions.[NxOptions.{temperature=0},NxOptions.{temperature=1,top_k=40}];seeds:=[2]u64.[0,126];
    labels:=[2]string.["greedy","seed 126, top-k 40"];
    for k:0..<2 {
        out:[64]int;g:=ne_generate(c,p[..],320,prompt[0..<r.written],settings[k],seeds[k],12,out[..],w,false);#assert g.status==NeStatus.ok;
        print("% reply, stopped by % after % positions\n",labels[k],g.stop,g.positions);show_ids("  ids",out[0..<g.written]);
        text:[512]u8;t:=ne_text(table[..],out[0..<g.written],text[..]);#assert t.status==NeStatus.ok;
        print("  text: ");show_text(text[0..<t.written]);print("\n");
    }

    // A conversation that ends with the user's turn has no reply to open.
    asked:=[1]NcMessage.[hello[0]];bad:=ne_prompt(table[..],sp[..],329,asked[..],none[0..<0],prompt[..],false);
    print("refused prompt: %, written %\n",bad.status,bad.written);
    // A prompt longer than the model's context refuses before any token is drawn.
    long:[70]int;for i:0..<70 {long[i]=97;}out:[8]int;
    g:=ne_generate(c,p[..],320,long[..],settings[0],0,8,out[..],w,false);
    print("refused generation: % (model %), written %\n",g.status,g.model,g.written);
}
```

```shell
coax/bin/coax run psionic/examples/nanochat-reply.coax
```

```text
prompt: 320 321 72 101 315 111 44 263 111 114 108 100 33 322 323
greedy reply, stopped by limit after 26 positions
  ids: 244 213 49 270 323 38 35 274 45 107 253 25
  text: \xf4\xd51in<|assistant_start|>&# o-k\xfd\x19
seed 126, top-k 40 reply, stopped by limit after 26 positions
  ids: 235 195 4 85 63 59 290 34 123 156 283 270
  text: \xeb\xc3\x04U?; st"{\x9c pin
refused prompt: prompt, written 0
refused generation: model (model shape), written 0
```

The [transcript](../fixtures/nanochat-reply.out) is exact in debug and release. The prompt row is the table above, and both replies stop at the limit after
26 positions, as counted by hand. The weights are deterministic stand-ins, not
a trained model, so the replies are not language; bytes outside printable
ASCII are shown as `\xNN`, and the greedy reply even contains
`<|assistant_start|>` as a special token. The first refusal is a conversation
that ends with the user's turn (`prompt`, nothing written). The second is a
70-token prompt for a 64-position context: chapter 125 refuses it as `shape`
before the sampler draws once.

## An independent oracle

`psionic/tests/nanochat_engine_oracle.py` composes three Python oracles that
share no code with Coax: chapter 123's renderer and tokenizer (Python `re`
with Unicode 17 classes, lowest-rank merging), chapter 125's binary64 model
with the export's F32 and round-to-nearest BF16 weights, and chapter 53's
sampler rules. It runs every position, prompt included, through its own
one-token-at-a-time path, so the Coax prefill-then-decode is compared with a
different evaluation order. It writes `psionic/fixtures/nanochat/engine-goldens.coax`
with five conversations:

| case | prompt | reply | stop |
| --- | --- | --- | --- |
| greedy, one user turn | 15 | 12 | `limit` |
| seed 126, top-k 40 | 15 | 12 | `limit` |
| system turn merged, temperature 0.8, top-p 0.9, seed 53 | 23 | 12 | `limit` |
| a second exchange, temperature 1.5, seed 9 | 34 | 31 | `context` |
| temperature 4, the first seed from 1 that stops by itself (5) | 6 | 21 | `assistant_end` |

`psionic/lib/nanochat/engine-checks.coax` renders, generates and decodes each
one in Coax and requires the prompt ids, every reply token, the stop reason
and every decoded byte to be equal. It also compares the first prompt's 329
last-position logits within `|a-b| <= 1e-12 (1+|b|)` and every 97th of the
53,290 stand-in weights exactly. The stand-in weights are the values chapter
125's committed `model.safetensors` carries, which chapter 125's export test
checks byte for byte against the same oracle.

The unit's own checks (`psionic/lib/nanochat/engine.coax`) use a one-block
model with 12 IDs, cheap enough to replay every pick: for eight seeds that
together reach all five stops, each emitted token and each sampled stop token
is recomputed with chapter 125's full pass over the prompt plus the reply so
far and chapter 53's sampler at counter k. The cached path and the full pass
therefore pick the same tokens at every step. A changed token, stop, position
count or seed fails the replay. Refusals, the stop rule's order (tool before
limit, limit before context), prompt boundaries (2,048 tokens accepted, 2,049
refused) and decoding are checked directly.

```shell
psionic/check.sh --only nanochat/engine --jobs 8
```

## What remains

- **A real nanochat checkpoint.** The checkpoint here is chapter 125's
  committed export of deterministic weights in nanochat's format, with
  chapter 123's 320-token tokenizer. A trained upstream checkpoint, its
  32,768-token `rustbpe` tokenizer exported without unpickling
  `tokenizer.pkl`, upstream-recorded token ids for fixed texts, and
  upstream-recorded greedy replies (or the first divergent token) are not yet
  in the repository.
- **Seeded upstream parity.** Seeded replies equal this chapter's oracle, not
  `torch.multinomial`'s stream.
- **The calculator tool.** Replies that call python stop as `tool`;
  `engine.py`'s forced output tokens are not produced.

No step executes a pickle: the tokenizer table is chapter 123's PST5 data, and
the weights are chapter 125's formula with the export's rounding.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
- [The verification record](../../docs/verification/psionic-ch126-nanochat-engine-2026-11-01.md)
