# 154. Transcribe while someone speaks

> **Status: partly implemented.** Chapter 152's encoder runs as a stream on
> the CPU with seeded weights: feature frames arrive in numbered chunks, each
> layer reads a limited left context from per-layer caches and its
> depthwise convolution reads only the past, greedy CTC decoding emits
> partial hypotheses that only grow, the end of turn flushes and closes the
> utterance, and each final hypothesis carries its latency in audio time.
> The stream matches a whole-utterance reference bit for bit, and with the
> whole utterance in one chunk it matches chapter 152's encoder bit for bit.
> Still to do: streaming a real model (Parakeet was trained with full attention
> and centred convolutions, so it is not a cache-aware streaming model), streaming the
> a real transducer decoder, computing features from audio as it arrives,
> taking end of turn and barge-in from the voice engine through
> `verse/platform/voice.h`, and wall-clock latency at P50 and P99 on a named
> host.

The [original-weight follow-up profile](../docs/parakeet.md#incremental-follow-up-12673)
now pins NVIDIA's separate cache-aware Nemotron/Parakeet-family English RNNT
checkpoint and its reference contract. Its files have been verified on Linux;
no original-weight streaming inference or consumer adoption is qualified yet.
The seeded implementation and examples below retain their existing profile.

[Chapter 152](ch152-a-speech-encoder.md) encodes a whole utterance at once.
Every frame attends to every other frame, and every depthwise convolution
reads frames on both sides. A voice interface cannot wait for that: it must
show what the person is saying while they say it. This chapter makes the
encoder streaming.

The design is NeMo's *cache-aware* streaming FastConformer. Audio arrives in
chunks of a fixed size. Each frame may attend to its own chunk and a fixed
number of chunks before it, never to a later chunk, and each depthwise
convolution reads only the current frame and the frames before it. Then a
frame's value is final as soon as its chunk has arrived, and each layer only
has to keep the keys, values and convolution inputs of the frames that later
chunks can still read.

Everything here is in [`listen.coax`](../lib/listen.coax). It reuses
chapter 152's pieces (subsampling, layer norm, feed-forward modules,
position sinusoids, softmax, CTC head) and chapter 153's edit distance. The
profile is chapter 152's, `psionic-conformer-v1` (f32 storage, binary64
compensated sums, one rounding to f32 per stored value), named
`psionic-listen-v1` for the streaming bounds: at most 255 encoded frames a
chunk and 256 frames of attention context.

## What streams, and what does not

The encoder in this chapter is a synthetic, seeded one: two layers, width 8,
16 mels, three causal convolution taps, and three word classes plus the
blank. Its input is seeded feature frames, not audio. Its transcripts are
sequences of three made-up words, " ba", " de" and " ko". Nothing here
recognizes speech; what the chapter checks is that streaming computes
exactly what it claims to compute, and how much limited context changes the
result.

Parakeet, chapter 153's model, cannot be streamed this way:

- its width is 1,024; chapter 153 now runs that full width offline, while
  this chapter's qualified stream remains the small seeded profile;
- it was trained with full attention and centred depthwise convolutions, so
  restricting it to past context gives a different model, not a streaming
  copy of the same one. The separately trained cache-aware RNNT candidate is
  pinned in the follow-up manifest, but has not been run by this chapter;
- its decoder is chapter 153's TDT transducer, which this chapter does not
  stream. The stream here decodes with chapter 152's CTC head.

## Chunks and frames

The input is log-mel feature frames, one every 10 ms. Chapter 152's
subsampling turns 8 feature frames into one encoded frame, 80 ms of audio. A
chunk carries `8 x chunk` feature frames, and a stream is set by two
numbers:

<!-- source: psionic/lib/listen.coax types -->
```jai
LsStatus :: enum u8 {ok;shape;limit;closed;order;duplicate;gap;length;capacity;number;}
// chunk: encoded frames a chunk carries (8 feature frames each, 80 ms);
// left: chunks before its own that a frame may attend to.
LsContext :: struct {chunk:int;left:int;}
// Offsets into the caller's work array: the position rows of every layer,
// the key, value and gated-frame rings, the kept tail of input, the
// subsampling window and its work, and the frames being built.
LsPlan :: struct {status:LsStatus;cap:int;rows:int;window:int;at_p:int;at_k:int;at_v:int;at_g:int;at_tail:int;at_window:int;
    at_sub:int;at_enc:int;at_h:int;at_q:int;at_ctx:int;work:int;}
// The stream between chunks. Latency counts are in feature frames (10 ms).
LsStream :: struct {open:bool;shape:CfShape;context:LsContext;plan:LsPlan;utterance:int;next:int;frames:int;encoded:int;
    tail:int;prev:int;tokens:int;first:int;lag:int;}
// written: tokens this chunk appended; total: tokens in the hypothesis;
// final: this chunk ended the utterance. On a final chunk, first is the
// audio heard when the first token appeared (-1 if none), lag the longest
// any token waited behind its own audio, held the audio only the end of
// turn released.
LsResult :: struct {status:LsStatus;written:int;total:int;encoded:int;final:bool;first:int;lag:int;held:int;}
```

A chunk is identified by its utterance number and its sequence number in
that utterance. The last chunk of an utterance carries the end of turn and
may be shorter.

### Subsampling a chunk

Subsampling is three stride-2 convolutions with three taps each. Follow one
encoded frame `u` back through them: it reads stage-2 frames `2u - 1` to
`2u + 1`, which read stage-1 frames `4u - 3` to `4u + 3`, which read feature
frames `8u - 7` to `8u + 7`. So encoded frame `u` needs no feature frame
after its own block of 8 (frames `8u` to `8u + 7`): the subsampling adds no
look-ahead. It does reach 7 frames back into the block before.

The stream therefore keeps the last 8 feature frames of each chunk and runs
chapter 152's subsampling on a *window*: those 8 frames, then the new chunk.
The window starts on a multiple of 8, so its stride-2 stages line up with the
whole utterance's. Its first encoded frame is wrong, because it reads the
window's zero padding where the utterance has real frames; that frame
belongs to the previous chunk anyway and is dropped. Every other frame reads
exactly what it reads in the whole utterance.

A worked case: chunks of 2 encoded frames (16 feature frames, 160 ms), and an
utterance of 51 feature frames, sent as 16, 16, 16 and an end-of-turn chunk
of 3. Chapter 152 gives the utterance `cf_length(51)` = 7 encoded frames
(51 to 26 to 13 to 7, halving and rounding up).

| Chunk | Window | Window's encoded frames | Dropped | New frames |
| --- | --- | --- | --- | --- |
| 0 (16 frames) | 16 frames | 2 | none | 0, 1 |
| 1 (16 frames) | 8 + 16 = 24 | 3 | 1 | 2, 3 |
| 2 (16 frames) | 24 | 3 | 1 | 4, 5 |
| end (3 frames) | 8 + 3 = 11 | 2 (11 to 6 to 3 to 2) | 1 | 6 |

Mid-utterance a chunk completes `frames / 8` encoded frames in all; the end
of turn completes chapter 152's full count:

<!-- source: psionic/lib/listen.coax keys -->
```jai
// The keys frame i may read when frames 0 .. known - 1 exist: its own chunk
// and context.left chunks before it, never a later chunk.
LsKeys :: struct {from:int;to:int;}
ls_keys :: (c:LsContext,i:int,known:int)->LsKeys {
    // max(0, a) is -min(0, -a); chapter 153's tdt_min has no comparison.
    own:=i/c.chunk;
    return LsKeys.{from=-tdt_min(0,(c.left-own)*c.chunk),to=tdt_min((own+1)*c.chunk,known)};
}
// Encoded frames a chunk completes, given the feature frames heard so far and
// the encoded frames already made. Mid-utterance a chunk completes
// frames / 8; the end of turn completes the rest, as chapter 152 counts them.
ls_new_frames :: (frames:int,encoded:int,end:bool)->int {
    if end {return cf_length(frames)-encoded;}
    return frames/8-encoded;
}
```

### Attention with limited context

With `chunk = 2` and `left = 1`, frame `i` may read the keys of chunk
`i / 2` and the chunk before. For the 7 frames above:

| Frame | Its chunk | Keys it reads |
| --- | --- | --- |
| 0 | 0 | 0, 1 |
| 3 | 1 | 0 to 3 |
| 4 | 2 | 2 to 5 |
| 6 | 3 | 4 to 6 (frame 7 does not exist) |

The relative position `i - j` of a query and a key it reads runs from
`-(chunk - 1)` (a key later in the same chunk) to `(left + 1) chunk - 1`.
Here that is -1 to 3: five position rows per layer, computed once when the
stream opens. Each row is chapter 152's sinusoid of that position through
the layer's position map, the same row chapter 152's shift reads for
`i - j`, so the scores are the same numbers.

Each layer keeps its keys and values in a ring: frame `j` lives in slot
`j mod cap`. A chunk reads at most `(left + 1) chunk` frames of keys, and the
convolution reads at most `chunk + kernel - 1` frames of its inputs, so a
ring of `cap = (left + 1) chunk + kernel` slots never overwrites a frame that
can still be read. In the worked case, `cap` = 2 x 2 + 3 = 7.

<!-- source: psionic/lib/listen.coax plan -->
```jai
ls_plan :: (s:CfShape,c:LsContext)->LsPlan {
    status:=cf_shape_check(s);if status!=CfStatus.ok {return LsPlan.{status=ifx status==CfStatus.limit then LsStatus.limit else LsStatus.shape};}
    if c.chunk<1 || c.left<0 {return LsPlan.{status=LsStatus.shape};}
    if c.chunk>LS_CHUNK_MAX || (c.left+1)*c.chunk>LS_KEYS_MAX {return LsPlan.{status=LsStatus.limit};}
    d:=s.dim;p:=LsPlan.{cap=(c.left+1)*c.chunk+s.kernel,rows=(c.left+2)*c.chunk-1,window=8*(c.chunk+1)};
    ring:=s.layers*p.cap*d;
    p.at_k=s.layers*p.rows*d;p.at_v=p.at_k+ring;p.at_g=p.at_v+ring;p.at_tail=p.at_g+ring;
    p.at_window=p.at_tail+8*s.mels;p.at_sub=p.at_window+p.window*s.mels;p.at_enc=p.at_sub+cf_plan(s,1,p.window).work;
    p.at_h=p.at_enc+(c.chunk+1)*d;p.at_q=p.at_h+c.chunk*d;p.at_ctx=p.at_q+c.chunk*d;p.work=p.at_ctx+c.chunk*d;
    return p;
}
```

Opening a stream fills the position rows and starts utterance 0:

<!-- source: psionic/lib/listen.coax open -->
```jai
// Check the encoder and context, fill each layer's position rows (relative
// positions -(chunk - 1) to (left + 1) chunk - 1, as chapter 152 maps them),
// and start utterance 0. Nothing is written on refusal.
ls_open :: (s:CfShape,c:LsContext,w:[]f32,work:[]f32,st:[]LsStream)->LsStatus {
    p:=ls_plan(s,c);if p.status!=LsStatus.ok {return p.status;}
    if w.count!=cf_weight_count(s) || st.count!=1 {return LsStatus.shape;}
    if work.count<p.work {return LsStatus.capacity;}
    if !cf_weights_ok(s,w) {return LsStatus.number;}
    d:=s.dim;x:[CF_DIM_MAX]f32;
    for l:0..<s.layers {
        a:=cf_layer_at(s,cf_layer_base(s,l)).att;
        for r:0..<p.rows {cf_position(r-(c.chunk-1),d,x[..]);cf_linear(w,a.pos,d,d,false,x[..],1,cf_part(work,l*p.rows+r,d));}
    }
    st[0]=LsStream.{open=true,shape=s,context=c,plan=p,prev=-1,first=-1};
    return LsStatus.ok;
}
```

### One layer on one chunk

The layer is chapter 152's, in the same order (half feed-forward, attention,
convolution, half feed-forward, layer norm), run on the chunk's new frames
only. The queries are the new frames'; the keys and values go into the ring
first, so a frame can read later keys of its own chunk.

<!-- source: psionic/lib/listen.coax layer -->
```jai
// One layer on the count new frames h, which sit at encoded positions e0 ..
// e0 + count - 1: chapter 152's layer with the attention keys limited to
// ls_keys and the depthwise taps reading only the past.
ls_layer :: (st:LsStream,w:[]f32,l:int,h:[]f32,e0:int,count:int,work:[]f32) {
    s:=st.shape;p:=st.plan;d:=s.dim;cap:=p.cap;L:=cf_layer_at(s,cf_layer_base(s,l));a:=L.att;v:=L.conv;
    ring:=l*cap;kr:=work[p.at_k..<p.at_v];vr:=work[p.at_v..<p.at_g];gr:=work[p.at_g..<p.at_tail];
    q:=work[p.at_q..<p.at_ctx];ctx:=work[p.at_ctx..<p.work];x:[CF_DIM_MAX]f32;
    for t:0..<count {
        row:=cf_part(h,t,d);cf_feed_forward(s,w,L.ff1,row);cf_norm(w,a.norm,d,row,x[..]);
        cf_linear(w,a.q,d,d,true,x[..],1,cf_part(q,t,d));
        slot:=ring+(e0+t)%cap;cf_linear(w,a.k,d,d,true,x[..],1,cf_part(kr,slot,d));cf_linear(w,a.v,d,d,true,x[..],1,cf_part(vr,slot,d));
    }
```

The scores are chapter 152's, with keys taken from `ls_keys` and the
position row taken by `i - j`. Keys are visited in increasing order, as the
whole-utterance computation visits them, so every compensated sum adds the
same terms in the same order:

<!-- source: psionic/lib/listen.coax attention -->
```jai
    dk:=d/s.heads;scale:=1/sqrt(f64(dk));score:[LS_KEYS_MAX]f64;
    for hd:0..<s.heads {for t:0..<count {
        i:=e0+t;keys:=ls_keys(st.context,i,e0+count);n:=keys.to-keys.from;
        for j:keys.from..<keys.to {
            ac:PsionicAccumulator;bd:PsionicAccumulator;
            k:=cf_part(kr,ring+j%cap,d);pos:=cf_part(work,l*p.rows+i-j+st.context.chunk-1,d);
            for m:hd*dk..<(hd+1)*dk {
                ac=psionic_ops_accumulate(ac,(f64(q[t*d+m])+f64(w[a.u+m]))*f64(k[m]));
                bd=psionic_ops_accumulate(bd,(f64(q[t*d+m])+f64(w[a.bias_v+m]))*f64(pos[m]));
            }
            score[j-keys.from]=(psionic_ops_total(ac)+psionic_ops_total(bd))*scale;
        }
        cf_softmax(score[0..<n]);
        for m:hd*dk..<(hd+1)*dk {
            o:PsionicAccumulator;
            for j:keys.from..<keys.to {o=psionic_ops_accumulate(o,score[j-keys.from]*f64(vr[(ring+j%cap)*d+m]));}
            ctx[t*d+m]=f32(psionic_ops_total(o));
        }
    }}
    y:[CF_DIM_MAX]f32;
    for t:0..<count {
        cf_linear(w,a.o,d,d,true,cf_part(ctx,t,d),1,y[..]);
        for m:0..<d {h[t*d+m]=f32(f64(h[t*d+m])+f64(y[m]));}
    }
```

The convolution module is chapter 152's except for the depthwise taps: tap
`j` of a kernel of `k` reads frame `t - (k - 1) + j`, so the newest tap reads
the current frame and none reads the future. The gated frames go into a ring
like the keys.

<!-- source: psionic/lib/listen.coax causal -->
```jai
    // Gated frames go into the ring; tap j of the depthwise convolution reads
    // frame t - (kernel - 1) + j, and a tap before the utterance reads nothing.
    two:[2*CF_DIM_MAX]f32;
    for t:0..<count {
        cf_norm(w,v.norm,d,cf_part(h,t,d),x[..]);cf_linear(w,v.pw1,2*d,d,true,x[..],1,two[..]);
        g:=cf_part(gr,ring+(e0+t)%cap,d);for c:0..<d {g[c]=f32(f64(two[c])*cf_sigmoid(f64(two[d+c])));}
    }
    for t:0..<count {
        i:=e0+t;
        for c:0..<d {
            acc:PsionicAccumulator;acc=psionic_ops_accumulate(acc,f64(w[v.dw+d*s.kernel+c]));
            for j:0..<s.kernel {
                u:=i-(s.kernel-1)+j;if u<0 {continue;}
                acc=psionic_ops_accumulate(acc,f64(w[v.dw+c*s.kernel+j])*f64(gr[(ring+u%cap)*d+c]));
            }
            b:=(psionic_ops_total(acc)-f64(w[v.bn+c]))/sqrt(f64(w[v.bn+d+c])+CF_EPSILON)*f64(w[v.bn+2*d+c])+f64(w[v.bn+3*d+c]);
            x[c]=f32(cf_silu(b));
        }
        cf_linear(w,v.pw2,d,d,true,x[..],1,y[..]);
        for c:0..<d {h[t*d+c]=f32(f64(h[t*d+c])+f64(y[c]));}
        row:=cf_part(h,t,d);cf_feed_forward(s,w,L.ff2,row);cf_norm(w,L.out,d,row,x[..]);
        for c:0..<d {row[c]=x[c];}
    }
```

## Partial and final hypotheses

Greedy CTC decoding (chapter 152) picks each frame's best class and emits it
unless it is the blank or repeats the class of the frame before. Both
depend only on frames that are already final. So the rule for partial
hypotheses here is the simplest one: a partial hypothesis never changes; each
chunk can only append to it. The one piece of decoder state that crosses a
chunk boundary is the previous frame's class.

The end-of-turn chunk flushes the frames that only the end could complete,
returns the final hypothesis, and resets every per-utterance counter. The
next chunk must be chunk 0 of the next utterance.

### Latency

Latency is counted in audio time, in feature frames of 10 ms, so it is the
same on every host:

- **first partial**: how much audio had arrived when the first token
  appeared;
- **longest wait**: for each token, the time from the end of its encoded
  frame's audio to the arrival of the chunk that completed it, at most;
- **held**: the audio after the last full chunk, which only the end of turn
  releases.

With chunks of 2 encoded frames, a token from a chunk's first frame (audio 0
to 80 ms) appears when the chunk completes at 160 ms: it waited 80 ms. A
token from the chunk's second frame waited 0. So the longest wait is
`8 (chunk - 1)` feature frames, and the first partial comes no earlier than
one chunk. The 51-frame utterance above holds 3 frames (30 ms) for the end of
turn. The time the voice engine takes to decide that the turn has ended is
not counted here; it is the voice engine's.

Admission comes first. A chunk is refused, and the stream left as it was,
when it repeats the chunk just taken (`duplicate`), is older than that or
belongs to an earlier utterance (`order`), or skips ahead in sequence or
utterance (`gap`). A mid-utterance chunk must carry exactly `8 x chunk`
frames, an utterance at least 1 and at most 2,048 frames, and every feature
must be finite and at most 10,000 in size:

<!-- source: psionic/lib/listen.coax admit -->
```jai
// A chunk is (utterance, seq): the next of the current utterance, or it is
// refused. Mid-utterance it carries exactly 8 x chunk feature frames; the
// end-of-turn chunk carries at most that many, and the utterance needs at
// least one frame and at most chapter 152's 2,048.
ls_admit :: (st:LsStream,x:[]f32,n:int,utterance:int,seq:int,end:bool,tokens:[]int)->LsStatus {
    if !st.open {return LsStatus.closed;}
    if seq<0 {return LsStatus.shape;}
    if utterance<st.utterance || (utterance==st.utterance && seq<st.next-1) {return LsStatus.order;}
    if utterance==st.utterance && seq==st.next-1 {return LsStatus.duplicate;}
    if utterance>st.utterance || seq>st.next {return LsStatus.gap;}
    s:=st.shape;whole:=8*st.context.chunk;
    if n<0 || n>whole || (!end && n!=whole) || x.count!=s.mels*n {return LsStatus.shape;}
    if st.frames+n<1 || st.frames+n>CF_FRAMES_MAX {return LsStatus.length;}
    if tokens.count<st.tokens+ls_new_frames(st.frames+n,st.encoded,end) {return LsStatus.capacity;}
    for v:x {if !psionic_tensor_finite_bits(f32_bits(v)) || abs(v)>CF_FEATURE_MAX {return LsStatus.number;}}
    return LsStatus.ok;
}
```

Then the chunk is encoded and decoded:

<!-- source: psionic/lib/listen.coax push -->
```jai
// Admit one chunk of n feature frames (mel-major: x[m n + t]), encode the
// frames it completes, decode them greedily onto tokens, and on end of turn
// close the utterance and open the next. The new encoded frames stay in
// ls_frames until the next chunk.
ls_push :: (st:[]LsStream,w:[]f32,work:[]f32,x:[]f32,n:int,utterance:int,seq:int,end:bool,tokens:[]int)->LsResult {
    if st.count!=1 {return LsResult.{status=LsStatus.shape};}
    status:=ls_admit(st[0],x,n,utterance,seq,end,tokens);if status!=LsStatus.ok {return LsResult.{status=status};}
    if w.count!=cf_weight_count(st[0].shape) || work.count<st[0].plan.work {return LsResult.{status=LsStatus.shape};}
    t:=st[0];s:=t.shape;p:=t.plan;m:=s.mels;d:=s.dim;
    // The window: the kept 8 frames of the chunk before, then this chunk.
    wn:=t.tail+n;win:=work[p.at_window..<p.at_window+m*wn];tail:=work[p.at_tail..<p.at_window];
    for f:0..<m {for i:0..<wn {
        if i<t.tail {win[f*wn+i]=tail[f*8+i];} else {win[f*wn+i]=x[f*n+i-t.tail];}
    }}
    t.frames+=n;count:=ls_new_frames(t.frames,t.encoded,end);
    h:=work[p.at_h..<p.at_h+count*d];
    // The window's first encoded frame saw zeros, not the frames before its
    // overlap; it belongs to the chunk before and is dropped.
    enc:=work[p.at_enc..<p.at_h];only:=[1]int.[wn];
    cf_subsample(s,w,win,only[..],1,wn,work[p.at_sub..<p.at_enc],enc);
    skip:=t.tail/8;for i:0..<count*d {h[i]=enc[skip*d+i];}
    for l:0..<s.layers {ls_layer(t,w,l,h,t.encoded,count,work);}
```

<!-- source: psionic/lib/listen.coax decode -->
```jai
    // Greedy CTC on the new frames: a frame's best class (the lowest on a
    // tie) is emitted unless it is the blank or repeats the frame before. The
    // frame before may be in the previous chunk.
    k:=s.vocab+1;z:[CF_VOCAB_MAX+1]f32;written:=0;
    for e:0..<count {
        cf_head_row(s,w,cf_part(h,e,d),z[0..<k]);
        best:=0;for c:1..<k {if z[c]>z[best] {best=c;}}
        if best!=s.vocab && best!=t.prev {
            tokens[t.tokens+written]=best;written+=1;
            // How long this token's audio, frames 8 (encoded + e) to
            // 8 (encoded + e + 1) - 1, waited before it appeared.
            wait:=t.frames-8*(t.encoded+e+1);t.lag-=tdt_min(0,t.lag-wait);
            if t.first==-1 {t.first=t.frames;}
        }
        t.prev=best;
    }
    r:=LsResult.{written=written,total=t.tokens+written,encoded=count,final=end,first=t.first,lag=t.lag,held=t.frames-8*(t.frames/(8*t.context.chunk))*t.context.chunk};
    t.tokens+=written;t.encoded+=count;t.next=seq+1;
    // Keep the last 8 frames for the next window.
    if !end {for f:0..<m {for i:0..<8 {tail[f*8+i]=win[f*wn+wn-8+i];}}t.tail=8;}
    if end {
        o:=t;t=LsStream.{open=true,shape=o.shape,context=o.context,plan=o.plan,utterance=o.utterance+1,prev=-1,first=-1};
    }
    st[0]=t;
    return r;
}
// The encoded frames the last chunk produced.
ls_frames :: (st:LsStream,work:[]f32,count:int)->[]f32 {return work[st.plan.at_h..<st.plan.at_h+count*st.shape.dim];}
```

## Run it

The example, [`listen.coax`](../examples/listen.coax), sets up the synthetic encoder, then streams two utterances in
chunks of 160 ms with 320 ms of left context.

Seeded weights leave every encoded frame close to one common frame, so a
seeded CTC head picks the same class at every frame. To get transcripts
that can disagree, the example rescales the head so that it reads each
frame's difference from the mean of a calibration utterance. This is part of
the synthetic setup, not of streaming:

<!-- source: psionic/lib/listen.coax divergence -->
```jai
// Word error rate of hyp against ref, as chapter 153 counts it, with each
// token a word: ls_edits is the edit distance, the rate is that over ref's
// length (here in parts per thousand, rounded).
LS_WORDS :: [8]string.["a","b","c","d","e","f","g","h"];
ls_edits :: (ref:[]int,hyp:[]int)->int {
    r:[LS_KEYS_MAX]string;y:[LS_KEYS_MAX]string;row:[LS_KEYS_MAX+1]int;
    for i:0..<ref.count {r[i]=LS_WORDS[ref[i]%8];}
    for i:0..<hyp.count {y[i]=LS_WORDS[hyp[i]%8];}
    return tdt_distance(r[0..<ref.count],y[0..<hyp.count],row[..]);
}
ls_rate :: (edits:int,words:int)->int {return (1000*edits+words/2)/words;}
// Seeded weights leave every encoded frame close to one common frame, so
// the CTC head would pick the same class everywhere. This rescales the head
// by k and moves its bias so that it reads each frame's difference from the
// mean of T calibration frames: the classes then follow the input. It makes
// a synthetic encoder whose transcripts can disagree; it does not make one
// that recognizes anything.
ls_spread_head :: (s:CfShape,w:[]f32,enc:[]f32,T:int,k:f64) {
    d:=s.dim;at:=cf_head_at(s);rows:=s.vocab+1;mean:[CF_DIM_MAX]f64;
    for c:0..<d {sum:PsionicAccumulator;for t:0..<T {sum=psionic_ops_accumulate(sum,f64(enc[t*d+c]));}mean[c]=psionic_ops_total(sum)/f64(T);}
    for r:0..<rows {
        b:PsionicAccumulator;
        for c:0..<d {w[at+r*d+c]=f32(k*f64(w[at+r*d+c]));b=psionic_ops_accumulate(b,-f64(w[at+r*d+c])*mean[c]);}
        w[at+rows*d+r]=f32(psionic_ops_total(b));
    }
}
```

<!-- source: psionic/examples/listen.coax setup -->
```jai
    s:=ls_test_shape();w:[2782]f32;#assert cf_init(s,w[..],21)==CfStatus.ok;
    work:[20000]f32;st:[1]LsStream;x:[2560]f32;enc:[160]f32;tokens:[20]int;full:[20]int;
    // The head reads each frame's difference from the mean of a calibration utterance.
    features(x[..],160,30);#assert ls_open(s,LsContext.{chunk=20,left=0},w[..],work[..],st[..])==LsStatus.ok;
    #assert ls_ref_stream(st[..],w[..],work[..],x[..],160,0,tokens[..],enc[..]).status==LsStatus.ok;
    ls_spread_head(s,w[..],enc[..],20,30);
```

<!-- source: psionic/examples/listen.coax stream -->
```jai
    c:=LsContext.{chunk=2,left=2};per:=8*c.chunk;
    print("chunks of % frames (% ms), % chunks of left context\n",per,10*per,c.left);
    #assert ls_open(s,c,w[..],work[..],st[..])==LsStatus.ok;
    lengths:=[2]int.[150,40];chunk:[256]f32;
    for u:0..<2 {
        N:=lengths[u];features(x[..],N,31+u);at:=0;seq:=0;
        print("utterance %: % ms of audio\n",u,10*N);
        while true {
            n:=N-at;end:=n<=per;if !end {n=per;}
            ls_ref_slice(x[..],16,N,at,n,chunk[0..<16*n]);
            r:=ls_push(st[..],w[..],work[..],chunk[0..<16*n],n,u,seq,end,tokens[..]);
            #assert r.status==LsStatus.ok;
            at+=n;seq+=1;
            if !end {print("  chunk %: heard % ms, % new: ",seq-1,10*at,r.written);show(tokens[..],0,r.total);print("\n");continue;}
            print("  end of turn: heard % ms, % new, final: ",10*at,r.written);show(tokens[..],0,r.total);print("\n");
            print("  first partial at % ms, longest wait % ms, % ms released only by the end of turn\n",10*r.first,10*r.lag,10*r.held);
            break;
        }
    }
```

```shell
coax/bin/coax run psionic/examples/listen.coax
```

```text
chunks of 16 frames (160 ms), 2 chunks of left context
utterance 0: 1500 ms of audio
  chunk 0: heard 160 ms, 1 new:  de
  chunk 1: heard 320 ms, 1 new:  de de
  chunk 2: heard 480 ms, 1 new:  de de ba
  chunk 3: heard 640 ms, 2 new:  de de ba de ko
  chunk 4: heard 800 ms, 2 new:  de de ba de ko ba de
  chunk 5: heard 960 ms, 1 new:  de de ba de ko ba de ba
  chunk 6: heard 1120 ms, 1 new:  de de ba de ko ba de ba ba
  chunk 7: heard 1280 ms, 1 new:  de de ba de ko ba de ba ba ko
  chunk 8: heard 1440 ms, 2 new:  de de ba de ko ba de ba ba ko ba de
  end of turn: heard 1500 ms, 1 new, final:  de de ba de ko ba de ba ba ko ba de ba
  first partial at 160 ms, longest wait 80 ms, 60 ms released only by the end of turn
utterance 1: 400 ms of audio
  chunk 0: heard 160 ms, 1 new:  de
  chunk 1: heard 320 ms, 1 new:  de ba
  end of turn: heard 400 ms, 0 new, final:  de ba
  first partial at 160 ms, longest wait 80 ms, 80 ms released only by the end of turn
```

Each partial extends the one before it. The 1,500 ms utterance is 9 full
chunks (1,440 ms) and an end-of-turn chunk of 60 ms; the 400 ms utterance
holds 80 ms for the end. Both waits are the 80 ms the worked case predicts.
Utterance 1 starts from chunk 0 on a clean state.

## The independent check

The check reference in
[`listen-checks.coax`](../tests/listen-checks.coax) never calls the stream.
It encodes the whole utterance at once: chapter 152's subsampling over every
frame, then each layer over every frame, scanning every key and keeping
those the chunk rule allows, computing each pair's position row from its
sinusoid with no table, and running the depthwise convolution over the whole
gated sequence. With one chunk at least as long as the utterance and no left
context, the rule allows every key: that is the full-utterance offline
encoder (with causal convolutions).

The checks in `listen.coax` compare them:

- for six contexts, from one-frame chunks with no left context to one chunk
  covering the utterance, the stream's encoded frames equal the reference's
  bit for bit and the transcripts equal chapter 152's greedy decoding of
  them;
- with one convolution tap, causal and centred convolutions are the same
  thing, so a stream with the whole utterance in one chunk must equal
  chapter 152's `cf_encode`, whose attention goes through the relative shift
  instead of position rows. It does, bit for bit, and limited context or
  three taps make it differ;
- each token's appearance time, recounted from the reference's per-frame
  classes and the chunk boundaries alone, gives the same first partial,
  longest wait and held audio for four contexts and lengths;
- a finished utterance leaves nothing behind: the next one decodes exactly as
  in a fresh stream.

The example shows the first two on the 1,500 ms utterance, and then measures
what limited context costs. For eight more utterances of 1.6 s, it compares
each context's final transcript with the full-context transcript of the same
audio, by chapter 153's word error rate pooled over the eight:

<!-- source: psionic/examples/listen.coax offline -->
```jai
    // The whole first utterance as one chunk against the full-context
    // reference, then the 160 ms stream against the masked reference.
    rw:[8000]f32;ref:[160]f32;logp:[80]f32;want:[20]int;
    features(x[..],150,31);
    contexts:=[2]LsContext.[LsContext.{chunk=19,left=0},c];heard:=0;
    for k:0..<2 {
        b:=contexts[k];T:=ls_ref_encode(s,w[..],x[..],150,b.chunk,b.left,ref[..],rw[..]);n:=ls_ref_decode(s,w[..],ref[..],T,logp[..],want[..]);
        #assert ls_open(s,b,w[..],work[..],st[..])==LsStatus.ok;
        r:=ls_ref_stream(st[..],w[..],work[..],x[..],150,0,tokens[..],enc[..]);
        same:=r.total==n;for i:0..<n {same=same && tokens[i]==want[i];}
        print("chunk % left %: % encoded frames bit for bit: %, transcript the same: %, ",b.chunk,b.left,T,px_same(enc[0..<T*8],ref[0..<T*8]),same);
        if k==0 {for i:0..<n {full[i]=want[i];}heard=n;print("% words\n",n);}
        else {print("% edits from the full-context transcript\n",ls_edits(full[0..<heard],want[0..<n]));}
    }
```

<!-- source: psionic/examples/listen.coax divergence -->
```jai
    // Eight more utterances of 1.6 s: the edits against the full-context
    // transcript, pooled, for each context.
    chunks:=[7]int.[1,1,1,2,2,4,20];lefts:=[7]int.[0,4,19,0,9,4,0];
    print("chunk  left context  latest first partial  word error against full context\n");
    for k:0..<7 {
        b:=LsContext.{chunk=chunks[k],left=lefts[k]};edits:=0;words:=0;first:=0;
        for u:0..<8 {
            features(x[..],160,40+u);
            #assert ls_open(s,LsContext.{chunk=20,left=0},w[..],work[..],st[..])==LsStatus.ok;
            f:=ls_ref_stream(st[..],w[..],work[..],x[..],160,0,full[..],enc[..]);
            #assert ls_open(s,b,w[..],work[..],st[..])==LsStatus.ok;
            r:=ls_ref_stream(st[..],w[..],work[..],x[..],160,0,tokens[..],enc[..]);
            edits+=ls_edits(full[0..<f.total],tokens[0..<r.total]);words+=f.total;if r.first>first {first=r.first;}
        }
        rate:=ls_rate(edits,words);
        print("% ms  % ms  % ms  %/% = %.%",80*b.chunk,80*b.chunk*b.left,10*first,edits,words,rate/10,rate%10);print("%%\n");
    }
```

```text
chunk 19 left 0: 19 encoded frames bit for bit: true, transcript the same: true, 10 words
chunk 2 left 2: 19 encoded frames bit for bit: true, transcript the same: true, 3 edits from the full-context transcript
chunk  left context  latest first partial  word error against full context
80 ms  0 ms  160 ms  30/79 = 38.0%
80 ms  320 ms  160 ms  19/79 = 24.1%
80 ms  1520 ms  160 ms  14/79 = 17.7%
160 ms  0 ms  160 ms  34/79 = 43.0%
160 ms  1440 ms  160 ms  14/79 = 17.7%
320 ms  1280 ms  320 ms  13/79 = 16.5%
1600 ms  0 ms  1600 ms  0/79 = 0.0%
```

When the context covers the utterance, the difference is zero, and the first
partial waits for the whole utterance. More left context brings the
transcript closer to the full-context one, but even with every past frame
in view (1,520 ms) the transcripts differ in 17.7% of words, because no
frame sees the future. These numbers measure disagreement between two
versions of one synthetic encoder on random features; they say nothing about
how a trained streaming model would score. A trained model learns to work
with its context, which this seeded one cannot.

## What is refused

The second example, [`listen-refuse.coax`](../examples/listen-refuse.coax), sends chunks that should not be taken:

<!-- source: psionic/examples/listen-refuse.coax refuse -->
```jai
main :: () {
    s:=ls_test_shape();w:[2782]f32;#assert cf_init(s,w[..],21)==CfStatus.ok;
    work:[20000]f32;st:[1]LsStream;tokens:[20]int;x:[256]f32;
    for i:0..<256 {x[i]=f32(i%7)*0.25-0.75;}
    c:=LsContext.{chunk=2,left=2};
    print("before ls_open: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,0,false,tokens[..]).status);
    print("chunk of 256 frames: %\n",ls_open(s,LsContext.{chunk=256,left=0},w[..],work[..],st[..]));
    print("33 chunks of 8 frames in context: %\n",ls_open(s,LsContext.{chunk=8,left=32},w[..],work[..],st[..]));
    print("work one value short: %\n",ls_open(s,c,w[..],work[0..<ls_plan(s,c).work-1],st[..]));
    #assert ls_open(s,c,w[..],work[..],st[..])==LsStatus.ok;
    print("chunk 0 of utterance 0: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,0,false,tokens[..]).status);
    print("chunk 0 again: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,0,false,tokens[..]).status);
    print("chunk 2 before chunk 1: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,2,false,tokens[..]).status);
    print("utterance 1 before utterance 0 ends: %\n",ls_push(st[..],w[..],work[..],x[..],16,1,1,false,tokens[..]).status);
    print("15 frames mid-utterance: %\n",ls_push(st[..],w[..],work[..],x[0..<240],15,0,1,false,tokens[..]).status);
    print("room for no new tokens: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,1,false,tokens[0..<st[0].tokens]).status);
    x[37]=1.0e9;print("a feature of 10^9: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,1,false,tokens[..]).status);
    zero:f32=0;x[37]=zero/zero;print("a NaN feature: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,1,false,tokens[..]).status);
    x[37]=0;r:=ls_push(st[..],w[..],work[..],x[..],16,0,1,false,tokens[..]);
    print("chunk 1: %, % tokens so far\n",r.status,r.total);
    r=ls_push(st[..],w[..],work[..],x[0..<48],3,0,2,true,tokens[..]);print("end of turn with 3 frames: %, final %\n",r.status,r.final);
    print("chunk 3 of the ended utterance: %\n",ls_push(st[..],w[..],work[..],x[..],16,0,3,false,tokens[..]).status);
    print("utterance 1 ending with no frames: %\n",ls_push(st[..],w[..],work[..],x[0..<0],0,1,0,true,tokens[..]).status);
}
```

```shell
coax/bin/coax run psionic/examples/listen-refuse.coax
```

```text
before ls_open: closed
chunk of 256 frames: limit
33 chunks of 8 frames in context: limit
work one value short: capacity
chunk 0 of utterance 0: ok
chunk 0 again: duplicate
chunk 2 before chunk 1: gap
utterance 1 before utterance 0 ends: gap
15 frames mid-utterance: shape
room for no new tokens: capacity
a feature of 10^9: number
a NaN feature: number
chunk 1: ok, 1 tokens so far
end of turn with 3 frames: ok, final true
chunk 3 of the ended utterance: order
utterance 1 ending with no frames: length
```

The checks go further: each refusal is followed by the chunk that should
have come, and the utterance decodes exactly as an undisturbed one does.

## What remains

- **A real streaming model.** Offline Parakeet TDT v3 does not fit chapter
  152's bounds and is not cache-aware. The separate original-weight candidate
  is pinned in [the follow-up manifest](../fixtures/parakeet-streaming/manifest.json).
  An encoder/cache implementation that admits its width and recorded reference
  transitions are still needed before any real transcript can be streamed.
- **The transducer.** The pinned candidate uses RNNT, so its prediction-network
  state and last emitted token must persist across chunks. Chapter 153's
  separate TDT decoder jumps ahead by predicted
  durations; streaming it means carrying its prediction-network state and
  stopping a jump at the chunk's end. Here the stream decodes with CTC.
- **Features from audio.** Chapter 151's whole-utterance normalization cannot
  publish stable incremental features. The candidate uses no such
  normalization; it needs preemphasis/STFT carry and explicit first/final
  padding with bounded lookahead. Uncentred interior windows with overlap
  must reproduce the pinned frontend's centred reference frames.
- **The voice engine.** End of turn arrives here as a flag on the last chunk.
  [`verse/voice.coax`](../../verse/voice.coax) decides end of turn and
  barge-in, but it cannot be loaded into the same Coax program as these
  units (both define top-level names such as `down` and `cut`), so the
  events would come through
  [`verse/platform/voice.h`](../../verse/platform/voice.h). Barge-in is not
  handled. Nothing here changes the voice engine's consent or floor rules.
- **Wall-clock latency.** The latency here is in audio time. Compute time per
  chunk at P50 and P99 on a named host is not measured.
- **Other models.** Voxtral-Mini-4B-Realtime (Apache-2.0) was a candidate in
  the plan; it is not used.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- Noroozi et al., *Stateful Conformer with Cache-based Inference for
  Streaming Automatic Speech Recognition*, 2023 (NeMo's cache-aware streaming
  FastConformer).
