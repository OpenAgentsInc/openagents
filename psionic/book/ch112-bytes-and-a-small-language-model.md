# 112. Bytes and a small language model

A small language model lets us run the complete experiment: define the data,
train all parameters, measure held-out likelihood and generate a continuation.
Here the vocabulary is every byte 0–255. No text normalization or unknown ID is
needed. A context byte indexes one row of a 256 by 256 logit table. There are
65,536 trainable binary64 parameters and one byte of context.

The runnable profile is `cpu-f64-byte-bigram-v1`. This is a deliberately tiny
CPU teaching model. It does not require a pretrained checkpoint or any GPU.
The [dataset](../fixtures/byte-lm/dataset.json) contains original synthetic
prose authored for this chapter under CC0-1.0, with five training, two
validation and two test documents. IDs and whole-document partitions are
fixed before training. Documents are joined with one LF and a final LF inside
each split, so those separators are explicit training transitions; no
transition crosses a partition. These small synthetic held-out results measure
this experiment, not general language quality or real-world generalization.

## Likelihood and a complete update

For context x, the probability of y is `exp(W[x,y]) / sum(exp(W[x,:]))`.
Subtract the row maximum before exponentiation. The loss is mean negative
log probability over adjacent byte pairs; the first byte supplies context and
has no prediction target. Bits per byte divide nats by ln(2). At zero weights,
every next byte has probability 1/256 and the baseline is 8 bits per byte.

Let C[x,y] be the training transition counts, C[x] their row sum, and N the
number of transitions. The exact full-batch gradient is
`(C[x] * p(y|x) - C[x,y]) / N`. Normalization uses all transitions, including
rows with few examples. One update changes every parameter in every observed
row; unused rows retain zero logits. This is gradient training, not a count
lookup substituted for an optimizer.

<!-- source: psionic/lib/byte-lm-math.coax update -->
```jai
bg_update :: (counts:[]int,weights:[]f64,total:int,rate:f64) {
 for row:0..<256 {at:=row*256;n:=bg_row_count(counts,at);if n==0 {continue;}
  norm:=bg_norm(weights,at);
  for j:0..<256 {p:=exp(weights[at+j]-norm.maximum)/norm.sum;gradient:=(f64(n)*p-f64(counts[at+j]))/f64(total);weights[at+j]-=rate*gradient;}
 }
}
```

The trainer starts at zero and performs the requested number of SGD updates
with a fixed rate. It records the initial loss and every updated training loss.
The admitted maximum is 64 updates, rate in (0,64], and 2–4,096 source bytes.
The fixed experiment uses 64 updates at rate 32, chosen as part of the profile;
validation and test data never enter an update or a fitted preprocessing step.

<!-- source: psionic/lib/byte-lm.coax train -->
```jai
bg_fit :: (train:[]u8,steps:int,rate:f64,weights:[]f64,stage:[]f64,trace:[]f64,overlap:bool)->BgFit #export_c {
 if overlap {return BgFit.{status=BgStatus.alias};}if !bg_data(train.count) || !bg_weights(weights.count) || !bg_weights(stage.count) {return BgFit.{status=BgStatus.shape};}if !bg_steps(steps) {return BgFit.{status=BgStatus.steps};}if !bg_rate(rate) {return BgFit.{status=BgStatus.number};}if trace.count<steps+1 {return BgFit.{status=BgStatus.capacity};}
 counts:[65536]int;bg_counts(train,counts[..]);for i:0..<65536 {stage[i]=0;}losses:[65]f64;total:=train.count-1;losses[0]=bg_count_loss(counts[..],stage,total);
 for step:0..<steps {bg_update(counts[..],stage,total,rate);losses[step+1]=bg_count_loss(counts[..],stage,total);}
 for i:0..<65536 {weights[i]=stage[i];}for i:0..<steps+1 {trace[i]=losses[i];}return BgFit.{initial=losses[0],final=losses[steps],updates=steps,transitions=total};
}
```

Weights and workspace each hold exactly 65,536 f64 values. The public trace
needs updates+1 entries; its unused tail is preserved. Admission finishes
before training writes, all updates occur in scratch, and only the completed
model and trace publish. The [host adapter](../platform/byte-lm-memory.c)
observes overlap across input and all writable spans. Arithmetic and policy
stay in Coax. Callers retain valid aligned immutable input storage and
exclusive output access for the synchronous call.

Evaluation accepts a complete finite logit table in [-4096,4096] and an
independent byte sequence. It reports nats, bits and transition count. A model
from the bounded zero-initialized trainer fits that parameter domain. Invalid
shape, nonfinite data, bad rate, insufficient trace or actual aliases refuse.

## Seeded generation

`splitmix64-top53-v1` maps a seed and counter to a wrapping SplitMix64 word,
uses its top 53 bits divided by 2^53 as a uniform draw in [0,1), and consumes
one counter per emitted byte. It uses the same word mapping as the existing
gpt-oss sampler. Counter exhaustion refuses before emission; a zero-length
generation preserves state. Categorical sampling scans IDs in increasing
order with strict CDF comparisons; the final byte gets the floating residual
and cutoff exactly one. The caller supplies the starting context byte and
room for 0–256 output bytes. Output publishes only after all requested draws.

The fixed capture starts from a space, seed 7, counter 11 and emits 128 bytes,
ending at counter 139. Every byte is legal, including controls and invalid
UTF-8. The example prints numeric bytes rather than interpreting them as text.
The untrained portions of the alphabet still receive probability, so a small
trained model can emit many non-text bytes. There is no claim of coherent
prose from this one-byte context model.

## Check the experiment

An independent stdlib Python oracle uses transition Counters and dictionary
contexts, separate row copies for updates, and per-transition loss evaluation.
It records the complete model, all 65 losses and the exact 128 sampled bytes.
The native comparison checks all 65,536 parameters within 1e-12 and losses
within 1e-11 nats, with byte-exact generation. Coax checks also compare analytic
gradients with finite differences, known RNG words, strict CDF ties, all-byte
endpoints, atomic refusals, trace tails and exact budgets.

| Partition | Bytes | Uniform bits/byte | Trained bits/byte |
| --- | --- | --- | --- |
| Train | 1209 | 8 | 3.382912 |
| Validation | 395 | 8 | 3.988865 |
| Test | 424 | 8 | 3.965398 |

The [reference](../fixtures/byte-lm/reference.json) pins every partition's
SHA-256 and the complete independent little-endian model bytes. The compressed
model is verification DATA, not shipped training logic. Updating provenance
metadata did not rerun training: partition bytes and their hashes stayed fixed.

[Run the complete example](../examples/byte-lm.coax):

<!-- source: psionic/examples/byte-lm.coax example -->
```jai
main :: () {
    train:[4096]u8;validation_bytes:[4096]u8;test_bytes:[4096]u8;
    for i:0..<BG_TRAIN.count {train[i]=BG_TRAIN[i];}for i:0..<BG_VALIDATION.count {validation_bytes[i]=BG_VALIDATION[i];}for i:0..<BG_TEST.count {test_bytes[i]=BG_TEST[i];}
    weights:[65536]f64;stage:[65536]f64;trace:[65]f64;
    fit:=bg_fit(train[0..<BG_TRAIN.count],64,32,weights[..],stage[..],trace[..],false);#assert fit.status==BgStatus.ok;
    validation:=bg_evaluate(weights[..],validation_bytes[0..<BG_VALIDATION.count]);test:=bg_evaluate(weights[..],test_bytes[0..<BG_TEST.count]);
    #assert validation.status==BgStatus.ok && test.status==BgStatus.ok;
    uniform:=8.0;#assert abs(fit.initial/log(2.0)-uniform)<1e-12 && validation.bits<uniform && test.bits<uniform;
    print("byte vocabulary: 256; parameters: 65536; updates: %\n",fit.updates);
    print("train nats: % -> %\n",fit.initial,fit.final);
    print("held out bits/byte: validation %, test %\n",validation.bits,test.bits);
    bytes:[128]u8;generated:=bg_generate(weights[..],CrSeed.{seed=7,counter=11},32,128,bytes[..],false);#assert generated.status==BgStatus.ok;
    print("splitmix64-top53-v1: seed 7, counter 11 -> %\n",generated.next.counter);
    print("first eight sampled bytes: % % % % % % % %\n",bytes[0],bytes[1],bytes[2],bytes[3],bytes[4],bytes[5],bytes[6],bytes[7]);
}
```

```shell
coax/bin/coax run psionic/examples/byte-lm.coax
psionic/check.sh --only byte-lm --jobs 8
```

The [transcript](../fixtures/byte-lm.out) is exact for the named CPU build.
[The verification record](../../docs/verification/psionic-ch73-2026-10-30.md)
records debug/release, mutation, native reference and sanitizer evidence.
The normal gate verifies captured data offline; `byte_lm_oracle.py --write`
explicitly regenerates the independent experiment when its inputs change.
Decoder gradients and transformer training remain chapters 113 and 115.
