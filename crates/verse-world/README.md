# Verse world

This portable crate owns chamber authority, typed commands, controller admission,
combat state, fixed-tick movement/collision admission, encounter AI, utility
spells, life-fenced respawns, and serialized events and checkpoints. It has no
GPU, socket, platform, credential, or retained Ruins dependency. The native
chamber reads its snapshots and cinematic projection. Human and controller
requests share admission. A trusted adapter supplies controller identity;
this crate does not authenticate network connections.

The `verse-chamber-owned-v16` rules profile independently implements retained
chamber behavior; it imports no vendor source. Firebolt deals 8 damage, each of
three magic missiles deals 4, and fireball deals 15 in a visible 6.096-meter radius
with three 6-damage burn ticks. Living characters regenerate one mana per second.
Spell projectiles stop at static box cover and sweep against relative actor motion
in 120 Hz slices. Actual character substep paths survive checkpoint replay;
admitted teleports skip intermediate space.
The native adapter admits commands at 30 Hz. One world clock consumes bounded
120 Hz steps for movement, combat, and timers; fractional input time remains in
the accumulator. Combat elapsed time derives from the step count, and scene time
uses a retained origin. Checkpoints validate both clocks and retain pending input.
Checkpoints rebuild static collision and compiled walkable
navigation from the scene profile. Grounded capsules admit slopes, stairs, jumps, and moving platform
poses; NPC routes fence dynamic blocker generations.

Life-bound kinematic player and NPC bodies retain 60-second corpse collision and
navigation masks across checkpoints. Removal and respawn fence exact generations.
Collision-only prop bodies share the blocker life, bounds, and removal fences;
checkpoints reject missing or mismatched prop ownership. Props cannot gain actor
damage or selection masks.
Living bodies contribute exact capsule colliders to movement queries. Controllers
ignore their own life, later actors observe admitted earlier movement, and
checkpoint loading rebuilds live colliders. Navigation plans use static and prop
geometry; controllers still admit contacts with other living actors.
The bow launches non-homing arrows through the shared continuous collision path;
cover and the first actor intercept them, moving targets can evade them, and
pending arrows replay exactly.
Hostile flights retain positions, sweep against player controller trajectories
and cover, and resolve shields through the shared damage path. Checkpoints fence
source and target lives and reject inconsistent flight positions.
The combat store now admits up to 64 cooperative players in one simulation, with
actor-scoped health, mana, regeneration, and projectile-spell cooldowns. Explicit
caster APIs derive release positions from admitted actors. Arrows, homing spells,
area hits, and burns exclude friendly players; projectiles and burns retain their
caster. A revival cancels only that caster's flights and burns. `advance` steps all
players on one clock, and `snapshot_for` projects the selected player's resources
alongside public shared actors. `Game::add_player` binds trusted controllers to
additional adventurers in this same chamber. `submit` dispatches by exact player
life and retains independent movement, jump, casting, bow/utility cooldowns,
shields, and travel animation phase. All ten original abilities share the same
NPCs, capsule collision, spell physics, projectiles, events, and fixed clock.
Hostiles select the nearest living adventurer with actor-ID ties and sweep their
actual movement paths; defeat ends the encounter only when all adventurers die.
Owned respawn and reset fence lives and commands without dropping other players.
Native effect projections include each player's shield and light; audio routes
projectile and shield cues by caster life. The native camera/HUD still focuses the
primary adventurer. `player_snapshot` and `player_admission` expose actor-specific
state to a trusted host; they do not authenticate a network caller. Additional
catalog spells outside the original ten still need shared-caster adapters.
`Game::new_in` and `Game::combat_in` bind collision, navigation, and actor lives to
the trusted host’s selected instance. Checkpoints and combat resets preserve it;
the existing local constructors select instance zero. Instance identity fences
commands and targets but does not authenticate a caller.
`service::Chamber` owns one game behind explicitly enrolled principal rights.
A trusted transport supplies verified identities and retains opaque connection
handles. Player commands derive controller identity from the connection;
spectators cannot act. Reconnect, disconnect, revocation, and instance reset
fence queued input. Only the host advances the world clock. Grants and connections
are bounded and remain in memory; handles are adapter bindings, not bearer tokens.
The optional `service-auth` feature adds `service::auth::Gateway`: enrolled
x-only public keys sign a versioned SHA-256 challenge with secp256k1 Schnorr.
OS-generated nonces bind each 30-second, single-use challenge to server lifetime,
instance, host-assigned connection, deadline, and key. Authenticated dispatch
accepts a transport-retained connection handle, with no request-supplied principal
or controller. Pending and authenticated connections share a 128-entry budget.
The host supplies monotonic time; expired, replayed, malformed, foreign, and
unenrolled proofs are refused. Executable/deployment integration, durable grants,
and replication remain; this is not a Nostr authentication protocol.
`service::wire` provides version-three JSON opening challenges and bounded request/
response messages. Requests carry correlation IDs and command life/epoch/sequence
fences, with no caller-selected principal, controller, or connection. Replies
include the host tick and the player’s current control state, including consumed
command sequences after gameplay refusals. Snapshots bind internal combat IDs to
actor lives; event pages expose retained serials and explicit history gaps.
Requests are limited to 16 KiB, replies to 2 MiB, and event pages to 64 entries.
Unknown fields, bad versions, malformed proofs, and unauthorized operations are
refused. The network adapter retains the connection handle and supplies time.
The optional `service-net` feature adds `service::net::serve`, a TLS-only listener
over big-endian u32-length-prefixed JSON. The host supplies a bound Tokio listener,
a Rustls certificate/key configuration, enrolled gateway, and shutdown future.
One host loop owns commands and 30 Hz world stepping; socket workers never hold
world authority during IO. Sockets and dispatch queues are bounded to 128, with
5-second TLS handshakes, 30-second initial authentication reads, 60-second idle
reads, 10-second writes, and 120 requests per second per socket. Catch-up is
bounded to 100 ms; transport statistics retain skipped elapsed time. Shutdown
drains workers, parks controllers, and returns authority plus failure diagnostics.
Temporary loopback TLS tests use generated certificates and synthetic identity
keys. Client prediction, subscribed replication, durable deployment configuration,
and native service integration remain.
`service::client::Client` connects with caller-configured Rustls trust and server
name, verifies the opening instance/version, and signs with a caller-provided
keypair without retaining it. Sequential requests validate correlation, host
ticks, player control fences, life bindings, event cursors, and response kinds.
Commands derive their life, epoch, and next sequence from acknowledged state;
valid gameplay refusals retain their consumed sequence. Ten-second IO deadlines,
protocol failures, and cancellation of uncertain requests drop the socket without
automatic replay. Poll snapshots at the replication cadence before issuing input;
this client does not yet predict ticks or subscribe to streaming snapshots.
Version-two snapshots also carry life-bound actor appearances, animation selection/
phase, health/visibility, and each player’s public shield, light, and area effects.
The shared authority extracts these alongside the combat snapshot on the same
tick. Client admission checks finite poses/times, actor/effect uniqueness, life
bindings, and budgets. The primary player’s presented health now follows actual
resources, including zero-health death poses. Version-one peers are refused;
streaming cadence and native service rendering remain separate integration work.
Wire version three adds a retained per-life Misty Step stamp, so short teleports
remain discontinuities even when another ability is cast before the next snapshot.
`service::replica::Buffer` admits snapshots atomically and retains two frames plus
a bounded generation history. Read-only samples interpolate compatible positions,
wrapped yaw, and animation phase, and keep shield anchors on sampled bodies.
Life/control changes, teleports, death/visibility, model/animation changes, and
large displacements snap. Resources and effect status remain authoritative latest
values. World resets require advanced lives; stale ticks, control fences, and
generations are refused. Streaming and input prediction still remain.

`service::event_cursor::Cursor` validates contiguous event pages and delivers
each committed serial once. Retention gaps report the missing range explicitly.
Instance-scoped checkpoints retain progress without dialogue or credentials;
the TLS client’s `delivered_events` helper advances them after validation.
Native effects/audio integration and durable client checkpoint storage remain.

`service::worker::run` owns client IO on a Tokio task outside rendering. Fixed
queues retain ordered snapshots, events, and command outcomes with backpressure;
33–1,000 ms polling skips missed intervals. Commands refresh admitted control
before submission. Shutdown cancels uncertain IO without replay. Persist event
checkpoints only after consuming their delivery. Native rendering and prediction
still require adapters.

Wire version four also carries hostile cast telegraphs/flights and transient
impact flashes. Presentation validates caster/target lives, finite positions,
timelines, radii, effect kinds, and budgets. These values describe visuals;
clients never apply hostile damage. Older wire versions are refused.

Wire version nine retains bounded NPC corpse poses after combat records retire.
Corpses carry exact lives, death animations, zero health, and hidden nameplates;
they expire or disappear when a new life respawns. Replica admission refuses
retired corpse reappearance and resurrection of an ended life.

`visuals::Combat` supplies read-only spell, shield, area, hostile, and impact
values to rendering. Local extraction and validated `State::combat_visuals`
share that contract. Native spell instances and dynamic lighting consume it;
the local app extracts it once per frame. Remote transport mounting, HUD,
props/blockers, and full service acceptance remain.

`service::view::View` projects interpolated remote poses and bow flights into
native scene frames using a validated client-owned camera. Ordered events
provide life-bound dialogue and camera handoff; duplicate delivery does not
replay cues. Respawn and world reset fence old dialogue, and retention gaps
remain visible. The view retains bounded read-only event history for HUD/audio
adapters. `View::damage_numbers` projects actual committed amounts onto matching
sampled lives, including lethal damage, and expires them after 1.35 seconds.
Local and remote values share native floating-text rendering and colors. Native
window/transport mounting and remote action/resource HUD still remain.

Wire version five adds `hud::Own` for the authenticated controlled life. Health,
mana, the ten shared-kit cooldown gates, and cast progress come from that
player’s state; spectators receive no owned HUD. Client and replica admission
match HUD life to acknowledged control and validate resources, clocks, slots,
and cast targets. Native `owned_hud` drawing shares the local ten-slot row,
portrait/resources, cast bar, and respawn button. Owned hit tests exclude hidden
catalog slots and gate death/respawn controls. Remote window mounting, target
HUD, and catalog shared-caster adapters remain.

The remote view retains an exact-life target and cycles live hostile poses in
stable actor order. Death events, hidden/dead snapshots, and new generations
clear selection; friendly/foreign/stale lives are refused. Native `target_hud`
shares local portrait/name/health drawing, hides dead targets, and rejects stale
frame lives before drawing. Remote mounting and catalog adapters remain.

Wire version six carries live physics prop box poses through presentation.
Kinds, secured variants, dimensions, centers, and unit rotations are bounded
and instance/life validated. Replica sampling interpolates compatible poses,
snaps life/shape changes, omits removed props, and refuses retired generations.
An admitted world reset clears prior prop interpolation/history. Native local
and remote prop values share model transforms without client physics stepping.
Wire version seven also carries active blocker lives and bounds, omits corpse
blockers, and preserves authored table proxy metadata. Admission rejects malformed
boxes and retired lives. Native local and remote bounds share model transforms;
remote bounds use the latest host state without constructing client collision.
A remote scene sample now supplies the frame, sampled prop/blocker bounds, and
combat visuals together. Native assembly shares actor/attachment, particle, prop,
cover, and lighting helpers behind Verse's `remote-chamber` feature; shield
anchors follow sampled body positions.
The `verse_remote` example mounts an authenticated worker in a native window,
with bounded update consumption, Classic controls, the shared ten-slot HUD,
respawn, targets, nameplates, committed damage, and cinematic-to-follow camera.
Movement submissions wait for an empty input queue to avoid accumulating stale
movement behind casts. Window exit stops and joins the worker.

Run it with:

```sh
cargo run -p verse --no-default-features --features imported-desktop,remote-chamber --example verse_remote -- CONFIG.json
```

The JSON configuration requires `address` (socket address), `server_name` (TLS
name), `instance` (host instance), `trust_der` (DER trust certificate path),
`key_file` (owner-only file containing the enrolled 32-byte signing key as hex),
`pack` (local pack manifest path), `scene` (local scene JSON path), and `dir`
(local pack asset directory). Paths resolve from the working directory. The key
is used during login and is not passed to the window or worker. Callers must
supply matching original scene/assets and a configured host; automatic host
setup, durable state/grants, remote audio, prediction, and native live
acceptance remain.

Run the configured host with:

```sh
cargo run -p verse --no-default-features --features remote-chamber --example verse_host -- HOST.json
```

The remote client configuration can include
`"record":{"output":"combat.mp4","seconds":45,"controller":true}` to run a
bounded programmatic shared-kit controller and record submitted GPU frames.
Recording requires `ffmpeg`, accepts 1–120 seconds, and uses a two-frame
readback/encoder queue. Its 1280 × 720, 30 FPS video preserves elapsed capture
time with reported duplicate frames when sampling is slower. The companion JSON
records sampled/duplicate/dropped counts, native dimensions, acknowledged cast
commands, committed damage/dialogue counts, and final admitted state. It does not
prove every acknowledged spell caused damage or establish gameplay frame budgets.
Without `record`, the window remains in human-control mode.
Set `record.respawn` to `true` with `record.controller` to attempt authenticated
respawn once per dead owned life. Spectators issue no commands. The capture proof
records attempted lives and admitted owned-life changes; neither counter alone
proves a successful respawn. Uncertain attempts are not replayed.
The [native multiplayer receipt](../../bench/verse/2026-10-04/native-multiplayer/run.json)
links three live 90-second captures, shared damage outcomes, and both player
respawns. Native polling uses 20 Hz replication with 30 Hz input; each encoder
uses two codec threads and one filter thread. Timing duplicates remain explicit.

Host JSON requires `listen` (socket address), nonzero `instance`, `scene`, `pack`,
`certificate_der`, `private_key_der`, and `enrollments`. Paths resolve from the
working directory. Each enrollment contains a 64-character x-only `public_key`
and `role`: `{"type":"primary"}`, `{"type":"player","spawn":[x,y,z]}`, or
`{"type":"spectator"}`. Keys must be unique; at most one primary, 63 additional
players, and 128 total enrollments are accepted. The private DER key requires
owner-only permissions on Unix. The host loads combat authority and pack prop
collision before serving TLS. Ctrl+C or Unix SIGTERM drains connections and
prints final tick/request/timing statistics. Grants and world state are not yet
saved across process restart; live native multiplayer acceptance remains.

Wire version eight binds content identity into the signed connection challenge.
Configured host/client entry points hash the validated scene and compiled pack,
then stream and verify every runtime texture against its manifest digest.
Textures must be beside the host pack manifest; the client uses its configured
`dir`. A different scene, pack, or texture digest is refused before the client
sends an authentication signature. Texture reads are bounded to 64 MiB each and
512 MiB total. The portable gateway/client APIs retain an explicit unconfigured
mode for fixtures; configured entry points require exact identity matching.

Revocation leaves an uncontrolled actor in the world; actor retirement and capacity reclamation remain lifecycle work.
Transactional saves, multiplayer
replication, and authoring tools remain on the
[engine roadmap](../../docs/verse/engine/roadmap.md).

The adventurer starts with 200 HP. After defeat, **Respawn** restores health and mana at the authored spawn, returns human control, and advances the player life and command epoch. NPC health and cultist respawn deadlines remain intact.

Presentation snapshots carry named animation states and exact actor lives. They select locomotion, combat, casting, prone, and death poses without assuming any model’s internal clip IDs.

## Spell physics and the spell playground

`spells::SpellWorld` gives the chamber a `physics::World` of dynamic props
(kind, SRD size, mass, material, `secured`, `flammable`, object hit points),
stepped on the 120 Hz clock and saved in checkpoints. Characters carry
external motion (`physics::character::Character::external`, friction decay on
the ground, ballistic in the air), a gravity override, and the fall height
SRD falling damage reads. `push` speeds are calibrated so an unobstructed push
travels its SRD distance; walls and contacts change the result. Spell fields
(box, cylinder, wall polyline, sphere) add acceleration; spell-owned bodies,
joints, and concentration end with their cast. Every external impulse is a
named `physics::Ledger` term.

To add a spell: write `src/spells/<name>.rs` with its cast and a
`playground::Scenario`, add one `SpellDef` line to `spells::CATALOG` (its
reserved row-two slot), and one line to `playground::scenarios`. Record it with
`cargo run -p verse --features imported-desktop --example verse_play --
--original --spell-playground <name> bench/verse/<date>/spell-<name>.mp4`.

`service::auth::Gateway::checkpoint` saves the bounded world and validated
identity-to-character grants with content identity. `Gateway::restore` requires
matching content and instance, preserves lives/resources/pending combat, parks
controls, and creates fresh authentication. It rejects duplicate or missing
character ownership and retains no challenges or sessions. Configured hosts use
`service::persistence::Store` for durable writes.

Set `state_dir` in the host JSON to a dedicated storage directory. The host creates
new directories with owner-only permissions, holds one exclusive writer lock,
and refuses corrupt, incompatible, or changed startup enrollments and authored
content. It recovers existing adventurers, resources, pending combat, and timers;
simulation time resumes without offline catch-up. Without `state_dir`, the host
uses memory only.

Durable hosts group bounded pending replies into one atomic, synced checkpoint
per world tick. Reads wait for that commit when mutations are pending. A storage
failure stops the host without publishing those replies. Committed snapshots
carry version, revision, and checksum; interrupted staging files are discarded
under the writer lock. The host reports checkpoint commits, bytes, and elapsed
storage work. The [durable host fixture](../../bench/verse/2026-10-04/durable-host/run.json)
retains restart/refusal checks and shared TLS input measurements.

Trusted hosts use `Gateway::grant_reward` for bounded character experience,
item stacks, and quest counters. A stable source ID binds one exact transaction
per character; retries return its original receipt, and conflicting reuse or
limit failures leave every field unchanged. Call `Store::commit` before
acknowledging a host-created reward. Version-five chamber saves replay the retained
transactions; version-one saves upgrade with an empty ledger. Instance reset
retains rewards and retry identities. Receipts are never evicted: after 4,096
transactions, new grants are refused. Host JSON accepts an optional sorted `rewards` array of NPC targets and grants,
for example `[{"target":2,"experience":45,"items":[{"id":1,"count":1}],"quests":[{"id":1,"count":1}]}]`.
The cooperative version-one policy grants every enrolled adventurer, including
disconnected or dead party members, once per defeated NPC life. Spectators receive
none. Respawns and instance resets create new NPC life generations; saved source
IDs and the event cursor prevent duplicate rewards after recovery. Changed reward
policies are refused on recovery. With no configured rewards, combat grants none.
An authority or storage failure stops the host and retains its previous durable
checkpoint, including when an entire cooperative reward batch cannot fit.

Wire version thirteen provides an authenticated `inventory` read. The connection determines
the character; request bodies contain no actor or grant amounts. Players can read
their experience, bounded item stacks, and quest counters while dead. Spectators
are refused. `service::client::Client::inventory` validates the owned life, counts,
and nonregressing transaction revision. The [combat reward receipt](../../bench/verse/2026-10-04/combat-rewards/run.json)
retains real loopback TLS spell/reward/restart assertions and their synthetic
fixture limits. Equipment and combat stat progression remain.

The native worker refreshes owned inventory once per second and immediately after
an owned-life change; spectators send no inventory requests. `service::view::View`
retains bounded read-only counters, rejects conflicting revisions, and hides
inventory until its exact life matches the owned snapshot. The native remote
window opens inventory with **B** or **I** and the quest log with **L**. **Escape**
closes an open window first. Use the arrow buttons, **Page Up**/**Page Down**, or
the mouse wheel over the panel to page through entries. Panel clicks do not reach
spell or camera input. Configured quests show authored names, objective progress,
rewards, and a **Claim** button when ready. Inventory shows the current level and
XP progress. Successful claims refresh authoritative inventory.
The [native panel receipt](../../bench/verse/2026-10-04/character-panels/run.json)
retains a GPU layout capture, source/artifact hashes, and focused checks. Its
synthetic counters establish layout, not live rewarded-combat acceptance.

Host JSON accepts optional version-one `progression` configuration, for example
`{"version":1,"levels":[0,100,300],"quests":[{"id":1,"name":"Disrupt the summoning","objective":1,"goal":12,"experience":75,"items":[{"id":1,"count":2}]},{"id":2,"name":"Secure the chamber","prerequisites":[1],"objective":1,"goal":24,"experience":150,"items":[]}]}`.
Campaign quests without prerequisites are available to every enrolled character.
An optional `prerequisites` array lists up to 16 sorted, unique earlier quest IDs.
The host unlocks a quest only after that character claims every prerequisite in
the same instance. Kill reward counters supply objectives, including counters
earned before unlocking; claims select only the quest ID under the current owned life
and control epoch. The host checks availability and completion and applies configured rewards
once per character and instance, including across respawn, reset, and recovery.
Durable hosts commit before acknowledging claims. Saved progression configuration
is immutable on recovery; earlier saves upgrade with no configured quests and
level one. Level thresholds affect presentation only. Abandonment,
repeatable quests, quest givers, and combat stat scaling remain.
Wire version fifteen carries quest availability. Locked quests show their status
and expose no claim action. Recovery replays prerequisite claims in ledger order
and rejects follow-up claims whose prerequisites are absent. Existing saves
without prerequisite arrays retain their original behavior.
A quest can declare a `giver` with an authored scene NPC ID. These quests require
explicit acceptance: the current adventurer and giver must be alive, within
four meters, and visible through collision queries. The request binds both life
generations and the adventurer's control epoch. Acceptance retains the current
objective counter as a baseline; only later progress counts. First turn-in
requires the same proximity and sight checks. Exact acceptance/claim retries
return original receipts under current character control without requiring a
repeat interaction. This does not change NPC faction or combat behavior.

Saved chamber version seven replays acceptance records before claims and checks
each baseline against prior objective transactions. Earlier saves retain their
automatic quest behavior. Wire version sixteen carries acceptance, giver life,
and interaction availability. The native quest panel shows **Accept** while near
an available giver and **Claim** for a completed accepted quest; outside range it
asks the player to return. The network worker refreshes inventory after actions.
Quest enrollment is permanent for the first campaign; abandonment, repeatability,
dialogue authoring, and friendly NPC behavior remain.
The [enrollment receipt](../../bench/verse/2026-10-04/quest-enrollment/run.json)
retains TLS storage-failure/restart evidence and synthetic GPU panel captures.

The [campaign receipt](../../bench/verse/2026-10-04/campaign/run.json) retains
focused TLS recovery checks and synthetic ready/completed GPU panel captures.

Host JSON accepts optional version-one `items` configuration, for example
`{"version":1,"items":[{"id":1,"name":"Ritual recovery ember","health":45,"mana":5}]}`.
The catalog contains up to 64 recovery definitions, with health capped at 200
and mana at 20. Definitions are immutable on recovery. Reward stacks without
a recovery definition remain collectible. The native remote inventory shows
authored names, effects, and **Use** buttons for owned recovery items.

`Client::use_item` requires an explicit nonzero 16-byte operation ID. Retain that
ID when an acknowledgment is uncertain; exact retries return the original
receipt without spending or restoring again. Requests use the current owned life
and control epoch, with no client-supplied amounts. One item restores a living
player's resources up to their caps; dead players, empty stacks, and already-full
resources are refused without mutation. Durable hosts commit the stack debit and
world restoration before acknowledgment. Version-four saves replay ordered
grants/debits without repeating restoration; earlier saves upgrade with an empty
catalog. The native worker generates one operation ID per click and stops on an
uncertain transport failure instead of automatically retrying with a fresh ID.
The [item-use receipt](../../bench/verse/2026-10-04/item-use/run.json) retains
TLS storage-failure/restart checks and a synthetic GPU inventory capture.

Host JSON accepts optional version-one `outfits` configuration, for example
`{"version":1,"outfits":[{"id":2,"name":"Ranger outfit","model":"universal-male-ranger"}]}`.
Outfits and recovery items require distinct item IDs. Kill and quest rewards can
grant outfit stacks through the existing reward entries. **Equip** selects an
owned outfit; **Unequip** returns to the character's base appearance. Selection
does not spend the stack or change combat resources, actor identity, or control.
`Client::equip_outfit` uses an explicit retained 16-byte operation ID, with zero
as the base outfit selection. Exact retries return the original receipt and do
not undo a later selection. Version-five saves replay owned selections against
immutable definitions; older saves upgrade with no configured outfits.

Wire version thirteen sends equipped render models for every controlled player,
including to spectators. Native drawing uses the equipped rig for character and
bow presentation while retaining authoritative actor identity. Host startup and
native extraction refuse missing outfit models or missing chamber animation
states. The existing Universal pack includes six `universal-*` appearances.
The [outfit receipt](../../bench/verse/2026-10-04/outfits/run.json) retains
TLS ownership/restart checks, synthetic panel layouts, and an animated-model GPU
capture. Head/main-hand attachments and resource bonuses are described below;
other gear slots, damage/armor modifiers, and live window input acceptance remain.

Host JSON also accepts version-one `equipment`, with up to 64 sorted, unique
gear definitions. For example:
`{"version":1,"gear":[{"id":3,"name":"Ritual hat","slot":"head","model":"gear-hat","offset":[0,0,230],"health":100,"mana":0},{"id":4,"name":"Ritual wand","slot":"main_hand","model":"gear-wand","offset":[0,0,0],"health":0,"mana":10}]}`.
Gear IDs must differ from recovery and outfit IDs. Reward entries grant ownership;
the inventory's **Equip** and **Unequip** actions select one item per slot without
spending its stack. `Client::equip_gear` requires a retained operation ID, current
life, and current control. New changes refuse defeated characters. Exact retries
return the original receipt without restoring a later unequipped selection.

Health and mana bonuses raise the owned maxima without healing current resources
or resetting cooldowns. Removing a bonus clamps current resources. Respawn and
instance reset restore full resources at the selected maxima. Saved chamber
version six checks derived maxima against the replayed ownership ledger; older
saves upgrade with no gear. Wire version fourteen carries slot selections and
visible static gear for both players and spectators. The native host admits gear
models and required sockets before serving; the renderer hides the main-hand
model while the bow occupies the hands. The [equipment receipt](../../bench/verse/2026-10-04/equipment/run.json)
retains TLS storage-failure/restart evidence and native rendering/panel fixtures.
Damage/armor modifiers remain. Native attachments now consume each parent's
final blended and grounded palette through the portable leaf-mount contract.
