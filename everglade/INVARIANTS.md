# Invariants — everglade/INVARIANTS.md

> Scope: Everglade realm, SRD 5.2.1 combat, NPCs, mounts, world edits (63 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## An admitted Everglade seat keeps a cast reservation

`coax/web/room/cast-budget.coax` (#12841, `#authority`, mutation-checked)
admits at most two outstanding casts per seat, across waiting, bar and flight
phases. `rc_hear` enforces this before recording the cast or spending its
cooldown/order; refusal returns the existing `r SEQ busy`. The room's 32 slots
cover two for every admitted seat, so a sender cannot consume another seat's
reservation. Releasing a cast frees its reservation. The bound is local to
each room; it does not qualify CPU isolation, hurt ordering, durable queues
or dependency outages. Checks: room-casts.coax's flooding, full-room and
separate-room cases; cast-budget.coax's independent boundaries.

The native and worker tick's world-damage input is one aggregate per seat,
held in a reserved mailbox (`hurt-budget.coax`, #12841). Repeated delivery
replaces that seat's whole-tick snapshot; it cannot occupy another seat's
space, multiply a hit or apply damage before the tick's heals. Resolution
takes and clears each mailbox once after due heals. Actual individual spell
and ability hits keep their existing rules path; they are not coalesced by
this adapter. Checks cover every seat, repeated reports, invalid inputs,
take-once behavior and the integrated heal-before-harm order.

World ability service reserves two starts and four ends per seat independently
(`effect-budget.coax`, #12841). Its 96-entry queue covers all sixteen seats;
one seat cannot consume another's service or let its ends consume its starts.
An overloaded start is refused before cooldown or rule resources are spent. An
ordered end queue refuses its fifth waiting request as busy rather than ending
an ability ahead of preceding movement. Doors/talks/boats reserve one nonnegative-ID
and one negative-ID request per seat per tick (the latter includes letting go of
a conversation and the boat's nearest/toggle request), with explicit busy refusal of extras. No full queue silently
accepts those commands. Checks: room-abilities and room-everglade overload cases.

`work-budget.coax` (#12841, authority) owns each room's measured-work credit.
The process's rooms share a pool of 720 microseconds per monotonic
millisecond: each live room earns an equal share, never less than 45 (all
sixteen live), and its credit cap (3,000 microseconds at 45) and start credit
(1,500 at 45) scale with its share (#12936: a fixed 45 per room deferred a
lone 16-player room on a 1 vCPU shard to 23-26 Hz and refused its joins). The
live rooms' shares never sum past the pool. The
shared native/worker tick charges its complete simulation and serialization
wall duration. It starts only with its share's start credit available. An overrun is
that room's debt; deferral advances neither world nor rule time, clears/counts
queued movement and resets its earning epoch. It grants no movement for the
deferred interval. Bar/flight casts and accepted lifecycle work remain held.
Waiting cast intents expire after 533 monotonic milliseconds with explicit busy
before cooldown/resources. New casts and seats are refused while credit is
insufficient or the last measured tick exceeds the burst; refusal precedes
cast order and ownership fencing. Invalid cost/debt retires that room; clock
reversal cannot refund work. This is amortized isolation: an uninterruptible
Coax call and OS scheduling are not preempted. HTTP and transport retain their
separate bounds. Checks: independent credit/debt/clock/age mutation gates and
the room-work separate-room, no-credit and refusal/expiry regressions.
Native startup prepares all sixteen world slots before Everglade readiness or
socket admission, including sharded relay paths. Other games retain their
startup admission contract. A missing initial publication refuses new play;
an already admitted room keeps its current world during dependency outages.

Retired with v1's room (#13346; it builds frozen from `everglade/server/legacy/v1.sh`'s
pin, and no production room runs a raid): raid deaths reserved one authoritative outcome
for each physical creature slot (`metaverse-engine/session/outcome-slot.coax`, #12841). A pending death prevents raid
reuse of that slot until its XP/contribution result is consumed. All 96
creatures may die in one step without a dropped result. If recording cannot
reserve space, the dead source remains for retry instead of disappearing.
The room settles deaths before reinforcements, drains once and releases each
reservation; deliberate event clear settles pending results first. Checks:
outcome-slot's mutation gate and room-events' full-capacity death/reuse/XP case.

Retired with v1's room (#13346, as above): the raid's protected player-spawn radius
excluded hostile cult actors only (`everglade-raid-spawn-policy.coax`). Friendly watch actors may walk through
the commons spawn without an artificial radius correction; ordinary world
collision still applies. Routing, final placement and Dimension Door use the
same faction policy. Checks cover Echo's bounded first step from (1, −16),
her natural first Meteor Swarm, hostile siege and watch combat; the policy's
mutation gate has zero survivors.

Accepted town impacts keep physical reservations independent of visual pools
(`metaverse-engine/session/effect-reservation.coax`, #12841): four per admitted player,
two per physical raid slot and eight for the house. Admission subtracts already
accepted waiting/staged work before cast order, cooldown or resources. The
shared native/worker adapter acknowledges only a consumed prefix; a refused
handoff remains queued. Player staging has a reserved region which NPC, forge
and house work cannot consume. NPC work defers before spending resources when
its reservation is full. Released impacts retain their source, position, seed,
damage and ownership through caster removal, raid clear and later dial changes.
Classic casts retain every meteor/bolt/orb until its deadline; a visual pool
cannot evict their gameplay. Each completed impact releases once. New broken
pieces retain their individual origin through later outcome collection. Fall
damage reserves every seat/person pair in one step, rather than truncating at
sixteen copies. Checks: independent authority mutation; room-effects' refusal,
prefix and raid-clear cases; world effect concurrency/deadline/ownership and
full fall-matrix regressions.

The native room's background HTTP effects use six independent lanes
(`background-budget.coax`): control, world, legacy vitals, complete checkpoints,
directory and statistics. Each has one serial SDK helper and one admitted Coax
request identity. The host offers whole bounded datagrams without blocking;
transport refusal goes back to the durable outbox as failure. Full, failed or
late requests cannot change another lane's owner. A response at or after its
15-second deadline cannot apply; a replaced request cannot acknowledge its
successor. Failed world fetches keep retrying (at least one second apart), and
an older publication's response cannot clear a newer publication's fetch.
Helpers survive process-group SIGTERM for final acceptance and stop after their
parent leaves. Neither admission nor HTTP success substitutes for the complete
checkpoint receipt. Checks: independent lane, clock, identity and publication
boundaries; actual native HTTP outage in `background-check.coax`.

## An Everglade room seats only a page of its own build, or of the release before

A room admits a hello only when the page's engine, rules and world numbers
(its `P` frame; a page from before `P` is build 1 1 1) equal the room's, or
all three equal the release before's, which the room names and serves for one
release (`everglade/shared/pins-prev.coax`, `#authority`; a seat on the content
tables before plays by those tables, `everglade/server/pins-prev.coax`, #13408;
a pin raised without the release before's value kept fails the room's checks
and `e2 prev-pins`), its
wire is in `WIRE_MIN..WIRE_VERSION` (`WIRE_MIN` is the wire production's page
speaks, `WIRE_LIVE`, and never newer than the version before `WIRE_VERSION`;
the room writes a seat on any older version what that page reads:
`everglade/server/wire-prev.coax`, #13398), and the saved data its ticket brings
back is a format the room reads (`Ticket.schema` up to `RV_SCHEMA`). Any
other hello gets `r 0 version`, its socket is closed, and the refusal is
counted by its reason. One encounter never mixes two rule sets or worlds, and a room
rolled back below newer saved data reads none of it. The rule is
`metaverse-engine/session/compat.coax` (`#authority`, mutation-checked) applied by
`coax/web/room/room-everglade.coax` `eg_admit`. Checks: compat.coax and
room-everglade.coax "a hello is admitted against its page's build" (#12839).
The previous pins: compat.coax "a server seats the release before's pins",
pins-prev.coax (both files) "previous pins: ...", and
`coax/web/room/pins-pair.coax` against production's commit.

The directory places a page that names its build only on a shard that runs
that build or whose build is unknown (no stats for 60 s), when any shard
fits; otherwise it places the page as before and the room refuses it
(`verse.room_pick`, migration 0128). A saved character names the build that
last saved it (`room_vitals.build`). Checks: `room_checks.coax` builds.

## Forest fires belong to the room's world

The shared Everglade room tick advances tree fire, spread and motion once
per world step. Session profile v5 carries each occupied sparse forest
record as an ordinary public entity, independent of its fire starter.
The starter retains blame; the latest caster cannot acquire it. The
authority in `everglade-forest-profile.coax` admits only whole finite
records in their declared domains. Its root gate kills all 103 mutants.

Room pages apply those records and render fire without advancing local
tree damage or spread. Forest records have a separate reserved table
range; boats, doors and abilities retain their capacity. Record reuse
advances LIFE, and an acknowledged old removal cannot erase a tree now
held by another record. Whole-tick ACK, retransmission and absolute/delta
baselines remain shared with other entities. Suite, wire and two-page observations are in
`docs/verification/everglade-forest-2026-11-05/`.

## Everglade's legacy GPU host bounds unfinished frames

`metaverse-engine/core/frame-pacing.coax` admits a frame only with a nonnegative count below
two. Everglade exports that Coax decision to `coax/web/host.js`, which checks
before simulation or GPU writes and completes submissions at queue fences.
Skipped callbacks retain elapsed simulation time. Pending counts belong to
their GPUDevice, so a late completion from a lost device cannot decrement a
replacement device's count. Shared-renderer validation remains separate.
All five admission mutants are killed; held fences and device recovery were
checked in headless Chrome. Native measurements are in
`docs/verification/aethyrforge-frame-pacing-2026-11-04.md`.

## Character Studio renderer identity

Character Studio binds only the exact `character-creator` slug and, since it is a
place in Everglade (#13018), the `everglade5e` producer as an explicitly tracked
legacy consumer. Its Quaternius wardrobe uses
the existing people palette and draw format; this adds no shared/native renderer
qualification. Unknown slug/producer pairs continue to refuse. The appearance
save contains only versioned choices, never pack addresses or renderer state.
See `docs/metaverse/character-creator.md`. A validated shared Character Create profile
completes the appearance choice: Everglade must not automatically open its old
Ranger picker or hold the room socket for another Enter press. Profile/ticket
load order and the local arrival route obey this; invalid records do not bypass
the fallback, and manual in-world editing is separate. Server room seating and
name authority are unchanged. Desktop controls share one compact left sidebar
with a 75%-opaque post-composited background; its depth must not occlude the
world. HUD strokes draw on the final display canvas after postprocessing.
Narrow layouts keep a single bottom panel, and scene-only mode excludes UI.
The player uses the glade sun and geometry visibility in linear HDR, with
normal-dependent sky/ground and soft front fill; do not reinstate the legacy
Studio brightness override or decode an already-lit display colour. Camera eye
height remains at least 0.28 m above the glade terrain after pan and zoom. The
reference sky covers the viewport and fades continuously into its extension;
camera tilt must not expose a finite sky-card edge.
Character Create's CLASS row (#13024) stores one of the twelve SRD 5.2.1 classes
in the appearance record (`CC3`, `CC2` plus one class letter); `CC1`/`CC2`
records still restore, with no class. A class plays a level 1 SRD character whose
only bar slots are its slot 1 (wire slot 96 + class, `FB_CL0`) and Repair: the page
rolls nothing, the room resolves the cast in its 5e engine, and the room refuses
every other slot for that seat, levels on or off. The hello's `srd-*` kind word
seats the class; the checkpoint's persisted class bit stays 0 or 1 (the Warlock
or not), so a restoring hello reseats a class. USE IN EVERGLADE's run drives
Everglade's ordinary input path (no teleport, no private movement), so the room
moves the seat as for held keys and the drawn and room facings agree at the
hand-off; its camera ends in the follow camera through `metaverse_orbit_toward`, and
any key during it only skips. Checks: `fivebar-test.coax` "Character Create",
`everglade5e-rules.coax`, `everglade/server/battle.coax` "a seat's class", room
`room.coax` "Character Create class", `everglade5e.coax` "create:" and "classes:".
(Character Create now opens in Everglade II, #13023, so the browser walk into
Everglade's Commons was removed.)
Everglade II's class and level are the server's (#13259). The API keeps a
character's class once (`verse.room_class_once`, migration 0160, from the room's
` c=CLASS` vitals token) and signs it with the level its stored XP gives into
the v5 ticket's EXT (`c=srd-fighter,l=5`; `metaverse-engine/session/seat-class.coax`).
The room never takes a hello's level for a ticketed seat. It takes the hello's
class only when the ticket names none. A re-hello, and Change character for a
non-admin, never changes class or level and never refills Hit Points or slots.
Level rises only as XP crosses the one table (`rules/srd/advancement.coax`,
the only XP-per-level table). The page decides only for an admin's ticket (ACCT
2), the dev stand-in's admin, and a room with no ticket keys. A v2/v3/v4 ticket
seats at its XP's level with its first hello's class, and is never refused for
this. The starting level is the server's `OA_ROOM_START_LEVEL` (default 1).
Checks: room `room.coax` "seat class:", the two authority files' tests and
`coax mutate` (zero survivors), `seat-ticket-profile.coax` on
`proto/verse/seat-ticket-v5.json`, the API's `room_checks.coax` "room-class:",
and `everglade/client/net.coax` "your seat's class and level are the room's".
The shared authoring catalog (`assets/registry/`) distinguishes stable logical
lookup keys from immutable revision hashes and the Metaverse Engine's runtime byte identities.
Source snapshots retain their exact file inventories. Published derivative
revisions retain exact parent revision pins. Generated Character Create tables
resolve stable IDs; CC2 legacy slots are frozen compatibility keys and are never
reordered or reused. Every referenced mesh must resolve before the wardrobe
scene is ready. Full-resolution interchange GLBs and contact sheets confer no
new runtime-renderer, collision-hull or shader qualification. See
`assets/registry/README.md` and `docs/verification/asset-library-2026-11-06/`.

Character Create's Nature glade uses one static environment record (flag 16,
32-byte vertices, zero joints) and three source-geometry grass instance groups
(flag 17). Neither reads the animation palette nor receives appearance/eye
markers. Original wardrobe vertex, index and image payloads stay byte-identical;
the combined pack stays below 32 MiB with 64 texture layers. Camera and
appearance saves retain separate state. The v4 private static profile uses
ordinary vertex indices; indirect firstInstance's high 16 bits encode object
slot + 1, low 16 bits the seed ID, with each grass group bounded to 65,535.
Leaf word 7 contains a finite 0.5–1 ambient mask and normalized crown normals;
source cards receive no second expansion. Tree normals use deterministic local
crown lobes with one lobe choice per four-corner source card. Wind-ribbon
geometry is absent; the pack oracle requires zero former ribbon markers.
Bark UVs preserve repeating negative coordinates. Desktop texture layers use 1024, phone 512 through the existing
optional host hook. Only the admitted Character Create page selects the
private `charactercreator.wgsl` graph (geometry PCF shadows, MRT contact
shading, bloom and reconstruction). Common fivelab keeps its single target.
Uniform texture sampling precedes material selection. The private graph keeps
linear HDR through lighting and bloom, decodes palette sRGB once, and encodes
output once. Its auxiliary target holds octahedral normal XY, indirect luminance
share and signed depth; contact attenuation is weighted by that indirect share.
Headless real WebGPU checks cover both graphs, desktop/phone allocation and
numeric color/lighting readback. The image-derived sky
and remixed cliff retain explicit provenance; no original scene recovery,
pixel equality or shared/native renderer qualification is inferred.
See `docs/metaverse/character-glade.md` and
`docs/verification/character-glade-source-parity-2026-11-06/`.

## Everglade II's pieces and tufts hold still and only fade (#13023)

Every drawn tree, plant, rock and grass tuft in `/worlds/everglade2` is one
planted instance at one fixed world position, turn, scale and shape at every
eye distance. Distance never moves, grows, widens or swaps an instance; it only
fades it out in a screen dither (`e2_inst_fade`, `e2_dither_keep`). A tree's
levels of detail are the same crown cards in the same places, and a change of
level is a dither cross-fade in which the two levels cover each pixel once. A
kit thins out past part of its reach by a per-instance rank hashed from where it
stands, never by a hard radius or by one piece standing in for its neighbours.
The frame depends only on where the eye is, so walking away and back gives the
same instances. The key light's shadow tiles snap to whole light-space texels
(`metaverse-engine/render/graph/cascades.coax`), so shadows stay fixed to the land.
Checks: `everglade/client/client.coax` "a piece keeps its place and size
at every distance and only fades", "the woods' pieces never move, grow or swap
as you walk, and come back the same", "Everglade II's shadow tiles: ...";
`everglade/client/gpu/everglade2.coax` "a grass tuft stands in one place at
one size wherever the eye is, and only fades" and "a shadow tile places a
point by its rows and draws it inside its own rect".

## Everglade II's GPU cull never hides a piece in view (#13050)

The GPU instance cull (`everglade/client/gpu/everglade2-cull.coax`,
`coax/web/instance-cull.js`) only leaves out pieces the page picked. It never
changes their level, fade, place or size. A piece is left out only when its
whole box, conservative for every vertex the kit stage places (any turn, scale
and wind), is outside the frustum or behind depth this frame really drew. The
second phase retests every piece the first phase hid against the pyramid of this
frame's depth, so the image is the image without culling.
Checks: `everglade/client/gpu/everglade2-cull.coax` "a placed piece's box holds
every vertex the kit stage places, any way round and in any wind" and "a piece
behind nearer depth is kept by the first phase and drawn by the second only once
the pyramid shows it"; `coax/web/everglade2-cull-check.coax` (headless: the
glade, the woods and the clearing, the same pixels with the cull on and off).

## Everglade II's ground draws from its admitted material table and bake (#13052)

The drawn ground's albedo in `/worlds/everglade2` is the admitted material
table's (`everglade/world/ground.coax` `e2g_table`), evaluated per pixel by
`metaverse-engine/render/shade/terrain-shade.coax`'s equations with the weights the tile's
converter uses (`everglade/world/forest.coax` `e2_ground_layer`). The ground's
indirect light takes `E2G_GROUND_AO_SHARE` of the published canopy bake, and
AO 1 before the bake publishes and off the tile. The pack's ground vertices
carry no colour (#13059). A material table entry may take part of its blend from a
layer share, one named material's weight against another's at the point. Such
a share names two other known materials, the shares add to one, and any share
makes the table's identity "ATM2". A table without shares keeps its "ATM1"
identity unchanged.
Checks: `metaverse-engine/world/terrain-tile.coax` "a layer share names two other known
materials ...", `metaverse-engine/render/shade/terrain-shade.coax` "a layer share is one weight
against the other ...", `everglade/world/ground.coax` "the table draws the
pack's ground colours ...", `everglade/client/gpu/everglade2.coax` "the
ground reads its table's material, the litter's share of the layers under it,
and the bake's AO".

## Everglade II's ground is one admitted tile set, published whole (#13059)

The walkable land in `/worlds/everglade2` is a set of Metaverse Engine terrain tiles
(`everglade/world/ground.coax`): the glade and the path's tiles on one f64
lattice, so a sample two tiles share is bit-identical in both. Neighbours of
one spacing repeat their shared edge exactly, and a fine edge meeting a coarse
one is stitched (LOD profile v1), so no edge cracks; every tile draws at its
stored level, so nothing swaps as you walk. The page takes the set from the
pack's catalog record (route 1): every tile is admitted against the page's own
table and must come out at the revision the record names, the seams and the
set's revision must check, and only then does the set publish, whole. A
refusal is counted and keeps the set published before; with no record or a
refused one, the page converts the land itself (route 2). The walk stands on
the admitted triangles wherever a tile has ground; reads past the tiles are
counted. Queries, the bake and the shader's store read tiles through one frame
origin, never raw world coordinates. The canopy bake is keyed by the glade
tile's revision, the crowns' digest and one world-state axis (the realm's
variant); a bake for another key refuses and the live bake stays. The pack's
ground vertices carry no colour.
Checks: `everglade/world/ground.coax` (the path's tiles and seams, the
record's round trip and its six damage refusals, far origin, the canopy bake's
state axis), and `coax/web/everglade2-terrain-check.coax` (route 1, no refusal
and no memory growth across the glade, the path and the seam walks).

## Shared combat rolls retain owners without multiplying named benefits

`rules/srd/effect-rolls.coax` supplies one Bless d4 and one Bane d4 per
attack/save, in that order, regardless of independent retained record count.
Each record keeps its caster and deadline; retiring one leaves other owners.
A next attack consumes all weakness and Guiding Bolt riders and all Vex riders
for that target, even on a miss or when Advantage and Disadvantage cancel.
Other-target Vex remains pending. The existing battle lifecycle retires timed
records; the roll policy does not invent a second expiry clock.

Checks: `rules/srd/effect-roll-checks.coax`, the hotbar regression in
`apps/openagents-web/old/v1/fivebar-test.coax`, and the scoped zero-survivor authority
gate. Exact native/evaluator scope and remaining #13039 requirements are in
`docs/verification/srd-effect-rolls-2026-11-08/README.md`.

## Damage and healing preserve the zero-HP lifecycle and condition owners

`rules/srd/hit-points.coax` owns HP loss, healing and death saves.
Temporary HP absorb HP loss but do not prevent death-save failures from taking
damage at zero HP. Stabilization resets both save counts. A natural 20 reports
recovery only if its attempted healing restores HP; blocked healing leaves the
creature dying and unable to act. Ineligible death saves consume no die.

Healing ends only the Unconscious source imposed by zero HP. Recorded independent
sources remain; ending such a record cannot wake a creature still at zero HP.
Ordinary healing never restores temporary HP and saturates without addition
overflow. The battle turn report describes actual HP recovery, including cases
where another Unconscious source still prevents acting.

Checks: `rules/srd/hit-point-checks.coax`, its scoped HP/storage authority
gates and the Healing Word hotbar regression. Exact source, native scope and
remaining #13039 work: `docs/verification/srd-hit-points-2026-11-08/README.md`.

## Prone is a movement choice, not automatic turn-start recovery

The shared SRD turn start leaves Prone intact, including after healing from
zero HP. In strict timing, `rs_stand_up` spends half current tracked walking Speed rounded down
in feet, refuses at Speed zero or when movement is forbidden, and spends no
Action or Bonus Action. Whole-cell `move_left` and residual `move_feet` retain
every foot of that payment. `rs_step_to` crawls while Prone: a five-foot step
costs ten feet, or fifteen into Difficult Terrain. Terrain sources do not
multiply that cost. Standing/crawling refusal changes neither position nor budget.

Prone imposes Disadvantage on the creature's attacks. Attacks against it gain
Advantage within five feet and Disadvantage farther away, regardless of attack
mode; other edge sources still cancel normally. `prone-movement.coax` owns
the changed payment, standing and edge policies. Evidence and the remaining
world controls/timing/movement requirements are in
`docs/verification/srd-prone-2026-11-08/README.md`.

## Condition roll consequences share one admitted save result

`rules/srd/condition-rolls.coax` owns range-independent condition attack
sources, five-foot Paralyzed/Unconscious hit criticals, automatic Strength and
Dexterity save failures and damage defenses. Petrified grants attack Advantage,
automatically fails those two saves and resists all damage. Immunity applies
before one resistance and then vulnerability; overlapping resistance never
halves twice. Nearby ranged weapon hits receive condition criticals too, while
misses and Stunned/Petrified targets do not receive an automatic critical.

Spell save programs and Cunning Strike share `rs_saving_throw`. An automatic
failure consumes neither dice nor Inspiration/Own Luck. Normal saves retain
named boon dice and the existing optional rescue order. Prone's separate
range-dependent attack sources remain owned by `prone-movement.coax`.

Source, core/native/hotbar evidence and remaining #13039 requirements:
`docs/verification/srd-condition-rolls-2026-11-08/README.md`. This does not qualify
every legacy save producer, condition lifecycle, immunity or concentration rule.

The existing ordinary saving-throw producers in `battle.coax` now all use that
resolver, including damage concentration, Fortitude, reactions, repeat saves
and class-feature saves. Fortitude refreshes its creature copy after the save
so its HP update cannot restore spent rescue resources. Death saves retain
their separate natural-face path. Integration evidence and remaining scope:
`docs/verification/srd-saving-throws-2026-11-08/README.md`.

Battle effect insertion in `effect-lifecycle.coax` applies immediate
incapacitation consequences only after actual retention. Concentration, Rage
and that creature's Turn Undead ownership end then, across all five
incapacitating conditions; other owners remain. Immune no-record outcomes and
full-storage refusal do not interrupt anything. Noncondition payload bits and
Command Halt do not imply Incapacitated. Zero-HP damage uses the same interruption
boundary so Rage ends immediately. Condition removal never resumes concentration.
Evidence and remaining condition clauses:
`docs/verification/srd-effect-lifecycle-2026-11-08/README.md`.

Battle death saves use `death-save-roll.coax`: eligible allied Aura, one
Bless/Bane die each and existing rescue policy apply without an ability modifier
or save proficiency. `death-saves.coax` separates the natural face from the
modified total: ten succeeds ordinarily, a natural one imposes two failures,
and a natural twenty attempts one-HP healing. A modified twenty never heals.
Rescue spending survives the final creature update; ineligible calls sample no
die and spend nothing. The standalone dice/creature primitive is unmodified.
Evidence, native scope and remaining work:
`docs/verification/srd-death-saves-2026-11-08/README.md`.

## Dodge is a retained benefit retired by actual turn and state transitions

`dodge.coax` owns Dodge's Action admission and benefit. Internal `RsCreature.dodging`
is cleared by its actual next turn start, incapacitating/zero-Speed condition
insertion and tracked speed-loss transitions. Freed creatures do not resume it.
Clock advancement and other actors' turns do not retire it. Attack/save consumers
reconcile before using it. Command Halt and exhausted movement are not Speed zero.
Patient Defense uses the same benefit; the Lab reads its flag. No protocol field
changes, but the old timestamp is replaced by a bool in the internal creature
layout. Scope, native limitation and remaining action/condition requirements:
`docs/verification/srd-dodge-2026-11-08/README.md`.
Its attack-roll benefit requires sight of the attacker; Dexterity-save
Advantage does not. Shared attack edges apply both Blinded/Invisible sight
directions in the current flag model. Alternate senses and world occlusion
still require authoritative producers. See
`docs/verification/srd-strike-reactions-2026-11-08/README.md`.

## Combat timing is a declared profile selected before the clock starts

`RsBattle.timing` defaults to `RS_STRICT_TIMING`: six-second turns, exact feet
of current Speed per turn and no automatic resource recovery. The only current
alternative is `RS_ACTION_TIMING`: a 1.5-second turn, the existing proportional
whole-square movement grant, nine-second slot/Rage/ordinary-use cooldowns and
4.5-second Focus and point/pool recovery. In this house profile standing costs half the
beat's movement grant, rounded down; strict standing uses current Speed.

`rs_set_timing` rejects unknown profiles and changes after begin, turn start
or tick without changing state. `rs_reset` restores strict and reopens selection.
No consumer assigns bare beat/recharge fields. A short action turn refreshes
its Reaction; repeat saves, death saves and Rage upkeep keep the existing
six-second round boundary. The timer policy does not grant spells, waive
materials or declare legacy free-resource paths qualified.

Checks: `timing-checks.coax`, the new policy's zero-survivor mutation gate,
strict/native witnesses, saved-seat and selected world/mode regressions.
Scope and outstanding #13045 acceptance:
`docs/verification/srd-timing-2026-11-08/README.md`.

## A world declares all of its rules deviations in one profile

`rules/srd/world-profile.coax` (`#authority`) is the one place a world
says how its combat departs from SRD 5.2.1: `RsWorldProfile` carries the timing
profile, area origin, measure and continuous walking mode, and
`rs_world_deviations` names each departure. Worlds call `rs_set_world` once,
before the clock is sealed; it applies all four fields or none. Shared rules
read only the battle's applied fields (`timing`, `areas`, `measure`, `walk`); no
world passes a literal timing, area or measure profile. `RS_STRICT_WORLD` (and
`rs_reset`) gives strict results. `RS_ACTION_WORLD` is the grid action worlds
(single 5E, MMO, Lab action); `RS_EVERGLADE_WORLD` is Everglade's profile
(`docs/dnd/everglade-combat-profile.md`, #13284): 1.5-second turns with no
cooldown recovery, harmful areas that spare the caster's team, straight-line
ranges and unpaid rate walking, for Experimental, the 5E room and Everglade II.

Under `RS_WALK_PAID`, continuous walking pays from the same turn budget as
grid steps (`walk-budget.coax`, `#authority`): one foot of movement per foot
walked, two in Difficult Terrain or crawling, three for both, plus Dash. The
walker asks `rs_walk_pay`/`rs_walk_take` before it moves and moves at most what
is granted, so it never spends past `rs_movement_left`. The part of a foot
walked carries in `RsTurn.walk_ft` and resets each turn.

Checks: `world-timing-checks.coax`, `walk-budget-walker-checks.coax` (the
actual walker), `turn-order-checks.coax` and the two zero-survivor mutation
gates. See `docs/verification/srd-world-profile-2026-11-08/README.md`.

## Casting components precede all costs and distinguish class from world grants

The compact canonical spell index retains V/S/M, priced and consumed flags
from the pinned catalog. `cast-components.coax` decides component admission
from authoritative speech, hand, focus and exact-material facts. A pouch or
authorized focus substitutes only for unpriced, unconsumed materials. A held
focus can share a Somatic gesture only on an M spell; a non-held focus exception
does not itself provide a gesture hand. A reusable priced material remains;
consumption removes exactly one complete matching material set, never generic
gold or another actor's stock. Ordinary casts pay after storage/target preflight;
Reaction and empty-target paths share the component policy.

Class-feature extra-list grants have separate provenance from world grants.
Both preserve spell access, but only the former extend the class focus's spell
eligibility. A world grant alone does not make a spell part of that class.
Operational spell helpers do not recast or repay their parent components.

These facts must come from authoritative equipment/effect state. They are not
proof of item acquisition, actual held-object ownership or a complete inventory
system. Current declared presets supply their kits; world input/gear/waiver
integration remains open. Contract and limits: `docs/dnd/casting-components.md`.
Evidence: `docs/verification/srd-components-2026-11-08/README.md`.

## A you-can-see choice refuses an unseen creature or point before costs

`cast-sight.coax` lists the executable spells whose descriptions choose a
creature or point "you can see" (compared with every canonical description and
Reaction trigger), and Vicious Mockery's see-or-hear. A Blinded caster or an
Invisible target refuses such a request before any cost, in single, chosen and
area-point casts and Hellish Rebuke. This is a malformed request, not a p.106
invalid target. Sight is the open-grid flag model; Total Cover, obscurement
and alternate senses remain world facts to admit. Evidence:
`docs/verification/srd-casting-workflows-2026-11-08/README.md`.

## An object target refuses before costs and takes damage by the object rules

`object-target.coax` decides which executable spells may attack an object
(Eldritch Blast, Fire Bolt, Produce Flame's hurl and Starry Wisp, compared with
every canonical description that names "a creature or (an) object"), an
object's AC by substance and Hit Points by size, its Poison and Psychic
Immunity, its damage threshold and Fire Bolt's ignition of an unattended
flammable object. `rs_cast_object` refuses another spell, a destroyed object or
one out of range before any cost and otherwise uses the shared cast admission
and component payment. Cover and GM-set vulnerabilities remain world facts.
Evidence: `docs/verification/srd-casting-workflows-2026-11-08/README.md`.

## Simultaneous spell saves share damage before individual defenses

`spell-damage-rolls.coax` retains one pre-save damage roll per instruction in
an explicitly local cast/burst/pulse scope. Each recipient rolls its own save
and applies its own defenses. Roll modifiers precede save halving. Attacks and
separate trigger invocations do not share; a new pulse gets a fresh scope.
World batch callers must pass one scope across simultaneous recipients rather
than repeatedly invoke the single-recipient convenience wrapper. Evidence and
remaining qualification: `docs/verification/srd-shared-damage-2026-11-08/README.md`.

## Dash credits current walking Speed without truncating feet

`dash.coax` shares admission and movement credit across Action Dash, Cunning
Action and free Step of the Wind. Each uses current tracked walking Speed;
zero-Speed states cannot gain movement. Whole-square and residual feet remain
exact. Action/Bonus Action budgets and feature events stay distinct; refusal
changes no budget, movement, dice or events. The declared action profile retains
its existing full-Speed Dash grant. Scope and remaining mid-turn/special-speed
work: `docs/verification/srd-dash-2026-11-08/README.md`.

## Walking movement reprices current Speed without refilling spent movement

`movement-credit.coax` keeps the previous ordinary-plus-Dash allowance and
overspending debt in the internal turn. Speed loss and restoration preserve
feet already spent; each walking Dash follows current Speed. New turns reset
credit/debt/Dash count. Step and stand publish repriced credit only after
successful payment; explicit movement queries reconcile the current actor.
Legacy untracked turns bind their pre-Dash basis before gaining a Dash.
Grid walking loops use that query; saved seats retain debt with the full turn.
Special speeds and feature-specific free movement remain separate requirements.
Evidence: `docs/verification/srd-movement-credit-2026-11-08/README.md`.

## Stunned movement is separate from incapacitated action and initiative effects

`condition-permissions.coax` allows a Stunned or Incapacitated creature to move
and stand, subject to independent movement blockers, Speed and payment. Stunned
still forbids actions/reactions and retains its shared save, attack and lifecycle
consequences. Initiative resolves incapacitation Disadvantage against Athlete or
Invisible Advantage with normal cancellation. Consumers must not use action
capability as a blanket movement predicate. The separate hostile-AI held adapter
remains migration work. Scope and evidence:
`docs/verification/srd-condition-permissions-2026-11-08/README.md`.

## Experimental hostile action and movement projections are independent

The physical Enemy's `held` blocks movement; `inactive` blocks starting or
continuing attacks. Experimental derives them from shared movement/current-Speed
and action permissions separately after accepted effects and expiry. Loss of
action permission retires a pending swing immediately and before physical hit
stop; recovery starts a new swing. Return-home movement respects held, stopped
and dead bodies. The legacy hostile damage/HP path still needs shared migration.
Evidence: `docs/verification/srd-hostile-permissions-2026-11-08/README.md`.

## Printed NPC melee attacks use the common strike executor

`stat-attack.coax` validates typed source/world-declared melee records and NPC
Action admission before spending. Printed attack/damage modifiers are supplied
explicitly, without rewriting scores or inventing catalog weapons. Normal weapon
and typed stat attacks share the resolved strike/target-defense/HP path. Natural
zero-dice damage has no Unarmed Strike base and cannot gain Sacred Weapon.
Ordinary weapon preflight remains. A record may also be ranged or thrown and
carry second damage, an Advantage rider and a Prone or Poisoned rider. Admission
(single attack, Multiattack and Opportunity Attack) refuses before spending
anything when the target has no effect room for every rider it could receive.
Multiattack picks each attack's record again from where the creatures stand and
stops once the target is down or the attacker can't act. Monster numbers come
from `monsters.coax`, the canonical catalog, and every world role in its
used-monster manifest names a block the engine builds.
Evidence: `docs/verification/srd-stat-attacks-2026-11-08/README.md`,
`docs/verification/srd-used-monsters-2026-11-08/README.md`.

## Stabilization records one recovery deadline per actual stable episode

`stabilization.coax` admits Help first aid through the shared Wisdom (Medicine)
check. An admitted attempt spends its Action even on failure; refusal is
transactional. Success and a third battle death-save success reset both save
counts without healing/waking, then arm one 1d4-hour logical-clock deadline.
Admission arms a standalone stable creature and preserves restored deadlines.
Clock and turn boundaries recover one HP only while living, stable and at zero;
blocked healing retains the deadline, and independent Unconscious owners remain.
New stabilization replaces an inactive prior episode's deadline after damage.
Raw borrowed-state producers must report the transition when clock ownership
begins. Saved-seat first aid spends only that player's Action. Exact native,
world/control and remaining recovery scope:
`docs/verification/srd-stabilization-2026-11-08/README.md`.

## A knockout owns its Unconscious; a Healer's Kit stabilizes without a check

`knockout.coax` decides Knocking Out (SRD 5.2.1 pp.17, 185). Only damage from the
melee attack the strike executor is resolving, whose attacker chose
`RsCreature.knockout`, can knock out, and only when it would reduce a living
creature above 0 HP to 0 after Temporary HP. The creature stays at 1 HP with an
`EF_KNOCKOUT` record owning Unconscious, Prone and no held focus, and starts a
Short Rest. Massive damage, Monster Death and Undead Fortitude never apply to that
damage. The record ends when that Short Rest finishes, when the creature regains
any HP, or on successful first aid; it is never clock-retired, and other
Unconscious owners and Prone stay. A creature immune to Unconscious, or with no
free effect record, takes the damage as dealt. `rs_healers_kit` spends the Action
and one use to stabilize an Unconscious creature at 0 HP with no check.
Condition owners (which record or state owns each of the 15 conditions) are listed
and checked in `rules/srd/hp-interaction-checks.coax`. Scope:
`docs/verification/srd-knockout-conditions-2026-11-08/README.md`.

## Experimental physical contacts resolve through shared NPC turns and hero HP

The live Experimental frame binds contact-only physics before its first tick.
A bounded FIFO carries actor IDs; it cannot manufacture damage or overwrite
accepted contacts. Saved NPC turns advance even without contact, and shared
stat attacks spend only that actor's Action. The shared hero owns HP/temp HP;
casts do not synthesize/reset health and the HUD projects class maximum and TEMP.
Legacy damage refuses in shared mode. Landed events drive each striker's hit
stop; positive loss alone drives damage pops. Respawn resets the shared preset,
clears pending contacts and preserves defeated foes. Named body attack profiles
are still house data, and the per-press resource adapter remains unqualified.
The subsequent checked Lab Wasm build clears the three reproduced baseline
check failures; browser/runtime qualification remains separate. See
`docs/verification/srd-siege-build-2026-11-08/README.md`. Contact evidence: `docs/verification/srd-contact-health-2026-11-08/README.md`.

## Help is a next-roll benefit owned by the helper's actual turn

`help.coax` admits one trained skill/tool for another ally, or a distraction on
an enemy within one five-foot grid square. Effect retention precedes Action
spending; capacity refusal loses neither resources nor another owner. Matching
ability-check Help joins Advantage before cancellation and retires on the next
admitted matching check, even if it fails automatically. Malformed requests,
other skills and passive scores leave it pending. Current helper capability and
per-helper task/geometry exclusions decide whether assistance can contribute.
Attack distractions benefit the next attack by another ally of the helper,
including spell/ranged attacks and misses; independent matching owners consume
together without stacking Advantage. Only that helper's actual next turn clears
its unused records; clock expiry never substitutes for that event. Saved-seat
adapters preserve the other actor's turn. Caller facts, native scope, world input
and remaining action/recovery boundaries:
`docs/verification/srd-help-2026-11-08/README.md`.

## Every press is paid

No world profile refills what a press spends (#13284,
`docs/dnd/everglade-combat-profile.md`). `RS_UNLIMITED_PRESS` and
`rs_press_prepare` are deleted. An ability's cost is its Ability record's
(`rules/srd/ability-record.coax`) and is paid in every world; it comes back
by the SRD's rests, or on the action profile's declared cooldowns. Everglade's
room gives a seat one turn each 1.5 seconds (`everglade/shared/cast-pace.coax`,
`#authority`): one Action, one Bonus Action and one slot spell, opened on its
first press, with the room's refusal undoing the opening. A single-player
zone's turn is its one-second press cooldown. `press-budget.coax`
keeps only the snapshot and restore a single-player zone uses to undo a
refused press. An accepted press never renews a Reaction; only a real turn
does. Checks: `everglade/shared/cast-rules.coax` (the level 5 party),
`everglade/server/room-checks.coax` (the room), `everglade/shared/cast-pace.coax`.

## Everglade II broadcasts accepted casts only after shared execution

`eg_e2_cast` retains class-slot/aim/range/network-cooldown validation and calls
the transactional `everglade/shared/cast-rules.coax` executor before publishing acceptance.
Self/ally entries affect the caster through shared rules. The declared social
profile sends offensive spells through shared empty-target costs/lifecycle
without damage or a harmful zone; empty swings spend an Action without a hit.
Unlimited press preparation does not renew Reactions or movement, and refusal
restores its budgets, seat and dice. Other actors' turns stay intact. The page
still predicts presentation; visible state, acknowledgement and broader world
qualification remain work. Evidence:
`docs/verification/srd-e2-shared-cast-2026-11-08/README.md`.

## Everglade II renders shared-rule refusals as bounded cast feedback

Only a well-formed positive sequence matching the latest local cast updates
cast feedback. Fixed messages replace server reason words; raw server text is
never drawn. The hotbar shows the message for four seconds, hides negative
clock ages and clears it on a new locally admitted press. Connection-version,
elsewhere and restart handling retain priority. Already predicted sound/effects
are not undone; shared state projection and broader runtime qualification remain
separate. Evidence: `docs/verification/srd-e2-refusal-feedback-2026-11-08/README.md`.

## Everglade II's health label reads the server mirror

The hotbar reads the current seat's valid WK_PLAYER WireMirror row while
connected/seated: HP/max, positive TEMP and zero-HP unconscious/dead status.
It does not initialize visible HP from class defaults or predict healing.
Unknown/invalid/wrong-seat state hides the label. Full snapshot omission,
mirror reset and disconnection cannot leave invented health visible. Movement
ACK and remote render-pool capacity are not prerequisites. Existing server rows
already carry these fields; no wire/core policy changes. Evidence and remaining
world scope: `docs/verification/srd-e2-vitals-2026-11-08/README.md`.

## Room departure removes participation without manufacturing death

`e5rr_leave` marks the seat inactive and moves its creature off-map without
changing actual HP/death. The retained owner permits the room's declared
summon/zone persistence across disconnect (multiplayer spec 6.3). Shared
concentration still ends on actual zero HP/death or duration; a new player
replaces the owner and retires old effects. Leave neither heals nor revives,
and existing HP rejoin persistence remains. The Conjure Animals and horde traversal baseline
failures are fixed; all 325 room-library checks pass. Evidence and scope:
`docs/verification/srd-room-leave-2026-11-08/README.md`.

## Real local sockets retain health and acknowledge E2 class casts

The checked native host passes actual WebSocket HP rejoin fixtures for both
hurt and zero-HP players, with zero malformed rows and no reload heal. E2's
interest harness now uses the real Druid class kit and observes acceptance,
refusal and peer events rather than sending an unchecked legacy slot. Its
four-client 90-second run accepts all 36 casts and retains/culls every sampled
near/far pair correctly. These are bounded local socket receipts, not browser,
restart/drain, every-class or production-scale qualification. Evidence:
`docs/verification/srd-room-sockets-2026-11-08/README.md`.

## Disengage belongs to the current turn and has distinct feature costs

`disengage.coax` admits Action Disengage, Cunning Action and the free/paid
level-two Patient Defense choices before spending. Free Patient Defense costs
no Focus; paid adds Dodge for one Focus. The current turn retains Disengage
through movement/saved seats and clears it on actual turn end; Dodge keeps its
next-turn-start lifecycle. The existing paid feature preserves its event.
Zero Speed alone does not prohibit this Action; incapacity and illegal budgets
refuse. Opportunity attacks still require implementation before the defensive
consequence is qualified. Evidence and remaining scope:
`docs/verification/srd-disengage-2026-11-08/README.md`.

## Opportunity triggers are queried without spending or changing movement

`opportunity-trigger.coax` requires explicit authoritative melee reach,
visibility and own-resource movement facts and reads logical/grid actor state,
Reaction availability, Addle and the active mover's Disengage. Teleportation
and unpowered forced displacement do not trigger; reach departure is exact.
Addle now retains its opportunity-only suppression until actual turn start,
not clock expiry, without consuming unrelated Reactions. This is a trigger
query, not Reaction strike execution or movement integration. The pending
executor must preserve true mover-turn spell budgets and strike-local state.
Evidence: `docs/verification/srd-opportunity-trigger-2026-11-08/README.md`.

## Opportunity strikes preserve the actual turn while borrowing attack state

The shared opportunity executor admits exactly one represented melee weapon
attack/Unarmed Strike, spends its Reaction on a hit or miss, and rolls back
all state on refusal. It resolves before movement; ordinary movement adapters
are not yet wired. A retained reaction frame keeps true owner spell budgets
and Shield duration separate from strike-local marks, and off-turn rider
deadlines use the upcoming reactor turn. Returned strike state carries optional
follow-up credit without changing the mover's budget. This supersedes the
pending-executor statements above, not their movement-integration limitation.
Evidence: `docs/verification/srd-opportunity-attack-2026-11-08/README.md`.

## Selected opportunity attacks precede paid grid steps without an outer rollback

`opportunity-movement.coax` shares read-only ordinary step admission with
`rs_step_to`, resolves explicit selected Reactions before departure, and
rechecks movement after each accepted strike. An interrupted move retains
accepted damage/Reaction spending, spends no movement and stops soliciting
later reactors. A completed move uses normal payment and entry effects once.
Lab grid/player-token/Siege walks select nearby visible foes through this
boundary. This does not wire Experimental's separate continuous controller or
Everglade II's network movement. Evidence and remaining scope:
`docs/verification/srd-opportunity-movement-2026-11-08/README.md`.

## Typed monster opportunities use declared melee numbers and reach

`stat-opportunity.coax` resolves exactly one trusted typed melee stat-block
attack through the shared strike executor and retained Reaction frame. Printed
numbers/reach replace weapon-bank guesses; owned Charmed prohibition, Reaction
spending on miss, true mover-turn spell budgets and refusal atomicity remain.
The record has no mastery/Finesse retained rider. The physical controller is
still a separate integration requirement; this does not alter E2's social
no-bystander-harm profile. Evidence and limits:
`docs/verification/srd-stat-opportunity-2026-11-08/README.md`.

## Experimental player locomotion resolves measured monster opportunities before publication

The physical controller supplies its real pre-step and proposed bodies to
Coax `zone_keep_body` (`everglade/zone.coax`; fivelab's `lab_keep_body` hands it
on) before publishing feet. Experimental measures departure
in terrain-adjusted feet, uses shared trigger/typed Reaction execution with
the saved player turn, and retains accepted damage if movement is stopped.
Disengage, ordinary sight and Addle remain shared; teleport placement updates
its origin without provoking for that displacement. Hit/number presentation
uses the same result path as physical Action contacts. This wires incoming
monster opportunities on player locomotion; monster AI departure/player
Reactions and broader physical distance/timing qualification remain separate.
Evidence: `docs/verification/srd-physical-opportunity-2026-11-08/README.md`.

## Experimental monster locomotion stages player opportunities before publication

The Lab advances shared-combat AI into local proposals; Character Create skips
its own hostile tick only while the Lab explicitly owns it. Each actual mover
keeps its prior physical position until the player's shared opportunity query
and strike resolve under the mover's saved turn. A miss spends Reaction; a
lethal hit retains damage and prevents departure. Later movers cannot reuse
that Reaction. Nonmoving AI state/contact intents and once-per-frame clocks
remain intact; per-enemy permission projection preserves shared death/action/
movement state without repeated army scans. This completes the two locomotion
producer boundaries, not all movement/timing/world qualification. Evidence:
`docs/verification/srd-monster-departure-2026-11-08/README.md`.

## Shield's AC benefit retires at the actual caster turn start

`shield-lifetime.coax` retains the admitted spell's active state and provides
its flat AC bonus. `rs_start_turn` retires it; clock-only advancement, another
actor's turn and a borrowed strike frame do not. This removes the obsolete
`shield_until` forecast, which double-counted a beat in already scheduled saved
seats. The Lab label and AC use the same state. Earlier Shield forecast checks
are superseded by real strict/action saved-seat trigger/retirement witnesses.
Other numeric deadlines and general turn scheduling remain separate work.
Evidence: `docs/verification/srd-shield-turn-2026-11-08/README.md`.

## Accepted locomotion reports Short Rest interruption without refilling history

The grid step reports movement only after admission/payment. The player body
path pairs actual admitted displacement with the controller's own dead-zone/
world-gated intent, and reports actual requested jump rises. Attempts with no
motion and passive displacement do not invent strenuous activity. Accepted
monster AI locomotion uses the same rest-state transition. In-progress rest and
post-completion choices close while Hit Dice and daily recovery history remain.
Rest must begin from actual settled downtime; controls, network movement and
other producers remain separate. Evidence:
`docs/verification/srd-rest-activity-2026-11-08/README.md`.

## Long Rest bank recovery is separate from timing admission

`long-rest-resources.coax` preflights represented counters/provenance before
resetting slots/uses/Focus/Rage/Hit Dice, daily recovery history, Temporary Hit
Points and cooldown bookkeeping. Remaining created slots are explicit; recovery
does not infer them from class or erase unrelated excess banks. Turn/Reaction
and HP state are not granted by this primitive. It has no world completion
caller yet: sleep/activity/interruption/16-hour admission and other completion
benefits remain separate required work. Evidence:
`docs/verification/srd-long-rest-resources-2026-11-08/README.md`.

The additive Long Rest journal retains actual activity modes and interruption
extensions separately from Short Rest choices. Advancement reports the first
completion/exertion/light boundary; a late poll cannot move completion history
later. Pending completion grants no benefits until its owner applies and
acknowledges them. Its private integer-second state has no world caller yet;
condition ownership, causal event hooks, full benefits and persistence remain
required. See `docs/verification/srd-long-rest-state-2026-11-08/README.md`.

Natural sleep has its own event-owned effect record. Admission retains capacity
before incapacitation consequences; full storage cannot end concentration or
evict an effect. Waking retires only sleep ownership, preserving independent
raw/recorded/zero-HP Unconscious sources and leaving Prone. Ordinary clock
expiry cannot wake it. World/journal transitions and full inventory dropping
remain separate. See `docs/verification/srd-rest-sleep-2026-11-08/README.md`.

Long Rest completion consumes an owned pending creature journal boundary once.
It preflights body/resource counters, restores source-authorized recorded losses
without erasing later increases, respects shared healing and food/water
Exhaustion restrictions, wakes only natural sleep, closes old Short Rest choices
and commits the actual completion timestamp. It grants no turn/Reaction budget.
World causal scheduling, source loss/nutrition producers and persistence remain
required. See `docs/verification/srd-long-rest-completion-2026-11-08/README.md`.

Admitted battle ticks advance a private integer-microsecond rest clock with
fractional carry. Invalid time/metadata or overflow refuse before ordinary tick
effects; rs_reset resets its ownership. The journal retains its explicit second
or microsecond unit across completion history and cannot reinterpret that
history by changing units. Old f32 clock assignments do not migrate the rest
timeline. Actor synchronization, causal event hooks, world controls and rejoin
binding remain required. See `docs/verification/srd-rest-clock-2026-11-08/README.md`.

Clock-bound Long Rest actor operations observe the previous activity before
changing mode. Admission preserves sleep capacity and settled-owner proof;
observed boundaries survive a refused mode request. Exact light-limit changes
can select sleep, while a report beyond that limit abandons the old period
without inventing intervening sleep or a source interruption penalty. Earlier
completion remains pending for ordered consumption. Automatic scheduling and
causal producers remain required. See
`docs/verification/srd-long-rest-owned-2026-11-08/README.md`.

## Shield answers Magic Missile before any dart strikes

A Magic Missile target without Shield up is offered the existing Shield Reaction
once, before its first dart; a target with Shield up takes no damage from any
dart (SRD 5.2.1 p.161). The decision is `rules/srd/magic-missile-ward.coax`
(`#authority`); slot/Reaction admission stays in `rs_shield`. Inflict Wounds,
Acid Splash, Magic Missile and Mass Healing Word execute through the existing
closed step set. Checks naming "the first unassigned spell ID" use
`RS_MORE_END`. See `docs/verification/srd-slice-spells-2026-11-08/README.md`.

## A caster is in its own area unless the area starts at it or a world declares otherwise

Strict battles include a caster standing inside its own Sphere, Cylinder or
Cube at a point (SRD 5.2.1 Areas of Effect; Fireball p.131) and leave it out
of a Cube or Line originating from it. The decision and the declared house
profile `RS_SPARE_CASTER_AREAS` are `rules/srd/area-origin.coax`
(`#authority`), consulted by both the cast loop and the effect-room preflight.
New battles are strict; the profile is chosen before the clock is sealed and
`rs_reset` restores strict. Worlds that use action timing declare the house
rule explicitly. See `docs/verification/srd-area-origin-2026-11-08/README.md`.
A spell whose text says "each creature (other than you)" (`RsSpell.others`,
Entangle p.128) leaves its caster out of any area, and a spell that affects
only a creature that can see it (`RsSpell.sight`, Hypnotic Pattern p.141)
leaves out a Blinded creature; both are decided in `area-origin.coax` for the
cast loop and the preflight alike, so a left-out creature needs no effect room.
See `docs/verification/srd-kit-spells-2026-11-08/README.md`.

## Rest boundaries precede later time

Opt-in battle-owned rest scheduling splits time at actor boundaries. Journal
credit precedes ordinary expiry through the boundary, and completion/fallback
is consumed before later time. Fractional carry cannot rewind either timer
epoch or let later expiry retroactively permit healing. A light-limit marker
prevents repeated same-quantum stalls. Explicit Short Rest consumes its quiet
segment; exertion closes old choices, and earned fallback is granted once
without automatic HP healing or renewed Reaction. Causal damage/casting/
Initiative hooks and world activation/persistence remain required. Evidence:
`docs/verification/srd-rest-scheduler-2026-11-08/README.md`.

## A spell's save rider is one more Disadvantage source, never an override

A SAVE step's `c` field names its rider (`rules/srd/save-riders.coax`,
`#authority`); Shatter's Construct rider adds one Disadvantage source that
joins the target's other Advantage and Disadvantage sources before they cancel.
`rs_saving_throw` and `rs_save_edge` keep their meaning as zero-rider wrappers.
See `docs/verification/srd-shatter-2026-11-08/README.md`.

## HP protection preserves damage taken and rest interruption

Accepted battle damage interrupts a synchronized Long Rest before HP snapshots
or protection saves, including Temporary HP absorption. Earned Short Rest banks
are available to the damage Reaction without renewing its budget. Natural sleep
wakes without erasing independent Unconscious ownership. Zero or entirely
house-floor-prevented damage does not interrupt. Capacity exhaustion retires an
unresumable rest instead of leaving it active. Optional recovery choices and
same-time controller ordering remain separate required producers.
Undead Fortitude protects remaining HP, not damage taken: full damage still
consumes Temporary HP, ends Turn Undead and calls for concentration. Saving
throw rescue expenditure is retained before the HP snapshot. Evidence:
`docs/verification/srd-rest-damage-2026-11-08/README.md`.

## Casting interruption precedes spell payment after admission

Ordinary and empty-target casts apply admitted non-cantrip rest interruption
before component/slot payment. Shared Reaction payment applies it after
spell/component and true-owner budget admission, before spending its slot.
Earned Pact recovery cannot refund a paid cast. Refusals and cantrips preserve
rest; damage floors do not suppress casting interruptions. The common consumer
retains one interruption/recovery and grants no turn or Reaction. Empty-target
free casts consume their source use where required, preserve unlimited Vigor
and never spend slot zero. Owners still synchronize journals before admission;
world activation, Initiative and optional ordering remain required. Evidence:
`docs/verification/srd-rest-casting-2026-11-08/README.md`.

## Initiative interrupts only its actor and terminal rest cannot stall time

`rs_roll_initiative` owns actual shared rolls; `rs_add` calls it only after
logical actor admission. Other actors' rests remain intact. `rs_begin` merely
schedules retained Initiative. Rolling interrupts the actor's synchronized rest
without itself waking natural sleep, so Incapacitated Initiative Disadvantage
and ordinary cancellation remain. Dead/borrowed/invalid rolls refuse.
Shared battle damage, death-save and Exhaustion terminal paths retire all rest
phases and pending fallback without revival/recovery, preserving completion
history and spent banks. Dead admission does the same without rolling a die.
Earlier earned damage-interruption recovery remains valid. Nonterminal
Exhaustion does not interrupt rest. World observations, optional ordering and
persistent epoch binding remain required. Evidence:
`docs/verification/srd-rest-events-2026-11-08/README.md`.

## Short Rest deadlines belong to the integer rest timeline

Short Rest retains an owned microsecond start; only that clock can satisfy its
one-hour deadline. Display-clock assignments cannot grant recovery. Admission
refuses an unrepresentable hour before seeding/mutation, and malformed active
rest metadata refuses time atomically. All ticks split at Short Rest boundaries,
including fractional/overdue observations, without duplicate recovery or lost
later Long Rest quiet credit. Long Rest journal scheduling remains opt-in;
manual journals are not advanced by this change. Internal f32 rest anchors are
not reinterpreted as integer anchors; persistent epoch migration remains open.
Evidence: `docs/verification/srd-short-rest-time-2026-11-08/README.md`.

## Everglade II rest belongs to the room

An Everglade II room enables the shared rest scheduler for its battle and is the
only authority over a seat's rest. A page asks with `Z SEQ REQ` and hears the
room's state as `z SEQ CODE DICE`; it never decides rest locally. The room
admits a request only through `rs_rest_control`, only after it has itself seen
that seat placed, grounded, still and with released controls in the latest
accepted step, and every input still queued also holds its controls released. Walking and jumping are observed from
the room's accepted steps and displacement, never from page claims; a sleeping
seat's steps carry no locomotion. Recovery happens once, in the scheduler. A
grace rejoin keeps the creature, rest and cast history and tells the new socket
the current state; the new page must be seen still again before it changes the
rest. Pre-v5 `L` rest requests are refused in Everglade II rooms. Evidence:
`everglade/server/rest.coax` checks and
`docs/protocol/everglade-session-v7.md`.

## An Everglade II seat plays at the level its hello names, and the room says its bar

Character Create's LEVEL (1 to 6) is kept in the appearance record (CC4 past
level 1; CC3 reads as level 1), and the hello's KIND says it as `srd-CLASS-L`.
The room seats the creature at that level through the shared rules
(`e5rr_join_level`), rejoins a seat whose level changes, and never rebuilds an
Everglade II seat with Everglade's XP levels. A wire slot is the ability's place
in its class's level 1 to 6 catalog (`xk_wire`), so level 1 slots keep their
numbers; `eg_e2_cast` and `e2r_apply` take a slot only from the seat's own
level's bar (`xk_entry_at`). Weapon swings, Flurry and Action Surge pay their real
Action, Bonus Action, Focus and uses; nothing picks a creature. The room is the
only source of the bar's counts and held features: it sends `b N V...` per seat
after an accepted cast and on change, and the page draws only what it said. A
cast the room refuses is undone on the page: its predicted voice fades out and
its puffs fade to nothing, never cut. Reactions and Opportunity Attacks need a
hostile creature, and Everglade II has none until the owner decides its social
profile. Evidence: `docs/verification/srd-e2-levels-2026-11-08/README.md`.

## A carried rest earns nothing offline and keeps the Long Rest gate

Character checkpoint profile 3 carries a seat's rest as 19 bounded integers
(`rules/srd/rest-carry.coax`, `docs/protocol/character-checkpoint-v3.md`).
`rs_carry_in` lays it on all or none: a field out of range, two rests in
progress, a mode without an open journal, a completion age without a
completion, parts that exceed the elapsed time, more Hit Dice than the body has,
or a body that cannot sleep for an open sleeping rest refuses it, and the body
is unchanged. Times resume as captured and are stamped at the restore, so time
between processes earns no rest. The completion age keeps the 16-hour gate, and
an Everglade II room's rest clock starts at `RS_CARRY_EPOCH`, so that age is
always representable. A dead or 0-HP body keeps its Hit Dice and history but no
rest in progress. Profile 2 readers never see a profile 3 body: the API gives a
profile 3 body's profile 2 part unless the page asks `rest=1`, and a room whose
profile 3 post is refused (400 or 5xx) sends profile 2 for the rest of its
process. Evidence: the `rest carry` checks (zero mutation survivors in
`rest-carry.coax`), the `character checkpoint` checks (zero root survivors),
the `e2 rest` room checks and the `resume3` API checks.

## Everglade II first aid uses room contact and the shared Action

Everglade II grants one ten-use Healer's Kit to a new room seat. Class changes
and grace rejoins preserve its charges. The page asks with the versioned
`U1 SEQ` extension. The room chooses a dying ally using stepped positions and
the forest's trunk and rock capsules. The room's saved turn and the shared
`rs_healers_kit` executor decide the Action, charge and stabilization. The
page presents the room's acknowledgment. See
[Healer's Kit v1](docs/protocol/everglade-healers-kit-v1.md).

## Experimental launches jumps from its saved SRD turn

Experimental measures the running start from accepted grounded motion.
Directional input chooses a Long Jump and a stationary press chooses a High
Jump. The saved turn supplies the jump score, doubling and remaining movement.
A refused controller step spends no movement. An admitted rise pays through
the shared rules, and the body remains within the admitted horizontal reach.
The ordinary controller keeps its own jump profile for other worlds.

## Wild Shape exchanges one known form after Long Rest

The shared creature keeps its Wild Shape known-form mask. An unset mask uses
the level's recommended forms. Completing a Long Rest grants an eligible
Druid one choice to replace a known form with an eligible unfamiliar form.
Rejection preserves the mask and choice. The Cat is a CR 0 option from the
pinned SRD. Wild Shape borrows and restores Jumper along with the rest of its
physical form. Experimental shows the outgoing and incoming choices in its
rest panel, and the Wild Shape slot selects among the forms still known.

## A familiar owns one non-attacking actor in its owner's battle

The familiar stores its owner as an actor index plus one. Recasting replaces
the living actor's form in place or reuses its retired slot. A familiar
receives its own initiative and turn. A rejected Wild Companion request
changes neither the owner's turn and resources nor the roster. An accepted
request spends the Magic action and the selected resource. The familiar
leaves the grid at 0 HP, and the Druid's Fey familiar also leaves when its
owner completes a Long Rest. Attack admission rejects familiars across
weapon, natural attack, Unarmed Strike, Ready Attack and Opportunity Attack.
Temporary dismissal spends the owner's Magic action and removes the familiar
from the grid and turn scheduler while preserving HP and initiative. Returning
spends another Magic action and requires an unoccupied square within 30 feet.
Permanent dismissal retires the familiar. Recasting can change a pocketed
familiar's form without bringing it back into the world.

A living, capable familiar within 100 feet can use its Reaction to deliver
an executable Touch spell. The casting path checks reach and cover at the
familiar. The owner supplies the spell, action, components, slot, attack
modifier and save DC. A refused cast spends neither actor's resources.

The owner can borrow a linked familiar's sight and hearing for one Bonus
Action until the start of the owner's next turn. The shared casting sight
checks can use either viewpoint, and world adapters can read the familiar's
special-sense range. For creature choices, borrowed Bat Blindsight uses
its own 60-foot range and respects Total Cover from the familiar. Blinded,
Darkness and Invisible do not defeat that sense. Experimental supplies
the familiar's own wall fact when the player selects a creature.
Commands can reach a familiar by telepathy within 100
feet or through audible speech supplied by the world. An admitted command
uses the familiar's own turn and shared movement, Opportunity Attack and
action rules.

Experimental's familiar hotbar uses the real turn seat and shared summoning
rules. The Wizard's preset has Find Familiar copied into the spellbook and
prepared in place of Mage Armor. The hotbar offers both the seventy-minute
ritual and one-hour prepared cast, and lets the player choose a spirit.
The Wizard has one incense supply per visit. Preset changes preserve the supply count. A level 2 or higher Druid
can choose a Wild Shape use or level 1 slot. Each summoned familiar retains
a world position, an independent turn seat, and a marker at its position.
Its top-left badge selects the lifecycle action or a Step command. Pocket,
Recall and Free use the owner's Magic action. Step uses the familiar's own
movement in the direction the owner faces. A blocked step spends no
movement, and a hostile Opportunity Attack resolves before departure.

Experimental's Touch spell badge switches between Hand and Fam. Fam
targets the clicked creature for offensive spells and uses the familiar's
position for reach and cover. The owner's saved turn and actual slot
balance are authoritative, and the familiar's Reaction must be available.

Experimental's Sense command spends the owner's Bonus Action. While the
shared sight link is active, the camera uses the familiar's eye and the
dungeon selects torches around that eye. Unlit surfaces within the form's
darkvision range render in gray. Spatial combat cues use the familiar's
position when it can hear. The owner and familiar keep their world
positions, and the view and listener return when the shared link ends.

The eleven named SRD familiar forms use stable monster IDs and the same
picker and admission list. A change of form preserves the existing actor
and replaces its Swim Speed along with the other movement statistics.

Everglade II reserves one familiar actor per seat. Its Wild Companion
request uses the room's position and world collision query, and spends the
owner's saved turn through shared rules. Replay, blocked placement and
invalid class or resource requests do not spend a use. A completed Long
Rest, retired seat or replaced preset removes the companion.

Everglade II familiar presentation follows the room's separate v1 state
snapshot. Visible rows carry the form, position and Hit Points, ordered by
owner seat. Either player's block removes the row from that recipient's
view. A complete empty snapshot removes all markers. The client applies
complete, strictly newer snapshots and forgets them on disconnect.

Everglade II's Wizard Find Familiar takes the shared one-hour prepared or
seventy-minute ritual path. The room keeps the owner's saved turn and
Concentration, and only a completed cast consumes incense and any slot.
Placement uses the room collision query. A seat's one preset incense
supply is tracked across class or level changes during that seat's visit.

Everglade II familiar lifecycle requests resolve through the owner's saved
Magic action. A pocketed familiar retains Hit Points while absent. Recall
requires room collision clearance, and refusal does not spend the action.
Release retires the familiar. The room rejects replayed familiar requests.

Everglade II's familiar Step uses the owner's admitted facing and the
familiar's room position, saved turn, movement and telepathic link.
The world collision query starts at the familiar. A blocked Step leaves
its movement unspent, and the owner's turn is unchanged.

Everglade II extends the room grid south through its distant clearing.
Room Z conversions use the room's grid offset in both directions. The
original Everglade grid and the world coordinates on the wire retain their
existing origins and units.

A familiar Step can provoke a hostile room NPC's Opportunity Attack.
The room samples sight from each eligible attacker to the departure square
and checks the route independently. The shared Reaction resolves before the
step. A blocked route never provokes, and a lethal Reaction persists while
the step spends no movement.

Everglade II Druids can deliver Cure Wounds through their familiar. The
room checks the target seat, mutual block state and contact from the
familiar. The shared executor uses the Druid's saved Magic action and
spell slot and spends the familiar's Reaction on admission.

Everglade II borrowed familiar senses spend the owner's saved Bonus
Action. The room publishes viewpoint state only to the owner. The
shared link and start of the owner's next turn determine when it ends.
The client also requires a current familiar row to move its camera or
spatial spell listener.

The Experimental and Everglade II pack builders share the original
familiar mesh source. Their glade/familiar catalog rows use the room's
stable named-form order and model-space metres, with positive Z forward.

Familiar draw records use the packed model's own index range and vertex
base. They use the rules actor's world position and are hidden when that
actor disappears or lends its viewpoint to its owner.

## Guidance modifies checks for one chosen skill

Blessed Warrior grants Sacred Flame and Guidance at Paladin level 2.
Guidance requires a willing creature in Touch range. Its newest active
record supplies one d4 to each rolled check using the chosen skill.
Passive Perception rolls no die. Concentration or expiry retires the
record. Experimental's skill badge chooses the skill, and Everglade II's
self control currently uses Perception.

## Zone of Truth binds saves to the leased sphere

A creature saves on a turn start inside or its first entry on a turn.
Failure belongs to that sphere and prevents deliberate lies while inside.
Releasing the zone clears only its own save state. Experimental keeps the
center in world coordinates while projecting the rules grid around the
player. The Everglade II room tests entry after applying a seat's pose.

## A released attack lands after its maker falls

Anything already released resolves; anything not yet released is cancelled
by the fall. `released.coax` stands the maker for that one resolution (up,
at 1 Hit Point when it had none, without its fall's conditions and halts)
and puts it back down after, keeping what the resolution spent. Experimental
uses it for an arrow that reaches a monster and a blow already swung; the
Everglade II room uses it for a cast in flight (RC_FLIGHT). A draw, a cast
bar or a new press is still refused once the maker is down. Checks:
`released:` in the SRD engine, the two `(#13120)` checks in
`everglade/zone-combat.coax`, and `casts: a cast released before its caster
falls` in `room-casts.coax` (GitHub #13120).

