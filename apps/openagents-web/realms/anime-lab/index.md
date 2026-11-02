# Anime Lab

Direct an anime with your voice, at https://openagents.com/worlds/anime-lab. It is unlisted: the [worlds list](https://openagents.com/worlds) leaves it out, so you open it by its address. It is drawn in the Verse's anime style, its action variant (cel shading in two flat tones, coloured ink lines, a vivid dusk sky with towering flat-shaded clouds).

## What it shows

- `?stage=direct` (the default) plays a directing session. A person directs, a line at a time, like a director on set: "Draconia raises her wand. She casts a fireball at the gate. Close up on Draconia. Flash to Kai, he's looking sad. Now he's angry..." The bottom of the frame is a live caption: the words as they are heard (partial, then final), then an arrow and what the director made of them, in the catalogue's words (`CAST Draconia · fireball → gate`, `SHOT close up · Draconia · push in`, `CUT flash → Kai`, `EMOTE Kai · sad`), each with its confidence and whether it was in stock. The scene plays each one as it comes.
- The voice and the decision model are simulated: the words, their timing and the director's answers are recorded in the session, as the typed selector would return them. Everything after that is the real path: each answer is read and held to the catalogue, laid out on a timeline under the anime grammar (a close-up holds at least 0.6 s, a flash cut is three white frames, no cut lands inside a wind-up), and played by the stage. When a direction's subject isn't in the shot, the engine frames it itself; those shots show as ENGINE.
- One line asks for something that isn't in stock: "an ink dragon crashes through the wall". It draws in while the scene keeps playing: ink strokes sketch it, flat colour floods in from the lines, then the shading, labelled DRAWING IN... then MADE.
- The stage is the courtyard of Skyreach Academy, our own cliff-top school of magic, at dusk: towers with lit arched windows, braziers, banners. The cast are our own designs: Draconia (long pale hair, a long dark coat), Kai, three hooded figures and a dark lord in a tall collar. Effects: fireballs with trails and bursts, smoke, rubble raining down, speed lines, impact frames and white flashes.
- `?stage=film` plays the same stage as a ten-second short with no overlay. `&t=SECONDS` holds one moment; `&card=1` leaves the overlay out. Space starts the take again, F switches between the two, H hides the overlay.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/anime/director.coax` is the director: the catalogue of directions, reading the decision model's answers, the session and the timeline with the anime grammar, with its checks.
- `verse/anime/stage.coax` plays a timeline at a moment: who stands where, their poses and feelings, the effects, the draw-in, and the camera's shots flown with `aethyr/cinematic.coax`.
- `apps/openagents-web/animelab.coax` is the world and its overlay; `animelab-cast.coax`, `animelab-world.coax` and `animelab-mesh.coax` build the cast, the academy and the effects; `animelab-style.coax` gives the anime style's numbers; `gpu/animelab.coax` is its shaders, written to `animelab.wgsl`.
- `docs/verse/anime-lab.md` in the OpenAgents repository is the plan: live voice, partial words, Psionic's decision model and the makers come next.

## Make your own world

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
