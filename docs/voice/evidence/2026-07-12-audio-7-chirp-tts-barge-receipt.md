# AUDIO-7 Chirp 3 HD and barge-in receipt

- Issue: #8740
- Date: 2026-07-12
- Deployed service/revision: `openagents-audio-staging-00009-r9n`
- Region/project: `us-central1` / `openagentsgemini`
- Voice: `en-US-Chirp3-HD-Sulafat`
- Provider: Google bidirectional streaming Text-to-Speech, Chirp 3 HD
- Media seam: signed PCM 16-bit little-endian, 24 kHz, mono
- Desktop test device: Christopher's MacBook Pro, macOS 26.4, built-in
  MacBook Pro Speakers
- Test network: the owner's Chicago connection to private Cloud Run in
  `us-central1`

## Real synthesis

The authenticated `/v1/speak` route received canonical assistant text and
streamed identity/turn/speech-bound media to the existing authenticated
WebSocket. The receipt contains no text:

```json
{"schema":"openagents.audio.tts_live_smoke.v1","ok":true,"assistantText":true,"mediaFrames":10,"adapterRef":"google-chirp3-hd-streaming","voiceRef":"en-US-Chirp3-HD-Sulafat","charsIn":36,"synthTtfbMs":198,"totalMs":465,"bytesOut":104494,"chunksOut":10,"outcome":"completed","transcriptLogged":false}
```

Canonical visible text was emitted before synthesis and remained independent
of audio success. Every media frame carried the exact voice identity,
generation, assistant turn, and speech ref.

## Qualified barge-in

Five private deployed runs synthesized a deliberately long reply while a
locally synthesized non-sensitive stop utterance streamed through real Google
STT. Qualification starts at the first interim/final containing at least three
non-whitespace characters after a Google speech-begin event. Backchannel/noise
below that threshold does not cancel.

Observed qualifier-to-server-interrupt-ACK milliseconds:

```text
0, 0, 0, 0, 2
p95 = 2 ms (target <= 500 ms)
```

Every receipt was speech-ref bound, carried an outcome ref, had transcript
logging disabled, and caused a `playback_cancel` frame. The Rust helper test
fills and flushes the full two-second bounded output queue twenty times on the
named device; p95 is below the 750 ms audible-stop target. Wrong generation,
identity, and stale speech refs cannot enqueue or cancel newer playback.

## Built Electron

The real Electron smoke reports the independent playback state and the
barge-in outcome while preserving all text controls:

```json
{"ok":true,"truth":["Mic capturing","Audio sending","Not retained","Playback off"],"registeredFocus":true,"muted":true,"bargeInOutcome":true,"stopped":true}
```
