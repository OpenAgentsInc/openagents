# Splice Bots

An unlisted OpenAgents Metaverse Engine game at `/worlds/splice-bots`: a robot smuggler scavenges
banned parts, splices them into a Digger in the rover's edit bay, and restores a scene the
Emperor erased. The public page is `apps/openagents-web/realms/splice-bots/index.md`. This file is
for people and agents who change the code.

## Files

| File | What it holds |
| --- | --- |
| `splice-game.coax` | The game: tuning, world, rules, interface, and checks that play the whole run |
| `front.coax`, `front-checks.coax` | The front door: the title, character creation, the credits, the run this browser keeps, and you drawn as your character |
| `splice-bots.coax` | The page's frame, the still scenes, and the host's exports |
| `splice-battle.coax` | Bot battles, the drill mech, the raider mech, the boss and the ending (below) |
| `splice-cast.coax` | The five characters of the slice |
| `digger.coax`, `digger-mesh.coax`, `digger-gait.coax` | The Digger: the designer's model as a 6,633-triangle table in the kit's flat fills, its rig turned in code (the walk, the standing breath, the sensor's look) and his stride. `tools/digger-rig.py` (Blender) writes the table and `assets/digger-rigged.glb`, the rigged model with idle and walk clips |
| `sound.coax`, `sound-checks.coax`, `tools/sounds.coax` | The sound table, the moments each sound plays, its checks, and the tool that writes the placeholder files (below) |
| `hud.coax` | The HUD and the edit bay in the SPLICE UI v2 frame (#13505): the compass and the objective, pickup toasts, the interact prompt, the gauges that fade while full or cool, the bot slots, the objective card; H hides it. The edit bay's inventory grid, the picked item's card, the stage, the hardware slots and the behavior strip. The compass, fades and toasts are `metaverse-engine/ui/hud-readout.coax` |
| `director.coax` | The Director's systems (#13505): the key map, sneaking, Digger picking up parts by his sensor's range, pointing him with a left click, the inventory grid, Heat's sensing range, the health bars, towing and charging Digger, the map and pause |
| `gpu/splice-bots.coax` | The shaders: Anime Lab's cel pass, the placed pieces' instances, and the ink overlay |
| `splice-face.c`, `splice-ue-face.c` | The C faces for the browser and for a native host |
| `content/splice.content`, `content.coax` | The content tables and their reader |
| `pieces.coax` | Which engine kit piece a row draws, and its measured footprint |
| `layer.coax` | The colliders (a World of entities) and the edit layer |
| `world.coax`, `world-api.coax` | Splice Bots as the world `splice-bots` (once `splice`): its document, its edits as the wire carries them, and what the API calls |
| `layer-draw.coax` | The layer's draw: a mesh for each kind, a row for each placed piece |
| `walk.coax` | The walk replay: the second check of a layer |
| `why.coax`, `why-checks.coax` | The answers to `coax drive ... "why ID"` and what `tree` lists: the diagnostic chain walked from the game's state (#13429) |
| `layer-fixture.coax` | A fixture layer: a piece beside every kept-clear zone |
| `layer-checks.coax`, `walk-checks.coax` | The checks |
| `splice-export.coax`, `splice-run.aescript` | The Unreal exporter and its scripted run |

Run the checks:

```sh
coax/bin/coax test metaverse/splice-bots/splice-bots.coax                 # everything, about 10 seconds
coax/web/tool.sh metaverse/splice-bots/tools/content-check.coax metaverse/splice-bots/content/splice.content --check
coax/remote/remote.sh run -- coax/remote/check.sh module splice   # the module builds inside its budget
```

## State and authority

Splice Bots' row in `metaverse-engine/editor/authority.coax` (#13431,
[what the kinds are](../../docs/architecture/state-and-authority.md)):

| Kind | Who decides |
| --- | --- |
| Content (the content tables, the edit layer) | The realm's owner and the site's admins |
| Simulation (the run in progress) | The player's own page; no server checks it |
| Durability | This browser only: your character (the crew page's record) and the last checkpoint of a run in the mine, in IndexedDB. Nothing reaches a server |

An undo, a content rollback or a layer load changes the content and the run in
progress, and nothing else.

## The front door

The page opens on the title screen (GitHub #13482). Each front screen is the engine's one
screen format (`metaverse-engine/ui/screen.coax`): a dark panel down the left over the live
world, the kicker and title at its top, the rows under them, the key hints at the bottom right.
Rows take the keys (up and down or W and S to move, Enter or Space to pick, A and D or left and
right to change a value, Escape or Backspace to go back) and the pointer (hover focuses, a press
picks).

| Screen | What it does |
| --- | --- |
| Title | The rover drives the rail at dusk and the camera follows the `dolly` table past it. The rows are the `menu` table's: CONTINUE (shown only when this browser keeps a run, with how far it got), NEW RUN and CREDITS |
| Character | Body, outfit, height, skin, hair and face on the crew document, shown on a turntable (drag off the rows to turn it by hand). USE THIS CHARACTER keeps it in the crew page's record (`/worlds/crew` reads the same one) and starts a run |
| Credits | The `credits` table's lines |

There is no SETTINGS row: nothing a setting would change yet. Every sound is off for now (below); a sound
that is on starts with your first click or key on the title screen (the browser's rule).

You play as your character. `metaverse-engine/kit/crew-player.coax` writes ten poses of a walk
cycle from the crew document (the legs and arms swing at the hips and shoulders) into the piece
store after the placed pieces' meshes, once a character; each frame draws one pose as an
instance at the smuggler's place. The smuggler's gameplay is unchanged: the same circle, speed
and stride.

Space jumps. The jump is the engine's (`metaverse-engine/move/jump.coax`, the walker's own rules: a
7.5 m/s press from the ground, 24 m/s^2 down, about 1.1 m high). It lands you on the ground or on
the top of a placed piece (`sl_floors`, asked as the walker asks for the floor under you); a piece
whose top is under your feet lets you over it, and walking off its edge drops you. In the air the
figure holds the walk's widest stride, since the rig has no jump clip. On the hoverboard Space
hops the board, and in the menus and the edit bay it still picks.

Sniper, the rover, is the checkpoint (#13505): standing by it keeps what you carry. Each second in
the mine the checkpoint's inventory and what is open (the rubble, the boss wall, the banned bot, the
core) is written as `splice.run.v1` when it changed. CONTINUE puts you back beside Sniper with them
(at night once the boss is down). SCENE RESTORED and a new run let
it go.

## Controls

The key map follows the slice's beat sheet (#13505). The pause screen (Esc) lists the same keys.

| Key | What it does |
| --- | --- |
| W A S D, mouse | Walk and look |
| Space | Jump; a second Space in the air jumps again, higher (on the hoverboard, hop) |
| Hold Left Shift | Sneak: you walk at 45 % of your speed and Imperial robots sense you from half as far |
| Q | Hoverboard: the first press steps on at casual speed, then each press switches casual and fast |
| Double-tap Q | Hoverboard super fast (the fast speed times 1.6, `board_super_cms`) |
| Hold Ctrl, or double-tap W and hold it | Hoverboard very fast (the fast speed) while held |
| Hold Q | Step off the hoverboard |
| E | Interact: enter the rover, take the core, go through a portal, swap a core, tow a powered-down Digger; beside a hurt Digger with a battery cell, charge him |
| Tab | The edit bay, only inside Sniper (the rover) |
| I | The inventory: six slots. A click on a slot drops what is in it; a click on CELLS charges Digger when he wants it |
| Left click | Point Digger at what is under the crosshair: he digs it, picks it up or attacks it |
| F | Stun a robot within 2 m for 4 seconds |
| 1 to 4 | A bot's battle moves (`d_take_move` hands each press to the battle) |
| Hold R | The bot wheel: move the mouse (or press 1 to 5) to pick DIG, PICK UP, ATTACK, FOLLOW or CHARGE, and let go of R to give Digger the order (`metaverse-engine/ui/wheel.coax`) |
| M | The deep-garage map; M, Esc, E or a click closes it (its "M / ESC CLOSE" hint says so) |
| Esc | Pause; letting go of the pointer pauses too |

A battery cell heals Digger (#13505, playtest 2): E beside him, CHARGE on the bot wheel or a click
on CELLS in the inventory puts one of yours into him, and it charges him full (a powered-down Digger
powers up) over about a second; his BATTERY gauge shows CHARGING and fills as it goes. A spent cell is
gone: dropping cells drops only the ones you still carry.

Digger follows you and picks up every part within his sensor's range (4 m, 8 m with the better
sensor that lies in the scrap room); you can't choose, so you make room in the inventory. A part he
can't fit stays where it is. What you drop he leaves until you have walked away from it. Hardware
spliced into him and cards on his strip are his, not yours.

Heat cools 5 a second at Sniper and nowhere else (not at the stash crate), each banned piece is +25,
and 100 brings the Emperor's sweep. Heat never cools below 25 for each banned piece with you (DRILL+,
the core), and an Imperial robot
senses you from its plan's distance plus 0.07 m for each point of Heat; sneaking halves that.

You turn to face the way you walk; S walks toward the camera rather than backwards. The camera rises
over the drill mech or the boss when one stands ahead of you, and comes in front of one behind you.

Everyone has a health bar: yours top left (your hearts), Digger's under it and over him, and one
over each foe. At zero you are back at Sniper with the inventory you had when you last left it;
banned pieces picked up since are lost. Digger at zero powers down where he is: press E beside him
to tow him, and Sniper's charger charges him back up. A foe Digger fights strikes back at him.

## Battles, the drill mech, the boss and the ending

`splice-battle.coax` (#13505, part C) runs on the Metaverse Engine's bot battles
(`metaverse-engine/action/bot-battle.coax`).

- **Bots.** Digger, the drill mech, Sniper, the raider mech and the boss each have health and a
  battery. Each holds up to four cards, which are its moves: keys 1 to 4 in a battle (the
  Director's `d_take_move` hands each press over). A move costs
  charge (big ones more) and cools down. A bot at zero powers down. Digger's health is the Director's
  (above): his bar, his tow and Sniper's charger. Standing by the charger for 1.5 s repairs Sniper. A
  bar over each battling bot shows its health, and a battle's hits on Digger land on his own bar.
- **Cards.** Digger's cards are the ones spliced onto his strip (MOVE, DRILL, DRILL+), the software
  in his core. The drill mech uses the same cards with its giant arm (DRILL hits twice as hard) and
  adds SLAM and STOMP. Sniper holds MOVE, ARM and FLARE.
- **The drill mech.** It waits in an empty section (landmark 91, else beside the boss wall). E beside
  it swaps Digger's core in: its lenses turn Digger's amber and it follows you; Digger's body waits
  dark. E at the blocked hallway (the boss wall) has it drill through; nothing else breaks it.
- **The raider mech.** It keeps watch beside the drive's second leg (landmark 92, else 12 m off the
  route a third of the way from the rail stop to the garage). Outrunning it is the default: out of a battle
  its claws never take Sniper below a third of its health. When Sniper passes within 16 m it gives
  chase at 16 m/s: faster than Sniper's cruise, slower than its top speed, so holding W outruns it
  (20 m behind, it gives up for the drive). Beside Sniper it claws it every 1.3 s. E while it is
  within 12 m starts a battle with Sniper fielded; in the battle Sniper drives at its slow speed and
  1 to 4 are its cards. 12 m before the garage it turns back. On foot nothing hunts you.
- **The boss.** A dinosaur-shaped bot glowing blue. Its collider is a body in the engine's World that
  moves with it (`sl_body_to`, layer.coax). Its battle begins when you are in its chamber
  with the mech. The mech fights on its own when you leave it be (its strongest ready card every
  couple of seconds); 1 to 4 pick its moves. A hit landing jolts the boss and flashes it, and the
  prompt names her part: when it reels, run to a glowing weak point and press F. Phase one: its tail
  sweeps. Below half health, phase two: it winds up, charges and
  is dazed after. Its three weak points open while it reels from a stunning hit, is dazed, or has no
  health left; stun all three with F (each stays down 12 s) and it powers down, drops its banned core,
  and night falls. You lose if your health or the mech's runs out: back at the checkpoint, the boss
  whole again and the mech repaired at the hallway.
- **The night return.** After the boss, night falls and she goes back to Sniper on foot: the long way
  round the field's edge under the rim, past the bays, or the shortcut across the field through the
  rave, where Imperial 3 and 4 patrol across her way at night. They sense her by Heat (two banned
  pieces: 8.5 m, half that sneaking), so sneaking is optional but helps. A check walks both ways with
  the walk replay's walker, and the run takes the shortcut.
- **The ending.** SPLICE CORE in Sniper's edit bay plays the 15-second memory hologram with its
  three captions, then SCENE RESTORED, then the mission's teaser with RESTART.

## Sound

`sound.coax` holds the sound table: 20 sounds, each an event with a name, a file and a level. The
page plays each by its name from `apps/openagents-web/realms/splice-bots/assets/sounds/NAME.wav`
(`apps/openagents-web/realms.json` lists each as a `sound:` asset). The files there now are
placeholders from the Metaverse Engine's synthesizer (`metaverse-engine/audio/sfx.coax`), each a few
KB. To use a real sound, replace the file and keep its name.

Every sound is off for now. `SND_PLAYS` in `sound.coax` is the switch for each row: false keeps the
sound from the page and from a native host's event, while its moment still runs and is logged for
the checks. Turn a row on to plug its sound back in.

| Sound | When it plays |
| --- | --- |
| `pickup` | A part, a card or a battery cell picked up |
| `drill` | Digger's DRILL card starts on the rubble; the drill mech starts on the hallway |
| `glitch` | A bad cut; the core's memory starting |
| `success` | The rubble opens, the hallway breaks, a fight won, the raider mech down, the core taken, SCENE RESTORED |
| `hit` | You hurt, a foe hit, a stun, a blow in a bot battle |
| `sensor-ping` | The rover's sensor pings and stops it on the rail |
| `slot-click` | Edit bay: a part or card set in its slot; Digger's core into or out of the drill mech |
| `film-whir` | The edit bay opens |
| `rover-door` | Into or out of the rover |
| `robot-alert` | A robot sees you and comes for you |
| `raider-mech` | The raider mech gives chase on the drive |
| `boss-wake`, `boss-charge`, `boss-reel`, `boss-fall` | The boss's battle begins; it charges; it reels; it powers down |
| `hover-casual`, `hover-fast` | Loops: the hoverboard at each speed |
| `rover-lofi` | Loop: inside the rover (its room and the edit bay) |
| `desert-music` | Loop: on the drive and in the desert |
| `rave-loop` | Loop: in the garage at night |

A sound that a message brings is the message's (`snd_for_message`); the rest are watched each step
(`snd_tick`). `sound-checks.coax` checks that each sound plays at its moment in the scripted full run,
and the board, the rave, a robot's alert and the boss's charge and reel in short scenes of their own.
`coax/web/tool.sh metaverse/splice-bots/tools/sounds.coax --log` prints the run's sounds in order;
`--write` writes any missing placeholder, `--check` fails when a file or its `sound:` asset is missing.

Each sound is also an event (`splice_events`, number 200 plus its row), named in the Unreal bundle's
manifest, so a row `event.NAME` of the Unreal host's overrides table plays a sound there. A loop's
event is its start.

## Edit layer

You can place things in Splice Bots without changing its code. A placed piece is data: a row of a
content table, an entity in the engine's World, and a typed edit you can undo.

### Content tables

`content/splice.content` is an `aethyr.content.v1` document
([profile](../../docs/protocol/aethyr-content-v1.md)). Edit it with the Metaverse Engine's table
editor:

```sh
coax/web/tool.sh metaverse-engine/tools/content-edit.coax metaverse/splice-bots/content/splice.content row landmarks '16|Dance floor|-160|-5120|800'
coax/web/tool.sh metaverse/splice-bots/tools/content-check.coax metaverse/splice-bots/content/splice.content --carry
```

The second command checks the tables against Splice Bots' rules and writes `content/shipped.coax`,
the copy the page is built with. Run it after every edit.

| Table | What a row is |
| --- | --- |
| `pieces` | A placeable piece: a stable content ID, a name, the engine kit piece it draws, its size, its look, its footprint and its height |
| `looks` | Three colours of Splice Bots' palette. A table of its own, because a content table has eight columns at most |
| `landmarks` | A named place in centimetres, with a radius. The game reads its places from here, not from constants |
| `kept_clear` | A zone the run needs: a box, grown by a radius. Nothing may be placed in one |
| `areas` | Where a piece may stand at all: the dome village, the gate corridor, the rail stop, the mine, the desert of the solar farm, the room inside the rover and the garage yard |
| `menu` | A title row: its key (`continue`, `new` or `credits`) and its label. The rows' order is the table's |
| `credits` | A line of the credits screen |
| `dolly` | A point of the title camera's path past the rover: the eye and where it looks, in centimetres in the rover's frame, and seconds to the next point |
| `portals` | A way from one place to another: E at any placed piece of a kind (`kind K`) or at a point (`at X Z`) takes the player to `X Z`, facing a yaw. A vehicle that opens onto a room inside it is a portal from its kind ([the module](../../metaverse-engine/world/portal.coax)) |
| `starts` | Where a player starts at a time of day. The row with the lowest ID is where a new run starts, at its time of day; a respawn at a time of day with a row goes to that row ([the module](../../metaverse-engine/world/flow.coax)) |
| `beats` | Where the day moves on: standing in an area while it is `from` makes it `to`, never earlier |

As shipped, a new run starts at the solar farm by day. E at the rover train goes into the room
inside the rover: a closed cabin of its own, 12 m by 9 m (`drive.coax` `dv_room`), with a floor,
four solid walls, a ceiling and a painted desert behind its windows, so nothing of the world shows
and no one walks out. E at its door goes back out beside Sniper. Inside, the camera stays close and
under the ceiling. E at the desk there starts the drive to the garage, and the garage yard turns
the day to dusk. Tables saved before the `portals`, `starts` and `beats` tables get them empty,
so such a world keeps its older opening in the dome village (#13505).

Splice Bots now has 16 tables, the most a content document holds. A new kind of row goes in an
existing table or needs the format's limit raised first.

Edits and saves name a piece by its content ID. Names are for people and agents to read.

A footprint is how far a piece's body reaches to each side and ahead, below 2 m. A check measures
every piece's triangles and compares them with its row, so a collider is never wider than what
you see. The collider is the stadium inside that footprint, so it turns with the piece.

### Foes' plans

The foes move by data, not by code (#13466). The table `plans` gives each of the nine actors
(`actors`: Raider 1 to 5, Imperial 1 to 4) a plan for dawn, day, dusk and night, rule-based with
no model call at run time ([the module](../../metaverse-engine/ai/npc-plan.coax),
[its steps](../../metaverse-engine/ai/plan.coax)):

| Column | What it holds |
| --- | --- |
| `do` | `stand`, `dance`, `sleep`, `guard`, `patrol`, `wander` or `follow` |
| `route` | Points in centimetres: one for stand, dance, sleep and wander (`X Z R`), the post and where it looks for guard, 2 to 8 for patrol, `foe N` for follow |
| `speed`, `notice` | Centimetres a second, and how near you come before it notices (0: never) |
| `on` | What it does on noticing you: `chase`, `call` (the others near it chase too), `flee` or `ignore` |

The shipped rows keep the raiders dancing and the Imperial robots patrolling by the rave, except at dusk
(#13505): then every foe's plan stands it out on the road beyond the garage (the raiders scavenging, the
Imperial robots recharging at the solar farm), so the garage is quiet. A plan whose every point lies
outside the ring and its bays (`g_plan_out`) has the foe out of the garage: it isn't drawn, senses no one,
and can't be pointed at or stunned; going out or coming back in, it is where its plan starts. The table `clock` holds
the editor's time of day (`story` in play). A route stays in the mine or in an area added after it
(ID 5 and up, such as a desert), and a point inside a wall
is refused with the nearest place a foe can stand. `world.coax` checks both on every cell edit.

An agent edits plans with the realm commands (`openagents realm world-foes`, `world-plan`,
`world-route`, `world-time`; [the table](../../docs/protocol/world-edit-v1.md)). A behaviour the
plans cannot express goes in a realm script first: `scripts/raider-desert.coax` makes raiders
hostile only while the player is in the desert (`coax/web/script.sh`, then `world-script-compile`
and `world-script-run --for "kind 1"`). Anything else the
plans cannot express is a change to `splice-game.coax` (`g_foes_step`) on a dev server, which
rebuilds and swaps the engine on save and keeps the world's plans and state.

### The hoverboard

The table `board` gives the player a hoverboard (#13485): one row, `on` or `off`, and its casual
and very fast speeds in centimetres a second. Turn it on or off with the realm tooling, not in code:

```sh
openagents realm world-board splice on --casual 700 --fast 2200 --origin http://127.0.0.1:PORT
```

The editor's agent does the same with the `realm_world_board` tool. As shipped it is on (#13505). In
play, everything is on Q: Q calls the board up at casual speed, each Q after switches casual and
fast, a double tap of Q (within 0.3 s) rides super fast, and holding Q for half a second steps off.
Holding Ctrl, or a double tap of W held, rides very fast too. W and S speed up and slow, A and D carve, and Space hops. The board is the Metaverse Engine's (`metaverse-engine/action/hoverboard.coax`, on
its mount); Splice Bots' own colliders push it, and the camera widens, pulls back and shakes at speed,
with Anime Lab's speed lines.

### Edits

`layer.coax` holds the layer as a scene in the engine's edit log (`metaverse-engine/editor/edit.coax`). Places
are whole centimetres and turns are whole degrees.

| Call | What it does |
| --- | --- |
| `sl_place(g, piece, x_cm, z_cm, deg)` | Places a piece. Returns its item ID |
| `sl_move(g, id, x_cm, z_cm)`, `sl_turn(g, id, deg)` | Moves or turns a placed piece |
| `sl_remove(g, id)` | Removes one |
| `sl_undo(g)`, `sl_redo(g)` | Walk the log |
| `sl_why(g, verdict, out)` | A refusal in words |

Each edit is checked before it is taken. An edit is refused when the piece:

- has no row in `pieces`;
- reaches past the map's edge (it must stand inside one area);
- reaches into a kept-clear zone (the refusal names the zone);
- stands on the player;
- stands inside a wall, the rover, the banned bot or another placed piece.

A placed piece is scenery. It blocks you, the party's people and the camera, and it has no AI and
no hit points.

### The world `splice-bots`

Splice Bots is the first world with an ID ([edits](../../docs/protocol/world-edit-v1.md),
[the document](../../docs/protocol/world-doc-v1.md), [dev and live](../../docs/protocol/world-live-v1.md)).
`world.coax` turns a wire edit into the calls above and answers its inverse. The API
(`/api/worlds/splice-bots`) runs the same file on every batch, then the walk replay, and saves the
document a fresh game builds from the pieces. Only an admin writes it.

| Call | What it does |
| --- | --- |
| `spw_edit(g, e, sure)` | One wire edit on the layer. `sure`: the server saved it already, so a page takes it where it stands |
| `spw_batch(...)` | A batch on a document: the new document, the batch as taken and its inverse, or the refusal |
| `spw_open(g, k, text)`, `spw_save(g, k, out)` | A document read into a game and written from one |
| `spw_page_open`, `spw_page_answer` | A page follows the world: the document once, then each saved edit in order. The run is not touched |

Open `/worlds/splice-bots?edit` on the dev server to join the dev lane. An edit from another tab or
from outside shows in under a second, and a placed piece rises out of the ground.
`coax/web/tool.sh world-tabs http://127.0.0.1:PORT` checks it with two headless tabs.

### How it is built

- **Colliders.** Everything that blocks is an entity in an engine World (`metaverse-engine/core/world.coax`):
  the 30 walls, the rubble and the boss wall, the banned bot and every placed piece. A step asks
  the World's spatial index for what stands near, and the engine's walking rules push the walker
  out. No list is scanned.
- **Order.** The World's placed pieces are built from the layer's items in ID order each time the
  layer's revision changes. `sl_digest` is the World's digest plus Splice Bots' shape columns, so two
  games given the same edits compare equal a step at a time.
- **Draw.** Each kind's mesh is written once and stays on the GPU. A placed piece is one row of a
  small store: where it stands and its turn. A kind's placed pieces are one instanced draw. An
  edit writes rows. It doesn't write the designed world's buffer or any mesh.

### The walk replay

`walk.coax` is the second check, made when a layer is saved. A walker goes the run's whole way on
foot, 40 legs from the solar farm to the banned bot, against the proposed layer. It uses the
engine's steering service to go round things and the game's own step and collision. It refuses a
layer when:

- a place the run starts you in is taken;
- a place the run needs can't be reached;
- something stands on the rail line or in the Digger's lane;
- something stands in the rubble or the boss wall.

The mutant check takes each of the 29 kept-clear zones out of the tables in turn, places a blocker
where that zone was, and expects the walk to refuse.

### See placed pieces

The dev server loads the fixture layer (28 pieces, one beside every kept-clear zone with open
ground beside it):

```sh
coax/web/serve.sh apps/openagents-web
# then open http://127.0.0.1:8800/worlds/splice-bots?layer=fixture
```

Only an address on this computer (`127.0.0.1` or `localhost`) loads it. On any other address the
page ignores `?layer=fixture`.

### In the Unreal host

The Unreal host draws the placed pieces and you as the page does: `aethyr_instanced`
(`metaverse-engine/export/unreal/aethyr_host.h`) hands it each kind's mesh in the piece store and the
place store's rows, and it puts the mesh at each row. Run it with `-AethyrWorld=` and the page's
address and it follows that world's saved edit layer as the page does, through the world's
requests (`aethyr_request`):

```sh
~/UnrealEngine-5.9/src/Engine/Binaries/Mac/UnrealEditor ~/UnrealProjects/SpliceExport/AethyrHost.uproject \
  /Game/Aethyr/Splice/Generated/L_Splice -game -AethyrWorld=http://127.0.0.1:8800/worlds/splice-bots
```

A host that draws you itself hides the smuggler (`aethyr_thing_hide`), and the page then does not
draw your character.

#### The jam build

The jam entry is an Unreal web build (UE 5.9's Wasm target) of slice 1 that plays with no server. By
default it is the shipped world (a new world: the farm opening, `content.coax`); `splice-export.coax
NEW_DIR WORLD_DIR` bakes a saved world instead, writing its live document and content tables into
`rules/aethyr_world.h`, and `splice-ue-face.c` answers the page's world requests from them.
`splice-run.aescript` plays slice 1 from the title to SCENE RESTORED (the farm, the edit bay, the drive,
the rubble, the garage, the drill mech, the boss, the core); the export runs it natively and the import
checks Unreal prints the same lines.

One command does the whole build, from the repository root of a checkout of `origin/main`:

```sh
git fetch origin && git worktree add /Users/christopherdavid/work/oa-splice-final origin/main
cd /Users/christopherdavid/work/oa-splice-final && coax/build.sh
metaverse/splice-bots/tools/jam-build.sh ~/UnrealProjects/out-slice1/final
# a saved world instead: a folder with its splice-bots.live and splice-bots.live.tables (a dev server's build/.worlds)
# metaverse/splice-bots/tools/jam-build.sh ~/UnrealProjects/out-slice1/final WORLD_DIR
```

The script exports, imports into `~/UnrealProjects/SpliceSlice1`, puts the sounds in
(`tools/ue-sounds.py`), plays the scripted run inside Unreal, packages the web build into `OUT/web`
(the playable folder is `OUT/web/Wasm`), plays the run in headless Chrome at 1440x900 (fixture
`splice-play` in `metaverse-engine/export/unreal/web-fixtures.json`), then real keys (fixture
`splice-keys`: a click on the title, a new run, the farm on foot with Digger, Q, I, M and Esc; shots in
`OUT/keys`), and fixture `splice-digger` (the mouse turns the view to Digger at the farm: `OUT/digger/turn12.png`), and zips the playable folder as `OUT/SpliceBots_Splicebots.zip`, refusing one of 1 GB or
more. About 15 minutes on a quiet Mac.

- `~/UnrealProjects/SpliceExport` is the earlier Tier 1-2 build, kept as it was (its web build is in
  `Saved/WasmOut`). `SpliceSlice1` is its copy for slice 1; its `Config/DefaultEngine.ini` names the
  page (Splice Bots) and its only splash image, `Build/Wasm/Branding/splash.png`, is the Unreal Engine logo.
- The import's fidelity checks fail on a few welded vertices of the far mesas and the boss-chamber
  reference shot since the garage moved the world out to 2.9 km. The script accepts those two and no other.
- Sound: `tools/ue-sounds.py` imports each `realms/splice-bots/assets/sounds/NAME.wav` as
  `/Game/Splice/Sounds/NAME` and adds the overrides row `event.NAME`, which the bridge plays when the
  game reports that sound. The page starts sound on the first click or key (the title's click). A loop
  (a bed) has no stop event on this host, so it plays once from its start. `OUT/sounds-played.txt`
  counts what the two browser runs played.
- No Windows build: this Mac has no Windows toolchain or Win64 engine binaries, and Unreal does not
  cross-compile Win64 from macOS.

### Not built yet

The saved world, its API, the command line and the editor with the conversation in its foot are
built: see [the browser editor](../../docs/editor/2026-11-12-browser-editor.md).

- The Unreal host does not read the world's import pack, so a kind whose mesh is an import draws
  nothing there. It draws every other placed piece and you (below).
- The walk is a rigid swing of the legs and arms at the hips and shoulders: no knee or elbow
  bend, no skinning, and the feet stay flat. The smuggler's own animation does not drive the
  crew figure's joints.
- The boss chamber has no piece beside it in the fixture, because no ground beside it takes one:
  rock and the boss wall are all that is there. A check holds that (`walk-checks.coax`).
- A turned piece's round parts (a machine's balls and masts) turn their facets with the piece,
  because a kind's mesh is written once and each piece is an instance of it. The shape and the
  middle are the same either way.
