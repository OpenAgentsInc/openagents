# 58. Continue a passage

> **Status: partly implemented.** The pinned checkpoint's inventory, the
> Llama 3 tokenizer on ASCII text, llama3 rotary frequencies, a key-value
> cache whose incremental logits equal the full recompute's bit for bit, and
> greedy and seeded continuation run on the CPU. On the original weights,
> layer 0 and an `lm_head` chunk match an independent float64 reference, and
> all 32 layers continue a prompt greedily and with a seed. An independent
> reference for the whole model's logits, the tokenizer's full Unicode
> pattern and positions past 64 remain.

This chapter runs the first NVIDIA text generator in the plan,
Llama-3.1-Minitron-4B-Width-Base. It is a base model, so the task is plain
continuation: given a passage, produce the next tokens. Its decoder has the
same parts as chapter 81's Ministral (RMSNorm, grouped-query attention, a
SwiGLU block), so this chapter reuses chapter 81's norm, BF16 projections,
embedding gather and readout from `psionic/lib/ministral.coax` and adds what
Minitron changes: the rotary rule, the pair layout, a separate output head,
the tokenizer, and a key-value cache.

## The pinned files

The files come from
`https://huggingface.co/nvidia/Llama-3.1-Minitron-4B-Width-Base` at revision
`5205ef7d36204947e3b973cb8b147a816ccd7e6a`. The model is released under the
NVIDIA Open Model License Agreement (June 2024); its `NOTICE` adds that
Llama 3.1 is licensed under the Llama 3.1 Community License. The weights are
two BF16 shards: `model-00001-of-00002.safetensors` (4,978,354,640 bytes,
SHA-256 `585da9fc…9bff3`) and `model-00002-of-00002.safetensors`
(4,047,172,128 bytes, SHA-256 `5ec2029c…dbe49`), both matching the hashes
Hugging Face publishes for them. They stay outside the repository in
`~/models/llama-3.1-minitron-4b-width-base-5205ef7d/`, with a `MANIFEST` that
records the source, revision, licence terms, the Llama notice and the
SHA-256 of every file. The `.nemo` archive was not downloaded, and nothing
from the download runs.

A tooling script, `psionic/tests/minitron_reference.py pin`, hashes the
files, checks `config.json` field by field, and writes the tensor inventory
of both shards (names, dtypes and dimensions, no weights) to
`psionic/fixtures/minitron-manifest.coax`. It reads only the safetensors
headers.

`config.json` gives the decoder:

<!-- source: psionic/lib/minitron.coax shape -->
```jai
// Llama-3.1-Minitron-4B-Width-Base, from config.json: 32 layers of width
// 3072, 32 query heads and 8 key-value heads of head_dim 128 (read from the
// config, not 3072 / 32 = 96), a SwiGLU block of 9216 and a 128,256-entry
// vocabulary with a separate output head.
LM_4B :: MsShape.{width=3072,layers=32,heads=32,kv_heads=8,head_dim=128,hidden=9216,vocab=128256};
LM_THETA :: 500000.0;
LM_FACTOR :: 8.0;
LM_LOW_FACTOR :: 1.0;
LM_HIGH_FACTOR :: 4.0;
LM_ORIGINAL :: 8192.0;
LM_BOS :: 128000;
LM_EOS :: 128001;
LM_POSITIONS_MAX :: 64;
```

The head dimension is a field of its own. Dividing the width by the heads
gives 3072 / 32 = 96, but the checkpoint's heads are 128 wide: the query
projection has 32 x 128 = 4,096 rows, more than the width. A shape built by
division asks for a 768-row key projection and is refused, as the second
example below shows.

The expected tensors follow from the shape: `lm_head.weight`,
`model.embed_tokens.weight`, `model.norm.weight`, and nine tensors a layer
under Hugging Face's Llama names. Unlike Ministral, the head is its own
128,256 x 3,072 matrix, not the embedding again:

<!-- source: psionic/lib/minitron.coax inventory -->
```jai
// The expected tensors: e in 0..<3+9*layers, Hugging Face's Llama names.
LM_LAYER_NAMES :: [9]string.[".input_layernorm.weight",".mlp.down_proj.weight",".mlp.gate_proj.weight",".mlp.up_proj.weight",
    ".post_attention_layernorm.weight",".self_attn.k_proj.weight",".self_attn.o_proj.weight",".self_attn.q_proj.weight",".self_attn.v_proj.weight"];
lm_expected :: (s:MsShape,e:int)->MsTensor {
    if e==0 {return MsTensor.{name="lm_head.weight",layer=-1,rank=2,rows=s.vocab,cols=s.width};}
    if e==1 {return MsTensor.{name="model.embed_tokens.weight",layer=-1,rank=2,rows=s.vocab,cols=s.width};}
    if e==2 {return MsTensor.{name="model.norm.weight",layer=-1,rank=1,rows=s.width};}
    k:=(e-3)%9;t:=MsTensor.{name=LM_LAYER_NAMES[k],layer=(e-3)/9,rank=2,rows=s.width,cols=s.width};
    kv:=s.kv_heads*s.head_dim;q:=s.heads*s.head_dim;
    if k==0 || k==4 {t.rank=1;}
    if k==1 {t.cols=s.hidden;}
    if k==2 || k==3 {t.rows=s.hidden;}
    if k==5 || k==8 {t.rows=kv;}
    if k==6 {t.cols=q;}
    if k==7 {t.rows=q;}
    return t;
}
// "model.layers.L.name BF16 rows[,cols]".
lm_line :: (t:MsTensor,out:[]u8)->int {
    n:=0;if t.layer>=0 {n=ms_put(out,n,"model.layers.");n=ms_put_int(out,n,t.layer);}
    n=ms_put(out,n,t.name);n=ms_put(out,n," BF16 ");n=ms_put_int(out,n,t.rows);
    if t.rank==2 {n=ms_put(out,n,",");n=ms_put_int(out,n,t.cols);}
    return n;
}
// The manifest matches s when it has exactly the expected lines, each once.
// cause names the first expected tensor missing or repeated, or is -1 - lines
// when the line count differs. written counts the parameters.
lm_inventory :: (s:MsShape,manifest:string)->MsResult {
    if !ms_shape_valid(s) {return MsResult.{status=MsStatus.shape};}
    ends:[513]int;lines:=0;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==512 {return MsResult.{status=MsStatus.checkpoint,cause=-1-lines};}
        lines+=1;ends[lines]=i;
    }}
    buf:[160]u8;parameters:=0;expected:=3+9*s.layers;
    for e:0..<expected {
        t:=lm_expected(s,e);n:=lm_line(t,buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {return MsResult.{status=MsStatus.checkpoint,cause=e};}
        size:=t.rows;if t.rank==2 {size*=t.cols;}parameters+=size;
    }
    if expected!=lines {return MsResult.{status=MsStatus.checkpoint,cause=-1-lines};}
    return MsResult.{written=parameters};
}
```

The pinned file holds exactly 291 tensors, 3 + 9 x 32, with 4,512,746,496
parameters: 2 x 128,256 x 3,072 for the embedding and the head, 3,072 for the
final norm, and per layer 2 x 3,072 (norms) + 2 x 4,096 x 3,072 (query and
output) + 2 x 1,024 x 3,072 (key and value) + 3 x 9,216 x 3,072 (SwiGLU).

## The tokenizer

Llama 3 splits text with one pattern, then merges bytes. On ASCII text its
Unicode classes reduce to letters A-Z and a-z, digits 0-9, whitespace (tab to
carriage return, and space) and everything else. Three things differ from
chapter 81's Tekken pattern: contractions such as `'ll` come first in any
case, a word is any run of letters with no upper/lower split, and digits go
in groups of up to three.

<!-- source: psionic/lib/minitron.coax pieces -->
```jai
// The Llama 3 split pattern on ASCII text, alternatives in order:
//   (?i:'s|'t|'re|'ve|'m|'ll|'d)   a contraction, any case
//   [^\r\n\p{L}\p{N}]?\p{L}+       one optional non-letter, non-digit,
//                                  non-newline, then letters
//   \p{N}{1,3}                     one to three digits
//   " ?[^\s\p{L}\p{N}]+[\r\n]*"     punctuation, then newlines
//   \s*[\r\n]+ | \s+(?!\S) | \s+   whitespace, as in chapter 81
lm_letter :: (c:u8)->bool {k:=ms_class(c);return k==MS_UPPER || k==MS_LOWER;}
lm_letters :: (s:string,at:int)->int {p:=at;while p<s.count && lm_letter(s[p]) {p+=1;}return p;}
lm_lower :: (c:u8)->u8 {if ms_class(c)==MS_UPPER {return c+32;}return c;}
LM_CONTRACTIONS :: [7]string.["s","t","re","ve","m","ll","d"];
lm_contraction :: (s:string,at:int)->int {
    if s[at]!='\'' {return at;}
    for c:LM_CONTRACTIONS {
        if at+c.count<s.count {
            same:=true;for i:0..<c.count {if lm_lower(s[at+1+i])!=c[i] {same=false;}}
            if same {return at+1+c.count;}
        }
    }
    return at;
}
lm_piece :: (s:string,at:int)->int {
    end:=lm_contraction(s,at);if end>at {return end;}
    c:=ms_class(s[at]);p:=at;
    if c==MS_OTHER || (c==MS_SPACE && !ms_newline(s[at])) {p+=1;}
    end=lm_letters(s,p);if end>p {return end;}
    if c==MS_DIGIT {return nx_min(ms_run(s,at,MS_DIGIT),at+3);}
    p=at;if s[p]==' ' {p+=1;}
    end=ms_run(s,p,MS_OTHER);
    if end>p {while end<s.count && ms_newline(s[end]) {end+=1;}return end;}
    run:=ms_run(s,at,MS_SPACE);last:=at;
    for i:at..<run {if ms_newline(s[i]) {last=i+1;}}
    if last>at {return last;}
    if run==s.count || run-at==1 {return run;}
    return run-1;
}
```

By hand, `The capital of France is` splits into `The`, ` capital`, ` of`,
` France` and ` is`: each piece is a letter run, the four later ones with the
space before them taken as the optional first character. Each piece is a
whole entry of the vocabulary, so each is one token: 791, 6864, 315, 9822 and
374. The tokenizer's template puts `<|begin_of_text|>` (128000) first, so the
prompt is six ids. In `I'll say it: DON'T`, the contraction rule makes `'ll`
and `'T` their own pieces, and `DON` stays one word.

The merging is chapter 81's tiktoken rule over original ranks (`ms_bpe`).
Llama 3's ids are its ranks, so the 1,000 special ids chapter 81 adds come
off again:

<!-- source: psionic/lib/minitron.coax encode -->
```jai
// ASCII text to ids: split into pieces, then chapter 81's tiktoken rule over
// original ranks. Llama 3's ids are its ranks, so chapter 81's offset of 1000
// special ids comes off. lm_prompt puts <|begin_of_text|> first, as the
// tokenizer's template does.
lm_encode :: (t:MsTable,text:string,out:[]int)->MsResult {
    for i:0..<text.count {if text[i]>=128 {return MsResult.{status=MsStatus.unsupported,cause=i};}}
    at:=0;n:=0;
    while at<text.count {
        end:=lm_piece(text,at);
        r:=ms_bpe(t,text[at..<end],out[n..<out.count]);
        if r.status!=MsStatus.ok {r.cause+=at;return r;}
        for i:n..<n+r.written {out[i]-=MS_SPECIALS;}
        n+=r.written;at=end;
    }
    return MsResult.{written=n};
}
lm_prompt :: (t:MsTable,text:string,out:[]int)->MsResult {
    if out.count<1 {return MsResult.{status=MsStatus.capacity};}
    out[0]=LM_BOS;r:=lm_encode(t,text,out[1..<out.count]);
    if r.status==MsStatus.ok {r.written+=1;}
    return r;
}
```

The fixture table is the vocabulary restricted to the fixture strings'
pieces (334 entries). The independent check is a different algorithm over
different data: the reference script runs Hugging Face's merge-list rule
(byte-level letters, `ignore_merges`, the earliest of 280,147 merges first)
over the full `tokenizer.json`, and the Coax ids must equal its ids on all
nine fixture strings.

## Positions

Llama 3.1 scales the rotary frequencies f_i = 500000^(-2i/d) by wavelength,
2 pi / f_i, against an original context of 8,192 positions:

<!-- source: psionic/lib/minitron.coax rope -->
```jai
// llama3 scaling of f = theta^(-2i/d): a wavelength 2 pi / f shorter than
// 8192 / 4 = 2048 positions keeps its frequency, one longer than 8192 / 1
// divides it by 8, and one between blends the two by
// smooth = (8192 / wavelength - 1) / (4 - 1). Clamping smooth to [0, 1]
// gives all three bands from one formula: (1 - smooth) f / 8 + smooth f is
// exactly f at 1 and exactly f / 8 at 0. min(a, b) = (a + b - |a - b|) / 2
// and max(a, 0) = (a + |a|) / 2 do the clamping without a branch.
lm_rope_frequencies :: (d:int,out:[]f64) {
    for i:0..<d/2 {
        f:=exp(-log(LM_THETA)*f64(2*i)/f64(d));wavelength:=2*3.141592653589793/f;
        smooth:=(LM_ORIGINAL/wavelength-LM_LOW_FACTOR)/(LM_HIGH_FACTOR-LM_LOW_FACTOR);
        below:=(smooth+1-abs(smooth-1))/2;clamped:=(below+abs(below))/2;
        out[i]=(1-clamped)*f/LM_FACTOR+clamped*f;
    }
}
// Hugging Face's Llama layout pairs element i of a head with element i + d/2
// (rotate-half), where chapter 81's Mistral layout pairs 2i with 2i + 1.
lm_rotate :: (v:[]f32,heads:int,d:int,position:int,freqs:[]f64) {
    half:=d/2;
    for h:0..<heads {for i:0..<half {
        a:=f64(v[h*d+i]);b:=f64(v[h*d+half+i]);angle:=f64(position)*freqs[i];c:=cos(angle);n:=sin(angle);
        v[h*d+i]=f32(a*c-b*n);v[h*d+half+i]=f32(b*c+a*n);
    }}
}
```

By hand for d = 128, f_i = 500000^(-i/64) and the wavelength is
2 pi x 500000^(i/64):

| i | wavelength | band | smooth | frequency |
| --- | --- | --- | --- | --- |
| 28 | 1,956 | kept (below 2,048) | 1 | f_28 = 0.0032114 |
| 29 | 2,402 | blended | (8192 / 2401.7 - 1) / 3 = 0.8036 | 0.1964 f / 8 + 0.8036 f = 0.8282 f = 0.0021666 |
| 35 | 8,219 | divided (above 8,192) | 0 | f_35 / 8 |

The example prints f28 = 0.0032114459947525913 and f29 =
0.00216657076350336, and the checks compare nine frequencies across the three
bands with the float64 reference.

The other difference is the pair layout. Hugging Face stores Llama's query
and key projections so that element i of a head turns with element i + 64
(rotate-half). Chapter 81's Mistral layout turns 2i with 2i + 1. The same
weights through the two layouts give different layers from the second
position on; a check runs both on the synthetic model to show they differ.

## The key-value cache

Generating one token at a time, a decoder would recompute every earlier
position's keys and values at each step. A cache keeps them: for each layer,
the rotated keys and the values of every position seen so far.

<!-- source: psionic/lib/minitron.coax cache -->
```jai
// The key-value cache: for every layer, `capacity` positions of rotated keys
// and of values, kv_heads * head_dim each. Position p of layer l starts at
// (l * capacity + p) * kv_heads * head_dim. The caller keeps the length.
LmCache :: struct {keys:[]f32;values:[]f32;capacity:int;}
lm_cache_size :: (s:MsShape,capacity:int)->int #export_c {return s.layers*capacity*s.kv_heads*s.head_dim;}
lm_cache_fits :: (s:MsShape,c:LmCache)->bool {
    n:=lm_cache_size(s,c.capacity);
    return c.capacity>=1 && c.capacity<=LM_POSITIONS_MAX && c.keys.count==n && c.values.count==n;
}
```

The layer step takes `n` new tokens at positions `start..<start+n`. It
writes their keys and values into the cache, then lets each new token attend
over every cached position up to its own:

<!-- source: psionic/lib/minitron.coax step -->
```jai
// One decoder layer over n new tokens at positions start..<start+n. Their
// keys and values go into the cache first; each new token then attends over
// every cached position up to its own. Prefilling a prompt is one call with
// start 0; each generated token is a call with n 1. Both run the same
// arithmetic in the same order, so their outputs agree bit for bit.
//   h = norm(x); q, k, v = wq h, wk h, wv h; rotate q and k at their positions
//   a = softmax(q k / sqrt(d)) v, query head j reading key-value head
//       j / (heads / kv_heads); scores are shifted by their maximum,
//       max(a, b) = (a + b + |a - b|) / 2, before exponentiating, since
//       layer 1 of the real model has scores that overflow exp unshifted
//   x += wo a; x += w2 (silu(w1 norm(x)) * w3 norm(x))
lm_layer_step :: (s:MsShape,w:MsLayer,c:LmCache,layer:int,start:int,x:[]f32,n:int,out:[]f32,work:[]f32)->MsResult #export_c {
    if !ms_shape_valid(s) || !ms_layer_sizes(s,w) || !lm_cache_fits(s,c) || layer<0 || layer>=s.layers || n<1 || start<0 || start+n>c.capacity {return MsResult.{status=MsStatus.shape};}
    d:=s.width;hd:=s.head_dim;q:=s.heads*hd;kv:=s.kv_heads*hd;group:=s.heads/s.kv_heads;base:=layer*c.capacity*kv;
    if x.count!=n*d || out.count<n*d || work.count<n*q {return MsResult.{status=MsStatus.capacity};}
    freqs:[128]f64;lm_rope_frequencies(hd,freqs[..]);
    hb:[8192]f32;ob:[8192]f32;ab:[16384]f32;gb:[32768]f32;ub:[32768]f32;sb:[64]f64;sg:[32768]f32;stage:=sg[..];
    h:=hb[0..<d];o:=ob[0..<d];a:=ab[0..<q];gate:=gb[0..<s.hidden];up:=ub[0..<s.hidden];
    for t:0..<n {
        p:=start+t;qs:=work[t*q..<(t+1)*q];ks:=c.keys[base+p*kv..<base+(p+1)*kv];vs:=c.values[base+p*kv..<base+(p+1)*kv];
        if ms_norm(x[t*d..<(t+1)*d],w.attention_norm,h)!=MsStatus.ok {return MsResult.{status=MsStatus.number,cause=1};}
        r:=ms_project(w.wq,q,d,h,qs,stage);if r.status!=MsStatus.ok {return r;}
        r=ms_project(w.wk,kv,d,h,ks,stage);if r.status!=MsStatus.ok {return r;}
        r=ms_project(w.wv,kv,d,h,vs,stage);if r.status!=MsStatus.ok {return r;}
        lm_rotate(qs,s.heads,hd,p,freqs[..]);lm_rotate(ks,s.kv_heads,hd,p,freqs[..]);
    }
    for t:0..<n {
        p:=start+t;
        for head:0..<s.heads {
            g:=head/group;top:=0.0;
            for u:0..<p+1 {
                acc:PsionicAccumulator;for j:0..<hd {acc=psionic_ops_accumulate(acc,f64(work[t*q+head*hd+j])*f64(c.keys[base+u*kv+g*hd+j]));}
                sb[u]=psionic_ops_total(acc)/sqrt(f64(hd));top=ifx u==0 then sb[u] else (top+sb[u]+abs(top-sb[u]))/2;
            }
            z:PsionicAccumulator;for u:0..<p+1 {sb[u]=exp(sb[u]-top);z=psionic_ops_accumulate(z,sb[u]);}
            total:=psionic_ops_total(z);
            for j:0..<hd {
                acc:PsionicAccumulator;for u:0..<p+1 {acc=psionic_ops_accumulate(acc,sb[u]/total*f64(c.values[base+u*kv+g*hd+j]));}
                a[head*hd+j]=f32(psionic_ops_total(acc));
            }
        }
        r:=ms_project(w.wo,d,q,a,o,stage);if r.status!=MsStatus.ok {return r;}
        for i:0..<d {out[t*d+i]=f32(f64(x[t*d+i])+f64(o[i]));}
        if ms_norm(out[t*d..<(t+1)*d],w.ffn_norm,h)!=MsStatus.ok {return MsResult.{status=MsStatus.number,cause=2};}
        r=ms_project(w.w1,s.hidden,d,h,gate,stage);if r.status!=MsStatus.ok {return r;}
        r=ms_project(w.w3,s.hidden,d,h,up,stage);if r.status!=MsStatus.ok {return r;}
        for i:0..<s.hidden {gv:=f64(gate[i]);gate[i]=f32(gv/(1+exp(-gv))*f64(up[i]));}
        r=ms_project(w.w2,d,s.hidden,gate,o,stage);if r.status!=MsStatus.ok {return r;}
        for i:0..<d {out[t*d+i]=f32(f64(out[t*d+i])+f64(o[i]));}
    }
    return MsResult.{written=n*d};
}
```

A prompt is one call with `start` 0. Each generated token is a call with one
token at the next position. The per-token arithmetic is the same either way
and in the same order, so a cached step's output equals the full
recompute's bit for bit. That makes the check exact rather than toleranced:
the synthetic model run one token at a time, as a prefill of four followed
by three and then single tokens, and as one call over all ten, gives
identical logits, and a cache full of another sequence's values is
overwritten position by position.

The step subtracts each row's maximum score before exponentiating, where
chapter 81's layer does not. A check scales the synthetic model's attention
gains by 256, which spreads its scores thousands apart and overflows `exp`
without the shift; with it the outputs are finite and the cached and full
runs still agree.

The head is read in chunks, because chapter 7's projection takes at most
32,768 rows. Chapter 81 only ever read a 4,096-row chunk, so the limit first
showed here, when the first whole-model run refused at the readout:

<!-- source: psionic/lib/minitron.coax readout -->
```jai
// Chapter 7's projection takes at most 32,768 rows, so the 128,256-row head
// is read in chunks of `rows`, at most that limit: three of 32,768 and one
// of 29,952. cause names the first row of a refused chunk.
LM_READOUT_ROWS :: 32768;
lm_readout :: (s:MsShape,head:[]u8,norm:[]u8,hidden:[]f32,logits:[]f32,work:[]f32,rows:int)->MsResult {
    if !qm_valid(1,rows,s.width) || head.count!=2*s.vocab*s.width || logits.count<s.vocab {return MsResult.{status=MsStatus.shape};}
    first:=0;
    while first<s.vocab {
        n:=nx_min(rows,s.vocab-first);
        r:=ms_readout(s,head[2*first*s.width..<2*(first+n)*s.width],n,norm,hidden,logits[first..<first+n],work);
        if r.status!=MsStatus.ok {r.cause=first;return r;}
        first+=n;
    }
    return MsResult.{written=s.vocab};
}
```

A check reads the synthetic head in every chunk size from 1 to 16 and gets
the same bits each time.

The whole decoder runs the layers in turn and reads the last token's logits
through the final norm and the separate head:

<!-- source: psionic/lib/minitron.coax decode -->
```jai
// The whole decoder over new tokens at positions start..<start+tokens.count,
// and the logits of the last one through the final norm and the untied head.
// hidden holds two residual buffers of count x width; work is lm_work.
LmModel :: struct {embed:[]u8;norm:[]u8;head:[]u8;layers:[]MsLayer;}
lm_work :: (s:MsShape,count:int)->int #export_c {
    a:=count*s.heads*s.head_dim;b:=s.width+s.vocab;return a+b-nx_min(a,b);
}
lm_decode :: (s:MsShape,m:LmModel,c:LmCache,start:int,tokens:[]int,hidden:[]f32,logits:[]f32,work:[]f32)->MsResult #export_c {
    n:=tokens.count*s.width;
    if m.layers.count!=s.layers {return MsResult.{status=MsStatus.shape};}
    if hidden.count<2*n || work.count<lm_work(s,tokens.count) {return MsResult.{status=MsStatus.capacity};}
    r:=ms_embed(s,m.embed,tokens,hidden[0..<n]);if r.status!=MsStatus.ok {return r;}
    from:=0;
    for l:0..<s.layers {
        to:=n-from;r=lm_layer_step(s,m.layers[l],c,l,start,hidden[from..<from+n],tokens.count,hidden[to..<to+n],work);
        if r.status!=MsStatus.ok {r.cause=l;return r;}
        from=to;
    }
    r=lm_readout(s,m.head,m.norm,hidden[from+n-s.width..<from+n],logits,work,LM_READOUT_ROWS);
    if r.status!=MsStatus.ok {return r;}
    return MsResult.{written=start+tokens.count};
}
```

## Continuing

Continuation prefills the prompt, then emits tokens through chapter 53's
`nx_step`: temperature 0 picks the largest logit (the lowest id on a tie);
any other temperature draws one counter of the seed per token. The run stops
at a stop token or after `steps` tokens; a token that ends the run is not
decoded, so a prompt of p tokens needs p + steps - 1 cache positions.

<!-- source: psionic/lib/minitron.coax continue -->
```jai
// Prefill the prompt, then emit up to `steps` tokens with chapter 53's
// nx_step: temperature 0 is greedy, anything else draws one counter of the
// seed per token. Each emitted token is decoded at the next position unless
// the run has stopped. out receives the emitted ids; written is their count
// and cause the final cache length.
LmSampler :: struct {options:NxOptions;seed:CrSeed;stops:[]int;probabilities:[]f64;work:NxWork;}
lm_continue :: (s:MsShape,m:LmModel,c:LmCache,prompt:[]int,steps:int,k:LmSampler,out:[]int,hidden:[]f32,logits:[]f32,work:[]f32)->MsResult #export_c {
    if steps<1 || out.count<steps || prompt.count+steps-1>c.capacity {return MsResult.{status=MsStatus.capacity,cause=prompt.count+steps-1};}
    r:=lm_decode(s,m,c,0,prompt,hidden,logits,work);if r.status!=MsStatus.ok {return r;}
    length:=r.written;run:=NxRun.{random=k.seed,limit=steps};
    while run.stop==NxStop.running {
        none:[1]u8;e:=nx_step(logits[0..<s.vocab],none[0..<0],k.options,run,k.stops,false,k.probabilities,k.work,false);
        if e.status!=NxStatus.ok {return MsResult.{status=MsStatus.number,written=run.emitted,cause=cast(int)e.status};}
        out[run.emitted]=e.token;run=e.run;
        if run.stop==NxStop.running {
            one:=out[run.emitted-1..<run.emitted];
            r=lm_decode(s,m,c,length,one,hidden,logits,work);if r.status!=MsStatus.ok {r.written=run.emitted;return r;}
            length=r.written;
        }
    }
    return MsResult.{written=run.emitted,cause=length};
}
```

The synthetic model is chapter 81's two-layer model at width 8 with an
untied head:

<!-- source: psionic/lib/minitron.coax tiny -->
```jai
// Chapter 81's tiny model (width 8, 4 query heads over 2 key-value heads of
// 4, SwiGLU of 12, 16 tokens, 2 layers, the same weight formula) with an
// untied head from formula 98. At head_dim 4 its two frequencies, 1 and
// 500000^(-1/2) (wavelength 4,443), fall in the kept and the blended band.
LM_TINY_HEAD_BYTES :: 2*16*8;
lm_tiny_model :: (bytes:[]u8,head:[]u8,layers:[]MsLayer)->LmModel {
    t:=ms_tiny_weights(bytes);filled:=ms_tiny_fill(head,0,16*8,98,false);#assert filled==LM_TINY_HEAD_BYTES;
    layers[0]=t.layers[0];layers[1]=t.layers[1];
    return LmModel.{embed=t.embed,norm=t.norm,head=head,layers=layers};
}
```

A check runs its greedy continuation and the same chain by full recomputes
(the argmax of the last logits each time): all 12 tokens agree. Seed 58 run
twice gives the same tokens; seed 59, and seed 58 starting at counter 1,
give different ones.

## The program

The programs are [`minitron.coax`](../examples/minitron.coax) and
[`minitron-refuse.coax`](../examples/minitron-refuse.coax); the library is
[`minitron.coax`](../lib/minitron.coax).

<!-- source: psionic/examples/minitron.coax run -->
```jai
main :: () {
    r:=lm_inventory(LM_4B,LM_PINNED_MANIFEST);
    print("checkpoint %: % tensors, % parameters: %\n",cast(string)LM_PINNED_REVISION[0..<8],LM_PINNED_TENSORS,r.written,r.status);

    tb:=LM_TOKEN_BYTES;tr:=LM_TOKEN_RANKS;t:=MsTable.{bytes=tb[..],ranks=tr[..]};ids:[64]int;
    p:=lm_prompt(t,"The capital of France is",ids[..]);print("prompt (% tokens):",p.written);
    for i:0..<p.written {print(" %",ids[i]);}print("\n");
    e:=lm_encode(t,"I'll say it: DON'T stop, they're HERE.",ids[..]);print("contractions:");
    for i:0..<e.written {print(" %",ids[i]);}print("\n");

    f:[64]f64;lm_rope_frequencies(128,f[..]);
    print("llama3: f28 % (kept) f29 % (blended) f35*8 % (divided)\n",f[28],f[29],f[35]*8);

    s:=ms_tiny();g:LmTinyRig;m:=lm_tiny_model(g.bytes[..],g.head[..],g.layers[..]);c:=lm_tiny_cache(g.keys[..],g.values[..]);
    tokens:=LM_TINY_TOKENS;hidden:[256]f32;work:[256]f32;full:[16]f32;step:[16]f32;worst:=0.0;
    for k:0..<10 {
        d:=lm_decode(s,m,c,k,tokens[k..<k+1],hidden[..],step[..],work[..]);if d.status!=MsStatus.ok {print("refused %\n",d);return;}
        for i:0..<16 {x:=abs(f64(step[i])-LM_TINY_LOGITS[k*16+i]);if x>worst {worst=x;}}
    }
    d:=lm_decode(s,m,c,0,tokens[..],hidden[..],full[..],work[..]);same:=d.status==MsStatus.ok;
    for i:0..<16 {if full[i]!=step[i] {same=false;}}
    print("tiny: 10 cached steps against one full pass: identical bits %, within 2e-6 of float64 %\n",same,worst<2e-6);

    st:[16]f64;o:[16]int;tm:[16]int;probs:[16]f64;nostop:[1]int;out:[8]int;
    k:=LmSampler.{options=NxOptions.{temperature=0},stops=nostop[0..<0],probabilities=probs[..],work=nx_work(st[..],o[..],tm[..])};
    for run:0..<4 {
        if run==1 {k.options=NxOptions.{temperature=1};k.seed=CrSeed.{seed=58};}
        if run==3 {k.seed=CrSeed.{seed=59};}
        n:=lm_continue(s,m,c,tokens[0..<3],8,k,out[..],hidden[..],step[..],work[..]);
        print("tiny 1 3 7, temperature % seed %:",k.options.temperature,k.seed.seed);
        for i:0..<n.written {print(" %",out[i]);}print("\n");
    }
}
```

```shell
coax/bin/coax run psionic/examples/minitron.coax
```

```text
checkpoint 5205ef7d: 291 tensors, 4512746496 parameters: ok
prompt (6 tokens): 128000 791 6864 315 9822 374
contractions: 40 3358 2019 433 25 45373 17773 3009 11 814 2351 19804 13
llama3: f28 0.0032114459947525913 (kept) f29 0.00216657076350336 (blended) f35*8 0.000764496988317175 (divided)
tiny: 10 cached steps against one full pass: identical bits true, within 2e-6 of float64 true
tiny 1 3 7, temperature 0.0 seed 0: 4 4 4 4 4 4 4 4
tiny 1 3 7, temperature 1.0 seed 58: 7 6 1 7 12 5 1 5
tiny 1 3 7, temperature 1.0 seed 58: 7 6 1 7 12 5 1 5
tiny 1 3 7, temperature 1.0 seed 59: 10 9 12 10 13 10 1 10
```

The tiny model's greedy continuation repeats one token; at width 8 it has
little to say. The seeded runs show the same seed and counter giving the
same eight tokens, and another seed giving others.

## Refusals

<!-- source: psionic/examples/minitron-refuse.coax refuse -->
```jai
main :: () {
    buf:[160]u8;
    d:=LM_4B;d.head_dim=d.width/d.heads;r:=lm_inventory(d,LM_PINNED_MANIFEST);n:=lm_line(lm_expected(d,r.cause),buf[..]);
    print("head_dim 3072/32 = %: %, first missing: %\n",d.head_dim,r.status,cast(string)buf[0..<n]);
    d=LM_4B;d.layers=26;r=lm_inventory(d,LM_PINNED_MANIFEST);print("26 layers: %\n",r);

    tb:=LM_TOKEN_BYTES;tr:=LM_TOKEN_RANKS;t:=MsTable.{bytes=tb[..],ranks=tr[..]};ids:[64]int;
    print("non-ASCII text: %\n",lm_prompt(t,"café",ids[..]));
    print("a byte outside the fixture table: %\n",lm_encode(t,"is ~",ids[..]));

    s:=ms_tiny();g:LmTinyRig;m:=lm_tiny_model(g.bytes[..],g.head[..],g.layers[..]);c:=lm_tiny_cache(g.keys[..],g.values[..]);
    tokens:=LM_TINY_TOKENS;hidden:[256]f32;work:[256]f32;logits:[16]f32;
    print("2 tokens at position 15 of a 16-position cache: %\n",lm_decode(s,m,c,15,tokens[0..<2],hidden[..],logits[..],work[..]));
    bad:=[2]int.[3,16];print("token 16 of 16: %\n",lm_decode(s,m,c,0,bad[..],hidden[..],logits[..],work[..]));
    st:[16]f64;o:[16]int;tm:[16]int;probs:[16]f64;nostop:[1]int;out:[16]int;
    k:=LmSampler.{options=NxOptions.{temperature=0},stops=nostop[0..<0],probabilities=probs[..],work=nx_work(st[..],o[..],tm[..])};
    print("6 prompt tokens and 12 steps in 16 positions: %\n",lm_continue(s,m,c,tokens[0..<6],12,k,out[..],hidden[..],logits[..],work[..]));
    k.options=NxOptions.{temperature=-1};
    print("temperature -1: %\n",lm_continue(s,m,c,tokens[0..<3],2,k,out[..],hidden[..],logits[..],work[..]));
}
```

```shell
coax/bin/coax run psionic/examples/minitron-refuse.coax
```

```text
head_dim 3072/32 = 96: checkpoint, first missing: model.layers.0.self_attn.k_proj.weight BF16 768,3072
26 layers: MsResult.{status = checkpoint, written = 0, cause = -292}
non-ASCII text: MsResult.{status = unsupported, written = 0, cause = 3}
a byte outside the fixture table: MsResult.{status = range, written = 0, cause = 3}
2 tokens at position 15 of a 16-position cache: MsResult.{status = shape, written = 0, cause = 0}
token 16 of 16: MsResult.{status = range, written = 0, cause = 1}
6 prompt tokens and 12 steps in 16 positions: MsResult.{status = capacity, written = 0, cause = 17}
temperature -1: MsResult.{status = number, written = 0, cause = 1}
```

Both programs exit with status 0, and their transcripts are compared exactly
in debug and release builds.

## The original weights

The examples run without the 9 GB of weights. A script runs the
original-weight slices:

```shell
COAX_CC=gcc psionic/tests/check_minitron_slices.sh ~/models/llama-3.1-minitron-4b-width-base-5205ef7d
```

It checks the pins and fixtures, compiles `psionic/tests/minitron_run.c`
(which maps the two shards and finds tensors with the Coax inventory reader;
all arithmetic is Coax) and runs the prompt's six tokens through the
embedding gather, layer 0 in one call, layer 0 again through the cache (four
tokens, then one, then one), and `lm_head` rows 0 to 4,095 for the last
token. The reference recomputes each slice in binary64 from the same bytes.
The tolerances were frozen before the first comparison: the gather is exact,
and layer 0 and the readout chunk may differ by at most 10^-4 of the slice's
largest magnitude. A recorded run:

```text
6 tokens: [128000, 791, 6864, 315, 9822, 374]
slice embed: 6 tokens, 0.000 s
slice layer0: 6 tokens in one call, 7.356 s
slice layer0 through the cache (4, then 1, then 1): identical bits
slice readout: lm_head rows 0..4096, 0.076 s
embed: 6 tokens, exact
layer 0: 6 tokens, max |difference| 2.055e-07, largest |value| 2.306e+00
readout rows 0..4096 of lm_head: max |difference| 3.252e-07, largest |value| 8.253e+00
```

With `--continue`, the script runs all 32 layers instead: a greedy
continuation of eight tokens followed by a full recompute of the last
logits from position 0, and a seeded continuation (temperature 1, seed 58)
alongside it. This takes minutes and is not a routine check; its output is
recorded here, not rerun:

```text
prompt: 128000 791 6864 315 9822 374
temperature 0 seed 0: 8 tokens, cache length 13, 143.0 s: 12366 13 578 4033 4221 374 8753 13
full recompute over 13 tokens, 128.0 s: last logits identical bits to the cached run
prompt: 128000 791 6864 315 9822 374
temperature 1 seed 58: 8 tokens, cache length 13, 144.5 s: 12366 13 578 4900 320 2127 453 6456
```

Read through `tokenizer.json`, the greedy tokens are ` Paris. The official
language is French.` and the seeded ones ` Paris. The President (An
althern`: both start with the most likely token, then the seeded run draws
lower-ranked ones. The full recompute over the prompt and the first seven
greedy tokens gives the same last logits, bit for bit, as the cached run that
drew the eighth. The cache made the continuation cheaper: 13 positions cost
143 seconds through the cache, while recomputing all 13 once for the
comparison cost 128 seconds, and recomputing at every step would cost the sum
over steps.

The whole-model outputs have no independent reference: the float64
reference script is pure Python and computes one layer over six tokens in
about a minute, too slow for 32 layers. What checks them is the cache's
bit-for-bit agreement with the full recompute, the layer 0 and readout
slices against the reference, and the plausibility of the text.

## What remains

- An independent reference for whole-model logits on recorded prompts.
- The tokenizer's full Unicode pattern; text outside ASCII is refused.
- Positions past 64 (the cache's bound here), where llama3 scaling starts
  to matter beyond the first few thousand positions.
- Throughput: one token through all 32 layers takes about 11 seconds on one
  core.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md):
  Llama-3.1-Minitron-4B-Width-Base.
- [Chapter 81](ch081-execute-ministral-4.md): the shared dense decoder.
- [Chapter 53](ch053-choosing-the-next-token.md): the sampler.
