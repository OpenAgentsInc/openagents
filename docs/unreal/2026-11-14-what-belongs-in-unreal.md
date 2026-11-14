# What belongs in the prototype and what belongs in Unreal

Audit, 2026-11-14. Status: a recommendation for the founders. Nothing here is
built except where a section says so.

Splice Bots is prototyped in the OpenAgents Metaverse Engine and finished in
Unreal Engine 5.9. The founders are happy with the prototype's speed and want
to start adding things in Unreal that use what Unreal is good at. Particle
effects are the first candidate. This document says what should live on each
side, why, and what the export pipeline needs before that works.

Background:

- [The exporter's specification](2026-11-12-aethyr-to-unreal-exporter-spec.md)
- [Why the two builds differed](2026-11-12-splice-unreal-vs-aethyr-session-audit.md)
- [Download sizes](2026-11-12-splice-download-size.md)
- [What Unreal Engine 6 removes](ue6.md)
- The exporter's own guide: `metaverse-engine/export/unreal/README.md`

## 1. Recommendation in one paragraph

Keep in the prototype everything that decides what is true in the game: the
rules, the layout, collision, encounters, timing, the interface and its text.
Put in Unreal everything that only presents a moment that the rules have
already decided: particles, lighting and post-processing, sound, final meshes
and materials, character animation, camera feel and cinematics. The line
between them is one direction of data. The rules tell Unreal what happened.
Unreal never tells the rules anything except input. Particle effects fit the
Unreal side cleanly, and the pipeline needs one small addition to carry them:
an event must say where it happened.

## 2. How the pipeline works today

These are facts about `main`, read from the exporter and the bridge plugin.

| Part | Where it comes from in Unreal |
|---|---|
| The rules | The game's Coax module compiled to C and built into the Unreal project. The bridge plugin (`AethyrBridge`) steps it each tick. No rule is rewritten. |
| The static world | Exported as glTF and imported as static meshes in a level. |
| Collision, placements, tuning, strings | One JSON manifest in the export bundle. |
| Moving things (player, Digger, raiders, boss) | Drawn from the rules' per-frame vertex stream, or by an Unreal mesh when a person supplies one. |
| The interface | Drawn from the rules' per-frame overlay stream. |
| The look | A hand-written Unreal material set driven by the game's uniform block. |
| Input | Unreal keys and mouse forwarded to the rules. |

A person changes the Unreal build through one table, `DT_AethyrOverrides`, in
`/Game/<Game>/Overrides/`. The importer never writes to that folder, so the
work survives every re-export. Each row names what it replaces:

| Row | Replaces | With |
|---|---|---|
| `thing.raider`, `thing.raider_v1` | A kind of moving thing, or one variant of it | A static mesh, placed each frame at the rules' position and yaw |
| `kit.garage_bay` | A kit slot's grey blockout, everywhere the slot stands | A static mesh |
| `material.cel`, `material.sky` | The look | A material |
| `event.pickup` | Nothing; it adds to the event | A sound |

Two details matter for this audit:

- **Events exist, and they carry only a name.** Each frame the bridge asks the
  rules for the events that fired (`aethyr_events`) and gets a list of
  numbers. The manifest maps each number to a name such as `hurt`, `smack`,
  `wall-broke` or `boss-down`. Today an event plays a 2D sound and, for four
  of them, a camera shake. It has no position, direction or strength.
- **A thing reports eight numbers:** its kind, variant, position and yaw. It
  does not report speed, state (drilling, stunned, sprinting) or a bone.

## 3. The rule for deciding

Ask one question about a feature: **if it were missing, would the game play
out differently?**

- **Yes: it stays in the prototype.** A wall that blocks, a raider that
  appears at dusk, a weak point that opens for four seconds, the text of a
  prompt. These must be the same in both builds, the agent playtester must be
  able to see them, and they must change in seconds.
- **No: it can live in Unreal.** Dust behind the rover, sparks from the
  drill, bloom on the LEDs, the sound of a hit, the cloth on her jacket. The
  run reaches the same ending without them.

A feature that is both gets split. The boss's attack warning on the floor is
gameplay: its shape and timing stay in the prototype. The glow and heat haze
on top of that shape are presentation: Unreal adds them.

Three reasons the line sits here:

1. **Speed.** A change in the prototype shows in about 8 seconds to a minute.
   An Unreal web build takes about 10 minutes. Anything iterated during a
   playtest has to be on the fast side.
2. **One copy of the rules.** The first Unreal session rebuilt the game by
   hand in 3,812 lines of Python and 15 Blueprints, and it drifted from the
   prototype within a day. Hosting the compiled rules ended that. Gameplay
   written in Unreal would bring the second copy back.
3. **Unreal Engine 6.** Epic has said UE6 removes Blueprints and Actors.
   Content assets carry forward: meshes, materials, Niagara systems, sounds,
   animations. Gameplay logic written in Blueprints does not.

## 4. Feature by feature

| Feature | Home | Notes |
|---|---|---|
| Rules, state, win and lose | Prototype | Never in Unreal. |
| World layout and collision | Prototype | Moving a wall in Unreal does not move the rules' wall. The importer reports generated actors moved by hand. |
| Encounters, time of day as a rule (raiders at dusk) | Prototype | The clock is state. |
| Interface, prompts, text | Prototype | Playtest 3's fixes were all text and timing. They need the fast loop. |
| Tuning numbers | Prototype | Exported in the manifest. |
| Input mapping | Prototype | Unreal forwards keys. |
| Camera position and framing | Prototype | The camera's job in a fight is gameplay. |
| Blockout meshes | Prototype | Grey shapes that prove the layout. |
| **Particle effects** | **Unreal** | Niagara. Section 5. |
| Final meshes for kits and things | Unreal | Already supported by `kit.` and `thing.` rows. |
| Materials, textures, decals | Unreal | Already supported by `material.` rows for the look; per-mesh materials come with the meshes. |
| Lighting, shadows, bloom, fog, colour grade | Unreal | The exporter's own notes list shadows, bloom and full ink as still missing in the Unreal look. This is where Unreal gives the most for the least work. |
| Sky and time of day as a picture | Unreal | Driven by the rules' clock value, so night falls when the rules say. |
| Sound and music | Unreal | Already supported by `event.` rows. Needs positions for 3D sound (section 5). |
| Character skeleton and animation | Unreal | The character pipeline (#13585) already asks for the UE5 Mannequin skeleton. Unreal can retarget, blend and add cloth and hair motion. The rules say which action she is in; Unreal plays it. Needs action state on a thing (section 5). |
| Camera feel: shake, lens, depth of field | Unreal | Shake is already wired to four events. |
| Cinematics (the memory at the end) | Unreal | Sequencer, started by an event. The rules wait for a "done" input or a fixed time. |
| Destruction (the rock wall) | Unreal | A Chaos break played on `wall-broke`. The rules already removed the wall's collision. |
| Foliage, clutter, set dressing | Unreal | Anything a player cannot collide with or shelter behind. If it blocks, it is layout. |
| Packaging and platforms | Unreal | The jam entry is the Unreal build. |

## 5. Particle effects in Unreal: how

### 5.1 What works already

A burst that does not need a place can be added today with no pipeline
change: a full-screen flash on `hurt`, a sting on `boss-down`. That is the
same mechanism the sounds use.

### 5.2 What to add to the export

Three additions cover almost every effect in the slice. Each is small, and
each is exported, so the prototype stays the source.

1. **An event carries a place.** Today an event is one number. Make it a
   short record: name, position, direction, strength, and the id of the thing
   it belongs to (or none). The drill's sparks, a hit's impact and a pickup's
   pop all need a position. This also gives 3D sound for free.
2. **A thing reports its state.** Add velocity and a small set of flags the
   rules already know: sprinting, on the board, drilling, stunned, charging,
   dead. A dust trail then follows speed, and the drill's sparks run while
   the flag is set.
3. **New row kinds in the overrides table.**

| Row | Plays | When |
|---|---|---|
| `fx.wall-broke` | A Niagara system, once | At the event's place, turned to its direction |
| `trail.rover` | A Niagara system, attached | While the thing exists; its spawn rate reads the thing's speed |
| `state.digger.drilling` | A Niagara system, attached | While that flag is set on that kind of thing |

The bridge passes strength, speed and the look's colours to each system as
Niagara user parameters, so one dust system serves the rover, the board and
the boss.

### 5.3 What to keep in the prototype

- **Anything a player must read to play.** The boss's attack outlines, the
  orange rings at weak points, the compass. They are flat shapes drawn by the
  rules, and they must look the same to the agent playtester.
- **A plain stand-in for each exported effect.** One puff or flash in the
  prototype where Unreal will play the real thing. It costs a few bytes, it
  keeps the prototype readable, and it proves the event fires at the right
  time and place. The archery range needed exactly this: an arrow nobody
  could see was not a working arrow.

### 5.4 Effects to build first

Chosen from playtest 3, where the player asked for feedback she did not get:

| Effect | Trigger | Why first |
|---|---|---|
| Drill sparks and rock dust | `state.digger.drilling`, `fx.wall-broke` | She liked the dig animation most. |
| Rover and hoverboard dust | `trail.rover`, `trail.player` on the board | Sells speed; she asked if she could go faster. |
| Hit impact on her and on the boss | `fx.hurt`, `fx.smack` | She was hit and did not know by what. |
| Weak point stun | `fx.stun` | The fight's one success moment. |
| Night LEDs and their glow | The clock value | She asked for yellow LEDs at night. |

## 6. What this costs

- **The two builds will stop looking alike.** That is the point, and it is
  also a risk: a playtest of the prototype no longer shows the finished game.
  Play the Unreal build at least once a day once effects go in.
- **The agent playtester cannot judge Unreal's effects.** It plays the
  prototype headless. It can check that an event fired; it cannot see whether
  the sparks look right. Effects need a person's eyes.
- **Unreal iteration is slow.** About 10 minutes for a web build against 8
  seconds for the prototype. Work on effects in the editor against a recorded
  run of events, not by rebuilding.
- **The web build is heavy.** The Unreal web build was 72.7 MB against the
  prototype page's 443 KB when measured on 2026-11-12. Niagara and textures
  add to it. If a browser build of the Unreal version matters, set a budget
  now.
- **Overrides can go stale.** If the prototype renames or removes an event,
  its `fx.` row plays nothing. The importer should list rows that match no
  event, kind or slot, as it already does for unknown `kit.` rows.

## 7. What to do, in order

1. Extend the event record (place, direction, strength, thing) and the thing
   record (velocity, state flags) in the rules' host interface, the manifest
   and the bridge. Keep the old calls working until the Unreal project is
   rebuilt.
2. Add `fx.`, `trail.` and `state.` rows to the bridge, with user parameters
   for strength, speed and colour.
3. Have the importer report override rows that match nothing.
4. Record a run's events and things to a file, and let the bridge replay it
   in the editor, so an artist tunes an effect without a rebuild.
5. Build the five effects in section 5.4.
6. Then lighting and post-processing, which need no pipeline change at all.

Steps 1 to 4 are one small change set in `metaverse-engine/export/` and
`metaverse/splice-bots/`. They do not touch the rules' behaviour.

## 8. Decisions for the founders

1. **Is the line in section 3 the rule?** If yes, a request such as "make the
   raider smarter in Unreal" is answered in the prototype, and a request such
   as "make the drill feel powerful" is answered in Unreal.
2. **Who owns `/Game/Splice/Overrides/`?** One person should, so two people
   do not edit the same table.
3. **Does the Unreal web build matter after the jam,** or only the desktop
   build? It sets the size budget for effects and textures.
4. **Stand-ins in the prototype: required or optional?** This audit
   recommends required, one plain shape per exported effect.
