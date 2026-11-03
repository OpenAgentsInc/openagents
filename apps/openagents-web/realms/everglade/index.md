# Everglade

Everglade, the forest city, at https://openagents.com/worlds/everglade. The whole city at its real size, built from art kits: timber and plaster houses with tiled roofs inside a log palisade, standing in a forest of kit trees. Seven of its buildings can be walked into. The plan is https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade.md and its map https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade-map.svg.

## What is here

- The road in from the south, past the stables, to the South Gate in a timber palisade. Three more gates lead out: east to the deep wood, west to the river crossing, north to the ruins and the crypt.
- The commons in the middle: the fountain and the town crier.
- The market to the west: six stalls, the general store, the smithy and the alchemist.
- The inn on Inn Street, between the gate and the commons.
- Rooms you can walk into: the inn (tables, a hearth, a stair), the general store, the smithy, the alchemist, the fighters' hall, the rangers' lodge and the temple. Their doors open and shut with E.
- Guild Row to the east: the fighters' hall and the rangers' lodge; the mages' tower stands to the north-east.
- Temple Hill to the north: the temple and the observatory.
- The forest all round: past a strip of open grass outside the walls the trees stand too thick to walk into, so the only ways out are the trails, to clearings at the stables, the deep wood's waystone, the ruins, the crypt and the river's far bank, and the river's glade by the bridge. Bushes, ferns and rocks line the trees' edge.
- The river past the West Gate runs the length of the world in a carved bed. Cross by the bridge, or wade and swim across in the glade beside it: half speed in the water, the current carries you, hold C to dive. Under water you hold your breath for a minute; then you tire, and if you stay down too long you come to on the bank. Ducks and lily pads float on it.
- You walk as the Ranger, the hooded green ranger you were in the previous Everglade, with its walk, run, jump and idle.
- The city's people, as plain figures, and three cats: the gate guards Bram and Tilda, Pip the stable hand, Oswin the crier, Marta at the inn, Hollis at the general store, the stallholders Fern, Ada and Quill, Garrick the smith, Isolde the alchemist, Rook and Sable of the guilds, Elowen the archmage, Brother Anselm at the temple, Vega the astronomer, Moss the woodwarden, Old Tam the ferryman, Corin among the ruins, and the cats Pebble, Soot and Wisp.

## Talking to people

Walk up to anyone and press E (on a phone, tap TALK). Type what you want to say and press Enter; they answer as themselves. Esc or BYE ends it. They remember you: tell Marta your name or what you like, come back another day, and she can bring it up. What you say to one of them is kept for that one only, against this browser's player key (the same one the Haunted House uses).

Names float over each place as you come near. The city is about 130 metres across inside its walls: half a minute's walk from the South Gate to the temple.

## Playing with others

- Everglade is one world for everyone in it: the room server runs it, and each instance holds up to 16 players. You join the lowest instance with room.
- INVITE, top right, copies a link to your instance. A friend who opens it joins you there, or the next instance with room when yours is full. `?room=everglade-3` names an instance; `?with=PLAYER_ID` joins the instance that player is in.
- CHARACTER, beside INVITE, picks your skin tone and the colours of your cloak and trim. On your first visit it opens before you enter. Everyone plays the druid for now. A guest gets a two-word name; [log in](/login) to use your account's name.
- The Everglade gate on [the Grid](/worlds/the-grid) brings you here, into the instance of the friend you walked there with.

## Controls

- Computer: W A S D move you at a run, Shift walks, Space jumps, C dives in the river, drag the mouse or the arrow keys turn the camera, the wheel zooms, M (or the MAP button) lifts the camera over the whole city and back.
- Phone: your left thumb walks, your right thumb turns the camera, a tap jumps, MAP at the top right shows the city from above.
- A reload puts you back where you were, facing the same way, inside or out. Logged in, the same place follows you to another device. A link with a place in it (`#commons`, `#in-inn`) starts you there instead.
- A link can start you beside someone, near enough to talk: `#marta`, `#old-tam`, `#pebble` and so on, by their name.
- A link can start you at a place: `#commons`, `#market`, `#guilds`, `#temple`, `#observatory`, `#inn`, `#road`, `#deep-wood`, `#ruins`, `#crypt`, `#river`, `#gate`, `#north-gate`; inside a building: `#in-inn`, `#in-store`, `#in-smithy`, `#in-alchemist`, `#in-fighters`, `#in-rangers`, `#in-temple`, `#in-mages`, `#in-observatory`, `#in-stables`, `#in-house-1` to `#in-house-6`; `#above` opens on the map.
- Players can't change Everglade. An admin logged in at /login gets EDIT by MAP (B): a hotbar of crates, barrels, benches, fences and stalls (1 to 5), a click or tap picks a placed thing to move (arrows), turn (R), resize (+ and -) or delete, or one of the city's people or cats to delete, with UNDO (Z), REDO (Y), ORIGINAL (the designed city back, everyone in it) and DONE. An admin also gets TALK, left of EDIT (hold V, or press and hold on a phone) and edits by voice: "put two barrels here", "remove that", "remove these crates", "remove Marta", "undo". Everyone sees the edits.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/everglade/plan.coax` is the city's layout: the palisade, every building and prop as a box, the streets and trails, the squares, where each character stands, and the checks that everything is reachable on foot, nothing overlaps and the paths connect.
- The people are `verse/everglade/people.coax`: who stands at each spot, their role, greeting and what they know of the city. Talking goes to the site's character chat (`/api/playtest/npc/ask`, Gemini on Vertex AI), which keeps what each player said to each character and tells the character on their next visit (`coax/web/api/migrations/0031_npc_memory.sql`).
- `apps/openagents-web/everglade5e.coax` is the page: the city (`everglade.coax`) with the SRD 5e rules beside it (`everglade5e-rules.coax`, its own C unit, on the engine in `aethyr/rules/srd/`) and the hotbar layer it shares with 5E MMO (`fivebar.coax`). `everglade5e-face.c` binds them for the browser, `everglade.wgsl` shades it. The river is `verse/everglade/river.coax` on the water system (`verse/water.coax`, `verse/water-look.coax`; https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/water.md). The ground and paths are `aethyr/terrain.coax`, the camera `aethyr/camera.coax`, walking `aethyr/player.coax`, and a phone's thumbs `verse/grid.coax`, as in the Garden.
- The forest's trees, bushes, ferns and rocks are from the CC0 Quaternius Stylized Nature kit, and the walls, roofs, doors, windows, palisade and furniture from the CC0 Quaternius Medieval Village and Fantasy Props kits, all in one asset pack (`everglade.pack`, about 8.4 MB) streamed when you enter, with the Ranger: Quaternius's CC0 Universal male Ranger, taken whole from the previous Everglade's pack, its clips from the CC0 Universal Animation Library, played by `aethyr/skin.coax` and skinned on the GPU; the full list is [licences.md](licences.md). `verse/tools/pack_realm.py` compiles it from `verse/packs/everglade.json`. The trees nearest you are the kit's; farther ones are simple cones, so a phone keeps its frame rate. Each building's walls, doors and rooms are worked out from its footprint in the plan, which checks every room can be walked into and out of.
- The spell bar along the bottom (on a phone, above the thumbs' sticks) holds Meteor Swarm, Thunderbolt, Mega Thunderbolt and Water Orb on 1 to 4, Reverse Gravity, Wind Wall, Levitate (hold, X sinks), Feather Fall, Gust of Wind and Thunderwave on 5 to 0, and Rebuild and Long Rest on R (the town stands again, and you rest: all your hit points back, your conditions gone, the watch as it was). Its second row, on Shift as the old Grove's bar (Ctrl and Alt reach the third and fourth; with Shift held too where the browser keeps Ctrl or Alt and a number for its tabs), holds Wall of Stone, Telekinesis, Black Tentacles, Ice Knife, Ice Storm and Sleet Storm on Shift+1 to Shift+6 (the ice they leave is slippery, on the ground and on the water, and their shards and hail fall as things of their own), and Water Walk, Control Water (flood, part, redirect, whirlpool: the river and the ponds, their rowboats and ducks with them), Create or Destroy Water and Water Breathing on Shift+7 to Shift+0, and Moonbeam on Shift+-. Its third row, on Ctrl, holds the druid's cantrips and first-level spells from the old Grove: Produce Flame (a light in your hand that you hurl), Starry Wisp, Shillelagh (your staff), Poison Spray, Elementalism (air, earth, fire or water, in turn) and Entangle on Ctrl+1 to Ctrl+6, then Faerie Fire, Healing Word, Spike Growth and Call Lightning on Ctrl+7 to Ctrl+0 and Wall of Fire on Ctrl+= (the townsfolk walk round it; fire on the grass spreads until water puts it out), and Conjure Animals (a pack of spectral wolves) on Ctrl+-. Its fourth row, on Alt, holds Polymorph (them a frog, or you a brown bear), Mass Cure Wounds, Sunbeam, Wall of Thorns and Fire Storm on Alt+1 to Alt+5, then Sunburst, Storm of Vengeance, Shapechange (a mammoth, a bear, a wolf), Speak with Animals (the cats and ducks have their say) and Wild Resurgence (a wolf) on Alt+6 to Alt+0, and Ray of Frost and Fog Cloud on Alt+- and Alt+=; Hold Person sits on = and Cone of Cold on Shift+= (the mouse wheel zooms); a wide screen stacks the rows, a phone shows one with a switcher at its end. Rest the pointer on a slot, or hold a finger on it, for its card. Its icons are from game-icons.net (CC BY 3.0, icons made by Lorc, Delapouite, Carl Olsen and Zeromancer), credited in [licences.md](licences.md).

## The spell bar on the SRD 5e rules

- You are a 9th-level Druid: AC 15, 57 Hit Points, spell save DC 15, +7 to hit with spell attacks.
- Every spell on the bar can always be cast. Nothing on the bar runs out: there are no spell slots, actions or turns to spend. While a cast is on its way, its icon dims from the bottom up until the spell strikes.
- What a spell does to a creature is the engine's: it rolls the attack against Armor Class or the saving throw, the damage dice and the conditions, and the number shows over the creature's head. Its fire, smoke and lightning are Everglade's.
- The engine keeps everyone's Hit Points. A guard's sword, a spear or the river hurts you through the engine, so it meets your Temporary Hit Points and your concentration.
- Everglade's people are their SRD stat blocks: Commoners, Guards, and the watch's Warrior Veterans, Mages, Knights, Priests, Archmages and Gladiators.
- What holds you shows on you and in a short line under your Hit Points: golden rings for Hold Person (HELD), rock tendrils round your shins for the watch's Earthbind (BOUND: you can't rise or fly), frost motes for Ray of Frost (SLOWED), and the engine's conditions (Restrained, Grappled, Prone and the rest) while they last.
- A spell that lasts (Moonbeam, Black Tentacles, Call Lightning, Wall of Fire and others) runs again on whoever is in its area each 6-second round while you keep your concentration. Putting it away ends your concentration.
- R stands the town up again, gives you a long rest and clears your WANTED stars.
- The frame rate and frame time show small at the bottom right.

## The spells

Spells from the SRD are their SRD records. Everglade's four aimed spells are house rules, marked as house rules in the engine and on their card.

| Slot | Spell | How it resolves |
|---|---|---|
| Meteor Swarm | House rule, 9th level | Dexterity save, DC 15, for half: 20d6 Fire and 20d6 Bludgeoning in a 25-foot sphere |
| The Thunderbolt | House rule, 7th level | Dexterity save for half: 20d10 Lightning in a 10-foot sphere |
| The Mega Thunderbolt | House rule, 9th level | Dexterity save for half: 100d10 Lightning in a 20-foot sphere |
| The Water Orb | House rule, 3rd level | Dexterity save for half: 6d10 Bludgeoning in a 10-foot sphere, and a failed save is pushed 10 feet |
| Reverse Gravity | 7th level, concentration | Dexterity save |
| Wind Wall | 3rd level, concentration | Strength save for half: 4d8 Bludgeoning |
| Levitate | 2nd level, concentration | You |
| Feather Fall | 1st level, a Reaction | You |
| Gust of Wind | 2nd level, concentration | Strength save or pushed 15 feet |
| Thunderwave | 1st level | Constitution save for half: 2d8 Thunder, pushed 10 feet |
| Wall of Stone | 5th level, concentration | The wall |
| Telekinesis | 5th level, concentration | Strength save or Restrained |
| Black Tentacles | 4th level, concentration | Strength save or 3d6 Bludgeoning and Restrained, each round |
| Ice Knife | 1st level | Spell attack: 1d10 Piercing; then a Dexterity save or 2d6 Cold within 5 feet |
| Ice Storm | 4th level | Dexterity save for half: 2d10 Bludgeoning and 4d6 Cold |
| Sleet Storm | 3rd level, concentration | Dexterity save or Prone, each round |
| Water Walk | 3rd level | You |
| Control Water | 4th level, concentration | Strength save or 2d8 Bludgeoning, each round |
| Create or Destroy Water | 1st level | The water |
| Water Breathing | 3rd level | You |
| Moonbeam | 2nd level, concentration | Constitution save for half: 2d10 Radiant, each round |
| Produce Flame | Cantrip | The flame to hand (a Bonus Action), then a spell attack: 2d8 Fire |
| Starry Wisp | Cantrip | Spell attack: 2d8 Radiant |
| Shillelagh | Cantrip, a Bonus Action | You |
| Poison Spray | Cantrip | Spell attack: 2d12 Poison |
| Elementalism | Cantrip | No roll |
| Entangle | 1st level, concentration | Strength save or Restrained |
| Faerie Fire | 1st level, concentration | Dexterity save or outlined |
| Healing Word | 1st level, a Bonus Action | Heals the friend most hurt: 2d4 + 3 |
| Spike Growth | 2nd level, concentration | 2d4 Piercing to each creature in it, each round |
| Call Lightning | 3rd level, concentration | Dexterity save for half: 3d10 Lightning on the one nearest the middle, each round |
| Wall of Fire | 4th level, concentration | Dexterity save for half: 5d8 Fire, each round |
| Conjure Animals | 3rd level, concentration | Dexterity save or 3d10 Slashing, each round |
| Polymorph | 4th level, concentration | Wisdom save or Incapacitated |
| Mass Cure Wounds | 5th level | Heals up to six friends within 30 feet: 5d8 + 3 |
| Sunbeam | 6th level, concentration | Constitution save for half: 6d8 Radiant, and Blinded on a failure |
| Wall of Thorns | 6th level, concentration | Dexterity save for half: 7d8 Piercing, each round |
| Fire Storm | 7th level | Dexterity save for half: 7d10 Fire |
| Sunburst | 8th level | Constitution save for half: 12d6 Radiant, and Blinded on a failure |
| Storm of Vengeance | 9th level, concentration | Constitution save or 2d6 Thunder and Deafened; then a Dexterity save for half of 10d6 Lightning on one creature each round |
| Shapechange | 9th level, concentration | You |
| Speak with Animals | 1st level | You |
| Wild Resurgence | A Bonus Action | A spell slot for a use of Wild Shape, and Wild Shape at once: Temporary Hit Points equal to your level |
| Ray of Frost | Cantrip | Spell attack: 2d8 Cold |
| Fog Cloud | 1st level, concentration | The cloud |
| Hold Person | 2nd level, concentration | Wisdom save or Paralyzed, a save again each round |
| Cone of Cold | 5th level | Constitution save for half: 8d8 Cold in a 30-foot cube from you |

Cantrips roll two dice, as a 9th-level caster's do.

## Make your own world

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
