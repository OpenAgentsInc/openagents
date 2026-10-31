# 67. Scores, mixing and a soundtrack

> Modules: [aethyr/audio/score.coax](../audio/score.coax) (buses, the reverb
> send, automation, and the master), [aethyr/audio/wav.coax](../audio/wav.coax)
> (WAV files and the sample hash), and
> [aethyr/audio/epic.coax](../audio/epic.coax) (the `promo-epic` score). Tool:
> [aethyr/tools/score.coax](../tools/score.coax). Checks: `coax/bin/coax test`
> on each module with `--library`; `coax/bin/coax mutate` on `wav.coax` (104
> mutants, 104 killed).

This chapter mixes the sounds of
[chapter 65](ch065-sound-from-numbers.md), placed by the sequencer of
[chapter 66](ch066-a-sequencer-on-the-beat-grid.md), into a stereo
soundtrack. The example score is `promo-epic`, the music bed of the
OpenAgents promo video. A tool renders it to a 16-bit stereo WAV file, and
the same score and seed always give the same file.

## What this chapter teaches

- How to put a voice on stereo buses with a pan and a reverb send.
- How a Schroeder reverb works: parallel combs, then allpasses.
- How to automate gain over a score with breakpoints.
- How to normalize, automate, saturate, and convert to 16 bits without a
  clip.
- How to write a WAV file and hash the samples, so that a check can confirm
  that a render didn't change.

## Buses

A score mixes into four buses, each a view of `int` samples:

- `l` and `r`: the dry left and right channels.
- `sl` and `sr`: the reverb send's left and right channels.

`au_put` adds a voice to the buses at a start sample. It scales the voice by
a gain, splits it between left and right with
[chapter 65](ch065-sound-from-numbers.md)'s constant-power pan, and adds a
share of each side to the send. It skips the samples before sample 0 and
stops at the end of the buses.

<!-- source: aethyr/audio/score.coax put -->
```jai
au_put :: (voice: [] int, start: int, amp: int, pan: int, send: int, l: [] int, r: [] int, sl: [] int, sr: [] int) {
  g := au_pan(pan);
  gl := au_gain(amp, g.left); gr := au_gain(amp, g.right);
  for k: 0..<voice.count {
    j := start + k;
    if j >= l.count { break; }
    if j >= 0 {
      a := au_gain(voice[k], gl); b := au_gain(voice[k], gr);
      l[j] += a; r[j] += b; sl[j] += au_gain(a, send); sr[j] += au_gain(b, send);
    }
  }
}
```

## The reverb send

`au_reverb` is a Schroeder reverb:

1. Six feedback combs run in parallel over the send. Their delays are 1,557,
   1,617, 1,491, 1,422, 1,277, and 1,356 samples, with feedback of 0.84 or
   0.83. Each comb lowpasses its feedback, so the tail loses its high
   frequencies first.
2. Two allpasses run in series over the combs' sum, with delays of 556 and
   441 samples and a gain of 0.5. An allpass spreads the echoes in time and
   leaves the frequency balance unchanged.

<!-- source: aethyr/audio/synth.coax comb -->
```jai
au_comb :: (input: [] int, out: [] int, line: [] int, delay: int, feedback: int, damp: int) {
  for i: 0..<delay { line[i] = 0; }
  at := 0; f := 0;   // mutant: equivalent (a cleared ring may start anywhere; f = 1 is below Q16's resolution after one gain)
  for i: 0..<input.count {
    y := line[at];
    f = au_gain(y, AU_ONE - damp) + au_gain(f, damp);
    line[at] = input[i] + au_gain(f, feedback);
    at += 1;
    if at == delay { at = 0; }
    out[i] += y;
  }
}
```

`au_reverb_return` runs the reverb over each send channel in turn and adds
the result into the dry bus at a wet level. The right channel's lines are
`spread` samples longer. The scores use a spread of 23, so the two sides
don't echo at the same time. The checks record the reverb's response to two
impulses, left and right: the first echo is at sample 1,277 on the left and
1,300 on the right.

## Gain automation

A `Point` is a level, in Q16, at a sample. `au_automation` interpolates
linearly between points. Before the first point, it returns the first level,
and after the last point, the last level.

<!-- source: aethyr/audio/score.coax automation -->
```jai
au_automation :: (points: [] Point, i: int) -> int {
  if points.count == 0 { return AU_ONE; }
  if i <= points[0].at { return points[0].level; }   // mutant: equivalent (at the first point the segment below gives its level too)
  for k: 1..<points.count {
    a := points[k - 1]; b := points[k];
    if i < b.at { return a.level + (b.level - a.level) * (i - a.at) / (b.at - a.at); }   // mutant: equivalent (at b.at the segment gives b's level)
  }
  return points[points.count - 1].level;
}
```

## The master

`au_master` turns the two dry buses into 16-bit samples. For each frame, it
does these steps:

1. Divide by the peak of both channels, scaled to `AU_HEADROOM` (0.95 of full
   scale).
2. Multiply by the automation's level at that frame.
3. Saturate with a drive of 1.3.
4. Scale to `AU_PCM_TOP` (32,000) steps.

It writes the frames interleaved, left then right, and returns the peak.
Silence returns a peak of 0 and writes zeros.

<!-- source: aethyr/audio/score.coax master -->
```jai
au_master :: (l: [] int, r: [] int, points: [] Point, drive: int, pcm: [] int) -> int {
  n := au_min(l.count, au_min(r.count, pcm.count / 2));
  peak := au_peak(l[0..<n], r[0..<n]);
  for i: 0..<n {
    g := au_automation(points, i);
    a := 0; b := 0;   // mutant: equivalent (only silence keeps them, and 1 * AU_PCM_TOP / AU_ONE is 0)
    if peak > 0 {
      a = au_saturate(au_gain(l[i] * AU_HEADROOM / peak, g), drive);
      b = au_saturate(au_gain(r[i] * AU_HEADROOM / peak, g), drive);
    }
    pcm[2 * i] = a * AU_PCM_TOP / AU_ONE;
    pcm[2 * i + 1] = b * AU_PCM_TOP / AU_ONE;
  }
  return peak;
}
```

The master can't clip. After step 1, every sample is at most 0.95 of full
scale. The automation's levels are at most 1, and the saturation maps 0.95 to
less than full scale. So the largest output is 31,325, below 32,767. The check
`whatever the mix, the master stays inside 16 bits` confirms it for 64 random
mixes, and confirms that the loudest sample of each one reaches exactly
31,325.

## WAV files and the sample hash

`wav_write` writes a 44-byte RIFF header and the samples as little-endian
16-bit integers. It returns −1 if the output array is too small, if the
channel count doesn't divide the sample count, or if a sample is outside 16
bits.

<!-- source: aethyr/audio/wav.coax wav -->
```jai
wav_write :: (pcm: [] int, channels: int, rate: int, out: [] u8) -> int {
  if channels < 1 || rate < 1 || pcm.count % channels != 0 { return -1; }
  data := pcm.count * 2;
  if out.count < WAV_HEADER + data { return -1; }
  for v: pcm { if v < -32768 || v > 32767 { return -1; } }
  tag := "RIFF....WAVEfmt ....";
  for i: 0..<20 { out[i] = tag[i]; }
  wav_u32(out, 4, 36 + data);
  wav_u32(out, 16, 16); wav_u16(out, 20, 1); wav_u16(out, 22, channels);
  wav_u32(out, 24, rate); wav_u32(out, 28, rate * channels * 2);
  wav_u16(out, 32, channels * 2); wav_u16(out, 34, 16);
  name := "data";
  for i: 0..<4 { out[36 + i] = name[i]; }
  wav_u32(out, 40, data);
  for i: 0..<pcm.count { wav_u16(out, WAV_HEADER + 2 * i, pcm[i] & 65535); }
  return WAV_HEADER + data;
}
```

`wav_hash` is FNV-1a (64-bit) over the same bytes. The same samples give the
same hash on every build and target. The checks use it to pin a render.

## The `promo-epic` score

`promo-epic` is in D minor at 120 BPM, with one chord a bar: Dm, B♭, F, C. Its
parameters are an `Epic` record. The times are typed
`units.Quantity(units.Second, f64)` values, and `au_samples` converts each
one to whole samples once. The defaults match the video's cut: 17.64 s long,
beat 0 at 0.138 s, and sections at 4.138 s (hold), 7.138 s (belt), 10.138 s
(orbit), and 14.138 s (home).

The score has these parts:

- A pad and drone every bar: three detuned saws on each chord tone and the
  root an octave down. It plays twice, 13 ms apart, panned 0.3 left and 0.3
  right.
- A choir on every bar that starts no earlier than half a second before the
  belt: sines an octave up, with a
  second harmonic and a 0.4% vibrato at 5.2 Hz.
- A string ostinato in sixteenths from the hold: offsets 0, 0, 12, 0, 0, 12,
  0, 7 on the root, accented on each beat, panned left and right in turn.
- Taiko from the belt on beats 1 and 3, and also on beat 4 from the orbit. A
  higher drum plays on the off-beat after beats 2 and 4, also from the orbit.
- Brass swells every bar from the orbit. The filter opens as the envelope
  rises: its coefficient is `0.02 + 0.12 × level`.
- A noise riser into the home, then a sub boom, a taiko, and the tonic chord
  at the home.

Every patterned part enters on the first grid step at or after its section.
The one-shots at the home land on its exact sample. The ostinato and the
drums show how a part uses the sequencer:

<!-- source: aethyr/audio/epic.coax parts -->
```jai
  // 2. the string ostinato in 16ths from the hold, accented on the beat, panned in turn
  os := Pattern.{step = AU_SIXTEENTH, length = 8};
  for k: 0..<8 { os.values[k] = AU_OSTINATO[k]; }
  steps: [512] Step;
  ns := au_sequence(g, os, au_grid_tick_at(g, hold, AU_SIXTEENTH), n, steps[..]);
  sixteenth := au_grid_sample(g, AU_SIXTEENTH) - g.origin;
  for k: 0..<ns {
    st := steps[k];
    c := au_chord_of_bar(st.tick / AU_BAR);
    note := [1] int.[c.root - 12 + st.value];
    held := sixteenth * 85 / 100;
    e := au_adsr(au_ms(4, rate), au_ms(50, rate), AU_HALF, au_ms(30, rate));
    m := au_adsr_length(e, held);
    z = au_saws(note[..], two[..], rate, m, z, voice);
    au_shape(voice, m, e, held, 5898);
    acc := ifx st.index % 4 == 0 then AU_ONE else 45875;
    au_put(voice[0..<m], st.at, au_gain(14418, acc), ifx st.index % 2 == 1 then -400 else 400, 16384, L, R, SL, SR);
    out.voices += 1;
  }

  // 3. taiko on beats 1 and 3 from the belt, and 4 too from the orbit, with a high drum
  //    on the off-beat after 2 and 4
  beat := Pattern.{step = AU_PPQ, length = 1};
  hits: [256] Step;
  nt := au_sequence(g, beat, au_grid_tick_at(g, belt, AU_PPQ), n - rate, hits[..]);
  for k: 0..<nt {
    st := hits[k]; q := st.index % 4;
    if q == 0 || q == 2 || (st.at >= orbit && q == 3) {
      z = au_taiko(rate, 1000, z, voice);
      au_put(voice[0..<au_ms(1200, rate)], st.at, 49152, 0, AU_HALF, L, R, SL, SR);
      out.voices += 1;
    }
    if st.at >= orbit && st.index % 2 == 1 {
      z = au_taiko(rate, 1600, z, voice);
      au_put(voice[0..<au_ms(1200, rate)], au_grid_sample(g, st.tick + AU_EIGHTH), 22938, 500, AU_HALF, L, R, SL, SR);
      out.voices += 1;
    }
  }
```

After the parts, the score adds the reverb at a wet level of 0.18 and runs
the master over this gain curve: 0.16 at the start, 0.35 at the hold, 0.55 at
the belt, 0.75 at the orbit, and 1.0 at the home, held until one second
before the end, then a fade to 0.

## Render the score

The tool renders a score to a WAV file. It never plays the file.

```shell
coax/web/tool.sh aethyr/tools/score.coax promo-epic OUT.wav
```

```text
promo-epic: 777924 frames at 44100 Hz, 165 voices, peak 227459 (Q16) before normalising
samples fnv1a64 13636686646915279186
wrote 3111740 bytes to OUT.wav
```

Each section time is a flag: `--seconds`, `--downbeat`, `--hold`, `--belt`,
`--orbit`, `--home`, `--riser`, and `--fade`. The tool also takes `--rate`
(8,000 to 48,000) and `--seed`. For example, the 21.64-second cut of the video
uses `--seconds 21.64 --fade 2` and gives 954,324 frames and the hash
4515052046525699459.

These numbers were measured on 2026-10-31 on the development Mac (Apple
silicon, load average about 44):

- Two renders of the default score gave identical files.
- One render took 0.43 to 0.52 s of wall time and 0.16 to 0.17 s of CPU time.
- The loudness, as RMS in 16-bit steps for each second of the left channel,
  rises from 236 in the first second to 9,676 at the home, then fades to
  1,793 in the last second.

## Run it

The example [`aethyr/examples/promo-epic.coax`](../examples/promo-epic.coax)
renders two short cuts. The first is the cut that `epic.coax`'s check
renders in the compiler's evaluator: 3 seconds at 1,000 Hz. The second is
6 seconds at 8,000 Hz.

<!-- source: aethyr/examples/promo-epic.coax example -->
```jai
// Two cuts of promo-epic: the 1 kHz one the checks render (its hash is the one
// epic.coax records), and a six-second one at 8 kHz. The full bed is the score tool's.
cut :: (rate: int, unit: f64) {
  l: [48000] int; r: [48000] int; sl: [48000] int; sr: [48000] int; wet: [48000] int;
  voice: [28000] int; pcm: [96000] int;
  s := au_epic_cut(rate, unit);
  a := au_render_epic(s, l[..], r[..], sl[..], sr[..], wet[..], voice[..], pcm[..]);
  samples := pcm[0..<2 * a.frames];
  top := 0; loud := 0;
  for x: samples { top = au_max(top, au_abs(x)); if au_abs(x) > 16000 { loud += 1; } }
  print("cut at % Hz: % frames, % voices, peak % before normalising\n", rate, a.frames, a.voices, a.peak);
  print("  largest sample % of 32767, % samples past half scale, fnv1a64 %\n", top, loud, wav_hash(samples));
}
main :: () {
  cut(1000, 0.5);
  cut(8000, 1.0);
}
```

```shell
coax/bin/coax run aethyr/examples/promo-epic.coax
```

```text
cut at 1000 Hz: 3000 frames, 33 voices, peak 102646 before normalising
  largest sample 31325 of 32767, 192 samples past half scale, fnv1a64 17619107288867054281
cut at 8000 Hz: 48000 frames, 60 voices, peak 173263 before normalising
  largest sample 31325 of 32767, 2344 samples past half scale, fnv1a64 6830452351595467604
```

The first hash, 17619107288867054281, is the hash that the check
`the cut renders inside 16 bits, on its recorded hash` asserts. So the same
cut gives the same samples in three places:

- the compiler's evaluator, in `coax test`;
- a native debug build, with checked arithmetic;
- a native release build, with wrapping arithmetic and `-O2`.

The checks gave the same results with Coax 0.44.0 and 0.45.0.

## The checks

- `score.coax`: buses, the reverb return, automation, and the master. They
  check pan and send levels, linear automation held outside its points, a
  master that normalizes to the peak of either channel, never clips, and gives
  the same output on both sides for the same input.
- `wav.coax`: the header bytes, little-endian samples, refusals, and the
  FNV-1a hash against a known value.
- `epic.coax`: seconds to samples, the promo's sections on beats 8, 14, 20,
  and 28, the chords, a taiko's decay, refused buffers, and the recorded hash
  of the 1 kHz cut.

Mutation testing on `epic.coax` kills 388 of 458 mutants. Most of the 70
survivors change a gain or send level by one Q16 step, which changes no
16-bit sample in the 1 kHz cut. `epic.coax` is score data, not engine
behavior, so its survivors are recorded here rather than marked equivalent.
The engine modules, `synth.coax`, `score.coax`, and `wav.coax`, have no
survivors.

## What the port changed

The score is a port of a Python sketch that was used once to make the
video's music. The Python file isn't part of the repository. The port
differs in these ways:

- Every value is an integer, and the sine and exponentials are computed
  without `libm`.
- Patterned parts start on the grid, not at the section's exact second.
- A release falls from the envelope's level at release time, not from the
  sustain level.
- Saturation uses the Padé approximant of `tanh` instead of `tanh`.

## Dependencies

- Chapters [65](ch065-sound-from-numbers.md) and
  [66](ch066-a-sequencer-on-the-beat-grid.md).
- Coax's `coax.quantity` units for the score's times
  ([chapter 2](ch002-spaces-and-units.md) uses the same library).
- `coax/lib/sys` (`tool.coax`, `args.coax`, `fs.coax`) for the tool.
