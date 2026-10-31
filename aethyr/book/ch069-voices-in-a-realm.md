# 69. Voices in a realm

> **Status: unimplemented — design draft.** Today the voice engine
> [`verse/voice.coax`](../../verse/voice.coax) decides turns, barge-in and
> playback for talking to an agent, and the C voice host in
> [`verse/voice-host/`](../../verse/voice-host/host.c) carries its frames.
> Players can't hear each other yet: the forwarder and `voice-space.coax`
> are phase V5 of the [voice plan](../../docs/voice/coax-native-voice.md),
> not built. This chapter plans voices inside a realm: players, NPCs and
> agents heard from where they stand. Nothing in `aethyr/` runs yet.

A realm with several people in it needs voice. You hear the person beside
you clearly and the person across the square faintly, from the side they
stand on. An NPC's line comes from the NPC. Who may be heard is decided by
consent and explicit grants, never by the words spoken.

## What this chapter teaches

- How one set of voice primitives (`Speaker`, `Utterance`, `Route`) serves
  players, NPCs, agents and narrators.
- How a server decides who hears whom from replicated positions, without
  decoding or mixing audio.
- How each client places the voices it receives with chapter 68's spatial
  sound.
- How consent, floors and media epochs keep the wrong audio from playing.

## Outline

1. **One speaker model.** A `Speaker` is a player key, an NPC ID or an
   agent ID with a role (`SPEAKER_ROLE_NPC = 2`, `SPEAKER_ROLE_AGENT = 3`,
   `SPEAKER_ROLE_NARRATOR = 4`), a voice profile and the world entity its
   voice comes from (chapter 6's stable `EntityId`), or none for a flat
   narrator. An `Utterance` carries text up to 600 bytes, a priority, and
   `interruptible`, `spatial` and `batch` flags (`WUtterance` in
   [`verse/voice-wire.coax`](../../verse/voice-wire.coax)).
2. **Frames and turns.** Uplink audio arrives in 20 ms frames (`FRAME`, 320
   samples at 16 kHz; `MAX_IN` 960 at 48 kHz). The turn machine moves
   through `TURN_IDLE`, `TURN_SPEAKING`, `TURN_TRAILING` and `TURN_ENDED`,
   with a 640 ms pre-roll (`PREROLL`, 32 frames) and an onset that confirms
   within 500 ms (`ONSET_SPAN`). Every decision passes one gate: clamps,
   epoch, deadline and a confidence floor.
3. **Who may be heard.** [`verse/voice-floor.coax`](../../verse/voice-floor.coax)
   holds up to 8 slots (`SLOTS`), each with consent and a route target, one
   input-floor holder and a media epoch. Withdrawing consent unbinds the
   slot and takes back its floor. A new epoch fences audio from before a
   reconnect.
4. **The forwarder.** Each client sends one stream up. The server sends
   each listener the streams within the realm's hearing radius (64 m
   today, the presence radius of [chapter 49](ch049-rooms-and-presence.md))
   and inside any `SpatialAudioZone {volume, live_session, falloff_m}`. The
   server does not decode or mix. The choice is a Coax function of
   positions and zones, so it replays.
5. **Placing a voice.** The client gives each voice a gain, pan and delay
   from the listener's lens, as [chapter 68](ch068-sound-in-the-world.md)
   does for sound sources. The downlink jitter buffer holds 16 packets
   (`JSLOTS`) of up to 200 ms (`JMAX`) and fades a missing packet to
   silence over 640 samples (`FADE`). Playback states are `PLAY_SILENT`,
   `PLAY_PLAYING`, `PLAY_DUCKED` and `PLAY_PAUSED`.
6. **NPC and agent lines.** An NPC's `Voice` in the realm document (up to
   16 per `verse.doc.v1`) gains a `VoiceProfile`. Lines that repeat render
   as cached batch jobs (phase V6) and play as spatial sources at the NPC.
7. **Cost and limits.** [`verse/voice-meter.coax`](../../verse/voice-meter.coax)
   gives each account a free tier per day (`FREE_MS` 600,000 ms recognized,
   `FREE_CHARS` 5,000 characters synthesized) and prices the rest in
   millisatoshis. Player-to-player voice uses no speech models, so it costs
   only bandwidth (Opus at 24 kbps is planned before player voice).
8. **Privacy as stated.** The host keeps no audio after a turn. TLS to the
   server is not end-to-end between players, and the realm's UI says so.

## What exists today

- [verse/voice.coax](../../verse/voice.coax): resampler, levels, voice
  activity, end of turn, barge-in and the jitter buffer, checked on
  recorded fixtures.
- [verse/voice-floor.coax](../../verse/voice-floor.coax),
  [verse/voice-wire.coax](../../verse/voice-wire.coax) and
  [verse/voice-meter.coax](../../verse/voice-meter.coax): consent and
  floors, the `openagents.voice.v1alpha1.Frame` wire (16 KiB frames), and
  metering.
- [verse/voice-host/](../../verse/voice-host/host.c): the C host with
  WebSocket, speech and WAV adapters, run locally on a Mac.
- [verse/haunt.coax](../../verse/haunt.coax): a bounded, symmetric gain and
  pan per sound from the eye and heading (`hh_gain`, `hh_pan`).
- [Coax chapter 25](../../coax/book/ch25-case-study-voice.md): how the voice
  engine is built from small checked policies.

## What it builds

- `aethyr/audio/voice-space.coax`: a voice's gain, pan and delay from the
  listener's lens and the speaker's entity.
- `aethyr/net/voice-route.coax`: who hears whom, from positions, zones,
  consent and floors.
- The realm document's `VoiceProfile` field on NPC voices.

## Acceptance checks and oracles

The chapter is done when:

- Two headless sessions in one realm hear each other, placed by side and
  distance, and the mouth-to-ear time is measured against the 250 ms target.
- A listener outside the hearing radius and outside every zone receives no
  stream from that speaker.
- A slot without consent is never routed or heard, checked with
  `coax mutate` on the routing function with no surviving mutants.
- Audio stamped with an old media epoch never plays after a reconnect.
- The routing decisions of a recorded session replay to the same result
  ([chapter 5](ch005-replay-a-session-exactly.md)).
- A phone in a full room keeps 60 FPS with voice on.

## Dependencies

Chapters 2 (spaces), 5 (replay), 6 (stable entity IDs), 10 (the lens the
listener hears from), 49 (rooms and presence) and 68 (spatial sound). From
Coax: [chapter 18](../../coax/book/ch18-protocol.md) for the protocol
boundary and [chapter 25](../../coax/book/ch25-case-study-voice.md) for the
voice engine.
