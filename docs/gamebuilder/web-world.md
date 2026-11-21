# The web world: log in, land on the grid, talk to GameBuilder

Written 2026-11-16. This replaces the Claude-first plan in
`docs/agent-mode-audit.md` as the default flow. Claude becomes an optional
connection later; what works with no setup is GameBuilder's own agent, a
router over a tree of game routes, powered by Gemini hosted by OpenAgents.
The page never names the model.

Status is at the end. In short: the world, the character and the chat are
built in the site and tested, and the login service knows gamebuilder.org. A
real login and a real answer on gamebuilder.org have not been seen yet.

## The flow

1. `/` is the home page: the meadow, its grass and sky, with the nine game
   types as a ring of portals round the camera ("The ring"). Pressing one
   flies through it into that type's world, as a guest.
2. `/login` leans the camera back 45 degrees. You type your email, then the
   code.
3. The moment you are logged in, the address becomes `/` again and you pick a
   game type over the same tilted grid: third person, first person, top-down,
   side-scroller, the medieval village or crates ("Game types"). Picking one starts its entrance: the
   camera keeps going, down and in, to that type's view of a character
   standing on the same grid (third person: behind it). The sky is pitch black; grass spreads from the character's feet
   and the sun comes up ahead, and four seconds later it is day ("The
   sky"). You move, look and build from the world's first frame: nothing
   waits for the entrance ("The first second"). Logged in, the home page is the world: your email and "log out"
   in the top bar, the chat at the foot.
4. Escape gives the keys to your feet: you run with W A S D and jump with
   Space (twice quickly, you fly). Drag the world to look round, turn the wheel
   to zoom ("The camera").
5. Enter puts the caret in the chat box. You type, press Enter, and
   GameBuilder routes it and answers. Escape gives the keys back to your feet.

No page loads between any of these. Every page asks who is logged in as it
opens. Logged in, `/`, `/login` and the builder's address `/play` all show the
world at `/`. Logged out, `/` is the home page and `/play` is the builder all
the same ("A guest"). Logging out returns to the home page, and the game built
is still the builder's ("Kept in this browser").

## A guest

**Nobody needs an account to build** (issue 13, 2026-11-17). `/play`, the
builder's address, opens for anyone: the game type picker with no login, then
that type's world with every build tool on its hotbar and in "more". The
home page's ring goes there: a portal picked asks for the address with its
type, and `gs_settle` in `web/page.coax` shows the world at once, whoever is
here. Before the page's code is up each name is a link, `/play?type=ID`, which
opens the builder with that type picked.

What a guest does not get is the agent: **no message of a guest's goes to the
model, none at all** (the owner's decision). Where the chat's box would be
there are words, "log in or create an account to talk to the agent.", and one
button, "log in" (`ch_section` in `web/chat.coax`; `Chat.here` is who is here,
set from the login by `gs_here`). ✦ decide, the agent's button on the hotbar,
says the same words over the hotbar when a guest presses it. Nothing a guest
presses or types makes a request to the chat's door: `ch_send` refuses before
it builds one, and the checks hold that (`web/chat.coax` "a guest gets no
message to the agent", `web/page.coax` "a guest: the chat's box says").

The account icon is there for a guest too. Its menu: "game type" (back to the
picker), "credits", and "log in". **Log in opens over the world**, as a panel
in the middle of the window with "keep building" under it; the world stays as
built behind it, drawn and running. Once the code is taken the panel goes,
the chat's box is a box, and the address becomes `/`: the same world, the
same things, no picker. (From the home page "log in" is still the login's own
page; a game that is up is up again after it.)

**Logging out** goes to the home page and forgets the conversation. It does
not forget the game: its things, its type, its camera and its settings stand,
in the page and in this browser, and `/play` is that game again.

## Kept in this browser

**What you build comes back on a reload**, on the type you picked, where you
left it, with no account (`web/keep.coax`, `web/level.coax`). A returning
person lands in their game, not on the picker; the picker is one press away
("game type" in the account menu, or in more).

**The kept form is the app's level file.** There is no second format: a kept
level is `world.doc.v1` (the engine's `world_save`) followed by the content
shape table (`aethyr.content.v1`, `shape id kind:text sx:cm sy:cm sz:cm`),
byte for byte what `coax/level.coax` writes and reads. A thing is an entity
at its place in metres (Y its bottom), its yaw in radians, its kind the row
of the shape table that has its size, one own column of 100 (its scale in
percent). The kind's word is the shared one (`shared/kinds.coax`: `block`,
`ball`, `wall`, `boost`, `cone`...; a village building is its model's word,
its row the model's own size and its own column its scale, 50 to 300).
`tools/game-types-check.coax` holds both directions: the file the website
keeps of a new game opens in the app (`level_open`) as that level, and the
file the app saves (`ed_save`) opens on the website (`lv_read`).

Three records a game type, in the browser's IndexedDB through the vendored
host's byte storage (`coax/web/STORAGE.md`: database `coaxial-bytes`,
namespace `gamebuilder.game`, version 1):

| Key | What |
| --- | --- |
| `game` | The game being built: the app's game document (`aethyr.content.v1 game`, its `game` and `levels` tables), whose template is the type and whose level is the type's number |
| `level-N` | Game type N - 1's level file; `level-10` and `level-11` are the crates game's second and third rooms (its game is three levels) |
| `settings-N` | That level's settings that differ from the world's start, as the registry's own text (`tun_save`: `path=value` a line; a thing's own settings name it by its place in the level) |

**Each game type keeps its own level.** Picking another type writes the one
you leave first and reads the other's; a type with nothing kept opens as a
new game of it, as before. "Reset world" is how a level is started over (and
it can be undone).

**When.** The page asks for `game` as it opens, whoever is here. While that
answer is out the builder shows neither the picker nor a world (a few
milliseconds; after two seconds it stops waiting and shows the picker). With
a game kept, its level and settings are read and the world comes up as it:
the page's class ends `still` (`gs v2 g7 still`: the settled start's seam,
`gs_still`), so the world is there as it was left with no entrance played. A change to the world's list is written
0.7 seconds after it, a burst of changes as one write, and at once when you
leave the world or pick another type. The same bytes are never written twice.

**What it says.** In the world's bottom right corner (under the account icon
on a phone), small: "kept in this browser". When it is not, the note is
bright and says why, and the reason is said once over the hotbar, in words
with the number:

| What | Its words |
| --- | --- |
| The browser keeps nothing (storage off, a private window that refuses it, no answer) | corner: "not kept: this browser's storage is off". You still build with everything; nothing more is asked of storage |
| The browser has no room | "this browser's storage is full: your game (12 kb) is not kept. free some space and it will be." and the corner "not kept: this browser's storage is full", until a write is kept (each change tries again) |
| The level is over the host's record limit | "this game is too big to keep: 1172 kb, and a kept game is 1024 kb at most. delete some things." (a full world of 1,000 things is under 262 kb: the check holds that it fits) |

**Not kept:** the conversation, undo's history, where the character stands
and where the camera looks (the world opens at its start, as a level opened
in the app does), and the hotbar's passing state (the picked tool, edit mode,
the selection). The hotbar's order is kept apart, as before ("Rearranging").

**Whose it is.** The browser's: logging in or out changes nothing in it.
Logged in, the game is kept on the account as well ("Kept on your account").

## Kept on your account

**A game built as a guest is kept on the account once you log in or create
one, without leaving the game** (issue 16, `web/account.coax`). "log in" (the
chat's button, or the account menu) opens the login as a sheet over the
world; over a game its heading is "log in or create an account." and it says
"your game stays as it is, and is kept on your account. a new email makes a
new account. we email you a code." Sign-up is that email-code login and
nothing else: the login service makes an account for an email it has not
seen. When the code is taken the sheet goes, the world is as it was, and
within a second or two the page says "your game is kept on your account now:
it opens in the app, and in any browser you log in on."

**Not a save path of the website's own.** The game is the app's game (the row
`new-game` with the type's id) and its level is the app's level file, the
same bytes this browser keeps (`web/level.coax`) but for the level's number.
The requests are the rows every surface uses, at the hosted levels API on
this site (docs/programmatic-access.md): `list-games`, `list-levels`,
`new-game`, `read-content`, `write-content`. The browser's login is the login
service's cookie; the hosted API takes it only from this site's own pages, as
a POST (`coax/api.coax` `gb_api_cookie_call`). Each pass first asks the login
service who is logged in (`GET /api/auth/account`), so a session the service
renews is renewed in the browser before the API is asked.
`tools/web-account-check.coax` (run by `cli/check.sh`) runs the page's own
module against the real `gamebuilder serve --accounts` and fails if the two
drift: the account's file must be the page's file byte for byte, a second
browser must get the same things in the same places, and a file the app's
core wrote must open on the page.

**When.** As you log in; four seconds after a change; and every minute while
a game is up (so a change made in the app or another browser arrives). A
failed save is tried again 15 seconds on.

**The rule when both copies exist**, the same as the app's sync
(`coax/sync.coax`), each said over the hotbar when it happens:

| What changed since both last agreed | What happens | Its words |
| --- | --- | --- |
| Only this browser's copy | It is written to the account, from the revision agreed on | none (the corner's note) |
| Only the account's copy | The account's copy opens here | "your account has a newer copy of this game: it is open here now." |
| Both | Neither is written over: this browser's copy is kept as a new game on the account and stays open; the account's stays as it is, under "your games" | "this game changed here and on your account. nothing is lost: this browser's copy is kept as a new game, and the account's is under your games." |
| The game is gone from the account | This browser's copy is kept as a new game | "this game is no longer on your account. this browser's copy is kept there as a new game." |
| The account's copy has things this page can't show (the app's own kinds) | Both are left as they are | "your account's copy of this game has things this page can't show. it is left as it is: open it in the app." |

A guest's built game is always kept as a **new** game: it never writes over a
game already on the account. A game is made once: its key is written to this
browser's record before the game is asked for, so a reload in between makes
no second game. Another account logged in on the same browser gets what was
built here as a new game of its own; the first account's games are not
touched.

**Your games.** Logged in, the picker lists the account's games under its
cards, each with its type ("Game 2 · racing"). One this page builds is a
button: it opens here as its type ("opened from your account."). If this
browser has a built game of that type of its own, that one is first kept on
the account (as above), so opening never loses it. A game of a type only the
app plays is listed as "· in the app". The list holds 32 games and says how
many more there are. A browser holds one game a type; the account may hold
more, and each is opened from this list.

**What the corner says**, in place of the browser's note while logged in:
"saving to your account", "kept on your account", and bright: "kept in this
browser only" (over a limit, or refused) and "not on your account yet" (a
save failed and is being tried again).

**Limits, said in words with the number.** The account holds what the app
holds (`shared/game-types.coax` `GT_LEVEL_*`, the app's own numbers from one
source): 512 things and 262,144 bytes a level (the village's buildings are
things among them).
The website's world holds 1,000 things, so a bigger game is not sent: it
stays in this browser, the account's copy stays as it was, and the page says
why. The app's limits are not raised here: the page refuses in words instead
(`tools/web-account-check.coax` holds that the app's core refuses the same
file, and `tools/game-types-check.coax` that 512 things open in the app and 513
do not).

| What | Its words |
| --- | --- |
| More than 512 things | "kept in this browser only: a game on your account holds 512 things at most, and this one has 513. delete some and it is kept there too." |
| A level file over 256 kb | "kept in this browser only: a game on your account is 256 kb at most, and this one is 257 kb. delete some and it is kept there too." |
| The account refuses the file | "not kept on your account: " and the account's own words, then "it is kept in this browser." |
| No answer, or a server's error | "not kept on your account yet: can't reach the server. it is kept in this browser, and tried again." (or "the server answered 503.") |
| The login ended | "log in again to keep your game on your account. it is kept in this browser." |

**A failed save never loses the browser's copy.** Nothing the account does or
fails to do changes what this browser keeps: the level is written to the
browser as ever ("Kept in this browser"), and the world on screen is replaced
only by a copy that was read whole and is a level this page builds.

**Not kept on the account:** a level's settings that differ (they are this
browser's, beside its level: the level file does not hold them), the hotbar,
the conversation and undo's history.

**The app has the same games.** The app's sync (`coax/sync.coax`, a pass every
10 seconds while logged in) reads and writes games through the same rows as
this page (`list-games`, `new-game`, `read-content`, `write-content`, and
`rename-game`, `move-level`, `delete-level`, `delete-game`), so a game made
here opens in the app as a game of its type, and a game made in the app is
under "your games" here as its type and opens here. A game of a type this page
does not build is listed too, and picking it says "that game is one the app
plays: open it there." `tools/sync-games-check.coax` (run by `cli/check.sh`)
runs the app's own sync and this page's own module against the same real
`gamebuilder serve --accounts` and fails if either direction stops working.
The app's rules, each said in the line under its toolbar when it happens:

| What changed since both last agreed | What happens | The app's words |
| --- | --- | --- |
| A game is on one side only, never agreed | It is made on the other as a game of its type, with its name | "Game 2 came from your account." when it came to the app |
| A level only on one side changed | It goes to the other | none |
| A level changed on both | The account's keeps its number and its game; the app's copy becomes a new level, and when the level is in a game, a new game of the same type and name, on both | "Level 1 changed here and on your account. Nothing is lost: ..." |
| A game's name or levels changed on one side | The other side is made the same | none |
| A game's name or levels changed on both | The account's comes to the app; the app's is kept as a new game with copies of its levels | "Game 1 changed here and on your account. Nothing is lost: your account's is here now, and this device's is kept as a new game." |
| A level or game deleted in the app, the account's as agreed | It is deleted on the account, and no pass brings it back | none |
| A level deleted on the account, the app's as agreed | It is deleted in the app; a copy of its file is set aside in the Levels folder's `deleted` folder | "Level 3 was deleted on your account, so it is deleted here too." |
| A game deleted on the account, the app's as agreed | It is no longer a game in the app; its levels stay, in no game | "Game 1 was deleted on your account, so it is no longer a game here. Its levels stay, in no game." |
| Deleted on one side, changed on the other | The changed one is kept, and comes back to the side that deleted it | none |

A deletion is only ever made on the say of the same account the device last
agreed with: after logging in as another account, with a `sync` file from
before this rule, or when the account lists no level at all, nothing is
deleted. **Not done yet:** a game of a type other than the arena holds one
level on the account through these rows (`move-level` refuses a level into a
game of another type), so a second level added to such a game in the app stays
on the device's side of that game; the order of a game's levels is each
side's own; and a game's owner is each side's own.

This browser's record of which game is which (`web/keep.coax`, key
`account`) is one small text: `gamebuilder.web-account.v1 ACCOUNT`, then a
line a game type, `KIND GAME LEVEL REVISION SUM BUILT KEY`.

## Publish

**A game is published from the builder at any time, and the link plays it
for anyone** (issue 17; `web/publish.coax`, `web/page.coax`;
[publishing.md](publishing.md), "What is built"). The top bar's **publish**
opens a sheet over the game. A guest gets the account sheet first, and when
the code is taken the publish sheet opens over the same game. The sheet has
the game's name (the game type's name until it is typed over), **publish**,
and the limits in words: "a published game holds 512 things, 64 different
shapes and 256 kb at most. an account publishes 16 games, 30 times an hour,
and its last 128 versions are kept."

Pressing publish keeps the game on the account first ("Kept on your
account"), then calls the row `publish`: the same row, on the same level
file, as the app and the command line. Then the sheet shows the link
(`https://gamebuilder.org/g/CODE`), **copy link**, **share** (shown only
where the browser has a share sheet: `navigator.share`), **play it**, the
last six versions with **make live**, and **unpublish**. A game the account
cannot hold (too many things, too big a file) is not published, and the
sheet says which limit with its number.

**The link's page** is this site's own page at `/g/CODE`: the world of that
game type with the published level in it, played with the same code as the
builder's play, and no build tools, no chat, no hotbar's building items and
no account. A small panel at the bottom left has the game's name, "made with
gamebuilder", **remix**, **create a game** and **report**, and the top bar
has **create a game** alone. Nothing is kept in the visitor's browser by playing.
**remix** opens a copy in the visitor's own builder at `/play`, as a guest;
if a game of that type is already built in this browser the page asks first
("replace it" or "keep mine"). **report** offers four reasons and sends the
one picked; the page then says it was reported. A link that plays nothing
(unpublished, taken down, never made) says "this game isn't available." and
offers **create a game**.

**The link's page counts its game** ([publishing.md](publishing.md), "Play
counts and remix lineage"): the game opened, still open a minute later, and
remix pressed, each sent once to `POST /api/play/CODE/played` with nothing
about the visitor. A program's browser is not counted. **remix** also keeps
the link's code beside the copy in this browser, and the copy's first
publish sends it, so the published remix says which game it came from.

Checked: `web/publish.coax`'s tests (the sheet's states, the limits' words,
a link's answer read), `web/page.coax`'s three ("publish without leaving the
game", "a published game's link plays it with no login and no build tools",
"a link's page: a link that plays nothing"), and
`tools/web-publish-check.coax` against the real hosted server.

Seen in headless Chrome with WebGPU (2026-11-17) on the dev server, with a
local `gamebuilder serve --accounts` behind a throwaway proxy that stood in
for the load balancer and the login service (any email, any code), at 1440
by 900 and at a phone's 390 by 844 with touch: racing picked as a guest, a
boost and three cones placed; publish pressed, the account sheet over the
game, the code taken, the publish sheet over the same game; named and
published; the link shown, "copy link" turned to "copied", share shown (that
Chrome has `navigator.share`); published again (version 2), version 1 made
live, unpublished (the link's page then 404 and "this game isn't
available."), published again. The link opened in a fresh browser profile
with no login: the title and preview tags in the HTML, the game's name, the
published things on the track, the car driven with W, no hotbar and no chat;
report sent (a line in `.reports`); Remix opened `/play` as a guest with the
same things and "kept in this browser". Not seen: the real login service, a
real phone, the system share sheet itself, and the clipboard's content
(headless Chrome refuses the read).

## Undo and redo

**Every edit is a step that can be taken back and made again** (`web/undo.coax`),
whoever made it: a tool's placement, a move or a size in edit mode, a
duplicate or a delete, a setting's slider, the camera picked, "reset world",
and the plan lines of one of the agent's answers, which are one step however
many they are (as an agent's batch is one step in the app).

| | Keys | Hotbar |
| --- | --- | --- |
| Undo | Ctrl+Z, ⌘Z | ↶ undo |
| Redo | Ctrl+Shift+Z, ⇧⌘Z, Ctrl+Y | ↷ redo |

The two items stand at the end of the bar where there is room (the meadow's
types, crates, racing, drift); the platformer's and the village's bars are
full, and theirs are in more, from where they can be put on the bar. In a
field the keys stay the browser's own, for what is typed there.

Nothing in undo knows the edits. The page marks the world's list before an
event that may edit it and compares after it (`un_begin`, `un_end` round the
event in `gs_event`), and the step is what differs: things that came, went or
changed, and settings that changed. Where things came to rest (the HUD's
reports) is not an edit and is no step. The wheel sizing one thing is one
step for its notches within 0.6 seconds.

**Its limits, in words.** The history holds the last 100 steps of this visit
(and 2,048 changed things between them; the oldest steps go first). With
nothing to take back: "nothing to undo: undo goes back 100 steps, from when
this page opened." With nothing to make again: "nothing to redo." A reload,
a game type picked and a level read back each start with no history.

## Who does what

One page, two Coax modules, as before (`AGENTS.md`, "The website"):

| Part | File | What it owns |
| --- | --- | --- |
| The page | `web/page.coax` | Which view is shown, the address, the top bar, the chat box and the transcript |
| The login | `web/login.coax` | The email-code login |
| Kept on your account | `web/account.coax` | A logged-in person's game on their account, through the rows every surface uses; the rule when both copies changed; the account's limits in words |
| The chat | `web/chat.coax` | The router and the conversation: the route tree, what is sent, what came back, what is shown |
| The game types and kinds | `shared/game-types.coax`, `shared/kinds.coax` | The one list of game types (ids, names, lines, pictures, the credit), each type's starter things, bounds and kinds, and a type's own kinds of block (pad, spikes, shard, checkpoint; wall, goal; boost, cone) with their words: shared by the page, the HUD and the app (AGENTS.md, "One way to make a game") |
| The world and the HUD | `web/hud.coax` | An OpenAgents Metaverse Engine scene: the camera, the ground and its grass, the blocks as drawn and as solid things, the character and how it moves; and the panels' frames |
| Its shaders | `web/gpu/hud.coax` | Names the engine's shaders (stylized shading, the ghost) and adds the HUD's strokes and the page's dimming of the ground. The build emits `web/build/hud.wgsl` from it; no WGSL is written by hand |
| The model | `web/ghost.coax`, `web/ghost-data.coax` | The character's mesh, rig and clips (baked by `tools/ghost-bake.coax`), the clip that plays, and the skinning |
| Glue | `web/hud-glue.js` | Tells the HUD the panels' boxes, the pointer, the scroll, the view, the keys held, the blocks, whether a press began on one of the page's own things, and the wheel; puts where the HUD says the character is into the page's hidden field; puts the caret in the chat box. It passes raw input: what the camera does with it is the HUD's |

The character lives in the HUD module because it draws every frame and owns
the camera. The HUD tells the page where the character is (its action 106,
`X Z FX FZ` in hundredths of a square, at most four times a second), and the
glue puts that into the page's hidden field.

## The engine

Everything in three dimensions is the OpenAgents Metaverse Engine's
(`vendor/openagents/metaverse-engine`), per the first rule in `AGENTS.md`.
Home, login and world are one engine scene. What it uses:

| What | Engine module |
| --- | --- |
| The frame: camera, light, haze, the sun's shadow box | `render/shade/stylized-frame.coax` (`sty_frame`, `sty_shadow_box`) |
| The world's camera: the free orbit round the character, its zoom, first person, its room before blocks and over the ground | `cam/camera.coax` (`Orbit`, `metaverse_orbit_start`, `_room`, `_limit`, `_shot`, `_unseen`), its shot from `cam/shot.coax` |
| What turns it: a drag, the wheel, a pinch, the held keys, the follow | `cam/chase.coax` (`cam_drag`, `cam_wheel`, `cam_pinch`, `cam_chase_orbit`), as Everglade wires them |
| A phone's thumbs | `core/sticks.coax` (`metaverse_sticks_world_touch`), as Everglade's face wires them |
| The sky, the ground, the blocks' shading, soft shadows | `render/shade/stylized.coax` |
| The sunrise's clock: the look from black to day, where the sun is | `sty_sunrise` (`stylized-frame.coax`), added to the engine for this page |
| The sky itself: the air's colour, the sun, the clouds, what distance fades to | `render/shade/atmosphere.coax` (`atmo_sky`, `atmo_sun`, `atmo_clouds`, `atmo_far`), through `StyLook.air`: added to the engine for this page |
| The frame as light: bloom and one tone over it | The stylized look's film entries (`stylized_film_bg_fs`, `_scene_fs`, `_grass_fs`, `stylized_bloom_fs`, `stylized_post_fs`) and the host's post pass (`coax/web/host.js`) |
| The grass: blades, wind, parting round the character | `stylized.coax` with `render/shade/vegetation.coax` (`sty_grass_field`, `sty_wind`, `sty_mover`) |
| The lush grass: curved blades in tufts, their colour and light, the far field's colour, and rests under things | `StyLook.lush`, `sty_grass_vertices`, `sty_rest_add` (`stylized-frame.coax`), drawn by `stylized.coax` (`sty_lush_blade`, `sty_lush_colour`, `sty_lush_light`, `sty_lush_ground`): added to the engine for this page |
| The grass spreading from the feet, and the bare grid floor | `sty_reveal`, `sty_bare` and `metaverse_style_reveal`, added to the engine for this page (openagents `f7f2dbdb0e`) |
| The ground part and the blocks' boxes | `arch/mesh.coax`, `arch/solid.coax` (`arch_quad`, `arch_slab`), materials from `arch/palette.coax` |
| The character's look | `render/ghost.coax` (`engine_ghost_rim`, `_alpha`, `_channel`), blended by the host's ghost profile; the platformer's hero `render/livery.coax` (`livery_part`, `_refine`, `_colour`, `_grain`, `_group`, `_swell`, `engine_livery_rim`), drawn solid |
| The character's pose arithmetic | `anim/pose.coax` |
| Walls, floors, standing on and falling off blocks | `physics/query.coax` (`query_sweep`, `query_stand`) and `physics/collide.coax` (`collide_walls`) |
| The blocks and the ball as rigid bodies: falling, resting, stacking, tipping, rolling, sleeping, pushed by the character | `physics/sim/bodies.coax`, added to the engine for this page, on its `rigid.coax`, `broad.coax`, `narrow.coax` and `contact.coax` |
| A turned or tipped block and a ball as things the character meets | `physics/query.coax`'s hulls and capsules, `collide_hulls`, `collide_bodies` |
| How it walks and jumps | `metaverse/splice-bots/rules/walk.coax`, the app's own rules (`coax/her.coax`) |
| Drawing | `coax/web/host.js`: its background, shadow, scene, grass, bloom, post and stroke passes |

**Units are metres. A grid square of the blocks is one metre**, so the page's
hundredths of a square are centimetres. The page's x is to the right, z ahead
and y up; the engine's camera looks along -z, so the page's (x, y, z) is the
engine's (x, y, -z). The rules and the collision run in the page's axes; only
what is drawn is turned.

Still not the engine's: the character's model. `web/ghost.coax` reads the
figure from its own small baked format and skins it on the CPU (see "The
character"). The engine's own path for this figure is the people pack (12 MB)
drawn through an Everglade zone, too heavy for this page; moving the model
needs a small skinned-mesh pack and a GPU skinning pass in the engine.

## Game types

Before the world comes up you pick what kind of game it is. The picker's
cards, their order, names, lines and pictures, and what each type starts with
are `shared/game-types.coax`'s, the same list the app's New Game shows
(`coax/picker.coax`); `bl_new` makes a type's starter things from it, as the
app's `ed_new_game` does. The first four
are the meadow, each from its own camera; the fifth, the medieval village, is
a world of its own ("The medieval village"). Each type has the
same family of entrance (the camera comes down from the picker's pose in 0.6
seconds, grass spreads from the feet over 2 seconds, the sun rises over 4, the
blocks and the ball drop in) ending in its own camera and keys. The entrance
goes on round you: the keys, a look and the hotbar's tools are taken from the
world's first frame ("The first second").

**In the app.** The Mac and iOS apps draw and play all nine by this same
scene: `coax/world.coax` loads `web/hud.coax` into the app's core and the
engine's page host draws it (AGENTS.md, `coax/world.coax`;
`docs/openagents-integration.md`, "Drawing"). A level being built there is
this scene held still under the editor's camera (`Hud.still`, `Hud.eye`,
`Hud.glow`: hooks that do nothing on the website); Play starts the world
over with the level's things in it, without the entrance, and the app's
keys are the page's key bits. A rule changed here is changed in the app.
The worlds' own courses, rooms, circuit and green are still the world's own
there as here: what a person places is what the level file holds.

| Type | The entrance ends | Camera after | Keys after |
| --- | --- | --- | --- |
| Third person | Behind the character, 9.5 m back | The free orbit, with the follow button ("The camera") | As "The camera" says |
| First person | In the character's head, looking the way the sun comes up | First person only: click the world and the mouse is held and looks ("The held pointer"); free, a drag (or the arrows, or a finger) looks; nothing zooms out; a small reticle at the middle; no follow button | W A S D run where you look, Space jumps |
| Top-down | 12 m out, looking 63 degrees down, north up the screen | Fixed: it follows the character and never turns or tilts; the wheel zooms 6 to 12 m | W A S D and the arrows run in screen directions, Space jumps |
| Side-scroller | Beside the character and level with it, the sunrise's end of the world to the right, through a long lens (0.5 radians) | Fixed: it follows along; the wheel zooms 7 to 12 m | A, D and the left and right arrows run left and right; W, Space and the up arrow jump; S does nothing |

### The ring

**The game types are a ring of portals** (issue 30), on the home page and
wherever the builder asks for a type (`/play` with no game up, "game type").
There is no other picker. The world is the meadow at once, its grass and its
sky, and round the camera stand nine portals, one a type, in
`shared/game-types.coax`'s order (the same list the app's New Game shows; a
check holds the ring's count to it, and to the engine's portals a world).

- **A portal** is the engine's architecture kit (`arch/solid.coax`): two
  posts, a lintel, a sill and a dark back, with the type's picture in the
  opening (`web/ring.coax`, `rg_mesh`). The pictures are the page's captures
  (`web/img/kind-*.jpg`, `scripts/web-shots.sh cards`), cut to 256 by 144 by
  the glue and given to the HUD as a store of their own (`pics`, HUD message
  122), which the scene's shader samples where a portal's picture is
  (`web/gpu/hud.coax`). The engine has no picture surface of its own yet; this
  is the stand-in, and the first thing to propose upstream.
- **The names** are the page's own: `web/home.coax` `gs_ring_list`, a link
  each to `/play?type=ID` with its number, its name and its line (the
  village's carries "0 A.D. art by Wildfire Games, CC BY-SA 3.0" as its
  title). The HUD says where each portal stands on the screen (action 123:
  its state, the lit one, and a place and width a portal) and the glue stands
  each name under its portal's sill (`html.ringed`, `--rIx`, `--rIy`); a
  name whose portal is behind the camera, too low, or at the window's edge
  is hidden, so none is cut.
- **The camera** stands at the middle, a little back from the ring
  (`RG_FAR`), and drifts: it rests on a portal three and a half seconds, then
  turns to the next (`HUD_RING_DWELL`, `HUD_RING_DRIFT`). The portal ahead is
  lit (its frame glows) and its name shows its line. A drag looks round.
- **Picking.** A click or a tap on a portal or on its name, its number key
  (1 to 9), the left and right arrows and Return (on the lit one), a swipe
  sideways on a touch screen (the next portal), or walking in (the engine's
  `world/portal.coax`). The camera flies through the portal into the type's
  world (about 0.7 seconds), then that type's entrance plays; under reduced
  motion it is a cut. A pick is the type picked: counted as such (the loop's
  "create" on the home page, then "builder" and "picked").
- **The heading** is one line, "make a game. pick a portal." (`GS_HEAD`,
  `GS_HEAD_HERE`), with the line under it.
- **Your games.** Logged in, a button by your icon, "your games", opens the
  account's games over the ring ("Your games"); a game picked there opens as
  its type.
- **Before the code, and with no WebGPU.** The first paint is the heading and
  the nine names as plain HTML (`web/index.html`, written from `home.coax`),
  each a link, so a press before the world is up still takes you to that type
  ("The first second"). With no WebGPU the names stay a list, with the words
  that say why ("No WebGPU").
- **Phones and tablets.** The camera's distance is held so a phone's narrow
  window shows the lit portal whole and a wide one its neighbours too
  (`ring.coax`'s check). Names are never clipped at the edge.

The type picked last is lit: the page remembers it while it is open, and the
glue keeps it in this browser's `localStorage` (`gb-kind`) as a convenience
only. "game type" in the account menu (the icon at the top right) goes back to
the ring; picking again replays that type's entrance, the grass and the
sunrise from the start. Logging out forgets the choice.

The protocol: the glue raises the ring with HUD message 120 ("1"; "0 KIND"
leaves it), sends keys as 121 (1 left, 2 right, 3 enter the lit portal) and
the pictures as 122 ("P ROW ROWS" then hex RGB); the HUD answers with actions
123 (where the portals are) and 124 (a portal picked, which the glue presses
as that name).

**How the HUD is told.** The page's class says it: `gs v2 pick ring` while
picking (the glue raises the ring and keeps the keys for it),
`gs v2 g0` to `g8` once picked (the glue sends the type as message 111, then
the world's view). The HUD (`web/hud.coax`) starts the entrance over each
time the world's view comes up.

**The cameras are the engine's.** All four are the one free orbit held to a
kind's rules by `metaverse-engine/cam/kinds.coax` (added to the engine for
this, openagents `119398bcb7`): `cam_kind_start` is where a type's orbit
begins, which is where its entrance ends; `cam_kind_in` drops the input a
fixed type does not take; `cam_kind_keep` puts a fixed type back on its look
and its range after each step; `cam_kind_wheel` and `cam_kind_pinch` zoom
within the type's range; `cam_kind_line` is the line a side-on character is
held on. The reticle is the engine's (`cam/reticle.coax`). What is written
here is which key means what for each type (`hud_walk`) and nothing of a
camera.

**The held pointer (first person).** Click the world and the browser holds the
pointer (its Pointer Lock): the mouse then looks with no button down, 0.0024
radians a CSS pixel (the engine's `ML_TURN`), to the orbit's first-person
limits; W A S D run, Space jumps, the reticle is the aim. It is let go by
Escape (the browser's own), by Enter (the caret goes to the chat box), by
leaving for the picker or logging out, and whenever the window loses the keys;
click the world to hold it again. It is never held until the world is clicked:
the world opens with the cursor free, for the chat and the menu, and while it
is free a drag looks as before. A browser that refuses the lock leaves the
drag. A finger never holds it. Third person keeps its drag (the engine's
mouselook reframes the body over the shoulder, which this page's orbit does
not do yet).

The rules are the engine's (`metaverse-engine/cam/mouselook.coax`: when it is
wanted, what a press, Escape and a refusal mean, the turn for a pixel), held
in `web/hud.coax` (`hud_mouse`, `hud_look_delta`, `hud_pointer_lock`, with a
check). The page host (`coax/web/host.js`) asks the browser inside the click
and forwards the mouse's movement; the glue only says when the caret is in a
field (message 112). Seen in a browser: the browser pane used here refuses
Pointer Lock (the page then keeps the drag, as it should), so the held path
was driven through the host's development stand-in (`coaxial.pointerLock(1)`)
with made-up mouse moves: the view turned, and stopped turning once the caret
was in the chat box. A real held pointer in Chrome or Safari has not been
watched. The mouse's movement is the browser's own (with the system's
acceleration); raw movement (`unadjustedMovement`) needs a line in the host
and is not asked for yet.

**Side-scroller, what is held.** The character alone is held on its line (the
line through where it stood when the world came up, running the way W used to
run): whatever pushes it, it is put back on the line each frame. The blocks
and the ball are not constrained: they are still bodies in three dimensions,
so a kicked ball can roll out of the character's plane.

**Things to know.** Top-down looks at the ground, so the sunrise is seen only
as the light changing. In first person the wall that stands ahead of the
start hides the sun itself until you walk round it. Leaving the world (to the
picker, or logging out) the camera goes back up three times as fast as it
came down, so the next entrance starts from the picker's pose. The model is
told the type with every message (`game type: first person`, the first line
of the world description).

### Every game type is built in

GameBuilder is for building games, so every game type can be built in: a
game type adds its own pieces, rules and actions, and it never takes building
away. Crates, the platformer, racing and drift each have the build tools on their
hotbar (shapes, then their own kinds, edit, decide, world settings), then
their own reset, then more; the chat's plan lines (`+ place`, `+ ball`, `+ size`, `+ move`,
`+ remove`, `+ clear`, `+ set`) are made there as in the meadow, against the
same list (`web/blocks.coax`) and the same registry. No camera menu in these
four: each one's camera is its own. Each type keeps its own level in this
browser ("Kept in this browser"): picking one opens what was built in it, or
a new game of it when nothing was.

What the model is told says how building goes in each (`web/chat.coax`
`GT_BUILT` in `shared/game-types.coax`, the line after `game type:`):

| Type | Building there |
| --- | --- |
| Crates | What you place stands in the room beside its crates, solid, and pushable as anywhere; the crates are the room's, drawn by the room |
| Platformer | The course runs along z and the character stays on the line x 0, so things go at x 0. There is no ground under the course, so whatever is built there is held where it is put (the HUD makes it a fixed body) and the model is told to give a height and `rest`. The page is told how high the feet are (`person: at x 0 z 12.5, feet at height 8.45, ...`), so "add a platform above my head" is a slab over the head at the right height, side on. Places reach 700 m from the middle and bottoms 60 m up, not the meadow's 200 and 12, because the course is 628 m long and rises to 49 m (`ch_bounds`, `bl_bounds`). The shapes tool, side on, places on the character's line where the pointer is, on a platform's top when the pointer is on one, else in the air there |
| Drift | The person's place is the ship's, in open space: no ground and no gravity, so whatever is built is held where it is put (a fixed body, as on the course) and the model is told to give a height and `rest`. Bottoms go from 100 m under where the floor was to 100 m over it (`bl_low`: the only type whose bottoms go under 0) and places reach 700 m, because the rings run 600 m ahead, 420 m aside and from 80 m under the start to 30 m over it. The page is told how high the ship is, as the platformer's feet. The ship meets what is built: the HUD gives the world the nearest 64 boxes each frame (`dr_built`) and the ship, a ball of 1.7 m, bumps off them (`contact_sphere_box`, `contact_resolve_static`). The shapes tool places 10 m out along the pointer's ray (`HUD_T_SPACE`), or against a thing met nearer |
| Racing | The person's place is the car's. What you place stands solid on the grass or the track, and the car meets it: the HUD tells the race each frame's built boxes (`rc_built`) and the nearest four give the car contact planes beside the road's (`rc_box_planes`): a top it is over, or one lower than 0.3 m under the car's middle, is a floor (a slab is driven over); else the face on the car's side is a wall it stops at |

Limits are said in words with their numbers as everywhere ("too far: things
stay within 700 m of the middle.").

**Each type's own kinds.** A block may be of a kind its game type has
(`shared/kinds.coax`; the app has the same kinds in its palette, `GK_PAD` to `GK_CONE`, saved under the same words but crates' goal, `crate-goal` in a level's file): the place line's last word (`+ place X Z W D H [BOTTOM]
[rest] [KIND]`, rest and the kind in either order) or `+ kind ID KIND` after
(`block` makes it plain again). The model is told each type's kinds, their
sizes and what they do (`GT_BUILT`), and each thing's kind in the world's
list (`b3: pad 0 15, ...`). A kind another type has is refused, said in words
("that kind is not built here: this game type's are pad, spikes, shard,
checkpoint, mover, lift, and plain blocks and balls."). The HUD is told a block of kind r
as KIND 10 + r on the wire, and runs it:

| Type | Kind | What it is |
| --- | --- | --- |
| Platformer | pad | Fixed, drawn pink: standing on it throws the character up, as the course's own pads do |
| Platformer | spikes | Not solid, drawn red, laid on a platform: feet in them put the character back (a fall counted) |
| Platformer | shard | No body, a gold gem: touched, it is taken (counted); the course started over brings it back |
| Platformer | checkpoint | A platform that is a gate of the run: stood on in its turn, a fall comes back to it; the furthest along the course is the goal |
| Platformer | mover | A moving platform: it slides along the course and back, as far and as often as its distance and period say (`+ goes ID DISTANCE SECONDS`), carrying who stands on it |
| Platformer | lift | The same, rising and coming down |
| Crates | wall | Fixed, dark: nothing pushes it |
| Crates | goal | No body, an amber pad, green when a crate rests on it: the gate opens only when a crate rests on every goal. Any plain block built there is a crate, and lights a goal it rests on. With no goal the room is never solved |
| Crates | gate | No body of its own: the room's way out, a bar the room keeps shut (solid) until it is solved, then sinks. Out through it is the game's next level. A room has one: the app keeps one a level (adding another selects the one there), and on the website the first placed is the one that counts. With none, solving the room goes on by itself |
| Crates | start | No body: where you start in the room and come back to on reset. A room has one, as the gate. With none you start in the middle |
| Racing | boost | No body, a cyan slab: over it the car speeds up, up to 1.5 times its top speed (`rc_boosts`, `rc_boosted`) |
| Racing | cone | A light body, orange, never among what the car meets (`rc_built` leaves it out): the car within 1.6 m knocks it away |

The hotbar has an item for each of its type's kinds after the shapes slot
(the platformer: pad, spikes, shard, checkpoint, and mover and lift under
more; crates: wall, goal, gate, start; racing: boost, cone). Picked, it is the shapes tool
in the kind's own shape (a pad, spikes, a checkpoint, a goal, a boost, a
moving platform and a lift are slabs, a wall a pillar, a shard and a cone
cubes), lit on its own slot, and what it places is of that kind
(`bl_pick_kind`; the HUD's place line is the shapes tool's as anywhere, and
the page gives the new block the kind). Picking a plain shape forgets the
kind.

**A world's course is things of the level** (issue 2, step 5). A new game of
a world of its own starts with its course as things (`shared/game-types.coax`
`gt_starter`, one source for the website and the app), which are selected,
moved, sized and deleted as any other thing, kept in the level's file, and
read by the world from the level, not from its own code. Racing's is done:
its circuit is nineteen track points (kind `track`), and the road is laid
through the level's track points in their order. Drift's is done: its course
is twelve rings (kind `ring`, the thing a ring's middle), flown in the list's
order. The platformer's is done: its course is 111 things (platforms, pads,
spikes, shards, checkpoints, moving platforms and lifts; "Platformer").
`tools/game-types-check.coax`
fails when a world's file holds a course of its own beside the shared one.

**Not yet:** ramps as a kind; racing's checkpoints placed by
hand (they are eight, evenly round the lap); the
crates' rooms are the world's own still, not things of the level. (The
village green's buildings are things of the level: "The medieval village".)
On the website a moving platform's distance and
period are set by a plan line (the chat's model writes it); edit mode has no
buttons for them yet.

## The hotbar

A row of slots at the foot of the window, in the middle, in every game type
(not on the picker, and not outside the world). It is the shared hotbar every
game and realm is meant to use: `components/kit/hotbar.coax` in OpenAgents
(added for this, openagents `aa14807d30`), with its science fiction skin.
The page fills it and places it; nothing of its look or its rules is here.

| Slot | Key | What |
| --- | --- | --- |
| shapes | 1 | The one placing tool for basic geometry ("Shapes", "Placing modes"). Its glyph is the shape it places now: ▢ a cube, a shaded ball (drawn by the hotbar's skin, unlike the camera's ring), ▭ a slab, ▯ a pillar |
| ✎ edit mode | 2 | A mode, lit while on ("Edit mode") |
| ✦ decide | 3 | The model picks the next edit and makes it (it was beside the chat box). Busy while the question is out |
| ◎ camera | 4 | The camera menu: pressed, a row stands over it as the shapes' row does: orbit, follow (locked behind you), first person, top-down, the one in use lit (Q and E step through them while it is open). Its glyph is the camera in use. Every meadow type and the village; each starts on its own camera. Side-scroller and crates keep their own and have no camera slot. The page tells the HUD the camera as a number (0 orbit, 1 follow, 2 first person, 3 top-down) and the HUD switches the engine's camera kind |
| ⚙ world | 5 | Opens the world's settings over the hotbar, lit while open ("World settings") |
| ↶ undo | 6 | Takes the last edit back ("Undo and redo"; Ctrl+Z, ⌘Z) |
| ↷ redo | 7 | Makes it again (Ctrl+Shift+Z, ⇧⌘Z, Ctrl+Y) |
| ⋯ more | 0 | Always the last slot, never moved or taken off: opens the palette of everything this world has ("More") |

That is the meadow's own bar. The bar is an ordered few (at most nine, keys 1
to 9 by place) of a catalogue of every item the world has; "more" follows
them. Crates, the platformer, racing and drift have their own kinds after the
shapes and ↺ reset (R) after world; undo and redo end every bar that has room
for them (not the platformer's or the village's, which are full: there they
are in more). The
medieval village has its buildings first: ⌂ britons house (1), ⌂ gauls house
(2), ⛫ market (3), ◆ rock (4), then edit, decide, camera, world and more; the
shapes are in more ("Placing buildings").

### More

"More" opens a panel over the bar (the shared `k_hb_palette`): every item, in
groups (build, edit, camera, world, agent), each with its glyph, its name and
its one line, and the key it has on the bar when it is there. The meadow's:

| Group | Items |
| --- | --- |
| build | shapes (the slot with its row), and each shape alone: cube, sphere, slab, pillar |
| edit | edit mode |
| camera | camera (from the palette it steps to the next camera) |
| world | world settings, ⇡ fly (starts or stops flying, as Space twice does; not side on, and off while the world's player.fly is), ⟲ reset world (the things back as the world began), game type (the picker), ⌫ reset hotbar |
| agent | decide |

A click uses an item once without putting it on the bar, and the panel shuts.
With it open the arrows move its focus, Enter uses the focused item and a
number key puts the focused item at that place on the bar (one there already
moves there). 0 or a press of more shuts it, and Escape shuts it with
everything else. A world of its own has everything the meadow's has but the
camera menu and fly, and its own reset.

### Rearranging

Hold Shift over the bar or the palette: what can be lifted shows a dashed
outline (the cue is only there while Shift is held over them, and Shift
anywhere else is still the descent while flying). Drag a slot: it lifts, the
others slide to make room, and where it is let go is where it stands. Let go
away from the bar, it is taken off (it is still in more). Drag an item out of
the palette onto the bar to put it there; on a full bar it takes the place of
the slot it is dropped on. A slot dragged onto the palette is taken off. On a
touch screen a long press (0.45 s) lifts it as Shift would. Keys follow
places: the slot at the first place is 1. A Shift-drag never moves the
character or the camera: Shift over the bar or the palette, or during a drag,
is not told to the HUD.

The glue shows the drag and tells the page the result in a hidden field
(`move FROM TO`, `off FROM`, `add ITEM TO`); the bar is the page's
(`gs_drag`, through the shared `hb_lay_move` / `hb_lay_remove` / `hb_lay_add`).
Each game type's bar is kept in this browser (`localStorage` `gb-bar-KIND`,
the item ids in order): the page writes it in a hidden element (`KIND ids`,
or `KIND default` while it is the type's own), and the first time a type is
shown its kept bar goes back in, checked against the catalogue (an id it does
not know is dropped; nothing it knows is no bar, and the type's own stands).
"Reset hotbar" in more puts the type's own back and forgets the kept one.
Storage that cannot be read or written leaves the type's own bar.

Every slot is named under its glyph in small mono type (shapes, edit,
decide, camera, world), keeps its key number, and says what it does in one
line when the pointer rests on it ("place a cube: click the world"). Names
are the shared hotbar's: a skin shows them always (science fiction), only
under the pointer, or never. Its skins live in the components' skin layer
(`components/skin/hotbar.coax`, openagents `c2da61b7c8`).

### Shapes

Cube and sphere were two slots; they are one (the owner's call, 2026-11-17:
"a single hotbar key for basic geometry"). Picked, the shapes slot shows a row
directly over it in two groups with a line between (the shared hotbar's
option groups, openagents `dd0b36536d`):

| Group | Options | Keys |
| --- | --- | --- |
| The shape | cube (1 m), sphere (0.6 m radius), slab (2 m square, 0.25 m high), pillar (1 m square, 3 m high) | Q and E, back and on, round and round |
| How it is placed | snap, free ("Placing modes") | Z and X |

Each group's active one is lit and is kept while you are logged in (a cube,
free, as the world opens). Pressing an option does not put the tool
down, and the slot's own glyph is the shape chosen. Every shape goes through
one placing path (`hud_aim`, by the shape's half footprint and height from
`hud_shape`): the ghost, the grid's cell (as wide as the shape: 1 m, 1.2 m for
the sphere, 2 m for the slab), stacking on tops, standing beside sides, the
reach.

A shape here is a size, not a new kind of body: the cube, slab and pillar
are boxes (`place X Z W D H`) and the sphere is the ball, so the page's list,
the physics and what the model is told did not change, and the model is told
the four shapes' sizes. **Left out: a ramp or wedge and a cylinder.** The
engine's rigid bodies (`physics/sim/bodies.coax`) have two colliders, the box
and the ball; a wedge or a cylinder needs its own narrow phase against boxes,
balls and the ground there first, with checks, and nothing here fakes one.
Balls have no cap of their own: blocks and balls share the one cap of a
thousand. The things' one mesh is sized for a thousand balls: the twelve
nearest the character are drawn smooth (588 vertices), the rest low (8 by 5,
192 vertices); a box is 36. With the world full the ghost shows its cross.

### Placing modes

The shapes tool's second group: **snap** and **free**, the active one lit.
Pressing one does not put the tool down. **Z and X** step between them. The
mode is kept while you are logged in (free, as the world opens).

**Nothing you place drops in from above** (the owner's call, 2026-11-17: "only
the first ones that spawn in, not those placed by the user"). There was a
third mode, drop; it is gone. A thing the tool places is set down exactly
where its ghost is, at rest, asleep; the ghost always stands where the thing
will be, never up in the air.

| Mode | The ghost | A press |
| --- | --- | --- |
| snap | The grid's cell lit under it (a bright square) and the thing's wire flush in it | Sets it down in that cell, at rest: asleep at exactly that place. A cell is as wide as the thing (1 m for a block, 1.2 m for the ball), and cells are the floor's own metre squares |
| free | The thing's wire alone, exactly where the pointer's ray meets | Sets it down exactly there, at rest |

Where the ray meets a thing's **top** the new thing goes on that top. On a
thing's **side** it stands beside it, at that thing's foot (snapped, in the
next cell that is clear of it; free, touching the face). To put a thing on
another, point at its top.

**What still falls in.** Only what a world opens with: the meadow's three
blocks and its ball, the green's ball (`bl_start`, `bl_green`: their `starts`
mark; the green's buildings rise instead, "The entrance"). They appear 3 m above their places as the camera comes down and fall
(a world that comes up settled has them at rest: "The first second")
("Things that fall and can be pushed"). Everything made afterwards appears at
its place: by the tool (at rest), by edit mode's twin and move, by the chat's
model and by decide. A thing the model gives a height that leaves it in the
air starts at that height and falls from there: that is gravity, not a
drop-in.

Set down at rest, a thing may not stand in another: there the ghost is faint
with a cross, and a press places nothing. The same when the world is full.

**Reach.** A thing may stand up to 200 squares from the middle (it was 40;
`BL_FAR`): the engine's bodies keep their places in 64-bit floats and sleep
and wake the same 200 m out (checked), the broad phase is a sweep with no
grid to outgrow, the grass keeps off the twelve things nearest the
character whatever the rest are doing, and 200 m is where the haze has hidden
the ground. The tools' ray reaches all of it (700 m). Far off:

- a pixel of the pointer covers metres of ground, so a place that is not on
  the grid keeps to a grain no finer than two pixels there (1 cm near by, then
  5, 10, 25, 50 cm, 1, 2, 5 m: `hud_grain`), and the ghost stands still;
  snapped, the cell is exact at any distance;
- a ghost lower than 28 pixels on the screen gets a marker standing over it;
- a ray that meets nothing (over the horizon) or the ground past the bounds
  is brought back to the furthest place a thing may stand that way, and a bar
  with its ends turned down is drawn across the ghost's foot to say so.

In first person the reticle's ray has the same reach, and with a placing
tool picked the first click on the world holds the pointer and the ones after
it place (a browser that refuses the lock leaves the click the tool's).

**At rest, on the wire.** A plan line's last word may be `rest`
(`+ place X Z W D H [Y] rest`, `+ ball X Z R [Y] rest`): the chat's model can
set things down exactly too, and is told so. The page's list keeps it, the
list message says how each thing is made with one more number on its line
(none: one the world opened with, which falls in; 1: at rest; 2: at its
place, awake), and the HUD makes a thing at rest at its place and puts it to
sleep (the engine's `bd_still`): it stays to the bit beside and on top of others (checked over
ten seconds of steps) until a push, a landing thing or a change of gravity
wakes it.

One placing tool is picked at a time; the picked tool pressed again is put
down. A press on a slot does not take the keys from the character and never
puts the caret in the chat box. The chat box is now the text field alone
(Enter still opens it).

**Escape clears everything.** One press puts the placing tool down (its row
of options goes with it), leaves edit mode with nothing selected, lets go of
a thing that was held, shuts the world's settings, and takes the keys' focus
off the gravity slider if it had it (`gs_clear` in `web/page.coax`; the glue
blurs the slider). What is a setting stays: the camera picked, the gravity, a
slot's option. In first person the browser lets the held pointer go on the
same press. With the caret in the chat box Escape only gives the keys back
to the character; the next press clears. The slider never keeps the keys: a
drag on it ends with the focus back on the world, and with it focused (by
Tab) the number keys and Escape still work.

**A place a thing cannot go.** The page refuses a thing over its count (1,000
things, blocks and balls alike) or past the world's bounds (200 squares from the
middle). The HUD knows the same limits (`hud_room`): with the world full the
ghost is faint with a cross, and a press places nothing; the bounds it never
passes, since the ghost is brought back to them ("Placing modes").

**Fixed 2026-11-17: the placing tool that stopped placing.** A thing pressed
a centimetre into the ground (five times the gravity, or a neighbour landing
against it) was told to the page with its bottom at -0.01; the page kept it;
the HUD's list reader took a bottom under the floor for a broken line and
stopped reading there, so that thing, every thing after it in the list and
every thing placed afterwards were gone from the world while the page still
counted them. The tool was lit, its ghost followed the pointer, and a press
did nothing to see. Now a rest is never told under the floor
(`hud_rest`), the page never keeps one (`bl_things`), and the list reader
puts such a thing on the floor and reads on (`hud_message`, 105).

**Whose drag it is.** With a placing tool picked, the mouse's first button
and a finger are the tool's; the mouse's other button still turns the camera.
In edit mode a press on a thing is edit mode's; a press on empty world is the
camera's (and lets the selection go). With neither, everything is the
camera's as before. The wheel zooms, except in edit mode with the pointer on
the selected thing, where it sizes it. In first person the tools' ray is the
reticle's (the middle of the window), wherever the pointer is.

**In a narrow window** (under 1680 CSS pixels: a bar of ten slots reaches the
chat's corner under that) the chat stands over the
hotbar instead of beside it, and under 640 the slots are 40 pixels instead of
52 and the row may wrap (a bar of more than seven slots is two rows on a
phone, and the chat stands over both; words over the hotbar stand between
the two; the login a guest opens over the world takes the chat's place while
it shows). A panel that is open (the settings, the selection)
covers the chat box in a phone-wide window until it is shut.

**How an edit is made.** The HUD does the picking: a ray from the pointer, by
the engine's ray query (`physics/query.coax` `query_ray`) over the things
(a box that has turned is met by the box round it) and the ground. It tells
the page what was done in the block list's own words (its action 114, into a
hidden field): `place X Z 1 1 1 Y rest`, `ball X Z 0.6 Y rest`, `move bN X Z`,
`size bN W D H`, `select bN`, `select none`. The page reads them with the
same code that reads the model's plan lines (`bl_edit`, `bl_plan` in
`web/blocks.coax`), so ids, limits, the 1,000-thing cap, the bodies and what the
model is told are one truth. The page tells the HUD the hotbar's state in a
hidden element the glue forwards (message 113: the shape being placed or -1, edit mode, the
selection, gravity, the camera: 0 orbit, 1 follow, 2 first person, 3 top-down, the placing mode: 0 snap, 1 free, the presses of a world's own action, and the presses of fly: one more than last told flies or lands). The glue passes the keys on as they
are (a digit, `esc`, `del`, `q`, `e`, `z`, `x`, `r`; and while the palette is open `up`, `down`, `left`, `right`, `enter`).

### Limits

**A limit you can hit is said on screen, in words, with its number** (the
owner's rule, 2026-11-17, in `AGENTS.md`). When a placement or an edit is
refused for a limit or a bound, a line stands over the hotbar for five
seconds, in the site's voice, and goes by itself (`gs_limit` in
`web/page.coax`; the words are `bl_why` in `web/blocks.coax`). The ghost keeps
its cross; the words are what says why.

| Limit | Its words |
| --- | --- |
| The world holds 1,000 things, blocks and balls alike | "world is full: 1000 of 1000 things. delete some to place more." |
| A thing stands within 200 m of the middle | "too far: things stay within 200 m of the middle." (a ghost brought back to the bound still places, and the bound is said) |
| A side is 0.25 to 12 m | "that size is out of range: sides are 0.25 to 12 m." (the selection's − and +, the wheel on it, a plan line) |
| A ball's radius is 0.15 to 1.5 m | "that size is out of range: a ball's radius is 0.15 to 1.5 m." |
| A thing's bottom is at most 12 m up | "too high: a thing's bottom is at most 12 m up." |
| Something stands where the thing would go | "no room there: something is standing in that place." |
| The sky's limit, 60 m | "as high as it goes: the sky stops at 60 m." (once each time you get there) |
| A chat message is 1,200 characters | "that message is too long: it is cut at 1200 characters." (under the chat) |
| The agent is sent the last 12 messages | "the agent sees the last 12 messages: older ones are let go." (the first time the oldest goes) |
| A guest has no messages to the agent | "log in or create an account to talk to the agent." (where the chat's box would be, with the log in button; over the hotbar when decide is pressed: "A guest") |
| Undo goes back 100 steps of this visit | "nothing to undo: undo goes back 100 steps, from when this page opened." and "nothing to redo." ("Undo and redo") |
| The browser's storage is full, or off, or the game is over 1,024 kb | "this browser's storage is full: your game (12 kb) is not kept. free some space and it will be." and the others in "Kept in this browser" |
| The account holds 512 things and 256 kb a level | "kept in this browser only: a game on your account holds 512 things at most, and this one has 513. delete some and it is kept there too." and the others in "Kept on your account" |

Who says it: the page knows its own list's limits (`Blocks.why`, set by the
edit that was refused), and the HUD tells the page the ones it meets where a
tool is used or with the character (`limit full`, `limit room`,
`limit far`, `limit size`, `limit sky`, through the same hidden field a tool's
edit goes through; the page turns the word into the sentence, with the
count). **The agent's own edits**: a plan line a limit refuses puts the same
words under its answer, and the model is told with your next message (`your
last edit was refused: ...`), once.

**A live count.** The shapes slot's tooltip ends "things: 12 of 1000", and the
world's settings have a row with the same count, bright from nine tenths
full. The thing that takes the world to nine tenths says "nearly full:
things: 900 of 1000." once.

**The cap, a thousand (2026-11-17, was 48 with 8 balls).** The engine's rigid
body store holds 1,024 (`RIGID_BODIES`; the broad phase 1,024, its pairs
16,384 a step, the contacts 2,048 a step). Measured natively (Apple silicon,
release, `metaverse-engine/tools/bodies-bench.coax`): a physics step of 1,000
resting things 0.32 ms, with 50 awake 0.36 ms, a pile of 200 awake 2.2 ms; at
most four steps a frame, so a heavy pile slows the world rather than freezing
the page. The HUD's frame with 1,000 resting things is about 0.45 ms natively;
rebuilding the things' mesh when one moves is 3.7 ms for 1,000 boxes and
6.2 ms for 1,000 balls (the whole mesh is made again: instancing the things is
the next step). In the browser (headless Chrome with WebGPU on this Mac, the release
build, 1440 by 900 and 2880 by 1800): with 1,000 things at rest the page's
frame costs the main thread 0.6 to 0.7 ms on average (9 ms at worst) and keeps
the display's rate; with 800 at rest and a pile of 200 falling, 3.0 ms on
average and 12 ms at worst. The 1,001st thing is refused and said. The page's list to the HUD (message 105) is still the whole
list, about 13 to 48 KB at the cap, sent when it changes (the HUD's inbox is
64 KB); the HUD's report of rests goes a few hundred at a time. The model is
told the count and only the 24 things nearest the person, the selected one and
any its message names (`bl_describe_for`); an edit by id reaches any.

### Edit mode

Slot 3. In it, the thing under the pointer is outlined faintly; a press
selects it (its box drawn bright, lines over the world); a press on empty
ground lets it go. Out of edit mode nothing can be selected. With a thing
selected a small panel opens over the hotbar with its id and size and:

| Control | Does |
| --- | --- |
| Drag the thing | Moves it: its box is drawn where it would go, snapped to the half metre, and it is put there when let go (the page moves it there; it is not lifted first) |
| Wheel, pointer on it | Sizes it a notch (10 percent), every side alike |
| − and + beside w, d, h | One side a quarter metre smaller or bigger (a ball: its radius) |
| ⧉ | A twin beside it (to its right, or its left at the world's edge), which is then the selected one |
| ✕, Delete, Backspace | Deletes it |

All within the block list's limits (sides 0.25 to 12, a ball's radius 0.15
to 1.5). The character still runs and jumps in edit mode. A thing placed in
edit mode is selected. Leaving edit mode lets the selection go. The model is
told `mode: edit` and `selected: bN` with every message, so "make this one
taller" has a meaning.

Not built: handles on the selection's box (per-axis sizing is the panel's
buttons), turning a thing, and holding a thing in the air while it is
dragged (it stays where it is until it is let go, then is put at the new
place). The app's editor keys (Q W E R tools) are not used here: edit mode is
key 3.

### Fly mode

In third person, first person, top-down and the village: **Space pressed
twice within 0.3 seconds** turns fly mode on, and twice again turns it off.
One press is a jump, as ever. These game types have no air jump any more
(the side-scroller keeps its jump and its air jump, and does not fly).

| Flying | |
| --- | --- |
| Gravity | None on the character, at any gravity setting: it hangs where it is |
| W A S D | Fly level in the direction the keys mean in that camera (ahead is away from the camera; in first person, where you look, level), at 12 m/s (1.74 times the run), from the first frame |
| Space, held | Up, 8 m/s |
| Shift, held | Down, 8 m/s. Shift is only this, and only while flying: there is no walk key |
| Let go | It stops at once |
| Space twice | Off: it falls under the world's gravity |
| The ground, or a thing's top, met while sinking | Off: it stands there |

It still meets the world: a block stops it, it comes down on a thing's top,
it rises no further under a block or past the sky's limit (60 m). It only
nudges things (an eighth of its shove on foot, and a ball is struck with a
fifth of its speed). It plays its air clip, the camera follows as it follows
a jump, and its feet stop parting the grass once it is 0.3 m clear of the
ground. The chat's model is told `moving: flying` with the world.

The rule is the engine's creative flight
(`metaverse-engine/move/creative-fly.coax`, added for this, openagents
`71ca1e66c2`): the double press, the velocity asked, the landing. `hud_walk`
sweeps that velocity against the things as it sweeps the walk. The glue
tells the HUD the keys the moment they change, and the HUD counts each press
of Space as it is told, so a tap shorter than a frame is still a press (for
the jump too).

### World settings

Slot 6 opens a panel over the hotbar: the world's settings, drawn from the
property registry ("Properties", below), and how full the world is. What
differs from the start comes first, each row with its reset, and a "reset
all"; then every other row of the world, the player, the camera and the
grass, under its group's name. A row's control is the one its kind takes: a
slider (the kit's `k_slider`) for a number, a press for on or off, a row of
presses for a choice. Under them, when there are any, is what was asked for
that nothing does yet. The selected thing's panel (edit mode) shows its own
rows the same way: bounce, friction, density, fixed.

**Gravity** (`world.gravity`) is a slider from 0 to 5 times the normal in
steps of 0.1, starting at 1. It is live as it is dragged:

- for the things it is the engine's scene gravity (`bd_gravity` in
  `physics/sim/bodies.coax`): a change wakes every body. It is part of the
  scene's state there (the digest, the snapshot, the history), so a run with
  a change in it repeats bit for bit;
- for the character it is the walk rules' own gravity (`SrWalkTuning`),
  rising and falling, multiplied: a jump leaves the ground as fast as ever,
  so at 0.5 it is twice as high and slow, and at 5 short and heavy.

**At 0 nothing falls.** A thing at rest stays exactly where it is and sleeps
again; a thing in the air (one just dropped in, one knocked up) hangs there,
or keeps drifting the way it was going, since nothing slows it but what it
hits; a push still moves and wakes things. The character runs on the ground
as ever. Its jump leaves the ground and keeps rising at the speed it left
with, up to **the sky's limit, 60 m** over the ground (`HUD_SKY`), where it
stops and stays: nothing brings it down but gravity back, or fly mode's
Shift ("Fly mode"). The limit holds at every gravity. Nothing divides by
gravity: the engine's bodies add it to a speed each step, and the walk rules
subtract it (their two jump metrics, `sr_metric_jump` and `sr_metric_double`,
do divide by it, and are used only in checks at a gravity that is not 0).

The chat can set it too: `+ set world.gravity N` (and the old `+ gravity N`,
the same door), and the model is told `settings changed: world.gravity 0.3`
whenever it is not 1. The slider follows. A setting that differs is kept with
the level it belongs to ("Kept in this browser"), across a reload and a
logout; "reset world" does not touch it, and its own row's reset does.

## The medieval village

The fifth game type is its own world: a village green. You stand at the near
end of a worn path that runs up the green to a market; a house stands each
side, another further back on the left, and rocks lie about. It is third
person (the orbit, the follow button, first person on a zoom in), on the
meadow's grass, under the engine's sky, in a low morning: the sunrise ends
with the sun 17 degrees up, to the right of the way you look.

**The buildings are 0 A.D.'s.** Two houses (Britons', Gauls'), the Gauls'
market and two highland rocks, from the selection of 0 A.D. art the owner
pulled into the engine earlier (the asset registry's `0ad-selected` package;
Everglade's experimental glade uses the same). **0 A.D. art by Wildfire
Games, licensed CC BY-SA 3.0**: not CC0. What that asks, and where it is met:

| The licence asks | Where |
| --- | --- |
| Credit, with links to the makers and the licence | Where the work is chosen: the village's card on the picker carries "0 A.D. art by Wildfire Games, CC BY-SA 3.0" as its tooltip (under the pointer, or with the keys' focus). And on the site's credits page, `/credits` ("The credits page"), linked from the account menu and the home page's footer. Also in what the model is told and in `web/assets/village-credits.txt`. **Nothing about licences shows while playing** (the owner's call, 2026-11-17) |
| That changes are said | The credits page, and `village-credits.txt` (served at `/assets/village-credits.txt`, linked from it as "the note"): exported to glTF, recentred, written as an engine pack, textures resized to 512 and re-encoded as JPEG, normal maps left out, scaled and turned on the page. No triangle is removed |
| That the changed art is shared alike | Linked from the credits page: the pack (`/assets/village.pack`), the tool that built it (`/assets/village-pack.coax`, a copy of `tools/village-pack.coax`) and the note are public files of the site, under the same licence. The site's code and other art are separate works and are not under it |
| Never mixed into a private pack | It is its own pack, and nothing else is in it |

The engine's own handling is reused: the pack's catalog carries the licence
line the engine's packs carry for this art (`L 0 0 A.D. art by Wildfire
Games|CC BY-SA 3.0|https://www.wildfiregames.com/`), the engine's pack reader
refuses a pack with an asset no declared licence covers, and the page refuses
a catalog without that line (`vl_catalog`).

**The pack.** `web/assets/village.pack`: 1,842,224 bytes,
a VPK1 pack of the engine's: a catalog, 14,904 vertices (48 bytes each),
11,030 triangles' indices, and 8 texture layers (JPEG, 512 square). It is
committed (the repository already carries built art: the character's baked
model, the Scrappers' models); the originals (2.4 to 2.7 MB each) are not, and
stay in the asset store. `tools/village-pack.coax` builds it from the asset
store's converted copies and writes `web/village-pin.coax`, the pack's size
and SHA-256, beside it (the command is in the tool's header).

**It is fetched only when the village is picked.** The page names the pack
(`data-assets` in `web/index.html`) and holds it back
(`coaxial_pack_wait_v1`, added to the engine's page host for this): nothing
of it is fetched on the home page, at the login, in the picker or in the
meadow. Picking the village lets it go; the green (ground, grass, sky, the
character, a ball) is there at once, a thin line at the foot of the window
shows how much of the pack is in, and the buildings rise when it is. The host
hashes what it fetched and the page admits it only if its size and SHA-256
are the pinned ones (the engine's pack gate): a pack that is not exactly the
one the page was built with is not drawn.

**How they are drawn.** By the engine's painted surfaces
(`metaverse-engine/render/shade/stylized-pack.coax`, added to the engine for
this): a pack vertex is stood by a record as a kit part is, and its texture's
colour is lit by the stylized look's own sun, shadow map, sky light, haze and
film, in the same frame as the ground and the blocks. Everglade's glade draws
the same models with its painterly renderer, which this page does not have;
the stylized look had no textured path, so one was added there. Nothing of it
is written here: `web/gpu/hud.coax` names the engine's entries.

**A building is a closed solid, to everything.** Its solids are boxes found
from the model's own surface by the pack tool (no box is placed by hand, so
any model gets them): the surface is sampled into columns half a metre
square, and a column is *standing* when it holds surface between 0.25 and
1.2 m up (a wall, a post, a stair, a rock: solid from the ground to the
highest surface in it), *inside* when it is not standing but cannot be
reached from the model's edge along the ground without crossing a standing
column (a room's floor: solid from the ground to the roof over it, so nothing
gets into a building by a door, a wall or its roof), and *hanging* when it
only holds surface higher up (an eave, an awning: solid from the lowest of
that surface to the highest, so you walk under it and can stand on it).
Heights are taken to quarter metres and equal columns are merged: 184 boxes
for the five models, 209 as placed. A pitched roof is steps a quarter metre
high: you can jump onto it, walk up it and stand on the ridge, and nothing
falls through. You cannot walk in at a door.

**One list, read by everything** (`vl_place_solids` in
`web/world-village.coax`: a model's boxes where a building of the level
stands; the HUD keeps them as `statics`, made again by `hud_statics` when a
building rises, the level's buildings change, the pack comes in or the game
type changes):

| Who | How |
| --- | --- |
| The character | The engine's box colliders (`physics/query.coax`): stopped by walls, stands on roofs and eaves, falls onto a roof and not through it |
| Blocks and balls | Walls of the rigid bodies' scene (`bd_wall`, added to the engine's `physics/sim/bodies.coax` for this): a ball bounces off a house, a block lands and rests on a roof, a kicked ball stops at a rock. Walls take none of the 64 bodies and are in the scene's digest, so a run still repeats |
| The camera | The orbit's room: it keeps in front of a building as it does a block |
| The hotbar's tools | The tools' ray (`hud_pick`) meets a wall or a roof where it is drawn, so a thing is placed against or on a building and never inside one |
| The grass | The engine's ground table (`arch/flora.coax` `ArchGround`, read by the look's grass and ground: the engine already had it, the page had never given it one): no grass at the nodes more than 0.5 m inside a box that reaches the ground (`VL_INSET`), for every place, risen or not; and a patch of the ground material with no cover lies 0.5 m inside each such box, so the floor inside a building is earth while the grass grows right up to every wall and rock, with no bare square showing round them (the owner, 2026-11-17: the squares looked blocked out). A box too thin to have a floor inside (a wall) keeps its grass |

Every building of the level, the green's nine and any placed since, gets its
solids from the same call: `vl_place_solids(village, place, out, n)` appends the
boxes of `place.model` at `place` (x, z, quarter turns, scale) to `out`; a
caller adds them to the HUD's `statics` and calls `hud_statics`' steps (the
bodies' walls, the colliders, the ground table).

Checked: the pack check verifies that every vertex and every triangle's
middle of every model, a quarter metre or more over the ground, is inside
one of its boxes or within one height step of one (27,176 points), and that
each building's middle is solid from the ground up; `web/hud.coax` rolls a
ball at a wall (it comes back, never inside), drops a block on a roof (it
rests on it) and one under an eave (it reaches the ground), runs the
character at a wall and drops it onto a roof, aims the tools' ray at a wall,
and runs the same script twice to the same bits; `web/world-village.coax`
checks the ground table and the earth. Seen in headless Chrome: the
character on a house's eave after a double jump at it, stopped at the
market's wall, a kicked ball at rest against a rock, bare earth and no blade
under the round house. Not seen: the inside of a building (there is no way
in to look), fly mode over the roofs, a ball's bounce in motion.

**The entrance.** The camera comes down, the grass spreads and the sun rises
as in the meadow; the path is there from the start; and the buildings rise
out of the ground one after another (the first 0.15 s in, one every 0.2 s,
0.5 s each, eased; a new village's nine stand by 2.25 s). The buildings that
rise are the ones the level holds when the world opens; one placed after
that stands at once. A building is solid once it is six tenths up. If the
pack arrives late they rise when it arrives. Leaving, they sink within a
second.

**Building.** The hotbar's tools and the chat's edits work on the green: it
starts with a ball and the green's nine buildings (the meadow's platform,
pillar and wall would stand in the path), and whatever else is placed is a
rigid body as in the meadow. The nine buildings are things of the level like
any placed since (`shared/game-types.coax` `gt_starter`, the one list the
website and the app both open a new village with; `tools/game-types-check.coax`
holds them to it): a person moves, turns and removes them, the level file
keeps them, and the model is told each one in the list of things. The HUD's
action 115 says only what the world is (`a village green. its buildings are
solid, and they are things of the list: yours to move, turn and remove.`,
into the page's hidden field, after the `game type: medieval village` line).
A village kept before the green's buildings were things of the level has
none of the nine in its file: it opens as an empty green with what was
placed on it, and a new village has them.

**Placing buildings.** The village's bar starts with the pack's own models:
a britons house, a gauls house, the gauls' market and a rock (grey or mossy:
the rock's options in its row, or Q and E). The pack has one gauls house
model, not two, so there is one gauls house item. The basic shapes are in
more. A building tool shows a ghost of the real model where the pointer meets
the ground (the middle of the window in first person), lit and shadowed like
the rest, tinted bluish, and red with a cross where it may not stand; its
footprint is outlined on the ground. Snap puts it on the half-metre grid,
free exactly where you point (Z and X, or its options); either way it stands
at rest on the ground. R turns it a quarter on, Shift+R a quarter back, and
the wheel does the same: buildings turn in 90° steps only, because their
solid boxes are kept square to the axes. A click puts it there. It is refused,
and the reason shown, where it would stand in another building or in the
person.

A building is a thing in the page's list, kind 2 (`ID 2 MOVES SIZES X 0 Z
MODEL QUARTER SCALE 1`, SCALE its own, in percent of its model): it counts
among the 1,000 things, at most 33 stand at once, and their solid boxes are
512 between them (the engine's walls; the green's nine use 209; a gauls
house 17, a britons house 43, the market 117, a rock 6, a mossy rock 1). Past
either, the place is refused with `no more buildings: a level's buildings
share 512 solid boxes (N used) and at most 33 stand. delete one to place
another.` A building's scale is 50 to 300 percent of its model (`that scale
is out of range: a building stands at 50 to 300 percent of its model.`); a
tool places one at its model's usual scale, and four of the green's nine
stand at their own. In edit mode a press on one selects it and a drag moves
it (on the ground, checked again); the panel shows its name, its scale and
its turn with ⟲ and ⟳ (no sizes: a model stands at its own, at its scale),
and delete removes it. Its boxes are walls of the bodies' scene, so balls
and blocks rest against it and the person is stopped by it; the grass's
table is made again when the buildings change. The model is told each one
(`bN: building WORD X Z, footprint W D, turned T, scale S`) and can place,
turn, move and remove them with `+ put MODEL X Z [TURN] [SCALE]`
(`britons_house`, `gauls_house`, `market`, `rock`, `mossy_rock`; TURN 0, 90,
180 or 270; SCALE 0.5 to 3) and `+ turn ID DEG`, in the village only. The
website has no control that changes a standing building's scale yet: the
app's inspector does. The models' footprints, heights, usual scales and box
counts, and a building's rules with their words, are
`shared/buildings.coax`, the app's too, checked against the pack by
`tools/village-pack-check.coax`.

**In the level file** a building is a row of the shape table by its model's
word at the model's own size (`3|market|1008|452|777`) and its own column is
its scale in percent, as the app keeps it, so a village made on either side
opens on the other. A file kept before buildings had a scale holds the size
the building stood at in its row, at 100 percent; both sides read that as
its scale.

**In the app** the five models are kinds of the village's palette
(`shared/kinds.coax`: Britons house, Gauls house, Market, Mossy rock, Rock).
Adding one puts it at its model's usual scale at the nearest place clear of
the others; the inspector moves it, turns it by quarter turns and sets its
scale (it shows no sides: a building's size is its model's); a drag's turn
goes to the nearest quarter. What breaks a rule is refused in the words
above (`coax/palette.coax` `ed_building_why`, which agents' batches and the
command rows go through too). The scene that draws them is the website's
own, on the engine's renderer (`coax/world.coax`).

**Who does what.** `web/world-village.coax` is the village: the level's
buildings as the page tells them (`vl_put_in`: a model, a place, quarter
turns, a scale each), the pack's admission and catalog, the entrance's clock, the solid boxes, the
records and draws, the paths, the day, and the words for the model, with
checks. `web/hud.coax` calls it at a handful of places (the game type, the
colliders, the look, the records, the draws, the frame) and owns nothing of
it. This is the shape the next worlds take ("a world": set up, step, draw,
describe, input); the meadow is still in `hud.coax` itself.

Checked: the pin, the catalog (a missing credit line, a missing model, a bad
line), readiness, the entrance's timeline, placement and the solid boxes
(`web/world-village.coax`); no credit text in the page's tree in the village
or the menu, the tooltip on the village's card, and the credits page from
the menu, the footer and its own address, logged in or not (`web/page.coax`); and the pack itself
(`tools/village-pack-check.coax`, run by `scripts/web.sh check`: it is the
pinned pack, the engine's reader admits it, every index and layer is in
range, every solid box is inside its model, and nothing stands where you
start). Seen in headless Chrome with WebGPU at 1440 by 900: the picker, the
entrance at 1, 3.2 and 6 seconds, the day, and a run up the path to the
market's corner. Frames came at the display's 60 a second there (16.7 ms,
median and 95th centile: the browser's own pace, so the frame's real cost is
under that and was not measured further). Not seen: the pack arriving slowly
(it is local here), a refused pack in a browser (checked in Coax), a phone.

## Crates

The sixth game type is its own world and has nothing of the meadow in it: a
puzzle. A small walled lab room seen from above; push every crate onto a
pad; every pad lit opens the gate; walk out through it and the next room
comes up. No sky, no grass, no sunrise: indoors, dark, lit from overhead,
with the pads, the seams between the floor's tiles, the walls' tops and the
gate glowing (the stylized look's glow, bloomed by the film passes).

| | |
| --- | --- |
| W A S D, the arrows | Run, in screen directions (north is up). No jump, no fly mode here (a jump would clear a crate): Space does nothing |
| Run into a crate | Pushes it, by its weight: it slides at about 2 m/s and stops within half a metre of being let go. A crate never turns: it stays square to the tiles |
| R, or ↺ reset on the hotbar (5) | The room again as it was entered: the crates drop in over their tiles, you at the start. A crate pushed into a corner cannot be got out, so this is how a room is tried again |
| The hotbar | Shapes (1), then the room's own kinds (wall, goal, gate, start), edit, decide, world settings, reset, then more. Built in as anywhere ("Every game type is built in"); no camera menu, no fly. Entering, the settings and the gravity are the world's own again |

**The rules.** A crate lights a pad when its middle is within 0.45 m of the
pad's along both axes, it is on the floor and it is going slower than
0.35 m/s (one sliding across a pad does not light it). A lit pad turns from
amber to green, and so does the light on the crate's top. When every pad is
lit at once the room is solved, for good: the seams flash, the gate turns
from red to green and sinks in 0.8 s, and it is no longer solid. Walking out
through it (past the gate's middle) opens the next level of the game, its next
room; after the last comes the first again. A room with no gate goes on by
itself once it is solved; one with no goal is never solved. The camera is the engine's top-down kind held on the
middle of the room from its furthest (12 m), so the whole room is in view
and never moves.

**A room is things of its level**, like any other level's
(`shared/game-types.coax` `gt_starter_level`; issue 2): its walls (`wall`,
fixed), its crates (plain blocks), its pads (`goal`), its gate (`gate`, one
a room) and where you start (`start`, one a room). Each is selected, moved,
sized, added, removed and undone in the editor, on the website and in the
app alike; the floor is laid under the walls' whole extent; a tile is 1.5 m.
The world keeps no room of its own: it reads the level's things each time
(`cr_room_of`). A crates game is three levels (`gt_levels`: 3), a room each,
rising: one crate and a turn; two crates round a block; three crates in a
west chamber that must all go through one door to pads in the east one, in
the right order (the crate through the door blocks it until it is pushed on).
`new-game` with `crates` answers with all three (`"levels":[4,5,6]`), and
`move-level` puts any level into a game whose type offers every kind of
thing it holds, up to 16 a game (`MINE_LEVELS`).

**The next room.** In the app, Play in a room that is solved and left opens
the game's next level and plays on (`coax/world.coax`, `ed_open_due`); the
level it arrives in is the one open when Play stops, and each level has its
own undo history. On the website each room is a level this browser keeps
(`web/keep.coax` slots: `level-6` the first room, `level-10` and `level-11`
the second and third), and logged in, the three are the three levels of one
game on the account (`web/account.coax`): the game is made once with all
three, each room is saved and read as its own level while it is the one
that is up, and a game picked from "your games" opens all its rooms. A
later room whose level the account's game does not have (the level was
deleted there, or the game was made with fewer) stays in this browser, and
the page says so ("room 3 is no longer on your account: it is kept in this
browser only."). A published link plays the one level it was sent: solved
and left, it is the same room again.

**Every room is proved solvable.** Each has a recorded solution
(`cr_solutions`): the keys held, frame by frame at 60 a second, as runs
(`d54x40sa1s11...`: w north, s south, a west, d east, x nothing). A check in
`web/hud.coax` enters each room, lets the crates land, replays the recording
through the real character, the real bodies and the room's rules, and
requires that it ends out through the gate with the next level asked for. The
recordings are of the three rooms a new game starts with; a room that was
edited is not proved by them. `tools/crates-record.coax` makes the
recordings: it walks each room by a short script (go to a tile; push a crate
until it is on a tile), holding the keys a person holds, and prints what it
held. Run it again when a room, the walk or the bodies change:
`vendor/openagents/coax/web/tool.sh tools/crates-record.coax`.

**The entrance.** The camera comes down from the picker's pose to its place
over the room while the picker's grid fades out, the room's light comes on
(0.8 s), the walls rise out of the floor one after another (the first 0.05 s
in, one every 0.015 s, 0.5 s each) and the crates, which hang 2 m and more
over their tiles until the camera is nearly down (0.4 s), drop in one after another. A
room after the first comes up the same way, without the camera's descent.
Leaving, the room sinks and darkens within a second.

**What is the engine's.** The crates are rigid bodies of `physics/sim/bodies.coax`, pushed
by the character as mover 1 exactly as the meadow's blocks are; the walls and
the shut gate are that scene's walls (`bd_wall`) and the character's box
colliders. The camera is `cam/kinds.coax`'s top-down kind. The room is drawn
with the architecture kit (`arch_slab`, `arch_obox`, quads) in the kit's
smooth stone and its glow material, lit by a `StyLook` with no air, no cloud
and no grass. Written here: the room read from its level's things, lit, solved and
through the gate, the entrance's clock, the mesh's layout and colours, the
words for the chat's model.

**One thing reaches into the engine's data**: a crate's inverse inertia is
set to nothing each frame (`hud_crates`), which is what keeps it from
turning. The engine has no call for "a body that slides and never turns"
yet; it should (`bd_upright`, with its own check and mutation run), and this
line should then use it.

**Who does what.** `shared/game-types.coax`: the three rooms a new game
starts with, as things. `web/world-crates.coax`: the recordings, a room read
from its things, entered and left, the rules for a frame, the solids, the mesh, the
light, the description, with checks. `web/hud.coax` calls it at the game
type, the world's solids, the look, the mesh, the frame, the camera's point,
the walk's keys and the page's word (`hud_crates_in`, `hud_crates`,
`hud_meadow`, `hud_no_jump`, `hud_cam_feet`). The page (`web/page.coax`)
has the card, gives a world of its own (`GS_OWN`) the build tools without
the camera menu, adds the reset slot, and tells the HUD a count of presses of the world's own
action as a seventh number of message 113; the HUD acts when the count
changes. The chat's model is told `game type: crates (a crate puzzle, seen from
above)`, how building goes there, and the HUD's words: the room, the pads lit, whether the
gate is open, the rooms cleared.

**What a later refactor should unify.** Each world of its own now has the
same handful of hooks in `hud.coax` (enter, leave, step, solids, look, mesh,
describe, camera point, which keys apply, the world's action) as separate
`if type ==` lines beside the village's. They are one interface: a world
record of procedures the HUD calls, with the meadow as its first
implementation. Not done now: two other agents are in `hud.coax`.

Seen in headless Chrome with WebGPU at 1440 by 900: the entrance at about
0.3, 0.7 and 1.1 s (grid fading, walls part risen, the crate hanging then
falling), room 1 at rest, room 1 solved (the recording replayed by time in
the page: the pad green, the seams flashing, the gate green and sinking) and
room 2 part solved. Frames came 33.3 ms apart (median; 34 ms at the 95th
centile), and the meadow's top-down type came at the same 33.3 ms in the
same browser at the same time: that browser was pacing at 30 a second, so
the room costs no more than the meadow there and its own cost was not
measured. Not seen: room 3 in a browser (checked by its replay in Coax), a
crate stuck and reset by R in a browser (checked in Coax), a phone, motion
(stills only).

## Platformer

The seventh game type is its own world too: a course of floating platforms
high in a bright sky, seen side on. Run right, jump the gaps, ride the moving
platforms and the lifts, let the pink pads throw you up, hop the spikes, pick
up the gold shards, touch each checkpoint's post and reach the goal against
the clock. None of the meadow's grass is in it. The course is things of the
level: each platform, pad, strip of spikes, shard, checkpoint, moving platform
and lift is a thing you select, move, size and delete as any other, and what
you add is part of the course ("Every game type is built in").

| | |
| --- | --- |
| A, D, the left and right arrows | Run along the course (it holds you on its line: there is no in or out) |
| Space, W, the up arrow | Jump; once more in the air (the walk rules' double jump, which side on keeps). There is no fly mode here: two quick presses are a double jump |
| R, or ↺ on the hotbar | The course again from the start: its entrance, the clock at nothing, the shards back |
| The hotbar | Shapes, then pad, spikes, shard and checkpoint, edit, decide, world settings, then start over (9, or R), then more (mover and lift are there: the bar is full). No camera menu: the view is the engine's side-on kind |

**The hero.** Here the character is not the ghost but a runner: the same
Quaternius outfit and clips, drawn solid and lit like the platforms, in the
engine's runner livery (`render/livery.coax`): a red cap with a white stripe
over a bare face, a yellow scarf, a sky-blue jacket with an orange pack, white
sleeves and gloves, navy trousers and red boots, with a cloth grain. Its head,
hands and feet swell about their middles (a third, two fifths, a quarter) so
it reads chunky side on, and a rim of the sky's light lifts it off the sky.
`ghost_paint` gives each vertex its part and colour once (the joint carrying
most of it, refined round the part's middle in the rest pose); `ghost_swell`
swells the pose each frame after the skinning; the skin store carries the
colour (black for the ghost, which the other worlds keep), and the scene
shader lights a coloured vertex as a solid surface, drawn unblended
(`hud_hero`, `hud_draw_blend`). A normal the thinned outfit turns from the eye
on a face toward it is laid across the view, so its layers do not speckle.

**The rules.** Falling into the void (9 m under the lowest platform within
40 m) or standing in a row of spikes puts you back on the last checkpoint
you touched (the start before the first), still. Spikes are not solid: a
strip lies on a platform, and feet in it at its foot are in spikes. A
checkpoint counts when you stand on its platform and it is the next one (the
engine's ordered run: touching a later one first counts for nothing); its
post turns from amber to green. The checkpoints are passed in their order
along the course, and the furthest along is the goal. A pad throws you up at 18.5 m/s (about 5.3 m; a double jump at the
top makes about 7.7). A moving platform or a lift carries what stands on it
by as far as it moves each frame. The clock starts when the course is up and
stops at the goal: the goal's arch flashes green and a board over it lights
with the time (minutes and seconds) and the shards taken of those there are,
in bars of light. The chat's model is told the type, the checkpoints
touched, the shards, the falls, and the time once finished.

**The course is things of the level** (issue 2). A new platformer starts
with its course as 111 things (`shared/game-types.coax` `gt_starter`, one
source for the website and the app): 44 platforms (plain blocks), 9
checkpoints (the last the goal), 4 pads, 6 moving platforms (kind `mover`),
4 lifts (kind `lift`), 6 strips of spikes and 38 shards. They are kept in the
level's file and read by the world from the level (`pf_make`,
`web/world-platformer.coax`): a plain block is a platform, a checkpoint a
platform that is a gate, spikes a strip lying on what is under it, a shard a
mark. The world holds no course of its own (`tools/game-types-check.coax`
fails if it does). A level with no checkpoint (an empty one, or one kept
before the course was things) is played on a new game's course with the
level's things on it, so there is always a goal.

The new game's course is written once, in `GT_COURSE`: one line of short
words a section, read by `gt_course` from a cursor (along the course, at a
height): `pL` a platform L long, `gL` a gap, `uH`/`dH` the next one H higher
or lower, `bL` a pad, `mL,D,P` a platform that slides D along and back every
P seconds, `lL,D,P` a lift that rises D and comes down every P seconds,
`sA,L` spikes on the last platform, `*H` a shard H over the height here, `c`
the last platform is a checkpoint, `f` it is the goal. Nine sections: first
jumps; double jumps; pads; moving platforms; lifts; spikes; a mix; a mix
with a lift; a staircase, a last lift and a last pad up to the goal. About
640 m, from 4 m up to 48.5 m. A side is 12 m at most, so the two platforms
longer than that are two things each.

**A moving platform's and a lift's distance and period.** Each has how far it
goes (0 to 50 m) and how long there and back takes (1 to 60 seconds); a new
one goes 8 m (a lift 6 m) in 6 seconds. In the app they are the inspector's
Distance (cm) and Period (ms) fields, shown only for those two kinds and set
as one edit each that undo takes back (`set-field LEVEL THING distance CM`,
`period MS`). On the website the plan line is `+ goes ID DISTANCE SECONDS`.
Past a limit either is refused in words: "out of range: a moving platform or
a lift goes 0 to 50 m, there and back in 1 to 60 seconds."; on any other
thing: "only a moving platform or a lift has a distance and a period." A
level with one has two columns more in its shape table (`go:cm every:int`);
a level with none is written as before. While a level is built in the app
each stands where its thing is; played, it moves.

**The course is proved completable.** `PF_SOLUTION` is a recorded run: the
keys held, frame by frame at 60 a second, as runs (`d43dj20...`: d right,
j jump, x nothing). A check in `web/hud.coax` brings the course up, replays
it through the real character, the walk rules and the course's rules, and
requires that it touches every checkpoint in order and reaches the goal with
no fall in two to three minutes. The recording takes 134 s and 27 of the 38
shards; a person who goes for the shards and falls now and then takes longer.
It is replayed twice: on the world's own course and, through the app's keys,
on a new game's level (`coax/world.coax`), so the course as things is the
course that was recorded. `tools/platformer-record.coax` makes it: a small player that runs right,
jumps at a platform's end when the next is not joined to it and before
spikes, jumps again in the air when it would come down short, coasts and
holds back over a short platform, waits for a moving platform or a lift to
come to its near end before jumping on, rides one to its far end, and never
jumps on a pad. Run it again when the course, the walk or the rules change
(the HUD's checks run when the tool is compiled, so take the replay's
asserts out while recording, or the old recording fails them):
`vendor/openagents/coax/web/tool.sh tools/platformer-record.coax`.

**The entrance.** The camera comes down to its place beside the start while
the grid floor's slabs rise and settle as the platforms (each up past its
place a little and back; the nearer the start the sooner, 0.015 s later a
metre along, all by 0.5 s, 0.45 s each), the floor falls away under them (from
0.3 s, 320 m in 0.7 s, into the haze) and the sky comes up from black to a high,
bright, cloudy day (1 s). The start's own slab stands from the first frame: you
are on it, and run and jump from it at once (a slab rising that fast under your
feet would throw you). At 1 s the floor is gone (nothing is under the course:
the void), the platforms start to move and the clock starts. Leaving, the
course sinks back within a second.

**What is the engine's.** The moving platforms and lifts are
`world/course.coax`'s slides (added to the engine for this: a slide eases
to and fro between two points; where it is, how fast, how far a rider is
carried over a step); the checkpoints and the goal are its run (gates passed
in order, against the clock: the last one passed is where you go back to);
the shards are its collectibles (`course_take`). The camera is
`cam/kinds.coax`'s side-on kind, its lens widened here to 0.95 rad
(`PF_LENS`: the side-scroller's long lens shows 6 m, too little to see a gap
coming). The double jump and no flight are the walk rules as side on already
has them (`hud_tuning` keeps `air_jumps` for a planar kind; `hud_walk` stops
fly mode there). The sky is the engine's atmosphere with its clouds
(`StyLook.air`, `cloud`), mixed up from black with `sty_look_mix`. Everything
is drawn with the architecture kit in its smooth stone and glow materials,
bloomed by the film passes.

**Who does what.** `shared/game-types.coax`: a new game's course
(`GT_COURSE`, `gt_course`). `web/world-platformer.coax`: the course made of
the level's things (`pf_make`, `pf_lay`), its recording, entering, the rules for a frame (`pf_step`: shards,
checkpoints, the goal, spikes, the void, pads, what the platform under you
moved), the solids near you, the ground (none once the course is up), the
mesh, the light, the description, with checks. `web/hud.coax` calls it at
the game type, the world's solids, the ground of its colliders, the look,
the mesh, the frame (before the walk: `hud_platformer`, which puts you back,
throws you, carries you), the lens and the page's word (`hud_plat_in`,
`hud_platformer`, `hud_lens`). The page has the card (key 7), gives it the
build tools as any world of its own, and shows the reset slot as
"start the course over (R)". Running the course at 1440 by 900 in headless Chrome on
this Mac, frames came at the display's 60 a second (median 16.7 ms, 95th
percentile 17.2 ms, worst 17.6 ms over 300 frames). The card is quality 88
JPEG, not 48 like the others: the open sky bands at 48.

**What a later refactor should unify** (as for crates): the worlds of their
own are now three sets of `if type ==` lines in `hud.coax`; with the
platformer the list of hooks is enter, leave, step before the walk, solids,
ground, look, mesh, lens, camera point, keys, describe and the world's
action. A world record of procedures would hold them.

## Racing

The eighth game type is its own world: a closed circuit in a meadow at the
end of a summer day, the sun low and warm. Drive a small roadster three laps
against the clock, through the checkpoints in order. Built in as anywhere
("Every game type is built in"); no fly mode.

| | |
| --- | --- |
| W, the up arrow | Throttle |
| S, the down arrow | Brake; held at a stop, reverse |
| A, D, the left and right arrows | Steer |
| Space | The handbrake (the engine vehicle's: the tail swings out) |
| R, or the hotbar's slot | Back on the track at the last checkpoint passed, still and on the line (after the flag: race again from the grid) |
| The hotbar | Shapes, boost, cone, edit, decide, world settings, then back on the track (7, or R), then more. No camera menu: the camera is a chase, behind the car, turning after it |

**The rules.** Eight checkpoints round a lap, the last of them the start
line. A checkpoint counts only as the next one (the engine's ordered run in
`world/course.coax`): the barriers hold the car on the ribbon, and passing a
gate out of turn gains nothing, so there is no cutting. Off the road and its
kerbs, on the gravel, the car slows. On its roof or its side, or off the
ribbon, for 1.5 s, it is put back on its own. The clock starts at green and
stops at the third crossing of the line; each lap's time is kept, and the
best. A board over the car shows the lap (1/3) and its time, and after the
flag the race's time and the best lap; the gantry's three lights count 3, 2,
1 and go green. The chat's model is told the type, the lap, the checkpoint,
the times, the best lap and the finish.

**The track is things of the level**: a new racing game starts with nineteen
track points (kind `track`; `shared/game-types.coax` `GT_CIRCUIT_*`), each
where the line goes, how high the road is there (its bottom's height) and how
far its corner banks (its turn, degrees). The page tells the HUD the level's
things; the track points among them, in the list's order, are the circuit
(`hud_track_points`), the second the start line. Move, add or remove one (the
editor in the app, a plan line or the tools on the website) and the road is
laid through them again (`rc_lay`: the car back on its grid, the grass's table
made again). With fewer than three, or points that make no ribbon, the
circuit is the first one. A level kept before the circuit was level data has
no track points and plays on the first circuit. The engine's `world/track.coax` lays a closed ribbon through them
(Catmull-Rom, 12 samples between two points, a frame at each: ahead, right,
up, with the bank eased in by how tight the turn is, the outside raised) and
answers where a point is on it (how far along, to the side, over), the frame
at any distance, and whether a mark was passed going forward. The circuit is
about 780 m: a start straight, banked sweepers rising 5 m to a crest, a
hairpin, an S and a fall back to the line, its corners no tighter than 11 m.
Kerbs are drawn where the bend is tighter than a 45 m radius. The road is
11 m wide with 7 m of gravel to the blue and white barriers, which the car
meets as two planes beside it (the engine's contact planes).

**Proved drivable.** A check drives three laps with an autopilot (pure
pursuit of a point ahead on the line, slowing for the bend coming): every
gate in order, three laps timed, the best kept, all under 150 s (about 38 s
a lap, at up to 40 m/s, never on the gravel, never put back). Others check
that a gate out of turn counts for nothing, that R puts the car at the last
checkpoint, that a car on its roof comes back, that driving straight at a
barrier stays inside, that the mesh fits through the entrance and the race,
and the page's words.

**The entrance.** The floor's grid lines bend into the ribbon's edges (0.5
s), the ribbon fills in from the start line both ways (from 0.25 s, 0.55 s
long) while the meadow and the low sun come up from black (1.5 s), the car
drops onto the grid 16 m before the line and settles on its springs (from
0.1 s, 0.6 m), and the lights count from 0.25 s, a quarter of a second each:
green a second in. The lights are the rule (nothing moves the car before
green), kept short for the first race so a person drives within about a
second of picking the type; building is never held for them. A race again
(R after the flag) keeps the full count: the first light 0.9 s after R, a
second a light.

**What is the engine's.** The car is `physics/vehicle.coax` (a roadster
tuned here: `rc_car`) on `physics/sim/rigid.coax`, stepped at a fixed 120 a
second; the ribbon is `world/track.coax` (added to the engine for this, with
checks and mutation); the checkpoints are `world/course.coax`'s ordered run.
The sky is the engine's atmosphere from a low western sun, mixed up from
black with `sty_look_mix`; the grass is the stylized frame's ground table,
kept off the road and the gravel. Everything is drawn with the architecture
kit (its smooth, metal and glow materials): the road, the edge lines, the
kerbs, the chequered line, the gantry, the grandstand and the cypresses.

**Who does what.** `web/world-racing.coax`: the track and the car, the race
and its step, the barriers, the autopilot (for its check), the entrance, the
mesh (the circuit drawn once it is in and kept; the car, the lights and the
boards each frame), the ground table, the light and the description, with
checks. `web/hud.coax` calls it at the game type, the keys (`hud_racing`),
the ground table, the look, the mesh, the camera (`hud_race_chase`: a third
person orbit eased round behind the car) and the page's word. The page has
the card (key 8) and shows the reset slot as "back on the track (R)".
Driving at 1440 by 900 in headless Chrome on this Mac, frames came at the
display's 60 a second (median 16.7 ms, 95th percentile 17.1 ms, worst 17.5
ms over 300 frames). The card is quality 76 JPEG.

**What a later refactor should unify**: with racing the worlds of their own
are four sets of `if type ==` lines in `hud.coax` (enter, leave, step,
solids, ground table, look, mesh, camera, keys, describe, action), and the
digits of light (`pf_bars`, `rc_bars`) and the boards are drawn twice; a
world record of procedures and a shared board of light would hold them.

## Drift

The ninth game type is its own world: a small ship in zero g among
drifting, tumbling rocks, a lit planet with a moon going round it and a sun
far off, the stars and a galaxy's band all round. Fly twelve glowing rings
in order against the clock. Built in as anywhere ("Every game type is built
in"); no fly mode (the ship flies).

| | |
| --- | --- |
| W, S | Thrust ahead and back |
| A, D | Strafe left and right |
| Space, Shift | Rise and sink |
| The mouse (a drag), the arrows | Aim: the ship turns to look where the camera looks; zoomed all the way in, first person |
| R, or the hotbar's slot | Back at the last ring passed, still (after the last ring: the course again) |
| The hotbar | Shapes, edit, decide, world settings, then back at the last ring (5, or R), then more. No camera menu: the camera is a chase, its yaw and pitch the ship's aim |

**The rules.** A ring counts only as the next (`world/course.coax`'s run,
`course_through` across the ring's disc), so no cutting; the board over the
ship says the ring and the time. A gentle drag lets the ship come to rest
(full thrust tops out near 45 m/s).

**The engine does the work.** The ship and the rocks are rigid bodies
(`physics/sim/rigid.coax`) stepped in no gravity (`physics/gravity.coax`),
pushed by thrusters and turned by RCS (`physics/thrust.coax`); the ship
nudges the rocks and they nudge each other (`contact_sphere_sphere`). The
rings are the engine's ring course (`world/rings.coax`), through the level's
ring things, and the sky is `world/starfield.coax`'s. All at a fixed 120 steps a second.

**The entrance.** The grid floor falls away, its tiles breaking off and
rising as the rocks, while the stars come out and the sun comes up, all
within a second. Nothing is held for it: the ship flies and the clock runs
from the world's first frame, and the view starts behind the ship, looking
the way it faces, because the view is its aim.

**Who does what.** `web/world-drift.coax`: the course, the ship, the rocks,
the sky, the mesh, the look and the description, with checks (a pilot flies
the whole course). `web/hud.coax` calls it at the game type, the keys
(`hud_drifting`), the camera (`hud_drift_chase` coming in), the floor, the
mesh and the page's word, and gives it what was built. The page has the card
(key 9) and shows the reset slot as "back at the last ring (R)". Flying at
1440 by 900 in headless Chrome on this Mac, frames came at the display's 60
a second (median 16.6 ms, 95th percentile 17.9 ms, worst 18.7 ms over 300
frames). The card is quality 90 JPEG (the black of space is small).

**The rings are things of the level.** A new drift game starts with twelve
rings (kind `ring`; `shared/game-types.coax` `GT_RING_*`, the course the
seed 2207 once gave, kept as numbers). A ring thing is the ring's middle: a
cube 2 m a side stands for a ring 6.5 m in radius, and a thing made wider is
a ring as much wider. The page tells the HUD the level's things; the rings
among them, in the list's order, are the course (`hud_ring_marks`,
`dr_course`): each ring faces along the line through it, between the way in
and the way out. One moved, sized, added or removed and the course is laid
again (`dr_lay`): the rocks drift round the new line, the ship is back at the
start and the run starts over, while the clock, the sky and what was built
stay. A level with no ring flies the first course, and so does a level kept
before the rings were level data.

**Not yet:** a gate or a beacon as kinds of drift's own; the start, which is
where the floor's middle was, as a thing.

## The credits page

`/credits` lists every work of someone else's the site uses: what it is, who
made it (a link), its licence (a link), and where the licence asks for it,
what was changed and where the changed files are. Today: 0 A.D.'s buildings
and rocks (Wildfire Games, CC BY-SA 3.0, with the pack, the note and the
tool); the Quaternius character, rig and clips (CC0: no credit is required,
it is given); Geist and Geist Mono (SIL Open Font License 1.1, the licence
file served beside the fonts); ARWES, whose frame geometry the HUD's
frames are a port of (Romel Perez, MIT); and the eight small icons the
agents page draws, from Apps SDK UI (OpenAI, MIT, their notice served beside
them at `/build/icons/NOTICE`). The rows are data
(`gs_credit_rows` in `web/page.coax`): something new that is shipped gets a
row.

It is a view of the one page, like the login: `web/page.coax` draws it at
`/credits` (a direct address and a reload work; `web/build.sh` makes the dev
server's copy, and `scripts/web-deploy.sh` uploads the object `credits` and
has the load balancer's rules for `/credits/` and `/credits/index.html`,
which the owner applies with its `domain` step). Anyone may read it, logged
in or not: being logged in does not turn it into the world. The HUD shows
the home page's flat grid behind it. The ways to it are quiet links: "credits"
in the account menu and in the home page's footer. The way back is "back"
(or the name in the top bar): logged in, to the world as it was (the game
type you had, its entrance again; the picker if you had not picked); a guest
who came from the builder, "back to the world" too (the name takes a guest to
the home page); else to the home page.

## The camera

*(Third person. The other game types' cameras are in "Game types".)*

One camera, the engine's (`StyCamera`). At home and at the login it is a
fixed pose; in the world it is the engine's orbit camera, the one Everglade
uses. The HUD eases between them.

| View | Camera |
| --- | --- |
| Home | Straight down, far enough that a metre of floor is 48 CSS pixels, on the middle of the window, sliding with the page's scroll |
| Login | The same point from the same distance, leaned back 45 degrees |
| World | The engine's free orbit round the character |

**The orbit** is `metaverse-engine/cam/camera.coax` (`Orbit`), stepped the way
Everglade steps it (`cam/chase.coax`). Nothing of it is written in this
repository: `web/hud.coax` hands it the input, the character's feet and the
blocks, and turns its shot into the frame's camera. Its numbers are the
engine's, the same as Everglade's:

- It circles a pivot 1.45 m above the character's feet, with the character a
  little left of the middle (the view is centred 0.55 m to its right, over the
  shoulder). The pivot catches up with the character, so a run or a jump moves
  the view smoothly.
- It looks up 0.55 radians past level and down 1.3 (nearly straight down).
- The zoom goes from 12 m out to 1.5 m, easing. At the closest, a little more
  of the wheel goes on **into the character's head: first person**, the eye
  where its eyes are (1.62 m), free to look nearly straight up and down. Any
  zoom out comes back out.
- In its head, the character's body is left out of the view (it fades in a
  moment); it still casts its shadow.
- It keeps in front of blocks and 0.25 m above the ground: the eye comes in at
  once when something is in the way and backs out gently when it is not. When
  that pulls the eye in to the character's shoulders (closer than 0.9 m), the
  body is left out too. That last rule was Everglade's own code; it is now the
  engine's (`metaverse_orbit_unseen`, openagents `af6778424a`) and both use it.
- A while after the last look, running eases the tilt back to where it rests
  (0.3 radians down). The turn is never changed for you.
- **W runs away from the camera**, wherever it has turned; A and D are to its
  left and right. The character turns only to face the way it is steered:
  turning the camera round a standing character does not turn it.

**The entry.** The world opens with the orbit behind the character, 9.5 m
back (this page's own choice; Everglade starts at 5.2 m), looking the way W
runs and 0.12 radians down (`HUD_WORLD_PITCH`; the orbit rests at 0.3), so
the top third of the window is sky and the sun comes up in it ("The sky").
Running eases the tilt to where the orbit rests, as the engine does. The camera comes down from the page's pose to the orbit's in 0.6 seconds (`HUD_DOWN`), and takes a
look and a zoom from the world's first frame, while it is still coming down ("The first second"). Leaving the world it goes back up to the
home pose, turning back as it goes; the next time the world opens the orbit
starts over behind the character.

**Controls.**

| Input | Does |
| --- | --- |
| Drag the world, either mouse button | Turns and tilts the camera (0.0055 radians a CSS pixel) |
| Wheel, trackpad scroll or pinch, over the world | Zooms; all the way in is first person |
| Left and Right arrows | Turn the camera |
| Up and Down arrows | Tilt it |
| = and - | Zoom in and out |
| W A S D | Run, away from the camera on W |
| Space | Jump |
| ◎ beside the chat box | The follow camera, on or off (bright when on) |
| Touch: a finger on the right half | Looks round; a quick tap there jumps |
| Touch: a second finger on the right half | Pinch to zoom |
| Touch: a thumb on the left half | The move stick, from where it went down (a ring is drawn); it runs |

**Free and follow.** The camera opens free: the turn is never changed for
you, and W A S D are relative to the camera. The ◎ button beside ✦ turns the
follow camera on (`aria-pressed`, bright when on).

Following, the camera is **always directly behind the character**: standing,
running, backing up, after a drag. Turned on, the view swings round to the
character's back in about a quarter of a second (the character does not
turn), and from then the body and the view are locked together, the way
Everglade II's mouselook locks them: the body faces where the view looks, the
same frame. The keys are then the character's own:

| Input, following | Does |
| --- | --- |
| W | Runs the way it faces |
| S | Backs up, still facing ahead, at the run's speed |
| A and D, Left and Right arrows | Turn the character, and the camera with it, standing or running (2.2 radians a second, 126 degrees: the orbit's own key turn) |
| A drag left or right, a finger on the right half | Turns the character and the camera together, as far as the same drag turns the free camera |
| A drag up or down, Up and Down arrows | Tilt the view |
| A thumb on the left half | Up runs, down backs up, sideways turns |
| Wheel, pinch, = and - | Zoom, into first person, as before |

Pressed again it is free: the camera stays where it is and the keys are
relative to the camera again. Nothing here is a new camera: the turn is the
orbit's (`metaverse_orbit_keys`, `cam_drag`), and the walk is the app's rules
with the view's yaw as the body's. Backing up plays the run clip (there is no
backward clip in the baked model). The page owns the button
(`web/chat.coax`, `web/page.coax`); the glue tells the HUD (message 109). A
logout leaves it as it is; the camera picked is a setting (`camera.view`), kept with the level.

The arrow keys used to run the character. They are the camera's now, as in
Everglade.

A drag or a wheel that begins on the chat, the account menu, a link, a
button or a field is the page's and does not move the camera. Over the world
the page does not scroll, no words are selected, the other mouse button opens
no menu, and a finger neither scrolls nor zooms the page. A mouse held on the
world stays the camera's until it is let go, wherever it goes. Keys are the
character's and the camera's only while nothing is being typed.

How the input arrives: the page host (`coax/web/host.js`) hands every pointer
of the window to the HUD (`hud_pointer`); the glue tells it, just before a
press, whether the press is on one of the page's own things (message 107) and
sends the wheel (108) and the held keys (104). The rules are in
`web/hud.coax`, with checks.

First person holds the pointer on a click ("Game types", "The held
pointer"). Third person does not: the drag is the look there.

At home and at the login the ground is the engine's ground with no grass
revealed: a black graybox floor with a faint line every metre and every four,
fading to black with distance, dimmed behind a panel of words and toward the
window's edges, lit a little round the pointer. Both look as they did before
the engine (compared in a browser at 1440 by 900: of 1,000 sampled pixels
none differs by more than 6 of 255).

## The first second

A person can move, look and build within about a second of picking a game
type (issue 14), the fly through its portal included (about 0.7 s; none under
reduced motion). The entrance is not a wait: it goes on round them. A type's
name pressed before the page's code is up is a link to `/play?type=ID`, so
the press is kept: the builder opens with that type picked.

- **Nothing is held for the entrance.** The world's first frame takes the
  keys, a look (a drag, the held pointer, the arrows), a zoom and a press of
  the hotbar's tools, in every game type (`hud_looks`, `hud_tooled` in
  `web/hud.coax`: the world is shown, nothing more). The camera is still
  coming down then; a tool's ray is the camera's as it is that frame.
- **Every entrance is short.** The camera is down in 0.6 s (`HUD_DOWN`), the
  grass out in 2 s (`HUD_SPREAD`), the sun up in 4 s (`HUD_SUNRISE`), and each
  world's own entrance is up within about a second (the sections above).
- **A rule that needs a start keeps it, short.** Racing's lights still hold
  the car till green: a quarter second a light for the first race, green a
  second in. Building is never held for a rule. Drift has no start: the ship
  flies and the clock runs from the first frame.
- **Reduced motion: no entrance.** When the browser says the person prefers
  reduced motion, the world comes up settled: the camera down, the day, the
  grass out, each world's own entrance over, the things it opens with at
  rest. Racing's lights still count (they are the rule, not the entrance).
- **The settled start.** A kept world come back (a restored game) comes up
  the same way, with no entrance. The seam is the page's: `gs_still(s, true)`
  in `web/page.coax`, said before the world is shown; the page's class then
  carries the word `still` beside the type (`gs v2 g3 still`). Picking a type
  on the ring says it is a new world again.
- **How the HUD is told.** The glue sends message 117 (`HUD_CALM_MSG`) each
  frame, before the type and the view: bit 1 the browser's
  `prefers-reduced-motion`, bit 2 the page's `still`. Either, and `hud_calm`
  is true: `hud_settle` puts the camera, the grass and the sun where the
  entrance ends, and each world settles where it is entered (`cr_settle`,
  `rc_settle`, the platformer's and drift's clocks, the village's in
  `hud_village`).
- **A slow load says how far it has come, in words.** While the world's code
  arrives, a line under the top bar says so (`#coaxial-load` in
  `web/index.html`; the host fills it as it loads: engine and its megabytes,
  compiling, the GPU device, shaders). A load quicker than a third of a
  second shows nothing, and the line goes once the world draws.

**The check.** `scripts/web.sh entrance` starts the dev server on a port of
its own and runs `tools/entrance-check.coax` in headless Chrome with WebGPU:
for each of the nine types it presses the card and times, in the page, how
long until the world takes a key (the character, the ship or the car moves;
racing by the car's own speed, so it includes the lights) and until a thing
is placed with the hotbar's first tool. Either over 2 seconds, or never,
fails. It then checks the load's words and the words for each way WebGPU can
be missing ("No WebGPU"). It runs beside the web checks, not inside
`scripts/check.sh`: it is a timing in a real browser, and a loaded machine
would fail it for no fault of the page. Measured on this Mac (Apple M-series,
Metal), milliseconds from the press on the card, before and after:

| Type | First key, before | after | Placed, before | after |
| --- | --- | --- | --- | --- |
| Third person | 166 | 156 | 2100 | 106 |
| First person | 164 | 181 | 2364 | 248 |
| Top-down | 163 | 163 | 2097 | 95 |
| Side-scroller | 148 | 180 | 2231 | 113 |
| Medieval village | 248 | 180 | 2082 | 264 |
| Crates | 181 | 182 | 2114 | 116 |
| Platformer | 114 | 131 | 2114 | 115 |
| Racing | 6682 | 1081 | 2116 | 115 |
| Drift | 3667 | 182 | 2118 | 114 |

Checks in `web/hud.coax`: "nothing is held for the entrance: the first frame
of each of the nine game types takes a look, the hotbar's tool and the keys",
"reduced motion: every game type comes up settled, with no entrance", "the
settled start: a kept world come back comes up with no entrance".

## No WebGPU

The world is drawn with WebGPU. A browser that cannot run it is told so in
words, in the site's voice, before the builder is entered, and never sees a
blank, black or frozen page.

- **Three ways it can be missing**, each with its own words and its own way
  on (`web/gpu-words.coax`, the one copy): no WebGPU at all (a browser that
  has it, or the app); WebGPU with no graphics adapter behind it (turn on
  hardware acceleration, or another browser, or the app); a device that
  would not start, its shaders failed, or it was lost and not got back
  (reload to try again; update the browser; or the app).
- **Who finds out.** The glue only asks the browser: no `navigator.gpu` is
  the first; its own `requestAdapter()` answering nothing is the second; the
  host taking the page's `gpu` class off when its start fails
  (`coaxial/host.js`) is the third. It writes the way's digit into the page's
  hidden field `gpu`.
- **Where it is said.** On the home page, under the game types' names, which
  stay a list (a bordered note, drawn by `web/home.coax`); before the login form at `/login`, since the
  login leads to the builder; and in the builder's place (`/play` for anyone,
  `/` when logged in), under the same names, instead of the ring and the world. `/login/cli` needs no WebGPU and says
  nothing. When the start fails, the loading line also says what failed and
  offers to try again.

Checks: "the words for each way WebGPU can be missing" in
`web/gpu-words.coax`; "a browser that cannot run the builder is told so in
words, each way WebGPU can be missing" and "logged in with no WebGPU, the
builder's place says so in words and is never blank" in `web/page.coax`; and
`scripts/web.sh entrance` takes WebGPU away each of the three ways in
headless Chrome and reads the page's words at `/` and `/play`.

## Counting the loop

The site counts each step of its core loop (issue 18), so the owner can read,
for yesterday, how many visits reached each step and where they left.

- **One list.** `shared/loop.coax` names the steps and what they are counted
  under: arrived, create, builder, picked, placed, prompt, account, message,
  published, shared, remix, kept (a game built here kept on the account for
  the first time, issue 16); the fact nogpu; the measures button_ms (the time to the types' names on the home page) and
  builder_ms. The page, the server and the funnel all read it.
  `tools/loop-check.coax` (in `scripts/check.sh`) fails when a call counts
  anything but a row of it, or a file counts past it.
- **What is sent.** A step's name and a number of milliseconds (since the
  visit's step before it; for a measure, the measure). No id of any kind: not
  for the person, the browser, the tab or the visit. No cookie, no third
  party's script. The request is the page's own, to `POST /api/count` on
  gamebuilder.org itself.
- **What is kept.** One file a UTC day, `DIR/.loop/YYYY-MM-DD.json` beside the
  accounts' folders on the API's server: for each row a total, and how many
  times fell under each of fourteen bounds. No single visit, time of day,
  address or account is in it (`coax/loop-day.coax`).
- **A visit is a browser tab.** Each step is counted once in a tab. The note
  of which ("MASK LAST") is in the tab's `sessionStorage`, which goes with the
  tab and is sent nowhere.
- **It never touches the first load.** Nothing is sent in a page's first five
  seconds unless the person does something first; a count held when the page
  is left goes as the browser's beacon. A count is sent once, its answer is
  not read, and the login's and the chat's requests go before it and take the
  wire from it. A browser driven by a program, a dev server and a player
  test's copy count nothing (`?count=1` lets a copy be tried).
- **Reading it.** The `funnel` row: `gamebuilder funnel --remote` answers
  today so far, yesterday and the five days before, each step with how many
  reached it, the percent of the step before and how many left before it, and
  the middle time. On gamebuilder.org only an owner of the shipped games
  (`GAME_OWNERS`) reads it. A step can exceed the one before it: the builder's
  address opened directly was never the home page's ring.
- **A new step** adds its row to the list and one call where it happens:
  `gs_count(s, LP_PUBLISHED)` (`web/page.coax`). The steps counted today are
  the ones the page has; published, shared and remix wait for issue 17's
  calls, and the account sheet (issue 16) calls `gs_count(s, LP_PROMPT)`
  where it opens.

Checks: `web/count.coax`'s own; "the loop is counted as a visit goes through
it" and "nothing is counted for a program's browser or off the site" in
`web/page.coax`; `coax/loop-day.coax`'s; `tools/hosted-check.coax` (the door
with no login, a name the list has not refused whole, another site's page
refused, the funnel only for an owner, the day's file holding totals alone).

## The grass

When the world comes up, grass spreads from the character's feet in every
direction and reaches the horizon in 2 seconds. Behind its front the ground
is meadow under the engine's blades, bent by the engine's wind and parted
round the character's feet; ahead of it the ground is still the grid floor.

- The front's shape is the engine's reveal (`metaverse_style_reveal`). Its
  timing is this page's (`hud_front` in `web/hud.coax`): the reciprocal of
  the front's distance from the camera falls evenly with time, so on the
  screen it climbs to the horizon at a steady pace. The horizon is 200 m,
  where the haze has hidden the ground. It spreads from where the
  character stood when the world came up.
- It is lit by the sky it spreads under ("The sky"): nearly black when it
  starts, in the first cold light as it runs out, and reaching the horizon
  as the sun's disc clears it.
- The field is 480 by 480 blades (77 m square: the camera looks any way), 3 m
  of it behind the eye and the rest ahead, a blade every 16 cm, thinning to
  nothing by 60 m. Past the blades the ground is the colour the blades show
  from far off, so the field goes on to the horizon as grass.
- Leaving the world, the grass goes back within a second.

**The blades are the engine's lush grass** (`StyLook.lush`, added to the
engine for this page; `render/shade/stylized.coax`). Nothing of it is written
here: `hud_day` turns it on and gives the blade's height (0.46 m), width
(4.6 cm) and spacing.

- Each blade is curved: nine vertices, bent through its middle, coming to a
  point. They grow in tufts half a metre apart and lean out of them, some
  tufts taller than others, with a pale seed stalk here and there.
- Colour: a deep green at the root to a bright tip, fresh green where the
  field is lush and a dry gold-green where it is dry, in patches from a few
  metres to tens of metres across, each blade a little its own hue.
- Light: the sun falls across each blade's bend, so the wind's waves are seen
  as light rolling over the field; blades are dark toward their roots (the
  field has depth), glint where they mirror the sun, and a low sun still
  comes through their tips at dawn.
- **Things lie in it.** Every block and ball on or near the ground is told to
  the grass as a rest (`sty_rest_add`, the twelve nearest the character):
  no blade grows under it and the blades thin toward it, so a block sits in
  the grass. The character's feet still part it.
- **A small window** (narrower than 700 CSS pixels, a phone's) gets a field
  of 190 by 190 wider blades 20 cm apart, to 32 m: under a sixth of the vertices.

The grid's lines do not show through the grass: the engine's ground material
is the floor or the meadow, by the reveal, and blades cover the meadow.

## The sky

The world comes up under a pitch black sky. The sun rises dead ahead of the
camera (the way W runs) and four seconds later it is day, and stays day.

| Seconds | What you see |
| --- | --- |
| 0 | Black: no sky, no light. Only the floor's grid and the character show |
| 0.3 to 0.8 | First light: a dark blue sky over a low red-orange glow where the sun will rise, the clouds dark shapes, the first grass in cold light |
| 1.3 | An orange band along the horizon under a slate-blue sky; the clouds catch the sun before the ground does and turn rose and gold |
| 1.7 | The sun's disc is on the horizon: a deep orange ball, larger and a little flattened there, with its glare |
| 1.9 to 2.5 | It clears the horizon and climbs, turning from orange to white-gold; long shadows run toward you and the far grass is lit from behind, gold at its tips |
| 4 | Day: a blue sky that pales to the horizon, soft white clouds lit from the sun's side, the sun 42 degrees up and 35 degrees right of ahead, small and very bright with a soft glare |

**The timing is one constant, `HUD_SUNRISE` (4 seconds) in `web/hud.coax`.**
The shares of it at which first light, the horizon and golden light fall are
the engine's (0.15, 0.42, 0.62; `sty_sunrise`).

What draws it, all of it the engine's and none of it a picture or a pack:

- **The air** (`render/shade/atmosphere.coax`, `StyLook.air`): the sky's
  colour comes from where the sun is, by how much air the eye's ray and the
  sun's light pass through. Nothing sets a sky colour here: `hud_day` names
  the sun, the cloud cover and the exposure, and the engine does the rest at
  every height of the sun, under the horizon too.
- **The sun**: a small disc 48 times as bright as white, soft at its edge,
  deep orange on the horizon.
- **The clouds**: soft heaped cloud all round the dome and thin high streaks,
  shaded on the side away from the sun and lit through their thin edges. Turn
  the camera: it is sky every way, with no join.
- **The frame is light** (the look's film entries, named in
  `web/gpu/hud.coax`): the page's host keeps it in a 16-bit target, blooms
  what is brighter than white (the sun's glare, the bright horizon, cloud
  tops) and tones it once. The floor, the grid, the blocks and the ghost keep
  the colours they had: the tone leaves anything under 0.6 alone.
- **The world's light follows**: the same look lights the grass, the blocks
  and their shadows each frame. Distance fades to the far air's own colour
  that way, so the land meets the sky with no band; and grass lets a low sun
  through its blades toward you.

It runs once each time the world comes up. Leaving the world, the sky goes
back to black within a second, with the grass. Home and the login are as they
were (compared in a browser at 1440 by 900 against the page before: the login
differs by at most 1 of 255 anywhere, the home page in 6 pixels). The page's
panels needed no change: the logo, the account button and the chat have their
own dark grounds.

Not built: light shafts and stars (Everglade's painterly sky has both), and a
phone tier (fewer cloud octaves, no bloom).

## The character

A ghost of a real model: the hooded ranger from Quaternius' **Modular
Character Outfits - Fantasy** (`Outfits/Male_Ranger.gltf`, CC0 1.0), on the
65-joint rig those outfits are made for, playing four clips from Quaternius'
**Universal Animation Library** (CC0 1.0): idle, run, the jump's air loop and
its landing. No texture is read. It is shaded as the engine's ghost
(`metaverse-engine/render/ghost.coax`): see-through, cool, bright where its
surface turns away from the eye, blended over what is behind it and hidden by
what is in front. It casts a soft shadow and parts the grass round its feet.
It is never the private SPLICEBOTS pack.

Where it comes from, and what is the engine's:

| Piece | Source |
| --- | --- |
| Mesh | The asset registry's `fantasy` package on this Mac (`~/.openagents/assets/originals/9415c92b.../source/Exports/glTF (Godot-Unreal)/Outfits/Male_Ranger.gltf`), read with the pinned tree's glTF reader (`metaverse/tools/humans/ph_gltf.coax`) |
| Rig and clips | The `people` set of the pinned tree's `everglade/model/rigs.vrg` (the Quaternius Universal Base Characters' male skeleton; clips `idle`, `run`, `jump_loop`, `jump_land`) |
| Pose arithmetic | The engine's `metaverse-engine/anim/pose.coax` (slerp, the cross-fade, the chain, the palette) |
| Walk and jump | The app's rules: `metaverse/splice-bots/rules/walk.coax`, the same functions `coax/her.coax` calls (`sr_walk_step`, `sr_jump_press`, `sr_jump_advance`), with their own tuning |
| Shading | The engine's ghost (`metaverse-engine/render/ghost.coax`) in the engine's scene pass |
| Collision | The engine's (`physics/query.coax`, `physics/collide.coax`) |
| Written here | The baker (`tools/ghost-bake.coax`), the baked file's reader, clip choice and CPU skinning (`web/ghost.coax`) |

That last row is still a stand-in: the model's format is this page's own.

The baker rebinds each vertex onto the rig's rest pose, thins the mesh by
vertex clustering (22 mm cells: 24,873 vertices and 26,982 triangles become
4,578 and 11,603), smooths the normals, and samples the clips 24 times a
second. The result is 244 KB, written as base-64 text in
`web/ghost-data.coax` (351 KB of source) and compiled into `hud.wasm`. The
mesh is skinned on the CPU each frame into a store of 4,578 vertices (110 KB
a frame to the GPU); its triangles are a second store, sent once. To bake
again:

```
vendor/openagents/coax/web/tool.sh tools/ghost-bake.coax OUTFIT.gltf vendor/openagents/everglade/model/rigs.vrg web/ghost-data.coax [CELL_MM]
```

The figure is 1.86 m to the top of its hood.

Rules, in `web/hud.coax` with checks. The numbers are the app's
(`SrWalkTuning`), not copies of them:

- Runs 6.9 m/s, with the app's acceleration and braking. No sprint, no
  sneak, no walk key.
- Space jumps 1.67 m; let go early and it is a shorter hop. Twice quickly:
  fly mode ("Fly mode"). The side-scroller alone keeps the air jump (3.90 m
  in all). No jetpack, no board.
- W is ahead (away from the camera, wherever it has turned), S back, A and D
  sideways. Diagonals are not faster.
- It turns to face the way it moves, quickly but not at once.
- It moves only in the world view, and only while the keys are its own (not
  while you type). Space does not scroll the page. The world opens with the
  caret in the chat box: Escape gives the keys to the character, Enter puts
  the caret back.
- A block is solid, by the engine's collision: a wall when its top is more
  than a step (0.55 m, the engine's) above the feet, ground to land on and
  stand on, an edge to walk off and fall from, a ceiling to a jump under it,
  and one put where the character stands pushes it out. The body is 0.3 m in
  radius and 1.8 m tall.
- Its clip: the air loop off the ground, the landing clip after landing on
  the spot, the run by the ground it covers (4.4 m a loop), else idle. A
  change of clip blends over a tenth to a quarter of a second.
- It is drawn only while the camera is down from the home view; it fades
  with the camera's rise.

The blocks are boxes of the engine's architecture kit in its smooth pale
stone, and the ball is the kit's round with a dark band so its roll shows,
lit and shadowed like everything else. They are rigid bodies ("Things that
fall and can be pushed", below): they hang a little above their places while
the camera comes down, then fall.

Touch: the engine's thumbs, as in Everglade ("The camera", Controls). Checked
in Coax and tried with made-up touches in a desktop browser; not tried on a
phone.

## The router

The idea is OpenAgents' composable general agent (`docs/transcripts/289.md`
in the pinned tree): one agent, every message mapped to a route, each route a
thing the agent can do, and a plain "can't do that yet, want it built?" when
no route fits. GameBuilder's tree is the games-focused one.

| Route | Leaves | What it is for |
| --- | --- | --- |
| `play` | `play/scrappers`, `play/party` | Start or join a game |
| `build` | `build/level`, `build/block`, `build/mode` | Make or change a level, a thing in it, a game mode |
| `games` | `games/scrappers`, `games/splicebots`, `games/rules` | Questions about the games |
| `world` | `world/set`, `world/move`, `world/where` | This grid world and the character in it; `world/set` changes a property ("Properties") or says what can be changed |
| `account` | `account/who`, `account/logout` | The login |
| `make` | `make/capability` | The last resort: something no route and no property covers. One line that GameBuilder cannot do it yet, and an offer to start a build agent on it; never a question about what it should do, never a claim that it is being worked on. The request is kept ("asked for, not here yet" in the world's settings), and the offer is pending until the next message |
| `offer` | `offer/accept`, `offer/decline` | Only while an offer is pending, and only at the select step: the next message takes the offer up (a build agent starts) or turns it down ("ok, i won't start it."). Any other route lets it lapse |
| `chat` | | Greetings and anything else |

How a message is routed today:

1. The select step ("Properties", below) chooses the route and the
   properties and things the message is about, from the tree and the
   registry's index. The decide button skips it (its route is build/block).
2. The page sends the conversation with the brief's core in front of your
   first message (`CH_CORE`, `CH_TREE`, `CH_CORE_END` in `web/chat.coax`): who
   GameBuilder is, the tree, the world's words, and never to say what model it
   is. With your newest message go only the parts the chosen route needs
   (building, the decide button's note, setting properties with the chosen
   properties in full, the other facts) and the world as it stands. With no
   route chosen, every part goes.
3. The model picks the route and writes it as the answer's first line,
   `route: build/block`, then the answer.
4. The page takes that line off, checks the route against the tree (a known
   root, at most one leaf), and keeps the route with the answer. The page shows two
   speakers only: "you" and "agent". An answer with no route, or one the tree
   lacks, is shown whole.

The page does no routing of its own and matches no keywords: the model
selects, the page only reads the route's name. While one is in flight the
page says "routing".

`build/block` has hands: it changes what stands in the world ("Blocks",
below), and `world/set` changes settings ("Properties"): a `set` or `reset`
plan line is made under a build or a world route, any other plan line only
under a build route. Every other route answers in words; `play`, whole levels
and game modes say they are in the app for now.

What powers it: Gemini 4 Hydrogen through OpenAgents' account chat
(`POST /api/chat/complete/v1`, `coax/web/api/account-chat.c` in the pinned
tree). The page sends the conversation as it stands and gets one whole answer
back; nothing streams. The route choice is meant to move to OpenAgents'
decision model (stage 3), which answers typed questions for a fraction of the
cost; the tree stays the same.

Request, `openagents.chat.thread.v1`:

```json
{"schema":"openagents.chat.thread.v1","id":"m_0123456789abcdef0123","revision":0,
 "messages":[{"role":"user","content":"<the brief>\n\nmessage: hello"}]}
```

Answer: a Chat Completions object; the text is `choices[0].message.content`.

Rules, in `web/chat.coax` with checks:

- At most the last 12 messages are sent, each at most 1,200 bytes typed. The
  brief goes with the first of them, once.
- One question at a time: a second Enter while one is in flight does nothing.
- What the service refuses is shown in its own words. Any logged-in account
  is answered (since 2026-11-17, issue 15); what refuses one is a limit, said
  with its number: "this account's 500 requests for today are used up",
  "this account's $2.00 spend for today is used up". Nobody logged in sends
  nothing: the page says "log in or create an account to talk to the agent."
- The conversation is not kept: a reload starts a new one. (The world is: "Kept in this browser".)
- The session is the login's cookie. The page holds no key and no token.

OpenAgents' general system prompt still goes first on the service's side; the
brief rides in the first message because that route takes no system message
from a caller.

## Properties

Everything about the world a person can tweak is a **property**: a row of the
engine's registry (`metaverse-engine/state/tunables.coax` in the pinned tree;
this world's rows are `metaverse-engine/state/world-tunables.coax`). A row has
a path, a kind (on or off, a whole number, a number with a step, or one of a
few names), a range, a default, a unit, a few words and one line of what it
does. GameBuilder's own part is small: `web/props.coax` (what this world
starts from, set and reset by a path as written, the refusal's words),
`web/props-panel.coax` (the panels) and where each rule reads its value
(`web/hud.coax`).

| Path | What it drives |
| --- | --- |
| `world.gravity` | the bodies' scene gravity and the walk rules' (0 to 5 x) |
| `world.time_scale` | the world's steps: slow motion to fast forward (0.25 to 2 x) |
| `world.wind` | how far the grass bends (0 to 2 x) |
| `world.time_of_day` | where the sunrise stops: night, dawn, sunrise, morning, day |
| `world.haze` | the sky's haze a metre (0 to 3 x) |
| `world.physics` | whether the things are simulated: off, every thing is held where it is (none falls, rolls or is pushed; you still walk and jump on them); on again, they fall from there. "disable physics" sets it off |
| `player.air_jumps` | jumps in the air: 0 is no double jump (0 to 2) |
| `player.fly` | whether Space twice may start flying; off, the second press is an air jump |
| `player.run_speed` | the walk rules' speed, m/s |
| `player.jump_height` | metres a jump rises at normal gravity |
| `player.push_strength` | how hard the character pushes things (0 to 3 x) |
| `camera.view` | the camera menu's camera: orbit, follow, first_person, top_down |
| `camera.distance` | how far back the orbit stands, m |
| `camera.fov` | the lens, degrees |
| `thing.bounce`, `thing.friction`, `thing.density`, `thing.fixed` | a thing's material (`bd_material`): set for one thing, a kind or every thing |
| `grass.height`, `grass.density` | a blade's height, m; how thick the meadow grows |

**One edit.** A property is set with one plan line, from the model or the
panels alike: `+ set PATH VALUE` and `+ reset PATH`. A thing's path names the
thing first: `b3.bounce` is thing b3's, `balls.bounce` every ball's,
`blocks.bounce` every block's, `things.bounce` everything's (a thing's own
value comes before its kind's, before every thing's). VALUE is a number, `on`
or `off`, or a name. The line is checked against the registry only after the
route was chosen; a line that is refused (no such property, a value out of
range or of the wrong kind, a thing that is not there) changes nothing, and
the answer says why in plain words ("world.gravity takes 0 to 5, not 9.").
`+ gravity N` is the same door as `+ set world.gravity N`. A thing removed
takes what was set for it alone; a cleared world, every single thing's.

**What the world is told.** After the hotbar's line (`.gs-state`, message
113) come the settings that differ, one a line, in the engine's own text
(`world.gravity=0.3`, `thing.bounce@4=0.9`); the HUD loads that map whole and
reads each value where its rule is. The model is told the same as
`settings changed: world.gravity 0.3, b4.bounce 0.9`.

**Two steps: select, then act.** Both go through `/api/chat/complete/v1`.

1. *Select.* A thread of its own, one message: a short brief, the route tree,
   the registry's index (group by group, each row's name and a few words: no
   ranges, no values), the things by id and kind, the one selected, a clip of
   the turn before, and your newest message. The answer is three strict
   lines: `route: NAME`, `props: PATH, PATH` and `things: b3`. The page checks
   each choice against what was listed (a route the tree has, a path the
   registry has, a thing that stands here) and drops anything else. Nothing on
   the page matches words in what you typed; it reads a typed answer's
   bounded fields only.
2. *Act.* The conversation, the brief's core, the parts the chosen route needs,
   and the chosen properties in full ("world.gravity: 0 to 5 x, step 0.1,
   default 1, now 1. how hard everything falls"), for each chosen thing or,
   with none, each kind. With no property chosen the index goes instead, so
   "what can I change here?" has an answer. Never every property's detail.

A registry too long to index whole (over `CH_INDEX`, 6,000 bytes, about 1,500
tokens) is chosen from in two select calls: its groups first (name and count),
then the chosen groups' rows. Measured with the page's own request builders:

| | 19 rows (today) | 30 rows | 500 rows (synthetic) |
| --- | --- | --- | --- |
| index | 632 B | 892 B | 11,840 B (too long: groups instead) |
| every row in full | 2,688 B | 3,899 B | 55,550 B |
| select call | 2,590 B | 2,852 B | 1,760 B (groups), 2,901 B (two groups' rows) |
| act call, first message | 3,653 B | about the same | about the same |
| one call with every part (the old way) | 7,186 B | grows with every row | over 60 KB |

The old single brief was 5,022 bytes before any world; the act call is now
smaller than that with the chosen rows in full, and does not grow with the
registry. A token is about four bytes.

**Ready for OpenAgents' decision model.** The select step is a typed choice:
a fixed label set (the routes), a bounded list of paths and ids, and a strict
three-line answer. That is the shape OpenAgents' decision model answers, for a
fraction of the cost. The page cannot call it today: the decision door needs a
device key, and the web may only reach `/api/chat/complete/v1` and the login
routes. What it needs on the server: a session-cookie route (say
`POST /api/decide/route/v1`) that takes the message, the label set and the
index, calls the decision model with the server's own key, and returns the
same three fields; the page then sends its select request there instead.

**What `make` does now.** It is the last resort. A clear request a property
covers is done at once in a short line ("disable double jump" sets
`player.air_jumps 0`, "disable physics" sets `world.physics off`), never
answered with a question about what the capability should do. Something
nothing covers gets one line and an offer to start a build agent on it, and
the request is kept on the page (the newest six, until logout) under "asked
for, not here yet" in the world's settings.

**The offer, and the build agent.** The offer is pending until the next
message. That message's select step is told of it (`CH_OFFER_A`/`_B` in
`web/chat.coax`) and may choose `offer/accept` or `offer/decline`; those two
are checked exactly and exist only while an offer is pending. "yes" after an
offer takes it up: no answer is asked for, and a build agent starts on the
request in your words (docs/agents.md, "Built: the first build agent"). The
chat says so with its cap in words ("it gets one model turn: at most 2,048
tokens of answer and about 6 minutes, and it costs you nothing for now") and
shows a row with its state: starting, waiting for a runner, working, done,
or failed with the reason. The agent is one durable turn on OpenAgents'
hosted runner and has no tools: when it is done its answer is kept as the
agent's and its plan lines are made as any build answer's are, checked
against the registry and the limits. The page reads it every two seconds and
gives up after seven minutes, saying so. One at a time; logging out forgets
it.

**Adding a property.** A new tweakable thing is a registry property, never a
new verb:

1. Add a row to the engine's table (`world-tunables.coax`: its path, kind,
   range, default, unit, few words and line) and an index constant, with its
   check (the table's own check proves it well made).
2. Read it where its rule is (`web/hud.coax`: `hud_tuning`, `hud_tune`,
   `hud_stuff`, or the frame) with `pr_value` or `pr_thing`, and add a check
   that the setting changes the behaviour.
3. Nothing else: the panels, the model's index and detail, `set` and `reset`,
   and the wire to the HUD all follow from the row.

Checks: `web/props.coax`, `web/blocks.coax` ("a property is set and put
back"), `web/chat.coax` (the select step; recorded conversations with stubbed
answers: "disable double jump", "turn off flying", "make gravity half", "make
the ball bouncier", "make b2 really heavy", "what can i change here?", an
unknown request), `web/hud.coax` ("disable double jump": the jump rule refuses
an air jump; the properties reaching the things, the character, time, the sky,
the grass and the camera) and `web/page.coax` (the panels).

## Blocks

The world holds up to 1,000 things, blocks (boxes) and balls alike, each with
a name (`b1`, `b2`, ...), a place, a size and how high its bottom is. It
opens with three blocks (a low platform, a pillar and a wall) and a ball.
Lengths are grid squares to two decimals; x is to the right, z is ahead, y
is up. Rules and checks are in `web/blocks.coax`.

You ask in the chat ("put a wall in front of me", "make b2 taller", "stairs
up to the platform", "clear it"). How it happens:

1. Each message goes with the world as it stands: where you are and facing
   (the HUD tells the page through a hidden field), and every block.
2. The model routes it `build/block`, says what it did in a line, and ends
   its answer with one plan line per edit:

   ```
   + place X Z W D H [Y] [rest]
   + ball X Z R [Y] [rest]
   + size ID W D H
   + goes ID DISTANCE SECONDS
   + move ID X Z [Y]
   + remove ID
   + clear
   ```
3. The page reads those lines, only under a `build` route, and makes each
   edit that is one (known thing, sides 0.25 to 12, a ball's radius 0.15 to
   1.5, within 200 squares of the middle). A line that is not an edit changes nothing. The plan lines are
   taken out of the words you see and go back with the thread.
4. The page writes the list into a hidden element; the glue hands it to the
   HUD (message 105), where each thing is a rigid body of the engine's.
5. The HUD tells the page where things came to rest (its action 110, into a
   second hidden field), so the world the model is told with your next
   message is where things are, not where they were put.

## Things that fall and can be pushed

Every block and ball is a rigid body in the engine's physics
(`metaverse-engine/physics/sim/bodies.coax`, written for this page and
landed in the engine; `web/hud.coax` only uses it). Nothing about the
physics is this page's own except the numbers below.

- **What a world opens with falls from the sky a little.** Each of those
  things appears 3 m above its place and falls (each 0.6 m higher than the
  last, so they land in turn). Nothing made or moved afterwards does
  ("Placing modes"): it appears at its place. A falling thing lands, bounces a few centimetres at
  most (a ball more), comes to rest and sleeps. It never goes into the
  ground. One put inside another is pushed apart by the engine's positional
  solve and gains no speed by it.
- **Weight.** Running into a thing pushes it, by its weight. The character
  pushes a standing thing with 12 kN, less the faster the thing is already
  going, to nothing at its own 6.9 m/s (the engine's `bd_push_share`, curve
  12); friction is half a thing's weight. So:

  | Thing | Mass | Goes at |
  | --- | --- | --- |
  | A block 0.6 m cubed | 43 kg | over 4 m/s: you run along with it |
  | A cubic metre | 200 kg | about 2.4 m/s |
  | The starting pillar, 1 by 1 by 3 | 600 kg | about 1 m/s |
  | The starting platform, 2 by 2 by 1 | 800 kg | about 0.7 m/s |
  | The starting wall, 6 by 1 by 2 | 2.4 t | centimetres in three seconds |
  | A wall 12 m long | 4.8 t | millimetres |

  Pushed off its middle a block turns. The character is never moved by the
  physics: it is stopped by things as before, stands on them and jumps onto
  them, steps onto anything up to 0.55 m high instead of pushing it, and a
  block that lands on it pushes it aside without throwing it.
- **The ball.** 0.6 m across, 4.5 kg. Running into it kicks it away at up to
  1.55 times your speed; it bounces (0.55), rolls, and loses about 1.4 m/s
  each second to rolling resistance, so it stops.
- **How things stand.** A block resting on any face is told to the page as
  the box it now is (a pillar pushed over is 1 high and 3 long), with the
  degrees it has turned; one left leaning is told as `tipped`. A `+ size`
  from the chat resizes the body where it rests, its bottom staying where
  it is; a `+ move` puts it upright at its new place, not lifted first.

The numbers (`web/hud.coax`): blocks 200 kg a cubic metre, friction 0.5,
restitution 0.15; the ball 40 kg a cubic metre, friction 0.6, restitution
0.55, rolling resistance 0.06 m; the push 12 kN, to nothing at 6.9 m/s,
curve 12; the entrance's drop 3 m. The engine's: 60 steps a second from an
accumulator (the frame rate does not enter), 10 solver passes, sleep after
half a second still.

The list message (105) is a count, then a line a thing: `ID KIND MOVES
SIZES X Y Z W H D` (KIND 0 a block, 1 a ball; lengths in hundredths), and
then how it is made when it is new: nothing more (one the world opened
with: it falls in), ` 1` (set down at rest, asleep) or ` 2` (at its place,
awake). The
HUD takes a thing's place only when it is new or MOVES has changed, and its
size only when SIZES has: otherwise where a thing is is the engine's to
say. A line that cannot be read ends the list; a line that reads but is no
thing is passed over; a bottom under the floor is the floor. The rest report (110) is `ID X Y Z W H D TURN TIP` for each thing
that has come to rest somewhere the page has not been told, at most twice a
second, read from the engine's history of the last step.

Cost, measured natively on this Mac (M-series, one core) with 48 boxes and
a ball falling into a heap and being walked through: 0.06 ms a step on
average and 0.31 ms at worst while things move, 0.003 ms when all sleep
(with 48 bodies; at the 1,000 cap see "Limits": 0.32 ms a step all asleep).
In the browser the page still draws at the display's rate with the
starting scene; the step's own time in WebAssembly was not measured.

### Ready for more than one player

The owner wants several players knocking the boxes around. No network is
built, but the physics is in the shape a room needs (OpenAgents'
`docs/research/2026-11-17-shared-physics-multiplayer-plan.md`, stage 0):

- **Repeatable.** Bodies run in the order of the page's ids, time enters
  only as whole steps, and a step calls no library function (its turns use
  the engine's strict sine and cosine). The engine checks that a recorded
  run of inputs replays into a fresh scene with the same state hash every
  step. Not yet run: the same inputs natively and in a browser, or on two
  machines.
- **Shared and local.** Every block and ball here is a shared body. The
  engine also has local bodies (decoration a page makes for itself) that
  shared bodies push but that never move a shared one.
- **A snapshot.** The engine writes the shared bodies as a versioned header
  and one 112-byte record a body (`bd_snapshot_write`): what a room would
  send. The page does not send or read one yet.
- **A history.** The HUD keeps the engine's history of the last 64 steps
  (`Bd_History`); the rest report reads from it. The engine can rewind to
  one of the newest 16 steps and step forward again to the same bits.
- **Movers.** The character is mover 1 of the engine's movers. A second
  player is a second mover with its own id; the engine takes them in id
  order and checks two of them pushing one block from two sides.

Next, in order: a room that steps the bodies and sends snapshots, pages
that draw the things from snapshots a little in the past (interpolation);
then each page predicting the island of things its own character touches
and rewinding when a snapshot disagrees.

**The ✦ button** beside the chat box is Decide: it sends `decide` for you,
and the model picks one edit itself and makes it. In the app the same button
asks OpenAgents' decision model (Gemini Decision 1) six typed questions,
signed with the device's key; a browser has no such key, so here the same
model that routes also decides, until the decision door takes the login's
cookie (stage 3).

Things are saved (in this browser, as the app's level file: "Kept in this
browser") and every edit can be undone ("Undo and redo"), since 2026-11-17.
Not yet: the chat cannot turn a thing (the physics can), and only boxes and
balls. The list in the page's memory is still the page's own
(`web/blocks.coax`); what is kept of it is an engine world document, each
thing an entity of the engine's World whose kind is a row of the content
shape table (`web/level.coax`, the app's `coax/level.coax` form). Building
the colliders and the boxes from that document directly, and keeping it on an
account, is stage 4.

## Stages

**Stage 1 (this change).** The flow above: land on the grid, run, talk.

**Stage 2 (begun: blocks).** Use the world-aware route,
`POST /api/chat/world/complete/v1`. It takes the world as the model reads it
(`world.turn.v1`) beside the thread, and the model answers with talk or a
typed plan (`world.plan.v1`: place, move, turn, size, remove). The web world
then needs things to place: blocks on the grid, kept in the HUD, described to
the model each turn, changed by its plan with undo. This is where "agent
mode" starts to mean something.

**Stage 3: the decision model beside it.** GameBuilder's Decide already asks
OpenAgents' decision model six multiple-choice questions for one edit
(`coax/decide.coax`). Use it for the cheap, frequent choices (which route,
which block, which way, how far) and Gemini for the words and the plan. Decide is signed with a device key today; on the web it
needs a route that takes the login's cookie.

**Stage 4: what you build here is a level.** Save the web world as the same
file the apps use (`coax/level.coax`: a `world.doc.v1` whose kinds name rows
of an `aethyr.content.v1` table of shapes), so a level made on the web opens
in the app. The app already answers an agent with that file inside a
`world.state.v1`, and takes `world.edit.v1` batches (`coax/agent.coax`).

**Later: `make` builds.** A capability nobody has yet is written, tested
against examples and added to the tree as a route, as in episode 289.

**Later: Claude as an optional connection**, for someone who has Claude Code
on their own computer, by the routes in `docs/agent-mode-audit.md`.

## What production needs

The browser host only talks to its own site, so gamebuilder.org's load
balancer hands these paths to the OpenAgents API service:

- `/api/auth/account`, `/api/auth/email/start`, `/api/auth/email/verify`,
  `/api/auth/logout`: the login. The login service knows gamebuilder.org
  as a second site (`OA_API_SITE_ORIGINS`).
- `/api/chat/complete/v1`: the chat. It needs a login and has no site check
  of its own.
- `/api/gamebuilder/agents/v1` (POST, a start), `/api/gamebuilder/agent/v1`
  and `/api/gamebuilder/agent/events/v1` (GET, `?id=`): the build agent. A
  login, any account's (at most 32 turns queued or running on an account);
  a start from another site is refused. The
  service runs it on its hosted runner (openagents
  `docs/protocol/gamebuilder-agent-v1.md`).

## Status

**A guest, kept in this browser, undo and redo (issue 13, 2026-11-17).** Built
and checked in Coax (`web/level.coax`, `web/keep.coax`, `web/undo.coax`, and
`web/page.coax`'s and `web/chat.coax`'s checks named in "A guest"); the level
file is checked against the app's own reader and writer
(`tools/game-types-check.coax`). Seen in headless Chrome with WebGPU on the
dev server, at 1440 by 900 and at a phone's 390 by 844 with touch, with no
login: `/play` shows the picker; racing picked; a boost and two cones placed
with the bar's tools and a click on the track; a reload shows the same three
things in racing with no picker (class `gs v2 g7 still`, IndexedDB holding
`game`, `level-8`, `settings-8`); undo with no history says its words; a cone
placed, undone and redone by Ctrl+Z and Ctrl+Shift+Z (the bar's items on the
phone); decide as a guest says the words and makes no request; "log in" opens
over the world. Not seen: a real login over a built world and a real phone
(the dev server has no login service; `NEEDS_OWNER.md`).

Built and checked in Coax: the views and addresses, the camera's poses and
easing, the character's movement, jump and air jump, its model (loads, every
clip keeps it a person's size), the blocks' list, their walls, tops and edges
by the engine's collision, the grass's spread, the scene's draws and stores,
the report of where the character is, the chat's request and answer handling;
the things as rigid bodies (a drop coming to rest and sleeping, a stack
staying stacked, the push by weight, the ball's kick and stop, overlap
parting, a block landing on the character, turned and tipped blocks, 48
things for half a minute, the list's moves and sizes, the rest reports).
Seen in a browser on this Mac (headless Chrome with WebGPU on the dev
server): the home page and the login as they were, the camera's descent, the
grass spreading from the feet to the horizon, the ghost standing and running
and the grass parted round its feet, the three starting blocks and their
shadows, the caret in the chat box and Escape taking it out, with the login
stood in for by hand (the dev server has no API); the sunrise as stills from
0.3 to 13 seconds, and the day sky with the camera turned a quarter, half and
three quarters round with the Left arrow; the
starting blocks and the ball in the air just after the login and at rest
after it, the character run up against the wall (the page then told the
wall had moved 2 cm), and a block and a ball placed by a stood-in chat
answer, landed and bouncing. Not seen: a thing being pushed in motion (only
stills before and after), the ball kicked, a block tipping, a stack; a jump, landing on a block's top and a block stopping a run (all
checked in Coax, not looked at); the sunrise in motion (only stills), on a
phone, or in a browser other than headless Chrome.

The orbit camera, seen in a browser on this Mac (the Claude app's browser
pane with WebGPU, 1280 by 800): the descent ending behind the character, a
drag turning it, the tilt at both limits, the zoom out to 12 m and in to the
closest, first person with the body gone, W running away from a turned
camera, the eye pulled in front of a block behind it, a drag and a wheel on
the chat moving nothing, and made-up touches turning the view and running the
character with the stick drawn. That pane drew two frames a second, so how
the camera feels in motion (its easing, the follow, the descent) was not
watched; its numbers are Everglade's.

A faint frames-a-second number sits in the world's bottom right corner; the page asks for every frame while the world is up so it counts the frames really drawn (frames run at once after a press, under 2 ms apart, are not counted).

Speed and size: the page draws at the display's 120 frames a second at 1440
by 900 and at 2880 by 1800 on this Mac (the frame time itself was not
measured, and nothing was measured on a phone: the field is not yet thinned
for one). For the world a browser downloads (release build, gzip) the world's
module, 205 KB (423 KB unpacked, most of it the character), its shaders,
16 KB, and the page's module, 33 KB, beside the host's scripts.

Not done: a real login or a real routed answer seen on gamebuilder.org, so
how well the model keeps to the `route:` line is unmeasured; touch on a real
phone; the camera's feel at full frame rate; a real held pointer (the browser pane refuses the lock); the hotbar's
tools by touch (checked in Coax only); handles on a selected thing; the game types'
entrances watched in motion (they were seen as stills at 20 frames a second);
the character's model on the engine's own asset path; anything in stages 2
to 4.

### Racing on a touch screen

A left thumb is the car's throttle/brake and steering: up drives, down brakes
and reverses, and sideways steers. Holding a still right thumb applies the
handbrake. These feed the same engine vehicle input as the keyboard.
