# 153. Transcribe with Parakeet

> **Status: implemented — bounded resident Metal profile.** The original
> Parakeet v3 weights run through a Coax frontend, Metal encoder and CPU TDT
> decoder, with a resident native API and CLI. All 29 frozen recordings match
> actual NeMo tokens, frame positions and text; complete intermediate tensors
> pass the declared tolerance. The earlier CPU observations below retain
> their original teaching profile. Streaming and Coder's microphone UI remain
> separate work.

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

This chapter has three parts:

- **the checkpoint** ([`tdt.coax`](../lib/tdt.coax)): the tensor inventory of
  the pinned file, compared line for line with what the architecture says it
  must hold;
- **the decoder** (`tdt.coax`): the prediction network, the joint network,
  the greedy decoding rules, pieces to text and error rates;
- **end to end** ([`parakeet.coax`](../lib/parakeet.coax)): where each
  checkpoint tensor goes, the vocabulary from `tokenizer.json`, and audio to
  scored text through chapters 151, 152 and this one, run on the real
  weights.

The original teaching implementation below stores f32 and sums in binary64 with chapter 4's compensated
accumulator, rounding once to f32 per stored value. That is the profile
`psionic-tdt-v1`. The bounds admit Parakeet's real decoder sizes: 8,192
tokens and the blank, encoder width 1,024, prediction and joint width 640,
2 LSTM layers, up to 8 durations, at most 10 symbols a frame and 256 encoded
frames (20.47 seconds of audio).

## The resident engine: the current route

The runnable product is now separate from the original test harness. It maps
and authenticates the pinned safetensors file and tokenizer once, owns the
vocabulary for the model's lifetime, keeps the encoder weights on Metal, and
serializes calls through one resident engine. No NeMo, Torch, ONNX runtime,
cuBLAS, cuDNN or Python is linked into the recognizer. The independent reference
uses NeMo on the 4080; the shipped arithmetic is Coax.

The [engine guide](../docs/parakeet.md) gives installation and the C API. With
the pinned model directory already downloaded, run from the repository root:

```shell
psionic/platform/parakeet-build.sh
psionic/build/parakeet/psionic-parakeet transcribe /path/to/model clip.wav --gpu --json
apps/openagents-cli/build.sh
apps/openagents-cli/build/openagents transcribe clip.wav --backend parakeet --model-dir /path/to/model --json
```

The default cache is
`~/.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99/`.
The loader checks the original 2,508,311,120 checkpoint bytes against the SHA
below, checks the entire tensor inventory and payload coverage, and checks
`tokenizer.json` against
`bd321b096832a3f270bd3b2a88823957920f1a5c5ada71114a26ea729d0cbe91`.
It needs no Python-generated range file. The host maps files, copies bytes,
allocates memory and binds Metal; Coax makes the admission and model decisions.

The public [native header](../platform/psionic-parakeet.h) provides
`psionic_parakeet_open`, `psionic_parakeet_text` and
`psionic_parakeet_close`. Open once, pass 16 kHz mono f32 samples, copy the
text into the caller's draft, then close when the engine is no longer needed.
There is one open model per process. Calls serialize; a failed request leaves
the previous successful text, token/frame metadata and stage snapshots intact.
An inspection copies a snapshot into caller storage rather than returning a
view into reused inference workspace.

The current profile accepts **320 through 327,520 samples**, or 20 ms through
20.47 seconds. A GPU request fails explicitly if Metal is unavailable; it
never silently performs a slow CPU transcription. `--cpu` selects the slower
compensated teaching arithmetic explicitly. The CLI supports text and JSON.
It refuses forced language, vocabulary bias, prompts and timestamp-file flags
that this recognizer does not implement. Language is inferred by the model.

This is a whole-utterance recognizer. Coder microphone capture, the dictation
timeline, vocabulary rescoring, 60-second segmentation and speculative partials
remain the separate [voice plan](../../docs/voice/2026-11-02-coder-voice-from-hex.md)
and chapter 154. No speech is captured, message submitted or command executed
by this chapter's transcription API.

## What the official reference caught

Four correct transcripts were insufficient evidence of NeMo parity. The first
actual comparison found that the old generic spectral route normalized one
extra physical STFT column. NeMo's valid length is the number of complete
160-sample hops; its centred STFT physically stores one column more:

```jai
samples = 56080;
valid_frames = samples / 160;       // 350
physical_stft_frames = 351;
mean = sum(log_mel[0..<350]) / 350;
std = sqrt(sum((log_mel[0..<350] - mean)^2) / 349) + 0.00001;
```

[`front-plan.coax`](../lib/parakeet/front-plan.coax) admits both extents.
[`front.coax`](../lib/parakeet/front.coax) computes the physical spectra,
normalizes only valid columns, and compacts those columns into a mel-major
128 × valid-frames tensor. Excluded tail values never enter the statistics.
NeMo's official encoder produces exactly the same valid outputs when given
these compact features as when given its physical tensor and length mask;
[the recorded probe](../docs/verification/parakeet-2026-11-02/reference/boundary-probe/record.json)
checks both an ordinary case and the eight-frame subsampling boundary.
This keeps the generic spectral and earlier compensated teaching profiles
unchanged while correcting the resident recognizer.

The complete integration also caught an uninitialized mel-filter bank: a
fresh zero-filled bank made every normalized feature zero, while reused
workspace from another clip length could contain old log features. The
resident frontend now constructs its bank before every request. Its regression
check uses a nonconstant waveform and poisoned/reused workspace. The failed
full run is retained, rather than renamed as a pass.

The new [Metal schedule](../lib/parakeet/fast.coax) composes subsampling,
half feed-forward residuals, relative-position attention, the convolution
module and final normalization through all 24 layers. Every kernel is in
[`device.coax`](../lib/parakeet/device.coax); the host only binds and launches
it. This f32 device profile is checked within frozen tolerances, not declared
bitwise equal to the compensated CPU arithmetic. The prediction and joint
networks keep the CPU TDT arithmetic below. The resident route now distributes
independent output rows over a persistent worker pool, as explained next.

NeMo's SentencePiece decoder also has a specific unknown surface: token zero
renders as `" ⁇ "`, including its spaces. Other control-looking pieces are
preserved literally. [`text.coax`](../lib/parakeet/text.coax) matches those
recorded cases without changing generic `tdt_text`, stripping controls, or
turning a transcript into an instruction.

## Recorded reference and evaluation

[The qualification record](../docs/verification/parakeet-2026-11-02/README.md)
binds the actual source, binary, model, inputs, numeric tolerances and failed
attempts. Its independent implementation is NVIDIA NeMo commit
`5f15875acd0885ed92eadfee874ae87b0529ffa2`, using `GreedyTDTInfer` with ten
symbols per frame, no dither and no extra `pad_to` padding. All 723 original
safetensors bindings load strictly into the official modules. No pickle is
read or executed. Thirteen actual forced-logit reference cases exercise the
symbol cap, zero duration and blank skips, including NeMo's corrected cap
behavior. The Coax rule is compared with their emitted tokens and frame IDs;
the implementations do not claim identical internal joint-call counts.

The frozen evaluation contains the four original English clips and one
versioned FLEURS test utterance for each of the 25 supported languages. Every
source revision, selection rule, PCM hash and reference is in
[`manifest.json`](../fixtures/parakeet-reference/manifest.json), frozen before
inference. These are **smoke slices**, not estimates of corpus accuracy.
Multilingual WER/CER uses the declared Unicode NFKC/casefold normalization;
it is distinct from the legacy ASCII normalization below. The four original
English clips retain their previously frozen 5% WER and 3% CER gate.

All **29/29** complete pipelines pass. Every feature and final encoder value
meets the frozen elementwise bound `abs(error) <= 0.001 + 0.001 * abs(reference)`;
the largest absolute gaps are 0.00043449 and 0.00004368 respectively. Token IDs,
frame positions and raw text match exactly. A separate resident CPU decoder
run on the recorded NeMo encoder tensors also matches all 29; that is a
decoder check, not a new full CPU encoder run. The original English gate has
**0/46 word errors and 0/258 character errors**. The
[per-language table](../docs/verification/parakeet-2026-11-02/reference/product-quality-latency.md)
reports every FLEURS score, including the weaker observations, without turning
one utterance into a language-quality claim.

On an Apple M5 Max with 128 GiB and macOS 26.4, one ordered resident pass
recognized 298.435 seconds of audio in **48.375 seconds** of reported pipeline
inference: **6.17 audio seconds per inference second**. Fresh engine opening
took **20.272 seconds**, with filesystem cache uncontrolled. The four English
requests took 743–1541 ms; the first may include Metal warm-up. These are
offline per-request times, not key-release or streaming latency. They do not
meet the Coder proposal's 300 ms goal. Keep a model resident and improve the
measured bottlenecks before selecting this as the dictation default.

The [qualification index](../docs/verification/parakeet-2026-11-02/README.md)
records focused mutation checks, native O0/O2 and ASan/UBSan API checks, the
actual GPU corpus, and the public CLI run from a foreign working directory
with a file name containing spaces. Passing unchanged legacy gates were reused;
no full CPU-model or old 822-mutant run was repeated.

## Speed without changing a reduction

The first resident baseline above is retained as an actual measurement, not
replaced by a later faster number. The next pass targets the decoder's dense
products. Each output row is independent, but the order within a row matters:
the old compensated sum adds the first bias, the second bias for an LSTM gate,
every input product in order, then every recurrent product in order, before one
f32 store. Splitting that sum into partial reductions would change the profile.

[`parallel.coax`](../lib/parakeet/parallel.coax) instead assigns complete rows
to workers and keeps that sequence intact. The host supplies available CPU
count and pthread effects; [`parallel-plan.coax`](../lib/parakeet/parallel-plan.coax)
owns a maximum of eight participants and the disjoint balanced ranges. The
caller participates. A completion barrier finishes all gate rows before the
LSTM updates its hidden/cell state or the next layer's input. Calls remain
serialized through the resident API, so requests cannot oversubscribe that pool.
Closing joins every worker before freeing model storage. Partial thread startup
uses the observed smaller pool and reports its actual size through
`psionic_parakeet_decoder_threads`; it does not change the chosen model/device.

The independent native oracle compares token IDs, frame positions and every
used workspace bit with the old decoder at O0, O2 and fatal ASan/UBSan. It covers
1/2/8 participants, rows with large cancelling terms, repeated zero-duration recurrent
updates, concurrent dispatch, partial pthread startup and repeated lifecycle
transitions. The new authority's 92 emitted mutants are all killed. These are
new-code checks; the unchanged legacy decoder mutation run is still reused.

The changed engine is compared with the **same** frozen NeMo recordings again:
all 29 full GPU pipelines and all 29 decoder-only inputs pass, with unchanged
tensor tolerances and exact token/frame/text results. Unicode WER/CER is
unchanged on every slice. The [independent comparison and timing table](../docs/verification/parakeet-2026-11-02/optimization/reference/timing-comparison.md)
retains every request alongside the original baseline.

In the new ordered pass, 298.435 seconds of audio takes **27.199 seconds** of
reported inference, versus **48.375 seconds** before: an observed **1.78×**
improvement, or **10.97 audio seconds per inference second**. The four English
requests take 769, 559, 604 and 387 ms. Opening takes 15.862 seconds in this run;
filesystem caches and other machine load are uncontrolled, so that difference
is not attributed to the decoder change. Neither pass is a repeated steady-state
benchmark or evidence of 300 ms release latency.

The public recognition timer starts after the API lock and ends before the
last-success snapshot and caller-output copies. It covers the frontend, encoder,
decoder and checked text rendering. Harness process time also includes model
opening and artifact I/O. No separate stage timing was measured. Future speed
work should profile those stages, cache fixed FFT twiddles, reuse decoder
projections and reduce encoder launch overhead, with the same reference gate.
The model remains an offline full-attention checkpoint; this optimization does
not make it a streaming model.

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

```jai
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

The programs are [`transcribe-tdt.coax`](../examples/transcribe-tdt.coax),
[`transcribe-tdt-refuse.coax`](../examples/transcribe-tdt-refuse.coax) and
[`transcribe-parakeet.coax`](../examples/transcribe-parakeet.coax); the
checks' references are in [`tdt-checks.coax`](../tests/tdt-checks.coax). The
last command needs the downloaded checkpoint; it is the run on the real
weights, described in [the end-to-end section](#end-to-end-on-the-real-weights).

```shell
coax/bin/coax run psionic/examples/transcribe-tdt.coax
coax/bin/coax run psionic/examples/transcribe-tdt-refuse.coax
coax/bin/coax run psionic/examples/transcribe-parakeet.coax
coax/bin/coax test psionic/lib/tdt.coax
coax/bin/coax test psionic/lib/parakeet.coax
coax/bin/coax mutate psionic/lib/tdt.coax --root-only
coax/bin/coax mutate psionic/lib/parakeet.coax --root-only
python3 -I psionic/tests/parakeet_manifest.py ~/models/parakeet-tdt-0.6b-v3-541d1f99
COAX_CC=gcc psionic/tests/check_parakeet.sh ~/models/parakeet-tdt-0.6b-v3-541d1f99
```

```jai
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

The inventory shows one difference from chapter 152's layout: in the
checkpoint, the Conformer layers' linear maps and convolutions have no
biases (`attention_bias` and `convolution_bias` are false in
`config.json`). Chapter 152's layout gives each of them a bias; loading
leaves those at zero, which adds exactly nothing to each sum. The
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

```jai
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

## End to end on the real weights

The checkpoint, chapter 152's encoder and the decoder above are enough to
transcribe speech. [`parakeet.coax`](../lib/parakeet.coax) joins them. The
same configuration gives both shapes: the encoder's, with a CTC head of one
token that Parakeet does not have (it stays zero), and the decoder's.

<!-- source: psionic/lib/parakeet.coax shapes -->
```jai
// The encoder and decoder shapes a checkpoint configuration implies. The
// encoder's layers have no biases (they load as zeros) and there is no CTC
// head (one token and the blank, zeros).
pk_encoder :: (c:TdtConfig)->CfShape {
    return CfShape.{mels=c.mels,channels=c.channels,dim=c.enc,heads=c.heads,hidden=c.hidden,kernel=c.kernel,layers=c.layers,vocab=1};
}
pk_decoder :: (c:TdtConfig)->TdtShape {
    return tdt_durations(TdtShape.{vocab=c.vocab,enc=c.enc,pred=c.pred,layers=c.pred_layers,joint=c.joint,max_symbols=TDT_SYMBOLS_MAX},c.durations);
}
```

### Where the weights go, by hand

The encoder array is chapter 152's layout at Parakeet's size. The
subsampling holds 2,560 + 2,560 + 65,792 + 2,560 + 65,792 values for its
five convolutions (kernel and bias each) and 1,024 × 4,096 + 1,024 =
4,195,328 for the linear map: 4,334,592. One layer holds two feed-forward
modules of 8,395,776, attention of 5,251,072, the convolution module of
3,165,184 and the final norm of 2,048: 25,209,856. Then 24 layers and the
head of 2 × 1,025:

4,334,592 + 24 × 25,209,856 + 2,050 = 609,373,186.

The checkpoint's encoder has 608,928,768 values. The difference is the
layers' 24 × 18,432 = 442,368 biases and the head's 2,050, which stay zero.

Tensor 51 is the 25th of layer 0 in the inventory's order, `k_proj`. Its
place is the subsampling (4,334,592), then the first feed-forward module
(8,395,776), the attention norm (2,048) and the query map with its bias
(1,048,576 + 1,024): 13,782,016, which the example prints. `pk_slot` gives
every tensor its place; batch norm's step counters are not loaded.

<!-- source: psionic/lib/parakeet.coax slots -->
```jai
// Tensor i of the checkpoint (tdt_expected's order) and its place: the
// decoder's in chapter 153's layout, the encoder's in chapter 152's, where a
// layer's tensors follow TDT_LAYER_NAMES.
pk_slot :: (c:TdtConfig,i:int)->PkSlot {
    if i<0 || i>=tdt_expected_count(c) {return PkSlot.{part=-1};}
    t:=tdt_expected(c,i);n:=1;for k:0..<t.rank {n*=t.dims[k];}
    p:=tdt_plan(pk_decoder(c),TDT_FRAMES_MAX);g:=4*c.pred*c.pred;
    if i<3 {at:=[3]int.[p.dec+c.joint*c.pred,p.dec,0];return PkSlot.{part=1,at=at[i],count=n};}
    j:=i-3;
    if j<4*c.pred_layers {
        // bias_hh, bias_ih, weight_hh, weight_ih of layer j / 4.
        at:=[4]int.[2*g+4*c.pred,2*g,g,0];
        return PkSlot.{part=1,at=p.lstm+j/4*(2*g+8*c.pred)+at[j%4],count=n};
    }
    j-=4*c.pred_layers;
    if j<4 {at:=[4]int.[p.enc+c.joint*c.enc,p.enc,p.head+p.outputs*c.joint,p.head];return PkSlot.{part=1,at=at[j],count=n};}
    j-=4;e:=pk_encoder(c);sub:=cf_sub_at(e);
    if j<10 {
        // Each convolution's kernel, then its bias; the inventory lists the bias first.
        stage:=[5]int.[sub.conv0,sub.dw1,sub.pw1,sub.dw2,sub.pw2];kernel:=[5]int.[9,9,c.channels,9,c.channels];
        at:=stage[j/2];if j%2==0 {at+=c.channels*kernel[j/2];}
        return PkSlot.{part=0,at=at,count=n};
    }
    if j<12 {at:=sub.out;if j==10 {at+=c.enc*c.channels*cf_length(c.mels);}return PkSlot.{part=0,at=at,count=n};}
    j-=12;l:=cf_layer_at(e,cf_layer_base(e,j/TDT_LAYER_TENSORS));m:=c.enc;
    // Batch norm's step counter, tensor 2, is not loaded.
    if j%TDT_LAYER_TENSORS==2 {return PkSlot.{part=-1};}
    at:=[29]int.[l.conv.dw,l.conv.bn+3*m,l.conv.bn,l.conv.bn,l.conv.bn+m,l.conv.bn+2*m,l.conv.pw1,l.conv.pw2,
        l.ff1.w1,l.ff1.w2,l.ff2.w1,l.ff2.w2,l.conv.norm+m,l.conv.norm,l.ff1.norm+m,l.ff1.norm,l.ff2.norm+m,l.ff2.norm,
        l.out+m,l.out,l.att.norm+m,l.att.norm,l.att.u,l.att.bias_v,l.att.k,l.att.o,l.att.q,l.att.pos,l.att.v];
    return PkSlot.{part=0,at=at[j%TDT_LAYER_TENSORS],count=n};
}
// Tensor i's name, as the safetensors header writes it; the byte count, or
// -1 for an index past the inventory or too small an out.
pk_name :: (c:TdtConfig,i:int,out:[]u8)->int {
    if i<0 || i>=tdt_expected_count(c) {return -1;}
    t:=tdt_expected(c,i);full:[160]u8;k:=tdt_put(full[..],0,t.prefix);
    if t.layer>=0 {k=tdt_put_int(full[..],k,t.layer);}
    k=tdt_put(full[..],k,t.suffix);
    if k>out.count {return -1;}
    for j:0..<k {out[j]=full[j];}
    return k;
}
```

A check places all 723 tensors and requires the encoder's to add up to
608,928,768 values, the decoder's to fill its 18,128,518 exactly, every
place to lie inside its array, and layers 0, 1 and 23 to match the layout
tensor by tensor.

### The vocabulary

`tokenizer.json` is 1.16 MB of JSON. Its `model.vocab` object maps each of
the 8,192 pieces to its token id. `pk_vocab` parses the file with the
repository's JSON reader ([`coax/lib/json`](../../coax/lib/json/json.coax)),
which writes a tree of nodes into a caller's array (the pinned file needs
50,878), and requires ids 0 to 8,191, each once. Each piece is unescaped
into one byte array; the pinned file's pieces take 45,886 bytes. None of
them is a byte-fallback piece such as `<0x0A>`, so the text is the pieces
joined, as `tdt_text` does.

<!-- source: psionic/lib/parakeet.coax vocab -->
```jai
// tokenizer.json's model.vocab, read with coax/lib/json: an object of
// "piece": id members with ids 0 to count - 1, each once. Each piece is
// unescaped into bytes and pieces[id] views it. nodes holds the parsed
// tree (the pinned file needs 50,878). Nothing is written to pieces on
// refusal.
pk_vocab :: (j:string,count:int,nodes:[]JvNode,bytes:[]u8,pieces:[]string)->PkVocab {
    if count<1 || count>TDT_VOCAB_MAX {return PkVocab.{status=PkStatus.shape};}
    if pieces.count<count {return PkVocab.{status=PkStatus.capacity};}
    d:[1]Jv;if !jv_parse(d[..],j,nodes) {return PkVocab.{status=PkStatus.vocabulary};}
    v:=jv_path(d[0],0,"model.vocab");
    if jv_kind(d[0],v)!=JV_OBJECT || jv_len(d[0],v)!=count {return PkVocab.{status=PkStatus.vocabulary};}
    from:[TDT_VOCAB_MAX]int;to:[TDT_VOCAB_MAX]int;seen:[TDT_VOCAB_MAX]bool;used:=0;m:=jv_at(d[0],v,0);
    for i:0..<count {
        id:=jv_int(d[0],m,-1);if id<0 || id>=count || seen[id] {return PkVocab.{status=PkStatus.vocabulary};}
        // The parser checked every escape, so only room can run out.
        n:=jv_unescape(jv_key(d[0],m),bytes[used..<bytes.count]);if n<0 {return PkVocab.{status=PkStatus.capacity};}
        seen[id]=true;from[id]=used;used+=n;to[id]=used;m=d[0].nodes[m].next;
    }
    for i:0..<count {pieces[i]=cast(string)bytes[from[i]..<to[i]];}
    return PkVocab.{count=count,bytes=used};
}
```

### Scoring

LibriSpeech's references are upper case without punctuation; Parakeet
writes cased, punctuated text. Before counting errors, both are normalized
the same way: ASCII letters lowercased, letters, digits and the apostrophe
kept, every other ASCII byte a word break, and bytes from 0x80 kept, so
other scripts pass through unchanged.

"Concord returned to its place, amidst the tents." becomes "concord
returned to its place amidst the tents": 8 words and 46 characters with the
spaces. Against "Concord returned to its place amidst the tent.", one word
changes ("tents" to "tent") and one character is deleted: 1 of 8 and 1 of 46.

<!-- source: psionic/lib/parakeet.coax normalize -->
```jai
// For scoring: ASCII letters lowercased; letters, digits, the apostrophe and
// every byte from 0x80 kept; anything else separates words; one space
// between words. The byte count, or -1 when out is too small.
pk_normalize :: (text:string,out:[]u8)->int {
    n:=0;gap:=false;
    for b:text {
        // Setting bit 0x20 lowercases an ASCII letter.
        l:=b|0x20;letter:=l>='a' && l<='z';x:=ifx letter then l else b;
        keep:=letter || (x>='0' && x<='9') || x=='\'' || x>=0x80;
        if !keep {gap=n>0;continue;}
        if gap {if n==out.count {return -1;}out[n]=' ';n+=1;gap=false;}
        if n==out.count {return -1;}
        out[n]=x;n+=1;
    }
    return n;
}
// Word and character errors of hypothesis against reference, both
// normalized; each at most PK_TEXT_MAX bytes.
pk_score :: (reference:string,hypothesis:string)->PkScore {
    a:[PK_TEXT_MAX]u8;b:[PK_TEXT_MAX]u8;
    ref:=cast(string)a[0..<pk_normalize(reference,a[..])];hyp:=cast(string)b[0..<pk_normalize(hypothesis,b[..])];
    ru:[PK_TEXT_MAX]string;hu:[PK_TEXT_MAX]string;row:[PK_TEXT_MAX+1]int;r:PkScore;
    r.words=tdt_words(ref,ru[..]);h:=tdt_words(hyp,hu[..]);r.word_errors=tdt_distance(ru[0..<r.words],hu[0..<h],row[..]);
    r.chars=tdt_chars(ref,ru[..]);h=tdt_chars(hyp,hu[..]);r.char_errors=tdt_distance(ru[0..<r.chars],hu[0..<h],row[..]);
    return r;
}
```

### Audio to text

`pk_transcribe` computes NeMo's features in binary64 (chapter 151's
`sf_nemo_spec`), rounds them to f32 for chapter 152's `cf_encode`, decodes
the encoded frames with `tdt_decode`, turns the tokens into text and scores
it. Work is two caller arrays. For the longest utterance admitted, 327,520
samples: 2,048 feature frames and 256 encoded. The binary64 array holds the
features (128 × 2,048 = 262,144) and the mel bank (128 × 257 = 32,896):
295,040. The f32 array holds the features again (262,144), the encoder's
work (35,651,584, of which two copies of the first convolution's output,
256 × 1,024 × 64 each, are almost all), the encoded frames (262,144) and the
decoder's work (179,078): 36,354,950, about 145 MB.

<!-- source: psionic/lib/parakeet.coax transcribe -->
```jai
// NeMo's features at the configuration's mel count.
pk_spec :: (c:TdtConfig)->SfSpec {s:=sf_nemo_spec();s.mels=c.mels;return s;}
pk_work :: (c:TdtConfig,samples:int)->PkWork {
    if samples<1 || samples>TDT_SAMPLES_MAX {return PkWork.{status=PkStatus.audio};}
    s:=pk_spec(c);frames:=sf_frames(samples,s);encoded:=cf_length(frames);
    w:=PkWork.{frames=frames,encoded=encoded,f64=c.mels*frames+c.mels*(s.n_fft/2+1)};
    w.f32=c.mels*frames+cf_plan(pk_encoder(c),1,frames).work+encoded*c.enc+tdt_plan(pk_decoder(c),encoded).work;
    return w;
}
// Audio to text: features in binary64 (w64 holds them and the mel bank),
// rounded to f32 for the encoder; the encoder and the decoder work in w32;
// the decoder's tokens become text through pieces (one per token), which is
// then scored against reference. The text is written last, so a refusal
// leaves it untouched.
pk_transcribe :: (c:TdtConfig,ew:[]f32,dw:[]f32,pieces:[]string,a:SfAudio,reference:string,w64:[]f64,w32:[]f32,text:[]u8)->PkRun {
    e:=pk_encoder(c);d:=pk_decoder(c);
    if cf_shape_check(e)!=CfStatus.ok || tdt_shape_check(d)!=TdtStatus.ok {return PkRun.{status=PkStatus.shape};}
    if ew.count!=cf_weight_count(e) || dw.count!=tdt_plan(d,TDT_FRAMES_MAX).weights || pieces.count!=c.vocab {return PkRun.{status=PkStatus.shape};}
    if tdt_admit(a)!=TdtStatus.ok {return PkRun.{status=PkStatus.audio};}
    if reference.count>PK_TEXT_MAX {return PkRun.{status=PkStatus.text};}
    k:=pk_work(c,a.samples.count);
    if w64.count<k.f64 || w32.count<k.f32 {return PkRun.{status=PkStatus.capacity};}
    s:=pk_spec(c);nf:=c.mels*k.frames;bank:=w64[nf..<k.f64];
    if sf_mel_bank(s,bank).status!=SfStatus.ok || sf_features(a,s,bank,w64[0..<nf]).status!=SfStatus.ok {return PkRun.{status=PkStatus.features};}
    for i:0..<nf {w32[i]=f32(w64[i]);}
    work:=cf_plan(e,1,k.frames).work;at:=nf+work;size:=k.encoded*c.enc;
    lengths:=[1]int.[k.frames];got:[1]int;
    if cf_encode(e,ew,w32[0..<nf],lengths[..],1,k.frames,w32[nf..<at],w32[at..<at+size],got[..],false).status!=CfStatus.ok {
        return PkRun.{status=PkStatus.encoder};
    }
    tokens:[PK_TOKENS_MAX]int;frames:[PK_TOKENS_MAX]int;
    t:=tdt_decode(d,dw,w32[at..<at+size],k.encoded,w32[at+size..<k.f32],tokens[..],frames[..]);
    if t.status!=TdtStatus.ok {return PkRun.{status=PkStatus.decoder};}
    out:[PK_TEXT_MAX]u8;n:=tdt_text(pieces,tokens[0..<t.written],out[..]);
    if n<0 || n>text.count {return PkRun.{status=PkStatus.text};}
    for i:0..<n {text[i]=out[i];}
    r:=pk_score(reference,cast(string)text[0..<n]);
    return PkRun.{samples=a.samples.count,frames=k.frames,encoded=k.encoded,tokens=t.written,text=n,
        words=r.words,word_errors=r.word_errors,chars=r.chars,char_errors=r.char_errors};
}
```

At toy size, with chapter 152's seeded encoder (width 8) and a seeded
decoder of 4 tokens, the test signal of 0.1 seconds gives 11 feature frames,
2 encoded frames and the text "no no". A check runs the same steps one call
at a time and requires the same features, encoded frames, decoder work and
text, bit for bit.

The example prints these numbers and the refusals:

<!-- source: psionic/examples/transcribe-parakeet.coax run -->
```jai
main :: () {
    c:=tdt_parakeet();e:=pk_encoder(c);d:=pk_decoder(c);
    loaded:=0;skipped:=0;
    for i:0..<tdt_expected_count(c) {s:=pk_slot(c,i);if s.part<0 {skipped+=1;} else {loaded+=s.count;}}
    print("encoder array % values, decoder array %; % checkpoint values placed, % step counters skipped\n",
        cf_weight_count(e),tdt_plan(d,TDT_FRAMES_MAX).weights,loaded,skipped);
    buf:[160]u8;s:=pk_slot(c,51);
    print("tensor 51, %: encoder array at %, % values\n",cast(string)buf[0..<pk_name(c,51,buf[..])],s.at,s.count);
    w:=pk_work(c,TDT_SAMPLES_MAX);
    print("20.47 s of audio: % feature frames, % encoded; work % binary64 and % f32 values\n",w.frames,w.encoded,w.f64,w.f32);

    nodes:[64]JvNode;bytes:[64]u8;pieces:[4]string;
    j:="{\"model\":{\"vocab\":{\"▁now\":0,\"\\u2581o\":1,\"h\":2,\"▁no\":3}}}";
    v:=pk_vocab(j,4,nodes[..],bytes[..],pieces[..]);
    print("vocabulary: %, % pieces in % bytes: % % % %\n",v.status,v.count,v.bytes,pieces[0],pieces[1],pieces[2],pieces[3]);
    out:[64]u8;n:=pk_normalize("Concord returned to its place, amidst the tents.",out[..]);
    print("normalized: \"%\"\n",cast(string)out[0..<n]);
    r:=pk_score("CONCORD RETURNED TO ITS PLACE AMIDST THE TENTS","Concord returned to its place amidst the tent.");
    print("against \"... the tent.\": word errors % of %, character errors % of %\n",r.word_errors,r.words,r.char_errors,r.chars);

    t:=pk_tiny();te:=pk_encoder(t);td:=pk_decoder(t);ew:[1476]f32;dw:[276]f32;
    q:=cf_init(te,ew[..],5);u:=tdt_init(td,dw[..],31);
    x:[1600]f64;sf_test_signal(x[..]);a:=SfAudio.{rate=16000,channels=1,samples=x[..]};
    w32:[2000]f32;w64:[5000]f64;text:[64]u8;
    k:=pk_transcribe(t,ew[..],dw[..],pieces[..],a,"no oh",w64[..],w32[..],text[..]);
    print("seeded toy model, 0.1 s: %, % frames, % encoded, % tokens: \"%\"\n",k.status,k.frames,k.encoded,k.tokens,cast(string)text[0..<k.text]);
    print("  against \"no oh\": word errors % of %, character errors % of %\n",k.word_errors,k.words,k.char_errors,k.chars);

    b:=a;b.rate=8000;print("8 kHz audio: %\n",pk_transcribe(t,ew[..],dw[..],pieces[..],b,"no",w64[..],w32[..],text[..]).status);
    print("3 pieces for 4 tokens: %\n",pk_transcribe(t,ew[..],dw[..],pieces[0..<3],a,"no",w64[..],w32[..],text[..]).status);
    print("work one f32 short: %\n",pk_transcribe(t,ew[..],dw[..],pieces[..],a,"no",w64[..],w32[0..<pk_work(t,1600).f32-1],text[..]).status);
    print("room for 4 text bytes: %\n",pk_transcribe(t,ew[..],dw[..],pieces[..],a,"no",w64[..],w32[..],text[0..<4]).status);
    print("an id used twice: %\n",pk_vocab("{\"model\":{\"vocab\":{\"a\":0,\"b\":0}}}",2,nodes[..],bytes[..],pieces[..]).status);
    print("no model.vocab: %\n",pk_vocab("{\"vocab\":{\"a\":0,\"b\":1}}",2,nodes[..],bytes[..],pieces[..]).status);
}
```

```jai
encoder array 609373186 values, decoder array 18128518; 627057286 checkpoint values placed, 24 step counters skipped
tensor 51, encoder.layers.0.self_attn.k_proj.weight: encoder array at 13782016, 1048576 values
20.47 s of audio: 2048 feature frames, 256 encoded; work 295040 binary64 and 36354950 f32 values
vocabulary: ok, 4 pieces in 16 bytes: ▁now ▁o h ▁no
normalized: "concord returned to its place amidst the tents"
against "... the tent.": word errors 1 of 8, character errors 1 of 46
seeded toy model, 0.1 s: ok, 11 frames, 2 encoded, 2 tokens: "no no"
  against "no oh": word errors 1 of 2, character errors 2 of 5
8 kHz audio: audio
3 pieces for 4 tokens: shape
work one f32 short: capacity
room for 4 text bytes: text
an id used twice: vocabulary
no model.vocab: vocabulary
```

### The clips

The evaluation set, `parakeet-clips-v1`, is four utterances of LibriSpeech
test-clean (speaker 6930, chapter 75918), licence CC BY 4.0, listed with
their source, hashes and references in
[`clips.txt`](../fixtures/parakeet/clips.txt). They were fetched as FLAC and
converted losslessly to 16-bit PCM at 16 kHz; the PCM files are in the
repository, 17.6 seconds in all.

The thresholds were frozen before the first run: over the four clips
together, at most 5 word errors per 100 reference words and 3 character
errors per 100 reference characters. The model card reports a LibriSpeech
test-clean word error rate of about 2%, so a correct implementation should
pass easily and a broken one should not.

### The historical CPU run

[`check_parakeet.sh`](../tests/check_parakeet.sh) hashes the checkpoint and
checks its inventory, compiles
[`parakeet-run-runtime.coax`](../tests/parakeet-run-runtime.coax) and the C
harness [`parakeet_run.c`](../tests/parakeet_run.c), which maps the file and
copies each tensor's bytes to the place `pk_slot` names, then transcribes
each clip in its own process. Every decision above the bytes is Coax.

```jai
loaded 627057286 weights in 0.8 s: encoder array 609373186, decoder array 18128518
slices of 6930-75918-0011: 320 feature frames, 40 encoded frames, layers 0 and 1, 2.5 s
6930-75918-0000, 3.50 s of audio, 351 feature frames, 44 encoded, 15 tokens, 26 s:
  "Concord returned to its place amidst the tents,"
  word errors 0 of 8, character errors 0 of 46
6930-75918-0002, 5.03 s of audio, 503 feature frames, 63 encoded, 26 tokens, 37 s:
  "Congratulations were poured in upon the Princess everywhere during her journey."
  word errors 0 of 11, character errors 0 of 78
6930-75918-0006, 5.85 s of audio, 586 feature frames, 74 encoded, 32 tokens, 44 s:
  "This has indeed been a harassing day, continued the young man, his eyes fixed upon his friend,"
  word errors 0 of 17, character errors 0 of 91
6930-75918-0011, 3.19 s of audio, 320 feature frames, 40 encoded, 15 tokens, 24 s:
  "I am convinced of what I say, said the Count."
  word errors 0 of 10, character errors 0 of 43
all 4 clips: word errors 0 of 46, character errors 0 of 258: within the frozen thresholds; 44 s
```

All 46 words come out right. The punctuation and capitals are the model's;
normalization removes them before scoring. Each clip takes about 7.5
seconds per second of audio on one core, almost all of it the encoder's
compensated sums; the clips run side by side. The slices line is the
oracle's input, written by a fifth process.

### The independent check

[`parakeet_oracle.py`](../tests/parakeet_oracle.py) is written from NeMo's
published preprocessor and FastConformer encoder in plain Python with
binary64 numbers, and shares no code with the Coax: it computes the power
spectrum with a direct DFT, builds the Slaney mel filters as librosa defines
them, convolves with explicit loops, and computes relative-position
attention from the position i − j of each pair, with no shift. For the
shortest clip it recomputes the features from the PCM samples, the
subsampling from the Coax features, and layers 0 and 1 from the Coax output
of the stage before, so a difference would point at one stage. The
tolerance was frozen before the first comparison: 10⁻⁴ × (1 + the largest
value of the slice).

```jai
log-mel features, 320 frames: largest difference 2.01e-07, limit 0.00059, within
subsampling, 40 frames: largest difference 0.000247, limit 0.534, within
encoder layer 0: largest difference 3.5e-05, limit 0.0195, within
encoder layer 1: largest difference 1.47e-05, limit 0.0206, within
```

The subsampling's outputs reach about 5,300, so its limit is 0.53; the
largest difference is 2.5 × 10⁻⁴. The layers' outputs are layer-normed, and
differ from the oracle by at most 3.5 × 10⁻⁵.

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

The end-to-end unit, [`parakeet.coax`](../lib/parakeet.coax), checks every
tensor's place, the vocabulary reader on hand-written JSON (escapes, a
repeated id, an id past the count, a fractional id, too few nodes or bytes),
normalization and scoring by hand, the work sizes by hand, and the seeded
transcription against its steps run one by one, with each refusal leaving
the text untouched. `coax mutate psionic/lib/parakeet.coax --root-only`
makes 318 mutants; the checks kill all 318.

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

## The historical stopping point

The list below describes the initial CPU teaching run, retained to explain
why its four error-free transcripts did not finish this chapter. The resident
route and current qualification above supersede its reference/evaluation gaps.

- **Recorded references.** Comparing features, encoder outputs and token
  sequences with tensors recorded from NeMo's own model on fixed utterances.
  The oracle here is written from the published architecture, not run from
  NeMo's code; in particular, whether NeMo's decoder applies the same rule
  at the symbol cap has not been compared, and the four clips never reach
  the cap.
- **Evaluation per language.** Parakeet v3 covers 25 European languages.
  Here one English set of four clips is scored; WER and CER on a fixed,
  versioned test set per language slice are not measured.
- **Speed.** The CPU path is exact and slow, about 7.5 seconds per second of
  audio on one core.

Offline transcription does not establish streaming latency;
[chapter 154](ch154-transcribe-while-someone-speaks.md) develops the separate
streaming foundations and still requires a real model and wall-clock evidence.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the compensated accumulator.
- [Chapter 52](ch052-tokenizers-and-chat-templates.md): tokenizer families.
- [Chapter 53](ch053-choosing-the-next-token.md): the counter-based generator.
- [Chapter 151](ch151-waveforms-and-spectral-features.md): audio admission and
  the NeMo front end.
- [Chapter 152](ch152-a-speech-encoder.md): the FastConformer encoder and its
  80 ms frames, here at Parakeet's size on its weights.
- [`coax/lib/json`](../../coax/lib/json/json.coax): the JSON reader.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry and the
  Granary dataset note.
- The model card and `config.json` of `nvidia/parakeet-tdt-0.6b-v3` at the
  pinned revision.
- LibriSpeech (Panayotov, Chen, Povey and Khudanpur, 2015), test-clean,
  CC BY 4.0.
- [Pinned NeMo frontend](https://github.com/NVIDIA-NeMo/NeMo/blob/5f15875acd0885ed92eadfee874ae87b0529ffa2/nemo/collections/asr/parts/preprocessing/features.py)
  and [greedy TDT decoder](https://github.com/NVIDIA-NeMo/NeMo/blob/5f15875acd0885ed92eadfee874ae87b0529ffa2/nemo/collections/asr/parts/submodules/rnnt_greedy_decoding.py).
- [FLEURS](https://huggingface.co/datasets/google/fleurs/tree/70bb2e84b976b7e960aa89f1c648e09c59f894dd),
  CC BY 4.0; the local manifest records each selected source and conversion.
