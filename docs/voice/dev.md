# Voice: the local dev environment

How to run the Coax-native voice stack on a Mac: the voice host, the dev
client, and the tests. The design is [coax-native-voice.md](coax-native-voice.md);
the wire is `proto/openagents/voice/v1alpha1/voice.proto`.

## What's here

| Path | What it is |
| --- | --- |
| `verse/voice*.coax` | The voice engine: VAD, end of turn, barge-in, frame features and decisions, the wire codec, floor and meter authorities |
| `verse/platform/voice.h`, `voice.c` | The plain-C face of the engine, shared by the voice host and the iOS app |
| `verse/voice-host/host.c` | The voice host: WebSocket server, sessions, provider calls |
| `verse/voice-host/speech.c` | Speech providers (`google`, `local`), agents (`gemini`, `echo`) and the per-route agent instructions |
| `verse/voice-host/voice-reply.coax` | `#authority`: when a partial may stand as the final transcript, when to recognize early, where a streamed reply is cut into sentences |
| `verse/voice-host/ws.c`, `wav.c` | RFC 6455 WebSocket framing and handshake; WAV reading and writing |
| `verse/voice-host/client.c` | `voice-client`, the dev test client |
| `verse/voice-host/fixtures/` | Three 16 kHz mono speech fixtures, made once with Google TTS |
| `verse/voice-host/build.sh`, `run.sh`, `test.sh` | Build, run, test |

## Build, run, test

```sh
verse/voice-host/build.sh                 # -> verse/voice-host/build/voice-host, voice-client
verse/voice-host/run.sh                   # Google speech + Gemini on 127.0.0.1:8787
verse/voice-host/run.sh --offline         # local speech + echo agent, no network
verse/voice-host/test.sh                  # offline cases, then the live Google cases
verse/voice-host/test.sh --offline        # offline cases only
```

Talk to a running host with a fixture:

```sh
verse/voice-host/build/voice-client --out /tmp/reply.wav verse/voice-host/fixtures/hello-foreman.wav
afplay /tmp/reply.wav
```

`build.sh` uses only what's on the Mac: it builds `coax/bin/coax` if it is
missing or stale (the same way `apps/openagents-ios/generate.sh` does),
compiles each `verse/voice*.coax` and `verse/voice-host/*.coax` to a C library with
`coax c FILE --library --header` (`voice.coax` becomes `VoiceEngine.c/.h`,
because `Voice.h` would match `voice.h` on a case-insensitive disk; the others
are `VoiceWire`, `VoiceFloor`, `VoiceMeter`, `VoiceReply`), compiles `verse/platform/voice.c` over
them, and links the host against libcurl from the macOS SDK. `#authority`
files run their checks inside `coax c`, so a failing check stops the build.
Outputs go to `verse/voice-host/build/` (ignored by git).

## Credentials

The `google` provider and the `gemini` agent use an access token from the
automation service account:
`CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config gcloud auth print-access-token`
(override the config directory with `OA_VOICE_GCLOUD_CONFIG`). The host runs
that command itself, caches the token, refreshes it on a 401, and never
prints or logs it. A warm-up refreshes a token older than 30 minutes off
the turn path; a call refreshes one older than 45 minutes. Calls are billed to project
`openagentsgemini` (`--project` to change). If no token can be had, the host
refuses to start with `google` or `gemini` and says to use
`--provider local --agent echo`.

## The voice host

```
voice-host [--listen HOST:PORT] [--provider google|local] [--agent gemini|echo]
           [--model NAME] [--project ID] [--budget-msat N] [--verbose]
```

Defaults: `--listen 127.0.0.1:8787 --provider google --agent gemini
--model gemini-2.5-flash`. `--listen 127.0.0.1:0` picks a free port; the host
prints `listening on 127.0.0.1:PORT ...` on stdout when it is ready. The iOS
simulator reaches the Mac's `127.0.0.1:8787` directly. `GET /health` answers
`ok` over plain HTTP.

A real iPhone has no default voice host yet (none is deployed). On the same
Wi-Fi, run the host on every interface and point the phone at the Mac:

```sh
verse/voice-host/run.sh --listen 0.0.0.0:8787
```

On the phone, enter the Grid, long-press the microphone at the top left and enter
`ws://YOUR-MAC.local:8787` (the name from System Settings > General > Sharing),
then tap the microphone. iOS asks once for local network access. The app
allows plain `ws://` only to local-network names (`NSAllowsLocalNetworking`).
The host doesn't check `Hello.token` yet, so anyone on that network can use it
(and spend its Google quota) while it listens on `0.0.0.0`.

One process. The main thread owns every socket (`poll`) and every engine
call; eight worker threads make the provider HTTP calls and hand results back
through a pipe, so a slow Google call never blocks audio for any session.
Each session has its own `oa_voice` engine and floor (`oa_voice_floor`).
The meter (`oa_voice_meter`) is per speaker id, shared by that speaker's
sessions, and kept in memory only: restarting the host resets it. Before
any audio goes to recognition and before any text goes to synthesis,
`oa_voice_meter_may_run` decides whether it fits; then `oa_voice_meter_charge`
records it. Every recognized millisecond sent to the provider counts (partials
included, since each one re-sends the turn so far), and so does every
synthesized character. `--budget-msat` sets the spend allowed beyond the free tier
(default 0: free tier only).

Logs go to stderr: one line per session event, with transcript and reply
lengths only. `--verbose` adds the text and every applied or dropped decision.

### Providers and agents

| Name | What it does |
| --- | --- |
| `google` speech | Speech-to-Text v2 `recognize`, model `chirp_3`, on the turn so far (see "The final transcript" below). Text-to-Speech Chirp 3 HD (`LINEAR16` at the session's output rate), one request per reply sentence |
| `local` speech | Offline and deterministic. Recognition returns "I heard N milliseconds of speech. Thank you." for the audio's length; synthesis returns a rising chirp, 50 ms per byte of text plus 200 ms (at most 12 s) |
| `gemini` agent | Vertex AI `streamGenerateContent?alt=sse` in us-central1 with the route's instruction, and the session's last 10 exchanges as context |
| `echo` agent | Replies "You said: TRANSCRIPT" (two sentences with `local` speech, so the sentence path runs offline) |

The agent's instruction comes from `Hello.route` through one table,
`ROUTES` in `speech.c`. Its rows: `haunted-house` (the house's VOICE), `everglade` (its voice edits), `guide` (the Agent orb on the /verse landing, the playtest's guide: what the playtest, the Verse and the Haunted House are for), and `foreman`: the Foreman, an
agent standing by the start of the Grid in OpenAgents' Verse (a
world built in Coax), who speaks at most 30 words of plain spoken English,
says what it doesn't know instead of inventing projects or facts, and uses
the conversation so far. A Hello with a route that has no row gets
`Error PROTOCOL "unknown route"`; another NPC is a new row.

All HTTP goes through one shared libcurl connection cache (plus DNS and TLS
session caches), HTTP/2 where Google offers it, with one reused handle per
worker thread. When a session's `TURN_START` arrives (at most once every
20 s for the whole host), and once at startup, a warm-up job refreshes an
aging token and opens connections to the Speech-to-Text, Text-to-Speech and
Vertex AI hosts, so the turn's calls skip DNS, TCP and TLS. With
`--verbose` every call logs whether its connection was new or reused.

Voice profiles are a small table in `speech.c`: `foreman` (default,
`en-US-Chirp3-HD-Charon`), `kore`, `puck`, each at revision 1. Google
renders are cached under `~/.cache/openagents-voice/renders/`, named by the
SHA-256 of the profile id, revision and text (the `Render.key` of the proto)
plus the sample rate, so a line is synthesized once per voice and rate.

Recognition is over REST. Google's streaming recognition is gRPC-only; it
is not built yet. `JOB_RECOGNIZE`/`JOB_FINAL`/`JOB_TAIL` in `host.c` and
`speech_recognize` are where a streaming recognizer would replace them.

### A session

1. The client opens a WebSocket and sends `Hello` (protocol 1, input and
   output rates, profile, `observe`). Input rates 16000, 24000, 44100, 48000;
   output rates 16000, 24000, 48000; PCM s16le only. `Hello.token` is not
   checked yet. The host answers `Welcome` (session id, provider, the meter,
   `max_turn_ms`). Opening the session records the speaker's consent on the
   floor, binds its turns to `Hello.route` (default `foreman`) and grants it
   the floor.
2. The client's engine decides turns. It sends `Control TURN_START`, then
   `Audio` frames for that turn at the Hello's input rate, then
   `Control TURN_END`. Before a turn starts, `voice-meter.coax` decides
   (`oa_voice_meter_may_run`) whether it may run; if not the host sends
   `Error LIMIT`. `voice-floor.coax` decides whether the speaker may be heard.
   The host answers `TurnEvent STARTED` and a `Route` (reason `default`).
3. The host runs every received uplink frame through its own
   `oa_voice_uplink` too, so it has `FrameFeatures` for each 20 ms frame. A
   frame the engine marks `OA_EV_DROPPED` (a DROP decision) is left out of
   recognition. Turn boundaries stay the client's.
4. While the turn is open the turn so far is re-recognized every ~1 s of new
   audio, and early: as soon as `voice-reply.coax` sees 160 ms of silence
   after the last frame the host's engine heard speech in (up to three of
   these in flight at once). Each result with words is sent as
   `TurnEvent PARTIAL`. At `TURN_END` the final transcript is decided (below)
   and sent as `TurnEvent FINAL`, then a `Meter`. An empty transcript ends the
   turn (`TurnEvent ENDED`).
5. The transcript goes to the floor's bound target (never chosen from the
   words) and the agent streams its reply. Each sentence is charged to the
   meter and synthesized as soon as it is complete, while the rest still
   arrives; renders finish in any order and are joined in sentence order into
   one Utterance. When the first sentence's audio is ready the host sends
   `Speech` (utterance, speaker, rate, epoch, the text so far), then `Audio`
   in chunks of at most 200 ms (at most 9600 bytes) at the output rate,
   sent up to 400 ms ahead of the client's playback clock, each passed
   through the host's `oa_voice_downlink` first (so gain, duck and pause
   decisions apply). Each later sentence resends `Speech` with the same
   utterance id and the full text so far, so a caption shows the whole
   reply; audio continues the same stream. When the last sample has played
   by the host's clock: `Control SPEECH_END`, then `TurnEvent ENDED`. The
   host logs any playback gap (audio not ready in time). Replies queue if one
   is already playing.
   **Facing a character.** A page in a conversation (the Haunted House
   talking to Mortimer) sends a client `Route` whose `target` is that
   character and `reason` is `facing`, by its own binding, never from the
   words; an empty target faces nobody again. While a character is faced the
   host sends `TurnEvent FINAL` and stops there: the page asks the character
   (the same `/api/playtest/npc/ask` as a typed question) and sends a client
   `Speech` with `turn` and `utterance.text` (at most 600 bytes), which the
   host cuts into sentences, meters, renders and speaks exactly as an agent
   reply, its `Speech.speaker` the character. A `Speech` for any other turn
   is a `PROTOCOL` error.
6. `Control BARGE_IN` stops reply audio at once, cuts the engine's epoch
   (adopting the client's epoch if it is ahead), bumps the floor's epoch,
   stops the streaming agent call, discards replies (and sentences still
   rendering) for earlier turns, and sends
   `TurnEvent INTERRUPTED` with the client's `played_samples`. `CANCEL` does
   the same and also drops the open turn. `PING` gets `PONG` with the same
   `at_ms`. `PLAYBACK` records `played_samples`.
7. Errors: `PROTOCOL` (malformed or out-of-order frames; a bad first frame
   closes the connection), `LIMIT`, `UNAUTHORIZED` (the floor refused),
   `PROVIDER` (a Google call failed, with a short reason), `TOO_LONG`,
   `INTERNAL`.

### Observing and steering frames

With `Hello.observe`, the host sends that client a `FrameFeatures` for every
uplink frame and every downlink chunk it processes, and a `FrameDecision`
whenever its built-in policy (`voice.coax`) produces one.

A client may send `FrameDecision` frames; the host applies each to that
session's engine with `oa_voice_decide`, which drops late, stale-epoch and
low-confidence decisions and clamps every field. Downlink decisions (gain,
duck, pause, resume) change the reply audio the host sends; an uplink DROP
keeps frames out of recognition. This is the seam for an external decider.
Not built yet: attaching an observer to another client's session (the
proto has no field for it), and relaying decisions to the speaking client's
engine (where uplink END_TURN/HOLD_TURN would matter, since the client
decides turns).

### The final transcript

`verse_voice_reply_final_plan` in `voice-reply.coax` decides at
`TURN_END`, from the turn's numbers only:

- **partial**: a finished recognition with words covers every voiced frame
  plus 100 ms. Its text is the FINAL; nothing more is recognized. This is
  the usual case, because the early recognition started 160 ms into the
  trailing silence and the client's engine ends the turn after 700 ms.
- **wait**: a recognition still in flight will cover that; the host waits
  for it and decides again.
- **tail**: the finished recognition's cut sat in at least 200 ms of
  silence (so no word straddles it); only the audio after the cut is
  recognized, and its text is appended.
- **whole**: anything else; the whole turn is recognized, as before.

A partial is never used when its audio ended before the last voiced frame,
so nothing the speaker said is left out. The checks (`coax test`) and
mutation testing (`coax mutate`, zero survivors) are in the file.

### Sentences

`verse_voice_reply_sentence_end` decides where a streamed reply is cut:
after `.` `!` `?` followed by whitespace (or at a newline), once the
sentence is at least 8 bytes. The reply's first piece must be at least 24
bytes and may also end at `,` `;` `:`. That is short to render, but long
enough (about 1.5 s of speech) for the next piece to finish rendering
before it has played. A Chirp 3 HD render takes about 300 ms plus 130 ms per
second of audio. With an 8-byte first piece ("Hello there!") the
second sentence wasn't ready in time, playback had a gap, and a barge-in
during that gap went unnoticed by the client.

### Measured latency

Live, google + gemini, on this Mac, from the host's `TURN_END` to the first
reply audio sent, with medians of the host's `latency` log lines.
Before: 6 turns (`hello-foreman.wav` and `garden-pause.wav`, 3 each) with
per-turn REST recognition, a whole-reply `generateContent` and a
whole-reply render. After: 10 turns (5 each) with everything above.

| Step | Before | After | Target |
| --- | --- | --- | --- |
| Turn end to final transcript | 812 ms | 292 ms | 300 ms |
| Final transcript to first reply sentence (whole reply before) | 783 ms | 668 ms | (agent's own time) |
| First sentence to its rendered audio (whole reply before) | 1117 ms | 703 ms | 300 ms |
| Turn end to first reply audio | 2705 ms | 1761 ms | |

Every after-turn used the **partial** plan and had 0 ms of playback gaps.
Run-to-run spread is wide (totals 1358–2841 ms after); Gemini's time to its
first sentence and the render of a long first sentence dominate. Streaming
synthesis (gRPC-only at Google) and streaming recognition are the next
steps; neither is built. `test.sh` prints the client's and the host's
latency line for each live case.

## The dev client

```
voice-client [--url ws://127.0.0.1:8787/] [--observe] [--out reply.wav] [--expect TEXT]
             [--barge FILE.wav] [--turns N] [--timeout SECONDS] [--rate-out HZ]
             [--profile ID] [--speaker ID] [--decide-gain DB] [--quiet] FILE.wav
```

It does what the phone does: reads 20 ms frames of the WAV at real-time pace,
runs each through `oa_voice_uplink`, and sends `TURN_START` (followed by the
engine's pre-roll audio), the turn's 16 kHz audio, `TURN_END` and `BARGE_IN`
exactly when the engine raises them. After the WAV it keeps sending silence
until the expected replies (`--turns`, default 1, or 2 with `--barge`) have
ended or been interrupted. A repeated utterance id in `Speech` is counted as
a text update (`speech-update`), not a new utterance. Reply audio goes through `oa_voice_downlink` and
plays on a simulated clock from the moment it arrives, so `oa_voice_playing`
is true while it would be audible; audio from an older epoch is counted as
`stale` and not played.

- `--barge FILE` starts streaming a second WAV after 300 ms of reply
  playback, to talk over the reply.
- `--decide-gain DB` sends a downlink `FrameDecision` with that gain right
  after `Welcome`.
- `--expect TEXT` passes only if a FINAL transcript contains TEXT (case and
  punctuation ignored).
- It prints `latency final_ms=… first_audio_ms=…` (from its own TURN_END)
  for each reply, and `turn-start tN at M ms of input` for each turn.
- It prints one line per event and ends with a `summary` line
  (`finals=… partials=… speeches=… speech_updates=… speech_ends=… interrupted=… stale=…
  features=… decisions=… errors=… peak=…`). It exits nonzero on a timeout,
  no FINAL, any `Error` frame, or a failed `--expect`.

## Tests

`test.sh` builds, starts the host with `--provider local --agent echo` on a
free port, and checks:

| Case | Checks |
| --- | --- |
| `health` | `GET /health` is `ok` |
| `hello` | one FINAL containing "I heard", one Speech, downlink audio, one SPEECH_END, a Meter, no errors; the reply WAV is written |
| `pause-is-one-turn` | `garden-pause.wav` has a 350 ms pause mid-sentence; it is one turn with at least one PARTIAL |
| `observe` | more than 50 FrameFeatures arrive |
| `barge-in` | talking over the reply gives INTERRUPTED and a second FINAL |
| `decision-gain` | a -12 dB downlink decision makes the reply's peak at most half the plain run's |
| `two-sessions` | two clients at once both complete |
| `live-google` | only when a token can be had: `--provider google --agent echo`, the FINAL of `hello-foreman.wav` contains "what are we building today" |
| `live-gemini` | `--provider google --agent gemini`: the same FINAL, one Speech, one SPEECH_END, no errors |
| `live-gemini-barge` | talking over a streamed Gemini reply gives INTERRUPTED and a second FINAL |

The fixtures are Chirp 3 HD speech with silence trimmed to known lengths:
`hello-foreman.wav` ("Hello Foreman, what are we building today?"),
`garden-pause.wav` ("Let's take the long way through the garden, [350 ms]
then pick a door."), `wait-stop.wav` ("Wait, stop.").
