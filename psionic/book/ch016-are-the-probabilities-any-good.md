# 16. Are the probabilities any good?

Chapter 14 ended with a number: Elephant gave `yes` probability 0.757196 for
"Is the water wet?". This chapter asks three separate questions about such
numbers. Did we apply the calibration the model ships with? Can we print the
answer the published runtime would print? And on requests whose answers we
know, do the probabilities mean what they say?

None of these questions is about numerical conformance. Chapter 14 showed that
our arithmetic agrees with an independent implementation. That does not show
that the model is right, and a good score on a few requests does not show that
the arithmetic is right. We keep the two kinds of evidence apart.

Start with the [example program](../examples/probability-quality.coax). It runs
without model files:

```shell
coax/bin/coax run psionic/examples/probability-quality.coax
```

```jai
calibration: none, temperature 1.0
{"type":"choice","choice":"red","probabilities":{"red":0.665241,"green":0.244728,"blue":0.090031},"confidence":0.497861}
{"type":"choice","choice":"yes","probabilities":{"yes":0.757196,"no":0.242804}}
{"type":"score","score":0.42479,"legend":{"0":"low","1":"medium","2":"high"},"probabilities":{"0":0.665241,"1":0.244728,"2":0.090031},"confidence":0.362816}
accuracy 2 of 3, Brier 0.4396848500523909, calibration error 0.19109520884988118
```

## The calibration the bundle supplies

Both pinned revisions, Elephant `b3f8a934` and Mammoth `61cec44c`, contain the
same 63-byte `readout.json`:

```jai
{
  "readout": "slot_lm",
  "head_dim": 256,
  "n_slots": 256
}
```

It names the readout (the option letters' language-model logits, which chapter
13 reads), a head size and a slot count. It holds no temperature. Neither
revision contains a `calibration.json`. The bundle's `serve.patch` loads that
file only if it exists. Without it, OneJev's `Calibration` table is empty, its
source is `"none"`, and every question kind and option count uses temperature
1.0. The model card says the model is calibrated; that refers to training,
which added a Brier term to the loss, not to a temperature applied at serving
time.

So Psionic loads exactly that: it checks the readout bytes, refuses a
calibration file that the pinned revisions do not have, and returns 1.0 for
every valid question. It does not fit or invent a temperature.

<!-- source: psionic/lib/decision-calibration.coax calibration -->
```jai
pc_load :: (kind:u8,readout:[]u8,calibration:[]u8)->PcCalibration #export_c {
    if kind>1 {return PcCalibration.{status=PcStatus.kind};}
    if cast(string)readout!=PC_PINNED_READOUT {return PcCalibration.{status=PcStatus.readout};}
    // Neither pinned revision contains calibration.json; one is a different bundle.
    if calibration.count>0 {return PcCalibration.{status=PcStatus.unpinned};}
    return PcCalibration.{source=PcSource.none,slots=256,head_dim=256};
}
// With no table, the pinned runtime uses 1.0 for every kind and option count.
pc_temperature :: (c:PcCalibration,kind:QrKind,count:int)->f32 #export_c {
    if c.status!=PcStatus.ok || !qr_valid(cast(u8)kind,count,1) {return 0;}
    return 1;
}
```

A bundle with a `calibration.json` is a different bundle. Supporting one would
need a new pinned target identity and its own checks.

## The published answer

OneJev turns probabilities into answers in `qev/answers.py`, and its server
sends them with `json.dumps(..., ensure_ascii=False, separators=(",", ":"))`
(Starlette 1.8.0, as pinned by the bundle's `constraints.txt`). The rules are:

- Probabilities are first divided by their sum; an all-zero vector becomes
  uniform. Choice and score confidence normalize once more before use.
- Choice: `type`, `choice` (the first most probable name), `probabilities`
  keyed by option name, and `confidence`, the top probability rescaled so that
  uniform is 0 and certainty is 1.
- Noul: `type` and `noul`, the yes probability as given, clamped to 0..1.
- Score: `type`, `score` (the expected zero-based level), `legend` from level
  index to level text, `probabilities` keyed `"0"`, `"1"`, ..., and
  `confidence` (chapter 13's formula, never below 0).
- Every number is `round(x, 6)`: round half to even on the exact binary value,
  printed by Python's `repr`. So 0.0000005 prints `0.0` (its binary value is
  just below the midpoint), 0.000099 prints `9.9e-05`, and 0.0000999 prints
  `0.0001`.

The bundle's `engine.py`, used for the Decision Index, keeps fewer fields:
`type`, `choice` and `probabilities` for a choice, and `type`, `score` and
`probabilities` for a score. `PJ_DECISION_INDEX` selects that form;
`PJ_SYSTEM_ONE` selects the full answer.

<!-- source: psionic/lib/decision-json.coax answer-json -->
```jai
pj_emit :: (w:[]PjWriter,kind:QrKind,fields:u8,probabilities:[]f64,options:[]PtOption) {
    n:=probabilities.count;p:[255]f64;pj_normalize(probabilities,p[0..<n]);
    if kind==QrKind.noul {
        v:=probabilities[0];if v<=0 {v=0;}
        pj_text(w,"{\"type\":\"noul\",\"noul\":");pj_number(w,v);pj_byte(w,125);return;
    }
    if kind==QrKind.choice {
        pj_text(w,"{\"type\":\"choice\",\"choice\":");pj_string(w,options[pj_first_max(p[0..<n])].name);
        pj_text(w,",\"probabilities\":{");
        for i:0..<n {if i>0 {pj_byte(w,44);}pj_string(w,options[i].name);pj_byte(w,58);pj_number(w,p[i]);}
        pj_byte(w,125);
        if fields==PJ_SYSTEM_ONE {pj_text(w,",\"confidence\":");pj_number(w,pj_choice_confidence(p[0..<n]));}
        pj_byte(w,125);return;
    }
    pj_text(w,"{\"type\":\"score\",\"score\":");pj_number(w,pj_expected(p[0..<n]));
    if fields==PJ_SYSTEM_ONE {
        pj_text(w,",\"legend\":{");
        for i:0..<n {
            if i>0 {pj_byte(w,44);}pj_key(w,i);
            if options[i].description.kind==PtEntryKind.none {pj_text(w,"null");} else {pj_string(w,options[i].description.value);}
        }
        pj_byte(w,125);
    }
    pj_text(w,",\"probabilities\":{");
    for i:0..<n {if i>0 {pj_byte(w,44);}pj_key(w,i);pj_number(w,p[i]);}
    pj_byte(w,125);
    if fields==PJ_SYSTEM_ONE {pj_text(w,",\"confidence\":");pj_number(w,pj_score_confidence(p[0..<n]));}
    pj_byte(w,125);
}
```

`pj_round6` finds the six-decimal integer from the exact product
`x * 1,000,000`: a Dekker split gives the rounding error of the floating-point
product, and its sign decides values next to a midpoint. The printed digits
come from that integer, so no floating-point printing is involved.
`pj_answer` checks the shape, the probabilities (finite, 0..1), names and
levels (valid UTF-8, unique choice names) and overlap, then measures the output
before writing it: a refusal leaves the caller's buffer unchanged.

This covers one answer object. The response envelope (`model`, `answers`,
`usage`) belongs to the decision API in Part III. Score levels given as JSON
values are refused, because printing them would require re-serializing
arbitrary JSON. We add sums left to right, as Python's `sum` did before 3.12;
the Python version in the reference image was not checked. Compensated
summation could change a printed digit only for a value within one unit in the
last place of a rounding midpoint.

## Measuring decision quality

A labeled item names its question kind, its slice of a shared probability
array, the right option and, for a choice, an optional option that means "I
don't know". The metrics are:

- **Accuracy**: the first most probable option equals the label. For noul,
  slot 0 is yes; for score, the most probable level is compared.
- **Brier score**: the sum over options of `(p - target)^2`, from 0 to 2,
  averaged over items, overall and for each kind.
- **Calibration error**: top-label expected calibration error with 15
  equal-width bins `(b/15, (b+1)/15]`, as in OneJev's `calibrate.py`: for each
  bin, its share of the items times the gap between mean top probability and
  accuracy.
- **Abstention**: among items that offer an abstain option, how many chose it.
  Accuracy is also reported over the items that did not abstain. An abstention
  is correct only when the label is the abstain option.
- **Score error**: mean absolute difference between the expected level and the
  labeled level.

<!-- source: psionic/lib/decision-quality.coax quality -->
```jai
pq_measure :: (items:[]PqItem,probabilities:[]f64)->PqReport #export_c {
    if !pq_item_count(items.count) {return PqReport.{status=PqStatus.shape};}
    for item,index:items {
        if !pq_item_valid(item,probabilities.count) {return PqReport.{status=PqStatus.shape,cause=index};}
        sum:=0.0;
        for v:probabilities[item.start..<item.start+item.count] {if !(v>=0 && v<=1) {return PqReport.{status=PqStatus.number,cause=index};}sum+=v;}
        if abs(sum-1)>PQ_TOLERANCE {return PqReport.{status=PqStatus.number,cause=index};}
    }
    r:=PqReport.{items=items.count};confidence:[PQ_BINS]f64;hits:[PQ_BINS]f64;
    for item:items {
        p:=probabilities[item.start..<item.start+item.count];best:=pq_first_max(p);correct:=best==item.gold;
        brier:=0.0;expected:=0.0;
        for v,i:p {target:=0.0;if i==item.gold {target=1;}brier+=(v-target)*(v-target);expected+=f64(i)*v;}
        k:=cast(int)item.kind;r.kinds[k].items+=1;r.kinds[k].brier+=brier;r.brier+=brier;
        if correct {r.kinds[k].correct+=1;r.correct+=1;}
        if item.kind==QrKind.score {r.score_error+=abs(expected-f64(item.gold));}
        if item.abstain>=0 {r.offered+=1;}
        if item.abstain>=0 && best==item.abstain {r.abstained+=1;} else {r.answered+=1;if correct {r.answered_correct+=1;}}
        // Top-label calibration: the first equal-width bin (b/15,(b+1)/15] that holds c.
        c:=p[best];b:=0;while c>f64(b+1)/PQ_BINS {b+=1;}
        confidence[b]+=c;if correct {hits[b]+=1;}
    }
    n:=f64(items.count);
    // Sum over bins of (m/n)*|mean confidence - accuracy|, with each bin's count m cancelled.
    for b:0..<PQ_BINS {r.ece+=abs(confidence[b]-hits[b])/n;}
    r.brier/=n;
    for k:0..<3 {if r.kinds[k].items!=0 {r.kinds[k].brier/=f64(r.kinds[k].items);}}
    if r.kinds[2].items!=0 {r.score_error/=f64(r.kinds[2].items);}
    return r;
}
```

The checks derive small cases by hand (four items, bin edges at exactly 12/15,
the first bin, ties, two score items) and compare a 40-item random case with
an independent Python transcription of `calibrate.py`. Each distribution must
sum to one within 2^-30; malformed items refuse with their index.

## Elephant on a small held-out set

The [held-out set](../fixtures/quality/heldout-v1.json) has twelve owned text
decisions written for this chapter: five choices (two offer an `unknown`
option, and for one of them `unknown` is the right answer), four noul questions
and three scores with three levels each. The set, its labels and the metric
code were committed before Elephant saw any of them, and the
[corpus check](../tests/quality_corpus.py) pins the file's SHA-256. Nothing is
tuned on it; there is nothing to tune, since the temperature is fixed at 1.0.

The run harness authenticates the original files once, prepares every request
in Coax, then runs each decision in its own process. It does not download
anything:

```shell
coax/build.sh
psionic/tests/run_elephant_quality.sh /path/to/elephant-directory 12
```

The reported numbers come from Psionic's **CPU reference backend**: original
BF16 weights, release build, one i7-14700K shared with other jobs. Each
decision took 69 to 81 minutes; the whole run took 83 minutes. The
[complete output](../fixtures/quality/heldout-v1-elephant-cpu.txt) holds every
probability and answer, for example:

```jai
choice-room-unknown tokens=117 prefix=57 best=2 gold=2 mass=0.838727 seconds=4545
  p=0.004286453,0.007238023,0.988475524
  {"type":"choice","choice":"unknown","probabilities":{"room 101":0.004286,"room 204":0.007238,"unknown":0.988476},"confidence":0.982713}
```

| Measure | Result |
| --- | --- |
| Accuracy | 12 of 12 (choice 5/5, noul 4/4, score 3/3) |
| Brier score | 0.000056 (choice 0.000053, noul 0.000065, score 0.000050) |
| Calibration error (15 bins) | 0.003476 |
| Score mean absolute error | 0.003154 levels |
| Abstention | 1 of 2 offering items chose `unknown`, and it was the right answer; 11 of 11 answered items were right |

Read these numbers for what they are. Twelve items cannot measure
calibration: every top probability fell in the last bin (14/15, 1], so the
calibration error only says that the model was confident and right. No answer
was wrong, so the set cannot show overconfidence on mistakes. With 12 of 12
right, the 95% one-sided lower bound on accuracy is still only about 0.78. [Chapter 238](ch238-how-far-a-metric-moves-by-itself.md)
refuses accuracy on this suite for that reason: no item is left to gain on. The
requests are short and easy, and we wrote them ourselves; this is not a
benchmark and not a comparison with the published Decision Index. A larger
corpus, and a faster backend to run it, remain open.

## What the checks establish

The [answer oracle](../tests/answer_oracle.py) uses Python's own `round` and
`json.dumps` for 90 answers, including values on both sides of six-decimal
midpoints, quoted and escaped names, null legends, ties and every kind. Both
fixtures, the hand-derived checks and the native adapters run in debug and
release, plain and with ASan/UBSan; the three Coax files are `#authority` and
`coax mutate` finds no surviving mutant. The
[verification record](../../docs/verification/psionic-ch12-2026-10-29.md) gives
the exact identities, timings and limits.

These checks show that Psionic reads the bundle's calibration as the pinned
runtime does, prints the same answer bytes for the same probabilities, and
computes the stated metrics correctly. They do not show that Elephant is well
calibrated in general: twelve owned requests are far too few for that.
