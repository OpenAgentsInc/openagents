# 125. Execute the pinned nanochat GPT

Chapter 113 built a causal decoder from first principles. nanochat's `gpt.py`
is a real one with a dozen deliberate departures from the textbook, and
chapter 124 already trains it with nanochat's own optimizer. This chapter runs
that exact architecture, pinned to `karpathy/nanochat` revision `92d63d4e`
(`gpt.py` SHA-256 `8cdedc2e…04e3`), on the CPU in binary64, and loads a real
`torch.save` checkpoint without executing its pickle. Generic GPT
compatibility is not the claim: every feature below is the pinned file's, and
each one has an independent fixture that changes when it is removed.

## What the pinned profile computes

For token ids `x`, `gpt.py` at this revision does, in order:

- `wte` embedding, then `norm` (RMS with no weight and PyTorch's default eps,
  which in binary64 is `2^-52`), giving `x0`.
- **Smear**: from position 1 on, `x_t += smear_lambda * sigmoid(smear_gate ·
  x_t[:24]) * x_(t-1)`, using the normalized embeddings. Decoding keeps the
  previous embedding in the cache.
- Per layer `i`: `x = resid_lambdas[i]*x + x0_lambdas[i]*x0`; attention on
  `norm(x)`; MLP on `norm(x)`; both residual.
- Attention: `c_q`, `c_k`, `c_v` with grouped key/value heads; **value
  embeddings** on layers with `i % 2 == (L-1) % 2`, added to `v` with a gate
  `3*sigmoid(ve_gate(x[:12]))` per key/value head; **RoPE** over the two halves
  of each head, base 100000, rotated by `-theta`; **QK norm** then both scaled
  by 1.2; scaled dot product with scale `1/sqrt(head_dim)`.
- **Sliding windows** from `window_pattern` (`S` short, `L` long, tiled over
  layers, the last layer always long). The short window is
  `ceil(sequence_len/4/128)*128` tokens; a key `j` is visible from `t` when
  `j <= t` and `t - j <= window`. The comment in `gpt.py` ("2048->768") is
  stale: the code gives 512, and so does this chapter.
- MLP `c_proj(relu(c_fc(x))^2)` with hidden width `4*n_embd`.
- **Backout**: after layer `L//2` the residual is saved, and before the final
  norm `x -= backout_lambda * x_saved`.
- Final `norm`, an **untied** `lm_head` over the vocabulary padded to a
  multiple of 64 and cropped back, then softcap `15*tanh(z/15)`.

<!-- source: psionic/lib/nanochat/gpt.coax config -->
```jai
// GPTConfig's seven fields; window_pattern is bit i = 1 for 'S' at character
// i, over pattern_len characters. context is the KV cache's capacity.
NcConfig :: struct {sequence_len:int;vocab:int;layers:int;heads:int;kv_heads:int;width:int;pattern:u32;pattern_len:int;context:int;}
```

The parameters live in one binary64 view in `state_dict` order; the layout
gives each tensor its offset, rows and columns:

<!-- source: psionic/lib/nanochat/gpt.coax layout -->
```jai
nc_valid :: (c:NcConfig)->bool {
    if c.sequence_len<2 || c.sequence_len>256 || c.vocab<2 || c.vocab>512 || c.layers<1 || c.layers>4 {return false;}
    if c.heads<1 || c.heads>8 || c.kv_heads<1 || c.kv_heads>c.heads || c.heads%c.kv_heads!=0 {return false;}
    if c.width<NC_SMEAR || c.width>64 || c.width%c.heads!=0 {return false;}
    hd:=c.width/c.heads;if hd<2 || hd%2!=0 {return false;}
    if c.pattern_len<1 || c.pattern_len>16 || c.pattern>=(cast(u32)1<<c.pattern_len) {return false;}
    return c.context>=1 && c.context<=320 && c.context<=10*c.sequence_len;
}
nc_padded :: (c:NcConfig)->int {return (c.vocab+NC_PAD-1)/NC_PAD*NC_PAD;}
nc_head_dim :: (c:NcConfig)->int {return c.width/c.heads;}
nc_kv_width :: (c:NcConfig)->int {return c.kv_heads*nc_head_dim(c);}
nc_has_ve :: (c:NcConfig,l:int)->bool {return l%2==(c.layers-1)%2;}
// -(-sequence_len // 4 // 128) * 128: a quarter of the context rounded up to 128.
nc_short :: (c:NcConfig)->int {return ((c.sequence_len+3)/4+127)/128*128;}
// The left window of layer l; the final layer is always L.
nc_window :: (c:NcConfig,l:int)->int {
    if l==c.layers-1 {return c.sequence_len;}
    if ((c.pattern>>cast(u32)(l%c.pattern_len))&1)==1 {return nc_short(c);}
    return c.sequence_len;
}
nc_block_size :: (c:NcConfig,l:int)->int {
    d:=c.width;kv:=nc_kv_width(c);n:=2*d*d+2*kv*d+8*d*d;if nc_has_ve(c,l) {n+=c.kv_heads*NC_GATE;}return n;
}
// Flat binary64 parameters in GPT.state_dict() order: resid_lambdas,
// x0_lambdas, smear_lambda, backout_lambda, wte, each block's c_q, c_k, c_v,
// c_proj, [ve_gate], c_fc, mlp c_proj, then lm_head, smear_gate and the value
// embeddings of the layers that have one.
nc_wte :: (c:NcConfig)->int {return 2*c.layers+2;}
nc_block :: (c:NcConfig,l:int)->NcBlock {
    d:=c.width;kv:=nc_kv_width(c);at:=nc_wte(c)+nc_padded(c)*d;for i:0..<l {at+=nc_block_size(c,i);}
    b:NcBlock;b.q=at;b.k=b.q+d*d;b.v=b.k+kv*d;b.proj=b.v+kv*d;b.gate=-1;at=b.proj+d*d;
    if nc_has_ve(c,l) {b.gate=at;at+=c.kv_heads*NC_GATE;}b.fc=at;b.mlp=b.fc+4*d*d;b.value=-1;
    if nc_has_ve(c,l) {before:=0;for i:0..<l {if nc_has_ve(c,i) {before+=1;}}b.value=nc_smear_gate(c)+NC_SMEAR+before*nc_padded(c)*kv;}
    return b;
}
nc_head :: (c:NcConfig)->int {at:=nc_wte(c)+nc_padded(c)*c.width;for i:0..<c.layers {at+=nc_block_size(c,i);}return at;}
nc_smear_gate :: (c:NcConfig)->int {return nc_head(c)+nc_padded(c)*c.width;}
nc_ve_layers :: (c:NcConfig)->int {n:=0;for l:0..<c.layers {if nc_has_ve(c,l) {n+=1;}}return n;}
nc_parameter_count :: (c:NcConfig)->int #export_c {return nc_smear_gate(c)+NC_SMEAR+nc_ve_layers(c)*nc_padded(c)*nc_kv_width(c);}
nc_tensor_count :: (c:NcConfig)->int {return 5+6*c.layers+2*nc_ve_layers(c)+2;}
```

<!-- source: psionic/lib/nanochat/gpt.coax rotary -->
```jai
// apply_rotary_emb: first and second halves of each head, rotated by -theta.
nc_rotary :: (x:[]f64,xo:int,heads:int,hd:int,pos:int) {
    half:=hd/2;
    for h:0..<heads {for i:0..<half {
        angle:=f64(pos)*(1/exp(log(NC_BASE)*f64(2*i)/f64(hd)));co:=cos(angle);si:=sin(angle);
        a:=xo+h*hd+i;x1:=x[a];x2:=x[a+half];x[a]=x1*co+x2*si;x[a+half]=x1*(-si)+x2*co;
    }}
}
```

The forward pass runs layer by layer over all positions, so the full path and
the cached path share one body. `nc_decode` appends one token to the cache and
runs the same code on it:

<!-- source: psionic/lib/nanochat/gpt.coax run -->
```jai
// Positions filled..filled+n-1 through every layer; keys and values of
// earlier positions come from the cache. Layer by layer, as the training path.
nc_run :: (c:NcConfig,p:[]f64,cache:[]f64,filled:int,tokens:[]int,work:[]f64,logits:[]f64,blocks:[]f64) {
    n:=tokens.count;d:=c.width;hd:=nc_head_dim(c);kv:=nc_kv_width(c);group:=c.heads/c.kv_heads;heads:=c.heads;kvh:=c.kv_heads;vocab:=c.vocab;
    X:=0;X0:=n*d;XB:=2*n*d;RAW:=3*n*d;Q:=4*n*d;Y:=5*n*d;S:=6*n*d;XN:=S;HID:=S+d;TMP:=S+5*d;SC:=S+6*d;
    lam:=p[2*c.layers];sg:=nc_smear_gate(c);PREV:=nc_prev(c);W:=nc_wte(c);L:=c.layers;
    for i:0..<n {nc_rms(p,W+tokens[i]*d,d,work,RAW+i*d);}
    // Smear: the previous position's normalized embedding, gated by its own first 24 channels.
    for i:0..<n {before:=-1;if i>0 {before=RAW+(i-1)*d;}
        if before>=0 || filled>0 {
            z:=0.0;for j:0..<NC_SMEAR {z+=p[sg+j]*work[RAW+i*d+j];}gate:=lam*nc_sigmoid(z);
            for j:0..<d {prior:=0.0;if before>=0 {prior=work[before+j];}else {prior=cache[PREV+j];}work[X+i*d+j]=work[RAW+i*d+j]+gate*prior;}
        } else {for j:0..<d {work[X+i*d+j]=work[RAW+i*d+j];}}
        for j:0..<d {work[X0+i*d+j]=work[X+i*d+j];}
    }
    for l:0..<c.layers {b:=nc_block(c,l);window:=nc_window(c,l);K:=nc_keys(c,l);V:=nc_values(c,l);
        for i:0..<n {t:=filled+i;key:=K+t*kv;value:=V+t*kv;
            for j:0..<d {work[X+i*d+j]=p[l]*work[X+i*d+j]+p[L+l]*work[X0+i*d+j];}
            nc_rms(work,X+i*d,d,work,XN);nc_mv(p,b.q,d,d,work,XN,work,Q+i*d);nc_mv(p,b.k,kv,d,work,XN,cache,key);nc_mv(p,b.v,kv,d,work,XN,cache,value);
            if b.gate>=0 {
                for g:0..<kvh {z:=0.0;for j:0..<NC_GATE {z+=p[b.gate+g*NC_GATE+j]*work[XN+j];}gate:=3*nc_sigmoid(z);
                    for x:0..<hd {cache[value+g*hd+x]=cache[value+g*hd+x]+gate*p[b.value+tokens[i]*kv+g*hd+x];}}
            }
            nc_rotary(work,Q+i*d,heads,hd,t);nc_rotary(cache,key,kvh,hd,t);
            nc_qk_norm(work,Q+i*d,heads,hd);nc_qk_norm(cache,key,kvh,hd);
        }
        for i:0..<n {t:=filled+i;first:=0;if t-window>0 {first=t-window;}
            for h:0..<heads {g:=h/group;top:=0.0;
                for j:first..<t+1 {s:=0.0;for x:0..<hd {s+=work[Q+i*d+h*hd+x]*cache[K+j*kv+g*hd+x];}s=s/sqrt(f64(hd));work[SC+j-first]=s;if j==first || s>top {top=s;}}
                total:=0.0;for j:first..<t+1 {work[SC+j-first]=exp(work[SC+j-first]-top);total+=work[SC+j-first];}
                for x:0..<hd {acc:=0.0;for j:first..<t+1 {acc+=work[SC+j-first]/total*cache[V+j*kv+g*hd+x];}work[Y+i*d+h*hd+x]=acc;}
            }
        }
        for i:0..<n {
            nc_mv(p,b.proj,d,d,work,Y+i*d,work,TMP);for j:0..<d {work[X+i*d+j]=work[X+i*d+j]+work[TMP+j];}
            nc_rms(work,X+i*d,d,work,XN);nc_mv(p,b.fc,4*d,d,work,XN,work,HID);for f:0..<4*d {z:=work[HID+f];if z<0 {z=0;}work[HID+f]=z*z;}
            nc_mv(p,b.mlp,d,4*d,work,HID,work,TMP);for j:0..<d {work[X+i*d+j]=work[X+i*d+j]+work[TMP+j];}
            if blocks.count>0 {for j:0..<d {blocks[(l*n+i)*d+j]=work[X+i*d+j];}}
            if l==L/2 {for j:0..<d {work[XB+i*d+j]=work[X+i*d+j];}}
        }
    }
    back:=p[2*c.layers+1];H:=nc_head(c);
    for i:0..<n {for j:0..<d {work[TMP+j]=work[X+i*d+j]-back*work[XB+i*d+j];}nc_rms(work,TMP,d,work,XN);
        for v:0..<vocab {acc:=0.0;for j:0..<d {acc+=p[H+v*d+j]*work[XN+j];}logits[i*vocab+v]=NC_SOFTCAP*nc_tanh(acc/NC_SOFTCAP);}
    }
    for j:0..<d {cache[PREV+j]=work[RAW+(n-1)*d+j];}
}
```

<!-- source: psionic/lib/nanochat/gpt.coax api -->
```jai
// The training path over a whole sequence (the reference asserts T > 1). It
// also fills the cache, so it doubles as the prefill. blocks, when not empty,
// receives each layer's residual output per position.
nc_forward :: (c:NcConfig,p:[]f64,tokens:[]int,cache:[]f64,work:[]f64,logits:[]f64,blocks:[]f64,overlap:bool)->NcResult #export_c {
    status:=nc_admit(c,p,cache);if status!=NcStatus.ok {return NcResult.{status=status};}
    n:=tokens.count;if n<2 || n>c.context {return NcResult.{status=NcStatus.shape};}
    for x:tokens {if x<0 || x>=c.vocab {return NcResult.{status=NcStatus.token};}}
    if work.count<nc_work_count(c,n) || logits.count<n*c.vocab || (blocks.count>0 && blocks.count<c.layers*n*c.width) {return NcResult.{status=NcStatus.capacity};}
    if overlap {return NcResult.{status=NcStatus.alias};}
    nc_run(c,p,cache,0,tokens,work,logits,blocks);
    return NcResult.{written=n*c.vocab,loss=nc_loss(c,logits,tokens)};
}
// One token at position `filled` of a cache that holds positions 0..filled-1.
nc_decode :: (c:NcConfig,p:[]f64,cache:[]f64,filled:int,token:int,work:[]f64,logits:[]f64,overlap:bool)->NcResult #export_c {
    status:=nc_admit(c,p,cache);if status!=NcStatus.ok {return NcResult.{status=status};}
    if filled<0 || filled>=c.context {return NcResult.{status=NcStatus.position};}if token<0 || token>=c.vocab {return NcResult.{status=NcStatus.token};}
    if work.count<nc_work_count(c,1) || logits.count<c.vocab {return NcResult.{status=NcStatus.capacity};}if overlap {return NcResult.{status=NcStatus.alias};}
    one:=[1]int.[token];none:[1]f64;nc_run(c,p,cache,filled,one[..],work,logits,none[0..<0]);
    return NcResult.{written=filled+1};
}
```

## An oracle written from the reference

`psionic/tests/nanochat_oracle.py` is a second implementation in plain Python
binary64, written from `gpt.py` and `flash_attention.py` (SHA-256
`a10b0b10…2c2f`) and not from the Coax code. Torch never computes a fixture.
It has three cases, with deterministic weights `scale*sin(0.61e + 1.3k +
0.25) + offset` per tensor `k`:

| case | config | tensors | values | loss |
|---|---|---|---|---|
| tiny | seq 8, vocab 5, 1 layer, 4 heads / 2 kv, width 24, `L` | 15 | 10228 | 5.853663622595427 |
| window | seq 256, vocab 7, 3 layers, 2 / 1, width 24, `SL`, 140 tokens | 29 | 23672 | 4.727822180934721 |
| checkpoint | seq 64, vocab 329, 2 layers, 2 / 1, width 32, `SSSL` | 21 | 53290 | 7.379816235132483 |

The window case is long enough that its short window (128) cuts in: the oracle
asserts that a full-context run differs from it at position 129 and at no
earlier one. Its own cached path equals its full path exactly.

The Coax checks (`psionic/lib/nanochat/gpt-checks.coax`) compare every logit
and every block's output at every position against the oracle with a stated
tolerance of `|a-b| <= 1e-12 (1+|b|)`; the largest observed difference is
2.46e-14. Decoding one token at a time reproduces the full pass bit for bit.
A sensitivity check makes every enabled feature observable: over the window
case, an all-long pattern changes the logits; and perturbing in turn a
residual scalar, an x0 scalar, `smear_lambda`, `backout_lambda`, the smear
gate, the value-embedding gate, a value embedding, `wte`, a query weight,
`lm_head` and the MLP moves them, while the smear terms leave position 0
alone.

## A checkpoint without pickle

A nanochat checkpoint is `model_<step>.pt` (a `torch.save` zip whose
`data.pkl` would run arbitrary code under `torch.load`) and
`meta_<step>.json`. `psionic/tools/export/nanochat_export.py` never imports
torch and never calls the default unpickler. It reads the zip, unpickles
`data.pkl` with an unpickler that admits exactly four globals
(`collections.OrderedDict`, `torch._utils._rebuild_tensor_v2`,
`torch.FloatStorage`, `torch.BFloat16Storage`) and builds nothing from them
but a tuple description; it checks little-endian byte order, contiguity and
storage bounds, strips a uniform `_orig_mod.` compile prefix (as
`checkpoint_manager.build_model` does) and refuses a mixed one. It writes
`model.safetensors` and an identity record, `export.json`
(`psionic.nanochat-export.v1`): the source repository, revision and the
SHA-256 of `gpt.py` and `checkpoint_manager.py`; the seven `GPTConfig` keys
exactly as written; the padding; the tokenizer's format, digest and
vocabulary; the `.pt` and meta digests, step and prefix; and the
safetensors digest.

```shell
python3 -I psionic/tools/export/nanochat_export.py \
  psionic/fixtures/nanochat/checkpoint/model_000000.pt \
  psionic/fixtures/nanochat/checkpoint/meta_000000.json \
  psionic/fixtures/bpe-train/learned.pst OUT \
  --tokenizer-format psionic-pst5+nanochat-specials --tokenizer-vocab 329
```

The committed checkpoint was written by `torch.save` (torch 2.14.1, CPU) from
the oracle's checkpoint weights, BF16 for `wte` and the value embeddings as
nanochat stores them and F32 for the rest. Its 21 tensors re-export byte for
byte to the committed `model.safetensors` (178160 bytes) and carry the oracle's
exact bits. Eight crafted archives refuse: a foreign `os.system` global, a
non-tuple persistent id, big-endian order, a strided tensor, an out-of-bounds
offset, a half-precision storage, a mixed prefix and a short storage.

## Admission in Coax

`psionic/lib/nanochat/checkpoint.coax` is the authority. It parses the
identity record against fixed key tables, refusing unknown and repeated keys,
and pins the source:

<!-- source: psionic/lib/nanochat/checkpoint.coax config -->
```jai
// GPTConfig's seven keys. window_pattern is upper-cased as gpt.py does; a
// missing one is checkpoint_manager's historical "L" patch and refuses.
nc_model_config :: (j:[]Json)->NcRead {
    if json_next(j)!=JSON_OBJECT {return NcRead.{status=NcLoadStatus.schema};}seen:=0;keys:=NC_CONFIG_KEYS;c:NcConfig;t:=json_next(j);
    while t!=JSON_OBJECT_END {m:=nc_member(j,t,keys[..],seen);if m.status!=NcLoadStatus.ok {return NcRead.{status=m.status};}seen|=m.bit;
        if m.bit==64 {
            v:=nc_string(j);if !v.ok {return NcRead.{status=NcLoadStatus.schema};}if v.text.count>16 {return NcRead.{status=NcLoadStatus.config};}
            bits:u16;for i:0..<v.text.count {ch:=v.text[i];if ch=='S' || ch=='s' {bits|=cast(u16)1<<cast(u16)i;}else if ch!='L' && ch!='l' {return NcRead.{status=NcLoadStatus.config};}}
            c.pattern=cast(u32)bits;c.pattern_len=v.text.count;
        } else {
            v:=nc_int(j);if !v.ok {return NcRead.{status=NcLoadStatus.schema};}
            if m.bit==1 {c.sequence_len=v.value;}if m.bit==2 {c.vocab=v.value;}if m.bit==4 {c.layers=v.value;}
            if m.bit==8 {c.heads=v.value;}if m.bit==16 {c.kv_heads=v.value;}if m.bit==32 {c.width=v.value;}
        }
        t=json_next(j);
    }
    if seen==63 {return NcRead.{status=NcLoadStatus.historical};}if seen!=127 {return NcRead.{status=NcLoadStatus.schema};}
    c.context=c.sequence_len;if !nc_valid(c) {return NcRead.{status=NcLoadStatus.config};}return NcRead.{config=c};
}
```

`checkpoint_manager.py` patches old checkpoints: a missing `window_pattern`
becomes `"L"` and missing `resid_lambdas`/`x0_lambdas` become ones and zeros.
Those are not the pinned profile, so both refuse as `historical`, before any
parameter is written; a separately versioned reader would be the place for
them.

<!-- source: psionic/lib/nanochat/checkpoint.coax identity -->
```jai
nc_identity :: (text:string)->NcIdentity {
    if text.count>NC_IDENTITY_MAX {return NcIdentity.{status=NcLoadStatus.json};}
    j:[1]Json;json_start_bounded(j[..],text,text.count,text.count);
    if json_next(j[..])!=JSON_OBJECT {return NcIdentity.{status=NcLoadStatus.json};}
    id:NcIdentity;seen:=0;keys:=NC_TOP_KEYS;vocab:int;t:=json_next(j[..]);
    while t!=JSON_OBJECT_END {m:=nc_member(j[..],t,keys[..],seen);if m.status!=NcLoadStatus.ok {return NcIdentity.{status=m.status};}seen|=m.bit;status:=NcLoadStatus.ok;
        if m.bit==1 {v:=nc_string(j[..]);if !v.ok || v.text!=NC_SCHEMA {status=NcLoadStatus.schema;}}
        if m.bit==2 {status=nc_source(j[..]);}
        if m.bit==4 {r:=nc_model_config(j[..]);status=r.status;id.config=r.config;}
        if m.bit==8 {v:=nc_int(j[..]);if !v.ok || v.value!=NC_PAD {status=NcLoadStatus.config;}}
        if m.bit==16 {r:=nc_tokenizer(j[..]);status=r.status;vocab=r.value;}
        if m.bit==32 {r:=nc_checkpoint(j[..]);status=r.status;id.step=r.value;}
        if m.bit==64 {v:=nc_string(j[..]);if !v.ok || !nc_hex_digest(v.text) {status=NcLoadStatus.schema;}id.digest=v.text;}
        if status!=NcLoadStatus.ok {return NcIdentity.{status=status};}
        t=json_next(j[..]);
    }
    if json_next(j[..])!=JSON_END {return NcIdentity.{status=NcLoadStatus.json};}
    if seen!=127 {return NcIdentity.{status=NcLoadStatus.schema};}
    // build_model's own assert: the tokenizer and the model agree on the vocabulary.
    if vocab!=id.config.vocab {return NcIdentity.{status=NcLoadStatus.tokenizer};}
    return id;
}
```

Admission checks the digest the caller supplies against the record, inspects
the safetensors header with Psionic's checked safetensors reader, matches every tensor's name,
rank and shape against the layout, and checks that every value is finite and
within `2^20` before it writes any. `nc_load` computes the SHA-256 itself:

<!-- source: psionic/lib/nanochat/checkpoint.coax load -->
```jai
// digest is the SHA-256 of file; nc_load computes it.
nc_admit_export :: (identity:string,digest:[]u8,file:[]u8,params:[]f64,inventory:[]PwTensor,draft:[]PwTensor,overlap:bool)->NcLoad #export_c {
    if overlap {return NcLoad.{status=NcLoadStatus.alias};}
    id:=nc_identity(identity);if id.status!=NcLoadStatus.ok {return NcLoad.{status=id.status};}c:=id.config;
    if params.count!=nc_parameter_count(c) {return NcLoad.{status=NcLoadStatus.capacity};}
    if digest.count!=32 || !nc_digest_is(digest,id.digest) {return NcLoad.{status=NcLoadStatus.digest};}
    r:=pw_inspect_checked(file,inventory,draft,false);if r.status!=PwStatus.ok {return NcLoad.{status=NcLoadStatus.format};}
    tensors:=inventory[0..<r.written];want:[NC_TENSORS_MAX]NcTensor;n:=nc_inventory(c,want[..]);name:[64]u8;
    if nc_find(tensors,"resid_lambdas")==-1 || nc_find(tensors,"x0_lambdas")==-1 {return NcLoad.{status=NcLoadStatus.historical};}
    if r.written!=n {return NcLoad.{status=NcLoadStatus.manifest};}
    for k:0..<n {t:=want[k];m:=nc_tensor_name(t,name[..]);at:=nc_find(tensors,cast(string)name[0..<m]);
        if at<0 {return NcLoad.{status=NcLoadStatus.manifest};}s:=tensors[at].shape;
        if t.cols==0 {if s.rank!=1 || s.dims[0]!=t.rows {return NcLoad.{status=NcLoadStatus.shape};}}
        else if s.rank!=2 || s.dims[0]!=t.rows || s.dims[1]!=t.cols {return NcLoad.{status=NcLoadStatus.shape};}
        width:=pw_width(tensors[at].dtype);
        for e:0..<nc_elements(t) {bits:=pw_float_bits(file,tensors[at].begin+e*width,tensors[at].dtype);
            if !psionic_tensor_finite_bits(bits) || abs(cast(f64)f32_from_bits(bits))>NC_NUMBER {return NcLoad.{status=NcLoadStatus.number};}}
    }
    for k:0..<n {t:=want[k];m:=nc_tensor_name(t,name[..]);tensor:=tensors[nc_find(tensors,cast(string)name[0..<m])];width:=pw_width(tensor.dtype);
        for e:0..<nc_elements(t) {params[t.offset+e]=cast(f64)f32_from_bits(pw_float_bits(file,tensor.begin+e*width,tensor.dtype));}}
    return NcLoad.{config=c,written=params.count,step=id.step};
}
nc_load :: (identity:string,file:[]u8,params:[]f64,inventory:[]PwTensor,draft:[]PwTensor,overlap:bool)->NcLoad #export_c {
    digest:[32]u8;if sha256_hash_checked(file,digest[..],false)!=SHA_OK {return NcLoad.{status=NcLoadStatus.digest};}
    return nc_admit_export(identity,digest[..],file,params,inventory,draft,overlap);
}
```

The C harness `psionic/tests/nanochat_native.c` reads the committed export
from disk, loads it through `nc_load`, runs the checkpoint, tiny and window
cases against the oracle header and decodes token by token; a flipped payload
byte refuses as `digest`, the historical record refuses as `historical`, and
both leave the parameters untouched.

```shell
psionic/check.sh --only nanochat
```

[The verification record](../../docs/verification/psionic-ch125-nanochat-2026-10-31.md)
records the checks, mutation and native evidence.
