# The app shell and the side

**Since 2026-11-13** (GitHub #13471, #13483). Someone logged in sees one side
down the left of the site:

- A head with the mark and the name, a link home, and Hide the side (the plain
  sidebar icon), which puts the side away. The same icon brings it back.
- Three folds: Chats, Projects and Worlds. Each heading opens and shuts its list
  in place; it never leaves the page. This browser keeps which folds are shut
  (`dom_keep`, `shut=N`, bit 1 Chats, 2 Projects, 4 Worlds). At the heading's
  right a + makes a new one:
  - **Chats** holds the chat's sessions under section heads (Running, Claude
    Code, Recent). Its + starts a new chat.
  - **Projects** holds the projects the person is in, each opening
    `/projects/SLUG`. Its + opens `/projects#new`, where a new project is
    dropped.
  - **Worlds** holds the worlds they may edit, Splice Bots first, each opening its
    editor (`/worlds/splice-bots?edit`). Its + opens `/worlds/new`.
- At the foot, who is logged in, a link to Settings, and Log out.

It is 300 px wide. Below 760 px it is hidden, as the chat's sidebar was. Each
list shows at most 24 rows, as many as its API sends.

## The pieces

- **Drawing:** the kit's side pieces in `components/kit/frames.coax`: `k_side`,
  `k_side_head`, `k_side_folds`, `k_fold` (a heading that opens and shuts, with
  its +), `k_side_mark`, `k_sub_place`, `k_who`, `k_side_open`, styled by
  `mono_css` (`components/skin/mono.coax`). `/components` shows them.
- **Rules:** `apps/openagents-web/site-side.coax`: which place an address is,
  which addresses the shell draws itself, the name at the foot (the first email
  from `/api/auth/account?as=lines`, else a short npub), the worlds under
  Worlds and the sections. Plain functions with their checks.
- **Worlds:** `GET /api/worlds` (`coax/web/api/worlds.c`) lists the worlds the
  asker may edit, one line each, `world ID NAME`, Splice Bots first. Who may is the
  rule a write to the world is held to (`wr_may`: a site admin, or a realm's
  owner and admins). Someone logged out gets an empty list.
- **Projects:** `GET /api/projects` (`coax/web/api/worlds.c`) lists the
  projects the asker is in, one line each, `project SLUG NAME`, by name: a
  private project whose members include their key, and a published project
  they dropped. Someone logged out gets an empty list.
- **The shell:** `apps/openagents-web/chat-web.coax`. It draws the side and,
  beside it, the chat, Settings, or another module's page
  held in a frame under its loading screen.

## How navigation keeps the side

**Since 2026-11-13** (GitHub #13487). The side never unloads while you move
between the pages it reaches: the home page, the chat, Settings, Projects, the
worlds' list and a world's editor. Moving between them changes only the room
right of the side.

The side and the chat's session list are one thing (the list is the side's
middle), so the shell is the chat's own module, `chat-web`, at the top of the
window. It draws some pages itself and holds the others in a frame.

- **Pages it draws itself:** the home page (`/`, a new chat, so the first
  thing you can do is type; the address stays `/`), the chat (`/chat`) and
  Settings (`/settings`, `/settings/SECTION`). Moving between them changes the
  module's drawing in place.
- **Pages it holds:** every other page the side reaches is another module's:
  Projects and each project's page (`/projects/...`, the GPU), the worlds'
  list (`/worlds`, `worlds-web`), a world's editor (`/worlds/ID?edit`, the
  world's GPU module and the editor's shell) and the older settings pages
  (`/settings/security` and the rest). `ss_held` in `site-side.coax` says which.
  The shell draws the side and, beside it, that page in a frame
  (`k_held`, `components/kit/listing.coax`) at the same address. The page in
  the frame runs as itself.
- **One frame for each page moved to.** A move to another held page makes a
  new frame (its id ends in a count), and a move away takes the old frame off
  the page. The browser then ends everything the page ran: its canvas, its GPU
  device, its sockets and workers, its listeners and its timers. Nothing is
  torn down by hand, so nothing is missed. A held page plays a world in the
  window, not in the frame: a world without `?edit` is a page of its own,
  with no side.
- **The loading screen.** Until the held page has drawn, the room right of the
  side shows the kit's loading screen (`k_loading`, styled by
  `mono_held_css`): a framed panel in the centre with the page's name, a line
  and what is loading. A world's page says how far it has come (its own
  loading screen's words, `ENGINE`, `SHADERS 7/9`, `ASSETS 3.1 / 12 MB`, and
  the percentage), and the line fills; a page that doesn't say runs the line.
  The screen goes when the page has drawn: a world's loading screen is gone,
  or a page without one has drawn its first frame.
- **Addresses.** A press on a link in the side, or in Settings, is the
  module's (`cw_keeps`): when the shell draws or holds the address it pushes
  it with the History API and draws what is there, and the browser does not
  follow the link. A link inside a held page (a world's Edit button, a
  project), or the page's own move (a GPU page's action 1), goes to the shell
  the same way (`shell.js`, `shellGo`). Any other address (a world to play,
  `/login`, another site) is a page load of the whole window. Back and Forward
  tell the module the address with its query (event 13) and it draws that.
- **A full load of any address works.** `/` and `/chat` load the shell's
  module. A held page names itself on its host's script: `data-shell="page"`
  (always) or `data-shell="edit"` (a world's page, when the address has
  `?edit`; `realms.ts` writes it). Opened at the top of the window, such a page
  becomes the shell (`shellTakes`): it empties itself, opens the chat's module
  there as the page's own shell, and its own module never starts; the shell
  then holds the same address in its frame. A held page that is opened inside
  some other frame, or a world played, starts as before.
- **A load of the whole window** (a world to play, `/login`) rebuilds the
  side; its state is kept in the browser (whether it is put away and which
  folds are shut, through `dom_keep`). There is no cross-document view
  transition: the skin's style comes with the module, after the new page is
  first shown, so the browser would abort it ("ViewTransition opt-in
  disabled").

The glue is `coax/web/shell.js`, imported by `host.js` and `dom-host.js`. It
carries words across the frame and decides no address:

| Word | From | What |
| --- | --- | --- |
| event 13 | the window | the address, with its query, as the shell starts and after Back or Forward |
| event 14 | the held page | how far it has come (`PERCENT WHAT`), or that it has drawn (slot 1) |
| event 15 | the held page | an address it goes to: the shell keeps it or the window goes there |

A word from any frame but the one shown now is not heard. The window's title is
the held page's while one is held.

## The world editor

The editor (`/worlds/ID?edit`) is a held page: the shell's side is to its left.
The editor's shell (`metaverse-engine/editor/shell.coax`) is told so (event 14,
`ES_HELD`) and draws no column for the side. The editor's own panes are the
strip across the top, one side on the right (what is selected, then the world's
things and actors; the code, the assets or the files take its place while one
is open) and the agent's pane at the foot, where the chat shows without a side.
The scene's canvas is the room they leave, inside the frame.

Opened anywhere but the shell's frame (a page that holds the editor in a frame
of its own), the editor still shows the site's side itself: the editor's shell
gives it a column, `k_dock_side`, and the page's host (`coax/web/host.js`)
opens a second copy of the chat's module there with `path=/side`, the side
alone, every place in it another page.

## What is left

- **The Projects and Worlds lists** live in the side's folds. A project in the
  Projects fold and the Projects + (`/projects#new`, the GPU galaxy's drop) are
  held pages; so is `/worlds`, the kit page `worlds-web`.
- **Published projects' pages** (`/projects/SLUG` from the API,
  `coax/web/api/migrations/0038_project_publish.sql`) have no `data-shell`: a
  full load of one shows no side. Moving to one from inside the shell holds it.
- **The login** (`/login`) is a page of its own, with no side.
- **A new page drawn by the browser** joins the shell by adding a view to the
  shell module (as Settings is) or, when it is large, as a held page: its
  module's own page with `data-shell="page"`, and its address in `ss_held`.
- **Resizing the side by dragging** is not built; the width is fixed at 300 px.

## Settings

**Since 2026-11-13** (GitHub #13480). `/settings` and `/settings/SECTION` are a
page of the shell, so the side stays put: the side's foot (who is logged in)
opens `/settings`, and the chat's gear opens `/settings/models`. Beside the side
is a list of sections, the one shown bright on the raised surface, and the
section's rows in framed groups. A row is a name, what it is or does under it,
and what changes it at its right: a switch that says On or Off, a button, a
select, a link or a value. A press on a section pushes its address, so Back and
Forward move between sections; a direct load of any of them works
(`settings/index.html` and one `index.html` per section load the shell's module).

| Section | Address | What it holds |
| --- | --- | --- |
| Account | `/settings`, `/settings/account` | Who is logged in, every email (masked) and key (a short npub) from `GET /api/auth/account?as=lines`, admin, Log out, and links to the older settings pages |
| Agents and models | `/settings/models` | The Claude account (key or token, Connect, Disconnect), This machine (Coder's agents, ready or why not, and the switch), the agent and model a new chat starts with, each local agent's model |
| Appearance | `/settings/appearance` | The side's switch; motion follows the system's setting |
| Limits | `/settings/limits` | The chat's own limits and every login limit from `GET /api/auth/limits?as=lines`, and a link to `/docs/limits` |
| About | `/settings/about` | The release's commit and build day from `/build/about.txt` (the release build writes it; a development build says it has none), the source code and the docs |

What the page keeps is kept in this browser with the side's state (`dom_keep`:
`local=off`, `side=closed`, `lmA=K`, and `m=K` for a new chat's model), except
the Claude account, which is sealed on the account.

- **Drawing:** the settings set in `components/kit/settings.coax` (`k_set`,
  `k_set_nav`, `k_set_body`, `k_set_group`, `k_set_row`, `k_set_value`,
  `k_switch`), styled by `mono_set_css`. `/components` shows them.
- **Rules and the sections that need nothing of the chat:**
  `apps/openagents-web/settings-web.coax` (addresses, the account's, limits' and
  release's lines; Account, Limits, About).
- **The shell's own sections:** `apps/openagents-web/chat/chat-settings.coax`
  (Agents and models, Appearance, the presses), loaded by `chat-web.coax`.

Moving an older settings page into the shell means drawing it as a section of
rows in place of its GPU page: a new section in `settings-web.coax`, its slug in
`SW_SLUGS`, its requests through the shell's (`cw_go`), and its folder's
`index.html` switched to the shell's module. What each needs beyond that:

- **Devices** (`devices.coax`): a list from `/api/auth/devices` with a remove
  per row. Rows and buttons only.
- **Security** (`security.coax`): passkeys, notices and "This wasn't me". Adding
  a passkey calls the browser's WebAuthn through the page's own script
  (`/login/key.js`); `dom-host.js` would need that call, as `host.js` has it.
- **Data and Delete** (`life.coax`): export, delete and its cancel, freeze and
  merge, some behind a challenge signed with your key or passkey (`/login/key.js`
  again), and a confirmation step before a delete.
- **Lightning** (`lightning.coax`): claim, change and turn off the address
  (`/api/lightning/address/*`), its own script (`lightning.js`), and a QR code,
  which needs a QR picture piece in the kit (the GPU page draws it from
  `coax/lib/qr`).
- **Wallet** and **Agents** (`wallet.coax`, `agents.coax`) were not in this
  pass's four; they are GPU pages with no script of their own.
