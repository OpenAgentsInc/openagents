# Publishing a game to the web

> **Owner decision, 2026-11-16, after this spec was written:** GameBuilder
> uses the OpenAgents account, the same login as the OpenAgents apps, not a
> separate one. The app now logs in that way (`host/Sources/Account.swift`,
> compiling openagents' `AppAuth.swift`). Where this spec recommends a
> separate GameBuilder account or Sign in with Apple, read "the OpenAgents
> account" instead. The three official games are playable by anyone and
> editable only by their owners, associated by hand (`coax/games.coax`).

Written 2026-11-16. Status: publishing from the website and playing at a
link are built (issue 17, 2026-11-17). The phone's flow, feedback on the
link's page and the frozen player build are not. The sections below this one
are the original specification; where they differ from "What is built", that
section is what the code does.

## What is built

**The rules and the rows (issue 3, step 7).** `coax/publish.coax` holds the
state and every rule it checks. The rows `publish`, `published`, `versions`,
`set-live`, `unpublish`, `remove-published`, `feedback`, `feedback-hide`,
`block` and `report` (`coax/commands.coax`, area `publish`) run it. The app's
side panel's Share section is drawn in Coax and presses them.

- A version is immutable and named by its level file's SHA-256.
- Live is a pointer on the game, so a version can be made live again.
- A share code is six characters with no look-alikes.
- Every call checks the account itself.
- Every action is the person's own: the table refuses it to an agent or a
  program acting for them, and the hosted server refuses a call that names
  an agent (`GameBuilder-Agent`) or comes through `/mcp`.

**Publishing on gamebuilder.org (issue 17).** One path, the rows':

- **The publish button** is in the builder's top bar (`web/page.coax`,
  `web/publish.coax`, drawn in Coax). It opens a sheet over the game: a name,
  one button, and the limits in words. A guest gets the account sheet first
  ("log in or create an account."), and when the emailed code is taken the
  publish sheet opens over the same game with nothing lost.
- **Publish** first keeps the game on the account (`web/account.coax`, the
  rows every surface uses), then calls the row `publish` on the hosted server
  (`POST /api/rows/publish`, the browser's login cookie). What is published
  is the account's level file, byte for byte.
- **The hosted store** is the hosted server's own folder
  (`gamebuilder serve --accounts DIR`, `cli/publish.coax`): the account's
  `published` file (the same text a device keeps), each version's level file
  at `DIR/ACCOUNT/published-GAME-VERSION.level`, and one index of links at
  `DIR/.published/CODE` naming the account. In production that folder is the
  existing accounts bucket: no new bucket and no database.
- **The link** is `https://gamebuilder.org/g/CODE`, shown in the sheet with
  **copy link**, **share** (the system share sheet, where the browser has
  one) and **play it**. It stays the same from version to version.
- **The link's page** (`GET /g/CODE`) is the site's own page with the game's
  name, a description and the game type's picture in its head (`<title>`,
  `description`, `og:title`, `og:description`, `og:image`, `og:url`), so a
  link pasted in a message previews. The server writes those tags into the
  site's `/play` page, which it fetches from the site and keeps for a minute.
  The page then asks `GET /api/play/CODE` and plays the level with the site's
  own world code: no login, no account, no build tools, no chat, nothing
  kept in the visitor's browser.
- **Versions.** Publishing again is a new version at the same link. The sheet
  lists the last six, each with "make live"; `set-live` plays an older version's
  own bytes again.
- **Unpublish** takes the link down at once ("this game isn't available.",
  404); publishing again puts it back at the same link. `remove-published`
  (command line) gives the link and the files back for good.
- **Remix** on the link's page opens a copy in the visitor's own builder, as
  a guest: it is theirs, kept in their browser, and a new game on their
  account once they log in. The creator's game is never touched. If the
  visitor already has a game of that type built in this browser the page
  asks before replacing it.
- **Report** on the link's page: four reasons (offensive, someone's personal
  details, spam, something else), no free text, no account. Each is one line
  in the operator's queue, `DIR/.reports` (time, code, reason). The operator
  takes a game down by writing the file `DIR/.published/CODE.down`: the link
  then plays nothing whatever its owner does, until the file is removed.
- **Counted** (`shared/loop.coax`): a game published, a shared link opened,
  Remix pressed. Those are the site's steps, a day at a time. Each game is
  also counted by itself, and a remix keeps the game it came from: see "Play
  counts and remix lineage" below.

`tools/web-publish-check.coax` (run by `cli/check.sh`) runs the page's own
modules against the real hosted server and fails if any of this drifts: the
published version is the app's level file byte for byte and the same game a
command line lists; the link plays it with no login and says nothing of the
account; an agent's call and a call with no login are refused; another
account can do nothing to the game; the limits the page says are the core's
numbers.

### The limits, as built

| Limit | Value | Where |
| --- | --- | --- |
| What a published game holds | An account's level: 512 things, 64 different shapes, 256 KB (the village's buildings are things among them) | `shared/game-types.coax`; the page says it before it tries |
| Published games an account | 16 | `PUB_GAMES` |
| Versions kept, between an account's games | 128; past it the oldest version no link plays goes | `PUB_VERSIONS` |
| Publishes | 30 an hour an account | `PUB_PER_HOUR` |
| A game's name | 48 bytes, no link in it | `PUB_NAME` |
| Reports | 120 an hour for the whole server; the queue holds 1 MB | `cli/publish.coax` |
| The link's page | 131,072 bytes of the site's page; kept 60 s | `cli/publish.coax` |
| Counts taken | 600 a minute for the whole server, a game's counts and the core loop's together; past it a count is refused (429) and not kept | `LP_PER_MINUTE` |

The sheet says the first four in words with their numbers.

### Play counts and remix lineage

The question this answers: how many new published games does one published
game bring? It is the first move in
[business/roblox-scale.md](business/roblox-scale.md), and nothing could
measure it while a game had no counts of its own and a remix forgot where it
came from.

**Four counts a game** (`shared/plays.coax`, the one list of them):

| Count | What happened | Who counts it |
| --- | --- | --- |
| `opened` | The link's page showed the game | The link's page |
| `minute` | The game was still open 60 seconds later (`PY_MINUTE_S`) | The link's page |
| `remix` | Remix was pressed on the link's page | The link's page |
| `remade` | A remix of the game was published by another account | The hosted server, when that remix is first published |

The page sends each of the first three once for a link while it is up (a
page loaded again counts again), as `POST /api/play/CODE/played` with the
body `{"fact":"opened"}` and nothing else (`web/publish.coax`:
`pb_fact_tick`). A count that fails is dropped:
the game is never held up for it. A browser a program drives is not counted
(`web/count.coax`: `ct_may`), as the core loop's steps are not. A creator
looking at their own game in the builder is not a link opened, and is not
counted.

**Where the totals are kept** (`cli/plays.coax`), under the accounts' folder:

- `DIR/.plays/CODE`: one game's four totals (`gamebuilder.plays.v1`).
- `DIR/.plays/.site`: the same four added up for every game, and the ten
  games most opened.

Each file is totals only: no visit, no visitor's id, no address, no time. A
file is replaced whole, so a stop half way leaves one that reads; one that
does not read is left as it is and that game is not counted until a person
has looked. `remove-published` removes the game's file and takes it out of
the ten; what it added to the site's totals stays.

**Lineage.** A published game keeps the code of the game it was remixed from
(`PubGame.from`, `coax/publish.coax`; an eleventh field of its line in the
`published` file, written only for a remix).

- Remix on the link's page writes the link's code beside the new game in the
  browser's own record of its games (`web/account.coax`: `ac_from_set`), and
  the publish sheet sends it with the first publish as the row's flag
  `--from CODE` (`"from"` in the HTTP body).
- It is kept at a game's **first** publish and never changed after: a later
  version cannot claim another origin, and a game's own code is never its
  origin.
- The hosted server drops a `from` it holds no link for, so a remix can't
  name a game that does not exist. The game is still published.
- A remix of your own game keeps its `from` and counts for nothing:
  `remade` is games brought by someone else.

**Reading them back.**

- `gamebuilder published --remote` (and `GET /api/rows/published` with a
  login): each of the account's games says `"from"` when it is a remix and,
  on a hosted server, `"plays":{"opened":N,"minute":N,"remix":N,"remade":N}`
  once anything has been counted.
- `gamebuilder funnel --remote` as the operator: a `games` section with
  `links` (codes the server holds a game at), the four site totals,
  `remade_per_1000_links` and `top`, the ten games most opened with their
  own totals. `remade_per_1000_links` is the number the question asks for,
  in thousandths: 1000 is one new published game for each published game,
  and above it the games spread on their own.

**What these numbers are not.**

- They are times, not people. One visitor opening a link twice is two.
  There is no player token yet, so nothing says how many people a game had.
- They can be sent by hand. Anyone can post a count for a link that plays,
  up to the server's 600 a minute. Read them to see which games spread;
  they are not fit to pay anyone by. `remade` is the hardest to raise: it
  takes a logged-in account publishing a game, 30 an hour and 16 games an
  account at most.
- They are totals since counting began, with no days in them. A game's
  counts can't say when it was played.
- Lineage is the browser's to carry until the first publish. A remix kept
  on the account and first published from another device, or after the
  browser's storage was cleared, is published with no `from`.
- A game published before this was built has no `from`, whatever it was
  remixed from.
- The publish sheet does not show a creator their game's counts yet: they
  are read by the `published` row.

`tools/web-publish-check.coax` runs all of it against the real hosted
server: a link opened and counted, the counts read back by `published`, a
count with a made-up fact, a longer body or a code that plays nothing
refused, a remix published by another account counted once and never again
for a later version, the two files' contents, the operator's `games`
section, and the counts gone when the game is removed.

### Where the link lives, and why

The specification's fixed point was a separate games domain. Its reason is
the browser's same-origin rule: a page of someone else's code on
gamebuilder.org could act as the visitor on gamebuilder.org. **That reason
does not apply to what a published game is today**: a level file (positions,
sizes, kinds from a closed list, a name with no markup) played by the site's
own code. Nothing a creator writes runs. So the link is a path on
gamebuilder.org, short, with no login to play (owner, 2026-11-17).

It is built to move: the server reads `GAMES_DOMAIN` from its environment
(unset: the site's own origin), and every link it says, in every row's
answer and in the page's `og:url`, is `GAMES_DOMAIN/g/CODE`. The day a
published game can carry a creator's code (scripts, uploaded assets with
active content), it must move to its own domain first, and that is that
variable plus the domain's route to the same service.

### The open questions: the option taken

1. **The domain.** gamebuilder.org itself, at `/g/CODE`, for the reason
   above; no second hostname.
2. **Where it runs.** The existing Google Cloud project and the existing
   hosted service and bucket (`deploy/api`), with two more paths on the load
   balancer (`/g/*`, `/api/play/*`).
3. **Accounts.** The OpenAgents account (the owner's decision above); sign-up
   is the email-code login.
4. **Children.** Not built: any logged-in account may publish and there is
   no age question (owner, 2026-11-17). The specification's position (an
   under-13 creator under a parent's account, a lawyer's read) stands for
   before a wider release.
5. **Feedback text.** The safest option: the link's page takes no feedback at
   all yet, neither thumbs nor text. The rows and their rules exist; the
   player's routes for them are not built.
6. **WebGPU only**, as proposed, with the count of turned-away visits the
   site already keeps (`LP_NOGPU`).
7. **What the link shows about the creator.** Nothing: not a name, not the
   account. The play answer holds the game's name, type and level only.
8. **Who works the report queue.** The operator (the owner), by reading
   `DIR/.reports`. A takedown is one file and is immediate. No time is
   promised in code.
9. **How long old versions stay playable.** For good, as proposed, up to the
   128 kept an account; `set-live` plays any of them again. What is kept is
   the level file. The player build is **not** frozen per version: every
   version plays with the site's current world code (see "Not built").
10. **Remixing.** Built, and on for every published game (the issue asks for
    it). The specification's "the creator's choice per game" is not built.

### Not built

- The frozen player build a version names (the specification's "exact player
  build"): a published game plays with the site as it is today. A change to
  the world code changes how old versions play.
- Per-version links (`/g/CODE/v/N`), thumbnails of the game itself (the
  preview picture is the game type's), and play facts (seconds played,
  rounds finished). A game's play counts are built: see "Play counts and
  remix lineage". Not built of them: how many people (the player token),
  counts by day, counts by version, and the creator's counts in the publish
  sheet.
- Feedback on the link's page and the creator's inbox on the web; push.
- Publishing a game past the account's limits.
- A public listing, a creator's name, age bands, a parent's account.
- The phone app's flow, and a device's own play routes: `gamebuilder serve`
  without `--accounts` answers a link with 404 and says where games are
  played.

The requirement, in the owner's words: from a single tap inside the app, a
creator deploys their game to the web and gets a shareable game with a
version; players leave feedback; the creator sees that feedback in the app.

Fixed points from the owner:

- **Mobile first.** Every flow is designed for the phone app. The Mac app is
  a power-user editor and gets the same features with more room.
- **A separate domain.** Games live on GameBuilder's own domain, written
  here as `GAMES_DOMAIN` until the owner names it. They are not listed on
  openagents.com or in the OpenAgents Metaverse. Any link between the two is
  decided later.
- **An account is required to publish.**
- **Money is postponed.**
- **GameBuilder is proprietary.** The engine and Coax are open source.

Sections marked **Verified** state what exists in the pinned openagents tree
(`vendor/openagents`, commit `19ff5d7715`), with paths. Everything else is a
proposal.

## Summary

1. **A published version is an immutable snapshot**: the level document, the
   game template and its settings, and the exact player build that runs it.
   The link keeps working when the app or the engine moves on, because the
   version names its build by content hash and that build is never deleted.
2. **The web player is the same Coax as the app's play mode**, compiled to
   WebAssembly, with a small WebGPU host. The rules are not rewritten in
   JavaScript.
3. **The service is GameBuilder's own**: its own domain, database, bucket
   and deploy. It reuses openagents' open code (the Coax web host, NIP-98
   auth, the world document) and shares no data with openagents.com.
4. **The first slice is small**: publish one level, play it at a link, send
   a thumbs up or down with a line of text, see it in the app.
5. **The first user is a child.** Feedback from strangers and a public
   listing are the risky parts. The first slice ships neither a public
   listing nor unfiltered text, and the plan says when each can be added.

## The creator's flow, on a phone

### Publish

One button in the editor's top bar: **Share**.

| Step | What the creator sees | What happens |
| --- | --- | --- |
| 1 | Taps Share | If not signed in, the sign-in sheet (see [Accounts](#accounts-and-identity)). Once, ever |
| 2 | A sheet: the game's name (prefilled "Level 3", editable), a thumbnail taken from the current view, and one button, **Publish** | Nothing is sent yet |
| 3 | Taps Publish. A progress line: "Checking… Uploading… Live" | The app saves the level, sends the snapshot, and the server answers with the link. Target: under five seconds on a phone connection |
| 4 | The link, a **Copy** button, a **Share…** button (the system share sheet), a QR code and **Play it** | `https://GAMES_DOMAIN/g/k7m2p9` |

After the first publish, Share shows the game's page instead of step 2.

"Single tap" is steps 3 and 4: once a game has a name and the creator is
signed in, publishing again is one tap on **Publish update**.

### The game's page in the app

| Part | What it shows |
| --- | --- |
| Link | The link, Copy, Share…, QR |
| Status | "Live: version 3, published 2 hours ago" and whether the level has changed since ("You have unpublished changes") |
| Publish update | Makes version 4 from the level as it is now. The link stays the same and now plays version 4 |
| Versions | A list: number, time, plays, thumbs. Tap one to see its own link (`/g/k7m2p9/v/2`), **Make this the live version** (roll back) or **Open a copy in the editor** |
| Feedback | The inbox, below |
| Unpublish | Takes the link down at once. The link then shows "This game isn't available". Versions and feedback are kept, and Publish puts it back at the same link |
| Delete | Removes the game, its versions and its feedback for good, after a confirmation |

### The feedback inbox

A list, newest first, per game, with a badge on the Share button when there
is something unread.

| Each item shows | |
| --- | --- |
| Thumbs up or down | Always |
| A line of text | When the player wrote one and it passed the filter |
| Which version | "on version 3" |
| When, and on what | "2 hours ago, on a phone" |
| How far they got | "played 3 minutes, 4 knockouts" (from the player, see [Play facts](#play-facts)) |

Actions on an item: **Hide**, **Report**, and **Block this player** (their
later feedback never arrives). There is no reply in the first slices: a
reply is a message to a stranger, and that needs more safety work than a
one-way inbox.

A summary sits above the list: plays, thumbs up and down, and median play
time, for the live version and for all versions.

## The player's flow, on the web

No account, no install.

| Step | What the player sees |
| --- | --- |
| 1 | Opens `https://GAMES_DOMAIN/g/k7m2p9`. A page with the game's name, the creator's display name, a thumbnail and **Play**. The page is ordinary HTML, so a link pasted in a chat shows a title and picture |
| 2 | Taps Play. The player loads (a few hundred kilobytes of wasm plus the level) and the game starts full-window |
| 3 | Plays. On a phone: touch sticks and buttons. On a desktop: keys and mouse |
| 4 | When a round ends, or from a small menu button at any time: **How was it?** Thumbs up or down, an optional line of text (160 characters), **Send** |
| 5 | The menu also has **Report this game** (pick a reason, optional text) and **Play again** |

A browser without WebGPU gets a page that says so and names browsers that
work. See [the renderer](#the-renderer).

The page shows "Made with GameBuilder" and a link to the app.

## Data model

| Thing | Fields | Notes |
| --- | --- | --- |
| **Account** | id, display name, created, age band, state | See [Accounts](#accounts-and-identity) |
| **Game** | id, owner account, share code, name, live version, state (`live`, `unpublished`, `removed`), listed (always false at first), created | One game per level a creator has shared. The share code is permanent |
| **Version** | game, number (1, 2, 3…), created, level document hash, template id and settings, player build id, thumbnail hash, size, state (`ok`, `removed`) | Immutable once made. "Live" is a pointer on the game, not a property of the version |
| **Blob** | SHA-256, bytes | Level documents, thumbnails and player builds, stored once by content hash |
| **Player build** | id (the SHA-256 of its manifest), template id, wasm hash, shader hash, host script hash, engine commit, GameBuilder commit, created | Uploaded by a GameBuilder release, never by a creator |
| **Feedback** | id, game, version, thumb (up or down), text (0 to 160 characters, filtered), play facts, device class, player token, created, state (`new`, `read`, `hidden`, `held`) | `held` is text waiting on the filter or a moderator |
| **Report** | id, game or feedback item, reason, text, reporter token or account, created, state | Goes to the operator's queue |
| **Block** | owner account, player token | Feedback from a blocked token is dropped |
| **Play** | game, version, day, count, total seconds | Counters only. No per-player record |

### What a version holds

A version is everything needed to play, named by content:

```
version 3 of game k7m2p9
  level     sha256:91c0…   a world.doc.v1 document and its content table (coax/level.coax)
  template  brawl-1        the game template's id
  settings  {…}            the template's settings: mode, round length, roster
  build     sha256:4e2a…   the player build this was published with
  thumb     sha256:77bd…
```

The level file is two Metaverse Engine documents, one after the other (see
`coax/level.coax`): the world (`world.doc.v1`), which carries its own digest
and is refused whole if a byte is wrong, then the content table its kinds
name (`aethyr.content.v1`, table `shape`), whose revision the world names and
whose every row is checked. The server checks both on upload, so it never
stores a level the player would refuse.

### Share links

| Link | Plays |
| --- | --- |
| `https://GAMES_DOMAIN/g/CODE` | The game's live version |
| `https://GAMES_DOMAIN/g/CODE/v/N` | Version N, for the creator to compare or send a tester an older one |

`CODE` is six to eight characters from an alphabet with no look-alikes
(no `0 O 1 l I`), random, not derived from the name. It is short enough to
read out and long enough not to be guessed in bulk. A game that is not
listed is reachable only by its link.

### Play facts

The play core counts a few plain numbers and the page sends them with
feedback and, once per session, to the play counter: seconds played, rounds
finished, knockouts made and taken, and whether the round was won. No
position traces, no input recording, nothing that identifies the player.

### The player token

A random value the page makes on first visit and keeps in the browser's
storage. It exists so that one browser gets one vote per version, so the
creator can block a sender, and so limits can be applied. It is not an
account, carries no personal data and is never shown.

## API

All under `https://GAMES_DOMAIN/api/`. JSON in and out. Creator routes are
signed (see [Accounts](#accounts-and-identity)); player routes are not.

### Creator routes (signed, account required)

| Route | Does |
| --- | --- |
| `POST /games` | Makes a game: name. Answers its id and share code |
| `GET /games` | The creator's games with live version, unread feedback count and totals |
| `PATCH /games/ID` | Rename, set the live version, unpublish or republish |
| `DELETE /games/ID` | Removes the game, its versions and feedback |
| `POST /games/ID/versions` | Publishes: the level document, template id, settings, build id and thumbnail in one request. Answers the version number and link |
| `GET /games/ID/versions` | Versions with plays and thumbs |
| `GET /games/ID/feedback?after=CURSOR` | Feedback, newest first, with a cursor |
| `POST /feedback/ID` | Mark read, hide |
| `POST /blocks` | Block a player token (named by a feedback item, never shown raw) |
| `POST /reports` | Report a feedback item |
| `PUT /push` | Registers this install's push token for feedback notices |
| `GET /builds/current?template=T` | The build id the app should publish with |

Publishing is one request on purpose. A level is tens of kilobytes; a
thumbnail is under 100. There is no upload session to resume or abandon.

### Player routes (no account)

| Route | Does |
| --- | --- |
| `GET /g/CODE` and `/g/CODE/v/N` | The game's page (HTML) |
| `GET /api/play/CODE[/v/N]` | The version's manifest: level hash, template, settings, build manifest |
| `GET /b/SHA256` | A blob by content hash. Cached forever |
| `POST /api/play/CODE/feedback` | Thumb, text, play facts, player token |
| `POST /api/play/CODE/report` | A reason and optional text |
| `POST /api/play/CODE/played` | Adds to the play counters |

### Limits

| Limit | First value |
| --- | --- |
| Level document | 256 KB |
| Thumbnail | 200 KB, JPEG or PNG, re-encoded by the server |
| Games per account | 50 |
| Versions per game | 100 kept; older ones stay playable by link but leave the list |
| Publishes | 30 an hour per account |
| Feedback | One per player token per version; 20 an hour per address |
| Feedback text | 160 characters |
| Reports | 10 an hour per address |

Each limit is a constant in one Coax policy file on the server, so a change
is one line with a check.

## The web player

### What it is

The app's play mode is Coax. A published game runs the same Coax, compiled
to WebAssembly, so a game behaves the same in the app and at its link. This
is the reason the play mode must be written as a self-contained core: a
level document and settings in, input each frame, and vertices and uniforms
out. It must not call anything that exists only in the Swift host.

**Verified.** openagents already builds and hosts Coax this way:

| Piece | Path | What it does |
| --- | --- | --- |
| Coax to wasm | `coax/web/wasm.sh` | Coax to C11 to wasm32 with upstream clang and wasi-libc. Source paths are made checkout-independent so the same code gives the same bytes and the same content-named URL |
| Browser host | `coax/web/host.js` | Owns the WebGPU device and canvas, the frame clock and one `WebAssembly.Memory`. The app exports `coaxial_state_size`, `coaxial_state_init`, `coaxial_frame(state, vertices, uniforms, w, h, dpr, dt, still)` and input calls. The host decides nothing |
| Shaders | `coax wgsl`, `coax metal` | The same Coax shader source emits WGSL for the web and Metal for the app |
| Storage | `coax/web/STORAGE.md`, `byte-storage.js` | IndexedDB as an opaque byte store the Coax app drives |
| Deploy | `coax/web/deploy.sh` | Files live in a bucket a Cloud Run service mounts. Files are named by content hash and old ones are not deleted |
| Build pin | `metaverse-engine/editor/publish.coax` | A published realm records the exact engine manifest (host script, wasm, shaders, packs, each named by content) and plays that, "not whatever the site serves today" |
| Dev and live | `metaverse-engine/editor/world-live.coax` | A world has a dev revision and a live one; a publish copies dev to live; a session never takes a revision its build cannot read |

The decision recorded in `coax/web.md` is **WebGPU only**, with a text
fallback. There is no WebGL renderer in openagents.

### What has to be written

| Piece | Where it lives | Size |
| --- | --- | --- |
| The play core's C face for the web (`coaxial_*` exports over the play mode) | GameBuilder, proprietary | Small |
| A player page: loads the manifest, the build and the level, starts the host, draws touch controls, shows the feedback and report sheets | GameBuilder, proprietary | Medium |
| A trimmed host. `host.js` serves all of openagents.com; the player needs the canvas, the clock, input and storage | Start from `host.js`; anything general goes back to openagents | Small |
| Shaders in Coax, so one source gives Metal and WGSL | GameBuilder. Stage 3 of the integration plan; publishing depends on it | Medium |
| Touch controls for a first-person game in a phone browser | Engine (`core/input.coax` takes touch) plus a GameBuilder layout | Medium |
| The build pipeline: compile the play core to wasm, hash, upload as a player build | GameBuilder `scripts/` | Small |

### The renderer

WebGPU is in current Chrome, Edge and Safari, including Safari on recent
iOS. That covers most phones a shared link will be opened on, but not all:
older iPhones and some Android browsers lack it. Two choices:

| Option | For | Against |
| --- | --- | --- |
| WebGPU only, with a clear "this browser can't play it" page (recommended to start) | Matches openagents; one renderer; nothing new to write | Some players are turned away. The page must say so plainly and count how often |
| Add a WebGL2 path | Reaches nearly every browser | A second renderer to write and keep in step, in the open engine; weeks, not days |

Start with WebGPU only and record how many visits hit the fallback. If the
number is large, a WebGL2 path is engine work for openagents.

### How a version stays playable

This is the rule that matters most, and openagents already follows it for
published realms.

- A **player build** is the play core's wasm, its shaders and the host
  script, each named by its SHA-256, listed in a manifest. The build's id is
  the manifest's hash.
- A build is uploaded once by a GameBuilder release and **never deleted**.
- A version records the build it was published with. The link loads that
  build, whatever the newest one is.
- A level document is read only by a build that knows its format. The app
  asks the server for the current build id when publishing and sends it, so
  a version is always paired with a build that can read it.
- When the play core changes in a way that changes how old levels behave,
  old versions keep their old build. The creator's page shows "Published
  with an older GameBuilder" and offers **Publish update**.
- Security fixes are the exception. A build can be marked **retired**; its
  versions then show "This game needs an update from its creator" and the
  creator gets a notice. This should be rare and is the operator's decision.

The app and the server must agree on the build. The app ships with the id of
the build made from the same commit; if the server does not have it (a
TestFlight build ahead of the server), publishing is refused with "Update
needed" and never falls back to a different build silently.

## Where the service runs

The requirement is separation from openagents.com with reuse of its open
code.

| Option | What it is | For | Against |
| --- | --- | --- | --- |
| A. Routes on openagents.com | Add game routes to the existing API and serve `GAMES_DOMAIN` from it | Least work; accounts already exist | Games sit in the OpenAgents database and service, which the owner has ruled out. Proprietary server code would go in the open repo |
| **B. A separate service that reuses the open code (recommended)** | Its own Cloud Run service, Postgres database and bucket, in its own project or at least its own names, deployed from this repo. It links the open Coax libraries it needs: NIP-98 verification, the world document reader, SHA-256, JSON | Clean separation of data and deploys. Server code stays proprietary. Same patterns the team already runs | A second service to operate |
| C. Static hosting plus a serverless database | Blobs on a CDN, a hosted database for games and feedback | Cheap | Abandons the Coax server pattern; the level's digest check and limits would be rewritten in another language |

**Recommendation: B.** Concretely:

- **Code** lives in this repo under `server/`. Policy (who may publish, the
  limits, what a valid version is, the text filter's verdicts) is Coax with
  checks, as the app's rules are. A thin C or Coax HTTP layer moves facts in
  and answers out, the way `coax/web/api/worlds.c` does over
  `world-policy.coax`.
- **Reused from openagents, read-only through the pin:** `coax/web/api/nip98.coax`
  (request signing), `metaverse-engine/editor/world-doc.coax` (the level's
  form and digest), `coax/lib/crypto`, `coax/lib/json`, and the deploy
  pattern in `coax/web/deploy.sh`.
- **Not shared:** the database, the bucket, sessions, and any list of games.
  Nothing on openagents.com reads GameBuilder's data.
- **Two hostnames** are worth having from the start: `GAMES_DOMAIN` for
  pages and the API, and a separate origin such as `play.GAMES_DOMAIN` for
  the player and blobs. User-made content then runs on an origin that holds
  no creator session, which removes a class of web attacks before scripts or
  custom assets exist.

Whether this runs in the existing Google Cloud project or a new one is an
owner decision (question 2).

## Accounts and identity

**Verified.** The app makes a Nostr key on the device, keeps it in the
Keychain and signs requests with NIP-98 (`coax/decide.coax`,
`host/Sources/Decide.swift`). openagents verifies NIP-98 in Coax
(`coax/web/api/nip98.coax`) and has full account, passkey and device-key
code (`coax/web/api/auth*.c`, `auth-device-policy.coax`).

The device key proves "the same install as before". It is not an account:
it is lost with the phone, it cannot sign in on a second device, and it
says nothing about who the person is.

Proposal:

- **An account is a GameBuilder account**, separate from OpenAgents
  accounts, because the data is separate. Linking the two is the later
  decision the owner reserved.
- **Sign in with Apple** first. It is one tap on the devices the app runs
  on, it needs no password, it works for a second device, and App Review
  expects it when any third-party sign-in is offered. A passkey is a good
  second method later.
- **The device key stays as the request signer.** At sign-in the app
  registers its key with the account; every creator request is NIP-98
  signed by a registered key. The server maps key to account. Losing a
  phone means signing in again on the new one, which registers a new key.
- **The Mac app** signs in with the same account and gets the same games.

### Children

The first user is a child, and the audience for a game-making app will
include many more. This shapes the account design and cannot be bolted on.

- **Ask for an age band at sign-up** (under 13, 13 to 17, 18 and over), not
  a birth date, and keep only the band.
- **Under 13.** In the United States, COPPA requires verifiable parental
  consent before collecting personal information from a child under 13, and
  a display name, a persistent identifier used across sessions, or free
  text can all count. Apple's Kids Category has its own rules. The simplest
  honest position for the first releases: **an under-13 creator publishes
  under a parent's account**, set up by the parent, and has no public
  display name and no free-text feedback shown without the parent's
  setting. Other countries have their own ages and rules (the EU's range
  from 13 to 16).
- **This needs a lawyer's read before any release beyond the family.** The
  spec states the shape; it is not legal advice.

For the build the owner shows their own child, the owner's account is the
parent's account and none of this blocks the demo.

## Moderation, abuse and App Review

User-made games with links and feedback from strangers are user-generated
content. Apple's guideline 1.2 requires an app with such content to have:
a way to filter objectionable material, a way to report it, a way to block
abusive users, and published contact information. The plan meets each one.

| Risk | Control | When |
| --- | --- | --- |
| Abusive feedback text reaching a child | Text is optional. Every text goes through a filter before it is stored as visible; anything the filter doubts is `held` and not shown. A creator in a child band sees thumbs and play facts only unless the parent turns text on | First slice: thumbs only for the demo; filtered text right after |
| A game with offensive content (names, arrangements of blocks, later images) | The name passes the same filter. Report on every game page. Reports go to an operator queue; a game can be set `removed` | First slice has Report and manual removal |
| A stranger contacting a child | No replies, no profiles, no direct messages. Feedback is one-way and anonymous | By design |
| Spam and scripted votes | The limits above, the player token, per-address limits, and a proof the feedback came from a loaded game (a short-lived value in the manifest) | First slice has the limits |
| Public discovery of unreviewed games | **No public listing.** Games are reachable by link only. A listing, when it comes, shows only games a person has reviewed | Listing is a later stage |
| Personal data in a game's name or feedback | The filter flags phone numbers, emails, addresses and links | With filtered text |
| Uploaded images and models | Not in the first stages: levels are boxes. When assets arrive, the engine's packs already require a licence per asset (`metaverse-engine/editor/pack.coax`); images need scanning before they are served | With assets |
| User scripts | Not in the first stages. When they arrive they run in the engine's sandboxed interpreter, and on the separate play origin | With scripts |
| Blocking | A creator can block a feedback sender. A player has nothing to block, because nothing is addressed to them | First slice |

The filter is a good use of the decision model the app already calls: a
typed question ("is this text safe to show a child: yes, no, unsure") with
no free text back. Unsure means held.

**Verified, and a gap.** openagents has room moderation (kick, mute, ban:
`metaverse-engine/session/moderation.coax`, `coax/web/api/moderation-policy.coax`).
I found no report queue for content and no feedback surface for worlds.
Both are new work here.

Other App Review points:

- **2.5.2** (downloaded code). Published games are data and a play core the
  app already contains. The web player is a web page, outside the app.
  Opening someone else's game inside the app later is still data, as long
  as rules are template settings or sandboxed scripts.
- **5.1.1** (data collection) and the privacy label: the app will collect an
  account identifier and user content. The label must say so.
- **4.8**: offering any third-party sign-in requires Sign in with Apple as
  an option, which the proposal already uses.
- A **privacy policy** and a **contact address** on `GAMES_DOMAIN`, and
  terms that say what may be published.

## Feedback back into the app

| Way | What it does | When |
| --- | --- | --- |
| Fetch when the app opens or the game's page is shown | `GET /games` returns unread counts; the inbox pulls the items | First slice. Enough for a creator who shares a link and checks back |
| Fetch while the game's page is open | A request every 20 seconds while that screen is in front | Second slice. Makes "send the link to someone next to you" feel live |
| Push notification | "Someone played Level 3 and left a thumbs up" | Third slice. Needs the push entitlement, a server key for Apple's push service and the creator's permission. Grouped, at most a few a day, never with a stranger's text in it |

Push is worth doing, but not first: it needs an entitlement the app does not
have (`scripts/release.sh` currently skips the tool's push step), and the
text of a notification is another surface the filter has to cover.

## What the Mac app adds

The same account, games, versions and inbox, laid out for a wide window:

- The versions list and the feedback inbox side by side with the editor.
- Open any version in the editor without leaving the level being worked on.
- A table of versions with plays, thumbs and play time for comparison.
- Play the published web build in a window, to check exactly what players
  get.

Nothing is Mac-only. If a flow needs the Mac to complete, it is wrong.

## Stages

| Stage | What ships | Depends on |
| --- | --- | --- |
| P0 | The play core runs as wasm in a local page, loading a level file by hand | The play mode as a self-contained core; shaders in Coax (integration stage 3) |
| **P1, the first slice** | Publish one level from the app. Play it at `GAMES_DOMAIN/g/CODE`. Thumbs up or down plus a line of text. See it in the app. Unpublish | P0; the service (option B) at its smallest: four creator routes, three player routes; Sign in with Apple; the name and text filter |
| P2 | Versions: the list, per-version links, roll back, "unpublished changes". Play counts and play facts. Report and Block. The inbox polls while open | P1 |
| P3 | Push notices. The game page's link preview picture. The Mac layout | P2; the push entitlement |
| P4 | Age bands and the parent's account for under-13 creators, with the lawyer's read done. The privacy policy and terms | Before any release beyond the family and invited testers |
| P5 | A reviewed public listing; opening someone else's game in the app; remixing a copy | P4; a moderation queue someone actually works |
| P6 | Multiplayer games at a link | Server rooms; the largest piece, specified separately |

What P1 leaves out on purpose: versions beyond "the latest", play counts,
push, a listing, replies, assets and scripts.

For the demo in a few hours none of this is needed: the play mode on the
device is the demo. P0 is the first step after it.

## Open questions for the owner

1. **The domain.** What is `GAMES_DOMAIN`? Is a second hostname for the
   player (`play.…`) acceptable?
2. **Where it runs.** The existing Google Cloud project with separate names,
   or a new project for GameBuilder?
3. **Accounts.** Is a GameBuilder account separate from an OpenAgents
   account, as proposed, with Sign in with Apple first?
4. **Children.** Is "an under-13 creator publishes under a parent's
   account" the position to build, and who gets the lawyer's read before a
   wider release?
5. **Feedback text.** Thumbs only for a child's account until a parent
   turns text on, as proposed? Or filtered text for everyone?
6. **WebGPU only** to start, with a count of turned-away visits, or is a
   WebGL2 path needed before the first share?
7. **What the link shows about the creator.** A display name, or nothing?
   For a child's game, nothing is the safe default.
8. **Who works the report queue**, and how fast must a reported game come
   down?
9. **How long old versions stay playable.** For good, as proposed, or for a
   set time after a newer version is live?
10. **Remixing.** Should a player be able to open a copy of a shared game in
    their own app later? It changes what "publish" means for a creator's
    work and should be the creator's choice per game.
