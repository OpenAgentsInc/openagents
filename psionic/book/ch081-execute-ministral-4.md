# 81. Execute Ministral 4

> **Status: partly implemented.** The pinned checkpoint's inventory, Tekken
> and the chat format on ASCII text, the YaRN frequencies, and one decoder
> layer and a readout chunk on the original weights run on the CPU and match
> independent references. Tekken's full Unicode pattern, all 26 layers on the
> original weights with a recorded reference, greedy tokens from the real
> model, positions past 16,384 and the reasoning format remain.

This chapter opens the Mistral ladder with its smallest model. The plan names
the chapter "Ministral 4"; the pinned target is
Ministral-3-3B-Instruct-2512-BF16 (Apache-2.0, BF16 originals). Its text path
is a dense decoder, so it reuses the shared pieces: chapter 7's safetensors
reader, chapter 9's byte-pair idea with Mistral's own tables, and the
grouped-query attention of chapter 10.

## The pinned files

The files come from
`https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-BF16` at
revision `b6d637bef2393152b3da2b2fde72eecdee30557e`. Three are used:
`consolidated.safetensors` (7,698,234,952 bytes, SHA-256 `72243014…a340`),
`tekken.json` (16,753,784 bytes, SHA-256 `600bb279…2843`) and `params.json`.
They stay outside the repository, in a directory with a `MANIFEST` of source,
revision, licence and hashes. A tooling script,
`psionic/tests/ministral_reference.py pin`, hashes them, checks `params.json`
against the sizes below, and writes the tensor inventory (names, dtypes and
dimensions, no weights) to `psionic/fixtures/ministral-manifest.coax`. The
script reads only data from the download; it runs none of its code.

`params.json` gives the text decoder:

<!-- source: psionic/lib/ministral.coax shape -->
```jai
// The text decoder's sizes. Ministral 3 3B: 26 layers of width 3072, 32
// query heads and 8 key-value heads of 128, a SwiGLU block of 9216, and a
// 131,072-entry vocabulary whose embedding is also the readout.
MsShape :: struct {width:int;layers:int;heads:int;kv_heads:int;head_dim:int;hidden:int;vocab:int;}
MS_3B :: MsShape.{width=3072,layers=26,heads=32,kv_heads=8,head_dim=128,hidden=9216,vocab=131072};
// Rotary and normalization constants from params.json.
MS_THETA :: 1000000.0;
MS_FACTOR :: 16.0;
MS_ORIGINAL :: 16384;
MS_BETA_FAST :: 32.0;
MS_BETA_SLOW :: 1.0;
MS_LLAMA4_BETA :: 0.1;
MS_EPSILON :: 0.00001;
MS_SPECIALS :: 1000;
MS_TOKENS_MAX :: 64;
ms_shape_valid :: (s:MsShape)->bool {
    if s.width<1 || s.width>8192 || s.layers<1 || s.hidden<1 || s.hidden>32768 || s.vocab<1 {return false;}
    if s.kv_heads<1 || s.heads<s.kv_heads || s.heads%s.kv_heads!=0 || s.heads>128 {return false;}
    half:=s.head_dim/2;return half*2==s.head_dim && half>=1 && half<=128;
}
```

From the shape, the expected text tensors follow: the final `norm.weight`,
`tok_embeddings.weight`, and nine tensors a layer. The file holds 458
tensors. The other 222 belong to the image path (a vision encoder, its
adapter and patch merger), which this chapter does not run.

Count the text parameters by hand. A layer holds two gains of 3072, `wq` and
`wo` of 4096 × 3072 each, `wk` and `wv` of 1024 × 3072 each, and three SwiGLU
matrices of 9216 × 3072:
2·3072 + 2·12,582,912 + 2·3,145,728 + 3·28,311,552 = 116,398,080. Twenty-six
layers make 3,026,350,080. The embedding adds 131,072 × 3072 = 402,653,184 and
the final gain 3072, for 3,429,006,336. The embedding is tied: it is also the
readout, so there is no `output.weight`.

<!-- source: psionic/lib/ministral.coax inventory -->
```jai
// One expected text tensor: e in 0..<2+9*layers. Layer tensors come nine to
// a layer in a fixed order; all are BF16.
MsTensor :: struct {name:string;layer:int;rank:int;rows:int;cols:int;}
MS_LAYER_NAMES :: [9]string.[".attention.wk.weight",".attention.wo.weight",".attention.wq.weight",".attention.wv.weight",
    ".attention_norm.weight",".feed_forward.w1.weight",".feed_forward.w2.weight",".feed_forward.w3.weight",".ffn_norm.weight"];
ms_expected_count :: (s:MsShape)->int {return 2+9*s.layers;}
ms_expected :: (s:MsShape,e:int)->MsTensor {
    if e==0 {return MsTensor.{name="norm.weight",layer=-1,rank=1,rows=s.width};}
    if e==1 {return MsTensor.{name="tok_embeddings.weight",layer=-1,rank=2,rows=s.vocab,cols=s.width};}
    k:=(e-2)%9;t:=MsTensor.{name=MS_LAYER_NAMES[k],layer=(e-2)/9,rank=2,rows=s.width,cols=s.width};
    q:=s.heads*s.head_dim;kv:=s.kv_heads*s.head_dim;
    if k==0 || k==3 {t.rows=kv;}
    if k==1 {t.cols=q;}
    if k==2 {t.rows=q;}
    if k==4 || k==8 {t.rank=1;}
    if k==5 || k==7 {t.rows=s.hidden;}
    if k==6 {t.cols=s.hidden;}
    return t;
}
ms_put :: (out:[]u8,n:int,s:string)->int {for i:0..<s.count {out[n+i]=s[i];}return n+s.count;}
ms_put_int :: (out:[]u8,n:int,v:int)->int {
    digits:[20]u8;k:=0;x:=v;
    while true {digits[k]=cast(u8)('0'+x%10);k+=1;x/=10;if x==0 {break;}}
    m:=n;while k>0 {k-=1;out[m]=digits[k];m+=1;}
    return m;
}
// The manifest line for t: "layers.L.name BF16 rows[,cols]".
ms_line :: (t:MsTensor,out:[]u8)->int {
    n:=0;if t.layer>=0 {n=ms_put(out,n,"layers.");n=ms_put_int(out,n,t.layer);}
    n=ms_put(out,n,t.name);n=ms_put(out,n," BF16 ");n=ms_put_int(out,n,t.rows);
    if t.rank==2 {n=ms_put(out,n,",");n=ms_put_int(out,n,t.cols);}
    return n;
}
// Every other line must belong to the image path, which this chapter does
// not run.
MS_IMAGE_PREFIXES :: [4]string.["vision_encoder.","vision_language_adapter.","patch_merger.","pre_mm_projector_norm."];
ms_image_line :: (line:string)->bool {
    for p:MS_IMAGE_PREFIXES {if line.count>p.count && line[0..<p.count]==p {return true;}}
    return false;
}
// The manifest (one line a tensor) matches s when every expected text line
// is there exactly once and every other line is an image tensor. cause names
// the first expected tensor missing or repeated, or is -1 - lines when some
// line is neither. written counts the text parameters.
MS_LINES_MAX :: 1024;
ms_inventory :: (s:MsShape,manifest:string)->MsResult {
    if !ms_shape_valid(s) {return MsResult.{status=MsStatus.shape};}
    ends:[1025]int;lines:=0;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==MS_LINES_MAX {return MsResult.{status=MsStatus.checkpoint,cause=-1-lines};}
        lines+=1;ends[lines]=i;
    }}
    buf:[160]u8;text:=0;parameters:=0;
    for e:0..<ms_expected_count(s) {
        t:=ms_expected(s,e);n:=ms_line(t,buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {return MsResult.{status=MsStatus.checkpoint,cause=e};}
        text+=1;size:=t.rows;if t.rank==2 {size*=t.cols;}parameters+=size;
    }
    image:=0;for k:0..<lines {if ms_image_line(manifest[ends[k]+1..<ends[k+1]]) {image+=1;}}
    if text+image!=lines {return MsResult.{status=MsStatus.checkpoint,cause=-1-lines};}
    return MsResult.{written=parameters,cause=image};
}
```

## Tekken

Tekken is a tiktoken-style tokenizer. `tekken.json` lists 130,072 byte
strings, each with a rank; the first 256 are the single bytes. A token's id is
its rank plus 1000, because ids 0 to 999 are control tokens: `<s>` is 1,
`</s>` is 2, `[INST]` is 3, `[/INST]` is 4, `[SYSTEM_PROMPT]` is 17 and
`[/SYSTEM_PROMPT]` is 18.

Encoding has two steps. First a pattern splits the text into pieces. Then
each piece is merged by rank.

The pattern is the one in `tekken.json`:

```text
[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+|[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*|\p{N}| ?[^\s\p{L}\p{N}]+[\r\n/]*|\s*[\r\n]+|\s+(?!\S)|\s+
```

Unlike gpt-oss's pattern (chapter 63), it has no contraction branch and
splits digits one at a time. On ASCII text its Unicode classes reduce to five:
upper case, lower case, digit, space (tab to carriage return, and the space)
and other. This chapter implements the pattern for ASCII text only and refuses
anything else.

<!-- source: psionic/lib/ministral.coax pieces -->
```jai
// Tekken splits text with one pattern before byte-pair merging. This is the
// pattern on ASCII text, where its Unicode classes reduce to: upper A-Z,
// lower a-z, digit 0-9, space (tab to carriage return, and ' '), and other.
MS_UPPER :: 0; MS_LOWER :: 1; MS_DIGIT :: 2; MS_SPACE :: 3; MS_OTHER :: 4;
ms_class :: (c:u8)->int {
    if c>='A' && c<='Z' {return MS_UPPER;}
    if c>='a' && c<='z' {return MS_LOWER;}
    if c>='0' && c<='9' {return MS_DIGIT;}
    if c==' ' || (c>=9 && c<=13) {return MS_SPACE;}
    return MS_OTHER;
}
ms_newline :: (c:u8)->bool {return c==10 || c==13;}
ms_run :: (s:string,at:int,class:int)->int {p:=at;while p<s.count && ms_class(s[p])==class {p+=1;}return p;}
// [^\r\n\p{L}\p{N}]? then letters: upper* lower+ (first alternative) or
// upper+ lower* (second). Both runs are greedy over disjoint classes, so
// whenever either matches, both end at the end of the lower-case run: the
// two together are "upper* lower*, at least one letter". The optional first
// character is taken exactly when it is neither a letter, a digit nor a
// newline; without it the letters could not start.
ms_word :: (s:string,at:int)->int {
    p:=at;c:=ms_class(s[at]);
    if c==MS_OTHER || (c==MS_SPACE && !ms_newline(s[at])) {p+=1;}
    lower:=ms_run(s,ms_run(s,p,MS_UPPER),MS_LOWER);
    if lower>p {return lower;}
    return at;
}
// The end of the piece that starts at `at` (at < s.count).
ms_piece :: (s:string,at:int)->int {
    end:=ms_word(s,at);if end>at {return end;}
    c:=ms_class(s[at]);
    if c==MS_DIGIT {return at+1;}
    // " ?[^\s\p{L}\p{N}]+[\r\n/]*"
    p:=at;if s[p]==' ' {p+=1;}
    end=ms_run(s,p,MS_OTHER);
    if end>p {while end<s.count && (ms_newline(s[end]) || s[end]=='/') {end+=1;}return end;}
    // Whitespace: through the last newline of the run, else all but the last
    // space before a non-space, else the whole run.
    run:=ms_run(s,at,MS_SPACE);last:=at;
    for i:at..<run {if ms_newline(s[i]) {last=i+1;}}
    if last>at {return last;}
    if run==s.count || run-at==1 {return run;}
    return run-1;
}
```

Trace `What is the capital of France?`. At `W` the first alternative takes
one upper-case letter and the lower-case run: `What`. At the space, the space
is the optional first character, so ` is`, ` the`, ` capital`, ` of` and
` France` follow. At `?` no letter follows, so the fourth alternative takes
the punctuation run: `?`. Seven pieces.

Each piece then merges by rank, the rule tiktoken uses: a piece found whole
in the table is one token; otherwise start from single bytes and repeatedly
join the adjacent pair whose joined bytes have the lowest rank, the leftmost
on a tie.

<!-- source: psionic/lib/ministral.coax bpe -->
```jai
// tiktoken's rule over original ranks: a piece found whole is one token;
// otherwise start from single bytes and repeatedly join the adjacent pair
// whose joined bytes have the lowest rank (the leftmost on a tie) until no
// joined pair has a rank. Token ids are ranks plus the 1000 special ids.
MS_PIECE_MAX :: 64;
ms_bpe :: (t:MsTable,piece:string,out:[]int)->MsResult {
    n:=piece.count;if n<1 || n>MS_PIECE_MAX {return MsResult.{status=MsStatus.range,cause=n};}
    whole:=ms_rank(t,piece);
    if whole>=0 {if out.count<1 {return MsResult.{status=MsStatus.capacity};}out[0]=whole+MS_SPECIALS;return MsResult.{written=1};}
    bounds:[65]int;for i:1..<n+1 {bounds[i]=i;}parts:=n;
    while true {
        best:=-1;at:int;
        for i:0..<parts-1 {r:=ms_rank(t,piece[bounds[i]..<bounds[i+2]]);if r>=0 && (best<0 || r<best) {best=r;at=i;}}
        if best<0 {break;}
        for i:at+1..<parts {bounds[i]=bounds[i+1];}parts-=1;
    }
    if out.count<parts {return MsResult.{status=MsStatus.capacity};}
    for i:0..<parts {r:=ms_rank(t,piece[bounds[i]..<bounds[i+1]]);if r<0 {return MsResult.{status=MsStatus.range,cause=bounds[i]};}out[i]=r+MS_SPECIALS;}
    return MsResult.{written=parts};
}
```

A small table shows the tie rule. With `a` at rank 0, `aa` at rank 1 and `b`
at rank 2, the piece `aaa` has two equal pairs, `aa` at 0 and `aa` at 1. The
leftmost joins, giving `aa a`, ids 1001 and 1000.

The Coax side does not load all of `tekken.json`. The fixture
`psionic/fixtures/ministral-tekken.coax` holds Tekken restricted to the
fixture strings: every entry whose bytes occur inside one of their pieces,
282 entries with their original ranks. Merging a piece only ever forms
substrings of that piece, so the restricted table encodes the fixture strings
exactly as the full one does. A byte the restricted table lacks is refused,
not guessed.

<!-- source: psionic/lib/ministral.coax encode -->
```jai
// ASCII text to token ids. Text outside ASCII is refused: the Unicode
// classes of the full pattern are not built yet. cause is the byte offset.
ms_encode :: (t:MsTable,text:string,out:[]int)->MsResult {
    for i:0..<text.count {if text[i]>=128 {return MsResult.{status=MsStatus.unsupported,cause=i};}}
    at:=0;n:=0;
    while at<text.count {
        end:=ms_piece(text,at);
        r:=ms_bpe(t,text[at..<end],out[n..<out.count]);
        if r.status!=MsStatus.ok {r.cause+=at;return r;}
        n+=r.written;at=end;
    }
    return MsResult.{written=n};
}
// Mistral's chat format for one system prompt and one user turn:
// <s> [SYSTEM_PROMPT] system [/SYSTEM_PROMPT] [INST] user [/INST]. The
// control tokens are ids 1, 17, 18, 3 and 4; text never produces them. An
// empty system prompt leaves out its block.
MS_BOS :: 1; MS_EOS :: 2; MS_INST :: 3; MS_INST_END :: 4; MS_SYSTEM :: 17; MS_SYSTEM_END :: 18;
MS_CHAT_MAX :: 1024;
ms_chat :: (t:MsTable,system:string,user:string,out:[]int)->MsResult {
    a:MsResult;sys:[MS_CHAT_MAX]int;usr:[MS_CHAT_MAX]int;extra:=3;
    if system.count>0 {a=ms_encode(t,system,sys[..]);if a.status!=MsStatus.ok {return a;}extra=5;}
    b:=ms_encode(t,user,usr[..]);if b.status!=MsStatus.ok {return b;}
    if out.count<a.written+b.written+extra {return MsResult.{status=MsStatus.capacity,cause=a.written+b.written+extra};}
    n:=0;out[n]=MS_BOS;n+=1;
    if system.count>0 {out[n]=MS_SYSTEM;n+=1;for i:0..<a.written {out[n+i]=sys[i];}n+=a.written;out[n]=MS_SYSTEM_END;n+=1;}
    out[n]=MS_INST;n+=1;for i:0..<b.written {out[n+i]=usr[i];}n+=b.written;out[n]=MS_INST_END;
    return MsResult.{written=n+1};
}
```

The independent check is the Python script: it applies the same pattern
with Python's regular expressions and tiktoken's merge rule over the full
130,072-entry table, and records the ids. A `#test` compares every fixture
string, and the chat sequence, id for id.

## Rotary positions

Ministral rotates queries and keys with YaRN over theta 10^6. With head
dimension d = 128 there are 64 frequencies; frequency i starts as
theta^(-2i/d). YaRN divides the slow ones by the factor 16 and keeps the fast
ones. The bounds are the dimensions that turn 32 times and once over the
original 16,384 positions:

- low = floor(128 · ln(16384 / (32 · 2π)) / (2 · ln 10^6)) = floor(20.38) = 20
- high = ceil(128 · ln(16384 / (1 · 2π)) / (2 · ln 10^6)) = ceil(36.44) = 37

Frequencies 0 to 19 keep their value, 37 to 63 are divided by 16, and 20 to
36 blend linearly. `params.json` sets `apply_scale` to false, so no magnitude
scale follows. Ministral also scales queries by 1 + 0.1 · ln(1 + floor(p /
16384)) at position p, which is exactly 1 inside the original window.

<!-- source: psionic/lib/ministral.coax rope -->
```jai
// YaRN over theta 1e6: frequency i keeps its original value theta^(-2i/d)
// below the low correction dimension, is divided by the factor 16 above the
// high one, and blends linearly between. The bounds are where a frequency
// turns 32 (fast) and 1 (slow) times over the original 16,384 positions.
// apply_scale is false in params.json, so no magnitude scale follows.
ms_correction :: (rotations:f64,d:int)->f64 {
    return f64(d)*log(f64(MS_ORIGINAL)/(rotations*2*3.141592653589793))/(2*log(MS_THETA));
}
// For these constants both bounds lie inside 0..d-1 for every even d.
ms_rope_frequencies :: (d:int,out:[]f64) {
    low:=cast(int)floor(ms_correction(MS_BETA_FAST,d));high:=cast(int)(-floor(-ms_correction(MS_BETA_SLOW,d)));
    for i:0..<low {out[i]=exp(-log(MS_THETA)*f64(2*i)/f64(d));}
    for i:low..<high {
        original:=exp(-log(MS_THETA)*f64(2*i)/f64(d));ramp:=f64(i-low)/f64(high-low);
        out[i]=original/MS_FACTOR*ramp+original*(1-ramp);
    }
    for i:high..<d/2 {out[i]=exp(-log(MS_THETA)*f64(2*i)/f64(d))/MS_FACTOR;}
}
// Llama 4 query scaling: 1 + 0.1 ln(1 + floor(position / 16384)), which is
// exactly 1 inside the original window.
ms_query_scale :: (position:int)->f64 {return 1+MS_LLAMA4_BETA*log(f64(1+position/MS_ORIGINAL));}
```

The checkpoint is in Mistral's native layout, so each head rotates adjacent
pairs (2i, 2i+1) by position times frequency i.

## One layer

The decoder is the plain pre-norm transformer. RMSNorm with a BF16 gain:

<!-- source: psionic/lib/ministral.coax norm -->
```jai
// RMSNorm with a BF16 gain: x / sqrt(mean(x^2) + 1e-5) * g.
ms_norm :: (x:[]f32,gain:[]u8,out:[]f32)->MsStatus {
    squares:PsionicAccumulator;for v:x {squares=psionic_ops_accumulate(squares,f64(v)*f64(v));}
    divisor:=sqrt(psionic_ops_total(squares)/f64(x.count)+MS_EPSILON);
    for i:0..<x.count {
        bits:=qm_bits(QmDtype.bf16,gain,i);if !psionic_tensor_finite_bits(bits) {return MsStatus.number;}
        out[i]=f32(f64(x[i])/divisor*f64(f32_from_bits(bits)));
    }
    return MsStatus.ok;
}
```

The layer uses chapter 7's BF16 matrix-vector product (`qm_project`), which
refuses non-finite weights. Each of the 32 query heads reads one of the 8
key-value heads: query head j reads head j / 4.

<!-- source: psionic/lib/ministral.coax layer -->
```jai
// One decoder layer over tokens at positions 0..<count: x (count rows of
// width) in, the residual stream after the layer out.
//   h = norm(x); q, k, v = wq h, wk h, wv h; rotate q and k
//   a = softmax(q k / sqrt(d)) v over earlier positions, query head j
//       reading key-value head j / (heads / kv_heads)
// Positions stay below 64, inside the original window, so the Llama 4
// query scale is exactly 1 and is not applied. Scores are exponentiated in
// binary64 without a shift.
//   x += wo a; x += w2 (silu(w1 norm(x)) * w3 norm(x))
ms_layer :: (s:MsShape,w:MsLayer,x:[]f32,count:int,out:[]f32,work:[]f32)->MsResult #export_c {
    if !ms_shape_valid(s) || !ms_layer_sizes(s,w) || count<1 || count>MS_TOKENS_MAX {return MsResult.{status=MsStatus.shape};}
    d:=s.width;hd:=s.head_dim;q:=s.heads*hd;kv:=s.kv_heads*hd;group:=s.heads/s.kv_heads;
    if x.count!=count*d || out.count<count*d || work.count<ms_layer_work(s,count) {return MsResult.{status=MsStatus.capacity};}
    freqs:[128]f64;ms_rope_frequencies(hd,freqs[..]);
    qs:=work[0..<count*q];ks:=work[count*q..<count*(q+kv)];vs:=work[count*(q+kv)..<count*(q+2*kv)];
    hb:[8192]f32;ob:[8192]f32;ab:[16384]f32;gb:[32768]f32;ub:[32768]f32;sb:[64]f32;sg:[32768]f32;stage:=sg[..];
    h:=hb[0..<d];o:=ob[0..<d];a:=ab[0..<q];gate:=gb[0..<s.hidden];up:=ub[0..<s.hidden];scores:=sb[0..<count];
    for t:0..<count {
        if ms_norm(x[t*d..<(t+1)*d],w.attention_norm,h)!=MsStatus.ok {return MsResult.{status=MsStatus.number,cause=1};}
        r:=ms_project(w.wq,q,d,h,qs[t*q..<(t+1)*q],stage);if r.status!=MsStatus.ok {return r;}
        r=ms_project(w.wk,kv,d,h,ks[t*kv..<(t+1)*kv],stage);if r.status!=MsStatus.ok {return r;}
        r=ms_project(w.wv,kv,d,h,vs[t*kv..<(t+1)*kv],stage);if r.status!=MsStatus.ok {return r;}
        ms_rotate(qs[t*q..<(t+1)*q],s.heads,hd,t,freqs[..]);ms_rotate(ks[t*kv..<(t+1)*kv],s.kv_heads,hd,t,freqs[..]);
    }
    for t:0..<count {
        for head:0..<s.heads {
            g:=head/group;
            for u:0..<t+1 {
                acc:PsionicAccumulator;for j:0..<hd {acc=psionic_ops_accumulate(acc,f64(qs[t*q+head*hd+j])*f64(ks[u*kv+g*hd+j]));}
                scores[u]=f32(psionic_ops_total(acc)/sqrt(f64(hd)));
            }
            z:PsionicAccumulator;for u:0..<t+1 {z=psionic_ops_accumulate(z,exp(f64(scores[u])));}
            total:=psionic_ops_total(z);
            for j:0..<hd {
                acc:PsionicAccumulator;for u:0..<t+1 {acc=psionic_ops_accumulate(acc,exp(f64(scores[u]))/total*f64(vs[u*kv+g*hd+j]));}
                a[head*hd+j]=f32(psionic_ops_total(acc));
            }
        }
        r:=ms_project(w.wo,d,q,a,o,stage);if r.status!=MsStatus.ok {return r;}
        for i:0..<d {out[t*d+i]=f32(f64(x[t*d+i])+f64(o[i]));}
        if ms_norm(out[t*d..<(t+1)*d],w.ffn_norm,h)!=MsStatus.ok {return MsResult.{status=MsStatus.number,cause=2};}
        r=ms_project(w.w1,s.hidden,d,h,gate,stage);if r.status!=MsStatus.ok {return r;}
        r=ms_project(w.w3,s.hidden,d,h,up,stage);if r.status!=MsStatus.ok {return r;}
        for i:0..<s.hidden {g:=f64(gate[i]);gate[i]=f32(g/(1+exp(-g))*f64(up[i]));}
        r=ms_project(w.w2,d,s.hidden,gate,o,stage);if r.status!=MsStatus.ok {return r;}
        for i:0..<d {out[t*d+i]=f32(f64(out[t*d+i])+f64(o[i]));}
    }
    return MsResult.{written=count*d};
}
```

The embedding gather and a readout chunk close the path. The readout applies
the final norm and multiplies by rows of the tied embedding:

<!-- source: psionic/lib/ministral.coax ends -->
```jai
// Embedding rows for tokens, and a readout chunk: the final norm of one
// hidden row against `rows` rows of the tied embedding.
ms_embed :: (s:MsShape,table:[]u8,tokens:[]int,out:[]f32)->MsResult #export_c {
    if !ms_shape_valid(s) || table.count!=2*s.vocab*s.width {return MsResult.{status=MsStatus.shape};}
    if out.count<tokens.count*s.width {return MsResult.{status=MsStatus.capacity};}
    for k:0..<tokens.count {if tokens[k]<0 || tokens[k]>=s.vocab {return MsResult.{status=MsStatus.range,cause=k};}}
    for k:0..<tokens.count {for i:0..<s.width {
        bits:=qm_bits(QmDtype.bf16,table,tokens[k]*s.width+i);if !psionic_tensor_finite_bits(bits) {return MsResult.{status=MsStatus.number,cause=k};}
        out[k*s.width+i]=f32_from_bits(bits);
    }}
    return MsResult.{written=tokens.count*s.width};
}
ms_readout :: (s:MsShape,head:[]u8,rows:int,norm:[]u8,hidden:[]f32,logits:[]f32,work:[]f32)->MsResult #export_c {
    if !ms_shape_valid(s) || rows<1 || rows>s.vocab || head.count!=2*rows*s.width || norm.count!=2*s.width || hidden.count!=s.width {return MsResult.{status=MsStatus.shape};}
    if logits.count<rows || work.count<s.width+rows {return MsResult.{status=MsStatus.capacity};}
    if ms_norm(hidden,norm,work[0..<s.width])!=MsStatus.ok {return MsResult.{status=MsStatus.number};}
    return ms_project(head,rows,s.width,work[0..<s.width],logits,work[s.width..<s.width+rows]);
}
```

## A model small enough to check by reading

A two-layer model with Ministral's structure runs in every check: width 8,
4 query heads sharing 2 key-value heads of 4, SwiGLU of 12 and 16 tokens. Its
weights come from a formula that both the Coax file and the Python reference
compute, multiples of 1/32 in [-1/4, 1/4], exact in BF16.

<!-- source: psionic/lib/ministral.coax forward -->
```jai
// The whole decoder over tokens; logits for the last position. hidden holds
// two residual buffers of count x width; work is ms_layer_work.
ms_forward :: (s:MsShape,embed:[]u8,norm:[]u8,layers:[]MsLayer,tokens:[]int,hidden:[]f32,logits:[]f32,work:[]f32)->MsResult {
    n:=tokens.count*s.width;
    if layers.count!=s.layers {return MsResult.{status=MsStatus.shape};}
    if hidden.count<2*n {return MsResult.{status=MsStatus.capacity};}
    r:=ms_embed(s,embed,tokens,hidden[0..<n]);if r.status!=MsStatus.ok {return r;}
    from:=0;
    for l:0..<s.layers {
        to:=n-from;r=ms_layer(s,layers[l],hidden[from..<from+n],tokens.count,hidden[to..<to+n],work);
        if r.status!=MsStatus.ok {r.cause=l;return r;}
        from=to;
    }
    return ms_readout(s,embed,s.vocab,norm,hidden[from+n-s.width..<from+n],logits,work);
}
ms_argmax :: (v:[]f32)->int {best:=0;for i:1..<v.count {if v[i]>v[best] {best=i;}}return best;}
```

The Python reference computes the same model in binary64 from the
architecture's description: its own RMSNorm, pair rotation, causal softmax
and SwiGLU. A `#test` compares both layers' outputs and the 16 logits; the
largest difference must stay under 2 × 10^-6.

## Run it

The programs are [`ministral.coax`](../examples/ministral.coax) and
[`ministral-refuse.coax`](../examples/ministral-refuse.coax); the library is
[`ministral.coax`](../lib/ministral.coax).

<!-- source: psionic/examples/ministral.coax run -->
```jai
main :: () {
    r:=ms_inventory(MS_3B,MS_PINNED_MANIFEST);
    print("checkpoint %: % tensors, % text parameters, % image tensors: %\n",cast(string)MS_PINNED_REVISION[0..<8],MS_PINNED_TENSORS,r.written,r.cause,r.status);

    tb:=MS_TEKKEN_BYTES;tr:=MS_TEKKEN_RANKS;t:=MsTable.{bytes=tb[..],ranks=tr[..]};ids:[64]int;
    for k:0..<3 {
        e:=ms_encode(t,MS_TEKKEN_TEXTS[k],ids[..]);print("tekken %:",k);
        for i:0..<e.written {print(" %",ids[i]);}print("\n");
    }
    c:=ms_chat(t,"Hello, world!","What is the capital of France?",ids[..]);print("chat (% tokens):",c.written);
    for i:0..<c.written {print(" %",ids[i]);}print("\n");

    f:[64]f64;ms_rope_frequencies(128,f[..]);
    print("yarn: f0 % f20 % f37*16 % f63*16 %\n",f[0],f[20],f[37]*16,f[63]*16);

    s:=ms_tiny();bytes:[MS_TINY_BYTES]u8;m:=ms_tiny_weights(bytes[..]);layers:=m.layers;
    tokens:[8]int;n:=3;tokens[0]=1;tokens[1]=3;tokens[2]=7;hidden:[128]f32;logits:[16]f32;work:[256]f32;
    print("tiny prompt: 1 3 7, greedy:");
    while n<8 {
        g:=ms_forward(s,m.embed,m.norm,layers[..],tokens[0..<n],hidden[0..<2*n*8],logits[..],work[..]);
        if g.status!=MsStatus.ok {print(" refused %\n",g.status);return;}
        tokens[n]=ms_argmax(logits[..]);print(" %",tokens[n]);n+=1;
    }
    print("\n");
}
```

```shell
coax/bin/coax run psionic/examples/ministral.coax
```

```text
checkpoint b6d637be: 458 tensors, 3429006336 text parameters, 222 image tensors: ok
tekken 0: 22177 1044 4304 1033
tekken 1: 7493 1395 1278 8961 1307 5498 1063
tekken 2: 1032 8924 18971 1267 1421 1032 1050 1048 1050 1053 8091
chat (16 tokens): 1 17 22177 1044 4304 1033 18 3 7493 1395 1278 8961 1307 5498 1063 4
yarn: f0 1.0 f20 0.013335214321633242 f37*16 0.000339820832894256 f63*16 1.240937760751719e-06
tiny prompt: 1 3 7, greedy: 0 0 0 0 0
```

`Hello, world!` is four tokens: `Hello`, `,`, ` world`, `!`. The third
string, `  leading spaces\n\nand 2025 numbers`, shows two rules: the leading
two spaces split into ` ` and ` leading` (whitespace before a non-space keeps
its last space for the next piece), and `2025` becomes four single-digit
tokens. The synthetic model's greedy tokens are whatever its formula weights
prefer; they show the loop runs, not that the model says anything.

## Refusals

<!-- source: psionic/examples/ministral-refuse.coax refuse -->
```jai
main :: () {
    buf:[160]u8;
    d:=MS_3B;d.kv_heads=4;r:=ms_inventory(d,MS_PINNED_MANIFEST);n:=ms_line(ms_expected(d,r.cause),buf[..]);
    print("4 key-value heads against the pinned file: %, first missing: %\n",r.status,cast(string)buf[0..<n]);
    d=MS_3B;d.layers=27;r=ms_inventory(d,MS_PINNED_MANIFEST);n=ms_line(ms_expected(d,r.cause),buf[..]);
    print("27 layers: %, first missing: %\n",r.status,cast(string)buf[0..<n]);
    d=MS_3B;d.heads=30;print("30 query heads over 8 key-value heads: %\n",ms_inventory(d,MS_PINNED_MANIFEST).status);

    tb:=MS_TEKKEN_BYTES;tr:=MS_TEKKEN_RANKS;t:=MsTable.{bytes=tb[..],ranks=tr[..]};ids:[64]int;
    e:=ms_encode(t,"café",ids[..]);print("non-ASCII text: %\n",e);
    e=ms_encode(t,"on Q~",ids[..]);print("a byte outside the fixture table: %\n",e);
    print("chat into 6 ids: %\n",ms_chat(t,"Hello","What",ids[0..<6]));

    s:=ms_tiny();bytes:[MS_TINY_BYTES]u8;m:=ms_tiny_weights(bytes[..]);
    x:[520]f32;out:[520]f32;work:[2100]f32;
    bad:=[2]int.[3,16];print("token 16 of 16: %\n",ms_embed(s,m.embed,bad[..],x[..]));
    print("65 tokens: %\n",ms_layer(s,m.layers[0],x[..],65,out[..],work[..]));
    print("work one value short: %\n",ms_layer(s,m.layers[0],x[0..<40],5,out[..],work[0..<159]));
    w:=m.layers[0];w.wq=w.wq[0..<254];print("a short query matrix: %\n",ms_layer(s,w,x[0..<40],5,out[..],work[..]));
    bytes[1]=0x7f;bytes[0]=0xc0;bad[0]=0;print("a NaN in token 0's embedding row: %\n",ms_embed(s,m.embed,bad[0..<1],x[..]));
}
```

```text
4 key-value heads against the pinned file: checkpoint, first missing: layers.0.attention.wk.weight BF16 512,3072
27 layers: checkpoint, first missing: layers.26.attention.wk.weight BF16 1024,3072
30 query heads over 8 key-value heads: shape
non-ASCII text: MsResult.{status = unsupported, written = 0, cause = 3}
a byte outside the fixture table: MsResult.{status = range, written = 0, cause = 3}
chat into 6 ids: MsResult.{status = capacity, written = 0, cause = 7}
token 16 of 16: MsResult.{status = range, written = 0, cause = 1}
65 tokens: MsResult.{status = shape, written = 0, cause = 0}
work one value short: MsResult.{status = capacity, written = 0, cause = 0}
a short query matrix: MsResult.{status = shape, written = 0, cause = 0}
a NaN in token 0's embedding row: MsResult.{status = number, written = 0, cause = 0}
```

## The original weights

The examples run without the 7.7 GB file. One script runs the slices that
need it:

```shell
COAX_CC=gcc psionic/tests/check_ministral_slices.sh ~/models/ministral-3-3b-instruct-2512-bf16-b6d637be
```

It authenticates the files, checks the inventory and Tekken fixtures, builds
`psionic/tests/ministral_slices.c` (which maps the file and finds tensors with
the Coax safetensors reader in `psionic/lib/model-io`), and runs the Coax decoder on the 10-token prompt
`<s>[INST]What is the capital of France?[/INST]`: the embedding gather, layer
0, and readout rows 0 to 4095 for the last token after layer 0. The Python
reference recomputes each slice in binary64 from the same bytes. The
tolerances were frozen before the first comparison: the gather is exact; a
layer and the readout chunk may differ by at most 10^-4 of the slice's
largest magnitude. The recorded run:

```text
10 tokens: [1, 3, 7493, 1395, 1278, 8961, 1307, 5498, 1063, 4]
slice embed: 10 tokens, 0.000 s
slice layer0: 10 tokens, 6.415 s
slice readout: rows 0..4096, 0.072 s
embed: 10 tokens, exact
layer 0: 10 tokens, max |difference| 1.982e-07, largest |value| 3.535e+00
readout rows 0..4096: max |difference| 3.280e-07, largest |value| 1.433e+01
```

This model's evidence does not qualify any larger Mistral model.

## What remains

- Tekken's full pattern over Unicode classes, and the full table loaded from
  `tekken.json` rather than a fixture restriction.
- All 26 layers on the original weights, with logits recorded from an
  independent source, and greedy tokens from the real model.
- Positions past 16,384, where the Llama 4 query scale and the YaRN blend
  start to matter, and the key-value cache that makes them affordable.
- Mistral's reasoning and tool formats, and the image path.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
- [Chapter 7](ch007-reading-original-weights.md), [chapter 9](ch009-text-becomes-a-model-request.md),
  [chapter 10](ch010-attention-without-training.md),
  [chapter 33](ch033-measure-the-work-and-plan-the-memory.md) and
  [chapter 53](ch053-choosing-the-next-token.md).
