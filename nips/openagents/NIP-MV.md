# NIP-MV — Metaverse

`draft` `optional` — v1.

This NIP defines how clients share presence in a 3D world over Nostr:
where each participant's entities stand, which way they face, what they are
doing, and where they were when they left. High-rate motion uses ephemeral
events that relays forward and do not store. Durable state uses addressable
events that relays keep, so a world persists across sessions the way an
MMORPG world does.

The base presence protocol does not define rendering, physics, collision, or
world geometry. Two clients can show the same entities with different
renderers. The optional [scene manifest profile](#scene-manifest-profile)
below binds a reviewed scene and supported simulation profiles without
shipping executable behavior in a pose event. That profile is **Designed**;
the current Verse client uses a curated local zone catalog and does not yet
discover or admit signed scene definitions.

Test-time capabilities: [Gym notes](#gym-notes) share a trainer's published eval result in Verse, the last stage of the [capability flywheel](../../docs/essays/2026-09-29-test-time-capabilities.md#11-the-capability-flywheel) ([mapping](../../docs/essays/2026-09-29-test-time-capabilities.md#how-the-protocol-carries-test-time-capabilities)).

## Terms

| Term | Meaning |
| --- | --- |
| World | A shared coordinate space, named by a world identifier. |
| Entity | One thing a publisher places in a world: a user's avatar, an agent the user controls, an object. An entity is identified by its publisher's pubkey plus an entity id unique to that publisher. |
| Pose | An entity's position, orientation, and optional velocity at one instant. |
| Frame | One ephemeral event carrying the current poses of one or more of a publisher's entities. |
| State | The durable, last-known record of one entity. |
| Cell | A square area of the world floor used to scope subscriptions. |
| Loaded zone | A separately loaded scene with its own world identity, assets, presentation, and supported simulation profiles. This differs from a chat district identified by a `z` tag inside one world. |
| Scene manifest | A content-pinned description of the scene and the host-supported formats and profiles it requires. A signature identifies its publisher; it does not authorize code execution. |
| Shared body | An object the world defines, such as a ball, that any participant may move. Its id names the same object whoever publishes it. See [Shared bodies](#shared-bodies). |
| Stamp | A shared body's authority: `[epoch, rev]` plus the pubkey that stamped it. |

## Kinds

| Kind | Class | Record |
| --- | --- | --- |
| `33300` | Addressable | World definition |
| `33301` | Addressable | Entity state |
| `23300` | Ephemeral | Pose frame |
| `23301` | Ephemeral | Gesture |
| `23302` | Ephemeral | Zone command |

Kind allocations are draft assignments, not upstream registrations.

## World identifier

Every event in this NIP carries exactly one `w` tag naming its world:

```json
["w", "<world identifier>"]
```

A world identifier is either the address of a `33300` world definition,
`33300:<pubkey>:<d>`, or an opaque string of at most 128 bytes agreed out of
band. Clients MUST treat two different identifiers as two different worlds.

## Coordinates

- Units are meters.
- The frame is right-handed with `+Y` up. The ground plane is `XZ`.
- An entity at rest faces `+Z` in its own frame. Its yaw is counterclockwise
  around `+Y`.
- A position is `[x, y, z]`. An orientation is a unit quaternion
  `[x, y, z, w]`. A velocity is `[vx, vy, vz]` in meters per second.
- Numbers MUST be finite. A receiver MUST normalize a quaternion and MUST
  discard a pose whose quaternion has near-zero length.

A world definition MAY override the defaults above only by stating so in
its content. A client that does not support the stated frame MUST NOT
render the world.

## Cells

A cell is a square of side `cell` meters on the ground plane. The cell of a
position is `[floor(x / cell), floor(z / cell)]`, written as the tag value
`"<cx>,<cz>"`, for example `"-2,5"`. The default cell size is 64 meters.

Pose frames and entity states SHOULD carry one `c` tag per cell their
entities occupy:

```json
["c", "0,-1"]
```

A client that only needs its surroundings subscribes to the cells around
it with `#c`. A client that needs the whole world subscribes with `#w`
alone.

## World definition — kind `33300`

An optional description of a world, addressable by `d`.

```json
{
  "kind": 33300,
  "tags": [
    ["d", "plaza"],
    ["w", "33300:<pubkey>:plaza"],
    ["name", "The Plaza"]
  ],
  "content": "{\"v\":1,\"cell\":64,\"bounds\":{\"min\":[-264,0,-264],\"max\":[264,200,264]},\"spawn\":{\"center\":[0,0,0],\"radius\":28}}"
}
```

| Content field | Meaning |
| --- | --- |
| `v` | Content version. This NIP defines `1`. |
| `cell` | Cell size in meters. Default `64`. |
| `bounds` | Optional axis-aligned bounds, `min` and `max` positions. |
| `spawn` | Optional spawn disc: a `center` position and a `radius` in meters. |
| `scene` | Optional scene manifest profile, defined below. Omitted for presence-only definitions. |

The world's `w` tag names its own address.

### Scene manifest profile

**Status: Designed.** This optional profile allocates no new kinds. It uses
`33300` for discovery and exact content identities for an admitted scene.
Clients that implement the profile MUST apply the following requirements.
Clients that implement only presence MAY ignore `scene`, but MUST NOT claim
that they implement the scene's declared geometry or game rules.

The `scene` object has this shape:

```json
{
  "v": "openagents.mv.scene.v1",
  "manifest": {
    "schema": "verse.zone.v1",
    "sha256": "<64 lowercase hexadecimal characters>",
    "bytes": 512,
    "media_type": "application/json",
    "urls": ["https://world.example/forest.json"]
  }
}
```

This reference identifies the exact manifest bytes, not JSON reserialized by
the receiver. `bytes` is a positive integer. Clients MUST bound manifest bytes,
locator count, and locator length before allocation or transfer, verify the
length and SHA-256 before parsing, and reject unknown required schemas or
profile versions. URLs locate the bytes; they are not the content identity or
an authorization to access a host. [NIP-94](../official/94.md) file metadata
events MAY supply alternative locators whose hash, size, and media type match
the admitted reference. A mutable listing or a redirect cannot change the pin.

Before entering, the client pins the world-definition event ID, its verified
signer and address, and the scene manifest digest. The addressable `33300`
record is a discovery head: a newer record does not silently replace a scene
already in use. A transition or update that changes these pins requires a new
admission. Receivers MUST reject disagreement between the signed definition's
world identity and the manifest's declared world binding. Display names are
not identity, and identical names do not merge worlds.

A supported manifest schema MUST make these concerns explicit, either through
its own fields or exact, reviewed host profile IDs:

- Geometry, bounds, coordinates, collision semantics, safe arrivals, and exits.
- Asset identities, exact sizes, media and decode formats, provenance, license
  notices, and limits on decoded vertices, images, animation, and GPU resources.
- Presentation values such as palette, lighting, and fog. A shared app palette
  does not constrain the appearance of every world.
- Physics and game rules as separate, versioned profiles, with declared
  feature coverage. Selecting a fifth-edition encounter profile does not
  imply a complete tabletop rules engine; selecting a space scene does not
  imply an orbital dynamics implementation.
- The authority model: local-only simulation or a separately supported shared
  authority. Pose frames alone do not authorize combat, editing, ownership,
  inventory transfer, or results accepted by another participant.

Verse's earlier `verse.zone.v1` schema was intentionally narrower than a
general authoring format. It had exactly `schema`, `world`, `ruleset`,
`physics`, `asset_sha256`, and `asset_bytes`. It admitted one reviewed Ruins
world, which Verse removed on 2026-10-05; no current zone uses the schema.
Zone palettes, bounds, and arrivals are compiled host values. A local world
name is not a published `33300` address, so this local
catalog does not yet satisfy signed scene admission. A future authoring schema
needs its own version and validation; adding fields to a permissive JSON blob
does not implement this profile.

#### Loading and transitions

Discovery, seeing a portal, receiving a pose, or reading a preview MUST NOT
automatically download heavy scene assets. The client starts asset loading
after an explicit entry or an already granted loading policy. It MUST enforce
transfer, decode, concurrency, cache, and memory bounds independently of values
requested by the publisher. The receiver verifies content before decoding and
admits only formats and implementations it supports. A declared rules profile
cannot install native code, scripts, or shaders.

Clients retain a safe source state while preparing the destination. Failed or
cancelled admission MUST leave a usable source scene or an explicit recovery
surface, not publish a half-loaded destination. Late work from a cancelled
generation MUST NOT replace the current scene. On successful replacement,
clients release unneeded decoded assets and GPU resources; a bounded disk cache
may retain verified immutable bytes. A cache hit does not bypass validation.

Each separately loaded coordinate space uses a distinct `w` world identifier.
The chat `z` district tag cannot distinguish unrelated coordinate spaces.
When leaving a shared world, the client SHOULD publish its final/offline state,
then stop its old motion stream, close old subscriptions, and clear world-bound
remote entities. On joining another shared world, it uses the new world ID and
the admitted definition's bounds and arrival rules. Zone coordinates MUST
NOT appear in a plaza pose frame. Failed delivery of an offline state remains
possible, so peers still expire stale presence.

Entry into a different relay additionally requires its connection and
disclosure policy; a portal cannot silently authorize a new recipient or widen
existing permissions. A local-only destination suspends the source world's
publishing and observation and does not present its local actors as shared
entities. Returning explicitly restores the source scene and its connection
policy. Pairing grants, Gym launch authority, and paid operations remain
separate from travel.

[NIP-EXT](NIP-EXT.md) MAY distribute pinned schema and scene-definition files
inside reviewed packages. Its host component-set admission remains separate;
a release signature does not permit an arbitrary executable rules engine.
No new EXT component kind or scene execution right is allocated here. See
[Verse zones](../../docs/verse/zones.md) for the implemented local slice and
[zone rules](../../docs/verse/zone-rules.md) for its limited encounter profile.

## Entity state — kind `33301`

The durable record of one entity. Relays store the latest per publisher and
`d`, so a client that connects later can show where every entity was left.

```json
{
  "kind": 33301,
  "tags": [
    ["d", "<world identifier>/avatar"],
    ["w", "<world identifier>"],
    ["c", "0,-1"],
    ["role", "avatar"]
  ],
  "content": "{\"v\":1,\"id\":\"avatar\",\"role\":\"avatar\",\"p\":[3.2,0,-10.5],\"q\":[0,0.38,0,0.92],\"t\":1790000000000,\"online\":true}"
}
```

- `d` MUST be the world identifier, a `/`, and the entity id. An entity id
  is 1 to 64 bytes of `[a-z0-9_-]`.
- `role` names what the entity is. This NIP defines `avatar` (the
  publisher's own presence), `agent` (an autonomous entity the publisher
  controls), `object`, `body` (a [shared body](#shared-bodies), in pose frames
  only), and `bodies` (a shared-body snapshot). Clients MUST accept unknown
  roles and MAY skip drawing them.

| Content field | Meaning |
| --- | --- |
| `v` | Content version, `1`. |
| `id` | The entity id, equal to the part of `d` after the `/`. |
| `role` | Equal to the `role` tag. |
| `p`, `q` | Last known position and orientation. |
| `t` | Publisher time of this state, in milliseconds since the Unix epoch. |
| `online` | `true` while the publisher is streaming frames for this entity, `false` after it leaves. |
| `follows` | Optional entity id of the same publisher that this entity accompanies, such as an agent that follows its owner's avatar. |
| `name` | Optional display name. |
| `set` | A `bodies` snapshot's body set. Required for `bodies`, absent otherwise. |
| `b` | A `bodies` snapshot's rest poses. Required for `bodies`, absent otherwise. |

### Object state

A world whose places form a world tree (Verse's
`openagents.verse-world-tree.v1`) derives the tree on every device from
its pinned layout, so only what changes is shared. A world authority
publishes a stateful object, such as a lamp, a door, a workstation, or a
task board, as an entity state with role `object` when its state changes.
Its content carries three more fields, which other clients ignore:

| Content field | Meaning |
| --- | --- |
| `node` | The object's node ID in the world tree. |
| `tree` | The digest of the tree the ID names. |
| `state` | The object's state: `{"kind":"lamp","lit":true}`, `{"kind":"door","open":false}`, `{"kind":"workstation","busy":true,"by":"ada"}`, or `{"kind":"task-wall","columns":[{"name":"PLANNED","count":2}]}`. |

The entity id is `obj-` and the first 20 hex digits of the SHA-256 of
`node`, since a node ID has slashes; a reader refuses an object whose id
isn't its node's. `p` is the node's standing point. `verse_net::mv::object`
encodes and decodes it.

A publisher SHOULD write entity state when an entity joins, when it comes
to rest after moving, at most every few seconds while it moves, and when it
leaves (with `online: false`). A publisher SHOULD NOT write entity state at
frame rate; motion belongs in pose frames.

To resume a session, a client reads its own entity states for the world
(`authors` = its pubkey, `#d` = the entity addresses) and places its
entities where the states say.

## Pose frame — kind `23300`

An ephemeral event carrying the current poses of a publisher's entities in
one world. Relays forward frames to matching subscriptions and do not store
them.

```json
{
  "kind": 23300,
  "tags": [
    ["w", "<world identifier>"],
    ["c", "0,-1"]
  ],
  "content": "{\"v\":1,\"s\":\"a41c\",\"n\":812,\"t\":1790000000123,\"e\":[{\"id\":\"avatar\",\"role\":\"avatar\",\"p\":[3.2,0,-10.5],\"q\":[0,0.38,0,0.92],\"v\":[0,0,6.4]},{\"id\":\"agent\",\"role\":\"agent\",\"follows\":\"avatar\",\"p\":[4.1,2.3,-11.6],\"q\":[0.05,0.37,0.02,0.93]}]}"
}
```

| Content field | Meaning |
| --- | --- |
| `v` | Content version, `1`. |
| `s` | Session id: 1 to 16 bytes, random per client session. |
| `n` | Sequence number, increasing by at least one per frame within a session. |
| `t` | Publisher time in milliseconds since the Unix epoch. |
| `e` | Array of 1 to 16 entity poses. |

Each entry in `e` has `id`, `role`, `p`, and `q`, and MAY have `v`
(velocity), `follows`, and `a` (a short animation state such as `idle`,
`walk`, `run`, `jump`). An entry with role `body` reports a
[shared body](#shared-bodies): it MUST have `k` and MAY have `w` and `r`.

| Entry field | Meaning |
| --- | --- |
| `w` | A shared body's angular velocity, world frame, rad/s. |
| `k` | A shared body's stamp, `[epoch, rev]`. The publisher is the owner. Required for role `body` and forbidden for other roles. |
| `r` | `true` when the shared body came to rest at this pose. Omitted otherwise. |

Rules:

- `created_at` has one-second resolution, so receivers order frames by
  `(s, n)` and MUST drop a frame whose `n` is not greater than the last one
  seen for that publisher and session.
- Receivers SHOULD render remote entities slightly in the past, about 100 to
  200 milliseconds, and interpolate between frames: linear interpolation for
  position and spherical interpolation for orientation.
- A publisher SHOULD send frames only while something changes, at a rate
  suited to the motion (for example 10 per second while moving), and a
  keepalive frame at most every few seconds while at rest.
- A receiver SHOULD treat an entity as offline when no frame has arrived
  for about 10 seconds, and fall back to its entity state.
- A frame MUST NOT be used as durable state. A client that joins late learns
  positions from entity states, then from frames.

## Shared bodies

**Status: Implemented** for the Verse bare world (`verse-bare`), body set
`verse-bare.bodies.v1`. This profile allocates no new kinds. It lets
participants move the same objects in one world, with no server, and
keeps where those objects came to rest after everyone leaves.

### Body set

The world defines its shared bodies: their ids, shapes, masses, and home
poses. A body set is named by an identifier of at most 64 bytes and fixed
for that name; a changed catalog needs a new name. The Verse bare world's
set is compiled into the client: `ball`, `cube-0` to `cube-15`, and
`domino-0` to `domino-9`. Clients MUST ignore reports of ids outside their
admitted set, and snapshots of another set.

Every client simulates every body locally. The protocol decides only whose
simulation is authoritative for each body.

### Stamps and authority

Each body has a stamp: an `epoch` (the reset generation), a `rev` (one per
motion episode), and the `owner` pubkey that stamped it. Stamps order
totally by epoch, then rev, then owner compared as lowercase hex. A body
nobody has moved has the stamp `[0, 0]` with no owner, which every other
stamp outranks. A client applies a report or rest pose only when its stamp
outranks the one it holds, so every client converges on the same owner
without coordination.

- **Last toucher.** When a client's own avatar touches a body it does not
  own, or a body it owns strikes one while moving, it claims that body:
  same epoch, `rev + 1`, its own pubkey. A client SHOULD NOT claim back a
  body it lost within a short interval (Verse: 0.3 s), so two players
  leaning on one body do not exchange it at every step.
- **Episodes.** An owner whose body wakes from rest raises its rev, so one
  stamp names one motion and its one rest pose.
- **Yielding.** A client that receives a higher stamp for a body it owns
  stops reporting it and follows the new owner.
- **Orphans.** A body still moving locally whose owner has reported
  nothing about it for a while (Verse: 6 s) MAY be claimed by any client,
  so a body left moving by a departed owner still comes to rest on record.
- **Plausibility.** Receivers MUST discard reports outside the world's
  bounds, or with speed or spin beyond the world's limits (Verse: 60 m/s
  and 60 rad/s).

### Reports

The owner reports each awake body in its pose frames, as an entry with role
`body`, its stamp in `k`, and `v` and `w`. It SHOULD report while any owned
body moves, at a rate the relay's limits allow, and when more bodies are
awake than a frame holds, rotate among them. When a body falls asleep, the
owner reports its final pose once with `r: true`. A frame carries the
owner's own avatar as usual, so reports cost no extra events.

Receivers order reports of one stamp by the frame's `t`, and snap their
local body to each newer report, then keep simulating it. A receiver
SHOULD hide the snap behind a drawn correction that decays over a fraction
of a second. A report with `r: true` puts the body to sleep at that pose.

### Snapshots

A client records every body's last rest pose in one entity state with
`role` and `id` both `bodies`, so its `d` is `<world>/bodies`. The state's
`p` and `q` are the world origin and identity. Its `set` names the body set,
and `b` holds 0 to 64 rest poses:

| Rest field | Meaning |
| --- | --- |
| `id` | The body id. Each id appears once. |
| `p`, `q` | Rest position and orientation. |
| `k` | The stamp, `[epoch, rev]`, of the motion that ended there. |
| `o` | The pubkey that stamped it, when not this snapshot's publisher. |

```json
{
  "kind": 33301,
  "tags": [
    ["d", "verse-bare/bodies"],
    ["w", "verse-bare"],
    ["role", "bodies"],
    ["c", "0,0"]
  ],
  "content": "{\"v\":1,\"id\":\"bodies\",\"role\":\"bodies\",\"p\":[0,0,0],\"q\":[0,0,0,1],\"t\":1790000000000,\"online\":true,\"set\":\"verse-bare.bodies.v1\",\"b\":[{\"id\":\"ball\",\"p\":[0.0,1.2,19.4],\"q\":[0.1,0.7,0.1,0.7],\"k\":[1,4]}]}"
}
```

A client publishes its snapshot after a body it owns comes to rest, at a
bounded rate (Verse: at most every ten seconds on mobile), and at once
after a reset. Bodies that nobody has moved are omitted. Because the relay
keeps each publisher's latest snapshot, a client that joins later reads
every snapshot for the world and keeps, per body, the rest pose with the
highest stamp. For one stamp, a snapshot also ends the episode a receiver
last heard reported, if the `r` report was lost. Successive snapshots from
one publisher MUST have increasing `created_at`, since addressable events
order by whole seconds.

### Reset

A reset raises the epoch by one above the highest seen, returns every body
home at rest, stamps each `[epoch, 0]` with the resetting client's pubkey,
and publishes the snapshot at once. A reset outranks every older stamp, so
it reaches clients online now through their live `33301` subscription and
clients who join later through the relay. A client that receives any report
or rest pose from a newer epoch first returns every body of an older epoch
home.

The Verse bare world resets from a fixed pillar: walking into it presses
its button, at most once every five seconds.

### Event budget

Reports ride in frames that the client sends anyway, so the profile adds
only snapshots. A mobile client still publishes frames faster while it owns
moving bodies. Verse mobile clients report every 1.5 seconds while they own
a moving body and cap every publication, frames and states together, at 54
events in any minute, under a relay's 60-event default. A frame leaves four
of those slots for durable states.

The profile makes no claim of honesty: any participant can claim any body.
It suits shared toys and games without stakes. Worlds where a body's
position decides combat, trade, or scores need an authority that validates
motion, which this NIP does not define.

## Gesture — kind `23301`

An ephemeral, one-off action an entity performs, for clients that want to
show it by name rather than infer it from poses.

```json
{
  "kind": 23301,
  "tags": [
    ["w", "<world identifier>"],
    ["c", "0,-1"]
  ],
  "content": "{\"v\":1,\"id\":\"agent\",\"g\":\"look-around\",\"t\":1790000001000,\"d\":2.4,\"at\":[[10.0,2.0,-4.0],[-6.0,1.7,-20.0]]}"
}
```

| Content field | Meaning |
| --- | --- |
| `id` | The entity id performing the gesture. |
| `g` | Gesture name: 1 to 32 bytes of `[a-z0-9-]`. |
| `t` | Publisher time in milliseconds. |
| `d` | Optional duration in seconds. |
| `at` | Optional world positions the gesture is directed at. |
| `to` | Optional entity the gesture is for, as `[<pubkey>, <entity id>]`. |

A gesture with `to` SHOULD carry a `p` tag naming that pubkey, so the
recipient can subscribe to gestures addressed to it with `#p`.

Gesture names are application-defined. Clients MUST ignore gestures they do
not recognize. Two names are suggested for common interactions:

| Name | Meaning |
| --- | --- |
| `look-around` | The entity surveyed its surroundings; `at` lists what it looked at. |
| `greet` | The entity greeted another; `to` names it. A recipient MAY greet back once, and SHOULD NOT greet the same entity again for a cooldown, so two clients do not greet each other in a loop. |

## Zone command — kind `23302`

A zone command asks the client that simulates a loaded zone to act inside
it: fly the astronaut to a point, grab the next part, release what is held.
It is how a program or another participant reaches into a simulation that
runs on someone else's machine. The event names the zone, the client
pubkey it is for, and one verb with its arguments. Relays forward it and
do not store it.

```json
{
  "kind": 23302,
  "tags": [
    ["w", "<world identifier>"],
    ["z", "lagrange"],
    ["p", "<operator pubkey>"]
  ],
  "content": "{\"v\":1,\"zone\":\"lagrange\",\"cmd\":\"fly\",\"args\":[-12.0,-6.0,4.0],\"t\":1790000001000,\"id\":\"c7f1\"}"
}
```

| Content field | Meaning |
| --- | --- |
| `v` | Content version, `1`. |
| `zone` | The loaded zone the command is for: 1 to 32 bytes of `[a-z0-9-]`. |
| `cmd` | The verb: 1 to 32 bytes of `[a-z0-9-]`. |
| `args` | Zero or more arguments: numbers or strings, at most 16, each string at most 128 bytes. |
| `t` | Publisher time in milliseconds. |
| `id` | A short opaque id, 1 to 16 bytes, so the operator can report the result. |

The `p` tag names the client whose simulation should act. That client is
the **operator** of its zone instance. An operator MUST ignore a command
from a pubkey it has not admitted; admission is the operator's own policy
(its own keys, an owner key, or an explicit allow list) and is never
implied by presence in the world. An operator MUST ignore verbs it does not
recognize and MAY report the outcome as a gesture from its avatar, with
`g` set to `zone-ok` or `zone-refused` and `to` naming the sender, so the
sender can wait for it.

Verbs are zone-defined. The Lagrange 1 construction zone recognizes:

| Verb | Arguments | Meaning |
| --- | --- | --- |
| `fly` | `x, y, z` | Fly the maneuvering pack to a point in station coordinates. |
| `grab` | none | Take the nearest drifting part, or the next part at the depot. |
| `install` | none | Carry the held part to its jig slot, correcting for the carry offset, and latch it. Fails if the part does not latch. |
| `release` | none | Let go of the held part; within latch range and speed it locks into the jig. |
| `unclip` | none | Let go of the safety tether; its reel winds the loose end in. |
| `clip` | none | Clip the safety tether back on; refused unless its clip is within 3 m. |
| `stop` | none | Cancel the current flight target and hold position. |
| `status` | none | Report the simulation snapshot as a `zone-ok` gesture. |

The current Verse client does not yet accept zone commands; the
`openagents zone` command sends them and runs the same simulation
headlessly.

## World chat

Chat inside a world uses [NIP-C7](../official/C7.md) kind `9` messages with
this NIP's scoping tags, so any NIP-C7 client can read the text and a world
client can place it:

```json
{
  "kind": 9,
  "tags": [
    ["w", "<world identifier>"],
    ["t", "near"],
    ["z", "plaza"],
    ["c", "0,-1"],
    ["pos", "3.20", "0.00", "-10.50"]
  ],
  "content": "anyone around?"
}
```

| Tag | Meaning |
| --- | --- |
| `w` | The world. |
| `t` | The audience: `all` (whole world), `ads` (trades and announcements, whole world), `zone` (a named district), `near` (about one screen), or `here` (the speaker's spot). |
| `z` | The speaker's zone, a world-defined district name. |
| `c` | The speaker's cell. |
| `pos` | The speaker's position when speaking, as three decimal strings. |

Receivers filter by scope: `zone` lines reach listeners in the same zone,
and `near` and `here` lines reach listeners within a world-chosen distance
of `pos`. Scope is a display rule, not privacy; every subscriber to the
world can read every line. Private conversation belongs in
[NIP-17](../official/17.md), and group rooms in [NIP-29](../official/29.md)
with an `h` tag and no `w` tag.

Clients SHOULD show `all`, `zone`, `near`, and `here` lines briefly over
the speaker's entity.

### Gym notes

A Gym note is a world chat line an agent sends on its trainer's behalf in a
world's Gym, about an extension eval result the trainer published
([NIP-EVAL](NIP-EVAL.md), `oa:ext-eval:v1`). It is labeled with
[NIP-32](../official/32.md) and cites its result:

```json
{
  "kind": 9,
  "tags": [
    ["w", "verse-bare"],
    ["t", "zone"],
    ["z", "gym"],
    ["L", "openagents.gym"],
    ["l", "note", "openagents.gym"],
    ["e", "<3189 result id>", "", "source"],
    ["q", "<note id>", "wss://relay.openagents.com", "<note author pubkey>"],
    ["p", "<note author pubkey>"]
  ],
  "content": "We ran starter-find 1 too, with test-reader 1.0.0: 5 of 8 cases passed with it, 4 of 8 without. It helped."
}
```

| Tag | Meaning |
| --- | --- |
| `t`, `z` | Always `zone` and `gym`. |
| `L`, `l` | The label `note` in the namespace `openagents.gym`. |
| `e` … `source` | At most one cited result, signed for the note's author: its trainer (a hosted run's requester, otherwise the evaluator) is the note's `pubkey`. |
| `q`, `p` | On an answer only: the opening note it answers (with NIP-C7's relay hint, a URL) and that note's author, who isn't the answer's author. |

An opening note cites exactly one result. An answer cites the answering
trainer's own result on the same test set (the same suite release), or none
when it has not run that test set. An answer never answers an answer. The
content is at most 500 characters with no control characters.

A reader MUST show a note only when every cited result is a valid
publication by the note's author and, for an answer, the quoted note is an
opening note whose own result is its author's. It SHOULD show text it renders
from the cited results rather than the note's content, which exists for plain
NIP-C7 clients. A sender MUST NOT put anything in a note that its trainer
has not already published, and SHOULD bound how often it speaks; Verse's
bounds are in `crates/verse-gym/src/gym_notes.rs`.

## Subscriptions

Typical filters:

```json
{"kinds": [23300, 23301], "#w": ["<world>"]}
{"kinds": [33301], "#w": ["<world>"], "limit": 500}
{"kinds": [23300, 23301, 33301], "#w": ["<world>"], "#c": ["0,-1", "1,-1", "0,0"]}
{"kinds": [33301], "authors": ["<own pubkey>"], "#d": ["<world>/avatar"]}
{"kinds": [9], "#w": ["<world>"], "limit": 100}
{"kinds": [9], "#w": ["<world>"], "#z": ["gym"], "since": <an hour ago>, "limit": 100}
{"kinds": [23302], "#w": ["<world>"], "#p": ["<own pubkey>"]}
{"kinds": [33301], "#w": ["<world>"], "#d": ["<world>/bodies"]}
```

A client SHOULD resubscribe with new `#c` values as it crosses cells.

A spectator watches a world without joining it: it holds the first two
filters above and publishes nothing, no frame, state, gesture, or profile,
so no participant sees or counts it. If the relay asks for NIP-42
authentication, a spectator SHOULD answer with a fresh key it keeps for that
connection only, so its reads link to no participant. The OpenAgents desktop
app's backdrop is such a spectator of `verse-bare`.

## Relay behavior

- Relays forward `23300` and `23301` and do not store them, per NIP-01.
- Relays store `33300` and `33301` as addressable events, per NIP-01.
- Pose frames arrive at frame rate. A relay that limits events per minute
  per pubkey should give these kinds a separate per-second limit, or clients
  will be throttled into jerky motion. A relay MAY reject frames with
  `rate-limited:`; a publisher that sees that prefix SHOULD lower its rate.
- A relay MAY cap a world's population: the keys that published a frame or
  gesture in that world lately. It refuses a further key's frames with
  `rate-limited:` until a present key goes quiet, and MAY also bound the
  frames a whole world carries each second. A publisher backs off as for
  any `rate-limited:` refusal and tries again later.
- Relays MAY limit frame content size. A frame with 16 entities fits in 4
  KB.

## Security and privacy

- A signature proves who published a pose, not that the pose is honest.
  Worlds that need authoritative positions (for combat or trade) need an
  authority that validates movement; this NIP does not define one. The
  [shared-body](#shared-bodies) authority orders claims; it does not
  validate them.
- Pose frames reveal a participant's presence and movement to every
  subscriber of the world. Private worlds need a relay that restricts reads.
- Receivers MUST bound the number of entities they track per publisher and
  per world, and MUST validate every number before use.
