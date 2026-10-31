# 66. A sequencer on the beat grid

> Module: [aethyr/audio/score.coax](../audio/score.coax) (the grid, keys,
> chords, and patterns). Checks:
> `coax/bin/coax test aethyr/audio/score.coax --library` (34 checks, with
> `synth.coax`'s) and
> `coax/bin/coax mutate aethyr/audio/score.coax --library --root-only` (206
> mutants: 195 killed, 11 marked equivalent, 0 survivors).

Music is written in beats, and sound is played in samples. At 120 BPM and
44,100 samples a second, a sixteenth note is 5,512.5 samples. A sequencer
that adds 5,512 samples per step is 0.5 samples late on every second step,
and a minute behind after enough bars. Aethyr's sequencer computes each
step's sample from the step's own tick, so the error never grows.

## What this chapter teaches

- How to place ticks on samples with integer arithmetic that doesn't drift.
- How to snap a time in seconds to the next step of a grid.
- How to build a key's chords from one voicing rule.
- How to step a pattern on the grid and get each step's exact sample.

## The grid

A `Grid` has a sample rate, a tempo in beats per minute, and an origin: the
sample that beat 0 falls on. A beat is `AU_PPQ` (960) ticks. A sixteenth is
240 ticks, an eighth is 480, and a bar of four beats is 3,840.

`au_grid_sample` converts a tick to a sample:

```jai
sample(tick) = origin + ⌊tick × rate × 60 / (bpm × 960)⌋
```

`au_grid_tick_at` inverts it. It returns the first tick on a step grid whose
sample is at or after a given sample. Because `au_grid_sample` rounds down,
the inverse rounds up: first to a tick, then to a multiple of the step.

<!-- source: aethyr/audio/score.coax grid -->
```jai
au_grid_sample :: (g: Grid, tick: int) -> int {
  return g.origin + tick * g.rate * 60 / (g.bpm * AU_PPQ);
}
// The first tick on a `step` grid whose sample is at or after `sample` (0 at the origin).
// au_grid_sample floors tick * rate * 60 / (bpm * PPQ), so the first tick at or after a
// sample is that ratio inverted and rounded up, then rounded up again to the step.
au_grid_tick_at :: (g: Grid, sample: int, step: int) -> int {
  if sample <= g.origin || step <= 0 { return 0; }   // mutant: equivalent (at the origin the formula gives 0 too)
  per := g.rate * 60;
  // bpm * PPQ and rate * 60 share a factor of 60, so the product's remainder by per is never 1:
  // rounding up with per - 2 gives the same tick.
  t := ((sample - g.origin) * g.bpm * AU_PPQ + per - 1) / per;   // mutant: equivalent
  return (t + step - 1) / step * step;
}
```

The check `a grid step's sample is the tick's own` tests 512 ticks up to about 3,600
bars. At 120 BPM and 44,100 Hz, every pair of consecutive sixteenths is
5,512 or 5,513 samples apart, and never any other gap.

## Sections on the grid

A score gives its section times in seconds, and the grid snaps each one to a
step. The promo bed in [chapter 67](ch067-scores-mixing-and-a-soundtrack.md)
has its sections at 4.138, 7.138, 10.138, and 14.138 seconds. With beat 0 at
0.138 seconds (sample 6,086), these times are beats 8, 14, 20, and 28. So
every patterned part starts exactly on a section.

## Keys and chords

A `Key` has a tonic, as a MIDI note, and a mode: natural minor or major.
`au_scale_note` returns the note a number of scale degrees above the tonic.
Degree 7 is the octave.

`au_triad` builds the triad on a scale degree. It folds each of the three
tones into the octave that starts at `lo`, sorts them, and folds the root
into the octave that starts at `root_lo`. With D minor (tonic 50), `lo` 60,
and `root_lo` 46, one rule gives the promo's four chords: Dm, B♭, F, and C
(the minor key's i, VI, III, and VII). The same rule voices A minor's Am and
F for the first, smaller bed.

<!-- source: aethyr/audio/score.coax triad -->
```jai
au_triad :: (k: Key, degree: int, lo: int, root_lo: int) -> Chord {
  c: Chord;
  for i: 0..<3 { c.tones[i] = au_fold(au_scale_note(k, degree + 2 * i), lo); }
  for i: 0..<2 { for j: 0..<2 - i {   // mutant: equivalent (a third pass over three sorted tones does nothing)
    if c.tones[j] > c.tones[j + 1] { t := c.tones[j]; c.tones[j] = c.tones[j + 1]; c.tones[j + 1] = t; }   // mutant: equivalent (swapping equal tones changes nothing)
  } }
  c.root = au_fold(au_scale_note(k, degree), root_lo);
  return c;
}
```

## Patterns

A `Pattern` is a row of up to 16 values with a step length in ticks. A value
can be a note offset or an accent. The value `AU_REST` plays nothing.
`au_sequence` steps a pattern from a start tick until an end sample. It writes
each step that plays as a `Step`: the tick, the sample, the index counted
from the first step, and the value.

<!-- source: aethyr/audio/score.coax sequence -->
```jai
au_sequence :: (g: Grid, p: Pattern, from: int, end: int, out: [] Step) -> int {
  if p.step <= 0 || p.length < 1 || p.length > AU_PATTERN_MAX { return 0; }
  n := 0; k := 0;
  tick := from;
  while n < out.count {
    at := au_grid_sample(g, tick);
    if at >= end { break; }
    v := p.values[k % p.length];
    if v != AU_REST { out[n] = Step.{tick = tick, at = at, index = k, value = v}; n += 1; }
    k += 1; tick += p.step;
  }
  return n;
}
```

`au_sequence` refuses a pattern with a step of zero or less, or with a length
outside 1 to 16. It returns 0 steps for either. It stops when the output
array is full, and it returns how many steps it wrote.

## Run it

The example [`aethyr/examples/beat-grid.coax`](../examples/beat-grid.coax)
prints the promo's sections on its grid, the four chords, and the first eight
steps of the string ostinato:

<!-- source: aethyr/examples/beat-grid.coax example -->
```jai
// The promo's grid: 120 BPM with beat 0 at 0.138 s (sample 6086). Its sections snap to
// beats, D minor's i VI III VII are voiced from one rule, and the ostinato's first steps
// fall on exact samples that alternate 5512 and 5513 apart.
main :: () {
  g := Grid.{rate = 44100, bpm = 120, origin = 6086};
  names := [4] string.["hold", "belt", "orbit", "home"];
  at := [4] int.[182486, 314786, 447086, 623486];
  for k: 0..<4 {
    t := au_grid_tick_at(g, at[k], AU_PPQ);
    print("% at sample %: beat %, sample %\n", names[k], at[k], t / AU_PPQ, au_grid_sample(g, t));
  }
  d := Key.{tonic = 50};
  degrees := [4] int.[0, 5, 2, 6];
  for k: 0..<4 { c := au_triad(d, degrees[k], 60, 46); print("bar %: root % tones % % %\n", k, c.root, c.tones[0], c.tones[1], c.tones[2]); }
  p := Pattern.{step = AU_SIXTEENTH, length = 8};
  offsets := [8] int.[0, 0, 12, 0, 0, 12, 0, 7];
  for k: 0..<8 { p.values[k] = offsets[k]; }
  steps: [8] Step;
  n := au_sequence(g, p, 8 * AU_PPQ, 1000000, steps[..]);
  last := steps[0].at;
  for k: 0..<n { print("step %: tick % sample % (+%) offset %\n", steps[k].index, steps[k].tick, steps[k].at, steps[k].at - last, steps[k].value); last = steps[k].at; }
}
```

```shell
coax/bin/coax run aethyr/examples/beat-grid.coax
```

```text
hold at sample 182486: beat 8, sample 182486
belt at sample 314786: beat 14, sample 314786
orbit at sample 447086: beat 20, sample 447086
home at sample 623486: beat 28, sample 623486
bar 0: root 50 tones 62 65 69
bar 1: root 46 tones 62 65 70
bar 2: root 53 tones 60 65 69
bar 3: root 48 tones 60 64 67
step 0: tick 7680 sample 182486 (+0) offset 0
step 1: tick 7920 sample 187998 (+5512) offset 0
step 2: tick 8160 sample 193511 (+5513) offset 12
step 3: tick 8400 sample 199023 (+5512) offset 0
step 4: tick 8640 sample 204536 (+5513) offset 0
step 5: tick 8880 sample 210048 (+5512) offset 12
step 6: tick 9120 sample 215561 (+5513) offset 0
step 7: tick 9360 sample 221073 (+5512) offset 7
```

The gaps between steps alternate between 5,512 and 5,513 samples.

## The checks

The grid and pattern checks in `score.coax` cover these behaviors:

- The grid lands on exact samples at 120 and 125 BPM.
- A step's sample is the tick's own, so a thousand bars don't drift.
- `au_grid_tick_at` returns a tick at or after the time, and the step before
  it is before the time. The check tests 400 times on a 125 BPM grid.
- The minor and major scales, and the promo's and the first bed's chords.
- Patterns skip rests, stop at the end sample or a full output array, and
  refuse bad steps and lengths.
- Every sequenced step is on its tick, at 64 tempos from 97 to 160 BPM.

The 11 mutants marked equivalent have a reason on their line. For example,
rounding up with `per - 2` instead of `per - 1` gives the same tick because
`bpm × 960` and `rate × 60` share a factor of 60.

## Dependencies

- [Chapter 65](ch065-sound-from-numbers.md)'s integer helpers.
- The grid is independent of [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s
  frame clock. A realm that starts music on a frame converts the frame's
  simulation time to a sample once, and the grid takes over from there.
