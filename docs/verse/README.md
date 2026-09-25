# Verse

Verse is the OpenAgents desktop world: a walkable 3D city drawn in amber
lines on a near-black field. `crates/verse` holds the first slice, a
Tron-style city and a third-person character you run around with World of
Warcraft controls.

Status: partial. One world exists, and players share it over Nostr with
[NIP-MV](../../nips/openagents/NIP-MV.md): each player sees the others'
avatars and agents move and turn, and the relay remembers where everyone
left. Live OpenAgents state (Pylons, runs, sats) is not implemented.

## Run it

Start the local relay in one terminal, then open the game in another. From
the repository root:

```sh
scripts/verse-relay.sh
cargo run -p verse --release
```

To play alone without a relay, add `-- --offline`.

A 1440×900 window titled **Verse** opens with the character on a plaza,
facing a pylon, with the city around the plaza. The player's agent, a
floating 3D spade, hovers behind the character's right shoulder.

## Multiplayer

Verse speaks [NIP-MV](../../nips/openagents/NIP-MV.md), a standalone NIP
for shared 3D worlds that this repository defines. Every window is a player
with its own Nostr key.

**Signing up.** The first launch of a profile creates its key in
`~/.openagents/verse/<profile>.key` (owner-only) and publishes a NIP-01
profile naming it. `--profile <name>` picks the profile, so one machine can
run several players:

```sh
cargo run -p verse --release -- --profile north
cargo run -p verse --release -- --profile south
```

**Spawning.** On launch the game asks the relay for the player's own avatar
state. A returning player resumes where they left. A new player spawns at a
random clear spot within 28 m of the plaza center, so new players can see
each other.

**What goes over the wire.** Everything is in world `verse-plaza`.

| Event | Kind | Stored | When |
| --- | --- | --- | --- |
| Pose frame | `23300` | No (ephemeral) | 10 per second while moving, 4 while still. Carries the avatar's and the agent's position and quaternion. |
| Entity state | `33301` | Yes (addressable) | On join, every 3 s of movement, and on quit with `online: false`. |
| Gesture | `23301` | No (ephemeral) | `look-around` when the agent looks around, with the positions it looked at; `greet` when it greets another agent, addressed to that agent. |
| Profile | `0` | Yes | Once, when a profile is created. |

**Drawing others.** Remote avatars and agents are drawn 150 ms in the past,
interpolated between frames (`lerp` for position, `slerp` for the
quaternion), so their movement and turning stay smooth. The agent's
published orientation includes its wobble and emotes, so other players see
it spin and roll. A player with no frame for 10 seconds is drawn at a
quarter brightness where their state says they stopped: resting, not gone.
The window title shows the relay status and how many other players are
online and resting.

**The relay.** `scripts/verse-relay.sh` runs this repository's
`nostr-relay` on `ws://127.0.0.1:7447` with Postgres state under
`~/.openagents/verse/relay`, so the world persists across restarts (`--reset`
wipes it; `VERSE_RELAY_BIND=0.0.0.0` lets other machines join). It raises
the relay's rate limits, because the defaults (60 events a minute per
pubkey) are far below pose-frame rates. `--relay <url>` or `VERSE_RELAY`
points the game elsewhere; only `ws://` URLs work for now. The production
relay keeps the default limits, so it cannot carry pose frames until it gets
a per-second lane for these kinds, as NIP-MV recommends.

**Testing.** `cargo test -p verse` covers the protocol, interpolation, and
identity without a network. With a relay running,
`VERSE_TEST_RELAY=ws://127.0.0.1:7447 cargo test -p verse --test relay`
signs up two players, checks each sees the other's avatar and agent,
checks a returning player resumes, and checks two agents that meet greet
each other. It leaves two resting test players in
the relay's world.

## The agent

The agent is a spade from a deck of cards, extruded into 3D and drawn like
everything else: near-black faces and amber edges. It floats about 2.2 m up
and follows the player:

- It chases a point behind the player's right shoulder on a slightly
  underdamped spring. It trails when you run and overshoots a little when
  you stop.
- The point it chases drifts slowly, so its distance from the player is
  never exact.
- It bobs and wobbles on several unrelated frequencies, leans into its own
  speed, and turns lazily toward your heading.
- A faint ring on the ground below it shows where it is.

It also plays emotes over that motion:

| Emote | When | What it does |
| --- | --- | --- |
| Look around | You run for more than 0.6 s, stop, and it catches up to you. | Assesses its surroundings: queries the relay for entity states in the nine cells around it, then looks toward the two nearest players or agents it found, left one first. With nothing nearby, or offline, it glances a default left and right. Other players receive a `look-around` gesture naming what it looked at. |
| Spin | At random while idle beside you, every 5 to 12 s. | One full turn with a small lift. |
| Look up and down | At random while idle. | Tips back to look up, then forward to look down. |
| Barrel roll | At random while idle. | One full roll around its facing axis, rising a little. |
| Greet | Its agent comes within 7 m of another player's online agent, or that agent greets it first. | Turns to face the other agent, bows twice, and hops. Sends a `greet` gesture addressed to the other agent, whose player greets back. Each pair greets at most once every 45 s. |

The agent has no behavior yet beyond following and emoting. Its game design is in
[`gdd.md`](gdd.md).

## Controls

| Input | Effect |
| --- | --- |
| `W` / `S` | Run forward. Backpedal, which is slower and overrides `W`. |
| `A` / `D` | Turn left and right. While the right mouse button is held, strafe instead. |
| `Q` / `E` | Strafe left and right. |
| Arrow keys | Same as `W`, `S`, `A`, and `D`. |
| `Shift` | Sprint forward. |
| `Space` | Jump. |
| Left mouse drag | Orbit the camera without turning the character. |
| Right mouse drag | Mouselook: the character turns to face the camera, then turns with the mouse. |
| Both mouse buttons | Run forward. |
| Mouse wheel | Zoom between 2.5 m and 40 m. |
| `Esc` | Quit. |

While a mouse button is held, the cursor is hidden and locked. When the
character moves and the left button is up, the camera swings back behind it.

## Capture a frame

`--capture` renders the spawn view to a PNG without opening a window. Use it
to review visual changes:

```sh
cargo run -p verse --release -- --capture target/verse/spawn.png
cargo run -p verse --release -- \
  --orbit 150 --pitch 12 --distance 14 --size 1600x1000 \
  --capture target/verse/front.png
```

| Flag | Meaning |
| --- | --- |
| `--orbit <degrees>` | Camera angle around the character. `180` looks at its face. |
| `--pitch <degrees>` | Camera angle above the horizon. |
| `--distance <meters>` | Camera distance from the character. |
| `--size <width>x<height>` | Image size. Default `1600x1000`. |

## Design

**Look.** The look comes from the walkable Tassadar run board shown in
episode 240 ([`docs/transcripts/240.md`](../transcripts/240.md)): a dark,
Snow Crash–style street you look around in. The art is old-school Tron:
only lines. Buildings are solid near-black boxes with amber edges, so nearer
buildings hide farther lines. Fog fades distant lines into the field. A
ridge line at 900 m ignores the fog and marks the horizon.

**Palette.** Verse uses the Coder terminal's colors and nothing else: the
four [`coder_terminal::Intensity`](../../crates/coder-terminal/src/intensity.rs)
steps over one amber, and `NEAR_BLACK` for the clear color, the fog, and
every face. `palette.rs` converts those values to linear light and does not
restate them. The test `every_color_is_on_the_amber_ladder` fails if any
vertex in the world has another color.

| Step | Hex | Used for |
| --- | --- | --- |
| Quarter | `#463100` | Fine ground grid, building floor bands, horizon base |
| Half | `#835b00` | Streets, pylon rings, the character's ground ring, horizon ridge |
| ThreeQuarters | `#c18600` | Building edges |
| Full | `#ffb000` | Rooflines, masts, the pylon, the character, the agent's front edge |
| Field | `#080600` | Background, fog, solid faces |

**Stack.** Verse uses the Ruins of Atlantis engine family:

- `wgpu` 29 and `winit` 0.30, with no engine framework.
- A custom renderer: one WGSL shader with a face pipeline and a line
  pipeline, 4× MSAA when the adapter supports it, and an sRGB surface.
- `glam` for math.

The winit `wayland-csd-adwaita` feature is off because it pulls
BSD-2-Clause `arrayref`, which [`deny.toml`](../../deny.toml) does not
allow.

**Controller.** The movement rules and speeds are reimplemented from Ruins
of Atlantis `client_core` (`PlayerController`, `mouselook.rs`, and the
render crate's third-person follow camera), not copied:

- Run at 7 yards per second and backpedal at 4.5.
- Keyboard turn at 180° per second.
- The right mouse button switches `A`/`D` from turning to strafing.
- Jumps are under gravity.

The character collides with building footprints and the world edge.

## Code map

| File | Owns |
| --- | --- |
| [`src/main.rs`](../../crates/verse/src/main.rs) | The binary: window mode or `--capture`. |
| [`src/app.rs`](../../crates/verse/src/app.rs) | winit event loop, key and button state, cursor capture, the frame step. |
| [`src/controller.rs`](../../crates/verse/src/controller.rs) | `InputState`, `PlayerController`, footprints, collision. |
| [`src/camera.rs`](../../crates/verse/src/camera.rs) | `FollowCamera`: orbit, mouselook, zoom, settle, view-projection. |
| [`src/world.rs`](../../crates/verse/src/world.rs) | Seeded city, ground grid, pylon, horizon. The same city every launch. |
| [`src/avatar.rs`](../../crates/verse/src/avatar.rs) | The boxy line character and its distance-driven walk cycle. |
| [`src/agent.rs`](../../crates/verse/src/agent.rs) | The floating spade agent: spring follow, bob, wobble, emotes, scan requests, and geometry. |
| [`src/mv.rs`](../../crates/verse/src/mv.rs) | NIP-MV: kinds, frame, state, and gesture content, signing, cells, and validation. |
| [`src/net.rs`](../../crates/verse/src/net.rs) | The relay link: one websocket thread, reconnects, and replayed subscriptions. |
| [`src/session.rs`](../../crates/verse/src/session.rs) | Sign-up, spawn or resume, publish cadence, scans, and leaving. |
| [`src/crowd.rs`](../../crates/verse/src/crowd.rs) | Other players: buffered, interpolated poses, online and resting, and their meshes. |
| [`src/identity.rs`](../../crates/verse/src/identity.rs) | Profile keys under `~/.openagents/verse/`. |
| [`src/mesh.rs`](../../crates/verse/src/mesh.rs) | The shared vertex format and line, quad, cube, and ring builders. |
| [`src/palette.rs`](../../crates/verse/src/palette.rs) | The amber ladder in linear light. |
| [`src/render.rs`](../../crates/verse/src/render.rs), [`src/shader.wgsl`](../../crates/verse/src/shader.wgsl) | Pipelines, fog, the window renderer, and PNG capture. |

Test the crate with `cargo test -p verse`. Tests cover the controller rules,
the camera limits, the world's determinism and clear spawn, and the palette
guard. They do not need a GPU.

## Game design

The draft game design document is [`gdd.md`](gdd.md): an MMORPG plus
agents. Each player builds an agent whose stats set how it decides, sends
it on visible visits to do real work, and keeps its condition up.

## Next

These build on the direction in [`docs/game/README.md`](../game/README.md):

- Put live OpenAgents objects in the world: Pylons, training windows,
  verified work, and sats, as the episode 240 board did.
- Tab-targeting and a minimal HUD in the same amber.
- A per-second lane on the production relay for NIP-MV frames, `wss://`
  support, and cell-scoped subscriptions as the world grows.
- A web build over WebGPU, following Ruins of Atlantis.
