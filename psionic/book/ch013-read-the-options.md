# 13. Read the options

A decision model does not generate a free-form answer here. Its language-model
head produces a logit for every vocabulary token. We select the one-token slots
for the requested options and turn those logits into a distribution; the
caller keeps the option labels and assembles the eventual response.

```shell
coax/bin/coax run psionic/examples/read-options.coax
psionic/check.sh
```

```jai
choice: option 0, probability 0.6652409557748218, confidence 0.4978614336622327
slot mass in vocabulary: 0.9174054605564647
score expectation: 0.42478961739555854, confidence 0.3628155739066622
```

This small example uses synthetic logits. In the pinned Qwen tokenizer, slot
zero is token 32 and slot one is token 33. Our 256 pinned IDs come from the
original tokenizer fixtures checked in chapter 9. The caller must use those
IDs for these models; the generic readout accepts any unique valid IDs.

## Two different probabilities

Slot mass is the uncalibrated fraction of **all vocabulary** probability
assigned to the selected slots. Option probabilities normalize only over the
selected slots, after dividing their logits by the caller's positive
finite temperature. This distinction means option probabilities sum to one
even when most vocabulary mass lies elsewhere. Subtracting the appropriate
maximum before exponentiation prevents overflow.

```jai
slot_mass = sum(exp(slot_logit)) / sum(exp(vocabulary_logit))
option_probability = exp(slot_logit / temperature) / sum(exp(slot_logit / temperature))
```

A production caller needs every vocabulary logit to compute mass; projecting
only the chosen rows cannot establish it. The reference implementation's
[readout](https://github.com/OmniJev/OneJev/blob/81ce62f1597c91e46767d4d02d6ac2e18534fe94/qev/engine.py)
and [calibration](https://github.com/OmniJev/OneJev/blob/81ce62f1597c91e46767d4d02d6ac2e18534fe94/qev/calibrate.py)
separate those calculations. This chapter takes a temperature explicitly; it
does not claim that a calibration artifact has been loaded.

## From a distribution to a typed result

Choice returns the first maximum and rescales its probability from the uniform
baseline to confidence. A one-option choice has confidence one. Noul returns
the probability of slot zero, the yes slot. Score returns the expected
**zero-based level index**, and confidence measures distance from the modal
index relative to the uniform distribution. Its one-level confidence is one.
The caller maps indices to labels. The formulas follow the pinned
[answer assembly](https://github.com/OmniJev/OneJev/blob/81ce62f1597c91e46767d4d02d6ac2e18534fe94/qev/answers.py).
The result here retains `f64` values; reference JSON's six-decimal rounding
and response serialization are separate work.

<!-- source: psionic/lib/readout.coax readout -->
```jai
qr_read :: (kind:QrKind,temperature:f32,logits:[]f32,slots:[]int,out:[]f64,work:[]f64,overlap:bool)->QrResult #export_c {
    n:=slots.count;if !qr_valid(cast(u8)kind,n,temperature) || !qr_vocab_valid(logits.count) {return QrResult.{status=QrStatus.shape};}
    if out.count<n || work.count<n {return QrResult.{status=QrStatus.capacity};}
    if overlap {return QrResult.{status=QrStatus.alias};}
    for value:logits {if !psionic_tensor_finite_bits(f32_bits(value)) {return QrResult.{status=QrStatus.number};}}
    for i:0..<n {if slots[i]<0 || slots[i]>=logits.count {return QrResult.{status=QrStatus.index};}for j:0..<i {if slots[i]==slots[j] {return QrResult.{status=QrStatus.index};}}}
    max_vocab:=f64(logits[qr_argmax_all(logits)]);
    all:PsionicAccumulator;for value:logits {all=psionic_ops_accumulate(all,exp(f64(value)-max_vocab));}
    selected:PsionicAccumulator;best:=qr_argmax_slots(logits,slots);max_slot:=f64(logits[slots[best]])/f64(temperature);
    for id:slots {selected=psionic_ops_accumulate(selected,exp(f64(logits[id])-max_vocab));}
    denominator:PsionicAccumulator;
    for i:0..<n {work[i]=exp(f64(logits[slots[i]])/f64(temperature)-max_slot);denominator=psionic_ops_accumulate(denominator,work[i]);}
    total:=psionic_ops_total(denominator);
    for i:0..<n {work[i]/=total;}
    result:=QrResult.{written=n,best=best,mass=psionic_ops_total(selected)/psionic_ops_total(all)};
    if kind==QrKind.choice {
        result.confidence=1;
        if n>1 {uniform:=1/f64(n);result.confidence=(work[best]-uniform)/(1-uniform);}
    } else if kind==QrKind.noul {result.truth=work[0];}
    else {
        expected:PsionicAccumulator;distance:PsionicAccumulator;uniform_distance:PsionicAccumulator;center:=f64(n-1)/2;
        for i:0..<n {expected=psionic_ops_accumulate(expected,f64(i)*work[i]);distance=psionic_ops_accumulate(distance,abs(f64(i)-f64(best))*work[i]);uniform_distance=psionic_ops_accumulate(uniform_distance,abs(f64(i)-center));}
        result.score=psionic_ops_total(expected);result.confidence=1;
        if n>1 {result.confidence=qr_nonnegative(1-psionic_ops_total(distance)/(psionic_ops_total(uniform_distance)/f64(n)));}
    }
    for i:0..<n {out[i]=work[i];}return result;
}
```

The profile admits 1–262,144 finite `f32` vocabulary logits, 1–255 choice
slots, exactly two noul slots or 1–10 score slots. IDs must be distinct and
inside the vocabulary. The output and scratch each need one `f64` per slot,
must not overlap each other or either input, and can have unused tails.
Capacity, alias, bad numbers and bad indices refuse before publishing output;
scratch may change on a later refusal. Native address observations cannot
prove pointer validity, alignment, truthful extents or exclusive access.

## Run a tiny readout

The [example program](../examples/read-options.coax) uses the shared readout.

<!-- source: psionic/examples/read-options.coax example -->
```jai
main :: () {
    // Vocabulary logits: three option slots and one other token.
    logits:=[4]f32.[2,1,0,0];slots:=[3]int.[0,1,2];
    probabilities:[3]f64;work:[3]f64;
    choice:=qr_read(QrKind.choice,1,logits[..],slots[..],probabilities[..],work[..],false);
    #assert choice.status==QrStatus.ok;
    print("choice: option %, probability %, confidence %\n",choice.best,probabilities[choice.best],choice.confidence);
    print("slot mass in vocabulary: %\n",choice.mass);
    score:=qr_read(QrKind.score,1,logits[..],slots[..],probabilities[..],work[..],false);
    #assert score.status==QrStatus.ok;
    print("score expectation: %, confidence %\n",score.score,score.confidence);
}
```

Independent scalar fixtures cover ties, high magnitudes, temperature, score
confidence and different kinds. Native checks exercise the full 248,320-token
vocabulary, all pinned IDs and real overlapping buffers. The
[verification record](../../docs/verification/psionic-ch09-2026-10-29.md)
identifies the tested machines and limits. This chapter does not yet execute a
checkpoint or serve a complete request.
