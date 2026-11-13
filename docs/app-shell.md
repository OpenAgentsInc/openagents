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
    editor (`/worlds/splice?edit`). Its + opens `/worlds/new`.
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
  beside it, either the chat or, at `/`, nothing.

## Why one module, and how navigation works

Each page of the site is its own document with its own module, so a link
rebuilds whatever is on the page, the side included. To keep the side still,
the pages it holds have to be drawn by one module that stays loaded while the
address changes.

The side and the chat's session list are one thing (the list is the side's
middle), so the shell is the chat's own module rather than a second module
that would have to be told the chat's sessions. At `/` it draws the side and
nothing else; at `/chat` (or any other address it is opened on, such as the
editor's foot) it draws the chat.

- The browser host (`coax/web/dom-host.js`) treats a module that imports
  `dom_go` as an app shell. It tells the module the address as it starts and
  after Back or Forward (event 13). A press on a link in the module is
  reported as a press; when the module answers with a pushed address, the host
  puts it in the address bar with the History API and the browser does not
  follow the link. Any other link is followed as usual, so a project, a world
  and Settings are ordinary page loads.
- Moving between `/` and `/chat` never rebuilds the side. A direct load of
  either address works: `/` is `index.html`, `/chat` is `chat/index.html`, and
  both load the same module.
- For links that still leave the shell, the side is the same module's drawing
  with its state kept in the browser (whether it is put away, which folds are
  shut, with the agents setting, through `dom_keep`), and the skin turns on cross-document view
  transitions (`@view-transition { navigation: auto }`, with
  `view-transition-name: side`), so the side stays still between two pages
  that both have it.

## The home page and its cost

`index.html` holds the public page's engine
(`<script type="module" src="/coaxial/host.js" data-app="newland">`) inside a
`<template>`, so none of it loads until one request to `/api/auth/account`
answers. Someone logged in gets the shell (dom-host and `chat-web.wasm`, no
engine); anyone else gets the template's script, which the release build still
finds and rewrites as before. A logged-out visitor pays that one small request
(at most 3 seconds when the API is slow) before the engine starts.

## The world editor

The editor (`/worlds/ID?edit`) shows the same side down its left. The editor's
shell (`metaverse-engine/editor/shell.coax`) gives it a column, `k_dock_side`,
with a slot, and the page's host (`coax/web/host.js`) opens a second copy of
the chat's module there with `path=/side`: the side alone, and every place in
it another page. The editor's own panes are the strip across the top, one side
on the right (what is selected, then the world's things and actors; the code,
the assets or the files take its place while one is open) and the agent's pane
at the foot, where the chat shows without a side. The scene's canvas is the
room they leave. Putting the side away narrows its column to the button that
brings it back.

## What each remaining page needs

- **The Projects and Worlds lists** live in the side's folds, so the side no
  longer links to `/projects` or `/worlds`. `/worlds` is a kit page beside the
  side (`apps/openagents-web/worlds-web.coax`, opened with
  `dom-host.js?app=worlds-web`). `/projects` is still the GPU galaxy
  (`projects.coax`) and holds the drop where a new project starts, which the
  Projects + opens.
- **The older settings pages and the login** (`/settings/security`, `/data`,
  `/delete`, `/devices`, `/wallet`, `/agents`, `/lightning`, and `/login`) are
  GPU pages too; the same applies. Settings itself is in the shell (below), and
  its Account section links to them.
- **A new page drawn by the browser** joins the shell by adding a view to the
  shell module (as the home page is) or, when it is large, by being opened into
  a slot beside the side with `dom-host.js?app=NAME&root=ID`, as the editor
  opens the chat in its foot.
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
- **The shell's own sections:** `apps/openagents-web/chat-settings.coax`
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
