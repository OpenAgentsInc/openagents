# 65. Sound from numbers: oscillators and envelopes

> Module: [aethyr/audio/synth.coax](../audio/synth.coax). Checks:
> `coax/bin/coax test aethyr/audio/synth.coax --library` (20 checks) and
> `coax/bin/coax mutate aethyr/audio/synth.coax --library` (254 mutants: 237
> killed, 17 marked equivalent, 0 survivors).

A realm needs sound that is the same every time it runs. A soundtrack that
uses the C library's `sin` and `exp`, a floating-point mix, or a random
generator seeded from the clock can give different samples on a native build
and in the browser. Aethyr's synthesizer uses integers only, so the same
score and seed give the same samples on every build and target.

This chapter builds the parts that make one sound: phases and pitches,
oscillators, seeded noise, envelopes, a filter, panning, and saturation.
[Chapter 66](ch066-a-sequencer-on-the-beat-grid.md) puts sounds on a beat
grid, and [chapter 67](ch067-scores-mixing-and-a-soundtrack.md) mixes them
into a soundtrack.

## What this chapter teaches

- How to represent samples, phases, and time as integers.
- How to make saw, square, and sine waves from a phase without `libm`.
- How to make noise that depends only on a seed.
- How to shape a note's level with an ADSR envelope, and a decay with a
  repeated multiply.
- How to pan a sound with constant power and saturate a mix without a
  clip.

## The number formats

The module uses three integer formats:

- A sample is Q16 fixed point: `AU_ONE` (65,536) is full scale.
- A phase is a 32-bit fraction of one cycle: `AU_CYCLE` (2³²) is one turn,
  and `au_phase` wraps a phase with `& AU_MASK`.
- Time is a whole number of samples.

A frequency is in milli-hertz. `au_inc_mhz` converts it to the phase step
that one sample adds: `mhz × 2³² / (1000 × rate)`. For A4 (440 Hz) at
44,100 samples a second, the step is 42,852,281 phase units.

Pitches come from MIDI note numbers. `AU_TOP_MHZ` holds the twelve notes of
MIDI octave 10 (notes 120–131) in milli-hertz. `au_note_mhz` shifts that
octave right by the number of octaves between it and the note, so a low note
keeps the precision of the top octave. A4 is exactly 440,000 mHz, and A3 is
exactly 220,000 mHz.

## Oscillators

An oscillator maps a phase to a sample. The saw is the phase's top 17 bits,
shifted to be centered on zero. The square is full scale for the first half
of the cycle and negative full scale for the second half.

<!-- source: aethyr/audio/synth.coax saw -->
```jai
au_saw :: (p: int) -> int { return (p >> 15) - AU_ONE; }
au_square :: (p: int) -> int { return ifx p < AU_CYCLE / 2 then AU_ONE else -AU_ONE; }
```

The sine uses an odd polynomial on a quarter wave and mirrors it into the
other three quarters. The coefficients are fitted by least squares with the
constraint `a + b + c = 1`, so the sine is exactly full scale at a quarter
cycle. The largest error against `sin` is 1.04 × 10⁻⁴ of full scale.

<!-- source: aethyr/audio/synth.coax sine -->
```jai
AU_SIN_A :: 102914; AU_SIN_B :: -42081; AU_SIN_C :: 4703;
au_sine :: (p: int) -> int {
  q := p / AU_QUARTER;                      // which quarter, 0..3
  r := (p % AU_QUARTER) >> 14;              // 0..65535 into it
  x := ifx q == 0 || q == 2 then r else AU_ONE - r;
  x2 := x * x / AU_ONE;
  s := x * (AU_SIN_A + x2 * (AU_SIN_B + x2 * AU_SIN_C / AU_ONE) / AU_ONE) / AU_ONE;
  return ifx q < 2 then s else -s;
}
au_cosine :: (p: int) -> int { return au_sine(au_phase(p, AU_QUARTER)); }
```

The check `the sine is within 1e-4 of sin` compares 4,096 phases against the
`f64` `sin` of the compiler's evaluator. It allows an error of 12 Q16 steps,
which is 1.8 × 10⁻⁴.

## Seeded noise

Noise comes from xorshift32. The state is the only input, so a seed gives
the same sequence on every run. A zero seed would stay zero, so
`au_noise(0)` takes the fixed seed `AU_NOISE_SEED` instead. The module never
reads a clock.

<!-- source: aethyr/audio/synth.coax noise -->
```jai
au_noise_next :: (n: Noise) -> Noise {
  x := n.state;
  x = x ^ ((x << 13) & AU_MASK);
  x = x ^ (x >> 17);
  x = x ^ ((x << 5) & AU_MASK);
  return Noise.{state = x};
}
```

`au_noise_value` maps the state to a Q16 sample in `[-AU_ONE, AU_ONE)`.
`au_noise_phase` returns the state itself, which the scores use as an
oscillator's random start phase.

## Envelopes

An `Adsr` holds an attack, a decay, and a release in samples, and a sustain
level in Q16. `au_adsr` makes each length at least one sample and clamps the
sustain to `[0, AU_ONE]`.

`au_adsr_level` returns the level `i` samples after a note starts, for a note
held `held` samples:

1. During the attack, the level rises from 0 to full scale.
2. During the decay, it falls to the sustain level.
3. Until `held`, it stays at the sustain level.
4. During the release, it falls to 0 from the level it had at `held`.

The release starts from the gate's level, not from the sustain level, so a
note released during its attack doesn't jump.

<!-- source: aethyr/audio/synth.coax adsr -->
```jai
au_adsr_level :: (e: Adsr, held: int, i: int) -> int {
  if i < 0 { return 0; }   // mutant: equivalent (i <= 0 and i < 1 add only i == 0, where the gate is 0 too)
  h := au_max(0, held);
  if i < h { return au_adsr_gate(e, i); }   // mutant: equivalent (at i == h release starts from the gate's own level)
  if i >= h + e.release { return 0; }   // mutant: equivalent (> reaches 0 at i == h + release anyway)
  from := au_adsr_gate(e, h);
  return from - from * (i - h) / e.release;
}
```

A drum or a sweep needs an exponential decay instead. `au_decay_rate` turns a
time constant `tau`, in samples, into a Q30 factor of `1 - 1/tau`. This is
`exp(-1/tau)` to second order. `au_decay_step` multiplies a level by that
factor once a sample. The check `decay is a repeated multiply` confirms that
the level halves after 693 samples for a `tau` of 1,000, within 0.2%.

`au_pole` is the one-pole lowpass filter `y ← y + c(x − y)`. A small `c`
removes more of the high frequencies.

<!-- source: aethyr/audio/synth.coax decay -->
```jai
au_decay_rate :: (tau: int) -> int { return AU_Q30 - AU_Q30 / au_max(1, tau); }
au_decay_step :: (level: int, rate: int) -> int { return level * rate / AU_Q30; }

// ---- filter and gain

// One-pole lowpass: y moves a fraction c (Q16) of the way to x each sample.
au_pole :: (y: int, x: int, c: int) -> int { return y + c * (x - y) / AU_ONE; }
```

## Panning and saturation

`au_pan` takes a pan in thousandths, from −1000 (left) to 1000 (right). It
turns that into an angle of 0 to a quarter cycle, and returns the cosine as
the left gain and the sine as the right gain. The sum of the squares of the
two gains stays at full power. At the center, both gains are 46,344
(0.7072). The check `constant-power pan` confirms the power within 0.04% at
41 positions.

<!-- source: aethyr/audio/synth.coax pan -->
```jai
au_pan :: (pan: int) -> Pan {
  p := au_clamp(pan, -AU_PAN_WIDE, AU_PAN_WIDE);
  // The angle runs 0 .. a quarter cycle. One phase unit more is below the sine's 2^14-unit
  // input step for every pan from -1000 to 1000 (searched), so it cannot be heard.
  angle := (p + AU_PAN_WIDE) * (AU_QUARTER / 2) / AU_PAN_WIDE;   // mutant: equivalent
  return Pan.{left = au_cosine(angle), right = au_sine(angle)};
}
```

`au_saturate` is a soft clip. It uses the Padé approximant of `tanh`,
`u(27 + u²)/(27 + 9u²)`, which reaches exactly 1 at `u = 3`. Its slope is
`9(u² − 9)²/(27 + 9u²)²`, which is never negative, so the curve never
decreases. Dividing by the curve's value at the drive maps full scale in to
full scale out. At the scores' drive of 1.3, a quarter of full scale comes
out as 23,375, about 0.357.

<!-- source: aethyr/audio/synth.coax saturate -->
```jai
au_tanh :: (u: int) -> int {
  if u < 0 { return -au_tanh(-u); }
  if u >= 3 * AU_ONE { return AU_ONE; }   // mutant: equivalent (the approximant is exactly AU_ONE at u = 3)
  u2 := u * u / AU_ONE;
  return u * (27 * AU_ONE + u2) / (27 * AU_ONE + 9 * u2);
}
au_saturate :: (x: int, drive: int) -> int {
  top := au_tanh(drive);
  if top <= 0 { return 0; }
  return au_tanh(x * drive / AU_ONE) * AU_ONE / top;
}
```

The check `saturation never decreases, and full scale in never passes full
scale out` tests 512 inputs from −1 to 1, and 512 more up to eight times full
scale.

## Run it

The example
[`aethyr/examples/sound-from-numbers.coax`](../examples/sound-from-numbers.coax)
prints A4's phase step, each oscillator at four phases, an envelope, five pan
positions, four noise values, and the saturation curve:

<!-- source: aethyr/examples/sound-from-numbers.coax example -->
```jai
// A4 at 44.1 kHz: its phase step, one cycle of each oscillator at four points, an
// envelope's shape, pan gains, seeded noise and the saturation curve. Every value is an
// integer, so this transcript is the same on every build.
main :: () {
  inc := au_note_inc(69, 44100);
  print("A4: % mHz, % phase units a sample\n", au_note_mhz(69), inc);
  for k: 0..<4 {
    p := k * AU_QUARTER + AU_QUARTER / 2;
    print("  phase %/8: saw % square % sine %\n", 2 * k + 1, au_saw(p), au_square(p), au_sine(p));
  }
  e := au_adsr(441, 2205, 39322, 4410);        // 10 ms, 50 ms, 0.6, 100 ms
  print("envelope held 0.2 s:");
  for t: 0..<7 { print(" %", au_adsr_level(e, 8820, t * 2205)); }
  print("\n");
  for pan: -2..<3 { g := au_pan(pan * 500); print("pan %: left % right %\n", pan * 500, g.left, g.right); }
  z := au_noise(11);
  print("noise from seed 11:");
  for i: 0..<4 { z = au_noise_next(z); print(" %", au_noise_value(z)); }
  print("\n");
  print("saturate at drive 1.3:");
  for k: 0..<5 { print(" %", au_saturate(k * AU_ONE / 4, 85197)); }
  print("\n");
}
```

```shell
coax/bin/coax run aethyr/examples/sound-from-numbers.coax
```

```text
A4: 440000 mHz, 42852281 phase units a sample
  phase 1/8: saw -49152 square 65536 sine 46344
  phase 3/8: saw -16384 square 65536 sine 46344
  phase 5/8: saw 16384 square -65536 sine -46344
  phase 7/8: saw 49152 square -65536 sine -46344
envelope held 0.2 s: 0 44565 39322 39322 39322 19661 0
pan -1000: left 65536 right 0
pan -500: left 60549 right 25075
pan 0: left 46344 right 46344
pan 500: left 25075 right 60549
pan 1000: left 0 right 65536
noise from seed 11: -65446 -42987 -56210 65119
saturate at drive 1.3: 0 23375 42918 56846 65536
```

The book checker runs it in debug and release builds and compares both
transcripts byte for byte with
[`aethyr/fixtures/sound-from-numbers.out`](../fixtures/sound-from-numbers.out).

## The checks

The 20 checks in `synth.coax` cover these behaviors:

- Phases wrap at one cycle, and notes are equal-tempered from A4 = 440 Hz.
- The oscillators hit their exact values at the cardinal phases.
- The sine stays within 1.8 × 10⁻⁴ of `sin`, is odd about half a cycle, and
  rises through its first quarter.
- Noise is the same for the same seed, never reaches zero, and covers both
  signs.
- An envelope stays in `[0, AU_ONE]` and changes by at most one attack step
  a sample.
- Pan keeps constant power, and saturation never decreases.
- Recorded values pin the sine, noise, saturation, and envelope between the
  cardinal points, and the reverb lines that
  [chapter 67](ch067-scores-mixing-and-a-soundtrack.md) uses.

Each of the 17 mutants marked equivalent has a reason on its line. For
example, `au_max` returns the same value for `>` and `>=` when its arguments
are equal, and a cleared delay line can start at any index.

## Dependencies

- Coax integer arithmetic and fixed arrays, and the checked `#test` and
  `#forall` forms.
- Nothing from earlier chapters. A realm that plays sound uses
  [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock to decide when, and
  [chapter 5](ch005-replay-a-session-exactly.md)'s seed to decide which noise.
