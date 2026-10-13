# Draconia: a wizard-school cinematic and playable world in Verse

Status: **spec, 2026-10-13.** Nothing here is built yet.

The owner asked for two things:
- a cinematic in the style of the viral "Draconia Malfoy" fan anime;
- a playable world in the same setting, with wands and duels.

The work starts by porting the film's first few seconds into Verse shot for
shot, but in our realistic engine instead of anime.

## The reference

- **The film:** a 192 s, 21:9 (2520×1080, 30 fps) fan anime with sound,
  credited at its end to Aiden Guo (@aidenguoai, atelier432).
- **Copy on this Mac:** `~/Desktop/gWRgh471YT4j4E-4.mp4`.
- **Never in the repo:** neither the film nor frames from it are committed.
  Contact sheets live under `~/work/film-out/draconia/ref/`.
- **The story it tells:**
  - a night battle in a burning castle courtyard;
  - students pinned behind walls while green spell bolts tear through;
  - a blonde student (Draconia) in close-ups, her eye, a shout;
  - her standing alone in the ruins as dark hooded figures close in;
  - a blinding light, a conjured silver peacock guardian, wings of light;
  - a flashback to her laughing with a dark-haired boy;
  - the line "I am Draconia Malfoy".

## Rights: what we can publish

- **Two kinds of work belong to other people:**
  - The setting, the names and the house uniforms are Harry Potter (Warner
    Bros. / J.K. Rowling).
  - The shot design is Aiden Guo's.
- **The shot-for-shot port (phase D1) is an internal engine study.** We use
  it to measure Verse against a strong reference. It isn't published
  anywhere: not the website, X, an episode or a store build.
- **Anything we publish uses our own names, school, crest and uniforms.**
  The working title is "Draconia"; see [character.md](character.md) for
  original names.
  - Wands, duels and a castle school are a genre, not anyone's property.
  - Houses, the Malfoy name, "Patronus", "Avada Kedavra" and the crests are
    not ours to use.
- **To publish a shot-for-shot version, ask Aiden Guo first** (contact on the
  film's end card). That's the owner's call; it's in Open questions.

## The pieces

| File | What it covers |
| --- | --- |
| [shots.md](shots.md) | The reference broken into shots with times, and the Verse plan for each shot in the first segment (phase D1) |
| [character.md](character.md) | Draconia and the supporting cast, built on the Alice pipeline: long pale hair, the uniform, the wand, faces for close-ups |
| [world.md](world.md) | The playable zone: the castle courtyard, wands as a spell focus, duels, the guardian conjure, students as agents |

## What Verse already has

| Need | Verse today |
| --- | --- |
| A realistic female lead | Alice (`docs/verse/female-character.md`): scripted Blender build on the Universal rig (65 joints), hair strips, layered outfits |
| Shots from a keyframed camera | `crates/verse/examples/film_shot.rs` with `docs/verse/film/verse-shots/*.json`: scene, rig keys (eye, target, focal mm, ease), shake, subframes. Output 1920×1080/30 at `~/work/film-out/verse/` (`docs/verse/film/cinematic.md`) |
| Fire, embers, smoke, sparks, shockwaves | Sprite particles from TOML effects with Blender-made sheets (`docs/verse/particles.md`, Meteor Swarm) |
| Walls that blow apart | Destructible buildings, phase D1 (`docs/verse/destructible-buildings.md`, `verse --demolition`) |
| Medieval stone architecture | The Modular Medieval Town kit (`docs/verse/everglade-medieval-refactor.md`; pack outside the repo) |
| Spells, mana, cooldowns, duels | The MMO combat model (`docs/verse/combat-model.md`) and the spell crates (`verse-water-spells`: bolt, orb) |
| Night, moon, fog | The PBR sky and weather (Water Lab's weather; Everglade's town clock) |
| Cuts, music, titles | `scripts/film/cut.py`, `music.py`, the title overlay |

## What's missing

1. **A castle courtyard set.** Tall Gothic walls, arched windows, cobbles,
   rubble and night fires. It's built from the medieval kit plus new Blender
   pieces such as tracery windows and a broken wall.
2. **Wand spell effects.** A thin straight beam with a hot core (the green
   bolt), an impact burst, a shield shimmer, and a wand-tip light.
3. **Hair in wind.**
   - Alice's hair strips are skinned, not simulated.
   - The hero close-ups need long hair that streams and lifts.
   - The first step is a cheap spring-chain on the hair joints driven by a
     wind field. Strand rendering comes later, only if the shots need it.
4. **Face close-ups.** The reference lives on extreme close-ups (eye, mouth,
   cheek with a cut and soot). That needs:
   - an eye shader with a real iris and a wet highlight;
   - blend shapes for the mouth (a shout);
   - decals for soot and a cut.
5. **Depth of field and bloom good enough for bokeh embers.** The film
   engine has keyframed focal length. It needs `fstop` and focus distance per
   key, the way the Blender studio shots already have them.
6. **The conjured guardian.** A creature made of light, at first a peacock
   with its tail fanned. It's an emissive mesh with particles, and the
   largest single effect.
7. **21:9 output.** The reference is 2.33:1. Shots should render 2520×1080
   (or letterbox 1920×824) to match framing.

## Phases

| Phase | Deliverable | Done when |
| --- | --- | --- |
| **P** | **The peacock guardian (1:53–1:58), first:** feathers of light, the flash, and Draconia standing before the fanned tail ([shots.md](shots.md#the-peacock-guardian-153158-owners-first-pick)) | A 1080p render (and a side-by-side) the owner reviews |
| **D0** | Reference prep: shot list with times (done in [shots.md](shots.md)); contact sheets and per-shot stills in `~/work/film-out/draconia/ref/` | Every shot in 0–26 s has a still and a time |
| **D1** | **The first ~10 s, shot for shot** (shots 1–6): Draconia's close-up in wind and embers, the girl at the wall, the boy behind the wall as the green bolt passes, the wall blast, the wide duel | A side-by-side video (reference left, Verse right, 21:9) the owner reviews at 1080p |
| D2 | The rest of the battle (shots 7–15, to 26 s): the profile close-up, the run, the shout, the eye, her alone in the ruins, the ground shockwave, the wide courtyard | Side-by-side to 26 s |
| D3 | The playable courtyard (world.md W1–W2): walk it, cast with a wand, duel training dummies | Playable on desktop |
| D4 | Original version: our names, crest and uniforms; the guardian conjure; our own music and Bella's voice for the lines | A cut we can publish |
| D5 | Duels with students as agents, and the full playable zone (world.md W3–W4) | Multiplayer duel in the zone |

The owner picked the peacock scene (P) as the first to build, before D1. D1 is the "first few seconds" of the film. D4 is the first thing
that can leave the building.

## Open questions for the owner

1. Ask Aiden Guo about a public shot-for-shot version, or keep D1–D2
   internal and publish only the original D4?
2. Aspect: render 21:9 to match the reference, or 16:9 for our other films?
3. The original names: keep "Draconia" as the lead's name (it is the fan
   nickname, not a canon name) with an original surname and school?
4. Should the playable zone be its own world (a new zone crate) or a district
   of Everglade?
