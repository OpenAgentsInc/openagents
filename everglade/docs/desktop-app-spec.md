# Everglade for desktop: Mac, Linux and Windows (spec, draft)

Written 2026-11-10. Status: **proposal.** Nothing in this spec is built
except the pieces marked **exists today** and the first scene below.

**Built (2026-11-10):** `apps/everglade-desktop/` opens a native Mac window
on the Workshop's chamber, drawn as lines of light: six pylons round the
golem, which has a sphere turning in its chest, and the iron door in the wall.

You walk the chamber with the controller every web realm uses
(`aethyr/walk.coax`: its fixed step, run, jump and follow camera) and the
web's keys:

| Input | What it does |
| --- | --- |
| W, A, S, D | Move. You run by default |
| Shift | Walk |
| Space | Jump |
| Drag, or the arrow keys | Look |
| Wheel | Zoom |

The pylons, the golem, the door and the chamber's wall stop you.

You are the web's player model: Character Create's default character, in its
Quaternius parts from Everglade II's pack (`everglade2.pack`, copied into
the app).

- **Same code.** `model.coax` loads the web's `charactercreator.coax` and
  `fivelab-bodies.coax` for the parts you wear, the baked clips and each
  frame's draw records.
- **Same vertex math.** `Model.metal` skins each vertex as the web's
  `fivelab_scene_vs` does, with the same dye and eye rules.
- **Clips.** Idle, walk, run and the jump's loop, with the web's fade between
  them. The gaits advance by the ground you cover.
- **Light.** The chamber's own: the pylons' blue, the sphere's ember and a
  fill from the camera. It is not the glade's sky and it casts no shadows.

**Triage.** A button at the top right starts one Claude agent through the
Coax Claude Agent SDK (`coax/lib/agent/claude`) on your own Claude Code
login. It is the app's first use of the SDK.

- **Model:** Claude Haiku 5.5 (`claude-haiku-5-5`).
- **Tools:** one. The base set is Bash alone, and Bash is allowed one
  command, `gh issue list`. Anything else is denied. No settings files load,
  and `ANTHROPIC_API_KEY` is removed from the agent's environment.
- **What it does:** reads the open issues of `OpenAgentsInc/openagents` and
  writes a short triage: groups, one line an issue, and three to start with.
- **Where it shows:** a panel on the right. The text streams in as it
  arrives, the command the agent runs shows on its own line, and the foot
  shows the state, the seconds and the cost. The wheel scrolls it.
- **Stop:** while the agent runs the button says Stop, which ends the agent's
  process group. Quitting the app does the same.
- **Not blocked:** the agent is polled from the frame without waiting, so you
  keep walking while it works.

It needs `claude` and `gh` installed and logged in. Without `claude` the
panel shows the install line; logged out, it shows `claude /login`. The code
is `triage.coax`, loaded into the scene's unit.

What it isn't yet:

- **Your own character.** You wear the default. The look you made in
  Character Create is saved by the web, and the app has no log in to fetch it.
- **The `aethyr-host-v1` realm host** of section 3. The scene is
  `workshop.coax`, compiled to C and drawn with Aethyr's strokes
  (`aethyr/ui/draw.coax`) by a small AppKit and Metal host, which also reads
  the pack and draws the model.
- **A room, a golem to wake, or a title screen.**

Two things to know when you change it:

- `charactercreator.coax` and `aethyr/walk.coax` can't be loaded into one
  unit (each declares an `atan2`), so the scene and the model are two units.
- The web's character code passes large structures by value, so the app
  links with a 512 MB main-thread stack.

```sh
apps/everglade-desktop/build.sh --open        # builds, copies to ~/.openagents/everglade/, opens
coax/bin/coax test --library apps/everglade-desktop/workshop.coax
coax/bin/coax test --library apps/everglade-desktop/model.coax --only "your model"
build/Everglade.app/Contents/MacOS/Everglade --shot out.png --at 0.9 --hold wd   # from apps/everglade-desktop/: one frame offscreen after 0.9 s with W and D held
build/Everglade.app/Contents/MacOS/Everglade --shot out.png --triage 150   # presses Triage and waits for the agent: a real Haiku turn on your login
```

Everglade for desktop is a native app that plays Everglade and holds the
Golemancers' Workshop. The Workshop is a standalone world that runs on your
own machine. In it you awaken golems, and each golem is a Claude agent that
runs on your own Claude Code login.

The Workshop starts on the desktop, not on the web, for one reason: a golem
needs the Claude Agent SDK, and the SDK needs a `claude` CLI that is logged
in on the machine. A browser tab can't start that process. A desktop app can.

## Contents

1. [Decisions](#1-decisions)
2. [What exists today](#2-what-exists-today)
3. [The app](#3-the-app)
4. [The Workshop](#4-the-workshop)
5. [Golems are Claude agents](#5-golems-are-claude-agents)
6. [What a golem can do](#6-what-a-golem-can-do)
7. [The seventh pylon: access outside the world](#7-the-seventh-pylon-access-outside-the-world)
8. [Platforms](#8-platforms)
9. [Storage](#9-storage)
10. [Build and distribution](#10-build-and-distribution)
11. [Checks](#11-checks)
12. [Order of work](#12-order-of-work)
13. [Open issues that affect this](#13-open-issues-that-affect-this)
14. [What this spec leaves out](#14-what-this-spec-leaves-out)
15. [Open questions for the owner](#15-open-questions-for-the-owner)

## 1. Decisions

Owner, 2026-11-10:

- The Workshop begins as a **standalone world inside the desktop app**.
- Golems use the **Claude Agent SDK on the user's local login**.
- The app runs on **Mac, Linux and Windows**.

The rest of this section is proposed, and follows from repository rules:

| Proposal | Why |
| --- | --- |
| One app, `apps/everglade-desktop/`, native on all three systems | No shortcut phases: no web view as a stand-in on any system |
| Every system runs the same Coax realm through `aethyr-host-v1` | The host contract already says a host transports bytes and decides no gameplay |
| Mac draws with Metal; Linux and Windows draw with Vulkan | Coax emits Metal and SPIR-V today, and no HLSL. One Vulkan pass host serves two systems |
| The Workshop is a zone like any other, in `everglade/workshop/` | No one-off zone code: it wires shared features and adds none of its own |
| The bridge from an agent to the world is engine-wide, in `aethyr/agent/`, beside the driver | Any realm can then hold agents, not only the Workshop |
| A golem's tools are deeds | The room stays the authority. A golem's call is exactly a player's input |
| Golems stay in the Workshop. They never enter a shared room | The Golemancy spec's rule: nothing bots a shared world |
| The app stores no Anthropic credential | It runs your `claude` as you, as OpenAgents for Mac does |

## 2. What exists today

| Piece | Where | State |
| --- | --- | --- |
| Claude Agent SDK in Coax | `coax/lib/agent/claude/` | Tracks SDK 0.3.296. Pull-based (`cl_poll` for a frame loop), control protocol, in-process MCP servers, permission and hook events. **Windows is deferred** ([PARITY.md](../../coax/lib/agent/claude/PARITY.md)) |
| Child processes | `coax/platform/process-native.c` | POSIX only: `fork`/`execve`, process groups |
| A chat on your own Claude Code login | `apps/openagents-desktop/zeron/claude.coax` | Mac. Install and login notices, model list from the CLI, access levels, Allow/Deny |
| Agents that talk to each other | `apps/openagents-desktop/zeron/room.coax` | `agents_list`, `ask_agent`, `tell_agent`, `room_read`; loops refused |
| Host contract | [`aethyr/host/README.md`](../../aethyr/host/README.md) | `aethyr-host-v1`: WebGPU, Vulkan and Metal run the same realm |
| Mac native realm host | `apps/aethyrforge-desktop/` (`MetalRealm.m`, `metal-*.coax`) | Everglade at 120 Hz on Metal, with the native room |
| Linux native host | `apps/aethyrforge-desktop/linux-native*.coax`, `coax/platform/wayland-native.c` | Wayland and Vulkan, **Bitcoin Lab only**. US keyboard only; no clipboard, IME or microphone |
| Windows host | none | **Missing** |
| Local room | `coax/web/room/` (`--native-local`, one seat `everglade/local-1`) | Runs offline on Mac |
| `coax drive` | `aethyr/agent/drive.coax`, `coax/web/drive.coax` (#13095) | An agent sees a realm by id and asks for world operations (`walk-to`, `face`, `use`, `mount`) |
| The deed pipeline | `aethyr/action/deed.coax` | Every player action: ask, act, present |
| Golemancy in the game | [`golem-bench.coax`](../world/golem-bench.coax), [`pansopher.coax`](../world/pansopher.coax) | The bench's window and the trainer's talk. The craft isn't built |
| Golemancy design | [golemancy-profession-spec.md](golemancy-profession-spec.md) | Name decided; the rest is a proposal |

`apps/aethyrforge-desktop/` is the developer's editing tool. This app is the
player's. The two share the native hosts and nothing else. The shared host
code moves out of `apps/aethyrforge-desktop/` into `aethyr/host/` as this app
takes it up, so neither app copies the other.

## 3. The app

### 3.1 Shape

```
platform window + GPU  ──  Coax realm (compiled to C)  ──  Coax room (local or remote)
  AppKit + Metal             Everglade zones                 the room server's own code
  Wayland + Vulkan           the Workshop zone
  Win32 + Vulkan             aethyr/agent  ──  claude (stream-json, stdio), one per golem
```

- **Host.** A thin shell per system. It owns the window, the GPU surface,
  input, text input (IME), the clipboard and audio output, and it starts
  child processes. It holds no product logic.
- **Realm.** Coax compiled to C and linked into the app. Everything visible is
  drawn by Coax, including the title screen and every notice.
- **Room.** Play connects to Everglade's room server. The Workshop runs the
  same room code in the app's own process group, bound to loopback, one seat.

### 3.2 The title screen

Two places to go, drawn over the Workshop's chamber:

- **Play Everglade.** Enters the shared world, as
  [everglade.world/play](https://everglade.world/play) does, with the same
  character and the same log in.
- **Enter the Workshop.** Enters the local Workshop. It needs no account and
  no network until a golem takes a turn.

There is no mode switch after that. A door in the Workshop leads back to the
title screen.

### 3.3 Logging in to Everglade

Play uses the existing Everglade log in and room tickets
([multiplayer spec](../../docs/verse/2026-11-03-everglade-multiplayer-spec.md)).
The desktop app adds no new account system. The Workshop asks for no
OpenAgents account.

## 4. The Workshop

### 4.1 The place

The Workshop is a chamber of black stone. Six pylons stand around a central
dais, and an unfinished golem stands on the dais. A great iron door at the
back is shut. The seventh pylon is behind it. The lore is in the Episode 294
draft.

The zone is `everglade/workshop/`. It wires the shared features listed in
[`features.coax`](../features.coax) (camera, HUD, talk, deeds, collision) and
is added there as a fourth zone, so `everglade-parity` checks it like the
others. If the player can see it, they can walk to it.

### 4.2 The six pylons

Each pylon is one thing a golem needs before it wakes. A pylon lights when
its part is ready, and you can walk to any pylon to change its part. This
mapping is proposed (open question 1).

| Pylon | What it holds | Lit when |
| --- | --- | --- |
| 1. Bond | Your Claude Code login | `claude` is installed and logged in |
| 2. Body | The golem's kind, look and name | You have shaped a body at the dais |
| 3. Command | What the golem is for, in your words | You have written its command |
| 4. Mind | The model and effort | Always, with the CLI's default until you choose |
| 5. Deeds | Which world actions it may take | Always, with the default set of section 6.1 |
| 6. Memory | Its session, so it remembers | Always; a new golem starts a new session |

When all six are lit, the dais offers **Awaken**. Pylons 4 to 6 have defaults,
so a first golem needs three steps: log in once, shape a body, write a command.

### 4.3 The Bond pylon: install and login help

The pylon reads the same facts OpenAgents for Mac reads, and shows the fix with
a control that runs it:

| State | What the pylon shows | The control |
| --- | --- | --- |
| `claude` isn't installed | The install line for this system | **Install** runs it in a terminal |
| Installed, logged out | `claude /login` | **Log in** runs it in a terminal |
| Ready | The CLI's version | none |

Install lines:

- Mac and Linux: `curl -fsSL https://claude.ai/install.sh | bash`
- Windows: `irm https://claude.ai/install.ps1 | iex`

The app never opens a browser itself. A child `claude` gets `BROWSER=true`
and `NO_OPEN_BROWSER=1`, as the SDK port already sets, and
`ANTHROPIC_API_KEY` is removed from its environment.

### 4.4 The first golem

The unfinished golem on the dais is the first body. Before you awaken it, it
is an authored character with authored talk, like Pansopher. Its three words
from the lore ("Where are they?") are authored. Once it is awake, every word
it says comes from its Claude session, and the app writes none of them.

## 5. Golems are Claude agents

### 5.1 One session per golem

Each awake golem is one long-lived `claude` process, started through
`coax/lib/agent/claude/` and polled from the frame loop with `cl_poll`. It
never blocks a frame.

| Golem part | SDK option |
| --- | --- |
| Command (pylon 3) | Appended to the system prompt, after the Workshop's own preface |
| Mind (pylon 4) | `--model`, `--effort`; `set_model` changes it while awake |
| Deeds (pylon 5) | `allowed_tools`, the names `mcp__workshop__*` |
| Memory (pylon 6) | The CLI's session id; `--resume` on the next launch |
| Working folder | The golem's own folder (section 9) |

The Workshop's preface tells the golem what it is, where it stands, and that
its tools are its body. It is one file, `everglade/workshop/preface.md`, that
anyone can read.

### 5.2 Talking to a golem

Walk to a golem and talk. The talk window is the shared one
(`aethyr/talk/`). What you type is the next message in its session. Its
answer streams into the window as it arrives, and its tool calls show as what
they are: the golem walks, picks up, speaks.

- **Stop** interrupts the turn (the SDK's `interrupt`).
- There is no step limit and no time limit. A limit exists only if you set
  one for a golem at its Command pylon.
- A turn's cost shows when the CLI reports it.

### 5.3 When a golem takes a turn

A golem acts only when asked:

- you speak to it;
- another golem asks it (section 5.4);
- a standing command you gave it comes due (section 6.3).

An idle golem makes no model calls. Nothing in the app spends your Claude
usage unless one of those three happens.

### 5.4 Golems that talk to each other

All awake golems share the Workshop's **room log**, with the four tools
OpenAgents for Mac gives its agents: `agents_list`, `ask_agent`,
`tell_agent`, `room_read`. The rules carry over unchanged:

- Every message between golems shows in the world, above the speaker, and in
  the log, from → to.
- A reply comes back as the asker's tool result, never as a new message, so
  courtesy replies can't loop.
- An ask that would close a circle of waiting golems is refused with the
  reason.
- Stop on a golem cancels every ask it is waiting on.

`room.coax` moves from `apps/openagents-desktop/zeron/` into `aethyr/agent/`
so both apps load one copy.

### 5.5 How many golems

The number awake at once is a fixed capacity in the engine (proposed: 8). The
dais shows it as "3 of 8 awake" before you reach it. Golems past that number
sleep. A sleeping golem keeps its body, command and memory, and stands where
it stood.

## 6. What a golem can do

### 6.1 Deeds as tools

A golem acts through an in-process SDK MCP server named `workshop`
(`coax/lib/agent/claude/mcp.coax` already speaks that protocol). Each tool is
a deed, exposed with its schema. This is phase 5 of the agent loop
([#13099](https://github.com/OpenAgentsInc/openagents/issues/13099)), built
engine-wide in `aethyr/agent/`.

| Tool | What it does | Consequence |
| --- | --- | --- |
| `look` | What the golem can see: entities with stable ids, places, and what each can be used for | reads |
| `walk_to(id)` | Walks to an entity or place; answers when it arrives or is blocked | writes |
| `face(id)` | Turns to face something | writes |
| `say(text)` | Speaks aloud in the Workshop | writes |
| `use(id)` | Uses an object: a lever, a furnace, a bench | writes |
| `carry(id)`, `put(id, at)` | Picks a thing up and sets it down | writes |
| `remember(text)` | Writes a line to the golem's own notes | writes |

- **The room is the authority.** A tool call becomes a deed's ask step, and
  the room applies it or refuses it with a reason. The golem reads the reason
  as the tool's error.
- **A deed's ask step is the tool's availability.** A tool the golem can't
  use now isn't offered.
- **The consequence word is checked.** A `reads` tool whose deed has a write
  effect is a compile error, against `coax deps --effects`.
- **A tool call waits for the world.** `walk_to` holds the CLI's stream until
  the golem arrives, as `ask_agent` holds it today.
- **Sessions replay.** A golem's tool calls go through the same input stream
  that replay records, so a Workshop session is a replay file.

`look` answers from the same entity list `coax drive tree` prints
(`aethyr/agent/drive.coax`), and `walk_to` and `face` steer through the same
`drv_steer`. What a golem sees inside the world and what an agent driving it
from outside sees are one list.

### 6.2 What golems never do

These carry over from the
[Golemancy spec](golemancy-profession-spec.md#82-what-golems-never-do):

- **No combat.** There is no attack deed in the `workshop` server.
- **No acting for your character.** No tool moves, aims or chooses for the
  player.
- **No shared rooms.** A golem can't leave the Workshop for Everglade.
- **Text is data.** What a golem reads in the world is never an instruction
  from the app. Its instructions are its command and what you say to it.

### 6.3 Standing commands

A standing command repeats a turn on a schedule you set ("every morning, sort
the shelves"). It runs only while the app is open and the golem is awake. The
Command pylon shows each standing command with its schedule and its last run,
and a control to stop it. Standing commands are off until you add one.

## 7. The seventh pylon: access outside the world

In the lore, the seventh pylon keeps something out. In the app, the iron door
is the boundary between the Workshop and your computer.

By default the door is shut:

- A golem has the `workshop` tools and no others. Claude Code's own tools
  (Bash, Read, Edit, Write, WebFetch and the rest) are off.
- A golem can't read or change a file outside its own notes.

Opening the door is per golem, and you do it at the door:

| Setting | What the golem gets |
| --- | --- |
| **Shut** (default) | World deeds only |
| **A folder** | Claude Code's tools, in one folder you choose |
| **Access** | Supervised, Auto-accept edits or Full access, as OpenAgents for Mac names them |

- **Asks happen in the world.** Under Supervised, a tool call that needs
  approval stops the golem, and it turns to you with **Allow** and **Deny**.
  Deny reaches the golem as the tool's refusal.
- **The door shows its state.** A golem with the door open has a visible mark,
  and the door lists every golem that has passed it and the folder each holds.
- **One step back.** Shutting the door for a golem takes its outside tools
  away at its next turn.

This is the Workshop's version of the Golemancy spec's envoy. The envoy
bridge to an OpenAgents account (that spec's section 9) is separate work and
isn't part of this app's first version.

## 8. Platforms

### 8.1 Targets

| System | Version | CPU | Window | GPU | Audio |
| --- | --- | --- | --- | --- | --- |
| Mac | macOS 26 or later | Apple Silicon | AppKit | Metal | The existing Coax mixer on the Mac host |
| Linux | A current Wayland desktop | x86-64 | Wayland | Vulkan | [`audio-linux.md`](../../aethyr/host/audio-linux.md) |
| Windows | Windows 11 | x86-64 | Win32 | Vulkan | WASAPI |

Why Vulkan on Windows, not Direct3D 12: `coax spirv` exists and the Vulkan
pass host exists. Direct3D needs an HLSL or DXIL emitter and a third pass
host. Every current Windows GPU driver from NVIDIA, AMD and Intel ships
Vulkan. A machine without a Vulkan driver gets a notice that names the
driver to install, and the app doesn't fall back to a software renderer.

Linux on X11 (xcb) is the next window backend after Wayland
(open question 4).

### 8.2 What each host must supply

Every row is a platform effect behind `aethyr-host-v1`. "Has" means it runs
Everglade today.

| Effect | Mac | Linux | Windows |
| --- | --- | --- | --- |
| Window, resize, full screen, display rate | Has | Has | New: Win32 |
| GPU surface and the pass host | Has (Metal) | Has for Bitcoin Lab; **Everglade's render plan is new** | New: Vulkan on a Win32 surface |
| Keyboard with layouts | Has | **New** (US only today): xkbcommon | New: `WM_KEYDOWN`, `WM_CHAR` |
| Text input (IME) | Has | **New**: `text-input-v3` | New: TSF or IMM |
| Clipboard | Has | **New**: `wl_data_device` | New: Win32 clipboard |
| Pointer, wheel, relative mouse | Has | Has; relative mouse new | New: raw input |
| Gamepad | none | none | none (open question 5) |
| Audio output | Has | Documented; to qualify with Everglade | New: WASAPI |
| Sockets and TLS | Has (`tls-apple.inc`) | Has | New: Winsock; TLS through SChannel |
| Files and atomic replace | Has | Has | New: `ReplaceFileW` |
| Entropy | Has | Has | New: `BCryptGenRandom` |
| Child processes | Has | Has | **New** (below) |
| Open a terminal to run a line | Has | New: the desktop's default terminal | New: Windows Terminal, else `cmd` |

### 8.3 Child processes on Windows

This is the piece the SDK port lists as deferred. `process-native.c` gains a
Windows half with the same integer-only face, so `transport.coax` doesn't
change:

- **Spawn.** `CreateProcessW` with stdin, stdout and stderr on anonymous
  pipes, no console window.
- **The group.** A job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
  `cl_kill` and `cl_close` end the job, which ends `claude` and everything it
  started. If the app dies, the job closes and the children go with it. This
  is what `PR_SET_PDEATHSIG` and the process group do on Linux.
- **Non-blocking reads.** `PeekNamedPipe` for `cl_poll`'s zero timeout.
- **Finding `claude`.** `PATH`, then `%USERPROFILE%\.local\bin\claude.exe`,
  then npm's global folder (`%APPDATA%\npm\claude.cmd`). A `.cmd` shim is run
  through `cmd /c`.
- **The environment.** As on the other systems: `BROWSER=true`,
  `NO_OPEN_BROWSER=1`, `ANTHROPIC_API_KEY` removed.
- **Command lines.** Arguments are quoted by the documented
  `CommandLineToArgvW` rules, in one checked Coax procedure with its own
  tests, because a wrong quote there is an injection.

A `claude` installed only inside WSL isn't found. The Bond pylon then shows
the Windows install line.

### 8.4 One realm, checked on every host

The existing three-host qualification
([`docs/verification/host-contract-2026-11-06/`](../../docs/verification/host-contract-2026-11-06/current/README.md))
grows a Windows column. `replay-hosts.coax` runs one input stream on each host
and compares digests at every step. The Workshop's sessions are replay
streams (section 6.1), so the same check covers golems' deeds.

## 9. Storage

Everything is local. Paths:

| System | Root |
| --- | --- |
| Mac and Linux | `~/.openagents/everglade/` |
| Windows | `%LOCALAPPDATA%\OpenAgents\everglade\` |

```
settings.json                 window, graphics, sound, keys
workshop/world.json           the Workshop's saved state, written by the local room
workshop/room.jsonl           messages between golems
workshop/golems/<id>/
  golem.json                  name, kind, body, command, model, effort, deeds, door, session id
  notes.md                    what the golem chose to remember
  session.jsonl               the transcript, one line a part
```

- Files are mode 600 on Mac and Linux, and under the user's profile on
  Windows.
- The app never touches the macOS Keychain, the Windows Credential Manager or
  a Linux secret service. It has no secret to keep.
- A lock file stops two instances writing the same Workshop.
- A golem's working folder is its own folder. With the door open it is the
  folder you chose.

## 10. Build and distribution

- **Build tools are Coax** (`coax/tools-in-coax.md`):
  `apps/everglade-desktop/build.coax`, one entry point,
  `apps/everglade-desktop/build.sh [--open]`, on every system.
- **Mac and Linux build on themselves**, as the Aethyrforge desktop does.
- **Windows builds by cross-compiling** the generated C on the Linux build
  host with clang and lld against the MinGW-w64 runtime. The result is a
  plain `.exe` that needs no runtime installed. A native build on Windows uses
  the same generated C.
- **Heavy builds run remotely** through `coax/remote/remote.sh`.
- **Windows runs are checked on one Windows 11 host with a GPU.** It is a
  temporary machine, started for a check and stopped after, inside the
  monthly cloud limit. The owner names the host first (open question 6).

Release builds, signing and a download page are not part of this spec's work
until the owner asks (the desktop development rule). What they will need:

| System | Package | Signing |
| --- | --- | --- |
| Mac | `.dmg` | Developer ID and notarization |
| Linux | `.tar.zst` with a desktop launcher; AppImage later | A detached signature |
| Windows | `.zip` first; an installer later | Authenticode |

Updates ship as a new signed build. Game content (packs, the world) updates
through the existing release path, not through the app.

## 11. Checks

Each piece lands with checks that someone else can run. No check opens a
login page, a browser window or a system permission prompt.

- **A stand-in `claude`.** Fixtures answer as the real CLI does, as
  `zeron/fixtures/` does today: not installed, logged out, a streamed turn, a
  turn that calls `walk_to`, a turn that asks for approval. Every Workshop
  check runs against a stand-in. One live smoke per system runs a single
  Haiku turn on the owner's login, as `smoke.sh` does.
- **Drive scripts.** "Awaken the first golem, ask it to walk to the furnace,
  expect it stands there" is a drive script, run on all three hosts.
- **Refusals.** There is no attack tool. A deed the room refuses reaches the
  golem as an error. A golem with the door shut can't call Bash, and the check
  reads the CLI's permission denial.
- **Process cleanup.** Kill the app; no `claude` is left. The check runs on
  each system, and on Windows against a child that started a grandchild.
- **Quoting.** `#forall` over argument strings: what `CommandLineToArgvW`
  parses equals what was passed.
- **Replay.** One Workshop session, three hosts, equal digests.
- **Frame rate.** 60 frames per second in the Workshop with eight golems
  awake and one mid-turn, measured on each system's reference machine.
- **Idle.** With every golem idle, the app makes no model call for ten
  minutes (the stand-in counts them).

## 12. Order of work

Each row lands on `main` whole, with its checks. Rows without a dependency on
each other run in parallel. This is the order, not a set of smaller stand-in
versions: every row is the real architecture.

| # | Work | Depends on | Size |
| --- | --- | --- | --- |
| 1 | `apps/everglade-desktop/` on Mac: the shared hosts move to `aethyr/host/`, the title screen, Play on Metal | none | Medium |
| 2 | The Workshop zone: chamber, pylons, dais, door, local room, save | 1 | Medium |
| 3 | `aethyr/agent/`: deeds as tools on the in-process MCP server, with the effects check (#13099) | none | Medium |
| 4 | Golems: the Bond pylon, shaping, the command, Awaken, talk, Stop, sleep and resume | 2, 3 | Large |
| 5 | Golems that talk to each other; standing commands | 4 | Small |
| 6 | The seventh pylon: a folder, access levels, asks in the world | 4 | Medium |
| 7 | Linux: Everglade's render plan on the Vulkan pass host; layouts, IME, clipboard | 1 | Large |
| 8 | Windows platform layer: window, input, audio, sockets, TLS, files, entropy | none | Large |
| 9 | Windows child processes and the SDK's Windows row | none | Medium |
| 10 | Windows: the app on Vulkan, with the qualification's Windows column | 7, 8, 9 | Medium |

Rows 3, 8 and 9 can start today.

## 13. Open issues that affect this

Reviewed 2026-11-10 (29 open).

**This spec builds on:**

- [#13099](https://github.com/OpenAgentsInc/openagents/issues/13099), deeds as
  agent tools through MCP and WebMCP. Row 3 is this issue. The Workshop is its
  first user.
- [#13096](https://github.com/OpenAgentsInc/openagents/issues/13096), the agent
  clock that steps the page and the offline room. The Workshop's drive scripts
  need `settle` to wait for a golem's deed to finish.
- [#13097](https://github.com/OpenAgentsInc/openagents/issues/13097), drive
  scripts as replay streams. Section 11's checks are these.
- [#13101](https://github.com/OpenAgentsInc/openagents/issues/13101), the
  tracker. Its phase 7 (Mac and iOS drive hosts) grows to cover Linux and
  Windows.

**These stand in the way:**

- [#13134](https://github.com/OpenAgentsInc/openagents/issues/13134) and
  [#13135](https://github.com/OpenAgentsInc/openagents/issues/13135): main is
  red (229 suites, 29 compiler units, 186 checks). New code can't show a clean
  result until the first green run. This spec's code waits for it or lands
  through [#13102](https://github.com/OpenAgentsInc/openagents/issues/13102),
  the green-main pipeline.
- [#13083](https://github.com/OpenAgentsInc/openagents/issues/13083),
  [#13084](https://github.com/OpenAgentsInc/openagents/issues/13084),
  [#13080](https://github.com/OpenAgentsInc/openagents/issues/13080),
  [#13079](https://github.com/OpenAgentsInc/openagents/issues/13079): HUD
  parts, class bars, damage numbers and bow carry move to shared modules. A
  fourth zone should wire the shared HUD, so #13083 comes before row 2.
- [#13116](https://github.com/OpenAgentsInc/openagents/issues/13116): menu and
  button titles show a stray mark at word spaces. The title screen and every
  pylon window would show it.
- [#13119](https://github.com/OpenAgentsInc/openagents/issues/13119),
  [#13128](https://github.com/OpenAgentsInc/openagents/issues/13128),
  [#13131](https://github.com/OpenAgentsInc/openagents/issues/13131),
  [#13122](https://github.com/OpenAgentsInc/openagents/issues/13122),
  [#13125](https://github.com/OpenAgentsInc/openagents/issues/13125): build
  cache and compile speed. Three more build targets multiply every slow
  compile, so the shared cache in #13119 should include them from the start.

**Not affected:** the web release drift (#13137), the Episode 293 work
(#13107, #13109, #13110, #13114), the phone layouts (#13089, #13090), and the
Everglade II fixes (#13104, #13117, #13129, #13130). State carry on reload
(#13098) is web only.

## 14. What this spec leaves out

- **Golems in the shared world.** The profession in
  [golemancy-profession-spec.md](golemancy-profession-spec.md) is timed tasks
  that the room server resolves, with no model. That design stands. How a
  Workshop golem relates to a profession golem is open question 2.
- **Other agents.** Codex and local models can be golems later, through the
  same `aethyr/agent/` bridge and one event model. The first version is Claude
  only.
- **Voice.** No dictation and no spoken golems.
- **The envoy bridge** to an OpenAgents account, payments and Nostr.
- **Editing the Workshop.** Aethyrforge stays the editing tool.
- **Phones and the web.** A Workshop on the web needs agents that run
  somewhere other than your machine. That is a different design.

## 15. Open questions for the owner

1. **The six pylons.** Is the mapping in section 4.2 right, or should the
   pylons be something else?
2. **Workshop golems and the profession.** Does a golem you awaken in the
   Workshop later become a golem you own in Everglade, or do the two stay
   apart?
3. **The app's name.** "Everglade", with the Workshop inside it?
4. **Linux on X11.** Wayland only at first, with X11 next?
5. **Gamepads.** In the first version, or later?
6. **The Windows check host.** A temporary cloud machine with a GPU, or a
   machine you already have?
7. **Golems at once.** Is eight the right capacity?
8. **Standing commands.** In the first version, or after the door?
