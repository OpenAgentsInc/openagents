# Episode 293 show notes: what to put on screen

Written 2026-11-09 for [Episode 293: Everglade](293.md). The edited talk is
4:04 of Christopher speaking to the camera. These notes say what to show
over each part of it, how to make each shot, and what must be built first.

The model is the Verse trailer for Enter the Verse (#12458), where each
spoken idea was shown as a mechanic in the Grid world. Here every idea is
shown as something growing in Everglade, the way a player will see it.

## The short version

- **The centerpiece is the house time-lapses.** Five houses rise from bare
  ground, piece by piece, in the order a player builds them. Then a whole
  glade fills with houses and becomes a town.
- **Most other shots already exist** as things you can capture today from
  Everglade II, Character Create, Everglade v1 and Everglade Experimental.
- **One engine feature is missing.** A house today is drawn at a whole
  stage. A time-lapse needs it drawn at any point between stages. That is
  the one piece of new work, and the game needs it anyway to show a house
  being built.
- **Everything on screen should be the real engine.** A shot of something
  that isn't playable yet carries a small label that says so.

## What to show over each part

Times are from the edited video.

| Time | What is said | Show | Source |
| --- | --- | --- | --- |
| 0:00–0:19 | Every gamer has a game they wish existed. A button called Create Game. | Christopher on camera. No overlay for the first line. At "magic button", cut to a bare Grid with one glowing button; a press makes the Grid floor turn into the glade's grass and trees, spreading out from the button. | New shot. The Grid and the glade both exist. The spread is the trailer's "world reveals over the Grid" idea, done in Aethyr. |
| 0:19–0:31 | Our engine, our metaverse, creation tools. | Three quick cuts, about 3 s each: the engine (a frame of Everglade II with the wireframe or collider view toggled on, then off), the Verse (the home page's project towers, then walking through a gate), creation tools (a realm made at `/realms/new`). | Capture today. |
| 0:31–0:41 | "My dream game, which I'm calling Everglade." | The title. The camera rises out of the Character Create glade through the canopy, autumn light, and the word EVERGLADE sets in. Hold 3 s. | Capture today, with a keyed camera. The title is an overlay. |
| 0:41–1:15 | Old-school MMORPG on the D&D 5E SRD, and the mechanics list. | One short clip for each item, cut on his words. See [the mechanics montage](#the-mechanics-montage). | Mixed. |
| 1:15–1:46 | Built around a city, like Neverwinter Nights and Baldur's Gate. It begins as an empty glade. | The empty city clearing by its brook, at dawn, nobody there. Hold it for a full 4 s so the emptiness reads. Then one player walks in. | Needs the city clearing (#13093). Until it lands, use the glade and the clearing at the end of the path. |
| 1:46–2:00 | Be a founder. Get an early patch of land. | A top-down view of the clearing. Plot outlines draw themselves on the ground. One plot fills with a name. A player stands on it. | New shot. Plots are specified and not built. Label it. |
| 2:00–2:45 | The Housewright profession. Collect clay, stone and wood. Build simple houses. | **The house time-lapses.** Gathering first: hands pull branches, cut reeds at the brook, lift fieldstones. Then the lean-to, the hut and the cottage rise. See [the house time-lapses](#the-house-time-lapses). | Needs build progress between stages. Gathering is specified and not built. |
| 2:45–2:58 | The city grows over time, so at full release there is a whole city, built by players. | **The town time-lapse.** One fixed high camera over the clearing. Days pass as the light sweeps. Houses rise one after another, each at its own pace, small ones first, until the clearing is a town. End on the same framing as the empty clearing at 1:15. | Needs the same feature, plus many build sites at once. |
| 2:58–3:23 | AI friendly, used with taste. Real artists, designers and voice artists. | Split screen. Left: a realm changing as someone speaks to it. Right: the reference picture that set the glade's light, next to the glade as built. Then a row of the art kits' names as credits. | Capture today. See the note on credits below. |
| 3:23–3:41 | The game and its engine are open source. Fork our worlds and make your own rules. | The repository page, then one file of real game code (a spell or a house's parts), then FORK pressed on a realm and the same world with a changed rule: lower gravity or a different sky. | Capture today. |
| 3:41–4:01 | Connecting the worlds into the Verse. Get in, walk around. PRs welcome. | A player runs from the glade through a gate into the Grid, with other realms' gates visible. Then the link, large: `everglade.world`. | Capture today. Confirm the gate you want shown. |
| 4:01–4:04 | "We'll put cats in there." "Stay tuned." | A cat sits in the glade and looks at the camera. Cut to black on "Stay tuned." | Needs a cat. The asset registry has an animated animals pack; check whether it holds a cat. If not, end on Pantone, the real cat, and keep the promise for the game. |

### The mechanics montage

He lists six things in 34 seconds. Each gets one clip of 3 to 5 seconds,
with its name set small in a corner.

| Words | Clip | Status |
| --- | --- | --- |
| "Lore and creatures and abilities and math … fifth edition SRD" | A cast with the roll on screen: the d20, the modifier, the target's Armor Class, hit. Then a monster from the bestiary. | Exists. Experimental has SRD combat and damage numbers. |
| "Player-creatable housing from day one" | A two-second taste of the hut rising. The full time-lapse comes later, so keep this short. | Needs build progress. |
| "Dragonback battles" | A rider on a dragon over the treeline. | The engine has dragon effects and SRD dragon actions, and mounts are specified with horses. Check what can be flown today. If nothing, show a horse and rider at a gallop and label the dragon shot as planned. |
| "Destructible buildings" | A meteor hits a house in Everglade v1 and it breaks and burns. | Exists. This is the shot from Episode 290. |
| "PvP with consequence" | A player breaks the law, WANTED appears, and the guards give chase. | The guards and WANTED exist in v1. Player against player may not. Show the guards, and don't show a duel that isn't playable. |
| "Difficult and challenging and rewarding" | A player dies to a hill giant's thrown rock, respawns, and tries again. | Exists in Experimental. |

## The house time-lapses

### What exists

Everglade Experimental has a yard south-east of spawn with every house at
every stage, 24 houses in five rows:

| Tier | Stages | What it is built from |
| --- | --- | --- |
| Branch lean-to | 3 | Forked uprights, a ridge pole, leaning branches, leafy boughs |
| Woven hut | 4 | A ring of posts, woven withies, lashed roof poles, reed thatch |
| Wattle-and-daub cottage | 7 | A ditch, a fieldstone base, a frame, wattle, daub, thatch, a hearth |
| Timber house | 5 | A stone sill, a pegged frame, rafters, boarded walls, a roof |
| Stone cottage | 5 | Footings, dressed stone walls, gables, roof timbers, a chimney, thatch |

The houses are data: 146 parts, each a form (a pole, a wall, a ring, a roof
slope) in a material (bark, withy, daub, fieldstone, reed). The engine
draws a house at stage *k* by drawing every part of stages 1 to *k*
(`aethyr/world/build-site.coax`, `everglade/world/housewright.coax`).

### What is missing

A house appears one whole stage at a time. Cutting between 24 frozen houses
gives a slideshow. A time-lapse needs two things:

1. **Parts appear one at a time.** Each part gets a place in its stage's
   order: the corner posts before the posts between them, the low course
   before the high one.
2. **Each part grows.** A part is drawn at a fraction from 0 to 1, and each
   form grows its own way:

   | Form | How it grows |
   | --- | --- |
   | Pole or beam | It is carried in lying down, then stands up into its hole. |
   | Woven wall (ring) | Row by row from the ground, each withy threading round the posts. |
   | Stone wall, footing, sill | Stone by stone along a course, then the next course. |
   | Daubed wall | The wattle shows first. Daub spreads over it in patches from the bottom. |
   | Roof slope, gable, cone | Rafters or poles first, then thatch in overlapping courses from the eaves to the ridge. |
   | Slab or floor | It fills from one edge. |
   | Trench | It is dug along its line, and the spoil bank rises beside it. |
   | Kit piece (timber house walls, door, windows) | It lifts into its place in the frame. |

So a house's build is one number, its progress from 0 to its last stage.
The engine draws it at any value. The room already keeps the clock for a
stage's work time, so the same number drives the real game: a player who
walks past a half-built hut sees it half-woven, not at stage 1 or stage 2.

This is engine work in Aethyr, in the build-site module and its mesher. It
belongs in the engine, so every realm's buildings get it. The colliders
should keep to whole stages at first, so a wall blocks you only once its
stage is done.

### The shots

Each shot is one fixed camera and one continuous build. Use the recorder's
fixed game steps, so the clip is the same every time it is made.

| # | Shot | Length | Notes |
| --- | --- | --- | --- |
| 1 | Branch lean-to | 5 s | Low three-quarter view. Two uprights, the ridge pole, branches, boughs, and a fire lit in the stone ring as the last beat. |
| 2 | Woven hut | 8 s | Start top-down to show the ring of posts, then drop to eye level as the walls weave upward. The thatch closing to its knot is the payoff. |
| 3 | Wattle-and-daub cottage | 10 s | The most stages and the best to watch. Give the daub its own beat: bare wattle turning to a smooth wall. End with smoke from the roof. |
| 4 | Timber house | 8 s | The frame going up is the image. Hold on the bare frame against the sky for a moment before the boards fill it. |
| 5 | Stone cottage | 10 s | Course by course. Pass a full day and night over it, since this one takes 10 workdays. End on the chimney. |
| 6 | All five in a row | 6 s | The five finished houses side by side, small to large, with the tool that unlocks each one named under it: bare hands, stone blade, stone axe, Carpenter's Tools, Mason's Tools. |
| 7 | The town | 12 s | The fixed high camera over the clearing. See the 2:45 row above. |

Details that make these read as real:

- **Show the hands.** A character carries each pole and stone in. Without a
  builder, parts that assemble themselves look like a loading bar. If the
  carry animation isn't ready, show the builder standing at the site and
  let parts arrive from a pile beside them.
- **Show the pile.** The materials sit beside the site and shrink as the
  house grows: 60 withies, then 30, then none.
- **Let time pass.** The sun moves and shadows sweep. The spec's workday is
  20 real minutes, so a cottage's three days fit in 10 seconds of footage at
  about 360 times speed. Put the speed and the workday count in a corner.
- **Sound each material.** Wood knocks, withies creak, stones clack, thatch
  rustles. Aethyr builds its sound from numbers, so these can be made
  without recordings.
- **Use the glade's light.** It is the default for the art style and it is
  the look people will see when they play.

### If the build progress can't land in time

Use the frozen showcase as it is, in this order of preference:

1. **A walk down each row.** The camera travels past the stages of one tier
   at a steady pace, so the house appears to grow as you pass. This works
   today with no new code.
2. **Match-framed dissolves.** Shoot each stage from the same camera
   relative to its house and cross-fade between them, one second each. Every
   house faces west and uses the same frame, so the framing can match.

Both are honest about what the game does today. The real time-lapse is
worth the wait if the video can wait a few days.

## How the Verse trailer was made, and what we have now

### Then

The trailer for Enter the Verse was made from code, in three stages. The
scripts were removed in the repository reset on 2026-10-27. They are in git
history at `ba30ed21fe` (`scripts/film/`, `docs/verse/film/`). The rendered
media is on the Mac under `~/work/film-out/verse-trailer/`,
`~/work/film-out/verse-trailer-kiki/` and `~/work/film-out/cut/`.

| Stage | Tool | What it did |
| --- | --- | --- |
| Shots in the world | `film_shot`, a Rust program on the old engine | Rendered one shot from a JSON file: a scene, walkers and a keyed camera. Each frame averaged 8 sub-frames. A 10 s shot took 17 to 60 s. |
| The world over the Grid | The `astral` scene | Drew the same place as a Grid layer and as a lit world from one camera, with a reveal that spread from a point. |
| Screen footage | `scripts/film/footage/*.mjs` | Recorded the real apps headless, at each device's own pixel size. |
| Devices | Blender, headless | Put footage on 3D phone and laptop models. About 7.6 s a frame. |
| Labels | `tag-overlay.py` | Put a CONCEPT tag on any shot of something not built. |
| Voice | ElevenLabs Bella, seven lines | Kiki's storyboard set the six beats, the lines and the timings. |
| Music | `music.py` | Generated a bed with hits at the cut times. |
| The cut | `cut.py` over a JSON cut list | Assembled shots, overlays, voice and music, and printed contact sheets for review. |
| Review | `review.py` | A local page where the owner leaves notes at exact times. |

Two things made it work. Each shot was a small file anyone could change and
render again. And the cut list was data, so a note such as "hold this one
longer" was a one-line change.

### Now

| Need | What we have | Gap |
| --- | --- | --- |
| A deterministic recorder | `coax/web/clip.coax`: headless Chrome with WebGPU, one fixed game step a frame, optional sound ([docs/verse/clips.md](../verse/clips.md)) | None for 1080p. |
| A named setup for a shot | `?stage=NAME` on a realm's page | Everglade II and Experimental need stages for the shots above. |
| A keyed camera | The raid preview's camera flight (`?raid=preview&cine=1`), Aethyr chapter 78 | A general camera track that any zone can load. |
| Film quality | The film profile (`aethyr/film.coax`, #13004): many samples a frame for motion blur and depth of field on the native hosts | Use the recorder for this video. The film profile is for later. |
| A cut list and assembler | Nothing since the reset | Bring back `cut.py` and its JSON format from history, or cut by hand for this one video. |
| Labels, titles, music | Nothing since the reset | Same. |
| Review | The review page, still on the Mac at `~/work/film-out/tools/review.py` | None. |

For one four-minute video, cut by hand in an editor and spend the
engineering on the build progress and the shot stages. If a second video
follows, bring the cut list back as a Coax tool.

## Order of work

1. **Build progress between stages**, in the engine. Everything in the
   centerpiece depends on it.
2. **A stage for each time-lapse** in Everglade Experimental: one house at a
   clear site, a fixed camera, the sun moving, progress driven by the step
   count. Shots 1 to 5.
3. **Capture what exists today** while that is built: the title rise, the
   mechanics that exist, the fork, the gate to the Grid, the reference
   picture.
4. **The town shot**: many build sites in one clearing, each with its own
   start time. This needs the city clearing (#13093) or a stand-in meadow in
   Experimental.
5. **The builder and the pile.** A character at the site and a shrinking
   pile of materials.
6. **Labels and the end card.**

Render review cuts at 1080p, and capture headless.

## Suggestions

- **The video is the talk plus about 90 seconds of pictures.** Don't cover
  his face the whole time. Cut to pictures on the nouns (the glade, the
  houses, the fork) and come back to him for the claims (why, how you can
  join).
- **Open on the glade, not on him.** Three seconds of the empty glade with
  birdsong before the first word tells the viewer what this is.
- **Use the same framing twice.** The empty clearing at 1:15 and the built
  town at 2:58 from one camera is the whole pitch in two frames. It also
  makes a good thumbnail as a split image.
- **Put the numbers on screen.** "20 branches, 30 boughs, 5 minutes" under
  the lean-to. Players of old MMOs want the numbers. The spec calls them
  starting values, so say so in small type.
- **Name the link early and late.** `everglade.world` at about 1:20, when he
  says you can play, and again at the end.
- **Credit the art.** He says he wants real artists involved. A line at the
  end naming the art kits and their makers backs that up, and the SRD needs
  its attribution line anyway: put the CC BY 4.0 statement in the
  description and a short form on the end card.
- **Keep AI pictures out.** Every frame should come from the engine. A
  generated clip in a video that promises taste with AI would undercut the
  line at 2:58.
- **Don't show what a new player can't find.** If the link opens the glade
  and a path, a video full of towns, dragons and duels sets up a letdown.
  Label what is coming, and end on what is there today.
- **The cat.** Deliver it. One cat in the glade at launch costs little and
  closes the video's last promise.

## Corrections made to the transcript

The transcript wrote the profession as "House Rite". The profession is
Housewright ([spec](../../everglade/docs/housewright-profession-spec.md)).
The transcript now says Housewright.

## Open questions

1. **When does the video go out?** That decides between the real
   time-lapses and the fallback.
2. **Is the dragon flyable today?** If not, is the dragonback line shown
   with a labeled planned shot, or with a horse?
3. **How many plots does the clearing hold?** The town shot needs a number.
   The spec leaves it open.
4. **Voice.** Is this cut his camera audio only, or does it also use a
   narrator line for the title and the end card?
5. **Music.** Generated, as in the trailer, or licensed?
