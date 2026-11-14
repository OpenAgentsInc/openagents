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
| `digger.coax`, `digger-mesh.coax`, `digger-gait.coax` | The Digger: the designer's model, 29,998 triangles with smooth normals as a compact bit stream in the kit's flat fills, its rig turned in code (the walk, the standing breath, the sensor's look) and his stride. `tools/digger-rig.py` (Blender) writes the table and `assets/digger-rigged.glb`, the rigged model with idle and walk clips |
| `sound.coax`, `sound-checks.coax`, `tools/sounds.coax` | The sound table, the moments each sound plays, its checks, and the tool that writes the placeholder files (below) |
| `hud.coax` | The HUD and the edit bay in the SPLICE UI v2 frame (#13505): the compass and the objective, pickup toasts, the interact prompt, the gauges that fade while full or cool, the bot slots, the objective card; H hides it. The edit bay's inventory grid, the picked item's card, the stage, the hardware slots and the behavior strip. The compass, fades and toasts are `metaverse-engine/ui/hud-readout.coax` |
| `camera.coax`, `camera-checks.coax` | The follow camera behind her on foot and on the hoverboard (below), and its checks |
| `digger-orders.coax` | Commanding Digger (#13552): the reticle's selection and command, his answer to each order, the marker on his target, the drill card's windup, bite, shudder and break, and where he looks (see Commanding Digger) |
| `feel.coax` | How she moves (#13552): the walk, the jump's coyote time and buffer, her air poses, squash and lean, the movement demo, and the tunables and level metrics (see Movement and feel) |
| `director.coax` | The Director's systems (#13505): the key map (Shift is taken and does nothing), Digger picking up parts by his sensor's range, pointing him with a left click, the inventory grid, Heat's sensing range, the health bars, towing and charging Digger, the map and pause |
| `gpu/splice-bots.coax` | The shaders: Anime Lab's cel pass, the placed pieces' instances, and the ink overlay |
| `splice-face.c`, `splice-ue-face.c` | The C faces for the browser and for a native host |
| `content/splice.content`, `content.coax` | The content tables and their reader |
| `buildings.coax` | How the buildings look (#13552): printed concrete from `metaverse-engine/kit/printed.coax`. The garage's outer shell steps down to the desert in printed courses, its entry is a bay with a raised roll-up door, raider banners with a cog hang over the rim, and the village's ring wall is printed. Stills 40 to 42 (the entry, the shell, the village). It only draws; where things stand is the level's |
| `farm.coax` | The solar farm's field (layout per the level spec, #13552): four long rows of three lines of solar banks in the farm's north-west, 13 m aisles between them, one instanced draw (`metaverse-engine/kit/props.coax` `kit_solar_bank`); she is held out of a line below its glass and lands on its tilted top (`ff_floors`); the low sun, its long shadows and the dust haze at the farm by day |
| `plain.coax` | The desert between (#13552): the ground, the plain's edge (orange-flagged stakes, `MSG_EDGE`), the three landmark rocks she climbs and boards over (`land_mound_y`, the numbers their mesh is written from), the mountain's two peaks north of the garage, the Imperial robots' charging pads at the farm, and the garage's rim held as a wall from outside |
| `metrics.coax` | The grey metrics room, off the map (dev stage 40): her height and jump, walk, sneak and board speeds, Digger's sensor and an Imperial robot's sensing by Heat, and Sniper, the drill mech and the boss at size, each read from the constant the game plays by |
| `level-review.md` | The level design review (#13552): ranked items, built and proposed |
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
cycle and four air poses from the crew document (the legs and arms swing at the hips and
shoulders) into the piece store after the placed pieces' meshes, once a character; each frame
draws one pose as an instance at the smuggler's place.

Space jumps, and a second Space in the air jumps again. The jump is the engine's
(`metaverse-engine/move/jump.coax`) with the numbers in Movement and feel below. It lands you on
the ground or on the top of a placed piece (`sl_floors`, asked as the walker asks for the floor
under you), including the farm's solar rows, whose tilted tops are sloped floors. A top within
0.3 m of your feet you step onto. Walking off an edge drops you. On the hoverboard Space hops the
board, and in the menus and the edit bay it still picks.

### Movement and feel

She is light and quick (#13552): she reaches full speed in about a tenth of a second, reverses as
fast, stops within 0.3 m, and steers well in the air. Nothing waits for an animation: a landing is
drawn while your keys already move her. `feel.coax` holds every number:

| Constant | Start | Why it fits |
| --- | --- | --- |
| `FL_ACCEL` | 52 m/s^2 | 0 to walking speed in about 0.11 s: a press moves her at once |
| `FL_TURN` | 16 /s | a reversal takes about 0.1 s, with no skid |
| `FL_BRAKE`, `FL_FRICTION` | 26 m/s^2, 9 /s | letting go stops her in about 0.12 s, within 0.3 m, so she stops where you aim |
| `FL_AIR` | 0.75 | three quarters of her ground control in the air: you can steer a jump onto a panel |
| `FL_AIR_FRICTION`, `FL_AIR_BRAKE` | 4 /s, 12 m/s^2 | letting go in the air stops her drift in about 0.17 s, so she lands where you aim |
| `FL_FACE` | 18 /s | her body turns to her heading in a few frames |
| `FL_GRAVITY` | 30 m/s^2 | a quick, compact arc rather than a floaty one |
| `FL_JUMP` | 9 m/s | a 1.28 m jump, about 0.6 s in the air; with `FL_STEP` it reaches a solar row's top (1.34 m) |
| `JUMP_AIR` (engine) | 1.15 | the second jump lifts harder than the first, so it goes higher |
| `FL_CUT` | 0.45 | let go of Space while rising and she keeps this share of the rise: a tap hops about 0.3 m, a hold goes the full jump |
| `FL_COYOTE` | 0.1 s | Space just after walking off an edge still jumps from it |
| `FL_BUFFER` | 0.12 s | Space just before landing, with no air jump left, jumps on landing |
| `FL_STEP` | 0.3 m | small edges are stepped onto, never snagged on |
| `FL_SWING`, `FL_STRIDE` | 0.6 rad, 1.9 m | the gait's swing; the stride is measured from the character's own legs, so her feet keep pace with the ground and never slide |
| `FL_KNEE` | 1.1 rad | a knee folds as its leg comes through, so the foot lifts behind and the run reads light and springy; both knees are straight when she stands |
| `FL_STOP`, `FL_SETTLE` | 0.4 m/s, 14 /s | below that speed with no key down she eases into standing |
| `FL_LEAN`, `FL_LEAN_MAX` | 0.035, 0.2 | she leans into turns, by turn rate times speed |
| `FL_PITCH`, `FL_PITCH_MAX` | 0.006, 0.18 | she leans forward to start and back to stop |
| `FL_LEAN_EASE` | 14 /s | the lean follows smoothly |
| `FL_TAKEOFF`, `FL_STRETCH` | 0.14 s, 0.08 | a quick takeoff pose with a small stretch |
| `FL_FALL_VY` | -2.5 m/s | falling faster than this shows the fall pose |
| `FL_LAND` | 0.12 s | the landing pose and squash; input moves her through it |
| `FL_SQUASH`, `FL_SQUASH_MAX` | 0.016 per m/s, 0.2 | the squash grows with how hard she lands |
| `FL_SOFT` | 3 m/s | stepping off a crate has no squash and no landing pose |
| `FL_DECK` | 16 /s | she steps up onto the hoverboard's deck and down off it over about 0.15 s, never a pop |
| `FL_BOARD_LEAN` | 0.7 | on the board she leans with its carve and nose |

Her poses are takeoff (a knee drives up), second jump (tucked, knees in, arms back), fall (arms
out front), landing (a wide, low stance) and her stance on the board (knees soft, arms out). The Q
controls are unchanged: the board rolls off at her walking speed when she steps on, and when she
steps off she runs on at up to her walking speed and stops as she always does. Each footstep and hard landing plays `SND_STEP` and `SND_LAND`,
off like every other sound.

The metrics a level is built to are named constants in the same file (checked against the real
walk and jump in `splice-game.coax`):

| Constant | Value | What it is |
| --- | --- | --- |
| `FL_M_HEIGHT` | 1.75 m | her height as the first character stands (the creator goes from 1.45 m up) |
| `FL_M_JUMP` | 1.28 m | the top of one jump |
| `FL_M_DOUBLE` | 2.98 m | the top of a second jump taken at the first one's top |
| `FL_M_LEDGE` | 1.58 m | the highest top one jump lands her on (jump plus step) |
| `FL_M_WALK` | 5.6 m/s | her one speed on foot: no sneak (Shift is off) and no sprint |
| `FL_M_BOARD`, `FL_M_BOARD_FAST` | 7 m/s, 22 m/s | the hoverboard's casual and fast speeds (the `board` table); double-tap Q is super fast |

The movement demo plays her keys on the game's own clock: `coaxial.send(610, "5")`, then
`coaxial.send(615, "")` puts her before the solar rows. She walks, jumps onto the panels and
stops, reverses off, then double jumps forward and lands on a panel. A check plays the
same demo.

Sniper, the rover, is the checkpoint (#13505): standing by it keeps what you carry. Each second in
the mine the checkpoint's inventory and what is open (the rubble, the boss wall, the banned bot, the
core) is written as `splice.run.v1` when it changed. CONTINUE puts you back beside Sniper with them
(at night once the boss is down). SCENE RESTORED and a new run let
it go.

## The camera

`camera.coax` is the one camera behind her on foot and on the hoverboard, in every zone (playtest 2).
It is a boom from her head out behind her, 6.5 m (4 m in the dome village, 1.5 m more on the board
and more again at speed). Each step it looks for room along the boom in three dimensions: a placed
piece blocks it only up to the piece's top, so it passes over the farm's low panels; tall walls, the
garage's rock, the rock wall while it stands, Digger, the drill mech and the boss stop it. Where the
boom has no room it swings up before it comes in, as far as straight over her head, and it is never
nearer her head than 2.6 m where any way has room (`CM_MIN`); nearer than 1.2 m her body is not
drawn. The angle and length ease on the engine's spring, and it also looks where she is turning, so
it moves before it meets anything. Nothing else moves it: standing still, it stands still. The eye
is never further out than the room the boom has that frame.

Stills 30 to 33 (`coaxial.send(610, "N")`) are the camera's: the farm, Digger drilling the rock
wall, the board at speed, the drill mech following.

The drill mech is drawn at 0.95 (about twice her height; it was 1.25) and follows at her left, its
own radius and 1.4 m out from her, a little ahead; it is never nearer her than its radius and hers.

## Controls

The key map follows the slice's beat sheet (#13505). The pause screen (Esc) lists the same keys.

| Key | What it does |
| --- | --- |
| W A S D, mouse | Walk and look |
| Space | Jump; a second Space in the air jumps again, higher (on the hoverboard, hop) |
| Left Shift | Nothing for now: sneaking is off until it has its own pose |
| Q | Hoverboard: the first press steps on at casual speed, then each press switches casual and fast |
| Double-tap Q | Hoverboard super fast (the fast speed times 1.6, `board_super_cms`) |
| Hold Ctrl, or double-tap W and hold it | Hoverboard very fast (the fast speed) while held |
| Hold Q | Step off the hoverboard |
| E | Interact: enter the rover, take the core, go through a portal, swap a core, tow a powered-down Digger; beside a hurt Digger with a battery cell, charge him |
| Tab | The edit bay, only inside Sniper (the rover) |
| I | The inventory: six slots. A click on a slot drops what is in it; a click on CELLS charges Digger when he wants it |
| Left click | Give Digger the order under the reticle: he goes to what is selected, then digs it, picks it up or attacks it |
| F | Stun a robot within 2 m for 4 seconds |
| 1 to 4 | A bot's battle moves (`d_take_move` hands each press to the battle) |
| Hold R | The bot wheel: move the mouse (or press 1 to 5) to pick DIG, PICK UP, ATTACK, FOLLOW or CHARGE, and let go of R to give Digger the order (`metaverse-engine/ui/wheel.coax`) |
| M | The deep-garage map; M, Esc, E or a click closes it (its "M / ESC CLOSE" hint says so) |
| Esc | Pause; letting go of the pointer pauses too |

A battery cell heals Digger (#13505, playtest 2): E beside him, CHARGE on the bot wheel or a click
on CELLS in the inventory puts one of yours into him, and it charges him full (a powered-down Digger
powers up) over about a second; his BATTERY gauge shows CHARGING and fills as it goes. A spent cell is
gone: dropping cells drops only the ones you still carry.

Digger follows you and picks up every part within his sensor's range (`T_SENSE`, 4 m; twice that with
the better sensor that lies in the scrap room); you can't choose, so you make room in the inventory. A part he
can't fit stays where it is. What you drop he leaves until you have walked away from it. Hardware
spliced into him and cards on his strip are his, not yours.

Heat cools 5 a second at Sniper and nowhere else (not at the stash crate), each banned piece is +25,
and 100 brings the Emperor's sweep. Heat never cools below 25 for each banned piece with you (DRILL+,
the core), and an Imperial robot
senses you from its plan's distance plus 0.07 m for each point of Heat.

You turn to face the way you walk; S walks toward the camera rather than backwards. The camera rises
over the drill mech or the boss when one stands ahead of you, and comes in front of one behind you.

Everyone has a health bar: yours top left (your hearts), Digger's under it and over him, and one
over each foe. At zero you are back at Sniper with the inventory you had when you last left it;
banned pieces picked up since are lost. Digger at zero powers down where he is: press E beside him
to tow him, and Sniper's charger charges him back up. A foe Digger fights strikes back at him.

### Commanding Digger

Commanding Digger works like a block game on a controller (#13552, `digger-orders.coax`): look at
something, see what is selected and the command it gives, then click.

- **Selecting.** Corners frame the rock, part or robot nearest the reticle for its size. The cone
  widens with distance, so a far target is easy to hold. What is selected keeps a cone `T_AIM_KEEP`
  times wider, so two things side by side never flicker. Under the reticle is the command (DIG,
  PICK UP, ATTACK, in orange) or why not (DIGGER DOWN, BAG FULL, NO WAY THERE, NEEDS PARTS, DRILL
  MECH ONLY, pale).
- **Answering.** Every click gets an answer at once. He nods when he takes the order, and a ring
  snaps in round the target. When he can't, he shakes his head, the target gets a cross and the
  line says why: nothing there, no way there (something solid is in his straight way), the bag
  full, him powered down, parts missing for a dig, or the hallway wall only the drill mech breaks.
- **Going.** He goes to the target himself, at any distance the reticle reaches, and holds the order
  until the target is gone or you are `T_LEASH` away. The ring is dashed and turning while he is on
  his way, pulses while he works, and opens out green when he is done.
- **Drilling.** He rears back while the bit spins up, then bites in. The drill sound, the chips off
  the face and the rock's shaking all start at that bite. The rock shakes harder and its top stones
  loosen as he works. At the break he is thrown back, the rubble flies into the tunnel, and he
  settles. DRILL+ spins faster, throws more chips, shakes harder and breaks through in
  `T_PLUS_CARD` of a card.
- **Moving.** Each foot stays planted for `DG_STANCE` of a stride, then swings forward, in diagonal
  pairs. He turns toward a new heading rather than snapping to it, a turn on the spot is steps too,
  and he stops crisper than he sets off. Standing, his nose and sensor turn to you; with an order,
  to his target.

Digger tunables:

| Name | Value | File | What it sets |
| --- | --- | --- | --- |
| `T_SENSE` | 4 m | `director.coax` | His sensor range: he picks up parts within it. The better sensor doubles it (`T_SENSE_UP`) |
| `T_POINT_FAR` | 40 m | `director.coax` | How far the reticle selects |
| `T_AIM_KEEP` | 1.6 | `digger-orders.coax` | How much wider the selected thing's cone is |
| `T_LEASH` | 45 m | `digger-orders.coax` | He lets an order go when you are this far from him or his target |
| `T_GIVE_UP_FIGHT` | 25 m | `director.coax` | The bot wheel's reach for ATTACK and CHARGE |
| `T_ACK` | 0.35 s | `digger-orders.coax` | The marker snaps in this long before it shows his progress |
| `T_NOD` | 0.5 s | `digger-orders.coax` | His nod, or his head shake |
| `T_MARK_FADE` | 0.8 s | `digger-orders.coax` | A done marker opening out, or a refusal's cross |
| `T_CARD_TIME` | 2.4 s | `splice-game.coax` | A card of his strip |
| `T_BITE` | 0.18 | `digger-orders.coax` | The share of a drill card before the bit meets the rock (the windup) |
| `T_PLUS_CARD` | 0.7 | `digger-orders.coax` | DRILL+ breaks through in this share of a card |
| `T_DRILL_REACH` | 2.6 m | `splice-game.coax` | How near the rock a drill card must start to bite |
| `T_KICK` | 0.45 s | `digger-orders.coax` | A strike's lunge, or the break's recoil, back to standing |
| `T_BURST` | 1.2 s | `digger-orders.coax` | The broken rubble flying |
| `T_STRIKE`, `T_STRIKE_REACH` | 0.9 s, 1.4 m | `director.coax` | Between his strikes on a robot, and how near he stands to strike |
| `T_BOT_GO`, `T_FOLLOW` | 6.5, 7.5 m/s | `splice-game.coax` | His speed to his work, and keeping up with you |
| `DG_STANCE` | 0.62 | `digger.coax` | The share of a stride a foot stays planted |
| `DG_TURN_EASE` | 9 /s | `digger-gait.coax` | How fast his drawn heading follows his heading |
| `T_LOOK_EASE` | 6 /s | `digger-orders.coax` | How fast his nose turns to what he looks at |

For a playtest, open the dev page at `?stage=7&rubble` (the dev server only): you start with Digger
built, the rubble ahead of you.

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
- **The drill mech.** It waits in the empty section C2 (landmark 81, else the middle of C2's bay), and
  the compass's objective is DRILL MECH once the rubble is open. E beside it swaps Digger's core in: its
  lenses turn Digger's amber and it follows you; Digger's body waits dark. The compass then says BLOCKED
  HALLWAY: the raiders' barricade across E1's hallway, straight across the ring from the entry (packed
  rock, a yellow X of girders, BLOCKED, flashing lamps, and an orange beacon over it while the mech has
  the core). Led within 10 m of it, the mech drills it on its own (E there asks it too); nothing else
  breaks it. Until the hallway is open an E beside the mech leaves the core in, so it is never left
  empty on the way. DRILL+ in the vault is optional: Digger's DRILL is the run's drill. A check plays
  from the garage's door to the boss's fall with keys and the mouse alone, following the compass.
- **The raider mech.** It keeps watch 12 m off the road at its low point, a little past halfway
  (`DV_RM_AT`), where the road dips south across the plain between the farm and the garage. Outrunning it is the default: out of a battle
  its claws never take Sniper below a third of its health. When Sniper passes within 16 m it gives
  chase at 16 m/s: faster than Sniper's cruise, slower than its top speed, so holding W outruns it
  (20 m behind, it gives up for the drive). Beside Sniper it claws twice every 1.3 s or so (below). E while it is
  within 12 m starts a battle with Sniper fielded; in the battle Sniper drives at its slow speed and
  1 to 4 are its cards. 12 m before the garage it turns back. On foot nothing hunts you.
- **The boss.** A dinosaur-shaped bot glowing blue. Its collider is a body in the engine's World that
  moves with it (`sl_body_to`, layer.coax). Its battle begins when you are in its chamber
  with the mech. The mech fights on its own when you leave it be (its strongest ready card every
  couple of seconds); 1 to 4 pick its moves. A hit landing jolts the boss and flashes it, and the
  prompt names her part: when it reels, run to a glowing weak point and press F. Phase one: its tail
  sweeps a half circle over its left side. Below half health, phase two: it winds up, charges and
  is dazed after. Its three weak points open while it reels from a stunning hit, is dazed, or has no
  health left; stun all three with F (each stays down 12 s) and it powers down, drops its banned core,
  and night falls. You lose if your health or the mech's runs out: back at the checkpoint, the boss
  whole again and the mech repaired at the hallway.
- **The night return.** After the boss, night falls and she goes back to Sniper on foot: the long way
  round the field's edge under the rim, past the bays, or the shortcut across the field through the
  rave, where Imperial 3 and 4 patrol across her way at night. They sense her by Heat (two banned
  pieces: 8.5 m). A check walks both ways with
  the walk replay's walker, and the run takes the shortcut.
- **The ending.** SPLICE CORE in Sniper's edit bay plays the 15-second memory hologram with its
  three captions, then SCENE RESTORED, then the mission's teaser with RESTART.

### Enemy attacks

Every attack on her or her bots is a timeline of four beats (#13552), on the engine's
`metaverse-engine/action/telegraph.coax`:

1. **Wind-up.** The attacker rears back or raises its arm, and its warning shape appears on the ground,
   dashed, with an inner outline growing to its edge. It still turns to follow you.
2. **Lock.** It braces and stops turning; the outline turns solid. A dodge from here on works.
3. **Impact.** The blow is live, only inside the shape shown.
4. **Recovery.** It slumps, its arm or tail down: the opening. It does not attack again until this ends.

A combo (the raider mech's two claws) holds between its strikes with the arm still up, so a pause in a
combo looks different from its end, when the arm drops and it leans slack.

| Attack | Warning shape (the real hit area) | How to dodge |
| --- | --- | --- |
| A raider's or Imperial robot's blow on foot | A wedge ahead of it, 1.6 m, 43° to each side | Step out of the wedge, or back off, once it locks |
| The boss's tail sweep (phase one) | A half circle of 6.2 m over its left side | Jump over the tail, or leave the half circle |
| The boss's charge (phase two) | A lane 4.8 m wide, to the chamber's wall | Step out of the lane once it locks; it is dazed after |
| The raider mech's claws (the drive) | A ring of 2.4 m round where Sniper is heading | Change pace as it locks: press W, or let go of it |
| A foe's card in a bot battle | A ring round the bot it is for | Its wind-up shows which card is coming |

All foes share one lane (`tg_claim`): an attack begins only when its first impact comes at least
`SB_GAP` after the last impact already promised, so attacks never land together and there is
always time to react. An attack winding up off screen puts a marker on the screen's edge toward it
(`tg_edge`) before it can land. A blow that lands says HIT over her, with the `hit` sound from where
the blow came; one that misses says DODGED, with `attack-miss`. A wind-up sounds `attack-wind` from
the attacker. Sounds are panned to where they come from (`snd_play_at`) and stay off behind
`SND_PLAYS`. A hit's flash is kept short so it never hides the next warning.

The tunables, in one table at the top of `splice-battle.coax` (seconds unless named):

| Constant | Start | Why |
| --- | --- | --- |
| `SB_BLOW` | wind 0.6, lock 0.25, impact 0.15, recovery 0.9 | A foe's blow is the most common attack, so it reads at a glance: 0.6 s is long enough to see the rear-back at a walk, and the 0.25 s lock is a short step's time (about 1.4 m at her walk) |
| `SB_SWEEP` | wind `T_SWEEP_WARN` 0.9, lock 0.3, impact `T_SWEEP_TIME` 1.3, recovery 1.1 | The boss is the tough fight: a longer wind-up gives time to read the half circle and choose jump or step |
| `SB_CHARGE` | wind 0.7, lock 0.4, impact `SB_CHARGE_TIME` 1.2, recovery `SB_DAZE` 2.6 | The charge is the big hit, so its lock is the longest; its daze is the opening for F on the weak points |
| `SB_CLAW` | wind 0.55, lock 0.25, impact 0.12, hold 0.4, recovery 1.3, two strikes | A pace change at the rover's speed moves Sniper several metres in 0.25 s, so the claw is beatable but asks for timing |
| `SB_CARD` | wind 0.5, lock 0.2, impact 0.1, recovery 0.3 | Battle moves stay brisk: enough to see the card coming without slowing the battle |
| `SB_GAP` | 0.45 | About two reaction times between two impacts, so even overlapping foes never trap her |
| `SB_BLOW_START`, `SB_BLOW_REACH`, `SB_BLOW_HALF` | 1.7 m, 1.6 m, 0.75 rad | It starts just outside its reach, so backing off during the wind-up always works |
| `SB_JUMP_CLEAR` | 0.45 m | A single jump (about 1.1 m) clears it for most of its arc |
| `SB_CLAW_R`, `SB_CHARGE_W` | 2.4 m, 2.4 m | The hit areas the warnings draw |
| `SB_CUE` | 0.9 | HIT or DODGED shows long enough to read, short enough to clear before the next wind-up |

How far an Imperial robot senses her: its plan's `notice` (the `plans` table) plus `T_NOTICE_HEAT`
(0.07 m) for each point of Heat (`splice-game.coax`), halved while she sneaks (`stance_noticed`). With
two banned pieces (Heat 50) a 5 m notice is 8.5 m. `T_NOTICE` (5 m) is the notice of a foe with no plan.

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
| `drill` | The bit of Digger's DRILL or DRILL+ card meets the rubble, after its windup; the drill mech starts on the hallway |
| `glitch` | A bad cut; the core's memory starting; Digger refusing an order |
| `success` | The rubble opens, the hallway breaks, a fight won, the raider mech down, the core taken, SCENE RESTORED |
| `hit` | You hurt, a foe hit, a stun, a blow in a bot battle, Digger's strike on a robot |
| `sensor-ping` | The rover's sensor pings and stops it on the rail |
| `slot-click` | Edit bay: a part or card set in its slot; Digger's core into or out of the drill mech; Digger taking an order |
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
four solid walls with rounded corners, a ceiling and a painted desert behind its windows, so
nothing of the world shows and no one walks out. Seen from above, the front is on the right: one big
window across the whole front wall, the loft bunk and its ladder top left, the desk with its screens
top right, two window strips on the top wall, the workbench arm on the right by the front, and the
edit bay's console with its six slots at the bottom middle (Tab opens the bay). The door is in the
back wall, on the left; E at it goes back out beside Sniper. Inside, the camera stays close and
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
- Fixture `splice-camera` (step 5c) shoots the player camera's three views, the board at speed and a
  room's walls into `OUT/camera`, and `OUT/camera.lines` keeps its `AETHYR_CAMERA` lines.

#### The player camera

The Unreal host places the view itself while you are on foot (`AethyrCamera.h`,
`AethyrBridge/Private/AethyrCamera.cpp`). The world still owns where you stand, which way you face and
walk, its collision and the scripted run. The host gives it the view it shows and the ray it aims along
(message 622), so the bars over the bots and Digger's pointing match the picture. In the edit bay, the
drive, the memory and the title the world's own camera shows, blended to and from. A scripted run
(`-AethyrPlay`, `-AethyrScript`) sends no 622, so its clicks and lines stay the same.

| Input | Does |
| --- | --- |
| F5 or V | Next view: first person, behind, front, then first person again |
| Gamepad right stick click, or the top face button | The same |
| Mouse, gamepad right stick | Look (the world turns you; the host keeps the pitch) |

F5 also reloads the page in some browsers, so V is the safe key on the web.

- **First person**: the eye is at `EyeHeight`, pitch reaches ±89°, and your character is hidden from the
  camera. The game's materials are unlit and cast no Unreal shadows, so hiding the body hides its
  shadow blob too, except for a model Unreal draws itself, which keeps casting. The head bob scales with
  speed and is off standing still. A landing dips the eye by the height fallen. The eye is swept out
  from your middle, so it never enters a wall.
- **Behind**: a spring arm over the right shoulder with a sphere probe. It pulls in at once on a hit and
  lets out slowly. It has light position lag and very light turn lag, and it shortens where the world
  pulls its own camera in (rooms, the rover's cabin). You turn toward where you walk; the camera orbits.
- **Front**: the arm in front, looking back at you.
- The body is hidden whenever the camera is closer than `HideDistance`. On the hoverboard the field of
  view widens with speed (`SprintFovKick`), eased in and out.
- A view change, or a change to and from the world's camera, blends over `BlendTime`. The view is
  saved in `GameUserSettings.ini` (`[AethyrCamera] Mode`), so it survives a respawn and a new level.
- `AimTrace` traces from the eye in first person and from your head in third, along the aim.
  `PlayCameraShake` is called on `hurt` (light) and `smack`, `wall-broke` and `boss-down` (impact).

Every tunable is on `UAethyrCameraTuning`. To tune without code, make a data asset of it and set it on
the level's `AethyrGame` actor (**Camera Tuning**). The player's own settings are console variables,
saved with the view: `aethyr.cam.Mode` (0 first, 1 behind, 2 front), `aethyr.cam.MouseSensitivity`,
`aethyr.cam.StickSensitivity`, `aethyr.cam.InvertY`, `aethyr.cam.HeadBob`. `aethyr.Message KIND BODY`
sends the world a message; a test uses `611 X Z` (centimetres) to stand you somewhere.

| Tunable | Default | Turn it |
| --- | --- | --- |
| `BlendTime` | 0.25 s | up for slower view changes |
| `MouseDegreesPerUnit` | 1.4667 | the browser host's feel; the player's multiplier is `MouseSensitivity` |
| `StickYawSpeed`, `StickPitchSpeed` | 200, 140 °/s | at full stick |
| `StickDeadZone`, `LookSmoothing` | 0.18, 0.03 s | 0 smoothing is raw input |
| `FirstPersonFov`, `EyeHeight` | 90°, 155 cm | |
| `FirstPersonPitchLimit`, `FirstPersonProbeRadius` | 89°, 16 cm | |
| `BobVertical`, `BobSide`, `BobCycleMetres`, `BobFullSpeed` | 1.6 cm, 0.9 cm, 2.4 m, 560 cm/s | 0 heights for no bob |
| `LandDipPerMetre`, `LandDipMax`, `LandMinFall`, `LandRecover` | 6 cm, 12 cm, 40 cm, 8/s | |
| `SprintFovKick`, `SprintSpeedStart`, `SprintSpeedFull`, `FovEaseSpeed` | 8°, 650, 1100 cm/s, 5/s | walking is 560 cm/s |
| `ThirdPersonFov` | 0 (the world's) | the world's own widens on the board |
| `ArmLength`, `FrontArmLength`, `PivotHeight`, `ShoulderOffset` | 420, 330, 150, 45 cm | |
| `ProbeRadius`, `ArmEaseOut` | 18 cm, 3/s | |
| `PositionLagSpeed`, `RotationLagSpeed` | 14/s, 30/s | higher is stiffer |
| `ThirdPersonPitchMin`, `ThirdPersonPitchMax` | -70°, 55° | |
| `HideDistance`, `MinCameraHeight` | 70 cm, 25 cm | |
| `HitShakeDegrees`, `ImpactShakeDegrees`, `ShakeTime` | 0.6°, 1.2°, 0.3 s | 0 for no built-in shake |
| `HitShake`, `ImpactShake`, `ShakeAssetScale` | none, none, 0.3 | optional Unreal camera shake assets |

There are no first-person arms or held-item models yet, and no settings menu; the console variables
stand in for one.

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
