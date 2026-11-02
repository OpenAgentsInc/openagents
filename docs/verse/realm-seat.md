# A seat in a running realm (agents that play)

Status: design, 2026-11-02 (#12637). Nothing here is built yet.

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
running copy of the world. Only the Haunted House sends presence
(`POST /api/presence/<room>`); made realms don't show other players yet.

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

- **A typed scene summary**, read from the page's probe, never from pixels:
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

This needs one new probe in each realm's module, `realm_scene`, written as
lines like the routes answer:

```text
at 4.2 -1.0 90
camera third
near character 3.1 45 Maple|innkeeper
near building 8.4 10 A log cabin
tool talk 1
talking Maple|Welcome! The well is past the barn.
```

Made realms get it first, in `myrealm.coax`, from the layout it already has
(`myrealm-shapes.coax`). Each hand-built realm adds it when it wants seats.

## What the agent may do

Actions are a typed catalogue, each bounded, each one row of the CLI's
command table so MCP and the skill get them too:

| Action | Arguments | What happens |
| --- | --- | --- |
| `look` | `--turn DEG` (-180..180), `--pitch DEG` (-60..60) | Turns as a drag would. |
| `go` | `TARGET` (a `near` name or kind) or `X Z`; `--max SECONDS` (default 10, at most 30) | Walks with the realm's own controller (held keys, the way a person walks) until within 2 m, blocked, or out of time. Returns where it stopped and why: `reached`, `blocked`, `timeout`. |
| `say` | `"WORDS"` (400 characters) | Talks to the nearest character within 3.5 m, as E then typing does; returns the answer. Refused with `no one near` otherwise. |
| `use` | `TOOL` (`jump`, `map`, `build THING`) | Uses a tool the realm has on. A visitor's `build` becomes a proposal for the owner, as now. |
| `bye` | | Ends the talk. |

There is no free-form "press any key" or "click at x, y". An agent does only
what a visitor can do, through the realm's own rules, so a seat can't reach
anything a player couldn't.

## How a seat is granted, metered and shown

- **Who.** A seat acts as an account: the CLI's signed key, or the page's
  player key. A private realm seats only its owner and site admins, as its
  routes already require. A seat in someone else's realm needs the realm to
  be unlisted or public.
- **How many.** One seat at a time per account, and at most 8 a site,
  because each one is a headless Chrome. A seat closes after 10 minutes
  without an action.
- **Metering.** `say` is a character-chat call and counts against the same
  daily cap as a visitor's. Walking and looking cost nothing on the server.
  Seats carry the automation mark (`verse-automation`), so analytics leaves
  them out.
- **Shown to others.** When made realms get presence, a seat shows as a
  figure with an AGENT tag over its head, its account's name, and the
  `agent` flag on its presence row. Until then it's invisible to other
  visitors, which is honest because they can't see each other either.

## Where it runs

Slice 1 runs where the CLI runs: `openagents realm play` starts the headless
Chrome on the same computer, as `realm shot` does. A hosted seat (the server
starts the Chrome, for agents without one, and for the web page's agent
panel) comes after, on the same runner as the project agents.

## Slices

1. **The scene probe** in `myrealm.coax` (`realm_scene`), with checks.
2. **`realm-seat.coax`**, a tool like `forge-journey.coax`: opens a realm,
   reads JSON-line actions on stdin, writes one observation a line. This
   is the seat's engine, testable without the CLI.
3. **`openagents realm play SLUG`**: the CLI verbs `look`, `go`, `say`,
   `use`, `bye`, and MCP tools for each. This waits for the CLI's command
   table (#12635), so the verbs are rows, not hand-written commands.
4. **Presence for made realms**, and the AGENT tag.
5. **Hosted seats**, for agents with no Chrome and for the web agent panel.

A check walks a seat through one made realm: `go` to a character,
`say` hello, read the answer, `go` to a building, `look` round, all from
the summary, then a shot.
