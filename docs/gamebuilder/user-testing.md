# The automated user

Written 2026-11-18 for issue 28. A person's journeys through the web app (the core loop, publishing,
the account, the other pages), driven as a person drives them: taps, clicks, keys and typing in
headless Chrome, over its DevTools protocol, with the player's own browser machinery
(`vendor/openagents/coax/web/player-browser.coax`, loaded, not copied). No window opens and nothing
takes the desktop's focus.

The player (`docs/player-testing.md`) asks whether a game plays. This asks whether a person gets
from arriving to a published game someone else plays, and back.

## Running it

```sh
scripts/user-test.sh --checks     # the journeys' own checks and this page's coverage table (scripts/check.sh runs it)
scripts/user-test.sh --local      # every journey, three lanes, against this checkout on this Mac
scripts/user-test.sh --local --lanes chrome-mac --only arrive,guest-build,publish
scripts/user-test.sh --live       # gamebuilder.org itself: the lead runs this (below)
```

`--out DIR` puts the evidence in DIR (a private temporary folder when left out). Each run prints the
API's version (`/api/capabilities`) and the site's release (the page's `/b/NAME/` folder), a line
per journey, and writes `report.json` with every verdict, its time and its words.

### What `--local` stands up

- **The site** built and laid out as `scripts/web-deploy.sh` lays it out (release names, the page
  at `/login`, `/login/cli`, `/play`, `/credits` and `/agents`), served as the bucket serves it: a
  folder is its `index.html`, a missing file is `404.html` with status 404.
- **The edge** (`tests/user/edge.coax`): the load balancer's map in front of everything, on a free
  port of `localhost` (never 8810). It sends `/api/rows/*`, `/api/levels/*`, `/api/play/*`,
  `/api/pictures/*`, `/g/*`, `/api/capabilities`, `/api/openapi.json`, `/api/count` and `/mcp` to the
  hosted server, and answers the OpenAgents routes itself:
  - a stand-in login service: email codes (written to the run's own outbox file, never sent), five
    tries with the real service's words for a wrong code, the session cookie, `/api/auth/account`
    in the real service's shape, logout, and `/login/cli`'s code;
  - a stand-in chat model: answers from the run's `chat-plan` file, or a `make` route with an
    offer, as the chat's own steps ask;
  - a stand-in model for cloud Builders, on its own port, answering from the run's `model-plan`.
- **The hosted server**: `gamebuilder serve --accounts` on the run's own accounts folder, with a
  test model key of the run's own and the edge as its origin and its model.

Every file of a run (accounts, outbox, logs) is in its private temporary folder, removed with it.

## The lanes

| Lane | Screen | Input |
|---|---|---|
| `chrome-mac` | 1440×900 | mouse and keys |
| `chrome-phone-emulation` | 412×915, scale 2, mobile | touch |
| `chrome-tablet-emulation` | 1024×1366, scale 2, mobile | touch |

A type is picked on a keyboard lane by its number and on a touch lane by a tap on its name. The
journeys that spend model steps (`uj_model` in `tests/user/catalog.coax`) run last in each lane and
are labelled `[model]`. Model prose never passes a journey: the level, the row or the Builder's
state must have changed.

## Verdicts and evidence

A journey ends **pass**, **fail** (in the words of what a person would see go wrong) or **blocked**
(in words: what it could not reach, such as a journey it needs that did not pass). `touch` is "not
on this lane" on `chrome-mac`.

Evidence is screenshots, the page's own words, the services' answers, timings and the build's
versions. It holds email addresses and codes, so it stays private: in the run's own folder, never
committed, never posted.

## Not to drift

The journeys read what they name from the product's sources, never a copy:

- the game types, their names, keys and the 512 things a level holds: `shared/game-types.coax`;
- each type's own pieces and their words: `shared/kinds.coax`; the village's 33 buildings:
  `shared/buildings.coax` (`VL_PUT`);
- the funnel's steps: `shared/loop.coax`; the agents' state words: `coax/agent-record.coax`;
- the no-WebGPU heading: `web/gpu-words.coax`;
- the home page's load budget: `tools/home-load.coax`'s own default; room 1's recorded crates
  solution: `web/world-crates.coax` (`CR_SOLUTION_0`), read from the source text.

`scripts/check.sh` runs `tools/game-types-check.coax` and `tools/agents-drift.coax` over
`tests/user/*.coax` as over the player's journeys.

## The journeys

| # | Journey | What a person does |
|---|---|---|
| 1 | `arrive` | A cold `/`: the heading and the nine types' names, each a link, on the screen within the load budget (`HL_NAMES_MS`, 500 ms) in the shared list's order, their portals' pictures there; a tap on a name and key 1 at the ring each enter a world; a browser with no WebGPU is told so in the page's words, the names a list. |
| 2 | `guest-build` | For each of the nine types, as a guest: no login asked; a block and the type's own piece placed (a boost, a mover, a wall, a ring, a building); undo and redo; after a reload the same things. |
| 3 | `ai-asks` | The guest's chat says an account is needed; its "log in" opens the sheet in place, the world and the address unchanged. |
| 4 | `sign-up` | An email on that sheet, a wrong code first (refused, with the tries left), then the right one; the world unchanged and "kept on your account"; a reload shows it still. |
| 5 | `your-games` | The game under "your games"; a second browser logs in to the same account, opens it, adds a block; the first, reloaded, shows the change. |
| 6 | `limits` | The platformer built to 512 things (locally the chat's plan lines lay rows of blocks to 511, live it is all by hand; the last ones by hand either way): kept; the 513th kept in this browser only, its words saying both numbers; taken back, kept again. The village stood to 33 buildings with rocks by hand, and one more refused with its number. |
| 7 | `per-type` | Crates: room 1 solved with its recorded solution (keyboard lane). Platformer: a mover placed by hand and set going by a `goes` plan line; what the person stands on made a mover by plan lines, and the person carried with no key held. Village: a market put at a scale of 1.5. |
| 8 | `publish` | Publish gives a `/g/CODE` link with a six-character code; copy says copied and puts the link on the clipboard; a change and publishing again keep the same link and make a second version. |
| 9 | `others-play` | A fresh browser opens the link: it plays the newest version at once, with no login; Remix opens a copy in its own builder that can be built on. The funnel's `shared` row is read before and after: a driven browser is never counted (`web/count.coax`), so the owner's numbers do not move. |
| 10 | `unpublish` | Unpublish; the link then says the game isn't available, in another browser. |
| 11 | `agents-page` | `/agents` signed out is the login, and back on `/agents` once in (a second browser); signed in, the message box has the caret, the limits line says the record's numbers (`AGR_CLOUD_*`), and the list sits beside the conversation or, under 860 px, comes out as a drawer and goes away. |
| 12 | `cloud-builder` | A sentence and Return starts a cloud Builder on a new game: it works, comes to done, its commands show in the log, and the level has one thing more (counted in the builder through "open in the builder"); "take back its last change" leaves one fewer; a second message adds one again; a third is stopped with Stop and shown stopped, Send back in its place. |
| 13 | `builder-refusals` | A first message past `AGR_JOB` bytes is counted with both numbers and starts nothing. Locally, a step of nine commands: the shell command, another level and the ninth (past `AGR_CLOUD_COMMANDS`) shown refused, with the number. |
| 14 | `chat-offer` | A message the chat cannot do gets an offer; "yes" starts a build agent, whose row says its state. |
| 15 | `agents-empty` | A new account (a second browser) on `/agents`: nothing listed, the caret in the box; a sentence and Return makes a game and a Builder builds it, and reports; its change taken back leaves one thing fewer. Local only: the live inbox's account is never empty. |
| 16 | `leave-return` | A change, then away to credits and the home page, and back to the builder: the game as it was left. |
| 17 | `other-pages` | `/credits`, `/login/cli`, Back and Forward between them, an unknown address (not found), `/login`. |
| 18 | `touch` | Phone and tablet, portrait and landscape: every control on the home page, the types, a world and the publish sheet on the screen and not covered; with what every other journey found on the lane. |

## Coverage

Every address of the site and every step of the core loop, with the journeys that walk through it.
`scripts/user-test.sh --checks` holds this table to `web/page.coax`'s addresses (`GS_PATHS`, `/play`,
`GS_LINK`), to `shared/loop.coax`'s steps and to `tests/user/catalog.coax`: a new address or step
with no row, or a row that says other journeys, fails it.

| Address or step | Journeys |
|---|---|
| `/` | `arrive`, `leave-return` |
| `/login` | `other-pages` |
| `/credits` | `other-pages` |
| `/login/cli` | `other-pages` |
| `/agents` | `agents-page`, `cloud-builder`, `agents-empty` |
| `/play` | `guest-build`, `sign-up`, `per-type` |
| `/g/` | `publish`, `others-play`, `unpublish` |
| `not found` | `other-pages` |
| `arrived` | `arrive` |
| `create` | `arrive` |
| `builder` | `arrive` |
| `picked` | `guest-build` |
| `placed` | `guest-build` |
| `prompt` | `ai-asks` |
| `account` | `sign-up` |
| `message` | `chat-offer`, `per-type` |
| `published` | `publish` |
| `shared` | `others-play` |
| `remix` | `others-play` |
| `nogpu` | `arrive` |
| `button_ms` | `arrive` |
| `builder_ms` | `arrive` |
| `kept` | `sign-up`, `your-games` |

## The live run

`scripts/user-test.sh --live` is the lead's to run; nothing else runs it. It drives
`https://gamebuilder.org` as above, with these differences:

- **Credentials, by name only**: `AUTH_TEST_INBOX` and `AUTH_TEST_INBOX_PASSWORD`, the player's
  dedicated test inbox (`tests/player/mail.coax`). Each sign-in snapshots the inbox, asks for a
  code and reads the new one. No other account is used.
- **Emails**: one login code each time a person signs in: on each lane, `sign-up`, the second
  browser of `your-games` and the second browser of `agents-page` (three a lane, nine for three
  lanes). `sign-up` types a wrong code first (the
  right one with its last digit changed), which spends one of that code's five tries.
- **Pacing**: the login service sends one address 5 codes an hour and 20 a day, and the player
  uses the same inbox. So the user asks for a live code at most once every 13 minutes, keeping the
  time in the player's pacing file (`entitled-next-code-ms` under `GB_PLAYER_PROFILE_ROOT`, which
  `scripts/user-test.sh --live` sets as `scripts/player-test.sh` does). A full run on three lanes
  asks for nine codes, so it takes about two hours, and a day allows two runs at most, fewer if the
  player ran. If the service refuses anyway, `sign-up` says so in the service's terms, and the
  journeys after it are blocked.
- **Accounts**: the one the inbox's address has (made on its first code if it has none).
- **Games** on that account, which no person can delete: on each lane the guest's last game
  (saved on signing up), the platformer built to 512 things, the crates, platformer and village
  games of `per-type`, the racing game published and unpublished, and the drift game of
  `leave-return`. About six a lane.
- **Published games**: one a lane, published twice (two versions), then unpublished by `unpublish`.
  If a run stops between the two, a link stays live: unpublish it from the game's publish sheet.
- **Model steps**: the chat's own model for `per-type`'s three plan-line messages and `chat-offer`'s
  two; `chat-offer`'s yes starts one cloud build agent on the account, which spends its own steps
  within its limits (12 model steps, 30 changes, 10 minutes) and stops on its own. `cloud-builder`
  starts one cloud Builder on a new game (one game more), sends it three messages (the third
  stopped) and takes one change back; `builder-refusals` sends one over-long first message, which
  starts nothing. With the real model the level is checked to change, never the prose.
  `agents-empty` does not run live (it needs an empty account) and says so.
- **The funnel**: not read live (it is the owner's); the page counts no driven browser, so a live
  run adds nothing to it.

Nothing is cleaned up beyond unpublishing: games stay on the test account, as a person's would.
