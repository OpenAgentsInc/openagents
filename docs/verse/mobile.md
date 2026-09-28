# Verse in Coder for iOS and Android

Coder opens directly into **Verse**, which mounts the same seeded city, player controller,
collision rules, camera, avatar animation, following spade agent, meshes, and
wgpu scene renderer as the desktop application. Rust Native supplies a generic
native drawing-surface contract. Verse supplies the world; `coder-ui` supplies
the application and plaza palette. Loaded zones may use their own appearance.
No OpenAgents identity, theme, world, or network
implementation belongs to the reusable `rust-native` crate.

The OpenAgents app's **Verse** tab mounts the same surface in its bare mode
(`WorldRuntime::bare`), named **The Grid** wherever it is shown
(`WorldRuntime::zone_label`, the zone snapshot's label, and the return
controls); Coder's plaza keeps its own name. The Grid has only the plaza's ground grid, drawn in the neutral
palette (each amber step's lightness in white light), and the player with the
controls below ([the Grid's sticks](#the-grids-sticks)), and other players' avatars, and the Gym
([below](#the-grids-gym)). It has no chat, map, doors, computer, or
companion, and one zone: a walk-in portal to Lagrange 1
([below](#the-grids-portal-to-lagrange-1)). See
[presence in the OpenAgents app](#presence-in-the-openagents-app) and
[OpenAgents for iOS](../../bins/openagents-ios/README.md).

### The Grid's sticks

The Grid draws two faint sticks, rings with round knobs at half the opacity
of Coder's stick, above the bottom safe area:

- **Movement**, at the bottom left, as in Coder: push up to walk, down to
  back up, and sideways to strafe.
- **Look**, at the bottom right, in touch look only: push right to turn
  right and up to look up. Nothing turns inside a 12-point dead zone; past
  it the turn rate rises with the square of the deflection, to 2.6 rad/s of
  turn and 1.6 rad/s of pitch at the rim, smoothed over about 0.06 s while
  held and stopped at once on release. It works in first person too. Motion
  look turns the camera with the phone, so there the stick is hidden.

Both thumbs can hold their sticks at once, and a pinch with other fingers
takes neither: the host keeps both sticks' touches (`stick_pointer`,
`look_stick_pointer`) out of pinch arbitration. A drag anywhere else still
looks, but only one look control is held at a time. The camera buttons
(touch or motion look, and recenter) sit at the bottom center, between the
sticks.

The bare world's fog starts 6 m from the camera and is total at 110 m, well
inside the grid's 264 m edge, so the grid dims gradually toward the horizon
instead of drawing the distant lines as bright as the near ones. Coder's
plaza keeps its own fog, from 60 m to 250 m.

### The ball

The bare world has three physical objects: a ball, a stack of cubes, and an
arc of dominoes, plus a fixed pillar whose button puts them back. Everyone in
the world shares them (see [Sharing the ball](#sharing-the-ball)). The ball
is a 2.4 m sphere resting 7 m ahead of the world's spawn. Walk into it to
push it. It slides, spins up, rolls, and comes to rest on its
own. It lives in [`verse::ball`](../../crates/verse/src/ball.rs) on the shared
[`physics`](../../crates/physics/) crate, the one Lagrange 1 and the Physics
Lab use:

- The ground is a static box, the grid's edges are invisible static walls,
  and the player is a kinematic capsule (0.45 m radius, 1.8 m tall) that
  sweeps the path the shared player controller walks each frame. The player
  is then moved clear of the ball, so the ball cannot be walked through.
- The ball is a thin rubber shell: 40 kg, radius 1.2 m, I = 2/3 m r²,
  friction 0.8, restitution 0.35, torsional friction 0.02 m. The crate's
  contacts, friction cone, and island sleep do the rest.
- The crate has no rolling resistance or air, so the ball module adds both:
  a rolling-resistance moment of 0.1 × the measured ground reaction × the
  radius, bounded so it never turns the ball backward, and quadratic drag
  (Cd 0.47, air 1.2 kg/m³). Pushed at running speed (6.4 m/s), the ball
  rolls about 22 m and sleeps within 15 s.
- Steps are 1/120 s with at most 12 per frame, as in Lagrange 1; the ball is
  drawn between its last two poses. A step takes about 20 µs in a debug
  build (`cargo test -p verse --lib ball -- --nocapture` prints it), and the
  iOS simulator's debug build reports 0.02 ms asleep and 0.06 to 0.08 ms per
  frame while rolling.

The ball is drawn through the physical renderer on the neon stage. A studio
[`pbr::Key`](../../crates/verse/src/pbr/mod.rs) lights lit geometry with
Lagrange 1's shading: a shadowed key light (4,200 lux, 0.035 rad source, so
soft contact shadows), an unshadowed rim light behind, and a dim ambient sky,
pre-exposed at EV 10 so white reads near display white beside the lines. The
ball is white and charcoal lacquer (`Material::Lacquer`, a clear coat over
paint) in alternating octants, so its rotation shows. Nothing is drawn on
the floor under the ball or the blocks: no pool of light and no shadow disc,
so they stand on the grid's lines alone. Coder's plaza has no key light and
draws exactly as before.

### The stack and the dominoes

Beyond the ball, on either side of the line from the spawn to it, stand a
stack of cubes and an arc of dominoes
([`verse::blocks`](../../crates/verse/src/blocks.rs)). They are dynamic boxes
in the ball's physics world, so the crate's oriented box contacts, friction,
restitution, and island sleep apply between the blocks, the ground, the
ball, and the player's capsule, at the ball's fixed step:

- The stack is 2 × 2 × 4 lacquered cubes, 0.8 m on a side and 5 kg each
  (hollow boxes), in a white and charcoal checker, 18 m ahead of the spawn
  and 5 m to the left. It is placed exactly at rest, so it sleeps at once and
  stands until the player walks into it or the ball rolls into it; then it
  topples, tumbles, and settles as rubble that sleeps again.
- Ten white dominoes, 1.5 m tall, 0.8 m wide, 0.22 m thick, 8 kg, with a
  charcoal bar across each face, stand 0.7 m apart along a quarter arc of
  4 m radius, starting 16 m ahead and 2.5 m to the right and curving
  away. Walk into the first and the row falls in turn.
- Both stand at fixed places in the world, laid out in the spawn's frame, so
  every player sees the same arrangement. A restored position moves only the
  player. A block that leaves the world returns to where it stood.
- The studio key's shadow region widens to cover the ball and both demos
  while the ball is near them, so the ball shades the blocks. The player
  is kinematic, so walking on through fallen blocks shoves them aside.
- Asleep, the blocks cost nothing. A tumbling stack costs about 12 µs per
  step in a release build and 0.2 ms in a debug build
  (`cargo test -p verse --lib blocks -- --nocapture` prints it).

To render it offline:

```sh
cargo run -p verse --release --features capture --example bare_capture -- target/verse/ball.png 1.2 0.6 90
```

The arguments are seconds of walking forward, seconds of waiting, and a
sideways camera orbit in pixels.

### The Grid's Gym

The Grid has Coder's [Gym](gym.md), drawn in white and gray like the grid:
the same hall, low walls, open roof, **GYM** lettering, and boards, with the
same board, connection, and recipe rules. Only its place and palette differ;
Coder's plaza Gym is unchanged and amber.

- **Placement.** `verse::world::GymSite::GRID` turns the Gym a quarter turn
  so that its doorway stands 36 m straight ahead of the spawn
  (`GRID_GYM_AHEAD`), facing it, and the hall runs 24 m farther along the
  spawn's heading. From the spawn, the **GYM** lettering stands above the
  doorway in the middle of the view, past the ball and between the stack and
  the dominoes. Every block, the ball, the reset pillar, and the Lagrange 1
  portal stand at least 14 m short of its walls. `GymSite::PLAZA` is the
  identity, so every `GYM_*` constant stays in the Gym's own frame.
- **Walls.** The walls block walking, and the same boxes stand in the ball's
  physics world, so the ball and blocks bounce off them.
- **Board.** Walk in and up to the central board: from within 6 m it reads
  **TAP TO OPEN**, and a tap on it (not a drag, and not with the stick) opens
  the host's Gym panel. VoiceOver offers **Open Gym board**. Walking out
  closes the panel and pauses observation, as in Coder.
- **Connection.** The Gym grant is for the world key (the key the panel
  shows). The OpenAgents app keeps the pasted `gym-connect:` code in its own
  Keychain item (`com.openagents.app.gym`, this device only) and passes it as
  `gym_code` when the world mounts. A host without a Gym panel (Android, for
  now) shows the building but no tap cue, and its board never opens.
- **Preview.** Simulator and debug builds take `--gym-preview`: the labeled
  synthetic board, offline, starting just outside the doorway.
  `--verse-script walk,walk,walk,wait,board` walks up to the board and taps
  it.
- **Results.** Beside the central board stands the **RESULTS** board, the
  Gym's [published Terminal-Bench results](gym-leaderboard.md#gym-ux-in-the-grid).
  It needs no Gym connection. Entering the Gym starts loading them (a
  verified cached copy shows at once), leaving cancels it, and a tap on the
  board (or VoiceOver's **Open results board**) opens the white-on-black
  results panel: the boards list, a board, and an attempt, each drawn from
  Rust's view model. `--verse-script walk,walk,walk,right,wait,results`
  opens it in the preview, and `r=board:<id>`, `r=attempt:<id>`, and the
  other `r=` steps choose in it. A host without the panel (Android, for
  now) shows the lettering but no tap cue, and loads nothing.

### The Grid's portal to Lagrange 1

One arch stands on the bare world (the Grid), lettered **LAGRANGE 1** and
drawn in white and gray like the grid. It has no button: walk through its
opening and [Lagrange 1](lagrange-1.md) loads at once. Flying the pack back
through the station's return arch, lettered **THE GRID**, returns; so does
the zone panel's **The Grid** button, or a tap on that arch.

- **Placement.** The arch stands in the ball's layout frame
  (`verse::zones::gate::GRID_PORTAL_AT`): 9 m to the stack's side of the
  line from the spawn to the ball and 11 m ahead, short of the stack and
  opposite the dominoes, turned to face the spawn so its lettering reads
  head-on. A restored spawn lays it out with the ball and blocks, inside the
  walls. Tests keep it more than 5 m from every block and 6 m from the ball.
- **Crossing.** `verse::zones::Gate::crossed` admits feet that pass the
  arch's plane, or stand within 0.6 m of it, inside the 3 m opening and
  below its lintel. A long frame's step is checked where it crossed the
  plane. Walking past a pillar, approaching, or jumping over does not enter.
  After a crossing, both arches wait one second before admitting another.
- **Returning.** The player comes back 3.5 m in front of the portal, facing
  away from it, so walking on never re-enters. The ball and blocks wait
  where they were left: they are not stepped while the player is away, and
  the return does not lay them out again.
- **Neutral Lagrange 1.** Entered from the Grid, the station keeps its
  physical materials, Sun, Earth, Moon, and stars, but every guide and
  overlay is white or gray at the same brightness: the Earth reticle, the
  autopilot line, the next part's outline, the airlock refill ring, the
  forces overlay, and the return arch. A carried part's slot outline is gray
  until aligned and white when it latches (Coder draws amber, then green).
  The zone panel (caption, **Grab**/**Latch**, **Unclip**/**Clip**,
  **Forces**, **Art**/**Photo**, **The Grid**) is drawn in the neutral palette
  above the sticks.
  Coder's plaza, its three arches with their buttons, and its amber Lagrange
  1 are unchanged.
- **Presence.** Lagrange 1 is local, as every zone is in Coder: its station
  simulation runs on the device, and its coordinates are not Grid
  coordinates. Walking through the portal ends the `verse-bare` session, so
  nothing is published while the player is in the zone and other players'
  avatars are cleared; peers see the player leave, and the live-only crowd
  drops them. Coming back starts a fresh `verse-bare` session that
  publishes the player's state at the portal, without restoring the
  relay's saved position. No separate NIP-MV world is joined for the
  station, since no shared station state exists to show there.

#### Sharing the ball

Everyone in the bare world pushes the same ball and topples the same blocks,
and finds them where the last player left them, over NIP-MV's
[shared-body profile](../../nips/openagents/NIP-MV.md#shared-bodies)
([`verse::shared`](../../crates/verse/src/shared.rs)):

- **Every client simulates every body**; the protocol decides whose
  simulation counts. Each body carries a stamp: the reset epoch, a revision
  per motion, and the owner's key. Stamps order totally, so every client
  settles every conflict the same way.
- **The last toucher owns.** Walking into a body, or striking one with a
  body you own (the ball into the stack), claims it with the next revision.
  If two players claim at once, the higher key wins everywhere.
- **The owner reports.** While you own a moving body, your pose frames carry
  it, with velocity and spin, every 1.5 seconds, and its rest pose once when
  it stops. Other players snap their copy to each report and keep simulating
  it, and the drawn body glides to the correction over about 0.15 s instead
  of jumping. A frame holds your avatar and up to 15 bodies; when more are
  moving, reports rotate among them.
- **Rest poses persist.** After a body you own comes to rest, you publish a
  snapshot of every rest pose you know as one addressable state
  (`verse-bare/bodies`), at most every ten seconds. The relay keeps each
  player's latest snapshot, so a player who joins after everyone left
  merges them and keeps, per body, the newest.
- **Orphans.** A body still rolling whose owner has said nothing about it
  for six seconds (the owner left) is adopted by a player who sees it, so
  its rest is still recorded.
- **Plausibility.** Reports outside the world's walls, below the floor, or
  faster than 60 m/s or 60 rad/s are ignored.
- **Budget.** Reports ride in the frames you send anyway. Every bare-world
  publication, frames and states together, is capped at 54 events in any
  minute, under the public relay's 60-event default. While you own a moving
  body, that is about 40 frames, 6 snapshots, and 2 avatar states a minute.

Other players' avatars are drawn 3.3 seconds in the past, but bodies are
not, so a remote player reaches the ball on screen a little after it
starts to move. Anyone can move anything: the stamps order claims but do
not validate them, which suits a toy with no stakes.

#### The reset pillar

A charcoal pillar with a button on top stands 6 m to the right of the spawn
and 4 m ahead ([`verse::pillar`](../../crates/verse/src/pillar.rs)). The
bare world has no action button, only its sticks, so you press the button by
walking into the pillar: the first touch presses it, holding against it
does not press again, and presses are at least five seconds apart. The
button is lit white while anything sits away from home, and it sinks
briefly when pressed.

A press returns the ball and every block home, at rest, for everyone: it
raises the epoch, which outranks every earlier claim, and publishes the
snapshot at once. Players online see everything return within a moment,
and players who join later find it home. The pillar is a static box in the
physics world, so the ball and the blocks bounce off it, and the player is
kept out of it.

## Walk the world

The world fills the entire display behind the system clock and home indicator.
Camera icons sit at the bottom right inside the safe area. The hand/gyroscope
icon switches touch and motion look; the crosshair recenters the camera. The world has no
title banner or idle-status labels. New or unconfigured installs join
`wss://relay.openagents.com` automatically using their own Verse identity.
No chat pairing or model account is required. A computer sits directly ahead of the starting position.
Walk closer and tap the computer's monitor to open its controls. The monitor
shows **WALK CLOSER** until you are within reach, then **TAP TO OPEN**.

- In **Touch look**, drag anywhere on the world to turn and look up or down.
  A translucent movement stick sits above the bottom-left safe area: push it
  up to walk forward, down to back up, and sideways to strafe. Releasing the
  stick stops movement.
- Switch to **Motion look** to look around by turning your body with the phone
  or pointing the phone left, right, up, or down. Screen roll does not tilt the
  horizon. The shared Rust camera interpolates toward the latest orientation
  on each rendered frame. Hold the stick to walk forward; push it to adjust
  direction. **Recenter** uses
  your current phone position as the new reference and returns the camera behind
  your character at its default pitch. Zoom stays unchanged.
  Switch back to **Touch look** whenever you prefer finger controls.
- Motion look pauses while the app is in the background or an in-world panel
  is open. It starts from a fresh reference when you return. If motion is
  unavailable, use touch look.
- Double-tap open world space to jump, including while your other thumb holds
  the stick. You can also hold the stick and drag the camera at the same
  time. The computer keeps its single-tap action.
- Place two fingers together, then spread them to zoom in or pinch inward to
  zoom out. A deliberate pinch owns those touches until you lift them, but
  never the stick's: in the OpenAgents app a thumb holding the stick keeps
  walking while the other hand pinches. Adding
  a look drag after movement starts keeps independent controls active.
  The HUD has no walk/sprint toggle, jump button, or zoom buttons.
  In the OpenAgents app's bare world, keep spreading past the nearest orbit
  to enter first person: the camera moves to the player's head and the
  avatar is hidden. Pinch inward to return to third person. Coder's plaza
  and the zones stop at the nearest orbit.
- Use the world computer to reach your linked computers: see each one's
  status and route, open it, order work, and follow that work in
  **Activity**. Its **CHATS** page keeps QR pairing and the read-only Codex
  and Claude viewer. Selecting a transcript expands its reading area;
  **All chats** returns to the chat list. Tap **CLOSE** to continue walking.
- Walk east to the **GYM** building. Enter, approach the boards, and tap
  the physical **GYM** board to inspect Microcoder and Terminal-Bench runs.
  Its **TAP TO OPEN** prompt is world geometry, with no floating native entry button.
  [Gym setup and controls](gym.md) cover its separate host grant, recorded
  charts, and explicitly confirmed run recipes. Leaving pauses Gym updates.
- Follow the [pairing guide](../coder/guides/mobile-readonly.md). The computer
  command displays an expiring QR invitation and stays running to serve chats;
  the phone also accepts its complete pairing string. Camera permission is
  requested only when you choose to scan.

The iOS world requests a 60 Hz display callback (30 Hz minimum); Android
follows its native display callback. Motion samples are requested at 60 Hz.
Both use a single-sample render target: a Metal layer on iOS and an Android
native window on Android. Android draws with Vulkan, or with OpenGL ES where
Vulkan isn't available and on the emulator; see
[graphics backends](README.md#graphics-backends). Desktop retains its mouse and keyboard controls and
supported 4× MSAA.
Both surfaces run `verse::runtime::WorldRuntime`; mobile does not approximate
the city with a separate scene or image.

Native motion adapters supply device-to-world quaternions and separate sample
and receipt timestamps. Freshness is checked at receipt, so a slow previous
render cannot make a fresh sensor reading look stale. Recenter, app suspension,
and panel transitions discard the previous motion target. The camera keeps
its eye above the ground while preserving the requested view direction,
including upward look; the ground clamp does not force it to keep looking
at the avatar. See the
[motion-camera verification](../coder/verification/2026-09-26-motion-camera.md).

## Map navigation

Tap the top-right map to expand it, then choose a clear position or landmark to
walk there. The character follows a collision-aware route; manual movement or
jumping stops it. Camera input can continue while walking. See
[maps, companions, and doors](world-interactions.md) for the ordered demo work.

## Enter a zone

Three arches on the plaza lead to local zones: **Ruins** (west),
**Lagrange 1** (east), and the **Physics Lab** (north, behind the spawn).

### Ruins

Expand the map and choose **Ruins portal** to walk to its plaza arch.
Tap its opening or choose **Enter Ruins** while nearby. This starts the first
asset download; normal plaza startup and walking near the arch do not download
the pack. Loading shows progress and **Cancel**. A failure keeps the plaza
available with **Retry** or **Dismiss**.

Ruins uses its own colors and the original Wizard Woods real-time
simulation. Touch movement, motion look, pinch, and the map use shared Rust
controls on the retained source heightfield. Monsters chase targets and NPC
wizards cast as the foreground world updates. Tap **Firebolt**, **Missile**, or
**Fireball** on the bottom hotbar while moving. HP, mana, and cooldowns reflect
the original game state. There is no turn or movement-budget control.

Choose **Plaza** to leave, including during a fight, or use the return portal.
The app restores the saved plaza position and releases active zone geometry.
A verified disk cache speeds later entry. No Ruins model is embedded in the
app merely so the plaza can start.

### Lagrange 1

Choose **L1 portal** on the map, then tap the arch or **Enter L1**. The station
is generated on the device, so it opens immediately. The left joystick commands
the maneuvering pack; with no input it holds position, and motion builds up
gradually because the pack has only 40 N of thrust. Tilt the view above or
below level to climb or dive, double-tap for a short climb, or tap the map to
set an autopilot target. Fly to the depot, tap **Grab**, carry the part to its
outlined slot on the keel jig, and tap **Latch** when the outline turns green.
The caption shows Earth distance, remaining nitrogen, and speed; the airlock
ring refills the pack. See [Lagrange 1](lagrange-1.md).

### Physics Lab

Choose **Lab portal** on the map, then tap the arch or **Enter Lab**. The lab is
generated on the device, so it opens immediately. Walk around the railed stage
with the joystick. The zone panel shows the scenario, the selected knob, and
readouts above two rows of controls: tap **Prev** or **Next** to select a knob,
**-** or **+** to change it, **Reset**, **Pause** or **Run**, **Step**, and
**Plaza**. The first knob switches among nine scenarios. See
[Physics Lab](physics-lab.md).

The OpenAgents app's bare world reaches only Lagrange 1, through its
[walk-in portal](#the-grids-portal-to-lagrange-1).

Plaza presence and Gym observation pause while loading or visiting a zone,
then resume the configured plaza behavior on return. Zones are local-only;
this does not join another relay or publish their coordinates as plaza movement.
Pairing and retained-chat grants remain separate. See
[zone architecture and limits](zones.md) and
[source mechanics and parity](ruins-source-parity.md).

## Use the world computer

The computer prompt is part of the 3D monitor. Its amber lettering and corner
marks are Rust-rendered geometry on the display surface. They change size and
perspective with the camera, and world geometry can obscure them. There is no
visible SwiftUI or Android **Computer** button floating above the object.

Tap the display itself. Rust traces that screen position into the world and
accepts it only when the nearby monitor is facing the camera and the path is
clear. Touch and motion camera modes use the same interaction. A drag, cancelled
touch, long hold, or second finger does not open the computer. Tapping the
monitor also avoids starting a movement or camera gesture.

VoiceOver and TalkBack expose a **Use computer** action on the world surface
when the computer is in reach. That action uses the same Rust proximity and
visibility checks without drawing another control over the scene.

### The computer's screen

On iOS, the computer opens its screen in the world HUD, drawn by Rust with the
same amber lettering as the map and zone controls. A leader line joins it to
the monitor, and the world stays visible above and behind it. While it is
open, every touch goes to the screen: tap a control, or drag to scroll. The
world does not move.

- **COMPUTERS** lists your linked computers with an honest status and the
  route in use (on this computer, local network, tailnet, public address, or
  relay). **Open** selects one and shows its rights, the workspaces it
  shares, **Order work**, **Terminal**, **Access**, and its recent work.
- **Add a computer** enrolls this phone with a one-use `coder-host:`
  invitation. **SCAN QR** opens the camera; **PASTE** opens the keyboard.
  Create the invitation on the computer with `coder host invite`.
- **Order work** picks a workspace the computer lists, takes a prompt from the
  keyboard, and sends it with NIP-HOST `task.create`. The screen moves to
  **Activity**, where the computer's redacted summaries show each task's
  phase and revision. **Steer** replaces an open task's instructions;
  **Stop task** asks the computer to stop it after you confirm. The computer
  records the task and runs it only under its own policy.
- **Terminal** needs the `terminal` right and opens the **TERMINAL** page for
  that computer. See [NIP-TERM on mobile, #9733](https://github.com/OpenAgentsInc/openagents/issues/9733).
- **CHATS** is the existing native page: `coder-pair:` pairing, the read-only
  chat reader, and the world connection under **…**. Its computer button
  returns to **COMPUTERS**.

Native code supplies only the keyboard, the camera scanner, and the Chats
page. VoiceOver reads each control on the screen by its label, and swiping
up or down with three fingers scrolls it. **CLOSE**, walking away, or moving
the app to the background ends the interaction. Android keeps its native
computer panel, which shows the same Rust Computers screens, including
ordering and following work. See the
[verification record](../coder/verification/2026-09-27-verse-computer-hud.md).

## Open a terminal on a linked computer

A computer linked to this phone with the **Open terminals** right can run a
shell here, over [NIP-TERM](../../nips/openagents/NIP-TERM.md). The terminal
draws in the world computer's screen, like **COMPUTERS**.

1. Tap the world computer's monitor, then **Open** on an online computer.
2. Choose **Terminal**. It is disabled, with the reason, when the computer is
   offline or this phone lacks the **Open terminals** right. The **TERMINAL**
   page opens.
3. The shell opens in the computer's first shared workspace, sized to the
   grid the page fits. Tap **KEYBOARD** and type. Return sends Enter and
   Backspace deletes. The page stays above the keyboard, and the shell sees
   the smaller grid.
4. The key row adds **Esc**, **Tab**, **Ctrl**, **Left**, **Up**, **Down**,
   **Right**, **^C**, and **Paste**. **Ctrl** applies to the next key only;
   **Ctrl on** shows it is latched. A hardware keyboard's arrows, Escape,
   Tab, and Control combinations also work.
5. Turn the phone to landscape for a wider terminal. Only the terminal page
   rotates; the shell sees each size change.

**Back** detaches and returns to **COMPUTERS**; the shell keeps running there
until it exits, you end it, or the computer ends it after its idle period.
**End terminal** ends it. If the route drops, the page says **Reconnecting**
and then shows what you missed. Output the computer discarded meanwhile
appears as an **[output lost: …]** line, never joined to the output around
it. Nothing you type while disconnected is queued. After the computer's host
restarts, the terminal shows **Lost**; choose **Open a new terminal**.

Rust owns the session and the [`coder-vt`](../../crates/coder-vt/README.md)
emulator (`coder-computers::terminal`), the terminal view, and every byte
sent; the HUD draws each grid row in monospaced cells and asks for the grid
its page fits. The iOS host forwards keystrokes and the clipboard and polls
the terminal while its page shows. Android shares the Rust, and its renderer
draws terminal rows monospaced, but it has no terminal keyboard or grid
sizing yet. See
the [verification record](../coder/verification/2026-09-27-mobile-terminal.md).

## Join another player

The public plaza is the default world connection. It publishes the phone's
Verse presence and positions over NIP-MV. The computer controls can change
the relay or choose **Leave**. A custom relay survives relaunch; an explicit
**Leave** keeps the app offline until you choose **Join** again. Synthetic
previews remain offline. A saved-preference read failure also stays offline
and shows a storage error rather than replacing the choice with the default. It uses
a separate protected device identity from the encrypted chat reader
(Keychain on iOS; Keystore-encrypted storage on Android). It
neither copies desktop account credentials nor reads the computer's chat grant.

Use the same reachable `wss://` relay URL on desktop and phone. Mobile refuses
credentials, queries, fragments, and plaintext remote connections. On join, it
attempts to recover only its own signed avatar state before publishing its first
position. Recovery has a 1.5-second deadline, including connection setup; a slow
relay falls back to a clear local spawn rather than blocking the render thread. A computer's loopback
address such as `ws://127.0.0.1:7447` refers to the phone itself when entered
there. Use a reachable secure WebSocket deployment with NIP-MV support and
appropriate event limits. The existing [local relay helper](README.md#multiplayer)
serves the desktop world; exposing it to a phone is an operator deployment task.

Mobile publishes moving poses every three seconds, idle poses every five
seconds, and durable movement state every thirty seconds. Those publication
intervals are independent of display callbacks. This reduces mobile bandwidth
and relay pressure; other
players see less frequent position samples than with the desktop's 10 Hz
moving-pose profile. A compatible wire format does not bypass a relay's
admission, authentication, or rate limits.

Backgrounding pauses rendering, clears held input, and cancels the world
connection. Returning starts a fresh motion session if a relay was selected.
Opening the computer stops player movement while keeping the world visible. Cancellation does not prove the relay received an offline state;
other clients must age out stale presence. Leaving the relay saves an explicit
offline choice. Before build 48, iOS deleted the setting when leaving, so a
legacy missing setting cannot distinguish a past Leave from first use; both
now use the public default. A new native mount starts a new local world.

## Presence in the OpenAgents app

The OpenAgents app's Verse tab shares avatar presence with other players in
its bare world. It reuses Coder's NIP-MV session (`verse::session::Session`),
relay link, crowd interpolation, and mobile cadence, and adds only what the
bare world needs:

- **Its own world.** The tab joins `verse-bare` (`verse::session::BARE_WORLD`)
  on `wss://relay.openagents.com`, not Coder's `verse-plaza`. The bare world
  has none of the plaza's buildings or collision, so its positions are not
  valid plaza positions, and NIP-MV gives each separately loaded coordinate
  space its own world identifier. Players in the OpenAgents app see each
  other; they do not see Coder's plaza, and Coder does not see them.
- **Presence only.** `Session::start_presence` subscribes to pose frames and
  entity states for that world and publishes the avatar's frames and states
  with no display name, and the shared ball and blocks
  ([Sharing the ball](#sharing-the-ball)) in those same frames and states.
  It subscribes to and publishes no chat, rooms, private messages,
  gestures, zone commands, profiles, or companion entity.
- **Its own identity.** A separate secp256k1 key in Keychain
  (`com.openagents.app.verse`, this device only, available while unlocked)
  signs world events. The device key that holds host grants never signs a
  world event, so players cannot link it to the world identity. If Keychain
  cannot provide the key, the world stays offline.
- **Cadence.** The tab publishes at Coder's mobile cadence: a pose frame every
  three seconds while moving and every five seconds at rest, and durable
  state on join and at most every thirty seconds while moving. While a
  player owns a moving body, frames go every 1.5 seconds. A presence
  session caps everything it publishes at 54 events in any minute
  (`verse::session::EVENT_BUDGET`), within the public relay's per-key
  event limit. On join, it
  restores its own saved position in the bare world within 1.5 seconds, or
  spawns at random.
- **Rendering.** Other players' avatars are drawn in the neutral palette,
  3.3 seconds in the past (one moving interval plus a margin for jitter).
  Each drawn position lies between two received poses, so avatars walk
  continuously instead of jumping at every frame, at the cost of that delay.
  An avatar with no frame for 10 seconds rests dim at its last saved state.
- **Collision.** Other players' avatars are solid where they are drawn: the
  player is a standing capsule 0.45 m in radius and 1.8 m tall, and walking
  into another avatar stops against it instead of passing through. An
  avatar that walks onto the player pushes the player aside along the
  ground, never into a wall. Coder's plaza applies the same rule; zones do
  not. The ball and the blocks still pass through other players.
- **Pausing.** Switching tabs or backgrounding the app deactivates the world,
  which closes the relay connection: nothing more is published or received,
  and remote avatars are cleared. Returning starts a fresh presence session.
  As in Coder, the offline state is best effort, so peers also age out
  stale presence.

The tab has no Join or Leave control yet; it always joins the public relay
while shown. The
[verification record](../../bins/openagents-ios/verification/2026-09-28-verse-presence/README.md)
covers loopback tests and a production-relay exchange with the simulator.

## Shared code and platform boundaries

| Component | Ownership |
| --- | --- |
| Validated `Surface` element, viewport, active/disposed lifecycle, frame timing | `crates/rust-native`; generic and independent of product crates |
| Seeded geometry, movement/collision, camera, gait, follower, render pipelines, monitor lettering, and world picking | `crates/verse`; shared desktop/iOS/Android implementation |
| Device touch interpretation, world state, connection choices, C bridge | `crates/coder-mobile` |
| GPU surface, display callback, native controls, protected identities, scene lifecycle | Thin SwiftUI/UIKit host in `bins/coder-ios`; Android framework host in `bins/coder-android` |
| Application and plaza palette | `crates/coder-ui`; outside Rust Native |
| Loaded-zone appearance, assets, and rules | `crates/verse::zones`; independent of the plaza palette and admitted only by the shared Rust host |
| Desktop model chat, retained benchmark-file discovery, verified XP ledger | Desktop feature dependencies; not loaded by the mobile world |

Desktop chat, public-feed panels, XP/quest inspection, and the local Microcoder
versus Fable replay picker remain desktop UI features. The portable replay
clock/track/landmark types remain shared, but this mobile delivery has no replay
artifact importer, XP trust configuration, or world-chat composer. The follower
moves and emotes without a model. Walking through Verse does not start a model or a benchmark. The Gym can
request an explicitly confirmed, host-configured recipe through its separate
execution grant; the chat reader remains read-only.

The `desktop` feature is enabled by default for the Verse executable. Both mobile
applications depend on `verse` with default features disabled: no desktop
harnesses, local Gym result store, knowledge store, or window event loop enter
those targets. The portable `gym-bridge` client observes a separately configured
host; its filesystem readers and execution host are feature-gated out of both mobile targets. Reader text, pairing controls, and other panels remain native platform widgets; on iOS, the computer's Computers and terminal screens are HUD geometry drawn by `coder-mobile`'s `computer_hud`.
The computer prompt is scene geometry; it uses neither native text widgets nor
the desktop glyph atlas.

## Verification and distribution

See the [iOS build guide](../../bins/coder-ios/README.md),
[Android build guide](../../bins/coder-android/README.md),
[iOS startup and gesture correction](../coder/verification/2026-09-26-ios-static-link.md),
[Android emulator verification](../coder/verification/2026-09-26-android-mobile.md),
[the Verse verification record](../coder/verification/2026-09-26-verse-mobile.md),
[QR pairing verification](../coder/verification/2026-09-26-world-pairing.md),
[Gym verification](../coder/verification/2026-09-26-verse-gym.md),
[full-screen and motion verification](../coder/verification/2026-09-26-fullscreen-motion.md),
[world-computer interaction verification](../coder/verification/2026-09-26-world-computer.md),
[issue #9698](https://github.com/OpenAgentsInc/openagents/issues/9698), and
[Verse-first pairing #9699](https://github.com/OpenAgentsInc/openagents/issues/9699).
Simulator rendering and lifecycle evidence are separate from physical-device
frame rate, thermals, and a two-device relay session. Only checks recorded in
the linked verification records have been performed.

## World connection

At the computer, open **CHATS**, then **…**, to view the world connection. **Join** saves the
selected relay on this device; the app reconnects when reopened. **Connected**
means the world subscriptions are accepted, including authentication when the
relay requests it. It does not mean other players are currently present.
Failures appear in the same panel. Joining or reconnecting keeps your current
position and leaves the panel open. A fresh app mount may restore your own
signed saved position from the relay.

**Leave** stops the connection and forgets the saved relay. World presence uses
its own identity and does not pair a chat reader. Synthetic acceptance tests
show **Preview** and publish no world events.

## Companion and demo gates

Tap your floating companion for a short wiggle and hop. Use the map's Spark
or Halo shortcut to approach a local demo gate, then turn to face it. Choose a
key in the nearby strip and tap the gate to see its effect and destination.
Tap again after the effect to walk there. Empty hands reuse that gate's last
compatible key; Reset clears its choice. Choices stay on this device, while
walks and effects stop when the app becomes inactive. See the
[interaction guide and portal specification](world-interactions.md).
