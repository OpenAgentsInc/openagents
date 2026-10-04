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
