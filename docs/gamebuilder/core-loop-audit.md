# The core loop on the web: audit

Written 2026-11-17, against `origin/main` at `6a285c5` and the live site at
gamebuilder.org. Status: an audit with recommendations. Nothing here is
built by this document.

The loop the owner wants, and wants very tight:

1. A simple page at gamebuilder.org with one massive button, **Create a
   game**.
2. The button goes to the builder with the game types.
3. Anyone can click around the editor themselves, free, with no account.
4. To talk to the AI they log in or create an account first. (The owner
   first proposed about five free messages, then decided on none.)
5. They can publish at any time (an account is needed first) and get a link
   to share.

Read with: [web-world.md](web-world.md) (what the web world is today),
[publishing.md](publishing.md) (the publishing specification),
[agents-audit.md](agents-audit.md), and issues 2, 3 and 4.

## Summary

**Almost none of the loop exists yet.** The world on the web is good, but
every step of the loop is behind a login today, nothing a person builds is
saved, and publishing is a specification.

| Step | Today | Verdict |
| --- | --- | --- |
| 1. A page with one big button | Built 2026-11-17 (issue 12): one heading, one line, "create a game", in the HTML itself. When this audit was written the home page had no button at all. Since issue 30 (2026-11-18) the home page is the create-a-game flow itself: the nine game types as a ring of portals on the meadow, their names in the HTML as links to `/play?type=ID` (`docs/web-world.md`, "The ring") | Built |
| 2. Button to the builder | The builder shows only when logged in. Logged out, `/play` sends you to `/login` | Blocked by login |
| 3. Build free, with no account | Not possible. The world, the picker and the hotbar are drawn only when logged in | Missing |
| 3. About five AI messages free | Not possible. The chat's route answers 401 without a login, and 403 with one unless the account is on the team beta | Missing, and blocked for most accounts even when logged in  (Since 2026-11-17, issue 15: no guest messages by the owner's decision, and any account is answered) |
| 4. Prompt to make an account | Nothing to prompt from | Missing |
| 5. Publish and get a link | `docs/publishing.md` says nothing in it is built. `gamebuilder.org/g/CODE` answers 404  (Since 2026-11-17, issue 17: a publish button in the builder, a link at `gamebuilder.org/g/CODE` that plays with no login, a link preview, versions, unpublish, Remix and report; `docs/publishing.md`, "What is built") | Missing when audited; built |
| Under all of it: keeping what you built | Nothing is saved. A reload starts over | Missing |

The good news: the hard part, a working 3D builder in a browser with nine
game types, is built and live. The loop is mostly gates to move and a few
services to add.

## What was measured

Measured from this Mac on 2026-11-17, a first visit with nothing cached.

| Thing | Measured |
| --- | --- |
| Requests to show the home page | 54 |
| Bytes | 2.5 MB, none of it compressed |
| Time until the page has any words on it | About 2 seconds on a fast connection |
| JavaScript files | 39 modules under `/coaxial/`, fetched one after another as each names the next: about 1.5 seconds pass before the first WebAssembly file is even asked for |
| WebAssembly | `hud.wasm` 670 KB, `site.wasm` 220 KB, `host-layers.wasm` 56 KB, `frame-plan.wasm` 26 KB |
| Fonts | Two TTF files, 270 KB |
| Shader text | `hud.wgsl` 118 KB |
| Pictures | 440 KB; the icon alone is a 65 KB PNG |
| Caching | Every file is `cache-control: no-cache`, so a returning visitor asks for all 54 again |
| The page before its code runs | Black. The words are drawn by `site.wasm`; the HTML holds only a `noscript` line |

**After issue 12** (2026-11-17; measured by `scripts/web-load.sh`, which
fails over the budget, on the site built and laid out as a deploy uploads
it):

| Thing | Measured |
| --- | --- |
| Requests to show the home page | 11: the page, one script, five WebAssembly and shader files, two fonts, and the two login questions |
| Bytes | 716 KB as the bucket holds them (the script, modules and shaders gzipped) |
| Time until the heading and the button are on the screen | About 0.13 seconds from this Mac; they are in the HTML, so they do not wait for the code |
| JavaScript files | One, 142 KB gzipped |
| Fonts | Two WOFF2 files, 97 KB |
| Pictures | None: the icon and the mark are inside the page |
| Caching | The files are under `/b/NAME/`, kept for a year and never changed; only the pages are `no-cache`, so a returning visitor asks for the page alone |

Routes, asked with no login:

| Route | Answer |
| --- | --- |
| `GET /` | 200, the home page |
| `GET /play` | 200, the same page, which then sends you to `/login` |
| `POST /api/chat/complete/v1` | 401 |
| `GET /g/abc123` | 404 |
| `GET /agents` | 404 |

## Step by step

### 1. The home page

Built by issue 12 (2026-11-17): the page is one heading ("make a game.
right here."), one line, one massive "create a game" button that is a link
to `/play`, and one small line for the apps. The heading and the button are
in `web/index.html` itself, written there from `web/home.coax` by
`tools/home-check.coax`. What follows is the page as it was.

`web/page.coax` draws: "make games. play them.", a line, "iphone / ipad /
mac", one picture, a three-step "how" band with two phone pictures, and a
footer. There is no button. The top right says "log in".

Gotchas:

- **The page sells the apps, not the web builder.** "iphone / ipad / mac"
  and two phone screenshots. The three steps say "copy a game. drag
  blocks." and "party up", which are the app's. A person arriving from a
  link does not learn they can build right here, now.
- **The page is blank until 2.5 MB arrives and runs.** On a phone
  connection that is many seconds of black. The button must be in the HTML
  itself, so it shows at once and can be pressed while the rest loads.
- **The picture is of Scrappers Arena**, a game the web builder cannot make
  yet.

### 2. From the button to the builder

Today: `/login`, type an email, wait for an email, type a code, then the
picker ("pick a game."), then a card, then the entrance.

Gotchas:

- **The login is the first thing asked for, before any value is shown.**
  An email code is the slowest kind of login: leave the page, open mail,
  find the message, come back. Most people who press a button to try
  something will leave here.
- **The entrance is long.** Picking a card starts a camera descent, grass
  that spreads over 5 seconds and a sunrise that takes 12
  (`docs/web-world.md`, "Game types"). It is lovely once. For a tight loop
  the person should be able to move and build within a second or two, with
  the sunrise going on around them.
- **The picker shows every time you arrive**, including on a reload. A
  returning person should land back in what they were building.
- **WebGPU is required.** Without it the canvas is hidden
  (`html:not(.gpu) #coaxial{display:none}`) and the page draws plain
  frames. I found no words telling a person that their browser cannot run
  the builder, or what to do. Older Safari and Firefox versions, and many
  phones, are in this group. Count them before launch.
- **The picker's nine cards load nine pictures**, about 32 to 52 KB each.
  Fine, but they should be asked for the moment the button is pressed, not
  after the login.

### 3. Building with no account

Today the world is drawn only when `login.step == LG_IN`
(`web/page.coax`, `gs_tree`). The building itself needs no server: the
blocks, the hotbar, edit mode, the physics and every game type run in the
browser. So letting a guest build is mostly a matter of drawing the world
for someone who is not logged in.

Gotchas:

- **Nothing is saved.** A guest who builds for ten minutes and reloads, or
  whose phone sleeps the tab, loses everything. This is the worst gap in
  the loop. A guest's world must be kept in the browser as they go, and
  come back on a reload.
- **There is no undo on the web.** In an editor people click around in,
  that is felt at once.
- **There is no game document on the web.** The picker starts a world; it
  does not make a game with a name that can later be saved to an account or
  published. Issue 2 is fixing exactly this (one creation process, the same
  level file as the app).
- **The guest's work has to survive logging in.** The login is on the same
  page with no reload, which helps, but today logging out clears the world
  and the choice of type. The moment of logging in must carry the guest's
  game onto the new account, not start over.

### 3b. A few free AI messages

Today the chat is `POST /api/chat/complete/v1` on OpenAgents' account chat.
It needs a login (401 without one). `web/chat.coax` has a test for the
answer "Agent chat is currently available to the team beta." (403), so an
ordinary logged-in account may not be able to chat at all today. The build
agent has the same team-beta rule. (Both gates came off on 2026-11-17, issue
15: see "Decisions for the owner".)

Gotchas:

- **A guest allowance needs a server-side count.** Counting in the browser
  is cleared with the browser's storage. The service has to count by
  something it controls: a signed guest token set as a cookie, with a
  limit per address as well. This is work in the OpenAgents API, not in
  this repository.
- **It is an open tap on a paid model.** Five messages a guest with no
  account is free inference for anyone who scripts it. It needs a cap per
  address, a cap per day for the whole site, a small model, a short answer
  limit, and a kill switch. Decide the daily budget before it ships.
- **The team-beta gate must come off for this route**, or the prompt to
  "create an account to keep talking" leads to an account that still
  cannot talk. (Done 2026-11-17, for the chat and the build agent.)
- **Say the count.** "3 of 5 free messages left", in words, and when they
  are gone, one line and one button: log in or create an account to keep
  going. Never a silent refusal.
- **What a new account gets** has to be decided too: unlimited, a daily
  number, or Credits. Otherwise the prompt just moves the wall.

### 4. Making an account

Today: one flow, email then a code, on `/login`. The page has no words
about creating an account; it says "log in".

Gotchas:

- **I did not confirm that a new email makes a new account** through
  `POST /api/auth/email/start`. If it does not, there is no way to sign up
  on this site at all. Check this first.
  (Since issue 16, 2026-11-17: it does. The login service makes an account
  for an email it has not seen when the code is verified, and answers
  `"new":true` (`coax/web/api/auth.c` in the pinned tree). The sheet over
  the game now says "log in or create an account." and that a new email
  makes a new account. The owner decided: no other sign-up method. The
  guest's game is kept on the new account through the hosted levels API,
  with the same rows and level file as every surface: docs/web-world.md,
  "Kept on your account". Seen live only by the owner: NEEDS_OWNER.md.)
- **The prompt must not leave the builder.** A small sheet over the world,
  not a move to `/login`, so the person sees their game behind it.
- **An email code costs a minute and a tab switch.** A passkey or "continue
  with Apple or Google" would be far quicker. The login service already has
  passkeys for other apps.
- **Children.** `docs/publishing.md` says the first user is a child. An
  email login and a public share link both need the age question answered
  before this is opened to the public.

### 5. Publishing and the link

Built since (issue 17): see `docs/publishing.md`, "What is built". The link is
not on a frozen release: a published game plays with the site's current
code. When this audit was written: nothing. `docs/publishing.md` is a careful specification (an
immutable snapshot, the player build named by content hash, a short code,
`/g/CODE`, thumbs up or down), written for publishing from the phone app.

Gotchas:

- **The specification assumes the app's play mode compiled for the web.**
  The web builder is a different world from the app's play mode today.
  The quick path for this loop is the reverse: the published page is the
  web world itself, loaded with the saved level and with the build tools
  off. That needs the web world to load a level from data, which the save
  work above gives.
- **A published link must keep working when the site changes.** The
  `/player-releases/NAME` copies made for player testing are already
  frozen copies of the whole site. A published game can name the release it
  was made on, which is the specification's "player build by content hash"
  for almost no new work.
- **The link needs a preview.** A shared link with no picture and no title
  gets few clicks. Each game needs its own title, description and picture
  in the HTML the link returns, which a single static page cannot give:
  the server has to write them for `/g/CODE`.
- **A person who opens a link wants to play, then make their own.** The
  published page should end in the same big button: **Create a game**, or
  **Remix this one**. That closes the loop.
- **Moderation.** A public link to anything a stranger typed or built is
  the risky part. The specification's first slice has no public listing and
  filters names. Keep both.
- **Limits.** How many games an account may publish, how large a level,
  and unpublish, all said in words.

## Slowdowns, in order of cost

1. **39 script files fetched in a chain.** Bundle them into one file. This
   alone should take more than a second off every first visit.
2. **No compression.** JavaScript, WGSL and WebAssembly compress to roughly
   a third. The bucket serves them as stored. Turn on compression at the
   load balancer or store them compressed.
3. **No caching.** Give built files content-hashed names and a long cache
   life; keep `no-cache` for the HTML only.
4. **Everything loads before anything shows.** Put the heading and the
   button in the HTML and style them there, so the first paint needs no
   code. Load the world's 670 KB while the person reads.
5. **The home page loads the whole world's code** although a logged-out
   visitor sees only words and three pictures. Once the button is in the
   HTML, the world can start loading on arrival and be ready when it is
   pressed.
6. **Fonts as TTF.** WOFF2 is about half the size.
7. **The 12 second entrance.** Let the person act from the first second.
8. **Pictures.** `icon.png` at 65 KB for a small mark; the two phone
   pictures are below the fold and can load late.

A fair target: the button visible in under half a second, the builder
usable within three seconds of pressing it on a good connection.

## What is missing, as a list

- The button, and a home page about building here.
- The builder for a guest.
- A guest's game kept in the browser, and undo.
- A game document on the web (issue 2).
- Guest AI messages with a server-side count and a budget.
- The chat open to ordinary accounts.
- A sign-up that is known to work, shown as a sheet over the world.
- The guest's game carried onto the new account.
- Games saved on the account (the hosted API already keeps levels for an
  account; the web world does not use it).
- Publish, `/g/CODE`, the link's preview, unpublish, and the published
  page's own button.
- Words for a browser with no WebGPU.
- Counting: how many arrive, press the button, place a block, send a
  message, see the prompt, make an account, publish. Without this the loop
  cannot be tightened.

## Recommended order

| Step | What ships | Why here |
| --- | --- | --- |
| 1 | The home page: one heading, one line, the button, in the HTML itself. Bundle, compress and cache the files | The first impression and the load time, with no dependency on anything |
| 2 | The builder for a guest: the button goes straight to the picker, no login. The guest's world is kept in the browser and comes back on a reload | The whole top of the loop. Building needs no server |
| 3 | A short entrance, and words for no WebGPU | Removes the two ways to lose someone in the first ten seconds |
| 4 | Guest AI messages: the count on the server, a budget, the count said in words, then the sheet | Needs work in the OpenAgents API; start it alongside step 2 |
| 5 | The account sheet over the world; the guest's game moves onto the account and is saved there | Uses the hosted API's levels and issue 2's level file |
| 6 | Publish: `/g/CODE` serving the web world with the level and the build tools off, on a frozen release, with a preview and **Remix** | The end of the loop, and the start of the next person's |
| 7 | Counting each step | So the next round of tightening is aimed |

Steps 1 to 3 can start now and need nothing from the open issues. Step 5
leans on issue 2 (the same level file on web and desktop) and issue 3 (the
HTTP API at parity). Step 6 is a narrower, web-first cut of
`docs/publishing.md`.

## Decisions for the owner

**Decided 2026-11-17:** a guest gets no AI messages. Building is free with no
account; talking to the AI needs a login. So the guest count, the guest
token and the guest budget in "3b" and in step 4 below are not to be built:
step 4 is now only that the chat works for an ordinary account (issue 15).
Decision 1 below is closed by this.

**Done 2026-11-17 (issue 15):** an account gets unlimited AI chat for now,
inside the service's own limits. Before, an account off the team beta was
told `403 Agent chat is currently available to the team beta.` by
`POST /api/chat/complete/v1` (openagents `coax/web/api/account-chat.c`,
`act_funded`). Now that route answers any person's own account when it is
asked from gamebuilder.org (`act_funded_from`, openagents `INVARIANTS.md`).
What bounds an account is its day, 500 requests and $2.00 of model use, and
an answer's 2,048 tokens; a day's limit is said by the service in words with
its number ("this account's 500 requests for today are used up"), and the
page shows those words. A guest's message, the decide button and a build
agent's start make no request at all (`web/chat.coax`, `ch_guest`, with its
test). A build agent (the offer the chat makes for bigger work) is any
account's too, by the owner's second decision the same day: its durable turn
is stored as admitted for a build agent, and the hosted runner's second
admission reads that from the turn (openagents `INVARIANTS.md`, "A
GameBuilder build agent is any person's own account's"). Its limits: 32
turns queued or running on an account, one model turn of 2,048 tokens, and
the same day of 500 requests and $2.00.

1. **The guest AI budget.** How many messages (five is proposed), and how
   many dollars a day the whole site may spend on guests before the tap
   closes.
2. **What an account gets.** Unlimited chat, a daily number, or Credits.
3. **How to sign up.** Keep the email code, or add a passkey or a
   one-press provider.
4. **Who may publish.** Any account, or only after the age question.
5. **Does a guest pick a game type first,** or land straight in one (the
   third-person meadow) with the picker one press away? Landing in one is
   faster; the picker shows more.

## What I did not check

- Whether `POST /api/auth/email/start` makes an account for an email it has
  not seen.
- What a logged-in account that is not on the team beta is told by the
  chat today; I read the test for the 403, I did not log in as one.
  (Answered from the service's code and its suite on 2026-11-17, issue 15:
  it was the 403, and is now an answer. A live check with a fresh account
  is the owner's, in the workspace's `NEEDS_OWNER.md`.)
- Load times on a phone or a slow connection; the numbers above are from a
  Mac on a fast one.
- How many of our visitors' browsers have WebGPU.
- The live page for a logged-in person; I audited that from the code and
  `docs/web-world.md`.
