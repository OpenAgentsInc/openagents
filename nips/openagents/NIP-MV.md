# NIP-MV — Shared 3D worlds

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

## Kinds

| Kind | Class | Record |
| --- | --- | --- |
| `33300` | Addressable | World definition |
| `33301` | Addressable | Entity state |
| `23300` | Ephemeral | Pose frame |
| `23301` | Ephemeral | Gesture |

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

The current Verse `verse.zone.v1` schema is intentionally narrower than a
general authoring format. It has exactly `schema`, `world`, `ruleset`,
`physics`, `asset_sha256`, and `asset_bytes`. The host admits one reviewed Ruins
world, the original `ruins.wizard-woods.v1` real-time simulation, `ruins.heightfield.v1` terrain, and
one pinned asset pack. Its palette, bounds, and arrivals are compiled host
values. Its local world name is not a published `33300` address, so this local
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
  controls), and `object`. Clients MUST accept unknown roles and MAY skip
  drawing them.

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
`walk`, `run`, `jump`).

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

## Subscriptions

Typical filters:

```json
{"kinds": [23300, 23301], "#w": ["<world>"]}
{"kinds": [33301], "#w": ["<world>"], "limit": 500}
{"kinds": [23300, 23301, 33301], "#w": ["<world>"], "#c": ["0,-1", "1,-1", "0,0"]}
{"kinds": [33301], "authors": ["<own pubkey>"], "#d": ["<world>/avatar"]}
{"kinds": [9], "#w": ["<world>"], "limit": 100}
```

A client SHOULD resubscribe with new `#c` values as it crosses cells.

## Relay behavior

- Relays forward `23300` and `23301` and do not store them, per NIP-01.
- Relays store `33300` and `33301` as addressable events, per NIP-01.
- Pose frames arrive at frame rate. A relay that limits events per minute
  per pubkey should give these kinds a separate per-second limit, or clients
  will be throttled into jerky motion. A relay MAY reject frames with
  `rate-limited:`; a publisher that sees that prefix SHOULD lower its rate.
- Relays MAY limit frame content size. A frame with 16 entities fits in 4
  KB.

## Security and privacy

- A signature proves who published a pose, not that the pose is honest.
  Worlds that need authoritative positions (for combat or trade) need an
  authority that validates movement; this NIP does not define one.
- Pose frames reveal a participant's presence and movement to every
  subscriber of the world. Private worlds need a relay that restricts reads.
- Receivers MUST bound the number of entities they track per publisher and
  per world, and MUST validate every number before use.
