# NIP-MV — Shared 3D worlds

`draft` `optional` — v1.

This NIP defines how clients share presence in a 3D world over Nostr:
where each participant's entities stand, which way they face, what they are
doing, and where they were when they left. High-rate motion uses ephemeral
events that relays forward and do not store. Durable state uses addressable
events that relays keep, so a world persists across sessions the way an
MMORPG world does.

The NIP does not define rendering, physics, collision, or world geometry.
Two clients that share these events can show each other's entities
correctly even if they draw them differently.

## Terms

| Term | Meaning |
| --- | --- |
| World | A shared coordinate space, named by a world identifier. |
| Entity | One thing a publisher places in a world: a user's avatar, an agent the user controls, an object. An entity is identified by its publisher's pubkey plus an entity id unique to that publisher. |
| Pose | An entity's position, orientation, and optional velocity at one instant. |
| Frame | One ephemeral event carrying the current poses of one or more of a publisher's entities. |
| State | The durable, last-known record of one entity. |
| Cell | A square area of the world floor used to scope subscriptions. |

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

The world's `w` tag names its own address.

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

Gesture names are application-defined. Clients MUST ignore gestures they do
not recognize.

## Subscriptions

Typical filters:

```json
{"kinds": [23300, 23301], "#w": ["<world>"]}
{"kinds": [33301], "#w": ["<world>"], "limit": 500}
{"kinds": [23300, 23301, 33301], "#w": ["<world>"], "#c": ["0,-1", "1,-1", "0,0"]}
{"kinds": [33301], "authors": ["<own pubkey>"], "#d": ["<world>/avatar"]}
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
