# Verse world

This portable crate owns chamber authority, typed commands, controller admission,
combat state, fixed-tick movement/collision admission, encounter AI, utility
spells, life-fenced respawns, and serialized events and checkpoints. Its default
build has no GPU, socket, platform, credential, or retained Ruins dependency.
Optional service features add authentication, clients, and native transport;
a trusted adapter binds verified identity to the same command admission.
The native chamber reads snapshots and cinematic projections.

The [current capability table](../../docs/verse/status.md) identifies implemented
paths, platforms, acceptance, and limits. The generated
[runtime contract](../../docs/verse/runtime-contract.json) records source-owned
versions and Cargo features. Version numbers in milestone descriptions below
identify when a behavior was introduced; they are not the current wire version.

The `verse-chamber-owned-v24` rules profile independently implements retained
chamber behavior; it imports no vendor source. Firebolt deals 8 damage, each of
three magic missiles deals 4, and fireball deals 15 in a visible 6.096-meter radius
with three 6-damage burn ticks. Living characters regenerate one mana per second.
Spell projectiles stop at static box cover and sweep against relative actor motion
in 120 Hz slices. Actual character substep paths survive checkpoint replay;
admitted teleports skip intermediate space.
Friendly scene actors retain their authored health, stay out of encounter AI and
combat targeting, and bind to friendly simulation factions in checkpoints; legacy
v16 checkpoints remain admissible only when no friendly roles are present.
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
The same actor record and ability dispatcher serve primary and additional
players. The nine physics catalog spells capture the caster's current life,
source, pose, selection, and save difficulty. Catalog cooldowns, dice, concentration,
and commands belong to each caster. Validated character and encounter definitions
configure resources, catalog cost/timing, save difficulty, hostile health, and
encounter timing. World transfer carries character tuning and remaining catalog
cooldowns. Gameplay refusals restore spell effects and resources while retaining
consumed command sequences. Wind Wall retains the support state its next step
needs for checkpoint replay.
Hostiles select the nearest living adventurer with actor-ID ties and sweep their
actual movement paths; defeat ends the encounter only when all adventurers die.
Owned respawn and reset fence lives and commands without dropping other players.
Native effect projections include each player's shield and light; audio routes
projectile and shield cues by caster life. The native camera/HUD still focuses the
primary adventurer. `player_snapshot` and `player_admission` expose actor-specific
state to a trusted host; they do not authenticate a network caller. The original
kit and nine physics catalog spells use the shared per-caster authority path.
`Game::new_in` and `Game::combat_in` bind collision, navigation, and actor lives to
the trusted host’s selected instance. Checkpoints and combat resets preserve it;
the existing local constructors select instance zero. Instance identity fences
commands and targets but does not authenticate a caller.
`service::Chamber` owns one game behind explicitly enrolled principal rights.
A trusted transport supplies verified identities and retains opaque connection
handles. Player commands derive controller identity from the connection;
spectators cannot act. Reconnect, disconnect, revocation, and instance reset
fence queued input. Only the host advances the world clock. Grants and connections
are bounded; configured persistence retains grants across restart. Connections
and challenge state remain transient. Handles are adapter bindings, not bearer tokens.
The optional `service-auth` feature adds `service::auth::Gateway`: enrolled
x-only public keys sign a versioned SHA-256 challenge with secp256k1 Schnorr.
OS-generated nonces bind each 30-second, single-use challenge to server lifetime,
instance, host-assigned connection, deadline, and key. Authenticated dispatch
accepts a transport-retained connection handle, with no request-supplied principal
or controller. Pending and authenticated connections share a 128-entry budget.
The host supplies monotonic time; expired, replayed, malformed, foreign, and
unenrolled proofs are refused. Configured hosts integrate this gateway with
durable state and replication. This challenge is separate from Nostr relay AUTH.
`service::wire` provides versioned JSON opening challenges and bounded request/
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
5-second TLS handshakes, a 30-second absolute chamber authentication deadline, 60-second idle
reads, 10-second writes, and 120 requests per second per socket. Catch-up is
bounded to 100 ms; transport statistics retain skipped elapsed time. Shutdown
drains workers, parks controllers, and returns authority plus failure diagnostics.
Temporary loopback TLS tests use generated certificates and synthetic identity
keys. The optional `service-reach` feature adds `service::reach`: the same frames
over a NIP-REACH direct channel (TCP or WebSocket) with no certificate, admitted
by a NIP-HOST grant with the `world` right, rechecked before every request and on
a timer; a granted key outside the role table joins as a spectator. TLS and REACH
clients share the duplex runtime, replication worker, prediction, and native
session. The dedicated `verse-host` executable uses TLS; REACH remains an
application integration.
`service::net::AdmissionStats` retains aggregate admission/refusal and classified
connection outcomes. The shared policy reserves at most 32 of 128 transport
slots for pending authentication, with eight pending per IP. Principal request
credit survives reconnects; supersession closes the previous transport at once.
Commands and projections have independent principal and aggregate token buckets.
Overload returns `rate_limited` before authority dispatch, with no operation or
sequence consumed. Realm listeners share the policy across instances. These
budgets bound work counts; they do not establish hardware throughput or a CPU
frame-time guarantee. See [V08](../../docs/audits/2026-10-04-verse-engine-audit.md#v08-admission-and-request-work-have-bounded-policies)
for token prices, limits, and retained acceptance evidence.

`service::client::Client` connects with caller-configured Rustls trust and server
name, verifies the opening instance/version, and signs with a caller-provided
keypair without retaining it. Sequential requests validate correlation, host
ticks, player control fences, life bindings, event cursors, and response kinds.
Commands derive their life, epoch, and next sequence from acknowledged state;
valid gameplay refusals retain their consumed sequence. Ten-second IO deadlines,
protocol failures, and cancellation of uncertain requests drop the socket without
automatic replay. The sequential API remains available for explicit operations;
`client_runtime` and `worker` provide duplex scheduling, acknowledged replication,
and input confirmations. The presentation session owns movement prediction.
Version-two snapshots also carry life-bound actor appearances, animation selection/
phase, health/visibility, and each player’s public shield, light, and area effects.
The shared authority extracts these alongside the combat snapshot on the same
tick. Client admission checks finite poses/times, actor/effect uniqueness, life
bindings, and budgets. The primary player’s presented health now follows actual
resources, including zero-health death poses. Current peers must match the
source-owned wire version; the shared worker and native session consume these values.
Wire version three adds a retained per-life Misty Step stamp, so short teleports
remain discontinuities even when another ability is cast before the next snapshot.
`service::replica::Buffer` admits snapshots atomically and retains two frames plus
a bounded generation history. Read-only samples interpolate compatible positions,
wrapped yaw, and animation phase, and keep shield anchors on sampled bodies.
Life/control changes, teleports, death/visibility, model/animation changes, and
large displacements snap. Resources and effect status remain authoritative latest
values. World resets require advanced lives; stale ticks, control fences, and
generations are refused. Acknowledged spatial updates and prediction integrate
with this buffer through the shared worker and session.

`service::event_cursor::Cursor` validates contiguous event pages and delivers
each committed serial once. Retention gaps report the missing range explicitly.
Instance-scoped checkpoints retain progress without dialogue or credentials;
the TLS client’s `delivered_events` helper advances them after validation.
Native effects, captions, and audio consume committed deliveries. Persist client
cursor checkpoints after consumption when an adapter needs restart continuity.

`service::worker::run` owns client IO on a Tokio task outside rendering. Fixed
queues retain ordered snapshots, events, and command outcomes with backpressure;
33–1,000 ms polling skips missed intervals. Commands refresh admitted control
before submission. Shutdown cancels uncertain IO without replay. Persist event
checkpoints only after consuming their delivery. The shared chamber session
connects this worker to rendering and prediction on desktop, the Rust mobile
mount, and the browser.

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
the local app extracts it once per frame. The remote session mounts these values
alongside its HUD and prop/blocker projections; measured acceptance is scoped
in the [status guide](../../docs/verse/status.md).

`service::view::View` projects interpolated remote poses and bow flights into
native scene frames using a validated client-owned camera. Ordered events
provide life-bound dialogue and camera handoff; duplicate delivery does not
replay cues. Respawn and world reset fence old dialogue, and retention gaps
remain visible. The view retains bounded read-only event history for HUD/audio
adapters. `View::damage_numbers` projects actual committed amounts onto matching
sampled lives, including lethal damage, and expires them after 1.35 seconds.
Local and remote values share native floating-text rendering and colors. The
shared chamber session mounts the transport, view, and owned resource/action HUD.

Wire version five adds `hud::Own` for the authenticated controlled life. Health,
mana, the ten shared-kit cooldown gates, and cast progress come from that
player’s state; spectators receive no owned HUD. Client and replica admission
match HUD life to acknowledged control and validate resources, clocks, slots,
and cast targets. Native `owned_hud` drawing shares the local ten-slot row,
portrait/resources, cast bar, and respawn button. Owned hit tests exclude hidden
catalog slots and gate death/respawn controls. The remote session also mounts
the target HUD and catalog action-bar projection.

The remote view retains an exact-life target and cycles live hostile poses in
stable actor order. Death events, hidden/dead snapshots, and new generations
clear selection; friendly/foreign/stale lives are refused. Native `target_hud`
shares local portrait/name/health drawing, hides dead targets, and rejects stale
frame lives before drawing. The remote session consumes this target projection
and the catalog action bar.

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
supply matching original scene/assets and a configured host. Durable state and
grants require `state_dir`; remote prediction, native audio, and the bounded
native battle profile are implemented. Route discovery and device qualification
retain the limits in the [platform guide](../../docs/verse/platform-clients.md).

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
Profile version eight separates the first 120 submitted frames from steady
measurements. Set `VERSE_GPU_TIMING=1` to request optional delayed GPU pass
timestamps. Reports name the adapter, resolution, and sample count, and separate
CPU preparation, encoding, submission, surface acquisition, present calls,
capture queue residence, readback, and encoder pipe writes. A present call does
not establish display completion. Input-to-display latency and one-way network
age remain unavailable without platform feedback or synchronized clocks.

For a scratch TLS workload with offscreen clients, run:

```sh
VERSE_GPU_TIMING=1 cargo run -p verse --no-default-features --features remote-chamber --example frame_profile -- profile.json 3 480 30
```

The final argument samples raw readback every 30 frames; use zero to disable it.
The fixture uses temporary original assets and durable state, 1280 × 720 targets,
four-sample antialiasing, and one shared adapter. It measures request turnaround
and local snapshot age separately. It creates no display surface or video.
See the [frame attribution evidence](../../bench/verse/2026-10-04/frame-attribution/run.json)
for the measured workload and limits.

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
working directory. Optional `authored_combat_health: true` preserves hostile health
from the scene through restart, respawn, and saved-scene compatibility checks;
omitting it retains the chamber encounter defaults. Each enrollment contains a 64-character x-only `public_key`
and `role`: `{"type":"primary"}`, `{"type":"player","spawn":[x,y,z]}`, or
`{"type":"spectator"}`. Keys must be unique; at most one primary, 63 additional
players, and 128 total enrollments are accepted. The private DER key requires
owner-only permissions on Unix. The host loads combat authority and pack prop
collision before serving TLS. Ctrl+C or Unix SIGTERM drains connections and
prints final tick/request/timing statistics. With `state_dir`, the host restores
world state and grants and acknowledges mutations after ordered durable writes.
Without it, the host is ephemeral. Native acceptance is limited to the profiles
in the [status guide](../../docs/verse/status.md).

Wire version eight binds content identity into the signed connection challenge.
Configured host/client entry points hash the validated scene and compiled pack,
then stream and verify every runtime texture against its manifest digest.
Textures must be beside the host pack manifest; the client uses its configured
`dir`. A different scene, pack, or texture digest is refused before the client
sends an authentication signature. Texture reads are bounded to 64 MiB each and
512 MiB total. The portable gateway/client APIs retain an explicit unconfigured
mode for fixtures; configured entry points require exact identity matching.

A standalone chamber revocation parks control. Realm logout and character
selection additionally retire resident actors and reclaim dormant slots; these
are different lifecycle operations. Transactional saves, replication, and the
content workbench have implemented paths in the
[current capability table](../../docs/verse/status.md).

The adventurer starts with 200 HP. After defeat, **Respawn** restores health and mana at the authored spawn, returns human control, and advances the player life and command epoch. NPC health and cultist respawn deadlines remain intact.

Presentation snapshots carry named animation states and exact actor lives. They select locomotion, combat, casting, prone, and death poses without assuming any model’s internal clip IDs.

Quest catalogs can include bounded authored offer, objective-reminder, and turn-in
dialogue. Owned quest progress carries that text; the native giver panel selects
the offer, reminder, or turn-in text from enrollment and objective state.

Wire version 23 adds acknowledged spatial replication. `Client::snapshot` and
native worker reads send `replicate` with the last completely admitted baseline.
`Client::resync` discards retained bytes and requests a full baseline. The SDK
reconstructs packets into ordinary validated `Reply::Snapshot` values before
presentation, HUD, and prediction consume them. Diagnostic `snapshot` reads and
initial movement-mode entry still return complete snapshots.

The host selects a controlled character's center, or the primary adventurer for
an enrolled spectator. Clients cannot widen the 64-meter presentation radius.
Reusable 32-meter cell lists select nearby actors; relevant projectiles and
telegraphs retain their life-bound endpoints. Collision relevance uses an
80-meter radius and conservative sphere bounds for rotated shapes and large
supports. HUD, resources, and cooldowns belong to the admitted owner. Spectators
receive no private resource or ability projection. Inventory remains a separate
owner-only read.

Near poses, owned movement, and relevant dynamic collision update at the requested
cadence. Outer-band transforms refresh after six authority ticks (5 Hz), including
when delayed polling skips exact tick boundaries. Health, life, equipment, and
teleport changes bypass that hold. Unchanged object fields send no delta edits.
Every delta names its acknowledged revision, tick, and SHA-256 digest. A missing,
expired, or differently controlled baseline produces a full packet. The host and
SDK each retain two baselines of at most 512 KiB; edits have 4,096-operation and
24-component path limits. Full packets replace oversized deltas. Canonical JSON
normalizes signed zero before hashing. Malformed patches, digests, and reconstructed
state fail admission without acknowledging the candidate.

The duplex SDK admits one outstanding replaceable snapshot request. The worker
also bounds events and inventory independently and skips missed polling intervals;
reliable commands and event cursors keep their existing ordered semantics. TCP
still blocks later bytes behind earlier writes. This change bounds stale pose work
and does not claim different transport delivery guarantees. Relevance exit does
not retire a life; the replica retains generation and death fences across reentry.
Existing 512-entry generation-history bounds remain.

`Gateway::replication_stats` and `net::Exit::stats.replication` expose produced
full/delta counts, resyncs, encoded packet bytes, aggregate and peak encoding time,
maximum acknowledged-baseline age, and retained baseline bytes. Counts survive
connection closure; retained bytes drop when a connection closes or loses its
grant. These are packet-generation counters, not confirmed network delivery.

Shared public extraction and cell lists are cached until a host mutation or tick.
Per-viewer filtering still clones and scans bounded source lists. Large conservative
meshes can remain relevant to many viewers. Current entity admission limits and
real WAN loss, crowded battle budgets, and transport alternatives need separate
measurement. The retained replication fixture is in
`bench/verse/2026-10-04/spatial-replication/run.json`.

Wire version 23 retains an owned movement baseline with the exact capsule state,
yaw, life, control epoch, and applied sequence. Snapshots withhold it while
movement or jump input is pending, during cinematic/controller control, and
after death; spectators receive none. Client prediction and reconciliation use the shared capsule motor. The portable `prediction::History` bounds retained movement
to 64 intervals and 256 substeps, replays only unapplied input, and retires
history on a newer life or control epoch. Its snapshot observation ordering
allows multiple corrections within one server tick. Tracked worker inputs bind
their local token to the exact transmitted command and reject stale control.
Owned snapshots also carry bounded collision source geometry with exact collider
lives, layers, usage, and poses. Rebuilding preserves solid boxes, triangle
meshes, and capsules; spectators omit this prediction data. The native remote
client predicts owned movement and jump inputs before command
binding, advances the shared capsule at 120 Hz, and follows that pose with its
camera and locomotion. Accepted snapshots retire acknowledged inputs and replay
outstanding input; rejection and lifecycle changes retire estimates. Compiled
collision meshes survive pose-only updates. Current walking scale and jump
routing include primary difficult terrain, Telekinesis steering, and Levitate.
The deterministic delayed profile below covers movement corrections; hardware
latency and crowded-battle acceptance remain separate.
Movement inputs remain held for up to
60 physics substeps without refresh, rounded to an authority interval. Zero
input, control handoff, death, and respawn stop held movement. Baselines include
the physics-step watermark and held-input expiry. Version 17 checkpoints migrate
with no held input; version 18 retains leases for exact replay.

Native clients request `BeginMovementFrames` with the current life and epoch from
an unmodified, stationary grounded pose. Entry advances the epoch and anchors a
character clock to simulated world time. `MovementFrame` carries a complete
contiguous interval, with original movement-lease expiries, yaw changes, and
one-step jump edges. Each packet contains at most 12 substeps and 12 segments;
the authority queues at most 16 packets and processes at most 12 substeps per
world tick. It executes only intervals ending at or before simulated world time.
Time credit cannot create future travel. Casting, world timers, combat, and
receipts continue on the world clock; collision and spell modifiers use current
authoritative state rather than rewinding the world.

Movement envelopes allow an acknowledgment clock up to 12 world ticks old;
the character clock and life/epoch still bound effective movement time. The
worker reuses verified frame control for at most 400 ms instead of inserting
a snapshot round trip into the movement stream. Casts keep their existing
tick admission and use fresh control. A frame acknowledgment admits its envelope.
It does not confirm movement. The
owned baseline identifies `profile`, confirmed character `physics_step`, applied
frame sequence, and observed `world_step`. Prediction retires only the confirmed
prefix and replays original event times. The native adapter groups completed
steps into six-substep (50 ms) packets without dropping intervals or input changes;
local prediction still updates each rendered frame. Transmission waits when a
packet would exceed the latest authority `credit_step`, verified through a snapshot
or control acknowledgment, plus the existing 12-substep lookahead; local elapsed
time cannot grant credit during storage pauses.
The worker binds packets to the shared command sequence before transmission. Legacy CLI and headless movement retains the
arrival-time profile. Version-29 peers must upgrade both host and client; control
headers separate admitted body `world_step` from durable checkpoint `credit_step`, actual applied movement confirmations remain bounded by the admitted sequence; ability events require matching
event decoders.

The serial SDK retries an explicit `stale_tick` refusal only when its verified
control confirms that admission consumed no sequence, with at most three
envelopes inside the ten-second command deadline. It does not retry gameplay
refusals, future ticks, changed controls, or uncertain transport.

A character falling more than 32 substeps behind after ready work is considered
stale. The first interval has a separate 48-substep startup allowance, and
entry returns its initial snapshot directly. On expiry, its epoch advances,
queued frames and holds clear, and ordinary gravity resumes. Runtime diagnostics
count expiry across admission and both player tick paths, retaining the first 32
clock and queue samples with an omitted count; checkpoints exclude this evidence. Repeated entry
cannot renew this budget. Handoff, disconnect, restart,
respawn, and teleport fence the interval context. Teleport commands drain earlier
pipeline work. A refused interval stops the worker and requires reconnection;
uncertain transmission is never replayed. An explicit storage refusal consumes
no frame, and the serial SDK can retry that exact envelope through its bounded
storage-backpressure path. Snapshot recording counts reset reasons separately
from ordinary corrections and retains bounded discontinuity traces.

[`prediction::latency`](src/prediction/latency.rs) uses the actual authority and
local motor with 30 Hz world ticks, 120 Hz local steps, ordered delayed input and
acknowledgments, and 50 ms snapshot sampling. The
[retained deterministic receipt](../../bench/verse/2026-10-04/movement-intervals/run.json)
covers 0–167 ms nominal RTT, bounded jitter, combined intervals, starts/stops,
diagonals, jumps, walls, stairs, and a separate translating-support profile.
Static-profile correction p95 is below 0.10 m. Lifecycle regressions cover both
primary and secondary players, cast interruption, death/respawn, reconnect,
teleport, queued restore, and idle expiry. These checks do not establish real
network, crowd, GPU, input-to-display, or arbitrary moving-geometry acceptance.

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

Reverse Gravity (`src/reverse_gravity.rs`, slot Shift+9) is a concentration
cylinder field whose top band holds a hover spring; characters in it get a
zero-gravity override and exact upward motion, and ceiling strikes and the
final drop deal falling damage. C ends concentration.
Telekinesis (`src/telekinesis.rs`, row-two slot 0) grips an object with soft
joints to a kinematic hand, or a creature by driving its character toward the
hand with gravity suspended. T steers the hand along the camera ray; R releases it.
Gust of Wind (`spells::gust`, row-two slot 4) keeps its Line, save
clocks, and scene flames in the spell world. G re-aims the Line while concentration lasts. Creatures follow the SRD rule exactly, while
props and ordinary arrows feel quadratic drag (`gust`).

To add a spell: write `src/spells/<name>.rs` with its cast and a
`playground::Scenario`, add one `SpellDef` line to `spells::CATALOG` (its
reserved row-two slot), and one line to `playground::scenarios`. Record it with
`cargo run -p verse --features imported-desktop,remote-chamber --example verse_play --
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
uses temporary reward-history files and does not retain a recoverable world.

Durable network hosts normally group two simulation ticks into one owned
persistence copy. Bounded catch-up batches can contain more ticks; queue or
reward-history pressure forces an earlier flush.
Intervening world mutations remain uncommitted, and their reads and replies wait
for the same ordered durability fence.
One storage thread encodes changes, publishes reward-history nodes, appends and
synchronizes the journal, and periodically replaces the base snapshot. The
queue holds one active and one waiting copy; at most 128 replies wait in each
batch. Mutation replies and reads of new state wait for their ordered commit;
reads can share an existing commit or use already committed state. A failed
commit stops the host and withholds those replies. The copy contains no
connection challenges or dispatch interface, and later world mutations cannot
change it. Authenticated connections admit at most eight ordered requests while
durable replies are pending. A deferred read holds later admission until its
projection is captured; reply delivery still follows request order and commit
completion. Partial frame reads have a separate bounded reader.

When storage fills the queue or reward-history staging capacity, the host pauses
simulation and returns `storage_busy` for new requests using the last committed
control and tick. Ordered pipelined delivery uses the last delivered durable
control and tick for a storage refusal so its header cannot regress. Retry with
the same operation identity. Paused wall time is
counted separately and is not simulated later. Sequential client helpers retry
explicit storage refusals for up to ten seconds with the same operation fields;
raw requests and pipelined clients expose each refusal. Neither path retries
uncertain transport failures. Normal shutdown drains admitted
copies and commits parked controls before releasing the writer lock.

`chamber.json` retains the versioned, checksummed base snapshot;
`journal.jsonl` retains ordered structural changes with revision and state digest
chains. Compaction runs after 256 changes or 64 MiB of journal data. Recovery
replays complete records, discards an unterminated final append, and refuses
complete corruption or a broken chain. An interrupted compaction can leave a
validated journal prefix already covered by the snapshot; recovery discards
that prefix. Back up the snapshot, journal, and entire rewards directory together
while the host is stopped.

The host reports lifetime simulation, persistence capture, and commit latency
histograms with p50/p95/p99 bucket upper bounds, plus backlog, refusals, paused
time, committed bytes, and storage duration. Deferred read projection and running
checkpoint-copy timings separate those two costs within capture; they exclude
startup, shutdown, and reads dispatched outside the capture batch. `Store::commit` remains a synchronous
API for callers that manage their own scheduling. The original
[durable host fixture](../../bench/verse/2026-10-04/durable-host/run.json) records
the earlier synchronous implementation; the
[ordered durability fixture](../../bench/verse/2026-10-04/ordered-durability/run.json)
records the background writer's local checks and measurements.

Offline content updates use `service::persistence::migration` and the
`verse_migrate` example. Save version nine records character schema two separately
from the world rules revision and asset digest. Character ownership survives
revocation and re-enrollment; existing saves derive ownership from their retained
player grants. Legacy saves cannot reconstruct ownership already removed before
this schema existed. Characters remain local to the bounded instance roster.

1. Stop the host and retain both source and target configs and asset packs.
2. Generate and inspect a plan:

   ```sh
   cargo run -p verse --no-default-features --features remote-chamber \
     --example verse_migrate -- plan SOURCE.json TARGET.json > REVIEW.json
   ```

3. Apply that exact plan:

   ```sh
   cargo run -p verse --no-default-features --features remote-chamber \
     --example verse_migrate -- apply SOURCE.json TARGET.json REVIEW.json
   ```

4. Start the host with the target config, or roll back before starting it:

   ```sh
   cargo run -p verse --no-default-features --features remote-chamber \
     --example verse_migrate -- rollback TARGET.json MIGRATION_ID
   ```

The plan pins source revision and state, source and target asset/rules/schema
identities, complete config digests, candidate state, and enrollment changes.
Changed source state or inputs require a new plan. Planning validates a candidate
without publishing it; opening the store still performs ordinary crash recovery.
Both configs must retain the instance and storage directory. Known owners cannot
be replaced by another key. New keys receive new characters; revoked characters
retain their ownership, balances, and roster slot. Re-enrollment preserves the
actor and can explicitly change its authored spawn.

Apply retains XP, inventory, outfit, equipment, quest counters, accepted quest
baselines, completed quest status, and exact original receipts. It restarts world
dynamics and encounters, advances NPC/prop and player life fences, respawns
players, and refills resources to the target equipment limits. Existing catalog
IDs and equipment slots must remain. Changing an active quest's objective,
giver, prerequisites, or increasing its goal requires a separate progress adapter
and is refused. Completed quest retries return their original reward, even when
its definition changes. Unsupported rules and schema revisions remain refused;
this workflow does not invent adapters for arbitrary future revisions.

Each operation retains `migrations/MIGRATION_ID/{before,after,record,seal}.json`
beside the shared immutable receipt history. Back up that directory, the snapshot,
journal, and rewards together. An exclusive writer lock covers plan, apply,
rollback, and recovery. A synced marker precedes snapshot replacement; interrupted
operations recover the source until their target seal is synced. A sealed
operation recovers its target. Storage errors withhold success and may require
restoring storage availability before recovery. Rollback uses the same protocol,
records a new commit revision, and refuses any later commit, including host
startup. Backups never grant permission to discard acknowledged later progress.
Keep source packs available for rollback; migration does not rewrite asset files.

Trusted hosts use `Gateway::grant_reward` for bounded character experience,
item stacks, and quest counters. A stable source ID binds one exact transaction
per character; retries return its original receipt, and conflicting reuse or
limit failures leave every field unchanged. Call `Store::commit` before
acknowledging a host-created reward. Legacy chamber saves replay the retained
transactions; version-one saves upgrade with an empty ledger. Versions eight
and nine retain bounded character state, up to 128 active receipts, and the root of
an immutable indexed history under `state_dir/rewards`. Recover them through
`Store::open` and back up that directory with `chamber.json` and `journal.jsonl`; a checkpoint that
references archived receipts requires those files. Exact retries preserve the
original revision even after archival. Instance reset retains rewards and retry
identities. Lifetime transaction count has no 4,096-receipt limit. Standalone
chambers retain in-memory history until attached to a store or network host.
Host JSON accepts an optional sorted `rewards` array of NPC targets and grants,
for example `[{"target":2,"experience":45,"items":[{"id":1,"count":1}],"quests":[{"id":1,"count":1}]}]`.
The cooperative version-one policy grants every enrolled adventurer, including
disconnected or defeated residents, once per defeated NPC life. This policy does
not consult persistent party membership. Spectators receive
none. Respawns and instance resets create new NPC life generations; saved source
IDs and the event cursor prevent duplicate rewards after recovery. Changed reward
policies are refused on recovery. With no configured rewards, combat grants none.
An authority or storage failure stops the host and withholds uncommitted replies,
including when an entire cooperative reward batch cannot fit. Recovery can retain
a synchronized operation whose reply was lost; reuse its original identity.

Wire version thirteen provides an authenticated `inventory` read. The connection determines
the character; request bodies contain no actor or grant amounts. Players can read
their experience, bounded item stacks, and quest counters while dead. Spectators
are refused. `service::client::Client::inventory` validates the owned life, counts,
and nonregressing transaction revision. The [combat reward receipt](../../bench/verse/2026-10-04/combat-rewards/run.json)
retains real loopback TLS spell/reward/restart assertions and their synthetic
fixture limits. The bounded class, level, and equipment derivation is described
in [Party membership, progression, and trades](#party-membership-progression-and-trades).

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
The host unlocks a quest only after that character claims every prerequisite.
Realm character receipt books preserve these completions across instances. Kill reward counters supply objectives, including counters
earned before unlocking; claims select only the quest ID under the current owned life
and control epoch. The host checks availability and completion and applies configured rewards
once per character and quest cycle, including across respawn, reset, and recovery.
Durable hosts commit before acknowledging claims. Saved progression configuration
is immutable on recovery; earlier saves upgrade with no configured quests and
level one. Level thresholds also derive bounded class and equipment resource limits.
Authored repeatable quests and cycle operations are described in
[Party membership, progression, and trades](#party-membership-progression-and-trades).
Wire version fifteen introduced quest availability. Locked quests show their status
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
The first campaign retains its original enrollment behavior. Realm game services
add abandonment and repeatable quest cycles; authored friendly roles and giver
dialogue are available. These do not provide a general dialogue editor.
Remote view admission allows giver life and interaction availability to change
without a reward transaction. Other quest fields remain bound to the ledger
revision. Giver generation checks survive unavailable intervals and observe
newer replicated lives; conflicting giver metadata or stale generations refuse
the whole update. The [giver view receipt](../../bench/verse/2026-10-04/giver-view/run.json)
retains a TLS movement check at unchanged ledger revision zero.
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
capture. The original head/main-hand slice is described below. Realm game
services extend slots, item identity, and damage/armor modifiers; full commerce
UI and broader device interaction acceptance remain limited.

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
Realm game services add authored damage and armor modifiers and progression
scaling. Native attachments consume each parent's
final blended and grounded palette through the portable leaf-mount contract.

## Realm instances and transfer

With `service-net`, `service::realm::Realm` owns independent game instances under
one exclusive filesystem writer lock. A sealed manifest binds each instance's
content, immutable checkpoint, capacity, phase, endpoint, and authority epoch to
its registered character placements. Creation, admission, draining, stop,
restart, endpoint changes, and explicit 30-second leases use trusted host APIs.
Every tick and dispatch checks the lease. Recovery parks connections, revokes
leases, and advances authority epochs. Supply nondecreasing host milliseconds;
the TLS realm adapter uses Unix epoch milliseconds and stops if the clock
regresses or an expired lease cannot renew.

`Realm::transfer` moves a living additional adventurer between compatible
item, outfit, equipment, and progression catalogs. The host selects a destination
and collision-checked spawn. One atomic manifest publication selects both world
checkpoints, the character placement, and the immutable transfer retry index.
Retain the nonzero 16-byte operation ID across uncertain results. Exact retries
return the original transfer; changed arguments are refused. Health, mana,
equipment, progression, inventory, and remaining cooldowns survive. Temporary
world effects and input stop. The transferred character loses its old connection;
unrelated players and spectators retain their sessions. Their replication
baselines resynchronize with increasing revision counters. Character receipt books preserve original
mutation outcomes across local actor changes and repeated transfers; an old item
use cannot debit or heal again. Save version 12, character schema 4, retains these
books, the public guest policy, and quest cycles. Versions 1–11 remain readable
under their original schema rules; legacy resource limits upgrade without healing.

`service::realm::net::serve` hosts prebound TLS listeners with the existing wire
protocol and SDK. A separate coordinator thread owns storage and all games; TLS
workers hold no mutable authority. It bounds sockets and dispatch work to 128,
coalesces timer work, and records skipped time. Its local `Control` handle routes,
admits, drains, and transfers characters; client bodies cannot invoke those
operator actions. Run creation and lifecycle APIs before serving. This initial
adapter serializes durability work and is not accepted for a crowded 30 Hz battle.

The realm has 32 instance slots and 2,048 resident placements, with a 64-player
resident limit per game. Dormant characters and accounts use an immutable registry
outside the resident table. Primary characters can transfer; their authored
scene templates remain available for later character entry. Existing foreign
reward-history directories require an explicit import workflow. Distributed
failover, seamless world simulation, history garbage collection, and storage
throughput acceptance remain. The [realm transfer receipt](../../bench/verse/2026-10-04/realm-transfer/run.json)
records two actual TLS instances, five process-death boundaries, and the tested
limits.


## Persistent accounts and character lifecycle

Realm manifest version 2 selects a SHA-addressed registry of accounts,
credentials, and characters. Account and character IDs are independent of keys,
instance actor IDs, life generations, connection controllers, and render actors.
An account owns up to eight characters and has at most one resident character.
`Realm::admit` creates an initial account and character. After logout,
`create_character` creates another owned character; `resume` selects an existing
one. IDs do not change when a transient actor slot is reused. Retired slots retain
fresh generation fences without accumulating a body record per past character.
Public guest admission creates an account and character only after signature
verification. Guest caps count resident guests; logout releases that capacity.
Known dormant accounts select their existing characters, and retired credentials
cannot reenter through public admission. The guest policy survives realm restart.
Version-one realm heads upgrade without changing their character or receipt IDs.
Legacy scene origins remain readable in older checkpoints; serialization writes
`origin` in the current format.

`Realm::logout` saves the character in an immutable checkpoint, retires its
resident actor and authority, and frees placement capacity in one sealed head.
Inventory, progression, equipment, appearance, resources, and remaining cooldowns
remain owned by its character receipt book. A dormant character receives no NPC
rewards, mana regeneration, or cooldown progress. Logout ends temporary effects
and casts aimed at the retired life. A defeated character can log out and resume
with zero health; the existing controlled respawn contract then applies. Abrupt
transport closure parks a resident rather than implicitly logging it out. The
operator must apply its chosen disconnect retirement policy through `logout`.

Wire version 25 adds authenticated account metadata, character selection, and
explicit logout. `Client::account` returns only the authenticated account's owned
character IDs and epoch.
Known accounts without a resident in the destination authenticate as spectators.
`Client::select_character` admits only an owned dormant character, at one of 32
bounded entry points around the authored spawn; clients do not choose coordinates.
`Client::logout` requires its current life and control epoch, commits retirement,
and ends the session after a `LoggedOut` acknowledgment. Selection cannot replace
an active character implicitly. Refused selection retains spectator observation.
A retry after an uncertain resume authenticates against the selected resident;
transport failures never trigger automatic economic replay.

`recover_account` is a trusted operator operation with an expected account epoch,
a fresh credential, and current realm leases. The local realm control channel
exposes it; no client body can recover an account. Recovery preserves all character
IDs and books, fences the old sessions, increments the account epoch, and retires
the old credential permanently. Dormant characters remain selectable by the new
key. A stale recovery epoch is refused; read back the account after an uncertain
acknowledgment through the local control channel's `account` read. An uncertain
storage failure poisons the coordinator; reopen it before trusting readback.
This API does not decide who qualifies for human account recovery.

Authored combat reward policies select `enrolled_residents` (the retained legacy
cooperative default), `connected`, or `connected_within` with a 1–256-meter radius.
Connected modes exclude disconnected residents; the radius also excludes distant
participants. Rewards are private per-character copies with retained retry
receipts. This policy supplies no scarce shared drop, contribution ranking,
auction, or player-to-player trade. See the
[V09 audit](../../docs/audits/2026-10-04-verse-engine-audit.md#v09-persistent-accounts-and-characters-have-a-recovery-contract).

Registry nodes hold at most eight records and 128 KiB; lookup follows at most 64
hexadecimal key digits. Startup validates current resident bindings; dormant
snapshots validate on resume. Each dormant record pins an original content-bound
checkpoint, limited to 8 MiB, and shared reward history. Changed character
catalogs require migration. Whole-world archival checkpoints, growing immutable
storage, and serialized commit cost still need operating budgets and retention
work. These limits do not establish AAA population or throughput acceptance.

## Party membership, progression, and trades

Wire version 30 exposes `Client::services` and `Client::service_action` over an
authenticated realm connection. Every request names the expected stable character;
the server checks the current account, resident actor, and connection. Mutations
also name the realm and a nonzero 16-byte operation ID. Retain that ID and the
same action across an uncertain result. Immutable receipts return the original
outcome, including after transfer and restart; changed actions are refused.

A character can join one party of eight members and one guild of 64 members.
Leaders invite and remove members; invited characters join or decline. Leaving
transfers leadership to the lowest remaining character ID. Membership confers
no chat, world-control, or Studio permission. Private service reads expose only
the admitted character's memberships, invitations, items, and pending offers.

`Materialize` assigns identity to one already-owned gear unit. Each identity pins
its gear definition digest, stable owner, and version. Consumables and outfits
remain counted items. A trade names sorted item IDs and their expected versions,
with up to eight items per side and a deadline of at most five minutes. The offer
locks its sender's items; acceptance by the named recipient checks both sides
and matching destination definitions. One realm publication selects both reward
ledgers, ownership changes, cleared locks, and the retry receipt. Changed or
equipped last units refuse the whole exchange. Either participant can cancel,
including after expiry or a requested item's transfer. Characters retain at most
64 item identities, 16 invitations, and 16 pending offers each.

The local host's `Control::party_loot` publishes an authored outcome through the
existing reward ledger. The first nonzero event ID freezes the party's resident
recipients and amounts. Retries cannot include newly joined members or award a
transferred character again. A grant has at most eight item and objective entries;
dormant characters receive no new party loot. This trusted adapter does not give
clients authority to invent combat outcomes.

`Client::quest_cycle` and the worker's `Input::QuestCycle` capture an expected
cycle with each accept, claim, abandon, or reset. Abandon advances the cycle;
giver quests require a new acceptance, and auto-active quests start a fresh window. An authored `repeatable` quest can reset after
completion; reset starts a fresh objective window immediately. Earlier completions
still satisfy prerequisites. Old claims return their original receipt and cannot
claim a later window. Authored class health and mana gain 10 health and one mana
per level after the first, plus owned equipped gear, capped at 600 and 60.
Derivation preserves wounds and current mana; ordinary progression does not heal.
The character's authored spell catalog and save difficulty remain its class values.

These are engine and SDK operations. Crafting, auctions, mail, matchmaking,
reputation, a complete commerce UI, and automatic party combat attribution remain
outside this profile. Existing single-cycle UI actions continue to name cycle zero.

## Hosted social profiles

`play::Game::social_in` creates a closed social profile without hostile actors.
`play::social::Profile` revision 1 admits Plaza and Everglade variants with
bounded static boxes/terrain and seat/switch objects. Combat requests are refused.
Movement, social interactions, and replication use the same authenticated game
and control fences. `host::Config::social_profile` selects this profile;
`bind_content` includes its digest in the scene/asset identity. Startup recovery
refuses changed profiles. Host, CLI, and offline migration preparation use this
binding. Supported older combat checkpoints remain readable under rules v24;
wire clients must match the generated
[current wire version](../../docs/verse/runtime-contract.json).

Seats are exclusive and tied to a character life. Accepted interactions stop
queued movement and advance its control epoch. Movement and control retirement
release claims. Only a trusted host can publish typed public Studio poses,
including through the realm's lease-checked local control channel. Those values
contain no task text or work permissions; no world request controls Studio work.

With Verse's `remote-chamber` feature, `verse::hosted::Client::attach` connects
an already authenticated client and verified profile to `WorldRuntime`.
Destination admission pins the instance and profile digest. Render poses and
interaction state come from admitted snapshots; local movement and zone/Studio
operations are refused. Presence owners use `Session::tick_world` to retain
relay discovery while suppressing publisher poses during hosted play.

These are opt-in hosted variants with neutral shared geometry and generic figures.
Local asset packs keep their existing rules; Ruins, Lagrange, and Lab are refused
as hosted profiles. Existing realm instance and concurrent resident limits remain. The [social authority receipt](../../bench/verse/2026-10-04/social-authority/run.json)
records two-viewer convergence, transfer, recovery, and native projection checks.

Owned wire controls can include a bounded `applied_movement` confirmation.
The host supplies actual completed interval movement from the response's durable
fence, with an applied sequence no greater than its original admission prefix.
Admission acknowledgments alone do not confirm travel. Clients verify the life,
epoch, sequence, and world clock, retire confirmed prediction history, and retain
newer confirmed travel when a scene body represents an earlier prefix. Runtime
confirmation histories contain at most sixteen entries per actor and are omitted
from saves; reconnects establish a fresh baseline.

Interval controls retain the body's admission `world_step` separately from
completed durable `credit_step`. Credit renews permission without confirming
travel or advancing local elapsed time. Prediction retains at most 257 motor
states and 256 deferred physics steps. Grounded horizontal confirmations compare
the estimate at the confirmed step; changed blockers intersecting the retained
path or correction region, forces, vertical motion, or movement policy require
full replay. When unchanged walls obstruct the correction,
each retained motor state receives a collision-constrained fixed-scene adjustment.
Projected capsule changes affect future
integration. Blocking overlap holds the estimate until authority resolves it;
query errors and truncation still propagate.

Live game frames retain authority time after the authored cinematic ends, so
owned HUD and hostile-effect validation continue during long sessions. The
cinematic director itself retains its bounded playback duration.

Frame-profile input enters at the current prediction clock boundary and affects
future integration without replaying already processed travel against newer
crowd poses. Changed authority motor state, nearby fixed geometry, or retired
past input still requires reconciliation. An existing capsule overlap permits only
straight grounded motion that separates every initial contact, sweeps other
colliders, does not increase original penetration, and retains the same fixed
floor support. Inward movement, jumps, forces, floor loss, and opposing contacts
retain their prior handling. Past deferred steps stay deferred.

Local render values distinguish the prediction control epoch from authority and
authored animation clocks. A clock-owner handoff starts marker delivery at the
new phase without catching up another source's history. Gameplay snapshots and
control authority remain unchanged; marker budgets and atomic refusals still apply.

The `service-net` operator monitor publishes bounded latest-value diagnostics and
requests the existing ordered drain. The dedicated
[host operations commands](../verse-host/README.md#local-operations) expose it
through owner-local IPC. `service::persistence::backup` verifies and restores
chamber checkpoints, reachable reward history, and migration archives into new
storage. Offline pruning retains every current and migration history root.

## Account safety in a realm

Wire version 31 adds `Safety` and `SafetyAction`. The realm derives the acting
account from the authenticated connection; clients cannot select the owner of
its safety preferences. The private view also maps up to 128 resident avatars
in the world the connection can observe to public character and account
addresses. Clients target blocks and reports through those addresses; the
projection carries no credential key, epoch, inventory, or Studio data.
`service::safety` defines account blocks, typed reports,
private projections, and exact-retry receipts. Each account can retain 64
blocked accounts, with 128 block changes per UTC day. A block in either direction refuses new party or guild
invitations, joining a pending invitation from that leader, new gear trade
offers, and accepting pending trades. Cancellation remains available to release
locks. The policy follows every character of both accounts and survives
restart and credential recovery. Existing shared-world avatars and group
membership remain visible; blocks grant no simulation or Studio authority.

Reports carry `spam`, `harassment`, `unsafe_content`, or `cheating`, plus an
optional nonzero evidence digest. They carry no free text or private Studio
payload. Submission is limited to 16 reports per account per UTC day and a
realm-wide queue of 256 open reports. The public response contains only the
submission receipt. `Realm::pending_reports` and `resolve_report`, also exposed
through the trusted local `realm::net::Control`, provide operator-only review.
Completing a report releases queue capacity and retains the immutable receipt;
retrying the old submission cannot reopen it. Record selection and receipts use
the existing atomic realm head. Uncertain publication requires recovery. Active
queues and submission rates are bounded; immutable receipt history remains on
disk and requires the host's storage and retention policy.

These are native realm and SDK contact controls. They do not hide physical
avatars, remove existing members, implement a chat service, or add a phone or
browser moderation UI. Host operators decide what an actioned report requires;
report status alone does not ban an account or run a tool. World-only access
continues to expose seats while refusing private Studio views and operations.

## Operational input replay

`replay::Recorder` owns a game in a bounded local diagnostic profile. Its
operations call the existing command admission, handoff, join, social,
respawn, and movement-interval methods. Elapsed time uses the native host's
`FixedSchedule` at 30 Hz with a three-step catch-up cap. A record compares every
actual authority tick, including each tick inside a catch-up batch. Rule
refusals retain the real admission fences and state changes.

`Execution::native` hashes the executable and binds the compiler, target,
Cargo features, optimization, debug settings, target features, and Rust flags
captured by `build.rs`. The operator supplies content and configuration digests
and a declared OS/CPU/runtime profile. `Execution::declared` accepts an artifact
digest for portable adapters. Replay requires an independently trusted expected
profile with exact equality. These declarations and hashes are not attestation,
and no cross-build or cross-architecture equivalence is claimed.

A segment binds its canonical initial checkpoint and complete SplitMix64 dice
state, including forced rolls and per-caster streams. Ordered records carry
outcomes, explicit admission-sequence consumption, checkpoint state, host clock, authority and physics ticks, logical
commit revisions, and a hash chain. `Commit` pins an in-memory checkpoint;
`Restore` loads it and resets the host elapsed-time clock. `Shutdown` fences
controllers, commits, and closes the segment. Only a complete closed segment
can be replayed. `Trace::write_new` syncs a private file and, on Unix, its parent
directory; partial or altered files are refused. The trace digest must remain
in trusted retained evidence because a hash chain alone does not authenticate
its author.

Segments permit 512 records and 64 MiB, reserving room for shutdown. Recording
errors leave the authoritative checkpoint unchanged. Rotate diagnostic windows rather than
using this full-state format as an unbounded production event log. Replay
reports the first differing sequence, tick, and JSON field. Presentation
trajectories and client prediction histories remain separate formats.

The measured profile is native Linux x86-64 with the pinned compiler and exact
fixture executable. This SDK does not automatically record the TLS/REACH loop,
realm accounts, economy transactions, storage-worker completions, or Studio
operations. Those adapters must retain their own authority and durability
records; this world's logical commit marker is not a realm durable revision.

Wire version 32 allows large chamber frames to use lossless DEFLATE compression.
The high bit of the big-endian length marks compression; the payload starts with
the decoded u32 length. Both lengths retain the existing message byte limit,
and decoding refuses expansion beyond the declared length. Frames smaller than
4 KiB remain raw; larger frames compress only when they save at least one eighth
of their bytes. Movement authority and ordered durable replies are unchanged.

Wire version 33 adds an owned `MovementCredit` read after interval entry. Its
small reply grants time from a completed durable checkpoint without a new scene
projection or a movement acknowledgment. With no outstanding replies, the host
returns its committed credit immediately if it preserves the delivered control
prefix; otherwise, it waits for the normal durable fence. The native session preserves pending
inputs when consuming this credit. Reads cannot renew the server's movement
clock, increase its lead allowance, or cross a character life or control epoch.
