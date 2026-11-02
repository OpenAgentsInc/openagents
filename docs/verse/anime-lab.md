# Anime Lab: direct an anime with your voice

Status, 2026-11-02: the recorded demo is on `main`. The authenticated
[director API](2026-11-02-anime-director-api.md) is implemented and qualified
for #12632. Arbitrary takes, the Anime CLI, live voice and asset generation
remain distinct steps below. Source completion does not assert deployment.

Owner request, 2026-11-02:

> I want people to be able to real time basically direct an anime. Let's
> just demonstrate that a person can, with the power of their voice, create
> animes. Someone directs and says things like: "Draconia casts a fireball,
> close up on Draconia, flash to Harry, he's looking sad, he's looking
> angry, he starts casting something, Draconia is hesitating, all of a
> sudden three Death Eaters come, there's Voldemort, a huge fireball hits,
> rubble rains down", and these kick off decision-model calls: it gets
> matched with something that's in stock, or we draw it rapidly. Ideally
> the user can ask for things even if there's no library for it, and we
> draw it almost instantly, or show that it's drawing in, with a cool
> effect as it draws in. Simulate the voice for now if you need to.

Anime Lab is an unlisted realm, `/realms/anime-lab`, and a labs command,
`openagents labs anime`. It is the newest of the Labs: the same loop as
the pre-Nuke Video Lab and Game Lab, a typed document and checked edits
that make an output (see [the labs spec](../cli/2026-11-01-openagents-labs.md)).
What it adds is that the document grows live, one spoken line at a time,
and the output is a scene playing as you speak.

## What a person does

1. Opens Anime Lab. A stage waits: a courtyard of a cliff-top academy at
   dusk, under towering clouds, drawn in the anime style.
2. Holds **DIRECT** (or Space) and talks, like a director on set:
   "Draconia casts a fireball. Close up on Draconia. Cut to Kai, he looks
   sad. Now angry. He starts casting something. Draconia hesitates. Three
   hooded figures arrive. A huge fireball hits. Rubble rains down."
3. Each line becomes one or more **directions** within about half a second
   of the words ending. The scene plays them as they come: the camera
   cuts, the character acts, the effect fires. What was heard and what it
   became show at the bottom: `heard "close up on Draconia"`,
   `→ CLOSE-UP Draconia · in stock · 0.97`.
4. Something the library doesn't have ("a dragon made of ink crashes
   through the wall") doesn't stop the scene. It **draws in**: ink lines
   sketch its outline where it will stand, then flat colour floods in, then
   shading and the line weight settle. It takes as long as it takes to
   make; the scene keeps playing around it.
5. **TAKE** records what was directed. Every take is a document (below), so
   it plays back identically, renders to video at any size, and can be
   edited by voice or by hand and played again.

## The pipeline

```
mic ─► voice host ─► words ─► director (decision model) ─► directions ─► scene timeline ─► engine
       (Psionic       (final    (typed catalogue,            (anime.v1)     (now + queue)    (cameras,
        Whisper)       and       bounded fields,                                               cast, FX,
                       partial)  calibrated)                                                    draw-in)
                                              └─ not in stock ─► maker ─► asset (draws in when ready)
```

### 1. Speech to text: the voice host on Psionic

- The page's voice path is the one Everglade's voice edits use
  (`apps/openagents-web/everglade-voice.coax`, `coax/web/voice.js`): hold to
  talk (action 7 "on"), let go to end the turn ("end"), the voice host
  sends the final words back as message 50 `heard|WORDS`.
- The voice host runs speech to text on Psionic's Whisper
  (`voice-host --provider psionic`, commit d4a13dfafc, #12622): a
  re-recognition of up to 4.3 s takes 184-240 ms on the GPU and the final
  adds nothing after the end of the turn
  ([psionic/docs/transcribe.md](../../psionic/docs/transcribe.md)).
- **The cast's names are vocabulary.** Whisper's `--words` bias gets the
  names on the stage and the library's words ("close-up", "fireball",
  "hooded"), so made-up names are heard right.
- **Directing doesn't wait for the end of a turn.** A director talks in a
  stream. Anime Lab also reads the partial words: when a partial ends a
  clause ("…casts a fireball,") and the director is confident (below), the
  direction plays before the sentence is over. The final words can correct
  it: a direction that the final words no longer support is taken back
  within the same beat.

### 2. The director: words to directions with a decision model

The words never reach the engine. A decision model reads them and answers
typed questions over a fixed catalogue with bounded fields, the way
Everglade's voice edits and the Sandbox's set pieces already do
(`coax/web/api/npc.c`, `verse/setpiece.coax`), and the page checks the
answer again against the same catalogue before anything plays.

- **The state it sees:** the cast on stage (names, archetypes, where each
  stands, what each is doing), the current shot, the last few directions,
  and the library's entries.
- **The questions:** how many directions the words hold (a sentence often
  holds three), and for each: its kind, its subject (a cast slot), its
  object or target, its effect or shot, and its timing (now, after the
  last, with the last). Each answer is a probability per option.
- **Calibrated thresholds** ([psionic/docs/decisions.md](../../psionic/docs/decisions.md)):
  over 0.85 plays at once; 0.5 to 0.85 plays and shows a soft "did you
  mean" chip for a second; under 0.5 the line shows as heard and nothing
  plays. A destructive direction ("clear the stage", "delete the take")
  needs over 0.9 and a second word ("yes").
- **Where it runs, in order:**
  1. Today: the server's typed selector (Gemini picks one or more entries of
     the catalogue, held again by SQL), as Everglade's voice edits do.
     About a second.
  2. Next: Psionic's decision model through the SystemOne routes across
     the Pylon fleet (a51f7b19bd), one forward pass per question, no
     generated text: tens of milliseconds
     ([Psionic book, chapter 222](../../psionic/book/ch222-a-decision-in-milliseconds.md)).
  3. Offline: a small local decision model in the page, for the stock
     catalogue only, so directing works with no network.
- **No keyword routing.** Nothing matches strings to pick a direction
  (workspace rule: semantic routing only). The only deterministic parsing
  is of bounded fields after the decision: a number ("three hooded
  figures"), a colour, a cast name that is already on stage.

### 3. Directions: the catalogue

Each direction is a typed record. The stock catalogue for the first
version:

| Kind | What it does | Fields | Examples |
| --- | --- | --- | --- |
| `enter` | Brings a character onto the stage | archetype, name, count, where (left, right, behind, above), how (walks, runs, appears, lands) | "three hooded figures arrive", "Kai runs in from the left" |
| `exit` | Takes one off | who, how | "the figures vanish into smoke" |
| `act` | A body action | who, action (raise wand, cast, block, run, fall, kneel, turn, point, hesitate, recoil, shout) | "he starts casting something", "Draconia hesitates" |
| `emote` | A face and pose | who, emotion (calm, sad, angry, afraid, determined, smug, shocked, crying, laughing), strength | "he's looking sad", "now angry" |
| `cast` | A spell from one to another | who, spell (bolt, fireball, shield, frost ring, guardian, light), colour, target, size | "Draconia casts a fireball", "a huge fireball hits" |
| `fx` | A world effect | effect (explosion, rubble rain, flash, smoke, embers, wind, lightning, petals, shockwave), where, size | "rubble rains down", "a white flash" |
| `shot` | The camera | framing (extreme close-up, close-up, medium, wide, establishing, over the shoulder, low angle, high angle), subject, move (hold, push in, pull out, orbit, shake, whip pan), lens feel | "close up on Draconia", "wide shot of the courtyard" |
| `cut` | A hard change of shot | to (a cast slot, the last shot, a new framing), transition (cut, flash, smash, speed-line wipe, match) | "flash to Kai", "smash cut to the gate" |
| `beat` | Timing | hold seconds, slow motion, freeze frame, speed lines on | "hold on that", "slow motion" |
| `set` | The world | time of day, weather, variant (pastoral, action, dusk, neon), music mood | "make it night", "start raining" |
| `say` | A line spoken by a character | who, words (shown as a subtitle; voiced later) | "Draconia says: not today" |
| `make` | Anything not in stock | a short description, the archetype it is closest to | "a dragon made of ink crashes through the wall" |

- **Archetypes, not names.** The stock cast is archetypes drawn in our own
  designs: the heroine (long pale hair, a long dark coat), the hero, the
  rival, the mentor, students, hooded figures, a dark lord in a tall
  collar, a guardian of light, creatures. A spoken name binds to a slot the
  first time it is heard ("Draconia" becomes the heroine; "Kai" the hero),
  and is only a label after that.
- **Third-party names.** People will name characters from films and books
  they love. A private take may use any label. Our stock art never copies
  another work's character designs, crests, uniforms or spell words, and a
  take using a third party's names can't be published to a public page or
  listed (the same rule the pre-Nuke Draconia study set: our own names and
  school for anything published).

### 4. The scene: a timeline that plays while it is written

- **The take is a document,** `anime.v1`, plain text in the labs style, one
  direction per line with its time:

  ```
  anime 1
  stage courtyard variant action clock 0.78
  cast heroine name Draconia at 0 0
  0.0 cast Draconia fireball target gate size large
  1.2 shot close-up Draconia move push-in
  2.4 cut flash to Kai
  2.6 emote Kai sad
  3.8 emote Kai angry
  4.6 act Kai cast
  5.4 act Draconia hesitate
  6.2 enter hooded count 3 where behind how appear
  7.4 cast none fireball target courtyard size huge
  8.0 fx rubble-rain where courtyard
  ```

- **Live, it is a queue.** A direction with timing "now" interrupts what is
  playing at the next beat boundary; "after" waits for the last to finish;
  "with" plays alongside it. Shots and cuts follow the anime grammar: no
  cut lands inside an action's anticipation, a close-up holds at least
  0.6 s, a flash cut is three white frames.
- **The engine plays it** with the pieces that exist: camera flights
  (`verse/cinematic.coax`), the cast and its clips (`verse/cast.coax`),
  particles (`verse/particles.coax`) and the anime style's lines, flats,
  speed lines and impact frames.
- **The same document renders offline** at any size: `openagents labs anime
  render take.anime take.mp4 --size 1920x1080` drives the page's
  `?take=` in clip mode (`coax/web/clip.coax`), so the video matches what
  was seen.

### 5. Drawing in what isn't in stock

A `make` direction, or a field the library doesn't have (a creature, a
prop, a costume), never blocks the scene.

1. **At once (under 100 ms):** a placeholder of the nearest archetype, as
   ink lines only, sketches itself in where the thing will stand: strokes
   draw along its silhouette in a few frames, as an animator's rough. The
   camera may already frame it.
2. **While it is made:** the maker runs, in order of speed:
   - the kit: compose it from stock parts (a dragon from creature parts, a
     wall of ink from the rubble and smoke kits, recoloured);
   - procedural: a generator for the kind (a tree, a crowd, a tower);
   - generated: a 3D model generator for anything else, seconds to a
     minute.
3. **The reveal:** when the asset is ready, flat colour floods in from the
   lines outward, then the shadow tone, then the line weight settles to
   the style's. If the maker fails, the sketch stays as a stylised ink
   figure, which still reads as anime.
4. **What was made joins the library** for the rest of the take and, with
   the maker's consent, for everyone: the next "ink dragon" is in stock.

### 6. Rights, safety and cost

- No real people's likenesses on demand; the maker refuses a named real
  person, as the Sandbox's catalogue refuses anything outside it.
- Generated assets carry a label (`source: generated`) in the take and in
  any video's sidecar, as [clips.md](clips.md) requires.
- Directing costs: speech to text runs on our hosts or Pylons; decisions
  are milliseconds once on Psionic; a generated asset is the expensive
  part and counts against a budget the person can see.

## The demo

`apps/openagents-web/animelab.coax` implements the recorded demo.
`/realms/anime-lab?stage=direct` plays a simulated directing session: a
recorded list of spoken lines, each arriving at the time a person would
have said it (the voice is simulated; the words, their timing and the
directions they become are what the live path produces). The bottom of
the frame shows the line as heard, then what the director made of it, its
confidence, and whether it was in stock or drew in. The session is the
example above, with our own names: Draconia (the heroine), Kai (the hero),
three hooded figures and a dark lord.

## The work, in order

| Step | What | Done when |
| --- | --- | --- |
| A1 | The anime style (art-styles.md, Anime) | The style's checks pass; a realm can name `anime` |
| A2 | The stage, cast and effects in the anime style; `?stage=direct` with the simulated session | The demo plays and records with `coax/web/tool.sh clip` |
| A3 | `anime.v1` documents, `?take=`, and `openagents labs anime render` | A take renders the same frames twice |
| A4 | Live voice: DIRECT, the voice host on Psionic, the server's typed selector over the catalogue | A spoken session directs the stage, about a second per line |
| A5 | Partial words and corrections; the decision model on Psionic; calibrated thresholds | Under 0.5 s from a clause's end to its direction |
| A6 | Draw-in: sketch, kit and procedural makers; then generated assets | "An ink dragon crashes through the wall" draws in and joins the library |
| A7 | Interactive pieces: duels a player fights in the directed scene, viewers voting on the next beat | A playable take |
