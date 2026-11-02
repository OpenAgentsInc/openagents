# 153. Transcribe with Parakeet

> **Status: partly implemented.** The pinned `parakeet-tdt-0.6b-v3`
> checkpoint's 723 tensors are checked against the architecture, name by
> name and shape by shape, and the token-and-duration (TDT) decoder runs on
> the CPU: the prediction network, the joint network and greedy decoding with
> fixed rules for blanks, durations and the most symbols a frame, then pieces
> to text and word and character error rates. Each matches an independent
> reference. The weights the decoder runs here are hand-set or seeded. Still
> to do: running chapter 152's encoder at Parakeet's size on the
> checkpoint's 627 million weights, comparing features, encoder outputs and
> token sequences with recorded reference tensors, reading the
> SentencePiece vocabulary from `tokenizer.json`, and WER and CER on a fixed,
> versioned test set per language slice.

[Chapter 152](ch152-a-speech-encoder.md) built NVIDIA NeMo's FastConformer
encoder: it turns 100 log-mel frames a second into 12.5 encoded frames a
second. Parakeet TDT turns those frames into text with a *transducer*
decoder. A CTC head (chapter 152) scores every frame on its own. A
transducer also remembers what it has already written: a small recurrent
network, the *prediction network*, reads the tokens emitted so far, and a
*joint network* combines that memory with the current frame to score the
next token.

A token-and-duration transducer adds one more output. Besides the token, the
joint network predicts how many frames to move ahead: 0, 1, 2, 3 or 4. A
skip of 0 means "stay on this frame and emit another token"; a skip above 0
lets the decoder pass over frames that hold nothing new, which is most of
what makes TDT decoding fast.

This chapter has two parts, in [`tdt.coax`](../lib/tdt.coax):

- **the checkpoint**: the tensor inventory of the pinned file, compared line
  for line with what the architecture says it must hold;
- **the decoder**: the prediction network, the joint network, the greedy
  decoding rules, pieces to text and error rates.

Everything stores f32 and sums in binary64 with chapter 4's compensated
accumulator, rounding once to f32 per stored value. That is the profile
`psionic-tdt-v1`. The bounds admit Parakeet's real decoder sizes: 8,192
tokens and the blank, encoder width 1,024, prediction and joint width 640,
2 LSTM layers, up to 8 durations, at most 10 symbols a frame and 256 encoded
frames (20.47 seconds of audio).

## The pinned checkpoint

The weights are the Hugging Face export of NVIDIA's model:

- repository `nvidia/parakeet-tdt-0.6b-v3`, revision
  `541d1f99c6b0c3cd0b11a95167540bb8edefd82b`;
- `model.safetensors`, 2,508,311,120 bytes, SHA-256
  `3a2026366188c8c68598edbbff92f8d11590a08e0ae2e6775544e7b07d6a5e11`;
- licence CC BY 4.0: the weights are NVIDIA's, used with attribution.

The repository also holds a `.nemo` archive, whose weights are a PyTorch
pickle. Reading a pickle can run code, so this chapter uses the safetensors
file instead: an 8-byte length, a JSON header, then raw little-endian
numbers. Nothing from the download is imported or run. The weights stay
outside the repository (for example in `~/models`); the repository keeps only
their inventory.

[`parakeet_manifest.py`](../tests/parakeet_manifest.py) hashes the whole
file against the pin, reads the header, checks that the blank token's
embedding row is zero (the decoder relies on that, below), and writes one
line per tensor, sorted by name, into
[`parakeet-tdt-manifest.coax`](../fixtures/parakeet-tdt-manifest.coax):

```text
decoder.decoder_projector.bias F32 640
decoder.decoder_projector.weight F32 640,640
decoder.embedding.weight F32 8193,640
decoder.lstm.bias_hh_l0 F32 2560
...
encoder.layers.9.self_attn.v_proj.weight F32 1024,1024
```

The same pin's `config.json` gives the architecture, which `tdt_parakeet`
records:

<!-- source: psionic/lib/tdt.coax pinned -->
```jai
// nvidia/parakeet-tdt-0.6b-v3's shape, from its config.json at the pinned
// revision: the encoder (chapter 152's FastConformer) and the decoder.
TdtConfig :: struct {mels:int;channels:int;enc:int;heads:int;hidden:int;kernel:int;layers:int;
    vocab:int;pred:int;pred_layers:int;joint:int;durations:int;}
tdt_parakeet :: ()->TdtConfig {
    return TdtConfig.{mels=128,channels=256,enc=1024,heads=8,hidden=4096,kernel=9,layers=24,
        vocab=8192,pred=640,pred_layers=2,joint=640,durations=5};
}
// Durations 0, 1, ..., n - 1.
tdt_durations :: (s:TdtShape,n:int)->TdtShape {t:=s;t.durations=n;for i:0..<n {t.dur[i]=i;}return t;}
tdt_parakeet_shape :: ()->TdtShape {
    return tdt_durations(TdtShape.{vocab=8192,enc=1024,pred=640,layers=2,joint=640,max_symbols=10},5);
}
```

## Run it

The programs are [`transcribe-tdt.coax`](../examples/transcribe-tdt.coax)
and [`transcribe-tdt-refuse.coax`](../examples/transcribe-tdt-refuse.coax);
the checks' references are in [`tdt-checks.coax`](../tests/tdt-checks.coax).

```shell
coax/bin/coax run psionic/examples/transcribe-tdt.coax
coax/bin/coax run psionic/examples/transcribe-tdt-refuse.coax
coax/bin/coax test psionic/lib/tdt.coax
coax/bin/coax mutate psionic/lib/tdt.coax --root-only
python3 -I psionic/tests/parakeet_manifest.py ~/models/parakeet-tdt-0.6b-v3-541d1f99
```

```text
checkpoint 541d1f99c6b0c3cd0b11a95167540bb8edefd82b: 723 lines, 723 expected, ok; 627057286 F32 parameters
decoder: 18128518 weights; 256 frames need 179078 work values
hand decoder: ok, 4 tokens
  tokens: 1 2 2 0
  frames: 0 1 1 3
  equals the two-loop reference: true
  text: "ohh now" against "oh now"
  word errors: 1 of 2
  character errors: 1 of 6
seeded decoder, 12 frames: ok, 8 tokens
  tokens: 2 2 3 3 2 1 1 2
  frames: 0 1 3 5 7 8 9 10
  equals the two-loop reference: true
```

The example takes about 7 seconds in a debug build; most of it is the
inventory check. The last command needs the downloaded file and prints
`723 tensors, the blank embedding row zero: the fixture matches the pinned
checkpoint`. The sections below work each line out by hand before showing
the code that prints it.

## The checkpoint, by hand

Count the tensors the architecture needs.

- The prediction network: an embedding of 8,193 rows (8,192 tokens and the
  blank), then two LSTM layers of four tensors each (`weight_ih`,
  `weight_hh`, `bias_ih`, `bias_hh`), then a projection to the joint width
  with its bias: 1 + 8 + 2 = 11.
- The joint network: the encoder projection and its bias, the head and its
  bias: 4.
- The subsampling: five convolutions (the full 3 × 3 at position 0, the
  depthwise 3 × 3 at 2 and 5, the pointwise 1 × 1 at 3 and 6; positions 1, 4
  and 7 are the ReLUs, which have no weights), each with a bias, then the
  linear map to width 1,024 and its bias: 12.
- Each of the 24 Conformer layers: 29 tensors (five layer norms of two, two
  feed-forward modules of two matrices, five attention maps and the biases
  u and v, the convolution module's three kernels and batch norm's five,
  including its scalar step counter).

11 + 4 + 12 + 24 × 29 = 27 + 696 = 723, the first line's count.

The linear map after the subsampling reads 256 channels at 128 / 8 = 16
frequencies: 4,096 inputs. Its line is
`encoder.subsampling.linear.weight F32 1024,4096`.

The decoder's parameters, matrix by matrix:

- embedding: 8,193 × 640 = 5,243,520;
- one LSTM layer: two 2,560 × 640 matrices, 3,276,800, and two biases of
  2,560, 5,120; two layers: 2 × 3,281,920 = 6,563,840;
- prediction projection: 640 × 640 + 640 = 410,240;
- encoder projection: 640 × 1,024 + 640 = 656,000;
- head: 8,198 outputs (8,192 tokens, the blank and 5 durations) × 640 +
  8,198 = 5,254,918.

The sum is 18,128,518, the second line's first number. The whole file holds
627,057,286 F32 values; the encoder is the other 608,928,768.

The inventory shows one thing chapter 152 must change before it can load
these weights: in the checkpoint, the Conformer layers' linear maps and
convolutions have no biases (`attention_bias` and `convolution_bias` are
false in `config.json`). Chapter 152's layout gives each of them a bias, so
loading would set those to zero, or the layout needs a bias-free form. The
subsampling convolutions do have biases, as chapter 152 assumes.

`tdt_expected` gives the tensor the architecture requires at each position,
`tdt_line` writes it as a manifest line, and `tdt_inventory` requires each
line to occur in the manifest exactly once, and nothing else:

<!-- source: psionic/lib/tdt.coax checkpoint -->
```jai
// The tensor a checkpoint of shape c must hold at position i (0 to
// tdt_expected_count(c) - 1): its name (a prefix, a layer number when layer
// is not -1, a suffix), dtype and dimensions. The names are Hugging Face's
// for ParakeetForTDT.
TdtTensor :: struct {prefix:string;layer:int;suffix:string;dtype:string;rank:int;dims:[4]int;}
tdt_d1 :: (a:int)->TdtTensor {t:TdtTensor;t.rank=1;t.dims[0]=a;return t;}
tdt_d2 :: (a:int,b:int)->TdtTensor {t:=tdt_d1(a);t.rank=2;t.dims[1]=b;return t;}
tdt_d3 :: (a:int,b:int,c:int)->TdtTensor {t:=tdt_d2(a,b);t.rank=3;t.dims[2]=c;return t;}
tdt_d4 :: (a:int,b:int,c:int,d:int)->TdtTensor {t:=tdt_d3(a,b,c);t.rank=4;t.dims[3]=d;return t;}
tdt_tensor :: (prefix:string,layer:int,suffix:string,dims:TdtTensor)->TdtTensor {
    t:=dims;t.prefix=prefix;t.layer=layer;t.suffix=suffix;t.dtype="F32";return t;
}
tdt_whole :: (name:string,dims:TdtTensor)->TdtTensor {return tdt_tensor(name,-1,"",dims);}
TDT_LAYER_TENSORS :: 29;
TDT_LAYER_NAMES :: [29]string.[".conv.depthwise_conv.weight",".conv.norm.bias",".conv.norm.num_batches_tracked",
    ".conv.norm.running_mean",".conv.norm.running_var",".conv.norm.weight",".conv.pointwise_conv1.weight",".conv.pointwise_conv2.weight",
    ".feed_forward1.linear1.weight",".feed_forward1.linear2.weight",".feed_forward2.linear1.weight",".feed_forward2.linear2.weight",
    ".norm_conv.bias",".norm_conv.weight",".norm_feed_forward1.bias",".norm_feed_forward1.weight",".norm_feed_forward2.bias",
    ".norm_feed_forward2.weight",".norm_out.bias",".norm_out.weight",".norm_self_att.bias",".norm_self_att.weight",
    ".self_attn.bias_u",".self_attn.bias_v",".self_attn.k_proj.weight",".self_attn.o_proj.weight",".self_attn.q_proj.weight",
    ".self_attn.relative_k_proj.weight",".self_attn.v_proj.weight"];
// Tensor k of an encoder layer, in the order of TDT_LAYER_NAMES. Batch
// norm's step counter is a scalar I64; the layers' linear maps and
// convolutions have no biases.
tdt_layer_tensor :: (c:TdtConfig,layer:int,k:int)->TdtTensor {
    e:=c.enc;name:=TDT_LAYER_NAMES[k];p:="encoder.layers.";
    if k==0 {return tdt_tensor(p,layer,name,tdt_d3(e,1,c.kernel));}
    if k==2 {none:TdtTensor;t:=tdt_tensor(p,layer,name,none);t.dtype="I64";return t;}
    if k==6 {return tdt_tensor(p,layer,name,tdt_d3(2*e,e,1));}
    if k==7 {return tdt_tensor(p,layer,name,tdt_d3(e,e,1));}
    if k==8 || k==10 {return tdt_tensor(p,layer,name,tdt_d2(c.hidden,e));}
    if k==9 || k==11 {return tdt_tensor(p,layer,name,tdt_d2(e,c.hidden));}
    if k==22 || k==23 {return tdt_tensor(p,layer,name,tdt_d2(c.heads,e/c.heads));}
    if k>=24 {return tdt_tensor(p,layer,name,tdt_d2(e,e));}
    return tdt_tensor(p,layer,name,tdt_d1(e));
}
// The decoder (projection, embedding, LSTM), the encoder projection and the
// head, then the subsampling, then the encoder layers.
tdt_expected_count :: (c:TdtConfig)->int {return 3+4*c.pred_layers+4+12+c.layers*TDT_LAYER_TENSORS;}
tdt_expected :: (c:TdtConfig,i:int)->TdtTensor {
    g:=4*c.pred;outputs:=c.vocab+1+c.durations;ch:=c.channels;
    if i==0 {return tdt_whole("decoder.decoder_projector.bias",tdt_d1(c.joint));}
    if i==1 {return tdt_whole("decoder.decoder_projector.weight",tdt_d2(c.joint,c.pred));}
    if i==2 {return tdt_whole("decoder.embedding.weight",tdt_d2(c.vocab+1,c.pred));}
    j:=i-3;
    if j<4*c.pred_layers {
        names:=[4]string.["decoder.lstm.bias_hh_l","decoder.lstm.bias_ih_l","decoder.lstm.weight_hh_l","decoder.lstm.weight_ih_l"];
        return tdt_tensor(names[j%4],j/4,"",ifx j%4<2 then tdt_d1(g) else tdt_d2(g,c.pred));
    }
    j-=4*c.pred_layers;
    if j==0 {return tdt_whole("encoder_projector.bias",tdt_d1(c.joint));}
    if j==1 {return tdt_whole("encoder_projector.weight",tdt_d2(c.joint,c.enc));}
    if j==2 {return tdt_whole("joint.head.bias",tdt_d1(outputs));}
    if j==3 {return tdt_whole("joint.head.weight",tdt_d2(outputs,c.joint));}
    j-=4;
    if j<10 {
        // Convolutions 0, 2 and 5 are 3 x 3 (the first full, the others
        // depthwise); 3 and 6 are pointwise. Each has a bias.
        conv:=[5]int.[0,2,3,5,6];n:=conv[j/2];p:="encoder.subsampling.layers.";
        if j%2==0 {return tdt_tensor(p,n,".bias",tdt_d1(ch));}
        if n==3 || n==6 {return tdt_tensor(p,n,".weight",tdt_d4(ch,ch,1,1));}
        return tdt_tensor(p,n,".weight",tdt_d4(ch,1,3,3));
    }
    if j==10 {return tdt_whole("encoder.subsampling.linear.bias",tdt_d1(c.enc));}
    if j==11 {return tdt_whole("encoder.subsampling.linear.weight",tdt_d2(c.enc,ch*(c.mels/8)));}
    j-=12;return tdt_layer_tensor(c,j/TDT_LAYER_TENSORS,j%TDT_LAYER_TENSORS);
}
```

<!-- source: psionic/lib/tdt.coax inventory -->
```jai
// One manifest line: name, dtype, dimensions joined by commas. The byte
// count written into out (at least 160 bytes).
tdt_put :: (out:[]u8,n:int,s:string)->int {for i:0..<s.count {out[n+i]=s[i];}return n+s.count;}
tdt_put_int :: (out:[]u8,n:int,v:int)->int {
    digits:[20]u8;k:=0;x:=v;
    while true {digits[k]=cast(u8)('0'+x%10);k+=1;x/=10;if x==0 {break;}}
    m:=n;while k>0 {k-=1;out[m]=digits[k];m+=1;}
    return m;
}
tdt_line :: (t:TdtTensor,out:[]u8)->int {
    n:=tdt_put(out,0,t.prefix);if t.layer>=0 {n=tdt_put_int(out,n,t.layer);}
    n=tdt_put(out,n,t.suffix);n=tdt_put(out,n," ");n=tdt_put(out,n,t.dtype);n=tdt_put(out,n," ");
    for i:0..<t.rank {if i>0 {n=tdt_put(out,n,",");}n=tdt_put_int(out,n,t.dims[i]);}
    return n;
}
// A manifest (one line a tensor) matches shape c when it has exactly the
// expected lines, each once, in any order. The expected lines are distinct
// (a test compares every pair), so this is a one-to-one match. Otherwise the result names the
// first expected tensor that is missing or repeats (bad), or
// bad = -1 when every expected line is there once but there are more lines.
// parameters counts the F32 values.
// A manifest of more than TDT_LINES_MAX lines is refused unread.
TdtInventory :: struct {status:TdtStatus;lines:int;parameters:int;bad:int;}
TDT_LINES_MAX :: 1024;
tdt_inventory :: (c:TdtConfig,manifest:string)->TdtInventory {
    ends:[1025]int;lines:=0;ends[0]=-1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==TDT_LINES_MAX {return TdtInventory.{status=TdtStatus.checkpoint,lines=lines+1,bad=-1};}
        lines+=1;ends[lines]=i;
    }}
    r:TdtInventory;r.lines=lines;buf:[160]u8;
    for e:0..<tdt_expected_count(c) {
        t:=tdt_expected(c,e);n:=tdt_line(t,buf[..]);line:=cast(string)buf[0..<n];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==n && manifest[from..<ends[k+1]]==line {found+=1;}}
        if found!=1 {return TdtInventory.{status=TdtStatus.checkpoint,lines=lines,bad=e};}
        if t.dtype=="F32" {size:=1;for k:0..<t.rank {size*=t.dims[k];}r.parameters+=size;}
    }
    if lines!=tdt_expected_count(c) {return TdtInventory.{status=TdtStatus.checkpoint,lines=lines,bad=-1};}
    return r;
}
```

The test runs the check on the pinned manifest and also on configurations
that are wrong in one way: 80 mels instead of 128, 23 layers, 4 durations.
Each is refused as `checkpoint` and names the first tensor that does not
fit.

## The types and the layout

<!-- source: psionic/lib/tdt.coax types -->
```jai
TdtStatus :: enum u8 {ok;shape;limit;rate;length;capacity;number;token;checkpoint;}
// vocab tokens, and the blank after them (index vocab); enc the encoder
// width; pred the prediction network's width; layers its LSTM layers; joint
// the joint network's width; durations how many entries of dur are used;
// max_symbols the most tokens one frame may emit.
TdtShape :: struct {vocab:int;enc:int;pred:int;layers:int;joint:int;durations:int;dur:[8]int;max_symbols:int;}
// Where each part of the flat f32 weight array starts, and how much work
// space a decode of some frame count needs, at the offsets given.
TdtPlan :: struct {status:TdtStatus;weights:int;lstm:int;dec:int;enc:int;head:int;outputs:int;
    work:int;at_h:int;at_c:int;at_x:int;at_dec:int;at_z:int;at_logits:int;at_gates:int;}
TdtResult :: struct {status:TdtStatus;written:int;}
```

<!-- source: psionic/lib/tdt.coax bounds -->
```jai
TDT_VOCAB_MAX :: 8192;
TDT_ENC_MAX :: 1024;
TDT_PRED_MAX :: 640;
TDT_JOINT_MAX :: 640;
TDT_LAYERS_MAX :: 2;
TDT_DURATIONS_MAX :: 8;
TDT_SKIP_MAX :: 8;
TDT_SYMBOLS_MAX :: 10;
TDT_FRAMES_MAX :: 256;          // chapter 152's CF_TOKENS_MAX: 2,048 feature frames
TDT_RATE :: 16000;
TDT_SAMPLES_MAX :: 327520;      // 20.47 s: 2,048 frames at a 160-sample hop, centred
TDT_FRAME_MAX :: 100000.0;      // what the joint reads, as chapter 152's CF_FRAME_MAX
TDT_WEIGHT_MAX :: 1000.0;
```

The decoder keeps all its weights in one f32 array, in the order below. For
256 frames the work array holds the projected frames, 256 × 640 = 163,840
values, then the LSTM's h and c (2 × 2 × 640 = 2,560), the layer input
(640), the projected prediction (640), the joint's hidden values (640), the
8,198 outputs and the gates (4 × 640 = 2,560): 163,840 + 2,560 + 640 + 640 +
640 + 8,198 + 2,560 = 179,078, the second line's last number.

<!-- source: psionic/lib/tdt.coax layout -->
```jai
// The flat weight array, in order: the embedding (vocab + 1, pred), whose
// blank row is zero; per LSTM layer W_ih (4 pred, pred), W_hh (4 pred, pred),
// b_ih and b_hh, gates in PyTorch's order i, f, g, o; the prediction
// projection (joint, pred) and bias; the encoder projection (joint, enc) and
// bias; the head (vocab + 1 + durations, joint) and bias.
tdt_shape_check :: (s:TdtShape)->TdtStatus {
    if s.vocab<1 || s.enc<1 || s.pred<1 || s.joint<1 || s.layers<1 || s.durations<1 || s.max_symbols<1 {return TdtStatus.shape;}
    if s.vocab>TDT_VOCAB_MAX || s.enc>TDT_ENC_MAX || s.pred>TDT_PRED_MAX || s.joint>TDT_JOINT_MAX || s.layers>TDT_LAYERS_MAX
        || s.durations>TDT_DURATIONS_MAX || s.max_symbols>TDT_SYMBOLS_MAX {return TdtStatus.limit;}
    // Durations rise strictly from 0 or more and stay within TDT_SKIP_MAX.
    if s.dur[0]<0 || s.dur[s.durations-1]>TDT_SKIP_MAX {return TdtStatus.shape;}
    for i:1..<s.durations {if s.dur[i]<=s.dur[i-1] {return TdtStatus.shape;}}
    return TdtStatus.ok;
}
tdt_plan :: (s:TdtShape,frames:int)->TdtPlan {
    status:=tdt_shape_check(s);if status!=TdtStatus.ok {return TdtPlan.{status=status};}
    if frames<1 || frames>TDT_FRAMES_MAX {return TdtPlan.{status=TdtStatus.length};}
    p:TdtPlan;p.lstm=(s.vocab+1)*s.pred;p.dec=p.lstm+s.layers*(8*s.pred*s.pred+8*s.pred);
    p.enc=p.dec+s.joint*s.pred+s.joint;p.head=p.enc+s.joint*s.enc+s.joint;
    p.outputs=s.vocab+1+s.durations;p.weights=p.head+p.outputs*s.joint+p.outputs;
    p.at_h=frames*s.joint;p.at_c=p.at_h+s.layers*s.pred;p.at_x=p.at_c+s.layers*s.pred;p.at_dec=p.at_x+s.pred;
    p.at_z=p.at_dec+s.joint;p.at_logits=p.at_z+s.joint;p.at_gates=p.at_logits+p.outputs;p.work=p.at_gates+4*s.pred;
    return p;
}
```

## The prediction network

The prediction network reads one token at a time. Its input is the token's
embedding row; each LSTM layer updates its memory c and output h from that
input and its previous h, as PyTorch defines the LSTM; the last layer's h,
projected to the joint width, is what the joint network reads.

Decoding starts by feeding the blank. Its embedding row is zero (the
manifest tool checks this on the real file), so the first step sees a zero
input and a zero state. After that, the network reads each token the
decoder emits, and only those: a blank changes nothing.

<!-- source: psionic/lib/tdt.coax linear -->
```jai
// out[r] = bias[r] + sum over c of W[r][c] x[c]; W is (out.count,
// x.count) at w[at], the bias right after it.
tdt_linear :: (w:[]f32,at:int,x:[]f32,out:[]f32) {
    rows:=out.count;cols:=x.count;
    for r:0..<rows {
        t:PsionicAccumulator;t=psionic_ops_accumulate(t,f64(w[at+rows*cols+r]));
        for c:0..<cols {t=psionic_ops_accumulate(t,f64(w[at+r*cols+c])*f64(x[c]));}
        out[r]=f32(psionic_ops_total(t));
    }
}
```

<!-- source: psionic/lib/tdt.coax lstm -->
```jai
// One label through the prediction network: the label's embedding row
// enters layer 0; each layer's new h is the next layer's input. Gates are
// W_ih x + b_ih + W_hh h + b_hh, in the order i, f, g, o; then
// c = sigmoid(f) c + sigmoid(i) tanh(g) and h = sigmoid(o) tanh(c). The last
// layer's h is the prediction g the joint reads.
tdt_part :: (a:[]f32,at:int,n:int)->[]f32 {return a[at..<at+n];}
tdt_predict :: (s:TdtShape,p:TdtPlan,w:[]f32,label:int,work:[]f32) {
    n:=s.pred;x:=tdt_part(work,p.at_x,n);gates:=tdt_part(work,p.at_gates,4*n);
    for i:0..<n {x[i]=w[label*n+i];}
    for l:0..<s.layers {
        at:=p.lstm+l*(8*n*n+8*n);h:=tdt_part(work,p.at_h+l*n,n);c:=tdt_part(work,p.at_c+l*n,n);
        for r:0..<4*n {
            t:PsionicAccumulator;t=psionic_ops_accumulate(t,f64(w[at+8*n*n+r]));t=psionic_ops_accumulate(t,f64(w[at+8*n*n+4*n+r]));
            for k:0..<n {t=psionic_ops_accumulate(t,f64(w[at+r*n+k])*f64(x[k]));}
            for k:0..<n {t=psionic_ops_accumulate(t,f64(w[at+4*n*n+r*n+k])*f64(h[k]));}
            gates[r]=f32(psionic_ops_total(t));
        }
        for j:0..<n {
            c[j]=f32(tdt_sigmoid(f64(gates[n+j]))*f64(c[j])+tdt_sigmoid(f64(gates[j]))*tdt_tanh(f64(gates[2*n+j])));
            h[j]=f32(tdt_sigmoid(f64(gates[3*n+j]))*tdt_tanh(f64(c[j])));x[j]=h[j];
        }
    }
    tdt_linear(w,p.dec,x,work[p.at_dec..<p.at_dec+s.joint]);
}
```

## The joint network

The encoder's frames are projected to the joint width once, before decoding.
At each decision, the joint network adds the current frame's projection to
the prediction's, applies ReLU, and maps the result to 8,198 outputs. The
first 8,193 score the tokens and the blank; the last 5 score the durations.
The decoder takes the best of each group separately. It needs no softmax:
the largest score is also the largest probability.

<!-- source: psionic/lib/tdt.coax joint -->
```jai
// The joint network at encoded frame t: ReLU of the frame's projection plus
// the prediction's projection, then the head. The best token is the largest
// of the first vocab + 1 outputs, the best duration the largest of the rest;
// the lowest index wins a tie. Admitted weights and frames keep every
// output finite: |h| <= 1, so each projection and output is below 7e16.
TdtPick :: struct {token:int;duration:int;}
tdt_joint :: (s:TdtShape,p:TdtPlan,w:[]f32,t:int,work:[]f32)->TdtPick {
    z:=work[p.at_z..<p.at_z+s.joint];
    for j:0..<s.joint {z[j]=f32(tdt_relu(f64(work[t*s.joint+j])+f64(work[p.at_dec+j])));}
    logits:=work[p.at_logits..<p.at_logits+p.outputs];tdt_linear(w,p.head,z,logits);
    pick:TdtPick;pick.duration=s.vocab+1;
    for k:1..<s.vocab+1 {if logits[k]>logits[pick.token] {pick.token=k;}}
    for k:s.vocab+2..<p.outputs {if logits[k]>logits[pick.duration] {pick.duration=k;}}
    pick.duration-=s.vocab+1;
    return pick;
}
```

## Greedy decoding, by hand

Here are the rules, stated once:

- A token is emitted; a blank is not.
- The decoder moves ahead by the predicted duration.
- A blank with duration 0 moves ahead one frame anyway, so a blank always
  moves on.
- A token with duration 0 stays on the frame, unless the frame has now made
  `max_symbols` decisions; then the decoder moves ahead one frame.

The last two rules guarantee progress: no frame takes more than
`max_symbols` decisions, so a decode of T frames takes at most
T × `max_symbols` decisions and emits at most that many tokens.

<!-- source: psionic/lib/tdt.coax rule -->
```jai
// One decision of greedy TDT decoding at encoded frame t, after symbols
// decisions there. A blank emits nothing; a blank with duration 0 skips one
// frame anyway, so a blank always moves on. A skip above 0 moves to frame
// t + skip. A token with duration 0 stays on frame t, unless the frame has
// now made max_symbols decisions; then it moves on one frame.
TdtMove :: struct {emit:bool;t:int;symbols:int;}
tdt_rule :: (s:TdtShape,t:int,symbols:int,token:int,duration:int)->TdtMove {
    blank:=token==s.vocab;skip:=s.dur[duration];
    if blank && skip==0 {skip=1;}
    if skip==0 && symbols+1==s.max_symbols {skip=1;}
    if skip>0 {return TdtMove.{emit=!blank,t=t+skip};}
    return TdtMove.{emit=true,t=t,symbols=symbols+1};
}
```

To trace the rules by hand, the example builds a decoder whose decisions
depend on the frame alone. Its LSTM weights are all zero, so the prediction
is always zero. Frame j is the unit vector e_j, the encoder projection copies
it, so the joint's hidden values are e_j too, and column j of the head
writes 1 into one token's score and one duration's score; every other score
is 0.

<!-- source: psionic/lib/tdt.coax hand -->
```jai
// A decoder whose prediction network is all zeros, so each decision depends
// on the frame alone: frame j is the unit vector e_j, the encoder
// projection copies it, and column j of the head marks one token and one
// duration index. Frame 0: token 1, skip 1. Frame 1: token 2, skip 0.
// Frame 2: the blank, skip 0. Frame 3: token 0, skip 2.
tdt_hand :: ()->TdtShape {return tdt_durations(TdtShape.{vocab=4,enc=4,pred=4,layers=1,joint=4,max_symbols=2},3);}
tdt_hand_weights :: (w:[]f32,frames:[]f32) {
    s:=tdt_hand();p:=tdt_plan(s,TDT_FRAMES_MAX);for i:0..<w.count {w[i]=0;}
    token:=[4]int.[1,2,4,0];duration:=[4]int.[1,0,0,2];
    for j:0..<4 {
        w[p.enc+j*4+j]=1;w[p.head+token[j]*4+j]=1;w[p.head+(5+duration[j])*4+j]=1;
        for i:0..<4 {frames[j*4+i]=ifx i==j then 1.0 else 0.0;}
    }
}
```

The decoder has 4 tokens (the blank is token 4), durations 0, 1 and 2, and
at most 2 symbols a frame. Trace it:

| Frame | Decisions there | Token | Skip | Rule | Next frame |
| --- | --- | --- | --- | --- | --- |
| 0 | 0 | 1 | 1 | emit, move 1 | 1 |
| 1 | 0 | 2 | 0 | emit, stay | 1 |
| 1 | 1 | 2 | 0 | emit; 2 decisions, the cap: move 1 | 2 |
| 2 | 0 | blank | 0 | a blank moves at least 1 | 3 |
| 3 | 0 | 0 | 2 | emit, move 2 | 5, past the end |

The tokens are 1 2 2 0 at frames 0 1 1 3, as the example prints. An encoded
frame is 80 ms, so the frames are also timestamps: 0, 80, 80 and 240 ms.

The decoder itself is short: project the frames, feed the blank, then
decide, apply the rule and, after each emitted token, feed it to the
prediction network.

<!-- source: psionic/lib/tdt.coax decode -->
```jai
// Greedy TDT decoding of frames encoded frames (enc, frames x s.enc). Writes
// each emitted token and the frame it was emitted at; refuses before writing
// anything when an input is outside its bounds, and with capacity when the
// outputs fill up (written then counts what was kept).
tdt_decode :: (s:TdtShape,w:[]f32,enc:[]f32,frames:int,work:[]f32,tokens:[]int,at:[]int)->TdtResult {
    p:=tdt_plan(s,frames);if p.status!=TdtStatus.ok {return TdtResult.{status=p.status};}
    if w.count!=p.weights || enc.count!=frames*s.enc {return TdtResult.{status=TdtStatus.shape};}
    if work.count<p.work || at.count<tokens.count {return TdtResult.{status=TdtStatus.capacity};}
    if !tdt_values_ok(w,TDT_WEIGHT_MAX) || !tdt_values_ok(enc,TDT_FRAME_MAX) {return TdtResult.{status=TdtStatus.number};}
    for t:0..<frames {tdt_linear(w,p.enc,enc[t*s.enc..<t*s.enc+s.enc],work[t*s.joint..<t*s.joint+s.joint]);}
    for i:p.at_h..<p.at_x {work[i]=0;}
    tdt_predict(s,p,w,s.vocab,work);   // the blank starts the prediction network
    t:=0;symbols:=0;n:=0;
    while t<frames {
        pick:=tdt_joint(s,p,w,t,work);
        m:=tdt_rule(s,t,symbols,pick.token,pick.duration);
        if m.emit {
            if n==tokens.count {return TdtResult.{status=TdtStatus.capacity,written=n};}
            tokens[n]=pick.token;at[n]=t;n+=1;tdt_predict(s,p,w,pick.token,work);
        }
        t=m.t;symbols=m.symbols;
    }
    return TdtResult.{written=n};
}
```

## Text and error rates, by hand

Parakeet's vocabulary is SentencePiece pieces: `▁` (U+2581) marks the start
of a word. Give the hand decoder the pieces `▁now`, `▁o`, `h` and `▁no` for
tokens 0 to 3. Tokens 1 2 2 0 are `▁o` `h` `h` `▁now`: "ohh now" once each
`▁` becomes a space and the leading space is dropped.

<!-- source: psionic/lib/tdt.coax text -->
```jai
// Tokens to text: concatenate their pieces and turn each U+2581 (the
// SentencePiece word mark, bytes E2 96 81) into a space, dropping a leading
// one. The byte count, or -1 for a token outside pieces or too small an out.
tdt_text :: (pieces:[]string,tokens:[]int,out:[]u8)->int {
    n:=0;
    for k:tokens {
        if k<0 || k>=pieces.count {return -1;}
        piece:=pieces[k];i:=0;
        while i<piece.count {
            mark:=i+3<=piece.count && piece[i..<i+3]=="▁";
            if n>=out.count {return -1;}
            if mark {i+=3;if n>0 {out[n]=' ';n+=1;}}
            else {out[n]=piece[i];n+=1;i+=1;}
        }
    }
    return n;
}
```

Word error rate (WER) counts the fewest substitutions, insertions and
deletions that turn the reference's words into the hypothesis's, over the
reference's word count. Character error rate (CER) does the same over
characters. Against the reference "oh now":

- words: "oh" becomes "ohh" by one substitution, and "now" matches: 1 error
  of 2 words, a WER of 50%;
- characters: "oh now" is 6 characters, "ohh now" 7; inserting one "h" is
  enough: 1 error of 6, a CER of about 17%.

Characters are UTF-8 code points, so "été" is 3 characters, not 5 bytes.

<!-- source: psionic/lib/tdt.coax errors -->
```jai
// Words split on spaces; characters are UTF-8 code points, spaces included.
// Each unit is written into units as a view of text; the count, or -1 when
// units is too small.
tdt_words :: (text:string,units:[]string)->int {
    n:=0;i:=0;
    while i<text.count {
        if text[i]==' ' {i+=1;continue;}
        start:=i;while i<text.count && text[i]!=' ' {i+=1;}
        if n==units.count {return -1;}
        units[n]=text[start..<i];n+=1;
    }
    return n;
}
tdt_chars :: (text:string,units:[]string)->int {
    n:=0;i:=0;
    while i<text.count {
        start:=i;i+=1;while i<text.count && (text[i]&0xC0)==0x80 {i+=1;}
        if n==units.count {return -1;}
        units[n]=text[start..<i];n+=1;
    }
    return n;
}
// The fewest substitutions, insertions and deletions that turn ref into hyp
// (Levenshtein), with one row of hyp.count + 1 integers; -1 when row is
// too short. The error rate is that over ref.count.
// The smaller of a and b with no comparison: d >> 63 is all ones when d < 0.
tdt_min :: (a:int,b:int)->int {d:=a-b;return b+(d&(d>>63));}
tdt_distance :: (ref:[]string,hyp:[]string,row:[]int)->int {
    if row.count<hyp.count+1 {return -1;}
    for j:0..<hyp.count+1 {row[j]=j;}
    for i:1..<ref.count+1 {
        diag:=row[0];row[0]=i;
        for j:1..<hyp.count+1 {
            up:=row[j];row[j]=tdt_min(tdt_min(up+1,row[j-1]+1),diag+(ifx ref[i-1]==hyp[j-1] then 0 else 1));diag=up;
        }
    }
    return row[hyp.count];
}
```

## What the decoder admits

Audio must be one channel at 16 kHz, with every sample in [−1, 1] (chapter
151's `sf_admit`), and at most 327,520 samples: 2,047 hops of 160 samples,
which chapter 151's centred frames turn into 2,048 feature frames, which
chapter 152's subsampling turns into 256 encoded frames. That is 20.47
seconds. The test also checks that chapter 151's NeMo front end uses the
rate, FFT size, window, hop, mel count and pre-emphasis the checkpoint's
`processor_config.json` names: 16,000, 512, 400, 160, 128 and 0.97.

<!-- source: psionic/lib/tdt.coax audio -->
```jai
// What the decoder admits: one channel at 16 kHz in [-1, 1] (chapter 151's
// sf_admit) and at most TDT_SAMPLES_MAX samples. An encoded frame is 8 hops
// of 160 samples: 80 ms.
tdt_admit :: (a:SfAudio)->TdtStatus {
    st:=sf_admit(a,sf_nemo_spec());
    if st==SfStatus.rate {return TdtStatus.rate;}
    if st!=SfStatus.ok {return TdtStatus.shape;}
    if a.samples.count>TDT_SAMPLES_MAX {return TdtStatus.length;}
    return TdtStatus.ok;
}
```

The decoder refuses weights or frames that are not finite or exceed 1,000
and 100,000, before it writes anything:

<!-- source: psionic/lib/tdt.coax admit -->
```jai
// Every weight finite and within TDT_WEIGHT_MAX; every value of every frame
// finite and within TDT_FRAME_MAX.
tdt_values_ok :: (v:[]f32,limit:f64)->bool {
    for x:v {if !psionic_tensor_finite_bits(f32_bits(x)) || abs(f64(x))>limit {return false;}}
    return true;
}
```

## Refusals

<!-- source: psionic/examples/transcribe-tdt-refuse.coax refuse -->
```jai
main :: () {
    x:[327521]f64;a:=SfAudio.{rate=44100,channels=1,samples=x[0..<16000]};
    print("44.1 kHz audio: %\n",tdt_admit(a));
    a.rate=16000;a.samples=x[..];print("327,521 samples (over 20.47 s): %\n",tdt_admit(a));

    buf:[160]u8;c:=tdt_parakeet();
    d:=c;d.mels=80;r:=tdt_inventory(d,TDT_PINNED_MANIFEST);n:=tdt_line(tdt_expected(d,r.bad),buf[..]);
    print("an 80-mel model against the pinned file: %, first missing: %\n",r.status,cast(string)buf[0..<n]);
    d=c;d.kernel=31;r=tdt_inventory(d,TDT_PINNED_MANIFEST);n=tdt_line(tdt_expected(d,r.bad),buf[..]);
    print("31 depthwise taps: %, first missing: %\n",r.status,cast(string)buf[0..<n]);
    d=c;d.layers=17;r=tdt_inventory(d,TDT_PINNED_MANIFEST);
    print("17 layers: %, % lines for % expected\n",r.status,r.lines,tdt_expected_count(d));

    s:=tdt_hand();w:[260]f32;f:[16]f32;tdt_hand_weights(w[..],f[..]);work:[64]f32;tok:[8]int;at:[8]int;
    print("room for 3 tokens: %\n",tdt_decode(s,w[..],f[..],4,work[..],tok[0..<3],at[0..<3]));
    zero:f32=0;f[5]=zero/zero;print("a NaN frame value: %\n",tdt_decode(s,w[..],f[..],4,work[..],tok[..],at[..]));
    f[5]=0;w[0]=1001;print("a weight of 1,001: %\n",tdt_decode(s,w[..],f[..],4,work[..],tok[..],at[..]));
    w[0]=0;print("257 frames: %\n",tdt_decode(s,w[..],f[..],257,work[..],tok[..],at[..]));
    print("work one value short: %\n",tdt_decode(s,w[..],f[..],4,work[0..<59],tok[..],at[..]));
    b:=s;b.dur[1]=0;print("durations 0 0 2: %\n",tdt_decode(b,w[..],f[..],4,work[..],tok[..],at[..]));
}
```

```text
44.1 kHz audio: rate
327,521 samples (over 20.47 s): length
an 80-mel model against the pinned file: checkpoint, first missing: encoder.subsampling.linear.weight F32 1024,2560
31 depthwise taps: checkpoint, first missing: encoder.layers.0.conv.depthwise_conv.weight F32 1024,1,31
17 layers: checkpoint, 723 lines for 520 expected
room for 3 tokens: TdtResult.{status = capacity, written = 3}
a NaN frame value: TdtResult.{status = number, written = 0}
a weight of 1,001: TdtResult.{status = number, written = 0}
257 frames: TdtResult.{status = length, written = 0}
work one value short: TdtResult.{status = capacity, written = 0}
durations 0 0 2: TdtResult.{status = shape, written = 0}
```

An 80-mel model would read 256 × 10 = 2,560 values after the subsampling,
so its linear map is the first tensor that does not fit. A model of 17
layers finds all 520 tensors it expects, but the file has 203 more. When the
outputs fill, the decoder keeps the 3 tokens it has and says `capacity`.
Durations must rise strictly, so 0 0 2 is refused as `shape`. The program
exits with status 0 after printing every refusal.

## The checks

[`tdt-checks.coax`](../tests/tdt-checks.coax) holds references that share
nothing with the decoder but the weight order:

- a greedy decoder written as two nested loops, the way NeMo's reference
  greedy TDT decoder reads (an outer loop over frames, an inner loop while
  the skip is 0 and the frame has decisions left), with plain binary64 sums
  and its own sigmoid and tanh. On 24 seeded decoders with one or two LSTM
  layers and caps of 1 to 3 symbols, it must emit the same tokens at the
  same frames;
- a Levenshtein distance that fills the whole table, against the one-row
  version on 200 random word sequences;
- the inventory of the real checkpoint, written by a separate Python tool
  from the file's own header, against the architecture's lines.

They also check the rules case by case, the layout's offsets and the
real decoder's size by hand, every bound of the shape, audio admission, and
that every refusal leaves the outputs untouched.

The tests also recompute every seeded weight, compare the prediction
network's h, c and projection and the joint network's outputs with the
reference after each of four steps (within 10⁻⁶ and 10⁻⁵), check that the
723 expected lines are distinct, so the inventory is a one-to-one match, and
decode a second time on leftover work to show that nothing depends on it.

`coax mutate psionic/lib/tdt.coax --root-only` makes 822 mutants of the
unit's own code; the checks kill all 822. To get there, the helpers avoid
comparisons whose two branches agree at equality, where a mutant could not
be told apart: ReLU is (v + |v|) / 2, the minimum is computed from the sign
bit of a - b, and tanh and the logistic function share one formula.

## What remains

- **The encoder at Parakeet's size.** Chapter 152's encoder is bounded at
  width 128, 8 layers and 1,024 tokens; Parakeet needs width 1,024, 24
  layers, 4,096 hidden and 8 heads, without the layer biases chapter 152's
  layout has. Then the 627 million weights must be read from the
  safetensors file into that layout.
- **Recorded references.** Comparing log-mel features (dither and
  normalization details beyond the six constants above are not yet
  checked), encoder outputs and token sequences for fixed utterances with
  tensors recorded from NeMo's own decoder. In particular, whether NeMo's
  decoders apply the same rule at the symbol cap has not been compared.
- **The tokenizer.** Reading the 8,192 pieces from `tokenizer.json`; here the
  pieces are a table the caller supplies, and byte-fallback pieces are not
  handled.
- **Evaluation.** WER and CER on a fixed, versioned test set per language
  slice, with the set's identity in the result.

Offline transcription does not establish streaming latency;
[chapter 154](ch154-transcribe-while-someone-speaks.md) measures that.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the compensated accumulator.
- [Chapter 52](ch052-tokenizers-and-chat-templates.md): tokenizer families.
- [Chapter 53](ch053-choosing-the-next-token.md): the counter-based generator.
- [Chapter 151](ch151-waveforms-and-spectral-features.md): audio admission and
  the NeMo front end.
- [Chapter 152](ch152-a-speech-encoder.md): the FastConformer encoder and its
  80 ms frames.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry and the
  Granary dataset note.
- The model card and `config.json` of `nvidia/parakeet-tdt-0.6b-v3` at the
  pinned revision.
