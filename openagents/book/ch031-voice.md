# 31. Voice

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. The voice engine it builds on runs today; the agent's voice
> is the plan.

## Premise

You can type to the agent on every surface. This chapter lets you talk to
it: dictation into the composer, and a spoken turn that the agent answers
aloud, using the voice engine the repository already has.

## Draft

The voice engine is Coax
([`metaverse/voice.coax`](../../metaverse/voice.coax)): resampling to 16 kHz,
levels, the noise floor, voice activity, end of turn, barge-in and the jitter
buffer. Consent and the input floor are in
[`metaverse/voice-floor.coax`](../../metaverse/voice-floor.coax), and the
meter is [`pay/voice-meter.coax`](../../pay/voice-meter.coax). Dictation for
the native apps is shared in
[`apps/shared/dictation.coax`](../../apps/shared/dictation.coax). The iPhone
app already has a voice bar and session
([`apps/openagents-ios/OpenAgents/Voice`](../../apps/openagents-ios/OpenAgents/Voice)).

This chapter uses all of it and writes no second engine. Dictation turns
speech into text in the composer, and you send it as usual. A spoken turn
goes a step further: the microphone stays open, the engine finds the end of
your turn, and the reply is read back. Speaking while it reads stops it,
which is barge-in.

The microphone is a device service with consent. It opens only after you
allow it, a visible indicator shows while it listens, and it closes when you
leave the screen, background the app or log out (chapter 36).

Tests play recorded audio through the engine and compare the decisions it
makes, frame by frame. The real microphone, audio routes and interruptions,
such as a call arriving, are checked on a device.

## What we build

A plan for what this chapter ships once the draft is approved:

- Dictation into the composer, on the iPhone and the web.
- A spoken turn: listen, detect the end of turn, send, read the reply, and
  stop on barge-in.
- The meter's charge shown before a spoken turn starts, when one applies.

It builds on what exists today:

- [`metaverse/voice.coax`](../../metaverse/voice.coax)
- [`metaverse/voice-floor.coax`](../../metaverse/voice-floor.coax)
- [`pay/voice-meter.coax`](../../pay/voice-meter.coax)
- [`apps/shared/dictation.coax`](../../apps/shared/dictation.coax)
- [`apps/openagents-ios/OpenAgents/Voice`](../../apps/openagents-ios/OpenAgents/Voice)

### The chapter's check

- **Source:** the voice actions in the shared app, over the existing engine.
- **Trace:** a recorded WAV through the engine, printing each frame decision
  and the text that reaches the composer.
- **Refusal:** with consent withdrawn, a spoken turn does not open the
  microphone and prints the refusal.
- **Independent check:** `metaverse/test_voice.sh` against the same recording.

Planned paths: none new. Voice actions go in
[`apps/shared`](../../apps/shared).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): talking to the agent
- **Also:**
  - [Coder voice from hex](../../docs/voice/2026-11-02-coder-voice-from-hex.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Open questions for the owner

1. Which voice reads replies aloud by default, and may a person choose
   another?
