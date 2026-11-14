# Splice Bots slice 1: level design review (Lane F, #13552)

The slice played headless on the build box against the owner's principles in #13552 (affordances,
signifiers, wayfinding, spatial choice, rhythm, encounter readability, teaching, discovery,
immersion, world response, boundaries, composition) and the layout per the level spec. Ranked by
what it does for a first-time player; each item says the player experience it improves, the
smallest change that works, and how a playtest shows it worked. **BUILT** items are in the build
(commits on #13552); **PROPOSED** ones are not.

North is -z and east +x, as the compass shows (`hud.coax`).

## 1. One readable world: the farm west, the drive south, the garage east (BUILT)

- **Experience.** The run reads as a journey across one desert: the farm in the west, a road that
  dips south across the middle of the plain, the garage in the east with its mountain to the north.
  Before, the farm sat straight south of the garage's door, the drive was a short straight line, and
  pink mesas, dunes and a dome village with a rail line stood about the plain.
- **Change.** `plain.coax` (new): the plain's ground and edge, three landmark rocks and the
  mountain's two peaks. `farm.coax`: four long rows in the farm's north-west. `drive.coax`: the road
  leaves the farm south, dips across the middle and rises into the garage's south opening; the
  raider mech keeps watch at the dip, halfway. The content tables' farm places, the Imperial
  robots' plans and the start moved with it; the mesas, dunes and loose rocks are gone; the old
  village and rail line are drawn only for tables that still start there. Sniper's cabin and the
  edit bay's and memory's stages moved 500 m further south, out of the camera's 400 m from anywhere
  she can stand.
- **Playtest.** From the start the farm's rows, Sniper and the parts are all in view; the drive's
  camera shows the road curve; at the garage the mountain stands over the rim behind the field. No
  tester mentions "pink pyramids" or a stray village.

## 2. No invisible walls; what you see is what stops you (BUILT)

- **Experience.** She can walk the whole plain. What stops her is something she can see: the
  garage's rim and its slope, the mountain's peaks, Sniper, a row of panels. At the plain's edge a
  line of orange-flagged stakes and one line of text say why she turns back.
- **Change.** Removed the three height-0 walls that penned the ground south of the garage's mouth.
  From outside, the rim's slope held nothing (walking into it put her inside the ring through the
  back wall): `pl_push` now holds her at the slope's foot, and off the peaks. The edge is the plain's
  rectangle, staked every 40 m, with `MSG_EDGE`.
- **Playtest.** Walk south from Sniper and round the garage: no stop in open sand, no teleport
  through the rim; the edge message appears only at the stakes.

## 3. Landable tops look and act landable (BUILT)

- **Experience.** The farm's banks are platforms: she jumps onto the glass, runs along a row and
  hops from line to line; the three rocks are climbable slopes with flat tops she can stand on or
  board over.
- **Change.** A row is three lines of banks; each bank's tilted glass is a floor (`ff_floors`, the
  same tilt the panel is drawn with), and walking holds her off a line only below its low edge.
  The rocks' heights (`land_mound_y`) are the numbers their mesh is written from
  (`metaverse-engine/kit/land.coax`, checked vertex by vertex).
- **Playtest.** Testers jump onto a row unprompted and run along it; nobody floats over or sinks
  into a panel or a rock.

## 4. The farm per the level spec (BUILT)

- **Experience.** One glance from the start shows the whole farm: long rows ahead-left with wide
  aisles, Sniper at its charger ahead-right, the drill card and the hardware close on the left,
  the Imperial robots' two charging pads beyond.
- **Change.** Four rows (about 150 m long, 13 m aisles) in the farm's north-west; Sniper's train
  east of them with its charger just north of it; the start at the bottom between the cards and
  Sniper, facing north; the cards nearest the start and the hardware past them; two charging pads
  (white discs, dashed rims, a charger each) at the bottom-left, where the Imperial robots stand at
  dawn and dusk. By day they patrol the aisles, so a row is a choice: run an aisle past a patrol,
  or jump the rows.
- **Playtest.** A new player picks up the parts without being told where they are, and some go
  over the rows rather than down an aisle.

## 5. A grey metrics room (BUILT)

- **Experience.** A level designer sees the build's own numbers side by side before blocking out
  a space.
- **Change.** `metrics.coax`: off the map, reached with dev stage 40. Her height, jump and double
  jump on a pole; walk, sneak and the board's three speeds as lanes as long as a second at each;
  Digger's sensor and an Imperial robot's sensing at each Heat as rings; Sniper, the drill mech and
  the boss as grey blocks. Each number is the constant the game plays by.
- **Playtest.** The designer can size an aisle or a patrol's reach from the room without opening
  code.

## 6. The garage ring per the level spec (BUILT)

- **Experience.** The ring reads as a clock: the way in at 6 o'clock from Sniper, the empty section
  with the drill mech straight across the field at 3 o'clock, the hallway into the mountain at 12, the
  work bay's catwalk at 9, the rave's purple centre in the middle. At night the long way back is a
  real choice: west from the hallway through the bays, under their roofs and out of the rave's sight,
  or straight across the rave past the Imperial patrols.
- **Change.** `garage.coax`: the 24 section names in one table (`GA_NAMES`); the empty section is C1,
  due east, with the drill mech; the catwalk moved to G1, due west (its code now works on either side
  of the ring); a passage through each pier near the back wall, so the bays join up all the way round;
  a purple circle 45 m across at the field's centre. `sp_long_way` walks west through the bays and
  their passages; the walk replay and its checks follow the catwalk's new place.
- **Playtest.** From the field testers point at the drill mech before the compass names it; at night
  some take the bays and say they were "sneaking round the back".

## 7. Decoration that looked usable (BUILT)

- **Experience.** Each bay's back wall had a dark mouth under a yellow lintel that read as a way out
  and went nowhere. Now those are closed grey roll-up doors; the only dark openings are the real ones,
  the entry's tunnel and the hallway.
- **Change.** The mouths' colour (`garage.coax`).
- **Playtest.** Testers stop walking into bay backs; the hallway's mouth is the dark opening they
  head for.

## 8. The catwalk's floor matched its deck (BUILT)

- **Experience.** Behind the work bay's catwalk she stood 0.9 m above the bare floor for 8 m past the
  deck's edge.
- **Change.** The deck's floor ends where it is drawn (`DECK_W`, 4 m).
- **Playtest.** Walking behind the deck, her feet stay on the floor.

## 9. Kit slots by name (BUILT; the garage bay swaps, #13565)

- **Experience.** A designer dresses the grey blockout piece by piece without touching layout code.
- **Change.** One named table of the level's kit slots (`KS_ROWS`, `kits.coax`), in the bundle's
  manifest as `kits` with each slot's places and blockout groups; a `kit.NAME` row of the host's
  overrides table, or a mesh of that name in the world's import pack, replaces the blockout. Only the
  garage bay lists its places so far.
- **Playtest.** A dressed bay drops in for every bay at once, and the walk and the checks still pass.

## 10. A rim overlook into the garage (PROPOSED)

- **Experience.** From the parking spot she climbs the rim's outer slope and looks down on the whole
  ring: the drill mech in its section, the rave, the hallway. It rewards climbing, shows the next
  destination and lets her plan the night shortcut.
- **Change.** The outer slope's courses as a floor (as the rocks are), and a rail along the rim's
  inner edge so the overlook is not a way in past the rock wall.
- **Playtest.** Players who climb it point at the drill mech before the compass names it.

## 11. Something on top of each landmark rock (PROPOSED)

- **Experience.** Climbing a rock is its own reward today (the view). A battery cell or a part on each
  top would pay off the detour and teach that heights hide things.
- **Change.** A pickup row on each rock's top (the progression lane's tables).
- **Playtest.** Players who see one on the first rock climb the other two.

## 12. The farm's haze hides the rocks (PROPOSED)

- **Experience.** The dust haze that gives the farm its scale fades the two northern rocks to pale
  shapes from the start.
- **Change.** Start the haze further out (`ff_style`, 50 m now) or only over the rows' far ends.
- **Playtest.** From the start testers can name the north-west rock as a place to go.
