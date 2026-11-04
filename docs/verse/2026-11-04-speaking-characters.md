# Speaking characters for videos

Status: spec, 2026-11-04. Nothing in it is built yet.

This spec covers how you make a video in which a character speaks a scripted line. The character is a
MetaHuman rendered by Verse, with its voice made by text-to-speech and its lips synced to that voice.
The first use is the Cultist Raid promo: Everglade's mayor appears in a portrait panel over the raid
footage and says a line as the raiders reach the town. The same pipeline then serves any character,
any line and any video. You write a cue sheet and run one command, and the same inputs always give
the same video.

## What you do

1. **Cast the character once.** You add the mayor to the cast file: a name, a MetaHuman pack, an
   outfit, a voice and a framing.
2. **Write the cues.** Each cue names a time in the video, a character, the line and an optional
   mood:

   ```
   # cues/raid-promo.cues
   video  raid-video-v6-night.mp4
   00:04.0  mayor  worried  "Hey! Is that... I hope we're not under attack."
   00:11.5  mayor  urgent   "Everyone, to the square! Guard the stables!"
   ```

3. **Run it.**

   ```
   coax/web/tool.sh speak cues/raid-promo.cues ~/work/film-out/promo/raid-promo.mp4
   ```

   The tool voices every line, works out the mouth shapes, renders each talking portrait in Verse,
   and composites them over the video with their sound mixed in. A line you didn't change isn't
   voiced or rendered again, so changing one line takes about as long as that line.

4. **Watch the review copy.** The tool writes the video at 1080p and opens it. You add a cue, change
   a line or a mood, and run the command again.

The tool does text-to-speech: it turns the written line into the character's voice. Speech
recognition (speech-to-text) only checks timing. The tool transcribes its own audio to get exact word
times and to confirm that the voice said the line as written.

## The pieces

| Step | Input | Output | Where it runs |
| --- | --- | --- | --- |
| 1. Voice | line, voice, mood | WAV at 48 kHz, plus word and character times | text-to-speech API |
| 2. Align | WAV, line | timed phonemes, then visemes | local, Coax |
| 3. Perform | visemes, mood, audio level | a face and head track: controls per frame | local, Coax |
| 4. Render | track, cast entry, pack | the portrait as frames, 60 fps | Verse page, headless |
| 5. Composite | video, portraits, WAVs, cues | final MP4 | local, ffmpeg |

Every step caches its output under a key made from its inputs, with the tool's version in the key.
The cache lives in `~/.openagents/verse/private-build/speak/`, outside git. Because it holds MetaHuman
frames, it falls under the [binding rules](#rules).

### 1. Voice

- **Provider.** Each character's voice names a provider and a voice ID. Both are supported:
  - Google Cloud Text-to-Speech (Chirp 3 HD voices) through Vertex, per the Google-first rule.
  - ElevenLabs through the Scenario API, the path the launch film uses for Bella.

  You choose each character's voice by audition (see [Auditions](#auditions)), not by provider.
- **Mood.** A mood maps to the provider's settings: stability and style for ElevenLabs, or SSML rate,
  pitch and breaks for Google. It's also a tag the face reads in step 3. The moods are `calm`,
  `worried`, `urgent`, `angry`, `cheerful` and `grim`. A line can carry pauses (`...`) and
  emphasis (`*word*`), which the tool turns into the provider's own markup.
- **Timing.** The tool asks for timestamps: ElevenLabs's character-level alignment, or Google's
  SSML marks per word. It then runs Google Speech-to-Text with word time offsets on the result.
  If the transcript doesn't match the line, the tool fails with both texts, so a garbled take never
  reaches the video.
- **Determinism.** Text-to-speech isn't bit-for-bit repeatable, so the cache is what makes the output
  repeatable. A line is voiced once and kept by its key (text, voice, mood, settings). `--revoice LINE`
  asks for a new take on purpose, and the old take stays in the cache, so you can go back to it.
- **Keys.** Keys come from `~/work/.secrets/scenario.env` and the gcloud automation account. The tool
  never prints them.

### 2. Align

- The tool turns words into phonemes with a pronunciation dictionary: CMUdict, BSD-licensed, compiled
  into a Coax table. Names and made-up words, such as Odiodei and Everglade, come from a
  pronunciation list in the cast file.
- It spreads each word's phonemes across that word's time from step 1, weighted by typical phoneme
  length.
- Phonemes map to 14 visemes, the ARKit-style mouth shapes the face rig already names where it can:

  | Viseme | Sounds |
  | --- | --- |
  | `rest` | silence |
  | `mbp` | m, b, p |
  | `fv` | f, v |
  | `th` | th |
  | `dtnl` | d, t, n, l |
  | `kg` | k, g |
  | `sz` | s, z |
  | `chj` | ch, j, sh |
  | `r` | r |
  | `aa` | the open vowels, as in "father" |
  | `ee` | as in "see" |
  | `ih` | as in "bit" |
  | `oh` | as in "go" |
  | `oo` | as in "too", plus w |

- The output is a viseme track: start, end and weight for each viseme. It's plain text, so you can
  read it and correct it by hand.

### 3. Perform

The performance module (`aethyr/speak.coax`) turns the viseme track into face-rig controls frame by
frame:

- **Mouth.** Each viseme is a set of raw controls (jaw, lips and tongue), blended with
  coarticulation: each shape starts about 60 ms before its sound and overlaps the next. The lips close
  fully for `mbp` and touch the teeth for `fv`. The audio's loudness scales how far the jaw opens, so
  loud words open wider.
- **Mood.** The face holds the mood as its base expression (brows, eyes, cheeks), eased in over
  0.25 s, with the mouth layered on top.
- **Life.** Blinks run on the lab's clock (every 2.8 to 5.2 s), with an extra blink at phrase
  breaks. Small eye saccades look at the camera with brief glances away, the head nods on stressed
  words, and the breathing rises before each phrase. All of it comes from a seed set per cue, so a
  render is identical every time.
- **Body.** Head and shoulders move with the people rig's `talk` clip, at reduced strength.

The track is a list of controls per frame. Step 4 reads it, and a test reads it without a GPU.

### 4. Render

- **Stage.** MetaHuman Lab gets a speaking stage: `/realms/metahuman-lab?stage=speak&track=URL`. The
  stage loads the cast entry's pack through the broker, dresses it in the entry's outfit, frames
  head and shoulders, plays the track and the audio, and steps time by frames instead of the wall
  clock.
- **Look.** Every portrait gets the same three-point light, and a backdrop per character: for the
  mayor, a blurred, firelit town hall at night. The look lives in the cast file, not in code.
- **Recording.** The existing clip recorder does it:
  `tool.sh clip URL OUT --size 720x720 --fps 60 --frames DIR`. It's headless, renders with WebGPU,
  and paces frames deterministically.
- **Fallback.** Where the broker won't serve the pack (any host but openagents.com and staging), the
  stage draws the capsule fallback. The tool refuses to composite a fallback portrait, so you never
  ship a video with a capsule in it by mistake.
- **In the world.** Later, the same stage can render the mayor standing in Everglade (`?stage=speak`
  on `/worlds/everglade`). The first version is the portrait.

### 5. Composite

- **Panel.** The portrait goes in a panel in the lower left, 30% of the frame's height. It has a thin
  gold frame and the character's name under it in the game's own font, which matches Everglade's
  nameplates. The panel slides in 0.3 s before the line, stays 0.5 s after it, and slides out.
  Position, size and style come from the cue sheet's header, so a different video can put it
  elsewhere.
- **Sound.** Each line plays at its cue time. Under a line, the video's own sound ducks by 8 dB,
  easing over 0.2 s, and comes back afterwards.
- **Subtitles.** Burned-in subtitles are on by default, because promo videos mostly autoplay muted.
  They show word by word from step 1's times. You switch them off with `subtitles off` in the
  header.
- **Output.** The output is H.264 at 1080p for review, per the film-review rule, and 4K only once you
  approve the cut. The tool also writes an SRT and a JSON of every line's start and end.
- **Overlaps.** Two lines that overlap get two panels side by side. The tool warns if a line starts
  before the previous one ends unless the cue marks it `over`.

## The cast file

`verse/cast/cast.txt`, in git: the names, voices, framing and look. Packs are named only by an ID
that the owner-local private placements resolve. Rule 1 below keeps their digests and paths out of
the repository.

```
character mayor
  name      "Mayor Tobin Hale"        # placeholder; the owner names him
  pack      everglade-mayor            # private pack ID, resolved owner-locally
  outfit    mayor-coat                 # verse/tools/humans/outfits.py
  voice     google en-US-Chirp3-HD-Charon   # placeholder until the audition
  pitch     -2
  framing   head-shoulders
  backdrop  town-hall-night
  say       Odiodei = OH-dee-OH-day
  say       Everglade = EV-er-glayd
```

Characters can share a voice provider and differ only in voice, pitch and mood mapping, so a cast of
ten costs nothing extra.

## Making the mayor

This repeats the pipeline that built Draconia's MetaHuman, already proven, with one new character:

1. **Design (owner).** In MetaHuman Creator, the owner makes a man in his fifties: weathered, a
   short grey beard, kind and anxious. This is an owner step, recorded in `NEEDS_OWNER.md`, because
   it needs the owner's Epic login. The design can start from a preset.
2. **Export.** Unreal 5.7 or later, run headless (the `draconia_rebuild.py` path), exports DNA,
   meshes and textures to `~/work/film-out/everglade-mayor/metahuman/`. The export carries
   `NO-TRAINING.txt`.
3. **Pack.** `python3 verse/tools/humans/pack_human.py metahuman EXPORT_DIR` builds the pack with its
   face rig. `pack_checks.py` checks that the mouth moves: each viseme must move the lips by at least
   0.3 cm, and `mbp` must close them.
4. **Upload.** `upload_packs.sh` puts it in the private bucket. The owner-local placements file maps
   `everglade-mayor` to it.
5. **Outfit.** The mayor's coat, chain of office and hat go in `outfits.py`, made from CC0 parts or
   modelled for us, never from MetaHuman garments.

## Face rig work this needs

MetaHuman Lab's face rig runs today, with neutral, smile, frown, surprise, talk, blink and jaw. These
are needed for speech:

- **Visemes.** The 14 viseme expressions, as sets of raw controls (`CTRL_expressions_jawOpen`,
  `mouthFunnel`, `mouthPucker`, `mouthLipsTogether`, `mouthStretch`, `tongueOut` and so on). They go
  in `face_rig.py`'s named expressions. The pack build fails if any viseme moves the mouth less than
  its threshold.
- **Weighted mixing.** Several weighted expressions at once (mood plus mouth plus blink). The lab
  eases one expression into the next, and `facerig.coax` takes a control vector, so the work is
  summing weighted control sets before evaluation, then clamping.
- **Teeth and tongue.** These pieces are already in the pack, so they follow the jaw. The tongue
  needs its controls for `th` and `dtnl`.
- **Eyes.** A look-at target for the eyes.

## Auditions

`tool.sh speak --audition mayor "Hey! Is that... I hope we're not under attack."` voices the line with
about 12 candidate voices from both providers, with a short pick list per gender and age. It then
writes a local player page with one button per voice, the same way the launch film's sampler works.
You pick a voice, and it goes in the cast file.

## Rules

These are the MetaHuman binding rules ([metahuman-support.md](metahuman-support.md#binding-rules)) as
they apply here:

1. **Never in git.** The pack, the export, the portrait frames, the cache and the final video never
   enter git. In the repository go the code, the cast file (names, voices, look) and the cue sheets
   (lines and times). Placements stay in `~/.openagents/verse/private-assets.json`.
2. **Never for training.** No portrait frame, video or cache entry goes into a golden, dataset,
   eval, public trace or Labs output. The tool's outputs carry `NO-TRAINING.txt`. The
   frame-checking test uses the capsule fallback, never the MetaHuman.
3. **Publishing.** A promo video shows a render, not the asset, and publishing renders falls under
   the same settled rule as showing MetaHumans on openagents.com. The licence questions still open
   in metahuman-support.md (a custom engine, the revenue threshold) apply to the video too.
4. **No Epic trademarks in names.** The module is `aethyr/speak.coax` and the stage is `speak`.
5. **Voices.** Voices are stock provider voices, or ones you design in the provider's voice
   designer. No voice is cloned from a real person without that person's written consent.

## Tests

- **Align.** Fixed text and fixed word times give the expected viseme track, including the
  dictionary's names.
- **Perform.** A track gives the expected controls on chosen frames: the lips closed at each `mbp`,
  blinks only from the seed.
- **Stage.** With the capsule fallback, the speaking stage renders the same frame for the same track.
- **Composite.** From a test video and a test WAV, the panel's in and out frames, the ducking level
  and the subtitle times are right.
- **End to end.** A one-line cue sheet runs the whole tool in under 2 minutes on the Mac, using the
  fallback figure and a cached voice, so it runs offline.

## Building it

One issue under the raid launch work. The order:

1. The cast file, cue sheet parser and the composite with a still portrait. This alone proves the
   panel, sound and subtitles.
2. Voice and auditions: both providers, the cache, and the transcript check.
3. Align and perform, with the viseme expressions in `face_rig.py` and weighted mixing in the lab.
4. The speaking stage and portrait rendering.
5. The mayor: design (owner), export, pack, outfit, voice audition, and the raid promo's cues.

Steps 1 to 4 can use the MetaHuman Lab's existing pack while the mayor is being designed.
