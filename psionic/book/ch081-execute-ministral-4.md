# 81. Execute Ministral 4

> **Status: partly implemented.** The pinned checkpoint's inventory, Tekken
> over the whole of `tekken.json` with its Unicode pattern, the chat format,
> the YaRN frequencies, and all 26 layers with a key-value cache run on the
> CPU. A recorded greedy run on the original weights matches a float64
> reference at every layer and in its tokens. Positions past 64, the
> reasoning and tool formats and the image path remain.

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
splits numbers one code point at a time.

### Classes of code point

The pattern names Unicode general categories. Call the set
`[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]` A and `[\p{Ll}\p{Lm}\p{Lo}\p{M}]` B. Every
code point then falls in one of seven classes: upper (Lu, Lt; in A only),
lower (Ll; in B only), both (Lm, Lo; in A and in B), mark (M; in A and B, but
not a letter), number (N), space (Unicode White_Space) and other. The
classes come from a table, `psionic/fixtures/ministral-unicode.coax`, that
the reference script writes from Python's `unicodedata` (Unicode 15.1.0):
3,123 ranges, each a first code point and a class. Text is UTF-8, so a
decoder comes first, and it refuses anything that is not well-formed.

<!-- source: psionic/lib/ministral.coax classes -->
```jai
// Tekken's pattern sees seven classes of code point. ms_class is the class
// of one ASCII byte; ms_point_class looks any code point up in a table of
// 3,123 ranges written from Unicode 15.1.0's general categories.
MS_UPPER :: 0; MS_LOWER :: 1; MS_DIGIT :: 2; MS_SPACE :: 3; MS_OTHER :: 4; MS_BOTH :: 5; MS_MARK :: 6;
ms_class :: (c:u8)->int {
    if c>='A' && c<='Z' {return MS_UPPER;}
    if c>='a' && c<='z' {return MS_LOWER;}
    if c>='0' && c<='9' {return MS_DIGIT;}
    if c==' ' || (c>=9 && c<=13) {return MS_SPACE;}
    return MS_OTHER;
}
ms_point_class :: (code:int)->int {
    lo:=0;hi:=MS_UNICODE_RANGES;
    while hi-lo>1 {mid:=(lo+hi)/2;if MS_UNICODE_STARTS[mid]<=code {lo=mid;} else {hi=mid;}}
    return cast(int)MS_UNICODE_CLASSES[lo];
}
// One UTF-8 sequence at s[at]: its code point and the offset after it, or
// code -1 when the bytes are not well-formed UTF-8 (a stray continuation
// byte, a cut-off sequence, an overlong form, a surrogate, or past U+10FFFF).
// A stray continuation byte (0x80-0xbf) starts below zero and a lead of 0xf8
// or more lands past U+10FFFF, so the range checks at the end refuse both.
MsPoint :: struct {code:int;next:int;}
ms_utf8 :: (s:string,at:int)->MsPoint {
    lead:=cast(int)s[at];bad:=MsPoint.{code=-1,next=at};
    if lead<0x80 {return MsPoint.{code=lead,next=at+1};}
    n:=3;code:=lead-0xf0;least:=0x10000;
    if lead<0xe0 {n=1;code=lead-0xc0;least=0x80;} else if lead<0xf0 {n=2;code=lead-0xe0;least=0x800;}
    if at+n>=s.count {return bad;}
    for i:1..<n+1 {c:=cast(int)s[at+i];if c<0x80 || c>=0xc0 {return bad;}code=code*64+c-0x80;}
    if code<least || code>0x10ffff || (code>=0xd800 && code<0xe000) {return bad;}
    return MsPoint.{code=code,next=at+n+1};
}
ms_class_at :: (s:string,at:int)->int {return ms_point_class(ms_utf8(s,at).code);}
// The pattern's sets as masks of classes: A = Lu Lt Lm Lo M, B = Ll Lm Lo M,
// and the punctuation run [^\s\p{L}\p{N}] = other and marks.
MS_SET_A :: 0x61; MS_SET_B :: 0x62; MS_SET_PUNCT :: 0x50;
ms_in :: (s:string,at:int,set:int)->bool {return (set>>ms_class_at(s,at))&1==1;}
ms_span :: (s:string,at:int,set:int)->int {p:=at;while p<s.count && ms_in(s,p,set) {p=ms_utf8(s,p).next;}return p;}
```

### Pieces

A regular-expression engine tries the alternatives in order and, inside one,
backtracks: a greedy run gives back one code point at a time until the rest
matches. Because A and B overlap, the first alternative is the subtle one.
`[A]*` first takes the whole A run. If the code point after the run is in B
(it can only be lower case, since it is not in A), `[B]+` continues from
there. Otherwise `[A]*` gives back code points until the next one is in B,
which is the last both-class or mark code point inside the run, and `[B]+`
continues from it. The optional first code point is tried taken and then not
taken, in each alternative before the next. On ASCII text there are no
both-class or mark code points, and the split is the same as before: upper
case then lower case, at least one letter.

<!-- source: psionic/lib/ministral.coax pieces -->
```jai
// Tekken splits text with one pattern before byte-pair merging:
//   [^\r\n\p{L}\p{N}]?[A]*[B]+ | [^\r\n\p{L}\p{N}]?[A]+[B]* | \p{N}
//   | " ?[^\s\p{L}\p{N}]+[\r\n/]*" | \s*[\r\n]+ | \s+(?!\S) | \s+
// A regex engine tries the alternatives in order and backtracks inside each.
// [A]*[B]+ from p: A* first takes the whole A run up to e. If the code point
// at e is lower case (in B; it is not in A), B+ runs on from e. Otherwise A*
// gives back code points until the one after it is in B: the last code point
// of the run that is in B (an Lm, Lo or mark). B+ takes it and stops, since
// what follows it in the run is not in B. Each alternative returns p when
// it fails.
ms_alt1 :: (s:string,p:int)->int {
    e:=ms_span(s,p,MS_SET_A);
    if e<s.count && ms_class_at(s,e)==MS_LOWER {return ms_span(s,e,MS_SET_B);}
    last:=p;q:=p;while q<e {if ms_in(s,q,MS_SET_B) {last=ms_utf8(s,q).next;}q=ms_utf8(s,q).next;}
    return last;
}
// [A]+[B]*, tried only after [A]*[B]+ failed from the same p: then the code
// point after the A run is not lower case, so [B]* adds nothing.
ms_alt2 :: (s:string,p:int)->int {return ms_span(s,p,MS_SET_A);}
// The optional first code point may be other, a mark, or a space that is
// not \r or \n. A mark is in A and in B, so taking it as the first code
// point or as the start of the letters gives the same piece; only other and
// space make a difference, and with them taken the letters start after them.
ms_newline :: (c:u8)->bool {return c==10 || c==13;}
ms_run :: (s:string,at:int,class:int)->int {p:=at;while p<s.count && ms_class(s[p])==class {p+=1;}return p;}
ms_word :: (s:string,at:int)->int {
    k:=ms_class_at(s,at);p:=at;
    if k==MS_OTHER || (k==MS_SPACE && !ms_newline(s[at])) {p=ms_utf8(s,at).next;}
    e:=ms_alt1(s,p);if e>p {return e;}
    e=ms_alt2(s,p);if e>p {return e;}
    return at;
}
// The end of the piece that starts at `at` (at < s.count, s valid UTF-8).
ms_piece :: (s:string,at:int)->int {
    end:=ms_word(s,at);if end>at {return end;}
    if ms_class_at(s,at)==MS_DIGIT {return ms_utf8(s,at).next;}
    // " ?[^\s\p{L}\p{N}]+[\r\n/]*"
    p:=at;if s[p]==' ' {p+=1;}
    end=ms_span(s,p,MS_SET_PUNCT);
    if end>p {while end<s.count && (ms_newline(s[end]) || s[end]=='/') {end+=1;}return end;}
    // Whitespace: through the last newline of the run, else all but the last
    // space before a non-space, else the whole run.
    run:=at;last:=at;final:=at;
    while run<s.count && ms_class_at(s,run)==MS_SPACE {final=run;if ms_newline(s[run]) {last=run+1;}run=ms_utf8(s,run).next;}
    if last>at {return last;}
    if run==s.count || final==at {return run;}
    return final;
}
```

Trace `What is the capital of France?`. At `W` the first alternative takes
one upper-case letter and the lower-case run: `What`. At the space, the space
is the optional first character, so ` is`, ` the`, ` capital`, ` of` and
` France` follow. At `?` no letter follows, so the fourth alternative takes
the punctuation run: `?`. Seven pieces.

Now trace `Ça coûte 5 €`, 16 bytes. `Ç` (U+00C7) is upper case and `a` lower
case, so the first piece is `Ça`. At the space, the space is the optional
first code point and `coûte` is one lower-case run (`û` is lower case):
` coûte`. The next space is followed by `5`, a number, so no letter
alternative matches and no punctuation follows; the whitespace rules leave a
single space as its own piece ` `. Then `5` is one number, and ` €` is a
space and a run of one non-letter, non-number code point (`€` is a currency
symbol). Five pieces: `Ça`, ` coûte`, ` `, `5`, ` €`.

Backtracking shows in `A中B`, where `中` (U+4E2D) is Lo, in both sets. `[A]*`
takes all three code points, but nothing in B follows. It gives back `B`
(not in B) and then tries `[B]+` at `B`, which fails; it gives back `中`, and
`[B]+` matches `中`. The pieces are `A中` and `B`. The check compares these
splits with Python's regular-expression engine over the same pattern, for
26 such strings: marks first, after a space or after a newline, title-case
letters, letter-number code points, no-break spaces and ideographic spaces.

### Ranks

Each piece then merges by rank, the rule tiktoken uses: a piece found whole
in the table is one token; otherwise start from single bytes and repeatedly
join the adjacent pair whose joined bytes have the lowest rank, the leftmost
on a tie. Pieces work on bytes, so `Ç`, two bytes, can be one token or two.

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

Merging looks up many byte strings, so the full table needs more than a scan.
`MsTable` keeps an optional hash index: a power-of-two array of slots, at
least twice the number of entries, where slot h holds entry + 1 or 0 when
empty. A lookup hashes the bytes and steps forward to the next slot until it
finds the string or an empty slot. A table without slots is still scanned,
so the small tables in the checks need no index.

<!-- source: psionic/lib/ministral.coax table -->
```jai
// A table of byte strings and their ranks. With no slots, a lookup scans
// every entry. With slots (a power of two, at least twice the entries) it
// is an open-addressing hash index: slot h holds entry + 1, or 0 when empty.
MsTable :: struct {bytes:[]string;ranks:[]int;slots:[]int;}
ms_hash :: (s:string)->int {h:=0;for i:0..<s.count {h=(h*131+cast(int)s[i]+1)%16777213;}return h;}
ms_rank :: (t:MsTable,s:string)->int {
    if t.slots.count==0 {for i:0..<t.bytes.count {if t.bytes[i]==s {return t.ranks[i];}}return -1;}
    mask:=t.slots.count-1;h:=ms_hash(s)&mask;
    while t.slots[h]!=0 {i:=t.slots[h]-1;if t.bytes[i]==s {return t.ranks[i];}h=(h+1)&mask;}
    return -1;
}
// Fill t's slots. A repeated byte string is refused with its entry.
ms_index :: (t:MsTable)->MsResult {
    m:=t.slots.count;
    if m<2*t.bytes.count || m&(m-1)!=0 || t.ranks.count!=t.bytes.count {return MsResult.{status=MsStatus.capacity,cause=m};}
    for i:0..<m {t.slots[i]=0;}
    for i:0..<t.bytes.count {
        if ms_rank(t,t.bytes[i])>=0 {return MsResult.{status=MsStatus.checkpoint,cause=i};}
        h:=ms_hash(t.bytes[i])&(m-1);while t.slots[h]!=0 {h=(h+1)&(m-1);}
        t.slots[h]=i+1;
    }
    return MsResult.{written=t.bytes.count};
}
// The fixture's restricted table, copied into caller storage and indexed
// when slots are given.
ms_tekken_fixture :: (data:[]u8,bytes:[]string,ranks:[]int,slots:[]int)->MsTable {
    for i:0..<MS_TEKKEN_DATA_BYTES {data[i]=MS_TEKKEN_DATA[i];}
    for k:0..<MS_TEKKEN_ENTRIES {bytes[k]=cast(string)data[MS_TEKKEN_ENDS[k]..<MS_TEKKEN_ENDS[k+1]];ranks[k]=MS_TEKKEN_RANKS[k];}
    t:=MsTable.{bytes=bytes[0..<MS_TEKKEN_ENTRIES],ranks=ranks[0..<MS_TEKKEN_ENTRIES],slots=slots};
    if ms_index(t).status!=MsStatus.ok {t.slots=slots[0..<0];}
    return t;
}
MsTekkenRig :: struct {data:[MS_TEKKEN_DATA_BYTES]u8;bytes:[MS_TEKKEN_ENTRIES]string;ranks:[MS_TEKKEN_ENTRIES]int;slots:[1024]int;}
```

`ms_tekken_read` reads `tekken.json` itself. It is a bounded reader for this
one layout, not a general JSON parser: it checks the config (the pattern
exactly as the file spells it, 131,072 ids and 1,000 control ids), then reads
the first 130,072 entries of `"vocab"`. Entry k must say `"rank": k`; its
`"token_bytes"` are base64, decoded into one caller-supplied byte store that
the entries view. Other members (`token_str`, `special_tokens`, `image`) are
skipped by counting brackets outside strings. It refuses malformed JSON or
base64 (`range`), a config, rank or entry count that differs (`checkpoint`)
and storage that is too small (`capacity`), each with the byte offset where
it stopped. On the pinned file it reads 130,072 entries and 878,258 token
bytes in about 0.05 s, and the index takes 2^18 slots.

<!-- source: psionic/lib/ministral.coax encode -->
```jai
// UTF-8 text to token ids. Text that is not well-formed UTF-8 is refused
// (unsupported); cause is the byte offset.
ms_encode :: (t:MsTable,text:string,out:[]int)->MsResult {
    at:=0;while at<text.count {p:=ms_utf8(text,at);if p.code<0 {return MsResult.{status=MsStatus.unsupported,cause=at};}at=p.next;}
    at=0;n:=0;
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

The examples and checks use a restricted table so that they run without
the 16.7 MB file. The fixture `psionic/fixtures/ministral-tekken.coax` holds
Tekken restricted to the fixture strings: every entry whose bytes occur
inside one of their pieces, 417 entries with their original ranks, stored as
one byte array (some entries are partial UTF-8 sequences, which a Coax string
literal cannot hold). Merging a piece only ever forms substrings of that
piece, so the restricted table encodes the fixture strings exactly as the
full one does. A byte the restricted table lacks is refused, not guessed.

The independent check is the Python script. It builds the pattern for
Python's `re` with each `\p{..}` class spelled out as code point ranges, splits
the text with Python's engine, applies tiktoken's merge rule over the full
130,072-entry table, and records the ids. A `#test` compares all 18 fixture
strings (8 ASCII, 10 with accents, Cyrillic, Greek, Japanese, combining
marks, an emoji, no-break spaces and Unicode numbers), and the chat sequence,
id for id, through the index and through a scan. The original-weight script
below also loads the whole of `tekken.json` with `ms_tekken_read` and
compares the same strings' ids with Python's.

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

## A cache and the whole vocabulary

`ms_forward` cannot run the real model. It recomputes every position for
every new token, and it reads the logits in one projection, which chapter 7's
product limits to 32,768 rows; the tied head has 131,072. Chapter 58 solved
both for Minitron, a decoder built from this chapter's norm and projections:
`lm_layer_step` keeps each layer's rotated keys and values in a cache
(`LmCache`), so a new token runs one position, and `lm_readout` reads the head
in chunks of at most 32,768 rows. Its step takes the rotation as an `LmRope`,
so Ministral reuses it whole: Ministral is chapter 58's model with the YaRN
rotation over adjacent pairs and the embedding as its head.

<!-- source: psionic/lib/ministral-decode.coax model -->
```jai
// Ministral as chapter 58's model: the embedding is also the head, and every
// layer rotates with YaRN over adjacent pairs. The cache holds at most
// LM_POSITIONS_MAX = 64 positions, far inside the original 16,384, so the
// Llama 4 query scale is exactly 1 and is not applied.
md_model :: (embed:[]u8,norm:[]u8,layers:[]MsLayer)->LmModel #export_c {
    return LmModel.{embed=embed,norm=norm,head=embed,layers=layers,rope=LmRope.yarn};
}
```

Work out what the cache costs for the real model. A position of one layer
keeps 8 key-value heads of 128 keys and 128 values: 2 × 1,024 f32 values, or
8,192 bytes. The cache holds at most 64 positions, so 26 layers need
26 × 64 × 8,192 = 13,631,488 bytes, 13 MiB. The readout runs four chunks of
32,768 rows. The work buffer holds the larger of 64 tokens' queries
(64 × 4,096 = 262,144 values) and one readout (3,072 + 131,072 = 134,144), so
262,144 values.

The cached step and the full recompute agree bit for bit, not just closely. A
row's norm and projections depend only on that row, and a token's attention
sums the same cached keys and values in the same order whether they were
written a moment ago or in the same call. The checks in
`psionic/lib/ministral-decode.coax` hold the synthetic model to that: after
each of ten tokens decoded one at a time, the logits equal a full recompute
of the prefix from position 0; a prefill of four, then three, then single
steps gives the same bits; and greedy continuation equals the argmax chain of
full recomputes. Chapter 58's step also shifts scores by their maximum before
exponentiating, which `ms_layer` does not, so the two differ in rounding: the
cached logits stay within 10^-6 of `ms_forward` and within 2 × 10^-6 of the
float64 reference. A head of 40,000 rows of width 2 shows the chunks: one
projection refuses it, and chunks of 32,768 and 7,232, or of 7, give the
same bits as reading each row alone.

## Run it

The programs are [`ministral.coax`](../examples/ministral.coax) and
[`ministral-refuse.coax`](../examples/ministral-refuse.coax); the library is
[`ministral.coax`](../lib/ministral.coax).

<!-- source: psionic/examples/ministral.coax run -->
```jai
main :: () {
    r:=ms_inventory(MS_3B,MS_PINNED_MANIFEST);
    print("checkpoint %: % tensors, % text parameters, % image tensors: %\n",cast(string)MS_PINNED_REVISION[0..<8],MS_PINNED_TENSORS,r.written,r.cause,r.status);

    rig:MsTekkenRig;t:=ms_tekken_fixture(rig.data[..],rig.bytes[..],rig.ranks[..],rig.slots[..]);ids:[64]int;
    picks:=[5]int.[0,1,2,9,11];
    for k:picks {
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
tekken 9: 60297 1097 112203 1032 1053 20340
tekken 11: 10008 15199 2439 7282 14742 13854
chat (16 tokens): 1 17 22177 1044 4304 1033 18 3 7493 1395 1278 8961 1307 5498 1063 4
yarn: f0 1.0 f20 0.013335214321633242 f37*16 0.000339820832894256 f63*16 1.240937760751719e-06
tiny prompt: 1 3 7, greedy: 0 0 0 0 0
```

`Hello, world!` is four tokens: `Hello`, `,`, ` world`, `!`. The third
string, `  leading spaces\n\nand 2025 numbers`, shows two rules: the leading
two spaces split into ` ` and ` leading` (whitespace before a non-space keeps
its last space for the next piece), and `2025` becomes four single-digit
tokens. String 9, `Ça coûte 5 €`, is the trace above: `Ç` is one token (60297)
and `a` another (1097), because Tekken has no entry for `Ça`; ` coûte` is one
token (112203). String 11, `日本語のテキスト`, is a single piece of Lo code
points that merges into five tokens. The synthetic model's greedy tokens are
whatever its formula weights prefer; they show the loop runs, not that the
model says anything.

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

    rig:MsTekkenRig;t:=ms_tekken_fixture(rig.data[..],rig.bytes[..],rig.ranks[..],rig.slots[..]);ids:[64]int;
    cut:=[5]u8.[0x63,0x61,0x66,0xc3,0x28];e:=ms_encode(t,cast(string)cut[..],ids[..]);print("a broken UTF-8 sequence: %\n",e);
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
a broken UTF-8 sequence: MsResult.{status = unsupported, written = 0, cause = 3}
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

With `--run`, the script runs all 26 layers instead. It builds
`psionic/tests/ministral_run.c`, which calls `md_model` and chapter 58's
`lm_continue` on the same 10-token prompt for eight greedy tokens (stopping
early at `</s>`), then recomputes every layer from position 0 over the prompt
and the first seven emitted tokens, compares the last logits bit for bit with
the cached run, and writes each layer's output. Then
`psionic/tests/ministral_run_oracle.py` recomputes the whole model in float64
from the original bytes with the reference script's own norm, rotation and
softmax, spreading the matrix-vector products over worker processes. Its
tolerance was frozen before the first comparison: every layer's output and
the last logits within 10^-3 of the slice's largest magnitude, and the
reference's own greedy choice at each of the eight positions equal to the
token the Coax run emitted. The run takes about ten minutes, so it is
recorded in `psionic/fixtures/ministral-run.txt`, not rerun:

```text
prompt (10 tokens): 1 3 7493 1395 1278 8961 1307 5498 1063 4
greedy, all 26 layers through the cache: 8 tokens, cache length 17, 151.6 s: 1784 8961 1307 5498 1395 1603 42572 12617
full recompute over 17 tokens, 136.1 s: last logits identical bits to the cached run
layer  0: 17 positions: max |difference| 1.717e-07, largest |value| 3.535e+00
layer  1: 17 positions: max |difference| 2.971e-07, largest |value| 4.084e+00
layer  2: 17 positions: max |difference| 9.620e-05, largest |value| 5.357e+02
layer  3: 17 positions: max |difference| 8.375e-05, largest |value| 5.358e+02
layer  4: 17 positions: max |difference| 1.089e-04, largest |value| 5.360e+02
layer  5: 17 positions: max |difference| 9.338e-05, largest |value| 5.362e+02
layer  6: 17 positions: max |difference| 9.673e-05, largest |value| 5.363e+02
layer  7: 17 positions: max |difference| 1.133e-04, largest |value| 5.364e+02
layer  8: 17 positions: max |difference| 8.838e-05, largest |value| 5.366e+02
layer  9: 17 positions: max |difference| 5.475e-05, largest |value| 5.368e+02
layer 10: 17 positions: max |difference| 3.119e-05, largest |value| 5.370e+02
layer 11: 17 positions: max |difference| 2.495e-05, largest |value| 5.376e+02
layer 12: 17 positions: max |difference| 1.994e-05, largest |value| 5.379e+02
layer 13: 17 positions: max |difference| 4.872e-05, largest |value| 5.382e+02
layer 14: 17 positions: max |difference| 5.088e-05, largest |value| 5.384e+02
layer 15: 17 positions: max |difference| 7.391e-05, largest |value| 5.386e+02
layer 16: 17 positions: max |difference| 1.056e-04, largest |value| 5.388e+02
layer 17: 17 positions: max |difference| 1.158e-04, largest |value| 5.409e+02
layer 18: 17 positions: max |difference| 1.559e-04, largest |value| 5.411e+02
layer 19: 17 positions: max |difference| 1.462e-04, largest |value| 5.412e+02
layer 20: 17 positions: max |difference| 1.537e-04, largest |value| 5.416e+02
layer 21: 17 positions: max |difference| 1.482e-04, largest |value| 5.417e+02
layer 22: 17 positions: max |difference| 1.666e-04, largest |value| 5.416e+02
layer 23: 17 positions: max |difference| 1.555e-04, largest |value| 5.412e+02
layer 24: 17 positions: max |difference| 1.534e-04, largest |value| 5.414e+02
layer 25: 17 positions: max |difference| 1.247e-04, largest |value| 4.624e+01
last logits: max |difference| 1.223e-06, largest |value| 2.245e+01
reference greedy at positions 9..16: [1784, 8961, 1307, 5498, 1395, 1603, 42572, 12617] (the tokens the Coax run emitted)
emitted text: 'The capital of France is **Paris**.'
reference: 227 s on 20 processes
```

The cached continuation and the full recompute agree bit for bit. Every
layer stays within 3 × 10^-6 of the float64 reference relative to its
largest value. From layer 2 to layer 24 one residual value near 540
dominates the scale, so differences of about 10^-4 there are still more than
six digits of agreement; layer 25 drops that value, and its 1.2 × 10^-4
against a largest value of 46 is the loosest, 2.7 × 10^-6.
The last logits differ by at most 1.2 × 10^-6, and the reference picks the
same eight tokens: `The capital of France is **Paris**.` The cache paid for
itself even on a short run: 17 positions through the cache took 152 seconds,
and one full recompute of 17 positions took 136.

This model's evidence does not qualify any larger Mistral model.

## What remains

- The code point classes come from Unicode 15.1.0 (Python's `unicodedata`);
  the upstream tokenizer's regex engine may use another Unicode version, so a
  code point whose category changed could split differently. Pieces longer
  than 64 bytes are refused.
- Positions past 64, the cache's bound here, and past 16,384, where the
  Llama 4 query scale and the YaRN blend start to matter.
- Mistral's reasoning and tool formats, and the image path.
- Throughput: one position through all 26 layers and the readout takes
  about 9 seconds on one CPU thread.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
- [Chapter 7](ch007-reading-original-weights.md), [chapter 9](ch009-text-becomes-a-model-request.md),
  [chapter 10](ch010-attention-without-training.md),
  [chapter 33](ch033-measure-the-work-and-plan-the-memory.md),
  [chapter 53](ch053-choosing-the-next-token.md) and
  [chapter 58](ch058-continue-a-passage.md).
