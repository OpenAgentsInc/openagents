# Aethyr: live audio against offline synthesis (#12628)

Date: 2026-11-03. Machine: coderos-4080, headless and muted Chromium 152
(`cdp.coax` launches it with `--headless=new --mute-audio`). Nothing played
on the speakers. Check: `aethyr/tests/live-audio.coax` against the Grid page
served from this checkout (`coax/web/build/serve apps/openagents-web --static`).

```sh
coax/web/tool.sh aethyr/tests/live-audio.coax http://127.0.0.1:PORT/realms/the-grid/
```

## What runs live

A realm's sound is a decoded audio file. The realm returns lines of
`NAME GAIN PAN`; `coax/web/host.js` decodes the sound with `decodeAudioData`
and plays it as `AudioBufferSourceNode → GainNode → StereoPannerNode →`
the speakers, at most 16 at a time. Aethyr's synthesizer (chapters 65–67)
doesn't run on the audio clock. So the live path qualified here is the
synthesizer's offline render, played through the host's own graph. Real-time
rendering in blocks (chapter 68's `stream.coax` and worklet) doesn't exist yet
and isn't qualified.

The host now builds each voice with one function, `soundVoice`, which the live
path and the check both call. In a dev page, a check can end the live graph in
a tap (`globalThis.coaxialAudioTap`) and reach the host's path through
`coaxial.audio`.

## The declared input

`promo-epic` (aethyr/audio/epic.coax), squeezed into 1.0 s (downbeat 0.05,
hold 0.25, belt 0.45, orbit 0.65, home 0.85, riser 0.3, fade 0.1), seed 11,
rendered at the live context's rate: 48,000 frames at 48,000 Hz, 13 voices,
samples FNV-1a 64 `10648954814662572121`. The check writes that render as a
16-bit stereo WAV and adds it to the page as a realm sound.

## Results

Tolerance: 2^-20 absolute, 1/32 of a 16-bit step, against the Web Audio formula
applied to the offline samples in float64. NaN fails.

| Check | Result |
| --- | --- |
| Decode | All 96,000 samples within tolerance (max 0.0010 of a step); 45,231 of 48,000 frames are the float32 nearest the rule |
| Offline graph, gain 1, pan 0 | 96,512 samples, max error 0.0010 of a step, 0 past tolerance |
| Offline graph, gain 0.5, pan −0.4 | max 0.0016 of a step, 0 past |
| Offline graph, gain 0.8, pan 0.7 | max 0.0036 of a step, 0 past |
| Offline graph, gain 0.25, pan −1 | max 0.0009 of a step, 0 past |
| Live, gain 0.5, pan −0.4 | 563 quanta (72,064 frames), 0 gaps; 144,128 samples, max 0.0016 of a step, 0 past; silence before and after the voice |
| Voices | 20 lines plus one unknown name hold 16 voices; all released at their end |

The offline graph rows are the host's `soundVoice` rendered in an
`OfflineAudioContext`, with 256 frames of silence checked after the voice. The
live row is the realm's line played through the host's own path into its
running `AudioContext`. An `AudioWorklet` tap at the end of the live graph
records it, and the comparison starts at the voice's first frame. The live row
has the same maximum error as the offline row with the same gain and pan.

**The decode rule.** Chromium doesn't decode a 16-bit step as step / 32768.
A probe WAV gave 16384 → 16384.5 / 32768, 32767 → 1.0, −32768 → −1.0, and
100 → 100.003 / 32768: a negative step is divided by 32768 and a positive
one by 32767. The check models that rule (`la_decode`). Under step / 32768,
the same render missed by up to 0.88 of a step. Another browser's decoder
may use a different rule, so the profile is Chromium's.

**Latency.**

- The voice starts on the frame the context reports when the host's play
  call is made (0 frames after `currentTime × rate`).
- The context reports a base latency of 10,667 µs and an output latency of
  32,000 µs, about 42.7 ms together between the graph and the device, as the
  browser reports them (muted, headless; a real device's figures differ).
- The first decode of the 1 s sound took 1.7 ms. A sound that isn't decoded
  yet starts when its decode finishes, not on the frame it was asked for.

**Buffers.** The tap saw every 128-frame quantum in order, with no frame
skipped across 1.5 s. Device-level underruns aren't visible to the graph, so
this doesn't show the device never glitched.

## A fix this found

The 16-voice cap counted a voice only after its sound decoded. So a burst of
lines in one frame, or any line for a sound still decoding, could pass 16.
`playSoundText` now takes a voice's slot when it reads the line and gives the
slot back if the decode fails. The burst row above is that fix's check.

## The profile, and what is outside it

- Qualified: Chromium's decode of 16-bit PCM WAV at the context's own rate,
  then the host's gain and equal-power stereo pan, offline and live, within
  2^-20 of the formula on Aethyr's offline samples.
- Outside: a sound at another rate (the browser resamples it, and its
  resampler isn't specified), compressed sounds (the realms' MP3s: the
  decoder decides the samples), the loop asset's gain node, other browsers,
  and the iOS Haunted House's `AVAudioPlayer` sounds. The Linux native host
  plays no sound.
