# Invariants — metaverse-engine/INVARIANTS.md

> Scope: Metaverse Engine, render, physics, audio, anim, editor, scripts, net, session (37 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## Realm creation is gated by one server-side rule

`access.gate` (`coax/web/api/migrations/0090_realm_invites.sql`) is the only
rule for who may plan, make or fork a realm. `realms.c` asks it, through
`access_refused` (`access.c`), before `plan`, `pick`, `plan/doc`, MAKE IT and
a FORK's POST. With the `realm_invites` flag row absent or off it admits
everyone, so behaviour is what it was before 0090; on, it admits site admins
and approved creators (an account, or a grandfathered guest player) and
answers 403 `invite only` to everyone else. Pages only hide links; they never
decide. An invite code is kept only as its SHA-256, and a use is spent in the
same statement that checks it. Checks: `coax/web/api/test_access.py` (each
gated route refused on, admitted off and for creators and admins; codes
hashed, spent once, revoked and expired refused).

## A private realm is seen only by its owner and site admins

A made realm's `visibility` (`coax/web/api/migrations/0099_realm_visibility.sql`)
is `private`, `unlisted` or `public`; a new realm and a new fork start private.
For a private realm every route under its slug answers 404 to anyone but its
owner (`verse.builder_owns`, which a NIP-98 caller meets through its account)
and site admins: `verse.realm_hidden` decides, and `realms.c` asks it before
any slug route. It has no page in the site bucket (writing one removes it), is
not in `verse.builder_og_rows` or `verse.builder_slugs`, and no one else can
fork it. Only its owner, when `access.gate` admits them, and site admins change
it (`verse.realm_visibility_may`); a publish's `listed` is the same change with
the same right. Checks: `coax/web/api/tests/cli_auth.coax` (`visibility`:
private 404 to another key and to nobody, its document and fork too, off /og;
unlisted by slug; public on /og; another key 403; invite only refuses an owner
not approved and admits one approved; a site admin sees and changes it).

Its room is as private: presence and chat in `made-<slug>` answer 404 to
anyone `verse.realm_hidden` hides the realm from (`verse.room_hidden`,
`migrations/0108_realm_seat_presence.sql`; `room.c` passes the site admin
fact). Checks: `coax/web/api/test_room.py` (private room 404 to a visitor,
the homepage's GET, chat and another account's seat; its owner and an admin's
seat are in it).

## A made realm's document changes only from the revision it was read at

A made realm is a versioned document (`verse.realm_doc`,
`coax/web/api/migrations/0093_realm_documents.sql`) with a revision number
and a SHA-256 hash. Any change to what it is makes the next revision.
`POST .../<slug>/save` and `.../<slug>/publish` name the revision they were
made from, and the server refuses a stale one with 409 and the revision there
now: the newer write never simply wins, and the page keeps its changes
unsaved. Only the owner saves or publishes. A fork of a made realm copies the
document at one revision and records `forked_from {slug, rev}`; nothing done
to the fork changes the original's document or hash. Visitors read only the
revision published (`0094_realm_live_and_undo.sql`): the spec, `/doc` and a
fork by anyone but the owner are of that revision, and nothing but PUBLISH
changes it. Every revision the owner's edits make is kept, and `POST
.../<slug>/undo` restores an earlier one as a new revision, never rewriting
history. Each undo step is the edit's per-slot inverses, the slots
`metaverse-engine/editor/edit.coax` names (`0111_realm_edit_log.sql`): undo puts each
slot it changed back, newest first, and is refused (409, nothing changed) when
a slot is no longer what the step left; `.../<slug>/redo` applies it again
under the same rule, and a new edit drops what was undone. A document's moves
and shapes stand in entity order (by feature, then index:
`0166_realm_entity_order.sql`, and `edit.coax` keeps its overrides the same
way), so an undo or redo gives back the restored revision's bytes with only
`rev` changed; `proto/verse/realm-document-v1.json` pins it (`restores`),
checked by `metaverse-engine/editor/doc-goldens.coax` and
`coax/web/api/tests/realm_doc_checks.coax`. A made realm's lore
and quests are in its document (present only when it has some, so a document
without them keeps its bytes and hash), so publish, undo and fork carry them;
lore moved there from 0055's tables became one new revision and every older
revision kept its bytes and hash. A realm made or forked
by a tool (`verse-automation` cookie, `X-Verse-Automation` header) is
unlisted. Checks: `coax/web/api/test_realm_fork.py` (stale save and publish
refused, the original byte-identical after the fork is saved, tool-made realms
unlisted, visitors on the revision published, each edit undone as one step); `metaverse-engine/editor/edit.coax` (an edit is refused when its entity moved
since, or a slot changed since; apply then inverse is the identity, for every revision path; a replayed log gives the same document); `coax/web/api/tests/realm_shapes.sql` (turns, sizes and removals, 0109: a save without them keeps them; visitors, forks and undo carry them); `coax/web/api/tests/realm_edit_log.sql` (every revision path is one step of ops; undo and redo walk them to each earlier state; a stale step refused; older steps undo whole; lore and quests published, undone, forked and moved in without changing older hashes); `coax/web/api/tests/realm_cli_api.coax` (the built CLI against a local API: rotate, resize, remove, lore, quest, a stale refusal, undo per edit, a pinned publish, a fork); `apps/openagents-web/myrealm.coax`
(a 409 keeps the changes); `coax/web/tool.sh editor-journey` end to end.

CLI edits use `commands.coax` for bounded arguments and carry a revision to
the server. Migration `0102_realm_edit_fences.sql` locks the made realm while
comparing and applying switch/place/settings/lore/quest/ask/proposal changes;
asynchronous planner results compare their server-held revision again before
they apply. The `edit-v1` route family is recognized only by the forwarding
host: an old instance returns 404 without effects, even if another instance
advertised `edit_fence 1`. Missing or malformed revisions are refused. Existing
bodies without a revision retain their legacy behavior. Side-table revisions
order writers without changing the v1 document's undo coverage. CLI move saves
preserve the complete admitted move list, and consecutive batch moves share
one save. The first refusal stops a non-atomic batch and reports its committed
prefix. Checks: `coax/web/api/tests/realm_edit_*.coax`, the catalogue's mutation
gate and `coax/web/tool.sh cli-journey` against the built CLI.

## A published made realm plays the engine it was published with

Publishing a made realm pins its engine with its revision
(`coax/web/api/migrations/0107_realm_engine_pins.sql`, #12629). The engine
is `/realms/built`'s app script; the manifest keeps each attribute as one
line in the page's order. Every path in it must be named by its content and
is never deleted from the bucket. The rule is SQL's (`verse.engine_manifest`)
and Coax's (`metaverse-engine/editor/publish.coax`, `#authority`, mutation-checked).
They must give the same manifest and hash for the shared fixture. With a
site bucket, a publish whose engine page can't be read or named by content
is refused (503) in the same transaction, and nothing is published. The
realm's page builds the engine from its pin (`data-engine`), never from
today's template, unless the URL asks `?engine=current`. A fork starts on
its parent's pin; nothing done to it changes its parent's record. A realm
with a published revision but no pin is pinned to the engine served when
its page is next written. Every pin is a row of `verse.realm_publications`,
never rewritten. Checks: `metaverse-engine/editor/publish.coax` and
`coax/web/api/tests/realm_engine_pin.coax` (SQL, rolled back). After the
strip, `coax/web/engine-served.coax` adds each served file's own SHA-256 to
the engine's script (`data-served`), so a pin carries them; `engine-served
check` holds a record to a site's bytes. `realm_walkthrough.coax` checks the
whole route: edit, publish, reload, a later release, fork, refusals.

## A room host runs each realm by its declared profile

The room server serves Orbit League and Everglade on one 60 Hz beat, each by
a profile from `metaverse-engine/net/profile.coax` (`#authority`, mutation-checked):
its rate (a divisor of the beat), physics steps per world step, trust,
durability and fairness. Only an authoritative profile may be durable or earn
steps by the clock; self-asserted presence and small-game lockstep are
separate profiles and never qualify as authoritative play. The host steps a
realm only on the beats `net_due` names (`room-host.c`, `room.coax`
`rm_realm_due`). Checks: `metaverse-engine/net/profile.coax`, `room.coax` "profiles:".

## Authoring never writes durable state

A realm has three kinds of state. **Authored** state is what a maker wrote:
the realm document and its edit log (`metaverse-engine/editor/edit.coax`),
content tables and their revisions (`metaverse-engine/content/tables.coax`).
**Running** state is what a session holds while it plays: the World
(`metaverse-engine/core/world.coax`) and a page's state, which a code reload
keeps field by field (`coax/web/state-keep.js`). **Durable** state outlives
every session and belongs to a player: the character record
(`verse.characters`, #13290) and the item ledger (`verse.items`,
`verse.inventories`, `verse.item_ops`, #13291, #13318).

An authoring operation (an edit, undo, redo, a content rollback, a world's
save or load, a publish) writes authored state and may replace running state
with what it authored. It never writes the character record or the ledger:
rolling a realm back gives no item back and takes no XP away. Durable state
changes only through its own routes (the two entries above). The rule is a
table, `state_op_writes` in `metaverse-engine/editor/authority.coax`
(`#authority`, zero surviving mutants).

The proof for the code is `coax/web/tool.sh layer-check`'s `durable` gate,
which runs before every push and has no allowlist:

- The contexts that hold authored and running state (`realm-authoring`,
  `engine`, `splice`, `realm-tools`, `realm-seat` in `contexts.txt`) reach
  neither `characters` nor `items` through "may load" lists, directly or
  through another context. Product code that can't load the record's or the
  ledger's module can't call it.
- No line of `metaverse-engine/layer-allowlist.txt` excuses a load from one
  of them into either.
- No non-Coax source they own (`coax/web/api/realms.c`, `worldbuilder.c`, the
  hosts) names a durable table, the rules' generated headers or `room_items`.

A page can't write durable state at all: only a room-key-signed post reaches
the ledger. So the world's save and load routes (#13412) land under this
gate.

Each realm declares who decides each kind (`realm_authority`, same file;
[docs/architecture/state-and-authority.md](docs/architecture/state-and-authority.md)):

| Realm | Content | Simulation | Durability |
| --- | --- | --- | --- |
| Splice Bots | The owner and admins | The page | Nothing durable |
| Everglade | Admins | The room | The ledger |

Every other realm (a made realm, or a hand-built one with a board) is a
world of kind `realm` (migration 0185, #13422) and has one rule,
`realm_authority_of`: its owner, its own admins and the site's admins change
its content, its page simulates it, and it holds nothing durable. A realm with
a row is served only as its own kind. Anything else has declared nothing, and
`realm_kind_op_may_write` refuses every write for it. A new realm with a
durable store adds its row, and its statement in its README, in the same
change (#13431).

## Metaverse Engine realms step on one fixed clock whatever the frame rate

A realm's simulation advances only in whole fixed steps from `metaverse-engine/core/loop.coax`'s
`FrameClock`, which keeps time in integer nanoseconds. A frame's time is refused
when it is not a time, clamped to `max_frame_ns`, and at most `max_steps` run;
the backlog past that is dropped and counted, never carried into later frames.
A step, clamp or cap that is zero or negative, as in a clock read from zeroed
memory, takes its declared default, so a clock never freezes.
Drawing mixes the last two steps by `loop_alpha` and never steps physics. So
the same per-step inputs give the same states, bit for bit, at any frame rate.
Checks: `metaverse-engine/core/loop.coax` (step count follows the time fed at 30, 60, 120 and
144 FPS; mutation 0 survivors) and `metaverse-engine/move/walk.coax` ("a run and jump has the
same states at 30, 60, 120 and 144 FPS", "a jump drawn at 60 FPS has no stutter").
A realm's flight is the walker's per-step hook (`metaverse-engine/move/flight.coax`), never
a face's per-frame change to the player's speed, and a realm camera that
eases (Hecate's Cemetery's free orbit, Ice Cream Man's follow) eases once per
fixed step; Hecate's is drawn between the last two. Checks: `metaverse-engine/move/walk.coax` ("a flight of beats,
climbs, glides and a dive has the same states at 30, 60, 120 and 144 FPS and
on uneven frames"), `apps/openagents-web/villa.coax` ("Hecate's camera eases
on the fixed step ..."), `metaverse/icecream/play.coax` ("the camera steps with the
rules: the same input at 30, 60, 120 and 144 FPS and at uneven rates gives
the same camera every step").

## A realm's frame time is what its clock counts, and no frame runs past 8 steps

No realm keeps its own frame clamp or step accumulator. Every fixed step runs
on an `metaverse-engine/core/loop.coax` `FrameClock`, and everything a realm moves outside
its steps (animation, its room's timing) moves by `loop_counted` or
`loop_counts`: the frame's time after that clock's refusal, clamp and drop.
The page faces of the haunted house, Fairy Forest, the Garden, Everglade and
the villa move their realm by `metaverse_player_counts` and hand the player the
frame itself. No clock in a realm runs more steps in one frame than the
realm's declared budget (`steps = 8`, `metaverse-engine/core/budget.coax`); time past the
cap is dropped and counted on the clock, never run later. `metaverse-engine/core/loop.coax`
declares no `#export_c` procedure, because realms' libraries load it more
than once (Everglade's player, debris and river).
Checks: `metaverse-engine/core/loop.coax` ("a frame counts for its time, refused, clamped
and dropped as the steps are"), `metaverse-engine/move/walk.coax` ("a face's frame time is
what the player's clock counts, and runs at most 8 steps"),
`metaverse/deadgears/ecs.coax`, `metaverse/demolish.coax` ("the steps are the Metaverse Engine's
clock's ..."), Hold the Stair ("frames run at most its budget's steps,
fast-forward included"), and Orbit League's and Dead Gears' budget checks,
which take each frame's steps.

## A figure's world anchor includes its interpolation and its floor origin once

The current avatar renderers consume the interpolated feet for a frame, not
the last fixed step alone. Realms whose controller steps on a flat floor use
`feet_in_world` (`metaverse-engine/core/points.coax`) to apply the terrain or bridge origin
once; that world point is shared by the figure and camera. Everglade's wading
origin is relative, while its swimming height is already absolute.

The Grid converts feet, eye and aim through `metaverse-engine/core/space.coax`'s
`world_to_grid` on all three axes. Remote visitors retain and ease their
admitted height with their horizontal position; drawing never replaces it
with the floor. Checks: `metaverse-engine/tests/check_drawn_feet.py` observes the nine
current avatar renderer paths' native figure, object or instance records
against independent coordinates and four broken producer controls each;
`newland-walkers.coax` checks height updates and identity-preserving reorder.
This native f32 boundary evidence does not qualify browser rasterization,
replace every legacy vector ABI or establish f64 world coordinates.

## Each input press is taken by exactly one fixed step

Realm input reaches the simulation through `metaverse-engine/core/input.coax`. Keys and the
touch stick map to one `Intent` with one run rule and one dead zone. A press
(jump, act) is counted when it happens and taken by the first fixed step that
samples after it: a frame that runs no step keeps it, and a frame that runs
several gives it to one. At most `INPUT_MAX_WAITING` presses wait; more are
counted as lost, never kept. Values that aren't numbers, and axes past full
travel, are refused or clamped and counted. Checks: `metaverse-engine/core/input.coax`
("every press is taken by exactly one step, the one the model names";
mutation 0 survivors) and `metaverse-engine/move/walk.coax` ("a press in a frame with no
step jumps on the next one, and a frame of two jumps once", "the keyboard
and the touch stick drive the same session to the same states").

## Entities change only at the step's flush

An entity in `metaverse-engine/core/entity.coax` has a handle (slot and generation) for use
inside a step and an `EntityId` from the store's counter for saves, the
network and edits; nothing durable stores a handle. `entity_spawn` and
`entity_despawn` are commands: `entity_flush` applies the despawns in slot
order, then the spawns in the order they were asked for, so every system in
a step sees the same entities and a spawned entity takes part from the next
step; despawning a spawn that is still waiting cancels it. A despawn raises
the slot's generation, so a stale handle never reaches the slot's next
owner; a slot whose generation would wrap is retired, not reused. Checks:
`metaverse-engine/core/entity.coax` ("a store matches a plain list of IDs and values after
every flush", "despawning while walking: every live entity is visited once,
and none after its flush", "IDs survive save, load and replay", "despawning
a waiting spawn cancels it"; mutation 0 survivors).

## External events enter a step only from the session's record

Every external event a session on `metaverse-engine/core/replay.coax` accepts (a frame's
controls, a model's answer, a network update) is one `ReplayEvent` with its
apply tick, source, request number, epoch and payload. Only the gate,
`replay_offer`, accepts one: it refuses a duplicate (a model request already
answered, or a stream's last accepted number again), a stale event (another
epoch, a model request no longer open, or a stream's earlier number), an
unknown source or an answer to a request never asked, and every live event
during a replay; it stamps an accepted event with the next step,
never the sender's tick. A step applies only what `replay_take` hands it, in
live play and in replay alike, so a replay consumes the recorded decisions
and makes no remote call (`replay_ask` says when to send). A record starts
with the constructed state's digest; `replay_play` refuses a replay from any
other state. Checks: `metaverse-engine/core/replay.coax` ("delayed, duplicate and stale
events: a live session and its replay agree at every step", "a planted fault
is found at the step it starts"; mutation 0 survivors).

## Every walking realm asks one ground question

The walking controller's floor comes from `metaverse-engine/physics/query.coax`:
`floor_under` in `metaverse-engine/move/walk.coax` is one `query_stand` call over the
realm's colliders (flat or heightfield ground, walls as boxes, bodies as
capsules, and floors). The actual fixed step consumes `query_ground_checked`;
the trusted `floor_under` adapter retains the permissive query. The step height
(`QUERY_STEP_UP`, 0.55 m), the steepest slope you
can stand on (`QUERY_STEEP`, 55 degrees) and the floor pad
(`QUERY_FLOOR_PAD`, 0.15 m) are defined there once; the walker reads them
from it. Every shape is solid, so a ray or sweep that starts inside one hits
it at t = 0, and ties go to the earlier collider (the ground, then boxes,
then capsules, by index). Checks: `metaverse-engine/physics/query.coax` (rays, sweeps and
overlaps against the containment oracle in `metaverse-engine/tests/query-oracle.coax`;
root-only mutation 388/388 killed, with separate ground/ray/contact receipts)
and `metaverse-engine/move/walk.coax` ("jumping onto, landing on
and walking off a ledge is the same in every realm").

## Terrain and capsule publication carry checked receipts

The semantic query boundary (`query-filter.coax`, `query-scene.coax`,
`query-fluid.coax`) admits complete collider/surface/response catalogs before
selection. Category masks are separate from collision-pair policy and visual
materials. Selected shapes keep source tie order and durable collider/entity
generations. An old epoch or despawned selected entity refuses; overlap and
volume output publish whole or leave caller storage unchanged. Filtering all
terrain creates no fallback plane. Snapshot arrays stay retained immutable;
scratch belongs to one selection until its last use. Response policy zero is
the default; every nonzero reference must resolve before publication. The
engine-owned native/Wasm journey and explicit legacy-adapter inventory are in
`docs/verification/aethyr-query-12989/README.md`; whole-realm migration and
chapter 70's full wave/force/render implementation are separate scopes.

`metaverse-engine/physics/ground.coax` samples the actual rounded f32 mesh vertices, split on
the south-west/north-east diagonal. Exact stored vertices retain their bits.
Its metre-quantity boundary refuses invalid dimensions, spacing, coordinates,
sampled heights and gradients. Beyond the mesh, consumers zero the gradient
on each clamped axis before forming the constant continuation's normal.
`metaverse-engine/physics/ground-point.coax` holds the plain height, normal and ray readers on the same Ground C layout and samples through it.

`query.coax` admits at most 16 heightfields. The exact ray and sphere-contact
helpers have explicit cell budgets; exhaustion is a refusal, never a miss.
Terrain selection retains f64 hit order and the selected helper point before
narrowing the public distance. Checked overlap validates every field before
writing IDs. Legacy permissive ground queries remain explicit trusted APIs.

`collide.coax` bounds profile passes to 1–16, each shape table to 4,096 entries
and admitted horizontal travel to at most half the capsule radius. The real
Player step publishes only an accepted collision and ground result. Refusal
preserves its incoming physical/control state and queued jump; its scheduler
still consumes the assigned time. Diagnostics report the refusal and a
saturating count. Realm identity admission comes from
`metaverse-engine/physics/collision-profiles.coax`; unknown identities preserve the Player before
advancing its clock. Trusted direct large-move helpers retain their separate
contract. These endpoint contacts do not imply general CCD or two-way rigid
impulses. Evidence and independent native scopes are recorded in
`docs/verification/aethyr-book-implementation-2026-11-01.json`.

The Grid's execution/digest profile is now 2. It includes all seven collision
and query diagnostics, both refusal-counter halves and residual float bits.
Replay rejects any other full profile value before writing its destination;
the `ReplayEvent` envelope is unchanged. The historical profile-1 initial pin
is 3864885271; profile 2's independently calculated pin is 1068547055. The
current recorded session agrees at every step natively and in Wasm. Evidence:
`metaverse-engine/tests/grid-profile2-qualification.json`.

## Walking profiles and route followers share the realm's paid steps

`metaverse-engine/move/walk.coax` owns the walking controller; `metaverse-engine/move/player.coax` retains
its published C signatures as thin adapters. Input's legacy `input_runs` C
export remains; production pages reach one copy. The coupled Dead Gears and
Player boundary probe explicitly prefixes one duplicate helper copy in a
test-only include wrapper. `World.locomotion` declares
finite bounded tuning separately from capsule geometry. Invalid tuning
preserves the physical state and queued jump while a scheduled tick is paid.
Walking admits only the 1/64 s period and at most eight steps. A different
positive period, an excessive cap, an unpaid due count or a carry outside
the paid step interval refuses before accepting new frame time. Nonpositive clock settings retain Loop's defaults.

`advance_traced` returns temporary chronological post-step targets, including
unchanged poses on refused physical steps. It adds no persistent Player field
or Grid digest word. `metaverse-engine/move/follower.coax` consumes the paid clock and those
targets once, checking cursor continuity before integer arithmetic. It owns
no timer. The Garden and villa's actual faces use this path; restoring a
Player resets the realm's cursor. These floor-relative adapters apply the
terrain origin once to each target. The grounded route mode projects onto the
canonical mesh and uses strict turn/hand/throw arithmetic; it does not claim
the walking capsule's gravity, jump or contact behavior. With complete scenery
blocks, actual travel is bounded by half the route body's radius.

A refused fetch step restores the pre-command Fetch and leaves its action
queued. Drawing reads previous/current dog and ball poses at the same clock's
alpha without stepping either; counted frame time still drives room and
visual animation. The legacy variable-time APIs remain explicit compatibility
surfaces. Chapter 64 owns the remaining special realm adapters, including
flight and boat/warp paths. Evidence: `metaverse-engine/tests/walk-qualification.json`,
`metaverse-engine/tests/follower-qualification.json` and the chapter 9 verification data.

## One follow camera eases the walker's view

Valley Lab explicitly opts into `metaverse-engine/move/locomotion.coax`'s `source_velocity` flat-ground
CalcVelocity/braking profile, decoded from the source Echo defaults. Other
walking realms keep their existing profile. Source tuning never bypasses
paid 1/64 s steps, capsule collision or queued input. Source numbers qualify
this scalar port, not the complete Unreal camera, ability or AnimBP runtime.

The walking controller's camera (`metaverse-engine/move/walk.coax`, every walking realm and
Dead Gears' `camera_room`) moves only through `metaverse-engine/cam/follow.coax`: the look
and zoom (`follow_look`, `follow_zoom`), one step of the spring and the room
(`follow_track`), and the eye drawn between the last two steps (`follow_at`,
`follow_eye`). No other code eases that camera. `follow_room` reads a realm's
`Colliders` and never changes them, the camera pulls in at once and backs out
by a closed form in the step's time, and its direction uses strict math.
Checks: `metaverse-engine/cam/follow.coax` (mutation 0 survivors) and `metaverse/gridwalk.coax`
("the Grid's camera follows a walk, a run and a jump at 60 FPS without a
jump").

## The camera only reads the player, and every camera makes one shot

The player's step (`step` in `metaverse-engine/move/walk.coax`) never moves the camera.
The camera steps after it (`camera_step`, run by `step_once` in
`steps_in`) and changes only the camera's fields (`target`, `spring_vel`,
`dist`), so the player is the same, bit for bit, whether the camera steps
every step, never, or twice a step. Every camera answers with one
`CameraShot` (`metaverse-engine/cam/shot.coax`) built from state already drawn between
steps: third and first person (`follow_shot`, `follow_first_shot`), top down
(`shot_top`), the free orbit (`shot_orbit`, which `metaverse-engine/cam/camera.coax` draws
through) and the dive (`shot_dive`, where a time that isn't a number is a
cut). No camera steps physics. Checks: `metaverse-engine/move/walk.coax` ("the player is
the same whether its camera steps every step, never, or twice a step", "each
view mode is its own shot of the frame drawn"), `metaverse-engine/cam/shot.coax` and
`metaverse-engine/cam/follow.coax` (mutation 0 survivors).

The checked `cam/` query companion reads separate semantic body/sight views.
Attention rechecks the retained selection's epoch and entities; a query refusal
clears all targets rather than consuming hidden-target grace or publishing a
partial rank. The eye sweep tests the actual displaced segment with an extra
1 mm radius, including sideways shake beside corners. A start inside that skin
or a refused query returns no qualified view. A caller must not draw its fallback
as safe without another successful check. The adapters change no actor or query
geometry and leave the listener on the accepted unshaken shot. This boundary's
numerical fixture and remaining playable adoption are recorded in
`docs/verification/camera-13026-query/README.md`; it is not a claim that every
realm already uses these adapters.

An engine module that two libraries a page links both load
(`metaverse-engine/core/space.coax` and `metaverse-engine/cam/shot.coax`, which a realm's engine and the
player both reach) declares no `#export_c` procedure: each library would
define the symbol, and the page fails to link. Check: the site build
(`apps/openagents-web/build.sh`) links every realm.

## A realm's frame stays within its declared budget

Every realm's engine file in `apps/openagents-web/` declares a
`FrameBudget` (`metaverse-engine/core/budget.coax`): the steps, vertices, draw records and
host bytes one frame may use. Its vertex limit is below the realm's vertex
buffer, so a frame the quad writers clip at a full buffer is over budget. A
`BudgetMeter` takes a request whole or refuses it; a frame that passes any
limit fails the realm's check. A realm that outgrows its budget raises the
declaration, or its buffer, in the same change; it never clips silently.
Checks: each realm's "... frames fit its declared budget" (three frames at
each of three screens), and `metaverse-engine/core/budget.coax`'s planted overflow
(mutation 0 survivors).

## An asset is its content hash, and a pack is admitted whole or not at all

A Metaverse Engine asset's identity is the SHA-256 of its bytes (`metaverse-engine/core/assets.coax`,
`#authority`), never its name or path. `asset_admit` hashes every entry of a
pack before taking anything: an entry that doesn't hash to its table's id,
new bytes past the registry's declared limit, or more new assets than free
slots refuses the whole pack and changes nothing held. Bytes already held are
shared and counted once; the last `asset_release` frees them. Checks:
`metaverse-engine/core/assets.coax` (a planted edited texture, the limit to the byte, a
seeded model of uses; mutation 0 survivors with `--root-only`).

A realm opting into browser pack digest gate v1 admits the complete received
VPK1 digest and byte count in Coax before unpack/upload. The browser computes
SHA-256 as a platform effect and bounds the 32-byte destination; the realm's
authority module pins the expected identity. Failed admission exposes no
part of that pack to the GPU. Checks: `metaverse/valley/canyon-pack-policy.coax`
(mutation zero survivors) and `coax/web/valley-canyon-check.coax`'s same-length
vertex edit. The versioned optional ABI is `coax/web/PACK-GATE.md`.

A range-addressable VPK3 pack (`metaverse-engine/editor/page-pack.coax`, `#authority`,
#13001) is admitted a page at a time, and only by content. Its manifest's
SHA-256 is in its header. Every record must cover the image in order and the
file without gaps, and a deflated page must be smaller than its bytes. A page
is held only when its decoded bytes are its record's size and SHA-256
(`ppk_admit`, run in the browser's pack reactor by `metaverse-engine/host/page-stream.coax`
over the bytes already in its arena). A page refused is fetched again, never
used. The image is published to the complete-pack reader only once every root
page is held. An entry that is not a root is used only when `ppk_ready` says
every page under it is held. The assembled image is the VPK1 pack byte for
byte, so a realm's pack pin still names it. Checks:
`metaverse-engine/editor/page-pack.coax` (edited, short, repeated and out-of-range pages;
every fixed header and manifest byte) and `metaverse-engine/host/page-stream.coax`.

## A lost GPU device or an engine trap keeps the realm's session

The browser host (`coax/web/host.js`) answers `GPUDevice.lost` for its current
device by taking a new device, building the pipelines and uploading the
world's assets again, then putting the state region (`coaxial_state_size`)
back as the loss found it; frames and input wait meanwhile, and the first
frame after draws the current step. What reaches the realm from outside
meanwhile (a socket's event, a fetch's answer, a stored value) waits too,
in order, and is delivered after that first frame: written during the
rebuild it would be lost when the state is put back. A device lost while it is rebuilt is
rebuilt again. A device not replaced within 10 s, or a fourth loss in a
minute, is handled as a stop. A trap in the engine (a frame or an input
call), or such a stop, saves the region as it was at the start of the last
frame to IndexedDB `coaxial-rescue` under the save slot and shows the reload
message with one action. The next load of the same engine (its wasm's
SHA-256, layout and state size all match) puts the save back once its world
is in, so the save gives the same state digest; a save is put back once and
from no other build, and a realm that traps again within 300 frames of one
starts over. State a face keeps in its own statics is outside the region and
is not saved. Check: `coax/web/gpu-recovery.coax` against a realm page
(the Grid: recovered in about 0.35 s on SwiftShader with the same digest in
12 of 12 runs, with its waiting writes delivered; the trap's save put back
with the same digest).

Legacy recovery clears the initial pack's asset rows as well as ordinary
asset rows before binding the replacement device. A completed old-device
pack buffer/texture is never a ready resource for that replacement. Check:
`coax/web/valley-canyon-check.coax` restores the original source pair's exact
state and pixels with no cross-device buffer bind error. Aligned immutable pack
entries upload directly from their retained byte views; only unaligned or empty
entries allocate zero-padded staging. Legacy layer upload overlaps at most two
atlas decode/upload workers, closes each source and temporary mip, and joins
both workers before destroying a refused texture. The 10-second recovery fence
is unchanged. See `docs/verification/slay-gpu-recovery-12975-2026-11-07/`.


The bounded streamed WebGPU carrier additionally uses
`metaverse-engine/render/loss-policy.coax` for the same absolute ten-second deadline
and fourth-loss stop. Its checked face refuses paid frames during
replacement and permits stopped cleanup only from the actual closing
state. The live RAF driver resets its timestamp on the recovery serial,
so rebuilding wall time is not charged to simulation. Its separate
`stream-walk-rescue.coax` carrier checkpoints the complete Wasm32 reactor,
including C statics, allocated source bytes and pending immutable facts.
Coax admits only the exact engine/scene/manifest, layout, tier, memory extent
and snapshot checksum. IndexedDB takes a save once in one read/delete
transaction. Restore preserves allocator addresses and CPU owners, rebinds
GPU handles and resumes unfinished effects. A second stop before 300 frame
starts suppresses another save. Actual two-MiB desktop trap, pending-load
and failed-adapter reloads have their own scoped evidence; native rescue,
mobile hardware and larger-memory rescue are unqualified. See
`docs/verification/aethyr-streaming-12990/walk-persistent-rescue.md`.

The bounded native stream draw carrier uses an additive owned Vulkan
submission (`metaverse/gpu/vks-submit.{h,c}`). Pending or unknown fence errors
retain the command pool, fence and drawn-resource borrows. A timeout stops
paid frames; cleanup waits for the actual completion observation before
retiring those owners. Reported device loss uses the same checked recovery
rate/deadline policy and preserves CPU/source state. The qualified input
profile explicitly injects results after actual Vulkan work; it is not a
physical GPU-reset claim. Streamed buffer/image factories now publish only
after successful creation and unwind private partial effects. Their
[factory record](docs/verification/aethyr-streaming-12990/walk-native-factories.md)
qualifies SDK error prefixes and successful consumer adoption. Streamed
factories also offer a separate pre-allocation admission callback. It sees
the real requirement size/alignment/type mask and selected memory type before
`vkAllocateMemory`; only success permits that effect. A caller's exact
backing-budget lease survives any subsequent SDK failure until private cleanup
has returned, or successful final use and physical destruction. The component
[backing-gate record](docs/verification/aethyr-streaming-12990/walk-native-backing-gate.md)
qualifies this boundary. The bounded native corridor now binds all explicit
buffer/image backing to its authenticated manifest and measured device plan
before resource loading. Its staging, destination, renderer, replacement and
readback allocations require exact planned requirements and live inbox leases.
Actual allocation/free observations stay separate from reservations; release
waits for physical destruction or failed-factory unwind. The v7 observation
envelope carries the original observation contract and a mandatory backing
history, checked through Coax ownership replay and an independent oracle.
See the [integrated backing record](docs/verification/aethyr-streaming-12990/walk-native-backing.md).
Streamed
buffer/image uploads now use owned `VksTransfer` submissions; finish, destroy,
paid frames and replacement refuse while a transfer is owned. The
[upload record](docs/verification/aethyr-streaming-12990/walk-native-transfers.md)
qualifies actual held observations and exact replay. The integrated
[SDK failure record](docs/verification/aethyr-streaming-12990/walk-native-sdk-failures.md)
qualifies scoped factory/upload errors and deadline stops over real effects,
with retained source/owners through terminal observation and exact CPU replay.
Physical OOM/reset, surface/pipeline/readback adapters and native persistent
rescue remain unqualified. See
`docs/verification/aethyr-streaming-12990/walk-native-loss.md`.

An unwanted unfinished load in the bounded browser stream carrier retains
its generation, physical source/codec storage and tickets until its actual
platform effect joins and retires. The owner cannot allocate a newer
generation into that storage meanwhile. A captured old completion redelivered
after a newer generation becomes ready cannot finish, decode, allocate or
destroy that current owner; wrong epochs have the same fence. Actual held
response/callback histories preserve the current source, counters, canonical
state and full image, then replay every accepted paid step through CPU codecs.
See `docs/verification/aethyr-streaming-12990/walk-callback-retirement.md`.

## The Grid steps only on its session's record

The web Grid's simulation (`metaverse/gridwalk.coax`) is one `GridWalk`: you,
the phone's thumbs, the input buffer and the session's record
(`metaverse-engine/core/replay.coax`). The page's C face hands it the devices' input and
the frame's time once a frame and draws what it reads back; the face does
not clamp time, step the player or keep the player. Each frame's controls
are one `REPLAY_ACTIONS` event, and a fixed step applies only the events
`replay_take` hands it, through the input buffer, so look and zoom change
the state only on steps. When a record fills, a new one starts from the
state the session has then, with the frames still waiting for a step in its
start's input buffer, so no press or turn is lost. A blur releases what is
held outside any frame, so it also starts a new record from the state it
leaves. Checks:
`metaverse/gridwalk.coax` ("a recorded session replays to the same digest at
every step", "a record full of frames starts a new one", "a blur starts a
new record from the state it leaves, and that one replays", "the keyboard and
the touch stick drive the same session to the same digests"; mutation 0
survivors) and `coax/web/replay-hosts.coax` (a recorded Grid session
gives the same digest at every step natively and in wasm).

## A saved servo run restores only a verified bounded recipe

The SLR1 companion binds the source model, assembly, topology and exact
numerical profile, complete initial/final queue snapshots, ordered targets,
end reason, trace and artifact identity. Its sole codec recomputes at most
4,096 additional steps and admits the whole result before publication.
The Microduck consumer additionally matches the source joint's exact limit
and stand words. Refusal preserves the admitted run and presentation.
Scrubbing leaves the source record unchanged; explicit branching retains
the cursor's complete queue and links the parent artifact. Camera, selection
and explosion are separate presentation state. RXE1 and SLV1 retain their
published identities. Actual native/Wasm consumers reproduce the same
nonzero-state artifact byte for byte. See
`docs/protocol/servo-run-v1.md` and
`docs/verification/microduck-replay-13641/README.md` for the precise host,
offline, delivery and illustrative-simulation scope.

## The reduced iOS home has one product implementation

Coax owns its scene, animation, layout, reusable HUD touch policy and bounded
protobuf admission. Native adapters own platform lifecycle, serialized opaque
storage handles, packet transfer, font rasterization and Metal bindings. A
screen does not acquire chat or device permissions through this port, except
the microphone, which only the voice toggle asks for (see the voice section
below). The only network uses are the Spark wallet below and the voice host.
The home imports the `ios-home` descriptor profile for generated
HomeConfiguration and HudInvocation codecs. It accepts only its documented
schema-1 resource subset, rejects unknown/duplicate fields and non-minimal
varints, and emits existing HudInvocation bytes. Native adapters only transfer
bytes; the resource and invocation have no second hand-written wire codec.
The home is the Grid seen from high above, slowly orbiting, with only the Start
bar over it (`metaverse_home_hud_frame`). Start dollies the camera down to land
behind you; `metaverse-engine/cam/camera.coax` owns the overview orbit and the dolly path
(eased, from behind and above, never below where it lands, a cut under
reduced motion; a tap hurries it). Controls, the hotbar and the balance appear
only once it lands. The balance sits top right as whole sats with no grouping
(`₿1234`, or `₿–` with the reason spoken while there is none) and opens the wallet.
`metaverse/grid.coax` owns its projection and line art,
`metaverse-engine/core/sticks.coax` the move/look/jump touch policy; `metaverse-engine/move/walk.coax` owns walking and camera state behind the `metaverse-engine/move/player.coax` C facade; `metaverse/cast.coax` admits the bundled `Cast.bin`, blends your
character's clips and poses you and the robots. The native adapter only routes
the Start invocation there, passes numbers between the libraries each frame
and binds the buffers Coax admitted. A cast whose offsets, indices, joints or
floats fail admission is refused, not drawn. You collide with the robots
through `player.coax`: a capsule per robot bone, placed by `cast.coax` from
the same palette the robots are drawn with, pushes you out sideways and keeps
the camera in front of it. Robot line art takes its width and light by
distance from `light.coax`. A terminal (aethyr's desk and screen, from
`grid.coax`) stands 4 m ahead of the spawn; its boxes are both drawn and
solid, so `player.coax` stops you at the desk, lets you onto it only by
jumping, and keeps the camera out of it. The hotbar is aethyr's HUD slot:
`grid.coax` places it and claims touches on it before the sticks, and
`light.coax` draws it. Its one slot toggles the dance (`cast.coax` plays
Dance_Loop while you stand); walking or jumping ends it. The Grid has no
buildings, peers or network use yet.

Check: `coax/build.sh test`, `coax test metaverse/grid.coax`, `metaverse-engine/cam/camera.coax`,
`metaverse-engine/move/player.coax` and `metaverse/cast.coax`, `metaverse/test_cast.sh` (the
real Cast.bin under ASan/UBSan, with corrupted copies refused, and you
walking into the Foreman's leg and stopping),
`PROTOC=... python3 metaverse/test_home.py`, the sanitizer harness and the iOS
UI tests (Start opens the Grid, you stand idle, the stick walks you forward,
a jump plays its clips, the Foreman stands by the start and its leg stops
you, the hotbar's slot makes you dance and a step ends it, the terminal's
desk stops you, rotation and
backgrounding keep it). Metal layout/stage
bindings remain an explicit native boundary; the home retains its initial
flat-storage graph while decoded configuration views use bounded Coax scratch
buffers. Labels are copied into resident storage before those views expire.
There is no claim of a full engine migration or general protobuf compiler.


## Haunted House is a native Metaverse realm

The Grid's lit arch at (-1.5, 0, 10) enters Haunted House by tapping its name
or walking through it. Each world owns the window's full Metal surface; it
is not a modal web view. Grid returns to the same app, with the home, cast
and wallet intact. The scene's decisions, geometry packets, motion, trigger
schedule, footsteps/spatial sound, dialogue, teaching/character behaviours,
quest, editing and protobuf documents are in `metaverse/haunt.coax`.
`hud.coax` supplies reusable hotbar layout; `haunt-light.coax` emits the
material math as Metal. Native code binds lifecycle, fixed-capacity memory,
GPU buffers, UIKit/accessibility forms, audio clips and local file/share I/O.

The original current content (source 69848ca, not the older frozen house)
keeps all 120 objects, 21 doors, 13 rooms and three outdoor areas, the plain
roofs, pine/dead-tree forest without ground cover, Mortimer/Agatha and the
ghost/cat voices. All nine scare zones, including delayed thunder, the brew
morph and sheet ghost, run locally. This introduces no new network service,
account, microphone permission or payment path. Local copies and invitations
are actual proto files, not claims of a public-world publish. The retired
web public-world/owner/change-offer APIs and cloud NPC endpoint remain explicit
service parity gaps; see `docs/protocol/verse-haunt.md`.

`proto/openagents/metareal/haunt/v1alpha1` defines resource metadata and
editable snapshots. Geometry is separately bounded; every vertex/index and
draw range passes the Coax `#authority` buffer gate before upload/drawing.
Unknown identities in an imported save grant no authority. The matching base
revision, bounded finite overrides and unique IDs are checked before a save
changes the live scene. The platform retains bytes for every borrowed view;
undo rebuilds the base, then applies a complete override snapshot. Background
stops audio/input and saves; returning discards the world's native allocations.

Check: `metaverse/test_haunt.sh` (Coax checks, 109/109 buffer-gate mutants killed,
ASan/UBSan and real assets; with `PROTOC`/`PROTO_PYTHON`, source content parity
and official protobuf writer interoperability in both directions), Grid/player
checks, `HauntUITests` (real Metal portal, conversation, placement, save/reenter,
cameras/map/rotation/reset) and the existing `HomeUITests`.

## A shared-renderer GPU object exists only with a live ledger handle

`coax/web/render.js`, and the native Vulkan host (`metaverse/native/render-host.c`,
`render-realm.c`, `render-textures.c`), create a buffer or texture only after the Coax ledger
(`metaverse-engine/render/registry.coax`, `#authority`) admits its kind and size within
512 slots and a 1 GiB word budget, and releases the handle when it destroys
the object. A frame advances the ledger's epoch and refuses, before any GPU
write, if any object it draws with is not live. A lost device releases every
handle and keeps the generations, so no handle from before the loss is live
again. Checks: the ledger's own checks and its mutation gate;
`metaverse-engine/tests/render-ledger.coax` on the served Grid (exact count, a released
handle refused at use, recovery after a lost device); natively,
`metaverse/gpu/realm.sh local check` prints the ledger at the last frame and
none live after close.

## Retargeting declares its representable joints and preserves target rest

`metaverse-engine/anim/retarget-policy.coax` admits one rotational anchor per mannequin
slot. Canonical names disambiguate folded helpers; ambiguous aliases, unknown
slots and cyclic or forward parent edges are refused before plan publication.
The mapping authority killed 81 mutants in A2; its rig-view implementation
kills 84 in A3, with zero survivors. All source joint
errors are reported; only paired anchors have the 2° round-trip guarantee.
Unpaired target joints retain local rest. Independent input and output pose
snapshots keep parent-order evaluation deterministic. Root motion subtracts
the source's world rest position and scales by both rigs' leg chains.

Native animation exports admit all generated documents before creating a new
private output directory. Canonical parent resolution confines output below
`~/.openagents/`; existing destinations are refused. Path authority kills
32 mutants and token-aware CLI authority kills 115, all with zero survivors.
The synchronized two-rig GPU upload stays within the inspector's combined
64 MiB limit. Metrics and frame hashes are recorded in
`docs/verification/animation-lab-a2-2026-11-06/README.md`.

## Scene projects publish complete immutable banks and sample authored clocks

ALP1 validates the complete contiguous table before making payload views.
Libraries, performances, scene, cut and speech entries are admitted in an
inactive bank. Only a successful final stage publishes that bank. Failure
cannot replace bytes under the active project. Draw budgets, supported
stage/transition profiles and complete canonical sound spans gate final
admission. Source and geometry buffers remain bounded to 64 MiB each.

Scene paths, light keys, effects, pose layers, fix-ups and speech faces are
stateless functions of admitted records and scene time. Edit playback maps
its clock onto camera/scene ranges. Headless captures seek an authored frame
index and hash actual GPU bytes. Audio samples use that same edit clock and
are muxed after capture. No Unreal or Blender renderer supplies frames.

Canyon and image crossfades are refused in A5. Static focus/weather metadata,
heuristic canonical hinge limits and character-root capsules are explicit
model boundaries; a JSON diagnostic is not a claim of full collision or
anatomical validation. Licensed study payloads and captures remain private.

A5's five authority gates kill 701 mutants with zero survivors. Admission,
station checks, two complete film hash lists and rendering cost are recorded
in `docs/verification/animation-lab-a5-2026-11-06/README.md`.

## Declared facts publish with their world variant (#12991)

`metaverse-engine/state/` profile 1 admits full field identities, typed defaults and explicit
lifetimes. A paid fixed step prepares resets and changes privately. Persisted
epoch/revision/operation fences reject stale owners and duplicate effects.
Predicates select one complete six-resource variant; publication validates every
owned ready root before replacing facts and derived world together. On refusal
keep all old subsystems. Retire old references only after successful publication.

FST1 stores versioned facts and time, not pointers, caches or GPU objects. Schema
migrations require both full identities and explicit default/repair/discard masks;
never coerce owners or reference identities. Retry contains only declared local
fields and cannot roll back an external committed domain. Day changes use the
existing paid clock; visual time cannot drive authoritative routines. AESV, ACK1,
HH_Save v1 and server revision/CAS retain their existing contracts. The supported
bounds and residual adapters are in `metaverse-engine/state/README.md`.

## Experimental physical walking consumes shared Speed and per-foot costs

`locomotion-scale.coax` projects current walking Speed and additive crawl/
difficult-terrain costs onto the declared Speed-30 action locomotion baseline.
Player rate is separate from terrain pace, so rates above one reach the actual
controller; AI pursuit/home motion receives the same projection. Opt-in
`walk.coax`'s scaled entry (`advance_scaled_in`, `body_advance_scaled_in`) keeps one paid fixed tick/camera history while subdividing
collision integration through `walk-substep-policy.coax`'s bounded powers of
two. Original collision travel limits are unchanged. Refusal restores the
fixed step's physical/control state and correct interpolation history. Normal
controller exports/replay remain unchanged. AI opportunities resolve between
candidate subdivisions. This does not qualify strict continuous movement
payments or every movement/effect/world mode. Evidence and explicit tuning:
`docs/verification/srd-physical-scale-2026-11-08/README.md`.

## The painterly default is the glade profile, and its numbers exist once

Character Create's glade lighting numbers are held once, as `ENV_GLADE_*` in
`metaverse-engine/world/environment.coax`; `env_glade_profile` is built from them, and the
painterly default's GPU evaluation (`metaverse-engine/render/shade/painterly.coax`, with
`transfer.coax`, `contact-shade.coax` and `filmic.coax` `film_exposed`) reads the
same constants. The app's `cg_*` lighting names are delegates and hold no numbers.
`metaverse_style_preset(STYLE_PAINTERLY, ...)` names the glade profile as its default
environment (`environment = ENV_GLADE_ID`); lowpoly, flat, anime and photoreal
name none. Character Create's glade stays identical to the P1.1 baseline within
its declared bounds (GitHub #13060). Checks: `coax test metaverse-engine/render/shade/painterly.coax`
("the default's evaluation reads the glade profile's resolved values"),
`coax test metaverse/style-preset.coax` ("the painterly preset's default environment
resolves to the glade profile"), the delegate checks in
`everglade/scene/gpu/character-glade-light.coax` and `character-glade.coax`,
and `docs/verification/aethyr-lighting-p1-0-2026-11-08/`.

## Lit passes read the camera's resolved environment, written once a frame

Character Create, Everglade II, Experimental and 5E Lab write one `ResolvedEnvironment` a frame
(`metaverse-engine/render/shade/environment-frame.coax` `env_frame_write`: the realm's set admitted whole, the day
from `metaverse-engine/world/day.coax`, `env_resolve_camera` at the eye, `env_gpu_pack`) into words
208..315 of the frame's uniform block, and their passes read the key, direct colour, ambient,
haze, sky, cloud-shadow flag, exposure and saturation only from it (`metaverse-engine/render/shade/painterly.coax`
`paint_env_*`, through the `cg_*` delegates, which take the block). No lit path in those pages
holds its own key direction, ambient or fog numbers; Everglade II's shadow tiles look along the
record's key. A refused resolution writes the last record held, or the glade's noon, never a
partial one. Everglade II's set is `everglade/world/environment.coax` `e2_environment_set`, its
day the room's (`day_from_room`); a page with no room is the glade's fixed noon. A painterly made
realm (`myrealm`) that authors no time of day and no mist writes its style block under the glade
(`metaverse/style-preset.coax` `metaverse_style_glade_env`). Character Create stays inside the P1.1
baseline's bounds (GitHub #13066). Checks: `coax test metaverse-engine/render/shade/environment-frame.coax`
(`#authority`, zero mutants), `coax test metaverse-engine/render/shade/painterly.coax` ("the glade record draws
exactly the constants"), the `every frame writes the camera's environment` checks in
`everglade/client/client.coax` and `fivelab.coax`, `coax test metaverse/style-preset.coax`
("a made painterly realm with nothing authored is the glade's resolved light"), and
`docs/verification/aethyr-lighting-p1-6-2026-11-08/`.

Since #13067 this is the ordinary default, with no switch back to the old constants. Everglade II's
painted sky plate follows the record too: the plate's colour is scaled by the record's sky gradient
over the glade's at the same height (`paint_env_sky_tint`, through `cg_sky_tint`), so the glade's
record leaves the plate as painted and a night or dusk record darkens it with the land. The
developer's light views (`?light=ambient|direct|pregrade|postgrade|regions`,
`metaverse-engine/render/shade/environment-debug.coax`) are off without the query; they write uniform word 51 (0 is
the ordinary frame) and a readout of the camera's region weights, ambient, key and grade, and they
never change the record the passes read. Checks: `coax test metaverse-engine/render/shade/painterly.coax` ("the sky
plate's tint is 1 under the glade"), `coax test metaverse-engine/render/shade/environment-debug.coax`, the `?light=`
check in `everglade/client/client.coax`, and the Phase 1 receipt
`docs/verification/aethyr-lighting-p1-2026-11-08/`.


## A realm's code is edited from the browser only on the dev server, and only its own sources

The editor's Code pane (E12, #13421;
[docs/protocol/realm-code-v1.md](docs/protocol/realm-code-v1.md)) reads and
writes source files through one route, `POST /coaxial/code` on `coax serve`
while it watches. Owner decision, 2026-11-13: realm code is ours, or arrives
as a pull request. The rule has one exception, made by the owner the same
day: a realm script, which someone else may write for a world and which runs
only as a checked module behind the script API (see "A world names a script
by hash, and the server stores only modules that pass the check", "A
script's new version may do less, never more, unasked", "Other authors'
scripts are compiled in a job with no secrets, and only the checked module is
trusted" and "A script can be reported and switched off"). No other path lets
a visitor's code run for anyone, and there is no separate realm origin.

- **The dev server only.** `--static` (a deployed page's server), the API and
  the realm origin have no such route. The route answers only this computer's
  own page: the browser must say the request is same-origin
  (`Sec-Fetch-Site`), and the `Host` must be a loopback name.
- **The realm's own sources only.** A path is under `metaverse/PAGE/`, ends
  `.coax` or `.wgsl`, and has no part that starts with a dot
  (`metaverse-engine/editor/code-wire.coax`, `rc_path_ok`: the one rule, which
  the shell and the server both load). The server also holds it to a file the
  page's module or shaders were built from, and to a plain file that is where
  its name says. Any other path is refused, and nothing is read or written.
- **No pane on a deployed page.** The shell shows the Code button only after
  the page's host says the dev server serves the page and that server lists
  the realm's files. A deployed page's host never says so.
- **A failed build changes nothing that runs.** A save is written and
  answered with the compiler's check. The last good engine keeps running
  until a build succeeds.

Checks: `coax/web/tool.sh code-route-check` (a fixture repository: the list,
a read, a save, each refusal, `--static` and the realm origin);
`coax/bin/coax test metaverse-engine/editor/code-wire.coax --library`;
`coax/bin/coax test metaverse-engine/editor/shell.coax --library` ("code is
there only where the host says the dev server serves the page ...");
`coax/web/tool.sh metaverse-engine/tools/code-profile.coax proto/world/code-v1.json`.

## An imported asset has a licence from the list, fits its limits, and joins a pack that reads back whole

The editor's asset import (E11, #13420;
[docs/protocol/world-assets-v1.md](docs/protocol/world-assets-v1.md)) turns a
dropped glTF binary, PNG or sound file into an entry of a world's asset pack.

- **A licence from the list, never typed.** An import names its licence by
  number from `PIP_LICENCE` (`metaverse-engine/editor/pack-import-policy.coax`,
  `#authority`). The pack's catalog declares exactly those licences, in that
  order. No shape carries a licence as free text, and an import with no
  licence is refused.
- **Limits are enforced where the file is packed, and shown first.** A file
  over the file limit, a mesh over its triangles, a texture over its side, a
  pack that holds as many of a kind as it takes, and a pack that would be over
  its budget are each refused in words that name the file. The editor shows
  the limits and the pack's size against its budget before it sends anything
  (`world.assets.v1`), and refuses a file over the file limit without sending
  it. A realm sets its own limits (Splice Bots: `sp_import_limits`).
- **One pack writer.** The importer writes through `pack_write`
  (`metaverse-engine/editor/pack.coax`) and reads the result back with
  `pack_read` before it answers. A pack that does not read back is not kept.
- **A refused import changes nothing.** The pack, the files kept by content
  hash and the world's tables stay as they were.
- **A pack mesh's collider is what you see.** A `pieces` row that draws a pack
  mesh (`kit` is `pack/NAME`) is saved only when the world's pack has that
  mesh and the row's footprint and height are the mesh's bounds at the row's
  size (`metaverse/splice-bots/world.coax`, `spw_mesh_rows`). The server checks
  this; a page does not, since an edit may reach it before the pack does.
- **A page takes a new pack whole or keeps the one it has.** The page reads
  the pack with `pack_read` and the importer's catalog reader before it uses a
  byte (`metaverse/splice-bots/splice-bots.coax`, `splice_imports`).
- **An import is authoring.** It writes the world's pack and the files kept
  beside it, which are authored state. It reaches neither the character record
  nor the item ledger (`coax/web/tool.sh layer-check`, the `durable` gate).
- **Who may import.** Whoever may write the world. On the dev server
  everyone on this computer is the admin, and an import a browser sends must
  come from that server's own page (`Sec-Fetch-Site`). On the API it is an
  admin, from this site's page or signed (`coax/web/api/world-policy.coax`,
  `wr_asset_write`), checked before an upload is opened and again before the
  file is imported.
- **On the API a file is its content hash.** The page names a file by its
  SHA-256 before it sends it, and the API opens an upload in its private
  bucket for exactly the file's size and content type at
  `PREFIX/worlds/ID/files/SHA256`. Before it imports, the API reads the
  object back and checks its size and its SHA-256 against the name
  (`wr_asset_stored`). An object that is not the file named is deleted and
  never imported. The file's bytes never pass through an API request.
- **One importer for both servers.** The dev server's tool and the API call
  `metaverse/splice-bots/world-assets.coax`, so the list, the limits and every
  refusal are the same bytes.
- **A lane's pack goes with its document.** An import changes dev's pack. A
  publish makes live's pack dev's, in the statement that makes live's
  document dev's. A pack is kept while a lane names it (`wr_pack_kept`) and
  deleted when none does. An edit is checked against the pack dev has.

Checks: `coax/bin/coax test metaverse-engine/editor/pack-import-policy.coax`
and `coax/bin/coax mutate metaverse-engine/editor/pack-import-policy.coax
--root-only` (121 mutants, none survive);
`coax/bin/coax test metaverse-engine/editor/pack-import.coax` ("import: no
licence, an oversize file, a pack over its budget ..." and the three beside
it); `coax/bin/coax test metaverse/splice-bots/world-api.coax --only "pack mesh"`;
`coax/bin/coax test metaverse/splice-bots/world-api.coax --only "a world's server"`;
`coax/bin/coax mutate coax/web/api/world-policy.coax --library --root-only`
(301 mutants, none survive); the API's checks named "assets: ..." in
`coax/web/api/tests/world_checks.coax` (a build host:
`coax/remote/remote.sh run -- env API_TESTS_ONLY=world_checks.coax coax/web/api/test.sh`);
`coax/bin/coax test metaverse/splice-bots/splice-bots.coax --only "import pack"`;
`coax/bin/coax test metaverse-engine/editor/shell.coax` ("a file the rules
never take is refused in words before it is sent ...");
`coax/web/tool.sh metaverse-engine/tools/assets-profile.coax proto/world/assets-v1.json`.

## A realm that runs someone else's code has no origin, and speaks only typed messages

Such a realm is served from the realm origin, never the site's, in a frame
(#13432; [docs/metaverse/realm-origin.md](docs/metaverse/realm-origin.md)).
Every answer from the realm origin sandboxes its document
(`Content-Security-Policy: sandbox ...`, never `allow-same-origin`), so the
realm has no cookies and no storage, and the realm origin answers only a
realm's page and what it loads: never the API, a login or the chat
(`coax/web/realm-origin.coax`, `#authority`). The site, the editor's shell
and the chat reach it only by `window.postMessage`, with the messages of
`realm-frame.v1` (`metaverse-engine/host/realm-frame.coax`, `#authority`;
`proto/verse/realm-frame-v1.json`). A text that is not one of them is
refused whole. The glue (`coax/web/realm-frame.js`, `host.js`) carries text
and judges nothing; it refuses a frame on the site's own origin and hears
only the window and origin that hold it. The session cookie and our own auth
never reach the realm origin. This holds before code from the browser (E12,
#13421) opens to anyone but the owner.

Checks: `coax/web/tool.sh realm-frame-check` (headless Chrome: the site
frames Splice Bots from the realm origin; `ready splice-bots 1 1`, a typed ask
answered, another text refused, the frame unreadable from the site);
`realm-frame.coax` (242 mutants, 0 survivors) and `realm-origin.coax` (88
mutants, 0 survivors); `realm-frame-profile.coax`.

## A script's handler call is a transaction with a budget, and its failure is a typed diagnostic

`metaverse-engine/script/host.coax` runs a world's scripts for a tick
(#13467). The same file runs on the page and in a room.

- **A budget.** One handler call may spend 20,000 instructions and all of a
  world's scripts 400,000 a tick (`SH_CALL_FUEL`, `SH_TICK_FUEL`). A call
  past its budget stops; calls past the tick's are skipped and said once.
  The numbers are stated in `proto/world/script-v1.json` and shown where a
  script is edited.
- **A rollback.** A call's writes are the commands it queues and its own
  memory. A call that traps, runs out of budget or is refused leaves
  neither: the interpreter restores the memory it wrote and the host drops
  the commands and timers. A failed call never ends the tick, the page or
  the room.
- **A diagnostic.** Every failure is a `ScriptDiag` with a code registered in
  `proto/registry.json` (`engine.diagnostic.script-NAME.v1`), the script, the
  handler, the thing and the line when the script knows it. A number is
  never changed and never used again.
- **A switch.** Three failed calls in a row switch the script off until it
  is loaded again.
- **The same everywhere.** Time and chance reach a script as arguments (dt,
  the frame's time, a seeded draw), so the same frames give the same
  commands and spend the same instructions on the page and in a room.

- **Page and room, step for step.** The page's script module
  (`apps/openagents-web/script-host.coax`) and the room's unit
  (`coax/web/room/room-scripts.coax`) are the same Coax over the same bytes.
  The room's tick declares `#effects(reads, writes, traps)`: the compiler
  refuses a clock, I/O or a native call anywhere under it.

Checks: `coax/web/tool.sh coax/web/room/room-scripts-check.coax` (Splice Bots'
foes on their scripts for 900 ticks through the room's unit natively and
through the page's module under Node: the results are the same bytes, so
the same commands, diagnostics, calls and budget left, every tick);
`coax/bin/coax test metaverse-engine/script/host.coax --library`
("a call that never ends loses that call; three in a row switch the script
off", "a world's scripts share one budget a tick ...", "the module's own
trap names its line ...", "reads answer from the frame ... and a seeded
draw"); the profile's `result_hex` fixture, which pins the commands and the
instructions spent for a pinned module over a pinned frame; the fuzzer
(`metaverse-engine/tools/script-fuzz.coax`), which after every call that is
not kept compares the script's memory, globals, timers and the tick's
commands with a copy taken before it.

## A realm script writes running state only, and only its own thing

A script's writes are rows of the script API with the effect `write` or
`engine` (#13467; `metaverse-engine/script/api-table.coax`). There are no
others.

- **Its own thing.** Each names the thing it is for first, and the host
  refuses one that names any thing but the one the handler was called for
  (`not-yours`); the whole call is undone.
- **Running state.** A write becomes a command the realm applies to its own
  running state: where a thing walks, how it faces, one of its four running
  numbers, a timer. An engine call asks the engine for something it caps
  itself: a strike, a spoken line, an event.
- **Never authored or durable state.** No row edits the world document, and
  no row reaches a character record or the item ledger. A script module
  cannot name what it does not import, so this holds for any bytes that pass
  the module check, whatever compiled them. Anything with value goes through
  an engine call that the engine caps.
- **Finite numbers.** A write with a number that is not finite is refused
  (`bad-argument`), so a script cannot put a NaN into a realm.

Checks: `coax/bin/coax test metaverse-engine/script/api-table.coax --library`
("a write or an engine call names the thing it is for first, and returns
nothing"); `coax/bin/coax test metaverse-engine/script/host.coax --library`
("a write to another thing is refused and the whole call is undone");
`coax/web/tool.sh layer-check` (the engine's script folder loads neither
durable context).

## A world never waits on a script: a thing whose script said nothing walks its plan

A script adds a reaction to a thing's plan; it does not replace the plan
(#13467, #13466). In Splice Bots (`metaverse/splice-bots/scripts.coax`,
`splice-game.coax` `g_foes_step`) a foe a script row drives does what its
script said for the step being taken. A foe whose script said nothing for
that step walks and reacts by its plan, as a foe with no script does. That
covers a world with no script rows (the shipped tables), a page whose script
module has not loaded, a script that is switched off, a call that failed and
was undone, and a staggered foe.

- **Plan data first.** A script reads its thing's plan as a goal
  (`sa_goal_x`, `sa_goal_z`, `sa_goal_speed`, `sa_notice`, `sa_plan_on`), so
  a route, a speed and a time of day stay data that the editor edits.
- **Splice Bots' own rules still hold.** A scripted foe's walls stop it, its run
  speed caps it, and a strike lands only when the foe is hostile and within
  reach and the hit's cool-down allows.
- **The scripts are the plan's reaction.** `metaverse/splice-bots/scripts/`
  holds the raider's and the Imperial robot's reaction as realm scripts.

Checks: `coax/web/tool.sh metaverse/splice-bots/tools/script-play.coax` (builds
both scripts and steps a plan world and a scripted world through the same
run, with the host between steps as a page and a room run it: the foes stand
within 0.25 m of their plan twins, fight, give up and strike as they do, and
no call fails); `coax/bin/coax test metaverse/splice-bots/content.coax --library`
("a script row names a module by its hash and whom it drives ...", "tables
saved before ... the scripts table came ...").

In Everglade's rooms (`everglade/server/scripts.coax`) the room is the
authority for the monsters, so the room runs their scripts, by the same host
under the same budget. A monster whose script said nothing this tick, that is
mid-blow, or that runs home on its leash does what its behavior says
(`creatures.coax` `e2mo_step`). The engine caps every word: a walk no faster
than the monster's run, never onto the glade's safe ground and never past its
leash from home; a strike only on a seat that is its quarry, within reach,
when its blow is ready, and then as the rules' own blow after its tell. A
release with no scripts table, no row or no module that loads asks for no
script memory and runs every monster as before.

Checks: `coax/bin/coax test coax/web/room/room.coax --only "e2 scripts"` (a
script walks a wolf where it says and keeps it from its chase; a point on
safe ground is refused; its strike is the rules' blow; a call past its
budget is undone and the wolf chases by its behavior; a module that is not
its row's hash does not load).

