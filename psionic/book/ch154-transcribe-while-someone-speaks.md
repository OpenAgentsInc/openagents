# 154. Transcribe while someone speaks

> **Status: unimplemented — design draft.** Nothing here runs yet.

A voice interface must transcribe speech while the person is still talking.
This chapter takes chapter 153's offline transcription and makes it
streaming: audio arrives in chunks, the encoder and decoder carry their state
between chunks, and the engine reports partial and final hypotheses within a
latency budget.

## What this chapter covers

- Chunked streaming input with a fixed chunk size and look-ahead.
- Encoder and decoder state carried between chunks, with a defined reset at
  the end of an utterance.
- Partial hypotheses that can change, and final hypotheses that cannot.
- Per-chunk latency measured against a stated budget on a named host.
- Candidate models: Parakeet, and Voxtral-Mini-4B-Realtime (Apache-2.0).
- Integration with the voice engine through `verse/platform/voice.h`: end of
  turn and barge-in.

## Dependencies

- [Chapter 153](ch153-transcribe-with-parakeet.md): offline transcription
  and its error rates.
- The voice engine in [`verse/platform/voice.h`](../../verse/platform/voice.h).

## Done means

- Final transcripts from streaming match the offline transcript for the same
  audio, or the difference is reported as a change in WER.
- Per-chunk latency is reported at P50 and P99 for the named host and chunk
  size.
- End of turn and barge-in events reach the voice engine in the order the
  engine defines.
- The chapter does not change the voice engine's consent or floor rules.

## Sources

- [Chapter plan](PLAN.md), Part XV.
