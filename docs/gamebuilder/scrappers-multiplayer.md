# Scrappers multiplayer

## What is built (2026-11-16)

A first version is in the app. It is the two-player stand-in this plan
called "the honest shortcut", made wider: up to four people, online, with
a party and a quick match. It was written the same day and has passed its
checks (`coax/net.coax`, `coax/party.coax`), but when this was written no
two devices had played each other. The plan below, for the shared room
core, still stands as where this goes next.

### How people get into a match

- **Party.** In the lobby, tap someone and choose "Invite to play
  Scrappers". They see a Join / No banner over whatever they are doing. The
  person who invited leads. The leader sets each member to "Your team" or
  "Other team" and presses Start match. A party is the leader and up to
  three others.
- **Quick match.** A button in the lobby. Two people who are both looking
  are put in a party of two, on opposite teams, and the match starts
  without anyone being asked. Of the two, the one whose key is lower does
  the inviting.
- **Scrapper.** Each person picks theirs in the lobby's party card.
- **Mode and size.** The leader picks the mode (Knockouts, Last Stand,
  Scrapball, Hoops) and how many a side (1v1 to 5v5) in the party card;
  members see what was picked. The party's state words carry both
  (`,mode,size` at the end). Start match is refused while the party does
  not fit the size, and members are seated on their side, or the other
  side when theirs is full. The match snapshot carries the mode, size,
  round, pause, banner and the ball (where it is, how it moves, who holds
  it), so a guest's screen shows the same goals, hoops and score; a 5v5
  snapshot with twenty shots still fits one frame.
- Seats nobody holds are bots. A guest who goes quiet for six seconds
  becomes a bot. If the leader leaves, the match ends for everyone.
- Logging in is required to invite, join or look.

### How it is carried

Two connections, as Everglade has two (its API and its room):

| What | Where | Why |
| --- | --- | --- |
| Invitations, who is in the party, sides, "start", "the leader left" | The lobby's Nostr relay, `wss://relay.openagents.com`, as signed short-lived events (kind 25910) addressed to the people they are for | The lobby already knows people by their Nostr keys, these words are few, and a signature says who sent each |
| The match itself | The room server Everglade plays on, in one of its relay rooms: `wss://openagents.com/api/room/relay/scrappers/<party>` | These words are many and must be quick. A relay room passes every text frame to the others, reads nothing, and needs no server change for a game with up to four seats |

Everglade's own play does not go through the Nostr relay either; it goes
through the room server, which runs its world. Scrappers does not have a
world on the server yet, so the leader's device is the host.

### Who decides what

- **The host** (the leader's device) runs the whole match and alone decides
  hits, health, knockouts, packs, the clock and the score. Twelve times a
  second it tells everyone how the match stands.
- **A guest** moves its own Scrapper on its own device, so its feet answer
  at once, and fifteen times a second tells the host where it is, where it
  is aiming and whether it is firing. The shot is the host's to make. On a
  guest's screen the match is turned about: they are always "you", and
  their team is always the blue one.

### What this version does not do

- **More than four people.** The relay room seats four and a party is four
  (`PT_MEMBERS`). 5v5 can be picked; the seats nobody holds are bots. Ten
  people needs the room server's seat count raised, a deploy, and
  `PT_MEMBERS` raised to ten (the match already has ten fighters).
- **A second arena and a map vote.** Only one arena ships. A vote needs
  more arenas built in the core beside `ed_arena` (an `ed_open_arena(e, n)`),
  the host opening the winning one instead of the game's own level, each
  member's vote carried in the party words with the leader counting them,
  and the chosen arena in the state words so every device builds the same
  walls.
- **A public server.** Matches are among people who invited each other or
  quick-matched. Open matches with strangers need rooms and matchmaking on
  the server (the room server running the match, not a device), trust and
  anti-cheat, more seats, and a child-safety review like publishing's.
- **The owner's edited arena.** Everyone plays the arena as it ships, so
  every device has the same walls. Sending the host's level is the next
  step, and is what lets two owners play the level they are building.
- **Trust.** The room takes the word of whoever knows the party's name, and
  a guest's device says where the guest is. Fine for people who invited
  each other; not for strangers who would cheat.
- **A fair fight at a distance.** A guest's shots are judged where the host
  sees things, a moment later. There is no rewind.
- **Rejoining.** A guest who drops is a bot for the rest of that match.
- **Chat in the match.** None.
- **Children and strangers.** Quick match puts a child in a match with
  whoever else is looking. There is no chat in it, but this needs the same
  read as publishing before it is open to everyone.

### Moving to the shared core

The match's two messages (`net.coax`) are the same commands and snapshot a
realm on the room server would carry. Moving there means the room runs
`play.coax` in place of the leader's device: steps 1 to 3 of the plan
below. The party, invitations and quick match stay as they are.

---

# The plan: Scrappers on the shared multiplayer core

Written 2026-11-16 against openagents `42092fc2e9`. Status: a plan. Nothing
here is built. It covers the arena template in `coax/play.coax`: three
Scrappers a side, today one person and five bots.

## The short answer

**Yes: Scrappers multiplayer should be built on the one multiplayer core
that Everglade uses, and should improve that core, not sit beside it.** A
first draft of this plan gave Scrappers its own small protocol. That was
wrong for the goal, which is one solid core any game can use, from two
people in a room up to an MMO. This version replaces it.

## What is asked

1. Two people play one Scrappers match, each on their own device, on the
   same team or on opposite teams.
2. The way it is built is the shared multiplayer core, used by every game,
   including MMOs.

## The core that already exists

It is in openagents, open source, and in production under Everglade.
Paths are in the pinned tree.

| Piece | Where | What it does |
| --- | --- | --- |
| Networking contracts | `metaverse-engine/net/` (`README.md`) | A realm keeps its own world, rules and codec; these are the shared interfaces round them |
| Timing and trust | `net/profile.coax`, `net/realm.coax` | A realm declares its tick rate, substeps, who is the authority, what is durable |
| Sessions | `metaverse-engine/session/lifecycle.coax`, `net_session_*` | Admission, connection generations, reconnect holds |
| Commands | `net/command.coax` | Inputs from a player: credit, catch-up, acknowledgement after the step |
| Replication | `net/replication.coax`, `net/row.coax` | What each recipient is told: snapshots or deltas, baselines, removal, per-recipient fields |
| Scale | `net/grid.coax`, `net/budget.coax`, `net/spawn.coax` | Interest by spatial cell, a byte budget per seat with priorities, reliable spawn and despawn |
| Client feel | `net/predict.coax`, `net/interp.coax`, `net/rewind.coax`, `net/shots.coax` | Predicting your own body, smoothing others, judging hits fairly |
| Transport | `net/transport.coax` | One interface between a client and its room, with five channels (unreliable datagrams, and reliable streams for combat, loot, chat and session). Two implementations today: WebSocket and WebTransport |
| The room | `coax/web/room/` | The authoritative server process. It hosts realms by profile |
| The World | `metaverse-engine/core/world.coax` | One entity store used by the client, the room and the offline worker (decision 0007) |

Two facts matter most:

- **The room already hosts two different games.** Realm 1 is Everglade:
  30 Hz, durable characters, interest management, built toward 150 players
  a zone (decision 0003). Realm 2 is Orbit League v2: 60 Hz, authoritative,
  whole snapshots, bots filling seats, nothing durable, a disconnect ends
  the seat. The README's table shows one interface serving both.
- **Orbit League's profile is Scrappers' profile.** A short arena match, a
  handful of bodies, bots, no saved characters. Scrappers is not a new
  kind of consumer; it is a third realm of a kind the core already serves.

So the plan is: **Scrappers becomes realm 3 on this core.**

## What Scrappers adds to the core

Scrappers brings two things the core does not have yet. Both are general,
both go into openagents, and both are things the other games need.

### 1. The room running on a device

Today the room is a server process, plus a one-seat build for the browser
(`coax/web/room/worker.coax`). Nearby play needs the room to run inside the
app on the host's device. That is the same piece the Everglade plan needs
for offline play, and the same piece
SPLICEBOTS would use for a companion player. Build it once:

- the room core linked into a native app and stepped in-process;
- no database, no HTTP, no directory: the realm's profile already says
  "nothing durable" for games like this.

### 2. Two more transports

`net/transport.coax` is one interface with two implementations. Add two:

| Transport | Carries | For |
| --- | --- | --- |
| Memory | Bytes between a client and a room in the same process | Single-player and offline play of any multiplayer realm. Bots and one human are just a room with one seat |
| Nearby | Bytes between devices over the local network or Bluetooth (Apple's MultipeerConnectivity on iOS and macOS) | Two people in a room, no internet, no account |

The channels map directly: datagrams to unreliable sends, the four reliable
channels to reliable sends. The realm's frames do not change with the
transport, which is the point of the interface.

With those two, every game on the core gets the same three ways to play
from one codebase: **alone** (memory), **nearby** (nearby transport, the
host's device runs the room), **online** (WebSocket or WebTransport to a
room server).

## What changes in Scrappers

`coax/play.coax` today is one struct stepped by one person's inputs. To be
a realm on the core:

1. **Move the match onto the World.** Fighters and shots become entities in
   a `core/world.coax` World with a system list (input, movement, bots,
   shots, rules), as decision 0007 has every realm do. This is stage 2 of
   the integration plan and it is needed anyway; multiplayer makes it
   first.
2. **Seats.** A seat is a person, held by the session layer. A fighter is a
   bot exactly when no seat holds it. Inputs become commands
   (`net/command.coax`): move, look, fire, jump, super, with a sequence
   number. Nothing in the step reads the camera or the device.
3. **A match setup.** How many seats, each seat's team and Scrapper, the
   level document, the seed. Same team puts two people on one side with a
   bot; opposite teams gives each two bots.
4. **Declare the realm.** A profile like Orbit's: 60 Hz, authoritative,
   whole snapshots, nothing durable, a dropped seat becomes a bot (with a
   short hold so a dropped player can come back).
5. **Replicate.** Bodies and shots as rows through `net/replication.coax`;
   the score, the clock and the knockout line as session frames; sounds
   stay local, made from what arrives.
6. **Client feel.** Your own body predicted with `net/predict.coax`,
   others smoothed with `net/interp.coax`, hits judged with `net/rewind.coax`
   and `net/shots.coax`. On a local network these barely matter; online
   they decide whether it feels fair.
7. **The same binary alone.** Single-player becomes a room with one seat
   over the memory transport. There is then one way the game runs, not two.

The rules, Scrappers, levels, HUD and the app round them stay in
GameBuilder. Everything in the two sections above this one is engine work
and goes to openagents.

## The player's side

1. PLAY, then **Play together**.
2. One person taps **Host**, the other **Join**, and sees the host's device
   name. The host accepts.
3. Both pick a Scrapper. The host picks **Same team** or **Opposite teams**.
4. The countdown runs and the match starts in the host's level.
5. At the end both see the result and **Play again**.

Online, later, is the same screens with a short code in place of a nearby
device, and the room on a server on the games domain.

## Order

| # | Work | Where | Rough size |
| --- | --- | --- | --- |
| 1 | The match on the World, with seats and commands; single-player still passes every check | GameBuilder | 1 week |
| 2 | The room core in-process, and the memory transport; single-player runs through it | openagents | 1 to 2 weeks |
| 3 | Scrappers as a realm: profile, replication rows, prediction and smoothing | GameBuilder, with fixes upstream | 1 week |
| 4 | The nearby transport and the Play together screens | openagents (transport), GameBuilder (screens) | 1 week |
| 5 | Two devices: feel, drops, rejoin, Mac with iPhone | Both | A few days |
| 6 | Online: Scrappers in a room server on the games domain, joined by code | Both, after the publishing service exists | 2 to 3 weeks |

This is slower than the first draft's week and a half. A private protocol
for two players would ship sooner and then be thrown away. The difference
buys the device room and the two transports, which Everglade offline,
SPLICEBOTS and every later game use.

If two-player Scrappers is needed sooner than that, the honest shortcut is
to do steps 1 and 4 with a thin stand-in for the room and replace it at
step 2. I would only do that on your say (question 1).

## How this serves an MMO

Nothing in Scrappers needs interest grids, byte budgets or shards. It does
not have to use them, and that is how the core is meant to work: a realm
takes the interfaces its profile needs. What Scrappers does for the MMO
path is concrete:

- It is a third consumer. A core used by a 150-player RPG, a vehicle league
  and a first-person brawler has its interfaces tested from three sides.
- The device room and memory transport give Everglade offline play and
  give every realm a way to run its real server code in a check with no
  network.
- A new transport proves the transport interface is one.

What it does not do is test scale. Everglade's own soak work does that.

## What is left out of the first version

- **More than two people.** The design allows six seats; the first version
  is tested for two.
- **Chat.** None, in a game whose first users are children.
- **Strangers.** No public matchmaking: nearby devices, or a code you gave
  someone.
- **Host migration.** If the host leaves a nearby match, it ends.

## Risks

- **The room is a server today.** How much of it assumes a database, HTTP
  and a directory is not yet read. Step 2's size depends on it. The browser
  worker shows it can be cut down to one seat.
- **The core is moving.** Decision 0007's room core (#13275) is in
  progress. Building on it means bumping the pin often and fixing what
  moves.
- **Local network permission.** If the person says no when iOS asks,
  nearby play finds nobody. The screen has to say why.
- **Versions.** Two devices on different builds must be refused with
  "update the app". The session's admission already carries pins for this.
- **Children online.** Step 6 puts a child in a room with someone else.
  Code-only joining, no chat and a parent's account are the minimum, and it
  needs the same legal read as publishing.

## Things I have not confirmed

- How the room binds a realm (I read the README and `room.coax`'s size, not
  its code), and so how much work "realm 3" is.
- Whether Orbit League's realm is a clean pattern to copy or carries
  history.
- Whether the browser worker's one-seat room can be built natively as it
  is.

Each is a day's reading at the start of step 2, before anything is built.

## Questions for the owner

1. **Is the slower, shared route right?** Or do you want a two-player
   stand-in sooner, replaced when the device room lands?
2. **Two people, or up to six?**
3. **Mac and iPhone together** as a goal for the first version?
4. **Online: does a guest need an account?**
5. **Does a published web game join the same rooms?** The transport
   interface already has the two a browser speaks.
