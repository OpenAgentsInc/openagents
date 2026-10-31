# 153. Transcribe with Parakeet

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter executes the pinned `parakeet-tdt-0.6b-v3` model and turns audio
into text. The model's decoder is a token-and-duration transducer (TDT): at
each step it predicts a token and how many frames to skip. You measure
transcription quality with word and character error rates.

## What this chapter covers

- The pinned checkpoint and its tokenizer.
- The TDT decoder: the prediction network, the joint network and the
  duration outputs.
- Greedy decoding with exact rules for blanks, durations and the maximum
  symbols per frame.
- Bounded utterances: a maximum audio length and a typed refusal above it.
- Evaluation: word error rate (WER) and character error rate (CER) on a fixed
  test set, reported per language slice.
- The weights use CC BY 4.0 terms; the attribution is recorded.

## Dependencies

- [Chapter 151](ch151-waveforms-and-spectral-features.md): features.
- [Chapter 152](ch152-a-speech-encoder.md): the FastConformer encoder.
- [Chapter 52](ch052-tokenizers-and-chat-templates.md): tokenizer families.

## Done means

- Token sequences for fixed utterances match the reference decoder.
- WER and CER are reported on a fixed, versioned test set per language slice,
  with the set's identity in the result.
- The chapter states that offline transcription does not establish streaming
  latency; chapter 154 measures that.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry and the
  Granary dataset note.
