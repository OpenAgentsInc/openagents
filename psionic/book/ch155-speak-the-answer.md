# 155. Speak the answer

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter turns text into speech with one pinned text-to-speech
checkpoint. It streams audio frames into the voice engine's playback path
and measures whether the speech is intelligible and fast enough.

## What this chapter covers

- Target selection first: the checkpoint still needs a survey entry with its
  weights, code and data terms before you start. Voxtral-4B-TTS is a
  candidate, but its CC BY-NC 4.0 weights exclude commercial use.
- Text normalization: numbers, dates, abbreviations and symbols written out
  as words, with fixed rules.
- The acoustic model that predicts spectral frames or audio tokens.
- The vocoder that turns them into a waveform.
- Streamed audio frames into the voice engine's playback path.

## Dependencies

- [Chapter 151](ch151-waveforms-and-spectral-features.md): spectral
  features.
- [Chapter 153](ch153-transcribe-with-parakeet.md): transcription, for the
  intelligibility round trip.
- [Chapter 154](ch154-transcribe-while-someone-speaks.md): integration with
  the voice engine.

## Done means

- A pinned target with recorded terms exists before implementation.
- Outputs for fixed text match recorded reference audio or frames within a
  stated tolerance.
- Intelligibility is measured as the WER of a chapter 153 transcription of
  the generated speech on a fixed text set.
- Time to first audio frame and real-time factor are reported on a named
  host.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry (TTS is
  not yet surveyed).
