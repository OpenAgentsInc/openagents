# A seat in a running realm (agents that play)

Status: slices 1 to 3 built, 2026-11-02 (#12637): the scene probe, the seat
engine (`coax/lib/media/realm_seat.coax`, run by `coax/web/tool.sh
realm-seat`) and `openagents realm play` with its verbs and MCP tools.
Slice 4, presence in made realms with the AGENT tag, built 2026-11-02
(#12655). Seats as their account (a private realm seats its owner and site
admins) and slice 5, hosted seats, built locally 2026-11-02 (#12655); not
deployed. The agent pane (YOUR AGENT) built 2026-11-02 (#12655); not deployed.

Agents can make and edit realms through the routes and the CLI
([realm-edit.md](realm-edit.md), [the CLI spec](../cli/2026-11-01-openagents-cli-v1.md)).
A seat lets an agent be in a running realm the way a player is: look round,
walk somewhere, talk to a character and use the realm's tools. It's for
testing a realm before it's published ("can a visitor reach the well and
finish the quest?"), for showing a client round, and later for agents who
live in a realm.

## What a realm is at run time

A realm runs in the browser. Its Coax module (`myrealm.wasm` for made realms,
its own module for hand-built ones) steps the player, the camera, collision
and the characters, and draws the frame. The server holds the document, the
edits and the character chat (`POST /api/playtest/npc/ask`), but has no
running copy of the world. Presence (`POST /api/playtest/presence/<room>`)
is where each visitor stands, and nothing more; a made realm's room is
`made-<slug>` (slice 4).

So a seat must run the realm's own module. The server can't answer "what
would happen if I walked north" by itself.

## The surface: the page in headless Chrome, first

Three ways to run the module for an agent:

| Way | Pros | Cons |
| --- | --- | --- |
| **A. The page in headless Chrome**, driven over CDP | Works today for every realm, hand-built or made; the same code a visitor runs; shots come free | Needs Chrome where the seat runs; about 1 s an action |
| B. The module in a headless JS runtime, no drawing | Fast, no Chrome | Every realm's host imports must be stubbed; no shot |
| C. The native realm run (`c47fadafcd`, every realm draws natively) | Server-side and GPU-accurate | Needs a GPU host; native input and the talk routes aren't wired |

**We build A.** `coax/lib/net/cdp.coax` already drives pages this way for
`forge-journey.coax` and `realm shot`, with typed taps, keys and the page's
probe (`coaxial.guardProbe`). B and C stay options once the engine's typed
scene query lands (engine core, #12609 `ec-query`); a seat's protocol below
doesn't change if the runner does.

## What the agent observes

Every action returns an observation:

- **A typed scene summary**, read from the page's probe and the realm's
  spec, never from pixels:
  - where the agent stands (x, z, facing) and the camera kind;
  - what's in view and near, nearest first, up to 16: each with its kind
    (`character`, `building`, `tree`, `prop`, `water`, ...), its name when it
    has one, its distance and bearing;
  - the tools on (build, jump, fly, map, talk, voice);
  - the talk state: who you're talking to and their last answer;
  - the quest state, when a character gave one.
- **A shot**, when asked (`--shot`), at 1440x900 or 390x844, the same as
  `realm shot`.

The summary is what most agents use; it's small and exact. The shot is for
judging how something looks.

The page's probe (`coaxial.guardProbe(field)`, numbers only) gives the
positions, kinds, facing and talk state. The spec route
(`GET /api/playtest/realms/<slug>`) gives the names, roles and tools. The
seat joins the two. Made realms answer these fields (`myrealm.coax`
`realm_probe`, **built**):

| Field | Value |
| --- | --- |
| 10, 11 | where you stand, x and z, in centimetres |
| 17, 19 | facing (yaw) and pitch, in hundredths of a degree |
| 18 | the camera: 0 third person, 1 first, 2 top-down, 3 side |
| 64 | how many entities; 1000 + 2e and 1001 + 2e their x and z; 1400 + e their kind (tree, rock, prop, building, house, dwelling, lantern, placed, centre) |
| 65 | how many characters; 1700 + 2p and 1701 + 2p their x and z |
| 66 | the character you're talking to, or -1 |
| 67 | 1 while a question waits for its answer |
| 68 | lines in the talk so far |
| 69 | the tools on, a bit each: build, fly, jump, map, talk, voice |
| 3000 + i | byte i of the character's newest line, -1 past its end |

Each hand-built realm adds these fields when it wants seats.

## What the agent may do

Actions are a typed catalogue, each bounded, each one row of the CLI's
command table so MCP and the skill get them too:

| Action | Arguments | What happens |
| --- | --- | --- |
| `look` | `--turn DEG` (-180..180), `--pitch DEG` (-60..60), `--shot OUT.png` | Turns with a mouse drag, as a visitor does, and corrects from the facing the page reports, so a turn lands within 1.5 degrees. Pitch is first person only. |
| `go` | `TARGET` (a `near` name or kind) or `X Z`; `--max SECONDS` (default 10, at most 30) | Walks with the realm's own controller (held keys, the way a person walks) until within 2 m, blocked, or out of time. Returns where it stopped and why: `reached`, `blocked`, `timeout`. |
| `say` | `"WORDS"` (400 characters) | Talks to the nearest character within 3.5 m, as E then typing does; returns the answer. Refused with `no one near` otherwise. |
| `use` | `TOOL` (`jump`, `map`, `build THING`) | Uses a tool the realm has on. A visitor's `build` becomes a proposal for the owner, as now. |
| `bye` | | Ends the talk. With no talk open it does nothing: Esc then would leave the realm. |

There is no free-form "press any key" or "click at x, y". An agent does only
what a visitor can do, through the realm's own rules, so a seat can't reach
anything a player couldn't.

## How a seat is granted, metered and shown

- **Who** (**built**). A seat acts as an account. `POST /api/playtest/seats
  {slug}` (`coax/web/api/seats.c`), signed with the CLI's key (NIP-98),
  opens one and answers the seat's own player key, minted for that account
  (`migrations/0110_realm_seats.sql`). The seat's page plays with that key
  (localStorage `verse.player`, set before the page loads), so the realm's
  routes, character chat and presence meet it through the same rules a
  page's key meets: a private realm opens to its owner, and a site admin's
  seat sees it as the admin does. The key works only while the seat is open;
  closing it changes the key's hash so it never resolves again. A seat in
  someone else's realm needs the realm to be unlisted or public. Without a
  key the seat is a visitor's, unseen, as before. `realm shot SLUG --as-me`
  takes its picture through the same seat, closed after the shot.
- **How many** (**built**). `coax/web/api/seat-policy.coax` (`#authority`,
  56 mutants, 0 survivors) decides: one seat at a time per account (a new one
  closes the account's open one), at most 8 open on the site (a ninth
  account's is 429), closed after 10 minutes not kept (an action, its
  holder's keep); every made realm's presence post closes the idle ones.
- **Metering.** `say` is a character-chat call and counts against the same
  daily cap as a visitor's. Walking and looking cost nothing on the server.
  Seats carry the automation mark (`verse-automation`), so analytics leaves
  them out.
- **Shown to others** (**built**, slice 4). A made realm's page posts where
  you are about every second to `/api/playtest/presence/made-<slug>` and
  draws everyone else there as a figure with their name over it
  (`apps/openagents-web/myrealm-room.coax`). A seat's holder posts the seat's
  presence itself, signed with the CLI's key (NIP-98), so the server knows
  it's that account's seat: the row is marked `agent`
  (`coax/web/api/migrations/0108_realm_seat_presence.sql`), every page in the
  realm draws it in bronze with its account's name and AGENT under it, and
  the homepage's GET counts it. Only a signature marks a row `agent`; an
  unsigned "seat" automation mark is plain automation and stays hidden, as
  every check does (0069). A seat posts between actions, and an action takes
  up to 30 s, so a seat's row stays fresh for 40 s instead of 6; the holder
  posts `{"gone": true}` when it closes. The seat's own page (a guest player,
  automation) is in the room to see the others; the seat names it (`page`)
  and that page isn't shown its own agent. A private realm's room is 404 to
  everyone but its owner and site admins, as its routes are. Checks:
  `myrealm.coax` ("a made realm posts where you are each second ..."),
  `room.coax`, `realm_seat.coax` ("a seat's presence ..."), and
  `coax/web/api/test_room.py` (signed seat, its page, forged mark, 40 s,
  leaving, private room).

## Where it runs

Slice 1 runs where the CLI runs: `openagents realm play` starts the headless
Chrome on the same computer, as `realm shot` does. A hosted seat (the server
starts the Chrome, for agents without one, and for the web page's agent
panel) comes after, on the same runner as the project agents.

## Slices

1. **The scene probe** in `myrealm.coax`, with checks (**built**).
2. **The seat's engine** (**built**): `coax/lib/media/realm_seat.coax` opens
   a realm, runs one JSON action, and writes one observation. `coax/web/tool.sh
   realm-seat ORIGIN SLUG --actions FILE [--shots PREFIX]` runs a file of
   actions, one a line, and prints an observation after each.
3. **`openagents realm play SLUG`** (**built**): the verbs `play`, `look`,
   `go`, `say`, `use`, `bye` and `leave` are rows of the CLI's command table
   (`apps/openagents-cli/commands.coax`), so each is an MCP tool
   (`realm_play`, ...) and a line of the skill. `play` starts a detached
   holder (`apps/openagents-cli/realm-play.coax`) that keeps the page open
   between commands; see [the CLI spec](../cli/2026-11-01-openagents-cli-v1.md),
   "Play a realm". `openagents mcp --read-only` offers the verbs that change
   nothing on the server: all but `say` and `use`.
4. **Presence for made realms**, and the AGENT tag (**built**, #12655): see
   "Shown to others" above. The probe adds fields 70 (how many others), 71
   (how many of them are agents), 72 (the room answered), 1800 + 2i and
   1801 + 2i (where each is, cm) and 1850 + i (1 for an agent); a seat's
   observation carries `others` and `agents`.
5. **Hosted seats**, for agents with no Chrome (#12655, **built, not
   deployed**). `openagents realm play SLUG --hosted` opens the seat with
   `{"hosted": true}`; oa-api hands it to the project agents' runner
   (`coax/web/agent/seat-runner.c`, `POST /seat/open|act|close` beside
   `/wake`), which starts an `oa-seat` (`coax/web/agent/seat.coax`: the seat
   engine and its headless Chrome) with the seat's key on its stdin and
   nowhere else. Each verb is `POST /api/playtest/seats/ID/act`, answered with
   the observation. The runner holds at most 8 and closes one idle for 10
   minutes, as oa-api does; the caller never sees a hosted seat's key. The
   runner's image adds Chromium with SwiftShader's WebGPU (`CDP_WEBGPU`,
   `coax/lib/net/cdp.coax`). Checks: `coax/web/api/test_room.py` (oa-api
   against a stand-in runner; the runner against a stand-in oa-seat: key on
   stdin and not in its log, 8 at once, idle close, close) and `seat.coax`.
   The agent pane is below.

## The agent pane (YOUR AGENT)

**Built** 2026-11-02 (#12655), not deployed. A made realm's owner opens EDIT >
AGENT on the realm's page and presses START YOUR AGENT. The page posts
`{"slug": SLUG, "hosted": true}` to `/api/playtest/seats`, the same route and
runner `openagents realm play --hosted` uses. The page's logged-in session
(`oa_session`) is the account. The player key never opens a seat, and a POST
must carry this site's Origin. The runner holds the seat's key; the page
never gets it.

The pane's first look comes in by itself. After that the owner types a verb in
plain words, and the pane turns it into one of the actions above:

| Typed | Action |
| --- | --- |
| `look left`, `right`, `around`, `up`, `down`, `-45` | `look` |
| `look`, `see`, `where am I` | `see` |
| `go Maple`, `go to the well`, `go 4.5 -2` | `go` |
| `say WORDS` | `say` |
| `jump`, `map`, `build barrel`, `use TOOL [THING]` | `use` |
| `bye` | `bye` |

Each answer adds plain lines under the verb: what happened ("Go: reached."),
where the agent stands ("At 3.5, -1.3, facing 90 degrees."), who answered
("Maple: ..."), and the three nearest things ("Near: Maple 1.1 m, house 3.0
m, tree 4.0 m."). CLOSE ends the seat on the runner as well. A refusal gets one
plain sentence, for example "Log in to start your agent: it plays as your
account." or "All 8 agent seats are busy. Try again in a few minutes."

The pane's state and verbs are `verse/edit/agent.coax`, and it is drawn in the
edit layer (`verse/edit/edit.coax`, `redit_agent_pane`). A realm offers the
pane with `redit_agent`, as `myrealm.coax` does. The forge screen's YOUR
AGENT tab (`aethyrforge.md`) is the same pane.

Checks:
- `verse/edit/agent_test.coax`: the verbs, each request and answer, refusals,
  the log, and the pane on a desktop and a phone.
- `coax/web/api/test_room.py`: a logged-in page opens, acts in and closes a
  hosted seat. The session sent from another site's page is refused, and so
  are a POST without this site's Origin, an unknown session and the player key.
- `coax/web/tool.sh agent-pane API TOKEN MODULE [SHOTS]`: the page in
  headless Chrome against a local oa-api and a stand-in runner. Its header
  lists what to start first.

The check, run against staging on 2026-11-02 with the made realm
`village-green` (a well, two houses and Maple, a baker): `play`, `look
--turn 90` (facing 90.0), `go Maple` (`reached`, 1.1 m away), `say` (Maple's
answer in the view), `bye`, `use jump`, `use build barrel` (refused: build
is off there), `go --x 0 --z 0`, and the same through `openagents mcp`.
Not yet run live: `use build` in a realm with build on, and `--pitch` in a
first-person realm.

```sh
openagents realm play village-green
openagents realm go village-green Maple
openagents realm say village-green "What do you bake?"
openagents --json realm look village-green --turn -90 --shot view.png
openagents realm leave village-green
```
